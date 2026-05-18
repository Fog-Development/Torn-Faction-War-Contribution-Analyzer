# Claude Code — Project Guide

## What this project is

A Rust CLI tool + reusable library (`twr-core`) that ingests Torn ranked war CSV exports and a member activity CSV, then produces reports (XLSX, CSV, Markdown) identifying members with poor war contributions and/or low overall activity.

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
      main.rs
      glob.rs         --wars glob/dir/file expansion
      commands/
        analyze.rs
        validate.rs
        schema.rs
    tests/
      cli.rs                  assert_cmd integration tests
      markdown_snapshot.rs    insta snapshot test
      common.rs               shared helpers (wars_dir(), activity_csv(), etc.)
      snapshots/              insta snapshot files — commit these
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
cargo build --release          # produces target/release/torn-war-report
cargo test                     # run all 34 tests
```

All 34 tests must pass before committing:
- 25 unit tests in `twr-core` (parse + analysis)
- 7 CLI integration tests in `twr-cli/tests/cli.rs`
- 1 pipeline integration test in `twr-core/tests/fixtures_pipeline.rs`
- 1 insta markdown snapshot test in `twr-cli/tests/markdown_snapshot.rs`

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
- Member matching between war CSVs and activity CSV is **case-sensitive by name**
- War presence rule: `days > days_ago` (strict greater-than — `==` is NOT present)
- Low threshold percentile excludes zero-point members from the calculation
- `avg_points` in `MemberSummary` is the mean across **present** wars only (zeros included, Excluded not)
- Fixture reference time is always `2026-05-01T00:00:00Z` — don't use real `Utc::now()` in fixture tests
