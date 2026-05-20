//! File/directory picker commands and app settings persistence.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tauri::{AppHandle, Manager};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_shell::ShellExt;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    pub reports_root: Option<String>,
}

fn settings_path(app: &AppHandle) -> PathBuf {
    app.path()
        .app_data_dir()
        .expect("app data dir must exist")
        .join("settings.json")
}

#[tauri::command]
pub fn get_settings(app: AppHandle) -> AppSettings {
    let path = settings_path(&app);
    if let Ok(text) = std::fs::read_to_string(&path) {
        serde_json::from_str(&text).unwrap_or(AppSettings { reports_root: None })
    } else {
        AppSettings { reports_root: None }
    }
}

#[tauri::command]
pub fn set_settings(app: AppHandle, settings: AppSettings) -> Result<(), String> {
    let path = settings_path(&app);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_string_pretty(&settings).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn pick_directory(app: AppHandle) -> Result<Option<String>, String> {
    let path = app.dialog().file().blocking_pick_folder();
    Ok(path.map(|p| p.to_string()))
}

#[tauri::command]
pub async fn pick_files(app: AppHandle, multi: bool) -> Result<Vec<String>, String> {
    if multi {
        let paths = app
            .dialog()
            .file()
            .add_filter("CSV files", &["csv"])
            .blocking_pick_files();
        Ok(paths
            .unwrap_or_default()
            .into_iter()
            .map(|p| p.to_string())
            .collect())
    } else {
        let path = app
            .dialog()
            .file()
            .add_filter("CSV files", &["csv"])
            .blocking_pick_file();
        Ok(path.into_iter().map(|p| p.to_string()).collect())
    }
}

#[tauri::command]
pub async fn open_path(app: AppHandle, path: String) -> Result<(), String> {
    app.shell()
        .open(&path, None)
        .map_err(|e| format!("open failed: {e}"))
}
