//! Report writers for the Torn faction-war analyzer.
//!
//! Each output format (XLSX, CSV files, Markdown) is in its own module.

pub mod csv;
pub mod markdown;
pub mod xlsx;

use std::path::{Path, PathBuf};
use thiserror::Error;

use twr_core::AnalysisReport;

#[derive(Debug, Error)]
pub enum ReportError {
    #[error("io error writing {path}: {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("xlsx writer error for {path}: {details}")]
    Xlsx { path: String, details: String },
}

/// Generated files grouped by format. Each format may produce one or more files.
#[derive(Debug, Default)]
pub struct ReportOutputs {
    pub xlsx: Option<PathBuf>,
    pub csv: Vec<PathBuf>,
    pub markdown: Option<PathBuf>,
}

/// Write whichever formats are listed in `formats`. Unknown format strings are ignored
/// (the CLI should validate ahead of time).
pub fn write_all(
    report: &AnalysisReport,
    out_dir: &Path,
    formats: &[String],
) -> Result<ReportOutputs, ReportError> {
    std::fs::create_dir_all(out_dir).map_err(|e| ReportError::Io {
        path: out_dir.display().to_string(),
        source: e,
    })?;

    let mut out = ReportOutputs::default();
    for fmt in formats {
        match fmt.as_str() {
            "xlsx" => out.xlsx = Some(xlsx::write(report, out_dir)?),
            "csv" => out.csv = csv::write_all(report, out_dir)?,
            "markdown" | "md" => out.markdown = Some(markdown::write(report, out_dir)?),
            _ => {}
        }
    }
    Ok(out)
}
