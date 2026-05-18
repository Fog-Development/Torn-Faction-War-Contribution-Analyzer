//! `torn-war-report` — CLI entrypoint.

mod commands;
mod glob;

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use tracing_subscriber::EnvFilter;

#[derive(Debug, Parser)]
#[command(
    name = "torn-war-report",
    version,
    about = "Torn faction war contribution analyzer"
)]
struct Cli {
    /// Path to a TOML config file (layered on top of the bundled defaults).
    #[arg(long, global = true)]
    config: Option<PathBuf>,

    /// Increase logging verbosity. Repeat for more detail.
    #[arg(short, long, global = true, action = clap::ArgAction::Count)]
    verbose: u8,

    /// Suppress non-error output.
    #[arg(short, long, global = true)]
    quiet: bool,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Run analysis and emit reports.
    Analyze(commands::analyze::AnalyzeArgs),
    /// Parse inputs only, no outputs.
    Validate(commands::validate::ValidateArgs),
    /// Print expected CSV column layouts.
    Schema,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    init_logging(cli.verbose, cli.quiet);

    let result = dispatch(cli);
    match result {
        Ok(code) => {
            if code == 0 {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(code as u8)
            }
        }
        Err(e) => {
            eprintln!("error: {e:#}");
            ExitCode::from(1)
        }
    }
}

fn dispatch(cli: Cli) -> anyhow::Result<i32> {
    match cli.command {
        Command::Analyze(args) => commands::analyze::run(args, cli.config.as_deref()),
        Command::Validate(args) => commands::validate::run(args),
        Command::Schema => commands::schema::run().map(|_| 0),
    }
}

fn init_logging(verbose: u8, quiet: bool) {
    let level = if quiet {
        "error"
    } else {
        match verbose {
            0 => "info",
            1 => "debug",
            _ => "trace",
        }
    };
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(format!("twr_cli={level},twr_core={level},twr_report={level}")));
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .without_time()
        .with_writer(std::io::stderr)
        .try_init();
}
