# twr-gui

Tauri 2 desktop GUI for the Torn faction war contribution analyzer.

## Dev loop

```powershell
# 1. Build the CLI sidecar first (needed at runtime)
cargo build -p twr-cli

# 2. Copy it into binaries/ with the required Tauri target-triple suffix
$triple = rustc -vV | Select-String "host:" | ForEach-Object { $_ -replace "host: ", "" }
Copy-Item "..\..\target\debug\torn-war-report.exe" "binaries\torn-war-report-$triple.exe"

# 3. Launch Tauri dev server
cargo tauri dev
```

## Release build

```powershell
cargo build -p twr-cli --release
$triple = (rustc -vV | Select-String "host:").ToString().Trim().Replace("host: ", "")
Copy-Item "..\..\target\release\torn-war-report.exe" "binaries\torn-war-report-$triple.exe"
cargo tauri build
# Installer output: ../../target/release/bundle/
```

## Architecture

- **CLI contract**: the GUI spawns `torn-war-report --emit=json <subcommand>` as a sidecar subprocess. Stdout is NDJSON events; stderr is human-readable logs. See `src/events.rs` for the deserialization types and `crates/twr-cli/src/events.rs` for the emit side.
- **No `twr-core` dependency**: the GUI is decoupled from the analysis library. All analysis logic stays in the CLI.
- **State**: `ui/store.js` is the single source of truth for the frontend. Tab modules subscribe to fields and re-render on change.
