//! Resolves the path to the bundled `torn-war-report` sidecar.
//!
//! In release builds Tauri bundles the binary as an `externalBin` sidecar.
//! In dev builds we fall back to the workspace `target/debug` output.

use std::path::PathBuf;
use tauri::{AppHandle, Manager};

pub fn sidecar_path(app: &AppHandle) -> PathBuf {
    // Tauri 2 resolves sidecar resources via `path().resolve`.
    // The binary name must match the key in tauri.conf.json `bundle.externalBin`.
    if let Ok(p) = app.path().resolve(
        "binaries/torn-war-report",
        tauri::path::BaseDirectory::Resource,
    ) {
        if p.exists() {
            return p;
        }
    }

    // Dev fallback: find the workspace root relative to the manifest dir and
    // look for the debug binary.
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_dir
        .parent() // crates/
        .and_then(|p| p.parent()) // workspace root
        .unwrap_or(manifest_dir);

    if cfg!(windows) {
        workspace_root.join("target/debug/torn-war-report.exe")
    } else {
        workspace_root.join("target/debug/torn-war-report")
    }
}
