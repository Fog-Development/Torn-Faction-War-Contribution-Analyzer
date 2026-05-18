//! Shared helpers for integration tests.

#![allow(dead_code)]

use std::path::{Path, PathBuf};

pub fn workspace_root() -> PathBuf {
    // CARGO_MANIFEST_DIR for twr-cli = <workspace>/crates/twr-cli
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

pub fn fixtures_dir() -> PathBuf {
    workspace_root().join("fixtures")
}

pub fn wars_dir() -> PathBuf {
    fixtures_dir().join("wars")
}

pub fn activity_csv() -> PathBuf {
    fixtures_dir().join("activity").join("Member_Activity.csv")
}

pub fn malformed(name: &str) -> PathBuf {
    fixtures_dir().join("wars").join("malformed").join(name)
}

pub fn assert_path_exists(p: &Path) {
    assert!(p.exists(), "expected path to exist: {}", p.display());
}
