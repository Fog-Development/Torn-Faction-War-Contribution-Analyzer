# Torn Faction War Contribution Analyzer

A CLI tool that ingests Torn ranked war CSV exports and a member activity CSV, then produces reports identifying members with poor war contributions and/or low overall activity. Outputs XLSX, CSV, and Markdown — all from one command.

---

## Quick start

```
# Build the release binary
cargo build --release

# Run a full analysis
./target/release/torn-war-report analyze \
  --wars ./path/to/war/csvs \
  --activity ./Member_Activity.csv

# Dry-run: validate inputs without writing any output
./target/release/torn-war-report validate \
  --wars ./path/to/war/csvs \
  --activity ./Member_Activity.csv
```

Reports are written to `./reports/<UTC_timestamp>/` by default.

---

## Building

Requires Rust 1.75+ (stable). No system dependencies beyond the Rust toolchain.

```
cargo build             # debug build
cargo build --release   # optimized release build
cargo test              # run all tests
```

The release binary is at `target/release/torn-war-report`.

---

## Input files

### War CSVs

One file per ranked war, exported directly from Torn. The **filename must embed the war's UTC start time** in this format:

```
<anything>_<YYYY-MM-DDTHH-MM-SSZ>.csv
```

Colons are replaced with dashes in the time portion because Windows filenames don't allow colons. Examples:

```
TM_vs_REK_2_2026-05-15T14-00-00Z.csv
TM_vs_Baby_Champers_2026-04-23T19-00-00Z.csv
War_2026-03-06T00-00-00Z.csv
```

Required columns (case-sensitive): `attacker_name`, `attacker_id`, `Hits`, `WarHits`, `Points`

The actual Torn export column order is:
```
attacker_name, attacker_id, Hits, WarHits, NotWarHits, Assist, Retals,
Overseas, BonusHits, AvgFF, Extra, Points, Value, ...
```

Trailing blank rows and right-hand summary cells (Total Pool, Expenses, etc.) that Torn appends to the export are handled automatically.

### Member Activity CSV

A single file exported from your faction activity tracker. Default expected name is `Member_Activity.csv` but any path works via `--activity`.

Required columns: `Name`, `Days`, `avgE30`

The actual column order from the Torn faction activity export:
```
Name, Days, Donator?, Property, avgE7, avgE30, gym30, gym7, attacks7,
attacks30, Xanax Daily, act30, act7, Revs Last 30, Contract Last 30, Revive Skill
```

The second row is a blank separator row — this is expected and skipped automatically.

Run `torn-war-report schema` to print the full expected column layout for both file types.

---

## How the analysis works

1. **War presence** — For each member, their `Days` value is compared against how long ago the war was fought. A member is considered present only if `Days > days_ago` (strict). Members who joined too recently are marked `Excluded` for that war.

2. **Low threshold** — For each war independently, the 20th percentile of points scored by present, non-zero participants is computed (linear interpolation). This is the `Low` cutoff.

3. **Per-war classification** — Each (member, war) pair is classified as:
   - `Excluded` — not in faction yet
   - `Zero` — present but 0 points
   - `Low` — present, scored ≤ the low threshold
   - `Ok` — present, scored above the threshold

4. **Lists generated:**
   - **Auto-Kick** — 2+ zero-point wars
   - **Repeat Offenders** — 2+ poor wars (zero + low combined)
   - **Any Single Bad War** — at least one Zero or Low war
   - **Low Activity** — `Days ≥ 7` and `avgE30 < 750`
   - **Combined Kick List** — on both Repeat Offenders and Low Activity

All thresholds are configurable (see Configuration below).

---

## Commands

### `analyze` — run a full analysis and write reports

```
torn-war-report analyze [OPTIONS]

Required:
  --wars <PATH>          Directory, glob pattern, or individual file. Repeatable.
  --activity <PATH>      Member Activity CSV path.

Optional:
  --output <DIR>         Output directory. Default: ./reports/<timestamp>/
  --formats <LIST>       Comma-separated: xlsx,csv,markdown. Default: all three.
  --reference-time <ISO8601>  Override "now" for tenure calculation.
  --low-percentile <0..1>
  --activity-threshold <FLOAT>
  --min-days <INT>
  --zero-war-kick-threshold <INT>
  --poor-war-threshold <INT>
  --fail-on-warnings     Exit code 3 if any warnings were emitted.
```

Examples:

```
# Analyze wars in a directory, all three output formats
torn-war-report analyze \
  --wars ./wars \
  --activity ./Member_Activity.csv

# Only produce the Excel workbook
torn-war-report analyze \
  --wars ./wars \
  --activity ./Member_Activity.csv \
  --formats xlsx

# Use a stricter low-percentile cutoff and lower activity threshold
torn-war-report analyze \
  --wars ./wars \
  --activity ./Member_Activity.csv \
  --low-percentile 0.30 \
  --activity-threshold 600

# Analyze multiple individual war files
torn-war-report analyze \
  --wars ./TM_vs_REK_2_2026-05-15T14-00-00Z.csv \
  --wars ./TM_vs_Baby_Champers_2026-04-23T19-00-00Z.csv \
  --activity ./Member_Activity.csv
```

### `validate` — parse inputs without writing any output

Useful for checking that your CSVs are well-formed and filenames are correct before committing to a full run.

```
torn-war-report validate --wars ./wars --activity ./Member_Activity.csv
```

Exits 0 on success. Exits 1 on parse errors. Prints any warnings to stderr.

### `schema` — print expected CSV column layouts

```
torn-war-report schema
```

---

## Output files

All files are written to the output directory (`./reports/<timestamp>/` by default).

| File | Format | Description |
|---|---|---|
| `analysis.xlsx` | XLSX | Multi-sheet workbook with all lists, war matrix, and warnings |
| `auto_kick.csv` | CSV | Members with 2+ zero-point wars |
| `repeat_offenders.csv` | CSV | Members with 2+ poor wars |
| `any_bad_war.csv` | CSV | Members with at least one Zero or Low war |
| `low_activity.csv` | CSV | Members below the avgE30 threshold |
| `combined_kick.csv` | CSV | Intersection of Repeat Offenders and Low Activity |
| `war_matrix.csv` | CSV | Every member × every war, with points and hits |
| `warnings.csv` | CSV | Parse/analysis warnings, if any |
| `summary.md` | Markdown | Human-readable summary of all lists and warnings |

The XLSX workbook has one sheet per list plus an Overview sheet and a War Matrix sheet. Zero-point war cells are highlighted red; low-point war cells are highlighted yellow.

---

## Configuration

### Config file

Create a `torn-war-report.toml` in the directory you run the tool from (or pass `--config <path>`):

```toml
[analysis]
low_percentile = 0.20          # bottom-N% cutoff for "Low" classification
min_days_for_activity = 7      # minimum tenure to evaluate avgE30
activity_threshold = 750.0     # avgE30 below this = low activity
zero_war_kick_threshold = 2    # zero-point wars before auto-kick
poor_war_threshold = 2         # total poor wars before repeat-offender flag

[output]
formats = ["xlsx", "csv", "markdown"]
# directory = "./reports"      # uncomment to fix the output directory
```

### Config resolution order (lowest → highest priority)

1. Bundled defaults (compiled into the binary)
2. Config file: `--config <path>`, or auto-discovered at `./torn-war-report.toml`
3. Environment variables with `TWR_` prefix (e.g. `TWR_LOW_PERCENTILE=0.15`)
4. CLI flags

### Environment variables

| Variable | Config key |
|---|---|
| `TWR_LOW_PERCENTILE` | `analysis.low_percentile` |
| `TWR_ACTIVITY_THRESHOLD` | `analysis.activity_threshold` |
| `TWR_MIN_DAYS` | `analysis.min_days_for_activity` |
| `TWR_ZERO_WAR_KICK_THRESHOLD` | `analysis.zero_war_kick_threshold` |
| `TWR_POOR_WAR_THRESHOLD` | `analysis.poor_war_threshold` |
| `TWR_FORMATS` | `output.formats` |

---

## Exit codes

| Code | Meaning |
|---|---|
| 0 | Success |
| 1 | Runtime error (I/O failure, parse error, bad config) |
| 2 | Usage error (bad CLI arguments) |
| 3 | Analysis completed but `--fail-on-warnings` was set and warnings were emitted |

---

## Tips and edge cases

**Single war:** The tool works with just one war file, but the default auto-kick threshold (2 zero-point wars) will always produce an empty list. Lower it with `--zero-war-kick-threshold 1`.

**Members missing from the activity CSV:** If someone appears in a war CSV but not in the activity file, they still appear in war-based lists. A `MissingActivityRecord` warning is emitted for each. This is normal for members who have since left the faction.

**Members who changed their Torn username:** The tool matches by name. If the same `attacker_id` maps to two different names across war files, an `InconsistentIdName` warning is emitted.

**The `--wars` flag** accepts directories (all `*.csv` files in the directory), glob patterns (e.g. `./wars/TM_vs_*.csv`), or individual file paths. It can be repeated.

**Excluding the activity file from the wars directory:** If your war CSVs and activity CSV are in the same folder, pass the war files explicitly or use a glob that doesn't match `Member_Activity.csv`:
```
torn-war-report analyze \
  --wars "./my-data/TM_vs_*.csv" \
  --activity ./my-data/Member_Activity.csv
```

---

## Project structure

```
torn-war-report/
├── Cargo.toml                  workspace manifest
├── default-config.toml         bundled default config (compiled into binary)
├── crates/
│   ├── twr-core/               pure analysis library (no CLI deps)
│   │   └── src/
│   │       ├── model.rs        data types
│   │       ├── parse.rs        CSV + filename parsing
│   │       ├── analysis.rs     threshold calculation, classification, list generation
│   │       ├── config.rs       layered config
│   │       └── warnings.rs     WarningCollector
│   ├── twr-report/             output renderers
│   │   └── src/
│   │       ├── xlsx.rs
│   │       ├── csv.rs
│   │       └── markdown.rs
│   └── twr-cli/                CLI binary
│       └── src/
│           ├── main.rs
│           └── commands/
│               ├── analyze.rs
│               ├── validate.rs
│               └── schema.rs
├── fixtures/                   deterministic test data
└── sample-real-rw-and-member-data/   example real exports
```

`twr-core` has no dependency on `clap`, `anyhow`, or `tracing-subscriber` and can be used as a standalone library by other tools.
