//! NDJSON event types for `--emit=json` mode.
//!
//! One JSON line per event, flushed to stdout. Stderr remains human-readable.
//! This is the stable contract between the CLI and the GUI; rename fields carefully.

use serde::{Deserialize, Serialize};
use twr_core::config::AnalysisConfig;

/// Emitted as NDJSON on stdout when `--emit=json`.
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Event {
    /// First line: resolved config + reference time.
    Start {
        subcommand: String,
        config: AnalysisConfig,
        reference_time: String,
    },
    /// Progress through pipeline stages.
    Progress(Progress),
    /// A parse/analysis warning.
    Warning {
        kind: String,
        source: String,
        context: Option<String>,
        message: String,
    },
    /// Final line: summary of outputs.
    Done {
        exit_code: i32,
        output_dir: Option<String>,
        outputs: OutputPaths,
        warning_count: usize,
        list_sizes: ListSizes,
    },
    /// Used by `validate` subcommand.
    ValidateDone {
        exit_code: i32,
        war_files: usize,
        warning_count: usize,
    },
    /// Used by `schema` subcommand.
    Schema {
        war_required: Vec<String>,
        war_optional: Vec<String>,
        activity_required: Vec<String>,
        activity_optional: Vec<String>,
        filename_convention: String,
    },
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "stage", rename_all = "snake_case")]
pub enum Progress {
    ExpandWars {
        detail: String,
    },
    ParseWar {
        current: usize,
        total: usize,
        file: String,
    },
    ParseActivity {
        file: String,
    },
    Analyze,
    Write {
        format: String,
        path: String,
    },
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct OutputPaths {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub xlsx: Option<String>,
    pub csv: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub markdown: Option<String>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct ListSizes {
    pub auto_kick: usize,
    pub repeat_offenders: usize,
    pub any_bad_war: usize,
    pub low_activity: usize,
    pub combined_kick: usize,
}

/// Emit a single NDJSON event to stdout.
pub fn emit(event: &Event) {
    let line = serde_json::to_string(event).expect("event serialization is infallible");
    println!("{line}");
}
