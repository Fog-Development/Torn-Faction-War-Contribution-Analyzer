# Claude Code — Project Guide

## What this project is

A Rust CLI tool + reusable library (`twr-core`) that ingests Torn ranked war CSV exports and a member activity CSV, then produces reports (XLSX, CSV, Markdown) identifying members with poor war contributions and/or low overall activity.

There is also a **Tauri 2 desktop GUI** (`twr-gui`) that wraps the CLI as a subprocess. The GUI is the primary user-facing product; the CLI is the analysis engine and automation interface.

The future PHP web integration will call `twr-core::analyze()` directly — keep that crate free of CLI dependencies.

---

## Workspace layout

```
Cargo.toml                          workspace root
default-config.toml                 bundled defaults (compiled into binary via include_str!)
crates/
  twr-core/     pure analysis library — NO clap/anyhow/tracing-subscriber deps
    src/
      model.rs        core data types (War, MemberSummary, AnalysisReport, etc.)
      parse.rs        CSV + filename parsing, WarningCollector integration
      analysis.rs     threshold calc, classification, list generation
      config.rs       layered config (defaults → TOML → env → CLI flags)
      warnings.rs     WarningCollector, Warning, WarningKind
    tests/
      fixtures_pipeline.rs    end-to-end test against fixtures/
  twr-report/   output renderers
    src/
      xlsx.rs         rust_xlsxwriter — conditional cell fills, frozen headers
      csv.rs          one file per output list
      markdown.rs     summary.md; render_for_snapshot() used by snapshot test
  twr-cli/      binary: analyze / validate / schema subcommands
    src/
      main.rs         global --emit <text|json> flag + subcommand dispatch
      events.rs       NDJSON event enum (Event, Progress, etc.) — the GUI contract
      glob.rs         --wars glob/dir/file expansion
      commands/
        analyze.rs    pipeline orchestration; writes run.json manifest always
        validate.rs
        schema.rs
    tests/
      cli.rs                  assert_cmd integration tests (includes --emit=json tests)
      markdown_snapshot.rs    insta snapshot test
      common.rs               shared helpers (wars_dir(), activity_csv(), etc.)
      snapshots/              insta snapshot files — commit these
  twr-gui/      Tauri 2 desktop GUI — wraps the CLI as a sidecar subprocess
    src/
      lib.rs          Tauri app entry, plugin/command registration, RunRegistry state
      main.rs         binary entry (windows_subsystem = "windows" in release)
      cli.rs          resolves sidecar path (bundle resource → debug fallback)
      events.rs       deserialises CLI NDJSON events (mirrors twr-cli/src/events.rs)
      commands/
        analyze.rs    spawn_analyze + build_argv (unit-tested)
        validate.rs   spawn_validate
        cancel.rs     cancel_run
        schema.rs     get_schema
        presets.rs    preset save/load/delete ($APPDATA/presets.json)
        history.rs    list_history (reads run.json manifests)
        paths.rs      pick_directory, pick_files, open_path, settings
    ui/               vanilla HTML/JS frontend (no Node toolchain required)
      index.html
      main.js         tab switching, boot
      store.js        central reactive state (warPaths, activityPath, config, …)
      tabs/
        inputs.js     drag-and-drop + file picker, per-file date validation
        config.js     threshold form, preset management, output dir picker
        run.js        Run/Validate buttons, progress bar, warning panel, output paths
        history.js    past-run table, re-run from run.json
    tauri.conf.json   productName, sidecar bundle, window config
    build.rs          auto-creates sidecar placeholder so cargo check passes without binary
    binaries/         sidecar lives here at build time (gitignored except placeholder)
    icons/            icon.ico + icon.png
fixtures/
  wars/
    TM_vs_Alpha_2026-01-01T00-00-00Z.csv   20 participants, clean
    TM_vs_Beta_2026-02-01T00-00-00Z.csv    20 participants, Carol = 0 pts
    TM_vs_Gamma_2026-03-01T00-00-00Z.csv   20 participants, trailing blank rows
    TM_vs_Delta_2026-04-01T00-00-00Z.csv   20 participants, BadRow has "notanumber" in Points
    malformed/
      missing_column.csv    missing the Points column (used in negative tests)
      bad_filename.csv      no ISO-8601 date in filename (used in negative tests)
  activity/
    Member_Activity.csv     30 members, covering all edge-case code paths
sample-real-rw-and-member-data/   real Torn exports — use for format reference only
```

---

## Building and testing

```
cargo build --release          # produces target/release/torn-war-report (CLI)
cargo fmt --all                # ALWAYS run before committing — CI enforces rustfmt on all platforms
cargo clippy --workspace --all-targets -- -D warnings   # ALWAYS run before committing — mirrors CI exactly
cargo test                     # run all 37 tests (excludes twr-gui, which needs Tauri tooling)
cargo test -p twr-core -p twr-report -p twr-cli   # explicit scope — always use this
cargo check -p twr-gui         # type-check the GUI crate (build.rs creates a sidecar placeholder)
```

**Formatting rule**: run `cargo fmt --all` after every code change. The CI `cargo fmt --all -- --check` step will fail the build on all three platforms (Linux, Windows, macOS) if any file is not formatted.

**Clippy rule**: run `cargo clippy --workspace --all-targets -- -D warnings` after every code change. This is the exact command CI runs — it covers all crates including `twr-gui`. Do not use a narrower scope or clippy failures in `twr-gui` will only appear in CI.

All 37 tests must pass before committing:
- 25 unit tests in `twr-core` (parse + analysis)
- 10 CLI integration tests in `twr-cli/tests/cli.rs` (includes 3 `--emit=json` tests)
- 1 pipeline integration test in `twr-core/tests/fixtures_pipeline.rs`
- 1 insta markdown snapshot test in `twr-cli/tests/markdown_snapshot.rs`

**Git commit rule**: never include a `Co-Authored-By:` trailer in commit messages.

**GUI dev loop — use the helper script (Windows):**
```powershell
.\gui.ps1            # build CLI (debug) + launch cargo tauri dev
.\gui.ps1 release    # build CLI (release) + cargo tauri build → installer in target/release/bundle/
```
The script installs `tauri-cli` automatically if it isn't present, detects the host triple, copies the sidecar, then runs Tauri. Run it from anywhere inside the repo — it locates the workspace root itself.

---

## GUI architecture — key facts

- **Integration**: the GUI spawns `torn-war-report --emit=json <subcommand>` as a child process (Tauri sidecar). It does **not** link to `twr-core` or `twr-report` at all.
- **NDJSON contract**: `twr-cli/src/events.rs` is the emit side; `twr-gui/src/events.rs` is the parse side. Both must stay in sync. The event `type` field values are the stable contract — rename carefully.
- **`run.json` manifest**: written by `analyze.rs` into every output directory regardless of `--emit` mode. This is what the History tab reads. Its schema mirrors the `done` event payload plus `input_war_files`, `input_activity_file`, `reference_time`, and resolved `config`.
- **`RunRegistry` state**: a `Mutex<HashMap<String, Child>>` held in Tauri state. Always extract the child and drop the lock **before** any `.await` — holding a `MutexGuard` across an await makes the future `!Send` and fails to compile.
- **Frontend state**: `ui/store.js` is the single source of truth. Tab modules subscribe to fields via `subscribe(field, fn)` and call `set(field, value)` to update. Never read DOM state as truth — always read from the store.
- **Sidecar placeholder**: `build.rs` writes a dummy `torn-war-report-x86_64-pc-windows-msvc.exe` to `binaries/` if it doesn't exist, so `cargo check -p twr-gui` passes on a clean checkout. The real binary must be copied there before `cargo tauri dev` or `cargo tauri build`.
- **No Node toolchain**: the frontend is vanilla HTML/JS. `@tauri-apps/api` is available at runtime via Tauri's global injection — import from `@tauri-apps/api/core` etc. in ES module style.

### What to update when changing the GUI

**Adding a new Tauri command:**
1. Write the handler in `crates/twr-gui/src/commands/<module>.rs`
2. Register it in `tauri::generate_handler![...]` in `src/lib.rs`
3. Call it from JS via `invoke('command_name', { argName: value })`

**Changing the NDJSON event schema:**
1. Update the `Event` / `Progress` enum in `twr-cli/src/events.rs`
2. Mirror the change in `twr-gui/src/events.rs` (`CliEvent` / `CliProgress`)
3. Update any JS consumers in `ui/tabs/run.js` that pattern-match on `event.type` or `event.stage`
4. This is a **breaking change** for any external consumers — do it deliberately

**Adding a new analysis config field:**
- Follow the existing config knob checklist (below), then also add it to:
  - `PresetConfig` struct in `twr-gui/src/commands/presets.rs`
  - `get_default_config()` return value in the same file
  - `FIELDS` array in `ui/tabs/config.js`
  - `spawn_analyze` args mapping in `ui/tabs/run.js`

---

## Testing rules — what to update when

### Adding or changing a field on a core type (`model.rs`)
- Update any `MemberSummary`, `AnalysisReport`, `War`, etc. construction in tests
- If the field appears in any output format, update the relevant renderer in `twr-report/`
- If it appears in the markdown output, re-bless the snapshot: `cargo insta review`

### Changing analysis logic (`analysis.rs`)
- Unit tests are in the `#[cfg(test)]` block at the bottom of `analysis.rs`
- The pipeline integration test (`fixtures_pipeline.rs`) asserts specific members on specific lists — update its `assert!` calls if the expected outputs change
- The snapshot test will likely need re-blessing: `cargo insta review`

### Changing CSV parsing (`parse.rs`)
- Unit tests are in the `#[cfg(test)]` block at the bottom of `parse.rs`
- If you change what columns are required or how warnings are emitted, update the corresponding `parse::tests::*` unit tests

### Changing any output format (`twr-report/`)
- XLSX (`xlsx.rs`): no snapshot; covered by the CLI integration test checking file existence
- CSV (`csv.rs`): covered by CLI integration test checking file existence
- Markdown (`markdown.rs`): **snapshot tested** — any formatting change requires re-blessing: `cargo insta review`

### Adding a new config knob
- Add field to `AnalysisConfig` / `AnalysisOverlay` in `config.rs`
- Add the corresponding CLI flag in `twr-cli/src/commands/analyze.rs` (set it in `overlay()`)
- Add env var handling in `config.rs` (`from_env()`)
- Add a CLI override integration test in `cli.rs`
- Update `default-config.toml` and the README config section

### Changing fixture data
- The pipeline test (`fixtures_pipeline.rs`) hardcodes specific member names and list memberships (Carol on auto-kick, Heidi on repeat offenders, etc.) — update those assertions if fixture members change
- The snapshot test output will change — re-bless: `cargo insta review`
- Reference time in all fixture tests is pinned to `2026-05-01T00:00:00Z`

### Re-blessing insta snapshots
```
cargo test                          # see which snapshots fail
cargo insta review                  # interactively accept/reject each change
# or: INSTA_UPDATE=always cargo test   # auto-accept all
```
Snapshot files live in `crates/twr-cli/tests/snapshots/` — always commit them.

---

## Real Torn CSV formats

The `sample-real-rw-and-member-data/` folder has real exports. Use these as ground truth if Torn ever changes its export format.

**War CSV — actual column order:**
```
attacker_name, attacker_id, Hits, WarHits, NotWarHits, Assist, Retals,
Overseas, BonusHits, AvgFF, Extra, Points, Value, [summary cols...]
```
- `Points` is at index 11 — the parser uses header names, not positions, so order doesn't break anything
- `Value` is a quoted dollar string: `"$2,500,000"` (commas inside quotes, handled by `csv` crate with `flexible(true)`)
- `Extra` column is blank (empty string, not 0)
- Trailing blank rows look like: `,,0,0,0,0,0,0,,0.0,,0,$0,,,,,`
- Right-hand summary cells in cols 14+: `,,Total Pool,"$50,000,000",,` — parsed as extra fields and ignored

**Activity CSV — actual column order:**
```
Name, Days, Donator?, Property, avgE7, avgE30, gym30, gym7, attacks7,
attacks30, Xanax Daily, act30, act7, Revs Last 30, Contract Last 30, Revive Skill
```
- Second row is always a blank separator — skipped automatically (empty `Name`)
- `act30`/`act7` are percentage strings: `"24%"` — stored verbatim in `extras`, not parsed as numbers
- `Donator?` is `1` or `0`
- `Days` can be `0`

---

## Configuration layering

Resolution order (lowest → highest precedence):
1. Bundled defaults in `default-config.toml` (compiled in via `include_str!`)
2. User TOML: `--config <path>` or auto-discovered `./torn-war-report.toml`
3. Env vars: `TWR_LOW_PERCENTILE`, `TWR_ACTIVITY_THRESHOLD`, `TWR_MIN_DAYS`, `TWR_ZERO_WAR_KICK_THRESHOLD`, `TWR_POOR_WAR_THRESHOLD`, `TWR_FORMATS`
4. CLI flags

---

## Exit codes

| Code | Meaning |
|---|---|
| 0 | Success |
| 1 | Runtime error (I/O, parse, bad config) |
| 2 | Usage error (bad CLI args) |
| 3 | Success but `--fail-on-warnings` triggered |

---

## Key invariants to preserve

- `twr-core` must have zero dependency on `clap`, `anyhow`, or `tracing-subscriber`
- `twr-gui` must have zero dependency on `twr-core` or `twr-report` — all analysis goes through the CLI subprocess
- Member matching between war CSVs and activity CSV is **case-sensitive by name**
- War presence rule: `days > days_ago` (strict greater-than — `==` is NOT present)
- Low threshold percentile excludes zero-point members from the calculation
- `avg_points` in `MemberSummary` is the mean across **present** wars only (zeros included, Excluded not)
- Fixture reference time is always `2026-05-01T00:00:00Z` — don't use real `Utc::now()` in fixture tests
- The `--emit=json` flag default is `text` — never change the default, existing CLI consumers depend on text mode
- NDJSON events go to **stdout**; human-readable log lines go to **stderr** — never mix them
