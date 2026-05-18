//! `analyze` subcommand: parses inputs, runs the analysis, and emits reports.

use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use clap::Args;

use twr_core::{
    analyze_with_collector, parse_activity_csv, parse_war_csv, AnalysisReport, Config,
    ConfigOverlay, WarningCollector,
};
use twr_report::ReportOutputs;

use crate::glob::expand_war_paths;

#[derive(Debug, Args)]
pub struct AnalyzeArgs {
    /// War CSV directories, glob patterns, or individual files. Repeatable.
    #[arg(long, required = true, num_args = 1..)]
    pub wars: Vec<String>,

    /// Activity CSV path.
    #[arg(long, required = true)]
    pub activity: PathBuf,

    /// Output directory. Defaults to ./reports/<UTC_timestamp>/.
    #[arg(long)]
    pub output: Option<PathBuf>,

    /// Output formats: comma-separated subset of xlsx,csv,markdown.
    #[arg(long, value_delimiter = ',')]
    pub formats: Option<Vec<String>>,

    /// Override reference time (ISO-8601). Defaults to current UTC time.
    #[arg(long)]
    pub reference_time: Option<String>,

    #[arg(long)]
    pub low_percentile: Option<f64>,
    #[arg(long)]
    pub activity_threshold: Option<f64>,
    #[arg(long)]
    pub min_days: Option<u32>,
    #[arg(long)]
    pub zero_war_kick_threshold: Option<u32>,
    #[arg(long)]
    pub poor_war_threshold: Option<u32>,

    /// Treat warnings as errors (exit code 3).
    #[arg(long)]
    pub fail_on_warnings: bool,
}

impl AnalyzeArgs {
    fn overlay(&self) -> ConfigOverlay {
        ConfigOverlay {
            analysis: twr_core::config::AnalysisOverlay {
                low_percentile: self.low_percentile,
                min_days_for_activity: self.min_days,
                activity_threshold: self.activity_threshold,
                zero_war_kick_threshold: self.zero_war_kick_threshold,
                poor_war_threshold: self.poor_war_threshold,
            },
            output: twr_core::config::OutputOverlay {
                formats: self.formats.clone(),
            },
        }
    }
}

pub fn run(args: AnalyzeArgs, config_path: Option<&Path>) -> anyhow::Result<i32> {
    let overlay = args.overlay();
    let cfg = Config::layered(config_path, Some(&overlay))?;

    let war_paths = expand_war_paths(&args.wars)?;
    if war_paths.is_empty() {
        anyhow::bail!("no war CSV files matched the supplied --wars argument(s)");
    }

    let reference_time: DateTime<Utc> = match args.reference_time.as_deref() {
        Some(s) => DateTime::parse_from_rfc3339(s)
            .map_err(|e| anyhow::anyhow!("invalid --reference-time `{s}`: {e}"))?
            .with_timezone(&Utc),
        None => Utc::now(),
    };

    let report = build_report(&war_paths, &args.activity, &cfg, reference_time)?;

    // Determine output directory.
    let out_dir = match args.output.clone() {
        Some(p) => p,
        None => {
            let stamp = reference_time.format("%Y%m%dT%H%M%SZ").to_string();
            PathBuf::from(".").join("reports").join(stamp)
        }
    };
    std::fs::create_dir_all(&out_dir)?;

    let outputs: ReportOutputs =
        twr_report::write_all(&report, &out_dir, &cfg.output.formats)?;

    println!("wrote report bundle to {}", out_dir.display());
    if let Some(p) = &outputs.xlsx {
        println!("  xlsx: {}", p.display());
    }
    for p in &outputs.csv {
        println!("  csv : {}", p.display());
    }
    if let Some(p) = &outputs.markdown {
        println!("  md  : {}", p.display());
    }
    println!("warnings: {}", report.warnings.len());

    if args.fail_on_warnings && !report.warnings.is_empty() {
        return Ok(3);
    }
    Ok(0)
}

fn build_report(
    war_paths: &[PathBuf],
    activity_path: &Path,
    cfg: &Config,
    reference_time: DateTime<Utc>,
) -> anyhow::Result<AnalysisReport> {
    let warnings = WarningCollector::new();
    let mut wars = Vec::with_capacity(war_paths.len());
    for p in war_paths {
        wars.push(parse_war_csv(p, &warnings)?);
    }
    wars.sort_by(|a, b| {
        a.start_utc
            .cmp(&b.start_utc)
            .then_with(|| a.source_filename.cmp(&b.source_filename))
    });
    let activity = parse_activity_csv(activity_path, &warnings)?;
    Ok(analyze_with_collector(
        wars,
        activity,
        cfg,
        &warnings,
        reference_time,
    )?)
}
