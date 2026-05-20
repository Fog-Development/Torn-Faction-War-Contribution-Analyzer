//! Deserialization structs mirroring the CLI `--emit=json` NDJSON event schema.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CliEvent {
    Start {
        subcommand: String,
        config: serde_json::Value,
        reference_time: String,
    },
    Progress(CliProgress),
    Warning {
        kind: String,
        source: String,
        context: Option<String>,
        message: String,
    },
    Done {
        exit_code: i32,
        output_dir: Option<String>,
        outputs: serde_json::Value,
        warning_count: usize,
        list_sizes: serde_json::Value,
    },
    ValidateDone {
        exit_code: i32,
        war_files: usize,
        warning_count: usize,
    },
    Schema {
        war_required: Vec<String>,
        war_optional: Vec<String>,
        activity_required: Vec<String>,
        activity_optional: Vec<String>,
        filename_convention: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "stage", rename_all = "snake_case")]
pub enum CliProgress {
    ExpandWars { detail: String },
    ParseWar { current: usize, total: usize, file: String },
    ParseActivity { file: String },
    Analyze,
    Write { format: String, path: String },
}

/// Try to parse one line of NDJSON into a CliEvent. Returns None on empty lines.
pub fn parse_line(line: &str) -> Option<Result<CliEvent, serde_json::Error>> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(serde_json::from_str(trimmed))
}
