//! War-presence detection, per-war classification, and member rollups.

use chrono::{DateTime, Utc};
use std::collections::{BTreeMap, HashMap};
use thiserror::Error;

use crate::config::Config;
use crate::model::{
    AnalysisReport, MemberActivity, MemberId, MemberSummary, MemberWarResult, War, WarCategory,
};
use crate::warnings::{Warning, WarningCollector, WarningKind};

#[derive(Debug, Error)]
pub enum AnalysisError {
    #[error("invalid percentile {0}: must be in [0, 1]")]
    InvalidPercentile(f64),
}

/// Compute the low-points threshold for a single war.
///
/// Pulls all points from `present` members whose `points > 0`, sorts ascending,
/// and returns the value at the configured percentile using linear interpolation.
/// Returns `0.0` if no non-zero participants are present.
pub fn compute_threshold(present_points: &[u32], percentile: f64) -> f64 {
    let mut values: Vec<f64> = present_points
        .iter()
        .copied()
        .filter(|p| *p > 0)
        .map(|p| p as f64)
        .collect();
    if values.is_empty() {
        return 0.0;
    }
    values.sort_by(|a, b| a.partial_cmp(b).unwrap());

    let n = values.len();
    if n == 1 {
        return values[0];
    }
    let idx = percentile * (n - 1) as f64;
    let lo = idx.floor() as usize;
    let hi = idx.ceil() as usize;
    if lo == hi {
        values[lo]
    } else {
        let frac = idx - lo as f64;
        values[lo] + frac * (values[hi] - values[lo])
    }
}

/// Determine whether a member was "present" for a given war given their tenure (days) and
/// the war's elapsed days. The rule is `present = days > days_ago` (strict).
/// If `days` is `None` but the member appeared in the war CSV, treat as present.
pub fn is_present(days: Option<u32>, days_ago: i64, listed_in_csv: bool) -> bool {
    match days {
        Some(d) => (d as i64) > days_ago,
        None => listed_in_csv,
    }
}

/// Classify a single war-participation observation.
pub fn classify_member(points: u32, present: bool, low_threshold: f64) -> WarCategory {
    if !present {
        return WarCategory::Excluded;
    }
    if points == 0 {
        return WarCategory::Zero;
    }
    if (points as f64) <= low_threshold {
        WarCategory::Low
    } else {
        WarCategory::Ok
    }
}

/// Run the full analysis pipeline.
pub fn analyze(
    wars: Vec<War>,
    activity: Vec<MemberActivity>,
    config: &Config,
) -> Result<AnalysisReport, AnalysisError> {
    let warnings = WarningCollector::new();
    analyze_with_collector(wars, activity, config, &warnings, Utc::now())
}

/// Internal flavour that lets the caller inject a `WarningCollector` and a deterministic
/// reference time (useful for tests and `analyze_from_files`).
pub fn analyze_with_collector(
    wars: Vec<War>,
    activity: Vec<MemberActivity>,
    config: &Config,
    warnings: &WarningCollector,
    reference_time: DateTime<Utc>,
) -> Result<AnalysisReport, AnalysisError> {
    if !(0.0..=1.0).contains(&config.analysis.low_percentile) {
        return Err(AnalysisError::InvalidPercentile(
            config.analysis.low_percentile,
        ));
    }

    // Validate name<->id consistency across wars.
    {
        let mut id_to_name: HashMap<MemberId, String> = HashMap::new();
        for war in &wars {
            for p in &war.participants {
                if let Some(id) = p.id {
                    if let Some(prev) = id_to_name.get(&id) {
                        if prev != &p.name {
                            warnings.push(
                                Warning::new(
                                    WarningKind::InconsistentIdName,
                                    "analysis",
                                    format!(
                                        "attacker_id {id} appears as `{prev}` and `{}`",
                                        p.name
                                    ),
                                )
                                .with_context(war.source_filename.clone()),
                            );
                        }
                    } else {
                        id_to_name.insert(id, p.name.clone());
                    }
                }
            }
        }
    }

    // Build name -> activity lookup. Names are case-sensitive per spec.
    let activity_by_name: BTreeMap<String, MemberActivity> = activity
        .iter()
        .cloned()
        .map(|a| (a.name.clone(), a))
        .collect();

    // Compute days_ago for each war.
    let days_ago: Vec<i64> = wars
        .iter()
        .map(|w| (reference_time - w.start_utc).num_days())
        .collect();

    // Identify war-CSV names that have no activity record — these are members who have
    // since left or been kicked. Warn once per name and exclude them from all analysis.
    let act_names: std::collections::HashSet<&str> =
        activity.iter().map(|a| a.name.as_str()).collect();
    {
        let mut warned: std::collections::HashSet<String> = std::collections::HashSet::new();
        for war in &wars {
            for p in &war.participants {
                if !act_names.contains(p.name.as_str()) && warned.insert(p.name.clone()) {
                    warnings.push(
                        Warning::new(
                            WarningKind::MissingActivityRecord,
                            "analysis",
                            format!(
                                "member `{}` appears in war `{}` but has no activity record; \
                                 they have likely left or been kicked since that war and will \
                                 be excluded from all reports",
                                p.name, war.display_name
                            ),
                        )
                        .with_context(p.name.clone()),
                    );
                }
            }
        }
    }

    // Member universe = activity list only. War-CSV-only names (no activity record) are
    // treated as ex-members and excluded from all output lists.
    let member_names: Vec<String> = activity.iter().map(|a| a.name.clone()).collect();

    // Compute per-war low thresholds. Only count members who have an activity record
    // and are considered present — ex-members are excluded from the threshold too.
    let mut war_thresholds: Vec<f64> = Vec::with_capacity(wars.len());
    for (wi, war) in wars.iter().enumerate() {
        let mut present_points: Vec<u32> = Vec::new();
        for p in &war.participants {
            // Skip ex-members (no activity record) entirely.
            if !act_names.contains(p.name.as_str()) {
                continue;
            }
            let act_days = activity_by_name.get(&p.name).and_then(|a| a.days);
            if is_present(act_days, days_ago[wi], true) {
                present_points.push(p.points);
            }
        }
        war_thresholds.push(compute_threshold(
            &present_points,
            config.analysis.low_percentile,
        ));
    }

    // Build per-member summaries.
    let mut members: Vec<MemberSummary> = Vec::with_capacity(member_names.len());
    for name in &member_names {
        let activity_row = activity_by_name.get(name);
        let days = activity_row.and_then(|a| a.days);
        let avg_e30 = activity_row.and_then(|a| a.avg_e30);

        let mut war_results: Vec<MemberWarResult> = Vec::with_capacity(wars.len());
        let mut present_count = 0u32;
        let mut zero_count = 0u32;
        let mut low_count = 0u32;
        let mut points_sum: u64 = 0;
        let mut points_n: u32 = 0;

        for (wi, war) in wars.iter().enumerate() {
            let participant = war.participants.iter().find(|p| p.name == *name);
            let (listed, points, hits) = match participant {
                Some(p) => (true, p.points, p.hits),
                None => (false, 0, 0),
            };

            // Presence = member was in faction when war occurred.
            // If Days is known: days > days_ago (strict). If Days unknown but listed: present.
            // Not being listed in the war CSV does NOT mean absent — members who earned 0 points
            // may simply not appear in the export.
            let present = is_present(days, days_ago[wi], listed);

            let category = classify_member(points, present, war_thresholds[wi]);

            match category {
                WarCategory::Zero => {
                    present_count += 1;
                    zero_count += 1;
                }
                WarCategory::Low => {
                    present_count += 1;
                    low_count += 1;
                    points_sum += points as u64;
                    points_n += 1;
                }
                WarCategory::Ok => {
                    present_count += 1;
                    points_sum += points as u64;
                    points_n += 1;
                }
                WarCategory::Excluded => {}
            }

            war_results.push(MemberWarResult {
                war_index: wi,
                category,
                points,
                hits,
                listed_in_csv: listed,
            });
        }

        let avg_points = if points_n == 0 {
            0.0
        } else {
            points_sum as f64 / points_n as f64
        };

        members.push(MemberSummary {
            name: name.clone(),
            days,
            avg_e30,
            wars: war_results,
            present_count,
            zero_count,
            low_count,
            poor_count: zero_count + low_count,
            avg_points,
        });
    }

    // Build lists.
    let zero_threshold = config.analysis.zero_war_kick_threshold;
    let poor_threshold = config.analysis.poor_war_threshold;
    let act_threshold = config.analysis.activity_threshold;
    let min_days = config.analysis.min_days_for_activity;

    let auto_kick: Vec<String> = members
        .iter()
        .filter(|m| m.zero_count >= zero_threshold)
        .map(|m| m.name.clone())
        .collect();
    let repeat_offenders: Vec<String> = members
        .iter()
        .filter(|m| m.poor_count >= poor_threshold)
        .map(|m| m.name.clone())
        .collect();
    let any_bad_war: Vec<String> = members
        .iter()
        .filter(|m| m.zero_count > 0 || m.low_count > 0)
        .map(|m| m.name.clone())
        .collect();
    let low_activity: Vec<String> = members
        .iter()
        .filter(|m| match (m.days, m.avg_e30) {
            (Some(d), Some(a)) => d >= min_days && a < act_threshold,
            _ => false,
        })
        .map(|m| m.name.clone())
        .collect();
    let combined_kick: Vec<String> = members
        .iter()
        .filter(|m| {
            let on_repeat = m.poor_count >= poor_threshold;
            let on_low_act = match (m.days, m.avg_e30) {
                (Some(d), Some(a)) => d >= min_days && a < act_threshold,
                _ => false,
            };
            on_repeat && on_low_act
        })
        .map(|m| m.name.clone())
        .collect();

    // Members with Days < min_days → flag insufficient tenure (advisory).
    for m in &members {
        if let Some(d) = m.days {
            if d < min_days {
                warnings.push(
                    Warning::new(
                        WarningKind::InsufficientTenure,
                        "analysis",
                        format!(
                            "member `{}` has Days={} (< min_days={}); excluded from activity analysis",
                            m.name, d, min_days
                        ),
                    )
                    .with_context(m.name.clone()),
                );
            }
        }
    }

    Ok(AnalysisReport {
        reference_time,
        config: config.clone(),
        wars,
        war_thresholds,
        members,
        auto_kick,
        repeat_offenders,
        any_bad_war,
        low_activity,
        combined_kick,
        warnings: warnings.snapshot(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::model::WarParticipant;
    use chrono::TimeZone;

    fn make_war(name: &str, dt: DateTime<Utc>, participants: Vec<(&str, u64, u32, u32)>) -> War {
        War {
            display_name: name.to_string(),
            start_utc: dt,
            source_filename: format!("{name}.csv"),
            participants: participants
                .into_iter()
                .map(|(n, id, points, hits)| WarParticipant {
                    name: n.to_string(),
                    id: Some(id),
                    hits,
                    war_hits: hits,
                    points,
                })
                .collect(),
        }
    }

    #[test]
    fn compute_threshold_p20_classic() {
        let values: Vec<u32> = vec![10, 20, 30, 40, 50, 60, 70, 80, 90, 100];
        let t = compute_threshold(&values, 0.20);
        // index = 0.2 * 9 = 1.8 → between values[1]=20 and values[2]=30 → 20 + 0.8*10 = 28
        assert!((t - 28.0).abs() < 1e-9, "expected 28.0, got {t}");
    }

    #[test]
    fn compute_threshold_excludes_zero() {
        let values: Vec<u32> = vec![0, 0, 10, 20, 30, 40, 50, 60, 70, 80, 90, 100];
        let t = compute_threshold(&values, 0.20);
        assert!((t - 28.0).abs() < 1e-9);
    }

    #[test]
    fn compute_threshold_empty() {
        assert_eq!(compute_threshold(&[], 0.20), 0.0);
        assert_eq!(compute_threshold(&[0, 0, 0], 0.20), 0.0);
    }

    #[test]
    fn compute_threshold_single() {
        assert_eq!(compute_threshold(&[42], 0.20), 42.0);
    }

    #[test]
    fn classify_member_all_outcomes() {
        assert_eq!(classify_member(0, false, 10.0), WarCategory::Excluded);
        assert_eq!(classify_member(100, false, 10.0), WarCategory::Excluded);
        assert_eq!(classify_member(0, true, 10.0), WarCategory::Zero);
        assert_eq!(classify_member(5, true, 10.0), WarCategory::Low);
        assert_eq!(classify_member(10, true, 10.0), WarCategory::Low); // <=
        assert_eq!(classify_member(11, true, 10.0), WarCategory::Ok);
    }

    #[test]
    fn is_present_edge_case_strict_gt() {
        // days == days_ago → NOT present (rule is strict >)
        assert!(!is_present(Some(30), 30, true));
        assert!(is_present(Some(31), 30, true));
        assert!(!is_present(Some(0), 30, true));
        // missing days but listed → present
        assert!(is_present(None, 30, true));
        assert!(!is_present(None, 30, false));
    }

    #[test]
    fn analyze_basic_pipeline() {
        let war_a = make_war(
            "Alpha",
            Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap(),
            vec![
                ("Alice", 1, 1000, 10),
                ("Bob", 2, 500, 5),
                ("Carol", 3, 0, 0),
            ],
        );
        let war_b = make_war(
            "Beta",
            Utc.with_ymd_and_hms(2026, 2, 1, 0, 0, 0).unwrap(),
            vec![("Alice", 1, 1100, 11), ("Bob", 2, 0, 0), ("Carol", 3, 0, 0)],
        );
        let activity = vec![
            MemberActivity {
                name: "Alice".into(),
                days: Some(365),
                avg_e30: Some(1200.0),
                extras: BTreeMap::new(),
            },
            MemberActivity {
                name: "Bob".into(),
                days: Some(365),
                avg_e30: Some(500.0),
                extras: BTreeMap::new(),
            },
            MemberActivity {
                name: "Carol".into(),
                days: Some(365),
                avg_e30: Some(900.0),
                extras: BTreeMap::new(),
            },
        ];
        let cfg = Config::default();
        let warnings = WarningCollector::new();
        let ref_time = Utc.with_ymd_and_hms(2026, 5, 1, 0, 0, 0).unwrap();
        let report =
            analyze_with_collector(vec![war_a, war_b], activity, &cfg, &warnings, ref_time)
                .unwrap();

        // Carol has 2 zeros → auto-kick
        assert!(report.auto_kick.contains(&"Carol".to_string()));
        // Bob has 1 zero → not on auto-kick by default threshold of 2
        assert!(!report.auto_kick.contains(&"Bob".to_string()));
        // Bob (500) is on low_activity (avgE30 500 < 750, days >= 7)
        assert!(report.low_activity.contains(&"Bob".to_string()));
        // Alice clean
        assert!(!report.auto_kick.contains(&"Alice".to_string()));
        assert!(!report.repeat_offenders.contains(&"Alice".to_string()));
        // Carol has poor_count=2 and Days=365 (not low activity since avg=900) → not on combined_kick
        assert!(!report.combined_kick.contains(&"Carol".to_string()));
    }

    #[test]
    fn analyze_insufficient_tenure_excludes() {
        let war = make_war(
            "Alpha",
            Utc.with_ymd_and_hms(2026, 4, 1, 0, 0, 0).unwrap(),
            vec![("NewGuy", 1, 0, 0), ("Vet", 2, 1000, 10)],
        );
        let activity = vec![
            MemberActivity {
                name: "NewGuy".into(),
                days: Some(3),
                avg_e30: Some(100.0),
                extras: BTreeMap::new(),
            },
            MemberActivity {
                name: "Vet".into(),
                days: Some(365),
                avg_e30: Some(1200.0),
                extras: BTreeMap::new(),
            },
        ];
        let cfg = Config::default();
        let warnings = WarningCollector::new();
        let report = analyze_with_collector(
            vec![war],
            activity,
            &cfg,
            &warnings,
            Utc.with_ymd_and_hms(2026, 5, 1, 0, 0, 0).unwrap(),
        )
        .unwrap();
        let newguy = report.members.iter().find(|m| m.name == "NewGuy").unwrap();
        assert_eq!(newguy.wars[0].category, WarCategory::Excluded);
        assert_eq!(newguy.zero_count, 0);
        // Low activity list requires Days >= min_days, so NewGuy excluded
        assert!(!report.low_activity.contains(&"NewGuy".to_string()));
    }
}
