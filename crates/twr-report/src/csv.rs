//! CSV report writers: one file per list, plus a wide war-matrix file.

use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

use twr_core::{AnalysisReport, MemberSummary, WarCategory};

use crate::ReportError;

/// Convenience: write `\r\n`-stripped CSV with `\n` line endings, as required by the spec.
fn write_record<W: Write>(w: &mut W, fields: &[String]) -> std::io::Result<()> {
    for (i, f) in fields.iter().enumerate() {
        if i > 0 {
            w.write_all(b",")?;
        }
        write_field(w, f)?;
    }
    w.write_all(b"\n")
}

fn write_field<W: Write>(w: &mut W, value: &str) -> std::io::Result<()> {
    let needs_quote = value.contains(',') || value.contains('"') || value.contains('\n');
    if needs_quote {
        let escaped = value.replace('"', "\"\"");
        write!(w, "\"{}\"", escaped)
    } else {
        w.write_all(value.as_bytes())
    }
}

fn fmt_avg(p: f64) -> String {
    if p == 0.0 {
        "0".to_string()
    } else {
        format!("{:.1}", p)
    }
}

fn fmt_opt_u32(v: Option<u32>) -> String {
    v.map(|x| x.to_string()).unwrap_or_default()
}

fn fmt_opt_f64(v: Option<f64>) -> String {
    v.map(|x| format!("{:.1}", x)).unwrap_or_default()
}

/// Write all CSV files into `out_dir`, returning the list of files written.
pub fn write_all(report: &AnalysisReport, out_dir: &Path) -> Result<Vec<std::path::PathBuf>, ReportError> {
    std::fs::create_dir_all(out_dir).map_err(|e| ReportError::Io {
        path: out_dir.display().to_string(),
        source: e,
    })?;

    let mut written = Vec::new();
    written.push(write_auto_kick(report, out_dir)?);
    written.push(write_repeat_offenders(report, out_dir)?);
    written.push(write_any_bad_war(report, out_dir)?);
    written.push(write_low_activity(report, out_dir)?);
    written.push(write_combined_kick(report, out_dir)?);
    written.push(write_war_matrix(report, out_dir)?);
    if !report.warnings.is_empty() {
        written.push(write_warnings(report, out_dir)?);
    }
    Ok(written)
}

fn open(out_dir: &Path, filename: &str) -> Result<(BufWriter<File>, std::path::PathBuf), ReportError> {
    let path = out_dir.join(filename);
    let f = File::create(&path).map_err(|e| ReportError::Io {
        path: path.display().to_string(),
        source: e,
    })?;
    Ok((BufWriter::new(f), path))
}

fn member_by_name<'a>(report: &'a AnalysisReport, name: &str) -> Option<&'a MemberSummary> {
    report.members.iter().find(|m| m.name == name)
}

pub fn write_auto_kick(
    report: &AnalysisReport,
    out_dir: &Path,
) -> Result<std::path::PathBuf, ReportError> {
    let (mut w, path) = open(out_dir, "auto_kick.csv")?;
    let mut headers: Vec<String> = vec![
        "name".into(),
        "days".into(),
        "zero_count".into(),
        "low_count".into(),
        "present_count".into(),
        "avg_points".into(),
        "avgE30".into(),
    ];
    for war in &report.wars {
        headers.push(format!("{} (points)", war.display_name));
    }
    write_record(&mut w, &headers).map_err(io_err(&path))?;

    for name in &report.auto_kick {
        if let Some(m) = member_by_name(report, name) {
            let mut row = vec![
                m.name.clone(),
                fmt_opt_u32(m.days),
                m.zero_count.to_string(),
                m.low_count.to_string(),
                m.present_count.to_string(),
                fmt_avg(m.avg_points),
                fmt_opt_f64(m.avg_e30),
            ];
            for wr in &m.wars {
                row.push(format_war_cell(wr));
            }
            write_record(&mut w, &row).map_err(io_err(&path))?;
        }
    }
    Ok(path)
}

pub fn write_repeat_offenders(
    report: &AnalysisReport,
    out_dir: &Path,
) -> Result<std::path::PathBuf, ReportError> {
    let (mut w, path) = open(out_dir, "repeat_offenders.csv")?;
    let mut headers: Vec<String> = vec![
        "name".into(),
        "days".into(),
        "poor_count".into(),
        "zero_count".into(),
        "low_count".into(),
        "present_count".into(),
        "avg_points".into(),
    ];
    for war in &report.wars {
        headers.push(format!("{} (points)", war.display_name));
    }
    write_record(&mut w, &headers).map_err(io_err(&path))?;

    for name in &report.repeat_offenders {
        if let Some(m) = member_by_name(report, name) {
            let mut row = vec![
                m.name.clone(),
                fmt_opt_u32(m.days),
                m.poor_count.to_string(),
                m.zero_count.to_string(),
                m.low_count.to_string(),
                m.present_count.to_string(),
                fmt_avg(m.avg_points),
            ];
            for wr in &m.wars {
                row.push(format_war_cell(wr));
            }
            write_record(&mut w, &row).map_err(io_err(&path))?;
        }
    }
    Ok(path)
}

pub fn write_any_bad_war(
    report: &AnalysisReport,
    out_dir: &Path,
) -> Result<std::path::PathBuf, ReportError> {
    let (mut w, path) = open(out_dir, "any_bad_war.csv")?;
    let mut headers: Vec<String> = vec![
        "name".into(),
        "days".into(),
        "flagged_wars".into(),
        "zero_wars".into(),
        "low_wars".into(),
        "avg_points".into(),
    ];
    for war in &report.wars {
        headers.push(format!("{} (cat)", war.display_name));
    }
    write_record(&mut w, &headers).map_err(io_err(&path))?;

    for name in &report.any_bad_war {
        if let Some(m) = member_by_name(report, name) {
            let mut row = vec![
                m.name.clone(),
                fmt_opt_u32(m.days),
                (m.zero_count + m.low_count).to_string(),
                m.zero_count.to_string(),
                m.low_count.to_string(),
                fmt_avg(m.avg_points),
            ];
            for wr in &m.wars {
                row.push(wr.category.as_str().to_string());
            }
            write_record(&mut w, &row).map_err(io_err(&path))?;
        }
    }
    Ok(path)
}

pub fn write_low_activity(
    report: &AnalysisReport,
    out_dir: &Path,
) -> Result<std::path::PathBuf, ReportError> {
    let (mut w, path) = open(out_dir, "low_activity.csv")?;
    let headers: Vec<String> = vec![
        "name".into(),
        "days".into(),
        "avgE30".into(),
        "present_count".into(),
        "zero_count".into(),
        "low_count".into(),
        "avg_points".into(),
    ];
    write_record(&mut w, &headers).map_err(io_err(&path))?;

    for name in &report.low_activity {
        if let Some(m) = member_by_name(report, name) {
            let row = vec![
                m.name.clone(),
                fmt_opt_u32(m.days),
                fmt_opt_f64(m.avg_e30),
                m.present_count.to_string(),
                m.zero_count.to_string(),
                m.low_count.to_string(),
                fmt_avg(m.avg_points),
            ];
            write_record(&mut w, &row).map_err(io_err(&path))?;
        }
    }
    Ok(path)
}

pub fn write_combined_kick(
    report: &AnalysisReport,
    out_dir: &Path,
) -> Result<std::path::PathBuf, ReportError> {
    let (mut w, path) = open(out_dir, "combined_kick.csv")?;
    let headers: Vec<String> = vec![
        "name".into(),
        "days".into(),
        "avgE30".into(),
        "poor_count".into(),
        "zero_count".into(),
        "low_count".into(),
        "avg_points".into(),
        "auto_kick".into(),
    ];
    write_record(&mut w, &headers).map_err(io_err(&path))?;

    for name in &report.combined_kick {
        if let Some(m) = member_by_name(report, name) {
            let auto = report.auto_kick.iter().any(|n| n == name);
            let row = vec![
                m.name.clone(),
                fmt_opt_u32(m.days),
                fmt_opt_f64(m.avg_e30),
                m.poor_count.to_string(),
                m.zero_count.to_string(),
                m.low_count.to_string(),
                fmt_avg(m.avg_points),
                if auto { "yes".into() } else { "no".into() },
            ];
            write_record(&mut w, &row).map_err(io_err(&path))?;
        }
    }
    Ok(path)
}

pub fn write_war_matrix(
    report: &AnalysisReport,
    out_dir: &Path,
) -> Result<std::path::PathBuf, ReportError> {
    let (mut w, path) = open(out_dir, "war_matrix.csv")?;
    let mut headers: Vec<String> = vec!["name".into(), "days".into(), "avgE30".into()];
    for war in &report.wars {
        headers.push(format!("{} (points)", war.display_name));
        headers.push(format!("{} (hits)", war.display_name));
        headers.push(format!("{} (cat)", war.display_name));
    }
    write_record(&mut w, &headers).map_err(io_err(&path))?;

    for m in &report.members {
        let mut row = vec![
            m.name.clone(),
            fmt_opt_u32(m.days),
            fmt_opt_f64(m.avg_e30),
        ];
        for wr in &m.wars {
            row.push(wr.points.to_string());
            row.push(wr.hits.to_string());
            row.push(wr.category.as_str().to_string());
        }
        write_record(&mut w, &row).map_err(io_err(&path))?;
    }
    Ok(path)
}

pub fn write_warnings(
    report: &AnalysisReport,
    out_dir: &Path,
) -> Result<std::path::PathBuf, ReportError> {
    let (mut w, path) = open(out_dir, "warnings.csv")?;
    write_record(&mut w, &["kind".into(), "source".into(), "context".into(), "detail".into()])
        .map_err(io_err(&path))?;
    for warn in &report.warnings {
        write_record(
            &mut w,
            &[
                warn.kind.to_string(),
                warn.source.clone(),
                warn.row_or_member.clone().unwrap_or_default(),
                warn.detail.clone(),
            ],
        )
        .map_err(io_err(&path))?;
    }
    Ok(path)
}

fn format_war_cell(wr: &twr_core::MemberWarResult) -> String {
    match wr.category {
        WarCategory::Excluded => "—".to_string(),
        _ => wr.points.to_string(),
    }
}

fn io_err(path: &Path) -> impl Fn(std::io::Error) -> ReportError + '_ {
    move |e| ReportError::Io {
        path: path.display().to_string(),
        source: e,
    }
}
