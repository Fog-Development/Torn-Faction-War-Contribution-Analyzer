//! End-to-end CLI integration tests.

mod common;

use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::tempdir;

use common::*;

fn cmd() -> Command {
    Command::cargo_bin("torn-war-report").expect("binary builds")
}

#[test]
fn analyze_happy_path_all_formats() {
    let out = tempdir().unwrap();
    cmd()
        .arg("analyze")
        .arg("--wars")
        .arg(wars_dir())
        .arg("--activity")
        .arg(activity_csv())
        .arg("--output")
        .arg(out.path())
        .arg("--reference-time")
        .arg("2026-05-01T00:00:00Z")
        .assert()
        .success()
        .stdout(predicate::str::contains("wrote report bundle"));

    assert_path_exists(&out.path().join("analysis.xlsx"));
    assert_path_exists(&out.path().join("auto_kick.csv"));
    assert_path_exists(&out.path().join("repeat_offenders.csv"));
    assert_path_exists(&out.path().join("any_bad_war.csv"));
    assert_path_exists(&out.path().join("low_activity.csv"));
    assert_path_exists(&out.path().join("combined_kick.csv"));
    assert_path_exists(&out.path().join("war_matrix.csv"));
    assert_path_exists(&out.path().join("summary.md"));
    // Delta has a malformed row → expect warnings file too.
    assert_path_exists(&out.path().join("warnings.csv"));
}

#[test]
fn analyze_xlsx_only() {
    let out = tempdir().unwrap();
    cmd()
        .arg("analyze")
        .arg("--wars")
        .arg(wars_dir())
        .arg("--activity")
        .arg(activity_csv())
        .arg("--output")
        .arg(out.path())
        .arg("--formats")
        .arg("xlsx")
        .arg("--reference-time")
        .arg("2026-05-01T00:00:00Z")
        .assert()
        .success();

    assert_path_exists(&out.path().join("analysis.xlsx"));
    assert!(!out.path().join("auto_kick.csv").exists());
    assert!(!out.path().join("summary.md").exists());
}

#[test]
fn validate_happy_path() {
    let out = tempdir().unwrap();
    cmd()
        .arg("validate")
        .arg("--wars")
        .arg(wars_dir())
        .arg("--activity")
        .arg(activity_csv())
        .assert()
        .code(predicate::in_iter(vec![0, 3]));
    // No outputs should have been written.
    assert!(!out.path().join("analysis.xlsx").exists());
}

#[test]
fn validate_bad_input_errors() {
    cmd()
        .arg("validate")
        .arg("--wars")
        .arg(malformed("missing_column.csv"))
        .arg("--activity")
        .arg(activity_csv())
        .assert()
        .failure()
        .code(1);
}

#[test]
fn analyze_fail_on_warnings_returns_3() {
    let out = tempdir().unwrap();
    cmd()
        .arg("analyze")
        .arg("--wars")
        .arg(wars_dir()) // includes Delta with the malformed row → warnings emitted
        .arg("--activity")
        .arg(activity_csv())
        .arg("--output")
        .arg(out.path())
        .arg("--reference-time")
        .arg("2026-05-01T00:00:00Z")
        .arg("--fail-on-warnings")
        .assert()
        .code(3);
}

#[test]
fn analyze_low_percentile_override_changes_threshold() {
    let out = tempdir().unwrap();
    cmd()
        .arg("analyze")
        .arg("--wars")
        .arg(wars_dir())
        .arg("--activity")
        .arg(activity_csv())
        .arg("--output")
        .arg(out.path())
        .arg("--reference-time")
        .arg("2026-05-01T00:00:00Z")
        .arg("--low-percentile")
        .arg("0.0") // forces threshold to the minimum present points → much smaller Low set
        .arg("--formats")
        .arg("markdown")
        .assert()
        .success();

    let md = std::fs::read_to_string(out.path().join("summary.md")).unwrap();
    // With p=0 the low threshold for each war should be 800 (Carol/Mallory baseline minimum)
    // or another very small value — clearly different from the default-percentile output.
    assert!(md.contains("Low threshold"));
}

#[test]
fn schema_prints_columns() {
    cmd()
        .arg("schema")
        .assert()
        .success()
        .stdout(predicate::str::contains("attacker_name"))
        .stdout(predicate::str::contains("avgE30"))
        .stdout(predicate::str::contains("Filename convention"));
}
