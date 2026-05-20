//! `spawn_validate` Tauri command.

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State, Window};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

use crate::cli::sidecar_path;
use crate::events::parse_line;
use crate::RunRegistry;

#[derive(Debug, Deserialize)]
pub struct ValidateArgs {
    pub wars: Vec<String>,
    pub activity: String,
    pub fail_on_warnings: bool,
}

#[derive(Debug, Serialize)]
pub struct RunHandle {
    pub run_id: String,
}

pub fn build_argv(args: &ValidateArgs) -> Vec<String> {
    let mut v: Vec<String> = vec!["--emit=json".into(), "validate".into()];
    for w in &args.wars {
        v.push("--wars".into());
        v.push(w.clone());
    }
    v.push("--activity".into());
    v.push(args.activity.clone());
    if args.fail_on_warnings {
        v.push("--fail-on-warnings".into());
    }
    v
}

#[tauri::command]
pub async fn spawn_validate(
    app: AppHandle,
    window: Window,
    state: State<'_, RunRegistry>,
    args: ValidateArgs,
) -> Result<RunHandle, String> {
    let run_id = format!("validate-{}", chrono::Utc::now().timestamp_millis());
    let argv = build_argv(&args);
    let binary = sidecar_path(&app);

    let mut child = Command::new(&binary)
        .args(&argv)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("failed to spawn sidecar: {e}"))?;

    let stdout = child.stdout.take().expect("stdout was piped");
    let stderr = child.stderr.take().expect("stderr was piped");

    {
        let mut map = state.0.lock().unwrap();
        map.insert(run_id.clone(), child);
    }

    let win_clone = window.clone();
    let rid = run_id.clone();
    tokio::spawn(async move {
        let mut lines = BufReader::new(stdout).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            if let Some(Ok(event)) = parse_line(&line) {
                let _ = win_clone.emit(&format!("cli://event/{rid}"), &event);
            }
        }
    });

    let win_stderr = window.clone();
    let rid2 = run_id.clone();
    tokio::spawn(async move {
        let mut lines = BufReader::new(stderr).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            let _ = win_stderr.emit(&format!("cli://stderr/{rid2}"), &line);
        }
    });

    Ok(RunHandle { run_id })
}
