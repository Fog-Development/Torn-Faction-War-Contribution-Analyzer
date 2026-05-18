//! Markdown-output snapshot tests using `insta`.

mod common;

use chrono::{TimeZone, Utc};
use std::path::PathBuf;

use twr_core::{analyze_from_files_at, Config};
use twr_report::markdown::render_for_snapshot;

use common::*;

#[test]
fn summary_markdown_snapshot() {
    let wars_dir = wars_dir();
    let activity = activity_csv();

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
    let report = analyze_from_files_at(&refs, &activity, &cfg, reference_time)
        .expect("pipeline against fixtures");

    let rendered = render_for_snapshot(&report);
    // Strip absolute paths from warnings (vary between OSes / checkout locations).
    let normalised = scrub_paths(&rendered);
    insta::assert_snapshot!(normalised);
}

/// Replace OS-specific absolute paths to fixture files with a stable token.
fn scrub_paths(s: &str) -> String {
    // Regex over both `/` and `\` separators. We rewrite anything ending in
    //   .../fixtures/wars/<file>.csv  →  <FIXTURES>/<file>.csv
    let re = regex::Regex::new(r#"\S*?fixtures[\\/]wars[\\/](?:malformed[\\/])?([^\s\\/|]+\.csv)"#)
        .unwrap();
    re.replace_all(s, "<FIXTURES>/$1").into_owned()
}
