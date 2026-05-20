fn main() {
    // Create a placeholder sidecar binary for the host triple if it doesn't exist,
    // so tauri-build passes on clean checkouts and CI runners without a real binary.
    let triple = std::env::var("TARGET").unwrap_or_default();
    let manifest = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let bin_dir = manifest.join("binaries");
    std::fs::create_dir_all(&bin_dir).ok();
    let name = if triple.contains("windows") {
        format!("torn-war-report-{}.exe", triple)
    } else {
        format!("torn-war-report-{}", triple)
    };
    let sidecar = bin_dir.join(&name);
    if !sidecar.exists() {
        std::fs::write(&sidecar, b"placeholder").ok();
    }

    tauri_build::build()
}
