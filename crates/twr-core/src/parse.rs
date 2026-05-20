//! CSV and filename parsing for war and activity inputs.

use chrono::{DateTime, NaiveDateTime, Utc};
use regex::Regex;
use std::collections::{BTreeMap, HashSet};
use std::fs::File;
use std::path::Path;
use thiserror::Error;

use crate::model::{MemberActivity, MemberId, War, WarParticipant};
use crate::warnings::{Warning, WarningCollector, WarningKind};

#[derive(Debug, Error)]
pub enum ParseError {
    #[error("io error reading {path}: {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("csv error in {path}: {source}")]
    Csv {
        path: String,
        #[source]
        source: csv::Error,
    },
    #[error("missing required column `{column}` in {path}")]
    MissingColumn { path: String, column: String },
    #[error("could not extract ISO-8601 datetime from filename `{filename}`")]
    FilenameDateMissing { filename: String },
    #[error("invalid datetime `{value}` in filename `{filename}`: {detail}")]
    InvalidDatetime {
        filename: String,
        value: String,
        detail: String,
    },
}

/// Required columns for a war CSV (case-sensitive).
pub const WAR_REQUIRED_COLUMNS: &[&str] =
    &["attacker_name", "attacker_id", "Hits", "Points", "WarHits"];

/// Optional war-CSV columns we recognise (preserved verbatim if present).
pub const WAR_OPTIONAL_COLUMNS: &[&str] = &[
    "NotWarHits",
    "Assist",
    "Retals",
    "Overseas",
    "BonusHits",
    "AvgFF",
    "Extra",
    "Value",
];

/// Required activity-CSV columns.
pub const ACTIVITY_REQUIRED_COLUMNS: &[&str] = &["Name", "Days", "avgE30"];

/// Recognised optional activity-CSV columns (preserved in `extras`).
pub const ACTIVITY_OPTIONAL_COLUMNS: &[&str] = &[
    "avgE7",
    "attacks7",
    "attacks30",
    "act30",
    "act7",
    "Donator?",
    "Property",
    "gym30",
    "gym7",
    "Xanax Daily",
    "Revs Last 30",
    "Contract Last 30",
    "Revive Skill",
];

/// Pull the `<prefix>_<ISO8601>.csv` portion apart and return `(display_name, datetime)`.
pub fn extract_war_datetime(filename: &str) -> Result<(String, DateTime<Utc>), ParseError> {
    // Strip optional directory + extension.
    let stem = Path::new(filename)
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| filename.to_string());

    // Regex matches the datetime portion (with `-` separating time components).
    let re = Regex::new(r"(\d{4}-\d{2}-\d{2}T\d{2}-\d{2}-\d{2}Z)").unwrap();
    let m = re
        .find(&stem)
        .ok_or_else(|| ParseError::FilenameDateMissing {
            filename: filename.to_string(),
        })?;
    let datetime_str = m.as_str();

    // Convert `-` between time components back to `:` for parsing.
    // Pattern: YYYY-MM-DDTHH-MM-SSZ → YYYY-MM-DDTHH:MM:SSZ
    let parseable = {
        let bytes = datetime_str.as_bytes();
        // positions are fixed: indices 13 and 16 are the `-` between H-M-S
        let mut s: Vec<u8> = bytes.to_vec();
        if s.len() == 20 {
            s[13] = b':';
            s[16] = b':';
        }
        String::from_utf8(s).expect("ascii only")
    };

    let naive =
        NaiveDateTime::parse_from_str(&parseable[..parseable.len() - 1], "%Y-%m-%dT%H:%M:%S")
            .map_err(|e| ParseError::InvalidDatetime {
                filename: filename.to_string(),
                value: datetime_str.to_string(),
                detail: e.to_string(),
            })?;
    let dt = DateTime::<Utc>::from_naive_utc_and_offset(naive, Utc);

    // Display name is everything before the matched datetime, with trailing `_` removed.
    let prefix_end = m.start();
    let prefix = stem[..prefix_end].trim_end_matches('_');
    let display_name = prefix.replace('_', " ").trim().to_string();

    Ok((display_name, dt))
}

/// Verify that all required columns appear in a CSV header row.
fn check_required_columns(
    headers: &csv::StringRecord,
    required: &[&str],
    path: &str,
) -> Result<(), ParseError> {
    let present: HashSet<&str> = headers.iter().collect();
    for col in required {
        if !present.contains(col) {
            return Err(ParseError::MissingColumn {
                path: path.to_string(),
                column: (*col).to_string(),
            });
        }
    }
    Ok(())
}

/// Parse a war CSV at `path`. Filename is used to derive the display name + start time.
pub fn parse_war_csv(path: &Path, warnings: &WarningCollector) -> Result<War, ParseError> {
    let path_str = path.display().to_string();
    let filename = path
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| path_str.clone());

    let (display_name, start_utc) = extract_war_datetime(&filename).inspect_err(|e| {
        if let ParseError::FilenameDateMissing { .. } = e {
            warnings.push(Warning::new(
                WarningKind::FilenameDateMissing,
                path_str.clone(),
                format!("filename `{}` lacks ISO-8601 datetime stamp", filename),
            ));
        }
    })?;

    let file = File::open(path).map_err(|e| ParseError::Io {
        path: path_str.clone(),
        source: e,
    })?;
    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(true)
        .from_reader(file);

    let headers = rdr
        .headers()
        .map_err(|e| ParseError::Csv {
            path: path_str.clone(),
            source: e,
        })?
        .clone();

    check_required_columns(&headers, WAR_REQUIRED_COLUMNS, &path_str)?;

    // Capture column indices for required fields.
    let idx_name = headers.iter().position(|h| h == "attacker_name").unwrap();
    let idx_id = headers.iter().position(|h| h == "attacker_id").unwrap();
    let idx_hits = headers.iter().position(|h| h == "Hits").unwrap();
    let idx_points = headers.iter().position(|h| h == "Points").unwrap();
    let idx_warhits = headers.iter().position(|h| h == "WarHits").unwrap();

    let mut participants: Vec<WarParticipant> = Vec::new();
    let mut seen_ids: HashSet<MemberId> = HashSet::new();

    for (row_no, rec) in rdr.records().enumerate() {
        let row_no = row_no + 2; // header is row 1, first data row = 2
        let rec = match rec {
            Ok(r) => r,
            Err(e) => {
                warnings.push(
                    Warning::new(
                        WarningKind::ParseError,
                        path_str.clone(),
                        format!("csv read error: {e}"),
                    )
                    .with_context(format!("row {row_no}")),
                );
                continue;
            }
        };

        let name_raw = rec.get(idx_name).unwrap_or("").trim();
        if name_raw.is_empty() {
            continue; // blank separator / trailing summary row
        }

        let id_raw = rec.get(idx_id).unwrap_or("").trim();
        if id_raw.is_empty() {
            warnings.push(
                Warning::new(
                    WarningKind::MissingAttackerId,
                    path_str.clone(),
                    format!("row for `{name_raw}` has empty attacker_id; skipping"),
                )
                .with_context(format!("row {row_no}")),
            );
            continue;
        }
        let id: MemberId = match id_raw.parse() {
            Ok(v) => v,
            Err(e) => {
                warnings.push(
                    Warning::new(
                        WarningKind::MalformedRow,
                        path_str.clone(),
                        format!("attacker_id `{id_raw}` not numeric for `{name_raw}`: {e}"),
                    )
                    .with_context(format!("row {row_no}")),
                );
                continue;
            }
        };

        let hits = match parse_u32(rec.get(idx_hits).unwrap_or("").trim()) {
            Ok(v) => v,
            Err(e) => {
                warnings.push(
                    Warning::new(
                        WarningKind::MalformedRow,
                        path_str.clone(),
                        format!("Hits not numeric for `{name_raw}`: {e}"),
                    )
                    .with_context(format!("row {row_no}")),
                );
                continue;
            }
        };
        let points = match parse_u32(rec.get(idx_points).unwrap_or("").trim()) {
            Ok(v) => v,
            Err(e) => {
                warnings.push(
                    Warning::new(
                        WarningKind::MalformedRow,
                        path_str.clone(),
                        format!("Points not numeric for `{name_raw}`: {e}"),
                    )
                    .with_context(format!("row {row_no}")),
                );
                continue;
            }
        };
        let war_hits = match parse_u32(rec.get(idx_warhits).unwrap_or("").trim()) {
            Ok(v) => v,
            Err(e) => {
                warnings.push(
                    Warning::new(
                        WarningKind::MalformedRow,
                        path_str.clone(),
                        format!("WarHits not numeric for `{name_raw}`: {e}"),
                    )
                    .with_context(format!("row {row_no}")),
                );
                continue;
            }
        };

        if !seen_ids.insert(id) {
            warnings.push(
                Warning::new(
                    WarningKind::DuplicateId,
                    path_str.clone(),
                    format!(
                        "attacker_id `{id}` already seen (current name `{name_raw}`); keeping first"
                    ),
                )
                .with_context(format!("row {row_no}")),
            );
            continue;
        }

        participants.push(WarParticipant {
            name: name_raw.to_string(),
            id: Some(id),
            hits,
            war_hits,
            points,
        });
    }

    Ok(War {
        display_name,
        start_utc,
        source_filename: filename,
        participants,
    })
}

/// Parse an activity CSV at `path`.
pub fn parse_activity_csv(
    path: &Path,
    warnings: &WarningCollector,
) -> Result<Vec<MemberActivity>, ParseError> {
    let path_str = path.display().to_string();
    let file = File::open(path).map_err(|e| ParseError::Io {
        path: path_str.clone(),
        source: e,
    })?;
    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(true)
        .from_reader(file);

    let headers = rdr
        .headers()
        .map_err(|e| ParseError::Csv {
            path: path_str.clone(),
            source: e,
        })?
        .clone();

    check_required_columns(&headers, ACTIVITY_REQUIRED_COLUMNS, &path_str)?;

    let idx_name = headers.iter().position(|h| h == "Name").unwrap();
    let idx_days = headers.iter().position(|h| h == "Days").unwrap();
    let idx_avg = headers.iter().position(|h| h == "avgE30").unwrap();

    // Preserve every other column verbatim in `extras`.
    let preserved_cols: Vec<(usize, String)> = headers
        .iter()
        .enumerate()
        .filter_map(|(i, h)| {
            if h == "Name" || h == "Days" || h == "avgE30" {
                None
            } else {
                Some((i, h.to_string()))
            }
        })
        .collect();

    let mut activity: Vec<MemberActivity> = Vec::new();
    let mut seen_names: HashSet<String> = HashSet::new();

    for (row_no, rec) in rdr.records().enumerate() {
        let row_no = row_no + 2;
        let rec = match rec {
            Ok(r) => r,
            Err(e) => {
                warnings.push(
                    Warning::new(
                        WarningKind::ParseError,
                        path_str.clone(),
                        format!("csv read error: {e}"),
                    )
                    .with_context(format!("row {row_no}")),
                );
                continue;
            }
        };

        let name = rec.get(idx_name).unwrap_or("").trim().to_string();
        if name.is_empty() {
            continue;
        }

        if !seen_names.insert(name.clone()) {
            warnings.push(
                Warning::new(
                    WarningKind::AmbiguousMemberName,
                    path_str.clone(),
                    format!("duplicate activity row for `{name}`; keeping first"),
                )
                .with_context(format!("row {row_no}")),
            );
            continue;
        }

        let days_raw = rec.get(idx_days).unwrap_or("").trim();
        let days = if days_raw.is_empty() {
            warnings.push(
                Warning::new(
                    WarningKind::MalformedRow,
                    path_str.clone(),
                    format!("Days missing for `{name}`; member excluded from activity analysis"),
                )
                .with_context(format!("row {row_no}")),
            );
            None
        } else {
            match parse_u32(days_raw) {
                Ok(v) => Some(v),
                Err(e) => {
                    warnings.push(
                        Warning::new(
                            WarningKind::MalformedRow,
                            path_str.clone(),
                            format!("Days not numeric for `{name}`: {e}"),
                        )
                        .with_context(format!("row {row_no}")),
                    );
                    None
                }
            }
        };

        let avg_raw = rec.get(idx_avg).unwrap_or("").trim();
        let avg_e30 = if avg_raw.is_empty() {
            None
        } else {
            match avg_raw.parse::<f64>() {
                Ok(v) => Some(v),
                Err(e) => {
                    warnings.push(
                        Warning::new(
                            WarningKind::MalformedRow,
                            path_str.clone(),
                            format!("avgE30 not numeric for `{name}`: {e}"),
                        )
                        .with_context(format!("row {row_no}")),
                    );
                    None
                }
            }
        };

        let mut extras: BTreeMap<String, String> = BTreeMap::new();
        for (i, col) in &preserved_cols {
            if let Some(v) = rec.get(*i) {
                extras.insert(col.clone(), v.trim().to_string());
            }
        }

        activity.push(MemberActivity {
            name,
            days,
            avg_e30,
            extras,
        });
    }

    Ok(activity)
}

fn parse_u32(s: &str) -> Result<u32, std::num::ParseIntError> {
    // Tolerate plain integers and bare zero. Float strings like "0" pass; "10.5" fails.
    s.parse::<u32>()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_csv(content: &str, filename: &str) -> std::path::PathBuf {
        // Honour the requested filename (so we can drive datetime extraction in tests)
        // by creating the file inside a temp dir under that name.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(filename);
        std::fs::write(&path, content).unwrap();
        // Leak the tempdir so the path remains valid for the duration of the test.
        std::mem::forget(dir);
        path
    }

    #[test]
    fn extract_war_datetime_valid() {
        let cases = [
            (
                "TM_vs_Alpha_2026-01-01T00-00-00Z.csv",
                "TM vs Alpha",
                "2026-01-01T00:00:00Z",
            ),
            (
                "MyFaction_vs_Other_Faction_2025-12-31T23-59-59Z.csv",
                "MyFaction vs Other Faction",
                "2025-12-31T23:59:59Z",
            ),
            (
                "war_2024-07-04T12-30-45Z.csv",
                "war",
                "2024-07-04T12:30:45Z",
            ),
        ];
        for (filename, want_name, want_dt) in cases {
            let (name, dt) = extract_war_datetime(filename).expect(filename);
            assert_eq!(name, want_name, "name for {filename}");
            assert_eq!(
                dt.to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
                want_dt
            );
        }
    }

    #[test]
    fn extract_war_datetime_invalid() {
        for bad in [
            "no_date_here.csv",
            "TM_vs_Alpha.csv",
            "TM_vs_Alpha_2026-01-01.csv",
            "TM_vs_Alpha_2026-01-01T00:00:00Z.csv", // wrong separators
        ] {
            assert!(
                matches!(
                    extract_war_datetime(bad),
                    Err(ParseError::FilenameDateMissing { .. })
                ),
                "{bad} should be rejected"
            );
        }
    }

    #[test]
    fn parse_war_csv_clean() {
        let csv =
            "attacker_name,attacker_id,Hits,Points,WarHits\nAlice,1,10,1000,9\nBob,2,5,500,4\n";
        let path = write_csv(csv, "Foo_2026-01-01T00-00-00Z.csv");
        let w = WarningCollector::new();
        let war = parse_war_csv(&path, &w).unwrap();
        assert_eq!(war.display_name, "Foo");
        assert_eq!(war.participants.len(), 2);
        assert_eq!(war.participants[0].name, "Alice");
        assert_eq!(war.participants[0].points, 1000);
        assert!(w.is_empty());
    }

    #[test]
    fn parse_war_csv_blank_trailing_rows() {
        let csv = "attacker_name,attacker_id,Hits,Points,WarHits\nAlice,1,10,1000,9\n,,,,\n,,,,\n";
        let path = write_csv(csv, "Foo_2026-01-01T00-00-00Z.csv");
        let w = WarningCollector::new();
        let war = parse_war_csv(&path, &w).unwrap();
        assert_eq!(war.participants.len(), 1);
        assert!(w.is_empty(), "blank rows should not warn");
    }

    #[test]
    fn parse_war_csv_malformed_row_warns() {
        let csv =
            "attacker_name,attacker_id,Hits,Points,WarHits\nAlice,1,10,oops,9\nBob,2,5,500,4\n";
        let path = write_csv(csv, "Foo_2026-01-01T00-00-00Z.csv");
        let w = WarningCollector::new();
        let war = parse_war_csv(&path, &w).unwrap();
        assert_eq!(war.participants.len(), 1, "Alice's bad row is dropped");
        assert_eq!(war.participants[0].name, "Bob");
        let warnings = w.snapshot();
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].kind, WarningKind::MalformedRow);
    }

    #[test]
    fn parse_war_csv_duplicate_id_warns_keeps_first() {
        let csv = "attacker_name,attacker_id,Hits,Points,WarHits\nAlice,1,10,1000,9\nAlias,1,20,2000,18\n";
        let path = write_csv(csv, "Foo_2026-01-01T00-00-00Z.csv");
        let w = WarningCollector::new();
        let war = parse_war_csv(&path, &w).unwrap();
        assert_eq!(war.participants.len(), 1);
        assert_eq!(war.participants[0].name, "Alice");
        let warnings = w.snapshot();
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].kind, WarningKind::DuplicateId);
    }

    #[test]
    fn parse_war_csv_missing_attacker_id_warns() {
        let csv =
            "attacker_name,attacker_id,Hits,Points,WarHits\nAlice,,10,1000,9\nBob,2,5,500,4\n";
        let path = write_csv(csv, "Foo_2026-01-01T00-00-00Z.csv");
        let w = WarningCollector::new();
        let war = parse_war_csv(&path, &w).unwrap();
        assert_eq!(war.participants.len(), 1);
        assert_eq!(war.participants[0].name, "Bob");
        let warnings = w.snapshot();
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].kind, WarningKind::MissingAttackerId);
    }

    #[test]
    fn parse_war_csv_missing_required_column_errors() {
        let csv = "attacker_name,attacker_id,Hits,WarHits\nAlice,1,10,9\n";
        let path = write_csv(csv, "Foo_2026-01-01T00-00-00Z.csv");
        let w = WarningCollector::new();
        let err = parse_war_csv(&path, &w).unwrap_err();
        assert!(matches!(err, ParseError::MissingColumn { ref column, .. } if column == "Points"));
    }

    #[test]
    fn parse_activity_csv_clean() {
        let csv = "Name,Days,avgE30,avgE7\nAlice,365,1200,1300\nBob,200,800,850\n";
        let path = write_csv(csv, "activity.csv");
        let w = WarningCollector::new();
        let rows = parse_activity_csv(&path, &w).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].name, "Alice");
        assert_eq!(rows[0].days, Some(365));
        assert_eq!(rows[0].avg_e30, Some(1200.0));
        assert_eq!(rows[0].extras.get("avgE7"), Some(&"1300".to_string()));
        assert!(w.is_empty());
    }

    #[test]
    fn parse_activity_csv_blank_rows_skipped() {
        let csv = "Name,Days,avgE30\nAlice,365,1200\n,,\nBob,200,800\n";
        let path = write_csv(csv, "activity.csv");
        let w = WarningCollector::new();
        let rows = parse_activity_csv(&path, &w).unwrap();
        assert_eq!(rows.len(), 2);
        assert!(w.is_empty());
    }

    #[test]
    fn parse_activity_csv_missing_days_warns() {
        let csv = "Name,Days,avgE30\nAlice,,1200\n";
        let path = write_csv(csv, "activity.csv");
        let w = WarningCollector::new();
        let rows = parse_activity_csv(&path, &w).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].days, None);
        let warnings = w.snapshot();
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].kind, WarningKind::MalformedRow);
    }

    #[test]
    fn parse_activity_csv_bad_avg_warns() {
        let csv = "Name,Days,avgE30\nAlice,365,not_a_number\n";
        let path = write_csv(csv, "activity.csv");
        let w = WarningCollector::new();
        let rows = parse_activity_csv(&path, &w).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].avg_e30, None);
        assert_eq!(w.snapshot().len(), 1);
    }

    #[test]
    fn parse_activity_csv_missing_required_column_errors() {
        let csv = "Name,avgE30\nAlice,1200\n";
        let path = write_csv(csv, "activity.csv");
        let w = WarningCollector::new();
        let err = parse_activity_csv(&path, &w).unwrap_err();
        assert!(matches!(err, ParseError::MissingColumn { ref column, .. } if column == "Days"));
    }

    #[test]
    fn parse_activity_csv_duplicate_name_warns() {
        let csv = "Name,Days,avgE30\nAlice,365,1200\nAlice,400,1500\n";
        let path = write_csv(csv, "activity.csv");
        let w = WarningCollector::new();
        let rows = parse_activity_csv(&path, &w).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].days, Some(365));
        assert_eq!(w.snapshot()[0].kind, WarningKind::AmbiguousMemberName);
    }
}
