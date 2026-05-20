pub mod cli;
pub mod commands;
pub mod events;

use std::collections::HashMap;
use std::sync::Mutex;
use tokio::process::Child;

/// Tracks live child processes keyed by run_id so they can be cancelled.
pub struct RunRegistry(pub Mutex<HashMap<String, Child>>);

impl RunRegistry {
    pub fn new() -> Self {
        RunRegistry(Mutex::new(HashMap::new()))
    }
}

impl Default for RunRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_opener::init())
        .manage(RunRegistry::new())
        .invoke_handler(tauri::generate_handler![
            commands::analyze::spawn_analyze,
            commands::validate::spawn_validate,
            commands::cancel::cancel_run,
            commands::schema::get_schema,
            commands::presets::get_default_config,
            commands::presets::list_presets,
            commands::presets::save_preset,
            commands::presets::delete_preset,
            commands::history::list_history,
            commands::paths::get_settings,
            commands::paths::set_settings,
            commands::paths::pick_directory,
            commands::paths::pick_files,
            commands::paths::open_path,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
