//! `analyze` subcommand: parses inputs, runs the analysis, and emits reports.

use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use clap::Args;
use serde::Serialize;

use twr_core::{
    analyze_with_collector, parse_activity_csv, parse_war_csv, AnalysisReport, Config,
    ConfigOverlay, WarningCollector,
};
use twr_report::ReportOutputs;

use crate::events::{self, Event, ListSizes, OutputPaths, Progress};
use crate::glob::expand_war_paths;
use crate::EmitMode;

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
    pub fn overlay(&self) -> ConfigOverlay {
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

pub fn run(args: AnalyzeArgs, config_path: Option<&Path>, emit: EmitMode) -> anyhow::Result<i32> {
    let overlay = args.overlay();
    let cfg = Config::layered(config_path, Some(&overlay))?;

    let reference_time: DateTime<Utc> = match args.reference_time.as_deref() {
        Some(s) => DateTime::parse_from_rfc3339(s)
            .map_err(|e| anyhow::anyhow!("invalid --reference-time `{s}`: {e}"))?
            .with_timezone(&Utc),
        None => Utc::now(),
    };

    if emit == EmitMode::Json {
        events::emit(&Event::Start {
            subcommand: "analyze".into(),
            config: cfg.analysis.clone(),
            reference_time: reference_time.to_rfc3339(),
        });
    }

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
    let mut wars = Vec::with_capacity(total);
    for (i, p) in war_paths.iter().enumerate() {
        if emit == EmitMode::Json {
            events::emit(&Event::Progress(Progress::ParseWar {
                current: i + 1,
                total,
                file: p.display().to_string(),
            }));
        }
        wars.push(parse_war_csv(p, &warnings)?);
    }
    wars.sort_by(|a, b| {
        a.start_utc
            .cmp(&b.start_utc)
            .then_with(|| a.source_filename.cmp(&b.source_filename))
    });

    if emit == EmitMode::Json {
        events::emit(&Event::Progress(Progress::ParseActivity {
            file: args.activity.display().to_string(),
        }));
    }
    let activity = parse_activity_csv(&args.activity, &warnings)?;

    if emit == EmitMode::Json {
        events::emit(&Event::Progress(Progress::Analyze));
    }
    let report = analyze_with_collector(wars, activity, &cfg, &warnings, reference_time)?;

    // Emit per-warning events before writing output.
    if emit == EmitMode::Json {
        for w in &report.warnings {
            events::emit(&Event::Warning {
                kind: w.kind.to_string(),
                source: w.source.clone(),
                context: w.row_or_member.clone(),
                message: w.detail.clone(),
            });
        }
    }

    // Determine output directory.
    let out_dir = match args.output.clone() {
        Some(p) => p,
        None => {
            let stamp = reference_time.format("%Y%m%dT%H%M%SZ").to_string();
            PathBuf::from(".").join("reports").join(stamp)
        }
    };
    std::fs::create_dir_all(&out_dir)?;

    let outputs: ReportOutputs = twr_report::write_all(&report, &out_dir, &cfg.output.formats)?;

    if emit == EmitMode::Json {
        if let Some(p) = &outputs.xlsx {
            events::emit(&Event::Progress(Progress::Write {
                format: "xlsx".into(),
                path: p.display().to_string(),
            }));
        }
        for p in &outputs.csv {
            events::emit(&Event::Progress(Progress::Write {
                format: "csv".into(),
                path: p.display().to_string(),
            }));
        }
        if let Some(p) = &outputs.markdown {
            events::emit(&Event::Progress(Progress::Write {
                format: "markdown".into(),
                path: p.display().to_string(),
            }));
        }
    }

    let out_paths = OutputPaths {
        xlsx: outputs.xlsx.as_ref().map(|p| p.display().to_string()),
        csv: outputs.csv.iter().map(|p| p.display().to_string()).collect(),
        markdown: outputs.markdown.as_ref().map(|p| p.display().to_string()),
    };
    let list_sizes = ListSizes {
        auto_kick: report.auto_kick.len(),
        repeat_offenders: report.repeat_offenders.len(),
        any_bad_war: report.any_bad_war.len(),
        low_activity: report.low_activity.len(),
        combined_kick: report.combined_kick.len(),
    };
    let warning_count = report.warnings.len();

    let exit_code = if args.fail_on_warnings && !report.warnings.is_empty() {
        3
    } else {
        0
    };

    // Always write run.json manifest.
    write_run_manifest(&out_dir, &report, &out_paths, &list_sizes, exit_code, &args)?;

    if emit == EmitMode::Json {
        events::emit(&Event::Done {
            exit_code,
            output_dir: Some(out_dir.display().to_string()),
            outputs: out_paths,
            warning_count,
            list_sizes,
        });
    } else {
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
        println!("warnings: {warning_count}");
    }

    Ok(exit_code)
}

#[derive(Serialize)]
struct RunManifest<'a> {
    reference_time: String,
    config: &'a twr_core::config::AnalysisConfig,
    input_war_files: Vec<String>,
    input_activity_file: String,
    output_dir: String,
    outputs: &'a OutputPaths,
    list_sizes: &'a ListSizes,
    warning_count: usize,
    exit_code: i32,
}

fn write_run_manifest(
    out_dir: &Path,
    report: &AnalysisReport,
    outputs: &OutputPaths,
    list_sizes: &ListSizes,
    exit_code: i32,
    args: &AnalyzeArgs,
) -> anyhow::Result<()> {
    let manifest = RunManifest {
        reference_time: report.reference_time.to_rfc3339(),
        config: &report.config.analysis,
        input_war_files: args.wars.clone(),
        input_activity_file: args.activity.display().to_string(),
        output_dir: out_dir.display().to_string(),
        outputs,
        list_sizes,
        warning_count: report.warnings.len(),
        exit_code,
    };
    let json = serde_json::to_string_pretty(&manifest)?;
    std::fs::write(out_dir.join("run.json"), json)?;
    Ok(())
}
