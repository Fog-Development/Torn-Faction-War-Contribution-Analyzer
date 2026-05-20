//! Preset management: save/load/delete named configs in `$APPDATA/.../presets.json`.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tauri::{AppHandle, Manager};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PresetConfig {
    pub low_percentile: f64,
    pub activity_threshold: f64,
    pub min_days: u32,
    pub zero_war_kick_threshold: u32,
    pub poor_war_threshold: u32,
    pub formats: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Preset {
    pub name: String,
    pub config: PresetConfig,
}

fn presets_path(app: &AppHandle) -> std::path::PathBuf {
    app.path()
        .app_data_dir()
        .expect("app data dir must exist")
        .join("presets.json")
}

fn load_map(app: &AppHandle) -> HashMap<String, PresetConfig> {
    let path = presets_path(app);
    if let Ok(text) = std::fs::read_to_string(&path) {
        serde_json::from_str(&text).unwrap_or_default()
    } else {
        HashMap::new()
    }
}

fn save_map(app: &AppHandle, map: &HashMap<String, PresetConfig>) -> Result<(), String> {
    let path = presets_path(app);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_string_pretty(map).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_default_config() -> PresetConfig {
    PresetConfig {
        low_percentile: 0.20,
        activity_threshold: 750.0,
        min_days: 7,
        zero_war_kick_threshold: 2,
        poor_war_threshold: 2,
        formats: vec!["xlsx".into(), "csv".into(), "markdown".into()],
    }
}

#[tauri::command]
pub fn list_presets(app: AppHandle) -> Vec<Preset> {
    load_map(&app)
        .into_iter()
        .map(|(name, config)| Preset { name, config })
        .collect()
}

#[tauri::command]
pub fn save_preset(app: AppHandle, preset: Preset) -> Result<(), String> {
    let mut map = load_map(&app);
    map.insert(preset.name, preset.config);
    save_map(&app, &map)
}

#[tauri::command]
pub fn delete_preset(app: AppHandle, name: String) -> Result<(), String> {
    let mut map = load_map(&app);
    map.remove(&name);
    save_map(&app, &map)
}
