//! `list_history` Tauri command — walks `<reports_root>/*/run.json` and returns summaries.

use serde::Serialize;
use std::path::PathBuf;
use tauri::AppHandle;

#[derive(Debug, Serialize)]
pub struct RunSummary {
    pub run_id: String,
    pub reference_time: String,
    pub output_dir: String,
    pub warning_count: usize,
    pub exit_code: i32,
    pub list_sizes: serde_json::Value,
    pub config: serde_json::Value,
    pub input_war_files: Vec<String>,
    pub input_activity_file: String,
}

#[tauri::command]
pub fn list_history(
    _app: AppHandle,
    reports_root: PathBuf,
) -> Result<Vec<RunSummary>, String> {
    let mut summaries = Vec::new();

    let entries = std::fs::read_dir(&reports_root)
        .map_err(|e| format!("cannot read reports dir: {e}"))?;

    for entry in entries.flatten() {
        let run_json = entry.path().join("run.json");
        if !run_json.exists() {
            continue;
        }
        let text = match std::fs::read_to_string(&run_json) {
            Ok(t) => t,
            Err(_) => continue,
        };
        let v: serde_json::Value = match serde_json::from_str(&text) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let dir_name = entry.file_name().to_string_lossy().to_string();
        summaries.push(RunSummary {
            run_id: dir_name,
            reference_time: v["reference_time"]
                .as_str()
                .unwrap_or("")
                .to_string(),
            output_dir: v["output_dir"].as_str().unwrap_or("").to_string(),
            warning_count: v["warning_count"].as_u64().unwrap_or(0) as usize,
            exit_code: v["exit_code"].as_i64().unwrap_or(0) as i32,
            list_sizes: v["list_sizes"].clone(),
            config: v["config"].clone(),
            input_war_files: v["input_war_files"]
                .as_array()
                .unwrap_or(&vec![])
                .iter()
                .filter_map(|x| x.as_str().map(|s| s.to_string()))
                .collect(),
            input_activity_file: v["input_activity_file"]
                .as_str()
                .unwrap_or("")
                .to_string(),
        });
    }

    // Sort newest first by reference_time string (ISO-8601 lexicographic order works).
    summaries.sort_by(|a, b| b.reference_time.cmp(&a.reference_time));

    Ok(summaries)
}
