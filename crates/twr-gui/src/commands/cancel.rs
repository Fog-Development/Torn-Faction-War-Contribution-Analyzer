//! `cancel_run` Tauri command — kills a live child process by run_id.

use tauri::State;

use crate::RunRegistry;

#[tauri::command]
pub async fn cancel_run(state: State<'_, RunRegistry>, run_id: String) -> Result<(), String> {
    // Extract the child while holding the lock, then drop the lock before awaiting.
    let child = state.0.lock().unwrap().remove(&run_id);
    if let Some(mut child) = child {
        child.kill().await.map_err(|e| format!("kill failed: {e}"))?;
    }
    Ok(())
}
