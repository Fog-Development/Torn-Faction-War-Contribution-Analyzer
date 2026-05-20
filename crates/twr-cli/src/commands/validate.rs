//! `validate` subcommand: parses inputs only and reports warnings, no outputs.

use std::path::PathBuf;

use clap::Args;

use twr_core::{parse_activity_csv, parse_war_csv, WarningCollector};

use crate::events::{self, Event, Progress};
use crate::glob::expand_war_paths;
use crate::EmitMode;

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

pub fn run(args: ValidateArgs, emit: EmitMode) -> anyhow::Result<i32> {
    let war_paths = expand_war_paths(&args.wars)?;
    if war_paths.is_empty() {
        anyhow::bail!("no war CSV files matched the supplied --wars argument(s)");
    }

    if emit == EmitMode::Json {
        events::emit(&Event::Progress(Progress::ExpandWars {
            detail: format!("matched {} file(s)", war_paths.len()),
        }));
    }

    let warnings = WarningCollector::new();
    let total = war_paths.len();
    for (i, p) in war_paths.iter().enumerate() {
        if emit == EmitMode::Json {
            events::emit(&Event::Progress(Progress::ParseWar {
                current: i + 1,
                total,
                file: p.display().to_string(),
            }));
        }
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

    if emit == EmitMode::Json {
        events::emit(&Event::Progress(Progress::ParseActivity {
            file: args.activity.display().to_string(),
        }));
    }
    let _activity = parse_activity_csv(&args.activity, &warnings)?;

    let warns = warnings.snapshot();
    let warning_count = warns.len();
    let exit_code = if args.fail_on_warnings && !warns.is_empty() {
        3
    } else {
        0
    };

    if emit == EmitMode::Json {
        for w in &warns {
            events::emit(&Event::Warning {
                kind: w.kind.to_string(),
                source: w.source.clone(),
                context: w.row_or_member.clone(),
                message: w.detail.clone(),
            });
        }
        events::emit(&Event::ValidateDone {
            exit_code,
            war_files: total,
            warning_count,
        });
    } else {
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
        } else {
            println!("validation OK — {} war file(s), 0 warnings", war_paths.len());
        }
    }

    Ok(exit_code)
}
