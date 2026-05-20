//! Core analysis engine for the Torn faction-war contribution analyzer.
//!
//! Provides parsing for war + activity CSVs, the analysis algorithm itself,
//! layered configuration, and a shared warning-collection mechanism.

pub mod analysis;
pub mod config;
pub mod model;
pub mod parse;
pub mod warnings;

pub use analysis::{analyze as analyze_in_memory, analyze_with_collector, AnalysisError};
pub use config::{
    AnalysisConfig, AnalysisOverlay, Config, ConfigError, ConfigOverlay, OutputConfig,
    OutputOverlay,
};
pub use model::{
    AnalysisReport, MemberActivity, MemberId, MemberName, MemberSummary, MemberWarResult, War,
    WarCategory, WarParticipant,
};
pub use parse::{
    extract_war_datetime, parse_activity_csv, parse_war_csv, ParseError, ACTIVITY_OPTIONAL_COLUMNS,
    ACTIVITY_REQUIRED_COLUMNS, WAR_OPTIONAL_COLUMNS, WAR_REQUIRED_COLUMNS,
};
pub use warnings::{Warning, WarningCollector, WarningKind};

use chrono::Utc;
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PipelineError {
    #[error(transparent)]
    Parse(#[from] ParseError),
    #[error(transparent)]
    Analysis(#[from] AnalysisError),
}

/// Top-level convenience: read all war CSVs and the activity CSV from disk, then run analysis.
///
/// Uses `chrono::Utc::now()` as the reference time. Warnings emitted during parsing and
/// analysis are gathered into the returned [`AnalysisReport`].
pub fn analyze_from_files(
    war_paths: &[&Path],
    activity_path: &Path,
    config: &Config,
) -> Result<AnalysisReport, PipelineError> {
    analyze_from_files_at(war_paths, activity_path, config, Utc::now())
}

/// Like [`analyze_from_files`] but uses a caller-supplied reference time (for tests / CLI).
pub fn analyze_from_files_at(
    war_paths: &[&Path],
    activity_path: &Path,
    config: &Config,
    reference_time: chrono::DateTime<Utc>,
) -> Result<AnalysisReport, PipelineError> {
    let warnings = WarningCollector::new();

    let mut wars: Vec<War> = Vec::with_capacity(war_paths.len());
    for p in war_paths {
        wars.push(parse_war_csv(p, &warnings)?);
    }
    // Deterministic ordering by start_utc, then by source_filename.
    wars.sort_by(|a, b| {
        a.start_utc
            .cmp(&b.start_utc)
            .then_with(|| a.source_filename.cmp(&b.source_filename))
    });

    let activity = parse_activity_csv(activity_path, &warnings)?;

    let report = analyze_with_collector(wars, activity, config, &warnings, reference_time)?;
    Ok(report)
}
