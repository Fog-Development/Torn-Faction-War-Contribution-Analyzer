//! `get_schema` Tauri command — invokes the CLI schema subcommand and returns structured data.

use serde::Serialize;
use tauri::AppHandle;
use tokio::process::Command;

use crate::cli::sidecar_path;

#[derive(Debug, Serialize)]
pub struct SchemaInfo {
    pub war_required: Vec<String>,
    pub war_optional: Vec<String>,
    pub activity_required: Vec<String>,
    pub activity_optional: Vec<String>,
    pub filename_convention: String,
}

#[tauri::command]
pub async fn get_schema(app: AppHandle) -> Result<SchemaInfo, String> {
    let binary = sidecar_path(&app);
    let output = Command::new(&binary)
        .args(["--emit=json", "schema"])
        .output()
        .await
        .map_err(|e| format!("failed to run schema: {e}"))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let event: serde_json::Value =
        serde_json::from_str(stdout.trim()).map_err(|e| format!("invalid schema JSON: {e}"))?;

    Ok(SchemaInfo {
        war_required: json_str_array(&event["war_required"]),
        war_optional: json_str_array(&event["war_optional"]),
        activity_required: json_str_array(&event["activity_required"]),
        activity_optional: json_str_array(&event["activity_optional"]),
        filename_convention: event["filename_convention"]
            .as_str()
            .unwrap_or("")
            .to_string(),
    })
}

fn json_str_array(v: &serde_json::Value) -> Vec<String> {
    v.as_array()
        .unwrap_or(&vec![])
        .iter()
        .filter_map(|x| x.as_str().map(|s| s.to_string()))
        .collect()
}
