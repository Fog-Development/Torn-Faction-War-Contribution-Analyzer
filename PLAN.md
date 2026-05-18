# Torn Faction War Contribution Analyzer — Project Plan

A CLI tool and reusable library (Rust) that ingests an arbitrary number of Torn ranked war CSV exports plus a member activity CSV, and produces analytical reports identifying members with poor war contributions and/or low overall activity.

This document is the complete specification. It is intended to be handed to Claude Code as the starting point for implementation.

---

## 1. Goals and non-goals

### Goals

- Accept N ranked-war CSV files (where N ≥ 1) and one member activity CSV.
- Derive each war's UTC start datetime from the war CSV filename.
- For each member, decide whether they were in the faction at the time of each war, based on the `Days` column in member activity.
- Classify each member's participation in each war as one of: `Excluded` (not in faction yet), `Zero` (present but 0 points), `Low` (bottom 20% of non-zero participants), or `Ok`.
- Generate reports listing auto-kick candidates, repeat offenders, members with any single bad war, low-activity members (avgE30 below threshold), and a combined kick list.
- Output formats: XLSX, CSV (one file per list), and Markdown summary — selectable independently or in combination.
- Be configurable via a default TOML config bundled with the binary, optionally overridden by a user-supplied config file, optionally overridden by CLI flags.
- Be usable as a Rust library so a future PHP/web integration can call the analysis engine directly with already-structured data (skipping CSV parsing).

### Non-goals

- No Torn API integration (the user already has tooling for that).
- No GUI.
- No persistent state between runs (each invocation is self-contained).
- No web server. The future PHP integration will call the library, not consume an HTTP API.
- No automatic emailing, posting to forums, or other distribution of reports.

---

## 2. Inputs

### 2.1 Ranked war CSVs

One file per war. Filename **must** follow this convention:

```
<freeform_prefix>_<ISO8601_UTC_datetime>.csv
```

Where the datetime portion uses `-` instead of `:` (since `:` is not allowed in Windows filenames), and ends in `Z`. Examples:

```
TM_vs_Baby_Champers_2026-04-23T19-00-00Z.csv
TM_vs_REK_2_2026-05-15T14-00-00Z.csv
War_2026-03-06T00-00-00Z.csv
```

Parser rule: the war datetime is the substring matching the regex `\d{4}-\d{2}-\d{2}T\d{2}-\d{2}-\d{2}Z` anywhere in the filename stem. The portion before the date is treated as the war's display name (underscores converted to spaces, trailing connector words like `vs` preserved). If no match is found → strict error.

#### Expected columns

Required (case-sensitive):
- `attacker_name` — string
- `attacker_id` — integer (may be blank in trailing summary rows; those rows are skipped)
- `Hits` — integer
- `Points` — integer
- `WarHits` — integer (parsed but not used in current logic; preserved for forward compat)

Optional / ignored: `NotWarHits`, `Assist`, `Retals`, `Overseas`, `BonusHits`, `AvgFF`, `Extra`, `Value`, plus any extra columns past column 13 that contain summary/total data in the right-hand cells of the spreadsheet export.

Parsing rules:
- Rows where `attacker_name` is empty/whitespace are skipped silently (these are export padding).
- Rows where `attacker_name` is present but `attacker_id` is empty → warn, skip row.
- Non-numeric values in numeric columns → warn, skip row, continue.
- Duplicate `attacker_id` within one file → warn, keep first occurrence, skip subsequent.

### 2.2 Member activity CSV

A single file. Filename is arbitrary (default expected: `Member_Activity.csv`, but the path is supplied via CLI or config).

#### Expected columns

Required:
- `Name` — string
- `Days` — integer (days in faction)
- `avgE30` — float (30-day average energy)

Optional but preserved if present (used in output detail columns where available):
- `avgE7`, `attacks7`, `attacks30`, `act30`, `act7`, `Donator?`, `Property`, `gym30`, `gym7`, `Xanax Daily`, `Revs Last 30`, `Contract Last 30`, `Revive Skill`

Parsing rules:
- First row may be a blank "separator" row (the user's data has this). Skip rows where `Name` is empty.
- Members with `Days` missing → warn, exclude from analysis entirely (their tenure can't be evaluated).
- Members with `Days < min_days` (default 7) → flag as "insufficient data" and exclude from avgE30 analysis, but still include in war analysis if they appear in war CSVs.

---

## 3. Analysis algorithm

### 3.1 Reference time

The CLI accepts an optional `--reference-time <ISO8601>` flag. If omitted, use the current UTC time. This timestamp is the "now" against which `Days` is compared.

### 3.2 War presence determination

For each member with a known `Days` value, for each war:

```
days_ago = (reference_time - war_start_utc).days   # integer days, truncated
present  = days > days_ago                          # strict greater-than
```

Rationale: Torn roster locks at least a couple of days before war start. If `Days == days_ago`, the member is treated as having joined too late.

If `Days` is missing but the member appears in the war CSV: treat as present (CSV presence is evidence enough).

### 3.3 Per-war classification

For each (member, war) pair where the member is present:

```
points = csv_points if in_csv else 0
hits   = csv_hits   if in_csv else 0

if points == 0:
    category = Zero
elif points <= war.low_threshold:
    category = Low
else:
    category = Ok
```

If the member is not present: category = `Excluded`.

### 3.4 Low threshold computation

For each war independently:

1. Collect points from all members classified as present with `points > 0`.
2. Compute the configured percentile (default 20th) of that set using linear interpolation.
3. That value is `war.low_threshold`.

Note: 0-point members are deliberately excluded from the percentile calculation so the threshold reflects actual contribution levels, not the floor.

### 3.5 Per-member rollup

For each member, compute:
- `present_count` — wars they were present for
- `zero_count` — wars classified `Zero`
- `low_count` — wars classified `Low`
- `poor_count` — `zero_count + low_count`
- `ok_count` — wars classified `Ok`
- `avg_points` — mean of points across present wars (including zeros)

### 3.6 List generation

All thresholds below are config-overridable. Defaults shown.

1. **Auto-Kick List**: `zero_count >= 2` (default: 2). Triggers regardless of activity.
2. **Repeat Offenders**: `poor_count >= 2` (default: 2). Includes auto-kick members.
3. **Any Single Bad War**: members with at least one `Zero` or `Low` war. Useful for spotting one-off concerns.
4. **Low Activity (avgE30)**: `Days >= min_days` AND `avgE30 < activity_threshold` (default: 7 and 750).
5. **Combined Kick List**: on list 4 AND on list 2. The strongest evidence case.

### 3.7 Edge cases

- Single war provided: all logic still works. Auto-kick/repeat-offender thresholds may need adjustment via config (running with only 1 war and a 2+ threshold yields empty lists). Document this in `--help`.
- Two members with identical names: identify by `attacker_id` where available. The activity CSV has no ID, so it's matched purely by name. Document this as a known limitation; warn if a war-CSV `attacker_id` maps to multiple distinct names across files (would indicate a rename).
- Member appears in war CSV but not in activity CSV: include them in war analysis with `days = None`. They appear in war lists but not in avgE30 lists. Note in output that activity data is missing.
- Reference time before any war: warn loudly; results will be nonsensical.

---

## 4. Configuration

### 4.1 Default config (TOML, bundled with binary)

```toml
[analysis]
# Reference time for "now". If unset, use current UTC.
# reference_time = "2026-05-17T00:00:00Z"

# Bottom-N percentile cutoff for "Low" classification (within non-zero points).
low_percentile = 0.20

# Minimum tenure (days) required to evaluate avgE30 reliably.
min_days_for_activity = 7

# avgE30 threshold below which a member is flagged as low activity.
activity_threshold = 750.0

# Number of zero-point wars that triggers auto-kick.
zero_war_kick_threshold = 2

# Number of poor (zero + low) wars that flags a repeat offender.
poor_war_threshold = 2

[input]
# Glob patterns or directory paths handled at the CLI level.
# Member activity file column name overrides (only set if your export differs).
# activity_name_column = "Name"
# activity_days_column = "Days"
# activity_avge30_column = "avgE30"

[output]
# Which reports to produce.
formats = ["xlsx", "csv", "markdown"]
# Output directory. CLI flag overrides.
# directory = "./reports"
```

Resolution order (lowest to highest precedence):

1. Bundled defaults (compiled into the binary via `include_str!`).
2. User config file: `--config <path>` if provided; otherwise auto-discover in this order:
   - `./torn-war-report.toml`
   - `$XDG_CONFIG_HOME/torn-war-report/config.toml` (or `~/.config/torn-war-report/config.toml`)
3. Environment variables (prefix `TWR_`, e.g. `TWR_LOW_PERCENTILE=0.15`).
4. CLI flags.

Implementation note: use `figment` or `config` crate for layered config, or hand-roll with `serde` + manual overrides. Hand-rolled is fine and reduces deps.

### 4.2 CLI surface

```
torn-war-report [OPTIONS] <COMMAND>

Commands:
  analyze   Run a full analysis and emit reports
  validate  Parse inputs and report what would be analyzed, without writing outputs
  schema    Print the expected CSV column layouts and exit

Global options:
  --config <PATH>          Path to a TOML config file
  --verbose / -v           Increase logging (repeatable: -v, -vv)
  --quiet / -q             Suppress non-error output

`analyze` options:
  --wars <DIR_OR_GLOB>     Directory or glob matching war CSVs. Required.
                           Accepts repeated flags or a comma-separated list.
  --activity <PATH>        Path to the Member Activity CSV. Required.
  --output <DIR>           Output directory. Default: ./reports/<timestamp>/
  --formats <LIST>         Comma list of: xlsx, csv, markdown. Overrides config.
  --reference-time <ISO8601>   "Now" timestamp for tenure calc. Default: real now.
  --low-percentile <0..1>
  --activity-threshold <FLOAT>
  --min-days <INT>
  --zero-war-kick-threshold <INT>
  --poor-war-threshold <INT>
  --fail-on-warnings       Exit non-zero if any warning is emitted.
                           Useful when invoked from automation.
```

`validate` is the dry-run mode: parses inputs, prints what would be analyzed (which wars detected, member counts, any warnings), exits 0 on success. Used by the future PHP integration to sanity-check inputs before a real run.

### 4.3 Exit codes

- `0` — success, no errors, no warnings (or warnings present but `--fail-on-warnings` not set).
- `1` — runtime error (I/O, parse failure, invalid config).
- `2` — usage error (bad CLI args).
- `3` — analysis completed but `--fail-on-warnings` triggered.

---

## 5. Architecture

### 5.1 Crate layout

```
torn-war-report/                  # workspace root
├── Cargo.toml                    # workspace
├── crates/
│   ├── twr-core/                 # pure analysis library
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── model.rs          # data types
│   │       ├── parse.rs          # CSV + filename parsing
│   │       ├── analysis.rs       # threshold calc, classification
│   │       ├── config.rs         # Config struct + defaults
│   │       └── warnings.rs       # WarningCollector type
│   ├── twr-report/               # output rendering (xlsx/csv/md)
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── xlsx.rs
│   │       ├── csv.rs
│   │       └── markdown.rs
│   └── twr-cli/                  # CLI binary
│       ├── Cargo.toml
│       └── src/
│           ├── main.rs
│           └── commands/
│               ├── analyze.rs
│               ├── validate.rs
│               └── schema.rs
├── default-config.toml           # bundled defaults (include_str!)
├── fixtures/                     # test data (see §7)
└── README.md
```

The `twr-core` crate has zero I/O dependencies beyond `csv` and `serde` (and `chrono`/`time`). The future PHP integration links against `twr-core` (via FFI, command-line piping, or a thin wrapper binary) — splitting it out now keeps that option open.

### 5.2 Core types (sketch)

```rust
// twr-core/src/model.rs

pub type MemberId = u64;
pub type MemberName = String;

#[derive(Debug, Clone)]
pub struct War {
    pub display_name: String,
    pub start_utc: chrono::DateTime<chrono::Utc>,
    pub source_filename: String,
    pub participants: Vec<WarParticipant>,
}

#[derive(Debug, Clone)]
pub struct WarParticipant {
    pub name: MemberName,
    pub id: Option<MemberId>,   // optional because activity CSV has no ID
    pub hits: u32,
    pub war_hits: u32,
    pub points: u32,
}

#[derive(Debug, Clone)]
pub struct MemberActivity {
    pub name: MemberName,
    pub days: Option<u32>,
    pub avg_e30: Option<f64>,
    // ... other optional fields preserved for output
    pub extras: std::collections::BTreeMap<String, String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WarCategory {
    Excluded,   // not in faction yet
    Zero,       // present, 0 points
    Low,        // present, 0 < points <= threshold
    Ok,         // present, points > threshold
}

#[derive(Debug, Clone)]
pub struct MemberWarResult {
    pub war_index: usize,
    pub category: WarCategory,
    pub points: u32,
    pub hits: u32,
    pub listed_in_csv: bool, // false = absent-with-tenure (treated as 0)
}

#[derive(Debug, Clone)]
pub struct MemberSummary {
    pub name: MemberName,
    pub days: Option<u32>,
    pub avg_e30: Option<f64>,
    pub wars: Vec<MemberWarResult>,
    pub present_count: u32,
    pub zero_count: u32,
    pub low_count: u32,
    pub poor_count: u32,
    pub avg_points: f64,
}

#[derive(Debug, Clone)]
pub struct AnalysisReport {
    pub reference_time: chrono::DateTime<chrono::Utc>,
    pub config: Config,
    pub wars: Vec<War>,
    pub war_thresholds: Vec<f64>,
    pub members: Vec<MemberSummary>,
    pub auto_kick: Vec<MemberName>,
    pub repeat_offenders: Vec<MemberName>,
    pub any_bad_war: Vec<MemberName>,
    pub low_activity: Vec<MemberName>,
    pub combined_kick: Vec<MemberName>,
    pub warnings: Vec<Warning>,
}
```

### 5.3 Public library API

```rust
// twr-core/src/lib.rs

pub fn analyze(
    wars: Vec<War>,
    activity: Vec<MemberActivity>,
    config: &Config,
) -> Result<AnalysisReport, AnalysisError>;

// Convenience for the CLI path:
pub fn analyze_from_files(
    war_paths: &[&Path],
    activity_path: &Path,
    config: &Config,
) -> Result<AnalysisReport, AnalysisError>;
```

The PHP integration path will call `analyze` directly with already-parsed data.

### 5.4 Recommended crate dependencies

- `clap` (4.x, derive feature) — CLI
- `serde` + `serde_derive` — config + data
- `toml` — config files
- `csv` — CSV parsing
- `chrono` — datetime (preferred for ISO8601 + UTC ergonomics)
- `regex` — filename datetime extraction
- `rust_xlsxwriter` — XLSX output (pure Rust, no Excel COM dependency)
- `tracing` + `tracing-subscriber` — structured logging
- `thiserror` — error types in library
- `anyhow` — error reporting in CLI (only)
- `globset` — `--wars` glob expansion

Keep `twr-core` free of `clap`, `anyhow`, `tracing-subscriber` — those belong to the CLI crate.

---

## 6. Output formats

All output files go under `--output <DIR>` (default `./reports/<UTC_timestamp>/`).

### 6.1 XLSX

Single workbook `analysis.xlsx`. Sheets:

1. **Overview** — methodology, war list with dates, threshold values, list of warnings if any.
2. **Auto-Kick** — name, days, zero count, low count, present, avg pts, avgE30, act30, per-war summary string.
3. **Repeat Offenders** — name, days, poor/zero/low counts, present, avg pts, then one column per war showing `<pts>p / <hits>h` with conditional fill (red for zero, yellow for low).
4. **Any Single Bad War** — name, days, # flagged wars, zero wars, avg pts, detail string.
5. **Low avgE30** — name, days, avgE30, avgE7, attacks30, act30, act7, war-list flag.
6. **Combined Kick List** — name, days, avgE30, act30, poor/zero/low counts, avg pts, auto-kick yes/no.
7. **War Matrix** — every tracked member, one row, with per-war points/hits cells and totals.
8. **Warnings** — if any were emitted during analysis. Omitted if clean.

Formatting requirements:
- Arial 10pt body, 11pt bold white-on-blue headers.
- Frozen header rows.
- Conditional fills: red (#F4CCCC) for severe, orange (#FCE5CD) for moderate, yellow (#FFF2CC) for mild.
- Zero-point war cells: red fill. Low-point war cells: yellow fill.
- Auto-sized column widths (approximate; computed in code, no need for actual auto-fit).

### 6.2 CSV

One file per list:
- `auto_kick.csv`
- `repeat_offenders.csv`
- `any_bad_war.csv`
- `low_activity.csv`
- `combined_kick.csv`
- `war_matrix.csv` (full per-war breakdown)
- `warnings.csv` (only if warnings present)

Columns mirror the XLSX sheets. CSVs use UTF-8, RFC 4180 quoting, `\n` line endings.

### 6.3 Markdown

Single file `summary.md`. Structure:

```markdown
# Faction War Contribution Report

Generated: <UTC timestamp>
Reference time: <UTC timestamp>

## Wars analyzed

| War | Date (UTC) | Days ago | Participants | Low threshold |
|...|...|...|...|...|

## Methodology

[Brief description, mostly static]

## ⚠ Warnings (N)

[Bullet list, only if any]

## 🚨 Auto-Kick List (N members)

[Table]

## Repeat Offenders (N members)

[Table]

## Low Activity — avgE30 < <threshold> (N members)

[Table]

## 🚨 Combined Kick List (N members)

[Table]
```

The markdown summary is the primary human-readable output; the XLSX is for cross-referencing.

---

## 7. Testing strategy

### 7.1 Fixtures

`fixtures/` directory contains hand-crafted CSVs:

- `fixtures/wars/TM_vs_Alpha_2026-01-01T00-00-00Z.csv` — 20 participants, clean data.
- `fixtures/wars/TM_vs_Beta_2026-02-01T00-00-00Z.csv` — 20 participants, includes one with 0 points.
- `fixtures/wars/TM_vs_Gamma_2026-03-01T00-00-00Z.csv` — 20 participants, includes trailing blank rows like the real export.
- `fixtures/wars/TM_vs_Delta_2026-04-01T00-00-00Z.csv` — 20 participants, includes malformed row (non-numeric Points).
- `fixtures/wars/malformed/missing_column.csv` — for negative tests.
- `fixtures/wars/malformed/bad_filename.csv` — no date in filename.
- `fixtures/activity/Member_Activity.csv` — 30 members with varied Days values designed to hit every code path (some <7 days, some on the war presence threshold, some missing from war CSVs entirely, some present in war CSVs but missing from activity).

Build the fixtures with deterministic data so test assertions can hardcode expected outputs.

### 7.2 Unit tests (twr-core)

- `parse::extract_war_datetime` — table-driven: valid filenames, invalid filenames, edge cases (date at start vs middle vs end of stem).
- `parse::parse_war_csv` — clean file, file with blank trailing rows, file with malformed row (verify warning emitted, row skipped), file with duplicate `attacker_id` (verify warning, first kept), file with missing required column (verify hard error).
- `parse::parse_activity_csv` — same battery.
- `analysis::compute_threshold` — verify 20th percentile of `[10,20,30,40,50,60,70,80,90,100]` is 28 (linear interpolation). Verify excluding zeros works.
- `analysis::classify_member` — table-driven covering all four `WarCategory` outcomes plus the `Days == days_ago` edge case (must be `Excluded`).
- `analysis::analyze` — integration-level: feed the fixture data, assert the exact composition of each output list.

### 7.3 Integration tests (twr-cli)

Use `assert_cmd` and `predicates` crates.

- `analyze` happy path: run against fixtures, verify all expected output files exist, verify content of one row in the markdown matches the expected member.
- `analyze --formats xlsx` (single format): verify only the xlsx is produced.
- `analyze --formats xlsx,csv`: verify both.
- `validate` happy path: zero output files written, exit 0, stdout contains expected war count.
- `validate` with bad input: exit 1, stderr contains parse error.
- `analyze --fail-on-warnings` with fixture containing malformed row: exit 3.
- `analyze` with overridden `--low-percentile 0.10`: verify thresholds in output reflect the new value.
- CLI flag overrides config file value (precedence test).

### 7.4 Golden / snapshot tests

Use `insta` for snapshot testing of the markdown output. The fixtures are deterministic, so a stable snapshot is achievable. When intentionally changing output format, re-bless snapshots with `cargo insta review`.

### 7.5 Property-based tests (optional)

Use `proptest` for `compute_threshold`: generate random non-empty `Vec<u32>`, verify the result is between min and max, and that excluding zeros produces a threshold ≥ excluding nothing.

---

## 8. Warnings system

A `WarningCollector` is threaded through parsing and analysis. Each warning has:

- `kind`: enum (`MalformedRow`, `DuplicateId`, `MissingActivityRecord`, `InsufficientTenure`, `AmbiguousMemberName`, `FilenameDateMissing`, etc.)
- `source`: file path or "analysis"
- `detail`: human-readable string
- `row_or_member`: optional reference for context

Warnings appear:
1. In `tracing` logs (level: WARN).
2. In an "X warnings" line on stderr at end of run.
3. In the "Warnings" sheet (XLSX) / `warnings.csv` / "⚠ Warnings" section of `summary.md`, only if any present.
4. With `--fail-on-warnings`, cause exit code 3.

The library API exposes `AnalysisReport.warnings` so PHP-side callers can present them in their own UI.

---

## 9. Implementation order (suggested for Claude Code)

1. **Scaffolding**: workspace, three crates, basic `Cargo.toml` files, `default-config.toml`, README skeleton, CI placeholder.
2. **Core types** (`twr-core/model.rs`): `War`, `WarParticipant`, `MemberActivity`, `Config`, etc.
3. **Filename parsing** (`twr-core/parse.rs`): regex + tests. This is small and testable in isolation.
4. **CSV parsing** (war first, then activity): with `WarningCollector` integration and full tests.
5. **Analysis engine** (`twr-core/analysis.rs`): threshold, classification, member summary, list generation. Heavy unit testing here.
6. **Config layering** (`twr-core/config.rs`): defaults → user file → env → CLI flag overrides.
7. **CSV output** (`twr-report/csv.rs`): simplest format, get it working end-to-end first.
8. **Markdown output** (`twr-report/markdown.rs`): second simplest.
9. **XLSX output** (`twr-report/xlsx.rs`): most complex; do last so the data shapes are settled.
10. **CLI** (`twr-cli`): wire `analyze`, `validate`, `schema` commands.
11. **Integration tests**: full pipeline against fixtures.
12. **Snapshot tests**: lock in the markdown output.
13. **Polish**: `--help` text, README, example invocations, error message quality.

Each step should leave the workspace in a buildable, test-passing state.

---

## 10. Documentation deliverables

- `README.md` — quick start, common invocations, output examples.
- `docs/config.md` — every config knob, default, and override path.
- `docs/csv_formats.md` — exact expected columns for war and activity CSVs, examples.
- `docs/library_usage.md` — for the future PHP integration: how to construct `War` and `MemberActivity` from your own data and call `analyze()`.
- Inline rustdoc on every public item in `twr-core`.

---

## 11. Open questions for the implementer

These are intentionally left for Claude Code to flag if they come up during implementation, rather than pre-deciding:

1. Should `--wars` accept individual file paths in addition to directories/globs? (Recommendation: yes, treat any path that's a file as a single-file input.)
2. Should the XLSX sheet ordering be configurable, or is the fixed order acceptable? (Recommendation: fixed.)
3. For the future PHP integration: prefer FFI (`extern "C"` interface) or a JSON-over-stdin/stdout binary protocol? (Recommendation: defer; expose the Rust API cleanly now, and add a JSON CLI mode later when the integration is actually built.)
4. Time crate choice: `chrono` vs `time`. `chrono` is more familiar; `time` is more modern. (Recommendation: `chrono` unless there's a specific reason to prefer `time`.)

---

## 12. Acceptance criteria

The project is considered complete when:

- [ ] `cargo build --release` produces a single binary `torn-war-report`.
- [ ] `cargo test` passes with all unit, integration, and snapshot tests green.
- [ ] Running `torn-war-report analyze --wars ./fixtures/wars --activity ./fixtures/activity/Member_Activity.csv` produces the three output formats with deterministic, snapshot-matched content.
- [ ] All warnings from the fixture data are correctly emitted and visible in each output format.
- [ ] `validate` correctly distinguishes good inputs (exit 0) from bad inputs (exit 1).
- [ ] CLI flags successfully override config file values, which successfully override bundled defaults.
- [ ] The `twr-core` crate has zero CLI-specific dependencies and can be added as a library dependency by a hypothetical downstream consumer.
- [ ] README documents the filename convention, the column expectations, and at least three example invocations.
