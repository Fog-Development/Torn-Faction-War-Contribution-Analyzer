//! `spawn_analyze` Tauri command — runs the CLI sidecar and streams NDJSON events to the frontend.

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State, Window};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

use crate::cli::sidecar_path;
use crate::events::parse_line;
use crate::RunRegistry;

#[derive(Debug, Deserialize)]
pub struct AnalyzeArgs {
    pub wars: Vec<String>,
    pub activity: String,
    pub output: Option<String>,
    pub formats: Option<Vec<String>>,
    pub reference_time: Option<String>,
    pub low_percentile: Option<f64>,
    pub activity_threshold: Option<f64>,
    pub min_days: Option<u32>,
    pub zero_war_kick_threshold: Option<u32>,
    pub poor_war_threshold: Option<u32>,
    pub fail_on_warnings: bool,
    pub config_path: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct RunHandle {
    pub run_id: String,
}

/// Build the argv list for `torn-war-report analyze` from structured args.
/// Pure function — unit-testable without Tauri.
pub fn build_argv(args: &AnalyzeArgs) -> Vec<String> {
    let mut v: Vec<String> = vec!["--emit=json".into(), "analyze".into()];

    for w in &args.wars {
        v.push("--wars".into());
        v.push(w.clone());
    }

    v.push("--activity".into());
    v.push(args.activity.clone());

    if let Some(o) = &args.output {
        v.push("--output".into());
        v.push(o.clone());
    }
    if let Some(fmts) = &args.formats {
        v.push("--formats".into());
        v.push(fmts.join(","));
    }
    if let Some(rt) = &args.reference_time {
        v.push("--reference-time".into());
        v.push(rt.clone());
    }
    if let Some(lp) = args.low_percentile {
        v.push("--low-percentile".into());
        v.push(lp.to_string());
    }
    if let Some(at) = args.activity_threshold {
        v.push("--activity-threshold".into());
        v.push(at.to_string());
    }
    if let Some(md) = args.min_days {
        v.push("--min-days".into());
        v.push(md.to_string());
    }
    if let Some(z) = args.zero_war_kick_threshold {
        v.push("--zero-war-kick-threshold".into());
        v.push(z.to_string());
    }
    if let Some(p) = args.poor_war_threshold {
        v.push("--poor-war-threshold".into());
        v.push(p.to_string());
    }
    if args.fail_on_warnings {
        v.push("--fail-on-warnings".into());
    }
    if let Some(cfg) = &args.config_path {
        v.push("--config".into());
        v.push(cfg.clone());
    }
    v
}

#[tauri::command]
pub async fn spawn_analyze(
    app: AppHandle,
    window: Window,
    state: State<'_, RunRegistry>,
    args: AnalyzeArgs,
) -> Result<RunHandle, String> {
    let run_id = format!("analyze-{}", chrono::Utc::now().timestamp_millis());
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

#[cfg(test)]
mod tests {
    use super::*;

    fn base_args() -> AnalyzeArgs {
        AnalyzeArgs {
            wars: vec!["./fixtures/wars".into()],
            activity: "./fixtures/activity/Member_Activity.csv".into(),
            output: None,
            formats: None,
            reference_time: None,
            low_percentile: None,
            activity_threshold: None,
            min_days: None,
            zero_war_kick_threshold: None,
            poor_war_threshold: None,
            fail_on_warnings: false,
            config_path: None,
        }
    }

    #[test]
    fn build_argv_minimal() {
        let args = base_args();
        let argv = build_argv(&args);
        assert_eq!(argv[0], "--emit=json");
        assert_eq!(argv[1], "analyze");
        assert!(argv.contains(&"--wars".to_string()));
        assert!(argv.contains(&"--activity".to_string()));
        assert!(!argv.contains(&"--output".to_string()));
        assert!(!argv.contains(&"--fail-on-warnings".to_string()));
    }

    #[test]
    fn build_argv_all_flags() {
        let mut args = base_args();
        args.output = Some("/tmp/out".into());
        args.formats = Some(vec!["xlsx".into(), "csv".into()]);
        args.reference_time = Some("2026-05-01T00:00:00Z".into());
        args.low_percentile = Some(0.1);
        args.activity_threshold = Some(500.0);
        args.min_days = Some(14);
        args.zero_war_kick_threshold = Some(3);
        args.poor_war_threshold = Some(3);
        args.fail_on_warnings = true;
        args.config_path = Some("./config.toml".into());

        let argv = build_argv(&args);
        assert!(argv.contains(&"--output".to_string()));
        assert!(argv.contains(&"/tmp/out".to_string()));
        assert!(argv.contains(&"--formats".to_string()));
        assert!(argv.contains(&"xlsx,csv".to_string()));
        assert!(argv.contains(&"--reference-time".to_string()));
        assert!(argv.contains(&"--low-percentile".to_string()));
        assert!(argv.contains(&"--activity-threshold".to_string()));
        assert!(argv.contains(&"--min-days".to_string()));
        assert!(argv.contains(&"--zero-war-kick-threshold".to_string()));
        assert!(argv.contains(&"--poor-war-threshold".to_string()));
        assert!(argv.contains(&"--fail-on-warnings".to_string()));
        assert!(argv.contains(&"--config".to_string()));
    }

    #[test]
    fn build_argv_multiple_wars() {
        let mut args = base_args();
        args.wars = vec!["war1.csv".into(), "war2.csv".into()];
        let argv = build_argv(&args);
        // Each war path should be preceded by --wars
        let wars_indices: Vec<usize> = argv
            .iter()
            .enumerate()
            .filter(|(_, s)| s.as_str() == "--wars")
            .map(|(i, _)| i)
            .collect();
        assert_eq!(wars_indices.len(), 2);
        assert_eq!(argv[wars_indices[0] + 1], "war1.csv");
        assert_eq!(argv[wars_indices[1] + 1], "war2.csv");
    }
}
