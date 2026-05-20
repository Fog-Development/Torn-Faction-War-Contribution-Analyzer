fn main() {
    // Create a placeholder sidecar binary if it doesn't exist, so tauri-build passes.
    // In real dev/CI, copy the actual torn-war-report binary here before building.
    let manifest = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let bin_dir = manifest.join("binaries");
    std::fs::create_dir_all(&bin_dir).ok();
    let sidecar = bin_dir.join("torn-war-report-x86_64-pc-windows-msvc.exe");
    if !sidecar.exists() {
        std::fs::write(&sidecar, b"placeholder").ok();
    }

    tauri_build::build()
}
