//! `validate` subcommand: parses inputs only and reports warnings, no outputs.

use std::path::PathBuf;

use clap::Args;

use twr_core::{parse_activity_csv, parse_war_csv, WarningCollector};

use crate::glob::expand_war_paths;

#[derive(Debug, Args)]
pub struct ValidateArgs {
    /// War CSV directories, glob patterns, or individual files. Repeatable.
    #[arg(long, required = true, num_args = 1..)]
    pub wars: Vec<String>,

    /// Activity CSV path.
    #[arg(long, required = true)]
    pub activity: PathBuf,

    /// Treat warnings as errors (exit code 3).
    #[arg(long)]
    pub fail_on_warnings: bool,
}

pub fn run(args: ValidateArgs) -> anyhow::Result<i32> {
    let war_paths = expand_war_paths(&args.wars)?;
    if war_paths.is_empty() {
        anyhow::bail!("no war CSV files matched the supplied --wars argument(s)");
    }
    let warnings = WarningCollector::new();
    for p in &war_paths {
        match parse_war_csv(p, &warnings) {
            Ok(w) => {
                tracing::info!(
                    war = %w.display_name,
                    participants = w.participants.len(),
                    file = %p.display(),
                    "war parsed",
                );
            }
            Err(e) => return Err(anyhow::anyhow!(e)),
        }
    }
    let _activity = parse_activity_csv(&args.activity, &warnings)?;
    let warns = warnings.snapshot();
    if !warns.is_empty() {
        eprintln!("validation produced {} warning(s):", warns.len());
        for w in &warns {
            eprintln!(
                "  [{kind}] {source}{ctx}: {detail}",
                kind = w.kind,
                source = w.source,
                ctx = w
                    .row_or_member
                    .as_ref()
                    .map(|c| format!(" ({})", c))
                    .unwrap_or_default(),
                detail = w.detail,
            );
        }
        if args.fail_on_warnings {
            return Ok(3);
        }
    } else {
        println!("validation OK — {} war file(s), 0 warnings", war_paths.len());
    }
    Ok(0)
}
