//! End-to-end pipeline test against the workspace fixtures.

use chrono::{TimeZone, Utc};
use std::path::PathBuf;

use twr_core::{analyze_from_files_at, Config};

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("fixtures")
}

#[test]
fn full_pipeline_against_fixtures() {
    let wars_dir = fixtures_dir().join("wars");
    let activity = fixtures_dir().join("activity").join("Member_Activity.csv");

    let mut war_paths: Vec<PathBuf> = std::fs::read_dir(&wars_dir)
        .unwrap()
        .filter_map(|e| {
            let p = e.unwrap().path();
            if p.is_file() && p.extension().map(|x| x == "csv").unwrap_or(false) {
                Some(p)
            } else {
                None
            }
        })
        .collect();
    war_paths.sort();
    let refs: Vec<&std::path::Path> = war_paths.iter().map(|p| p.as_path()).collect();

    let cfg = Config::default();
    let reference_time = Utc.with_ymd_and_hms(2026, 5, 1, 0, 0, 0).unwrap();
    let report = analyze_from_files_at(&refs, &activity, &cfg, reference_time).expect("pipeline");

    // We expect 4 wars, in chronological order.
    assert_eq!(report.wars.len(), 4);
    assert_eq!(report.wars[0].display_name, "TM vs Alpha");
    assert_eq!(report.wars[3].display_name, "TM vs Delta");

    // Carol scored 0 in Beta and Delta → auto-kick.
    assert!(report.auto_kick.iter().any(|n| n == "Carol"));

    // Heidi/Mallory have multiple Low + Zero → repeat offenders, on combined kick (low avgE30 too).
    assert!(report.repeat_offenders.iter().any(|n| n == "Heidi"));
    assert!(report.combined_kick.iter().any(|n| n == "Heidi"));

    // Trent and Uma have Days < 7 → flagged as insufficient tenure (warnings).
    let warn_text: String = report
        .warnings
        .iter()
        .map(|w| w.detail.clone())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(warn_text.contains("Trent"));
    assert!(warn_text.contains("Uma"));

    // Ghost1 appears in Delta but not in activity CSV → MissingActivityRecord warning,
    // and excluded from members entirely (treated as an ex-member).
    assert!(warn_text.contains("Ghost1"));
    assert!(!report.members.iter().any(|m| m.name == "Ghost1"));

    // BadRow had malformed Points → MalformedRow warning emitted, member not in summary.
    assert!(warn_text.contains("BadRow"));
    assert!(!report.members.iter().any(|m| m.name == "BadRow"));

    // Ruth has no war participation → present_count == 0 across the board.
    let ruth = report.members.iter().find(|m| m.name == "Ruth").unwrap();
    assert_eq!(ruth.present_count, 0);
}
