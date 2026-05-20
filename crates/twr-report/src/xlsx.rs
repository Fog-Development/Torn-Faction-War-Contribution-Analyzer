//! XLSX report writer.

use std::path::{Path, PathBuf};

use chrono::SecondsFormat;
use rust_xlsxwriter::{Color, Format, FormatAlign, Workbook, Worksheet, XlsxError};

use twr_core::{AnalysisReport, MemberSummary, WarCategory};

use crate::ReportError;

const ARIAL: &str = "Arial";
const BODY_SIZE: f64 = 10.0;
const HEADER_SIZE: f64 = 11.0;
const HEADER_BG: u32 = 0x1155CC;
const HEADER_FG: u32 = 0xFFFFFF;
const ZERO_BG: u32 = 0xF4CCCC;
const LOW_BG: u32 = 0xFFF2CC;

#[derive(Clone)]
struct Formats {
    body: Format,
    header: Format,
    zero_cell: Format,
    low_cell: Format,
    body_bold: Format,
}

impl Formats {
    fn new() -> Self {
        Formats {
            body: Format::new().set_font_name(ARIAL).set_font_size(BODY_SIZE),
            body_bold: Format::new()
                .set_font_name(ARIAL)
                .set_font_size(BODY_SIZE)
                .set_bold(),
            header: Format::new()
                .set_font_name(ARIAL)
                .set_font_size(HEADER_SIZE)
                .set_bold()
                .set_font_color(Color::RGB(HEADER_FG))
                .set_background_color(Color::RGB(HEADER_BG))
                .set_align(FormatAlign::Center),
            zero_cell: Format::new()
                .set_font_name(ARIAL)
                .set_font_size(BODY_SIZE)
                .set_background_color(Color::RGB(ZERO_BG)),
            low_cell: Format::new()
                .set_font_name(ARIAL)
                .set_font_size(BODY_SIZE)
                .set_background_color(Color::RGB(LOW_BG)),
        }
    }
}

pub fn write(report: &AnalysisReport, out_dir: &Path) -> Result<PathBuf, ReportError> {
    std::fs::create_dir_all(out_dir).map_err(|e| ReportError::Io {
        path: out_dir.display().to_string(),
        source: e,
    })?;

    let path = out_dir.join("analysis.xlsx");

    let mut workbook = Workbook::new();
    let fmt = Formats::new();

    overview_sheet(&mut workbook, report, &fmt)?;
    auto_kick_sheet(&mut workbook, report, &fmt)?;
    repeat_offenders_sheet(&mut workbook, report, &fmt)?;
    any_bad_war_sheet(&mut workbook, report, &fmt)?;
    low_activity_sheet(&mut workbook, report, &fmt)?;
    combined_kick_sheet(&mut workbook, report, &fmt)?;
    war_matrix_sheet(&mut workbook, report, &fmt)?;
    if !report.warnings.is_empty() {
        warnings_sheet(&mut workbook, report, &fmt)?;
    }

    workbook.save(&path).map_err(map_err(&path))?;
    Ok(path)
}

fn map_err(path: &Path) -> impl Fn(XlsxError) -> ReportError + '_ {
    move |e| ReportError::Xlsx {
        path: path.display().to_string(),
        details: e.to_string(),
    }
}

fn add_sheet<'a>(wb: &'a mut Workbook, name: &str) -> Result<&'a mut Worksheet, ReportError> {
    let ws = wb.add_worksheet();
    ws.set_name(name).map_err(|e| ReportError::Xlsx {
        path: name.into(),
        details: e.to_string(),
    })?;
    Ok(ws)
}

fn write_headers(ws: &mut Worksheet, headers: &[&str], fmt: &Formats) -> Result<(), ReportError> {
    for (col, h) in headers.iter().enumerate() {
        ws.write_with_format(0, col as u16, *h, &fmt.header)
            .map_err(xerr)?;
    }
    ws.set_freeze_panes(1, 0).map_err(xerr)?;
    Ok(())
}

fn xerr(e: XlsxError) -> ReportError {
    ReportError::Xlsx {
        path: "<sheet>".into(),
        details: e.to_string(),
    }
}

fn write_str(ws: &mut Worksheet, r: u32, c: u16, s: &str, f: &Format) -> Result<(), ReportError> {
    ws.write_with_format(r, c, s, f).map_err(xerr)?;
    Ok(())
}

fn write_num(ws: &mut Worksheet, r: u32, c: u16, n: f64, f: &Format) -> Result<(), ReportError> {
    ws.write_with_format(r, c, n, f).map_err(xerr)?;
    Ok(())
}

fn fmt_opt_u32(v: Option<u32>) -> String {
    v.map(|x| x.to_string()).unwrap_or_default()
}

fn fmt_opt_f64(v: Option<f64>) -> String {
    v.map(|x| format!("{:.1}", x)).unwrap_or_default()
}

fn member_by_name<'a>(report: &'a AnalysisReport, name: &str) -> Option<&'a MemberSummary> {
    report.members.iter().find(|m| m.name == name)
}

fn overview_sheet(
    wb: &mut Workbook,
    report: &AnalysisReport,
    fmt: &Formats,
) -> Result<(), ReportError> {
    let ws = add_sheet(wb, "Overview")?;
    let mut row: u32 = 0;
    write_str(
        ws,
        row,
        0,
        "Faction War Contribution Report",
        &fmt.body_bold,
    )?;
    row += 2;
    write_str(
        ws,
        row,
        0,
        &format!(
            "Reference time: {}",
            report
                .reference_time
                .to_rfc3339_opts(SecondsFormat::Secs, true)
        ),
        &fmt.body,
    )?;
    row += 2;

    write_str(ws, row, 0, "Configured thresholds", &fmt.body_bold)?;
    row += 1;
    write_str(ws, row, 0, "Low percentile", &fmt.body)?;
    write_num(ws, row, 1, report.config.analysis.low_percentile, &fmt.body)?;
    row += 1;
    write_str(ws, row, 0, "Activity threshold (avgE30)", &fmt.body)?;
    write_num(
        ws,
        row,
        1,
        report.config.analysis.activity_threshold,
        &fmt.body,
    )?;
    row += 1;
    write_str(ws, row, 0, "Min days for activity", &fmt.body)?;
    write_num(
        ws,
        row,
        1,
        report.config.analysis.min_days_for_activity as f64,
        &fmt.body,
    )?;
    row += 1;
    write_str(ws, row, 0, "Zero-war kick threshold", &fmt.body)?;
    write_num(
        ws,
        row,
        1,
        report.config.analysis.zero_war_kick_threshold as f64,
        &fmt.body,
    )?;
    row += 1;
    write_str(ws, row, 0, "Poor-war threshold", &fmt.body)?;
    write_num(
        ws,
        row,
        1,
        report.config.analysis.poor_war_threshold as f64,
        &fmt.body,
    )?;
    row += 2;

    write_str(ws, row, 0, "Wars", &fmt.body_bold)?;
    row += 1;
    for (i, h) in [
        "War",
        "Start (UTC)",
        "Days ago",
        "Participants",
        "Low threshold",
    ]
    .iter()
    .enumerate()
    {
        ws.write_with_format(row, i as u16, *h, &fmt.header)
            .map_err(xerr)?;
    }
    row += 1;
    for (i, war) in report.wars.iter().enumerate() {
        let days_ago = (report.reference_time - war.start_utc).num_days() as f64;
        write_str(ws, row, 0, &war.display_name, &fmt.body)?;
        write_str(
            ws,
            row,
            1,
            &war.start_utc.to_rfc3339_opts(SecondsFormat::Secs, true),
            &fmt.body,
        )?;
        write_num(ws, row, 2, days_ago, &fmt.body)?;
        write_num(ws, row, 3, war.participants.len() as f64, &fmt.body)?;
        write_num(ws, row, 4, report.war_thresholds[i], &fmt.body)?;
        row += 1;
    }

    row += 1;
    write_str(
        ws,
        row,
        0,
        &format!("Warnings: {}", report.warnings.len()),
        &fmt.body_bold,
    )?;

    Ok(())
}

/// Generic helper for the four "kick-style" sheets that share a per-war point/cell layout.
/// `pick` returns the values for the leading "fixed" columns; the helper applies the
/// standard body format and the per-war zero/low cell shading.
fn write_member_sheet(
    ws: &mut Worksheet,
    report: &AnalysisReport,
    fmt: &Formats,
    fixed_headers: &[&str],
    names: &[String],
    pick: impl Fn(&MemberSummary) -> Vec<String>,
) -> Result<(), ReportError> {
    let mut all_headers: Vec<String> = fixed_headers.iter().map(|s| (*s).to_string()).collect();
    for war in &report.wars {
        all_headers.push(war.display_name.clone());
    }
    let header_refs: Vec<&str> = all_headers.iter().map(|s| s.as_str()).collect();
    write_headers(ws, &header_refs, fmt)?;

    for (rowi, name) in names.iter().enumerate() {
        let m = match member_by_name(report, name) {
            Some(m) => m,
            None => continue,
        };
        let row = (rowi + 1) as u32;

        let fixed = pick(m);
        for (col, val) in fixed.iter().enumerate() {
            if val.is_empty() {
                ws.write_with_format(row, col as u16, "", &fmt.body)
                    .map_err(xerr)?;
            } else if let Ok(n) = val.parse::<f64>() {
                ws.write_with_format(row, col as u16, n, &fmt.body)
                    .map_err(xerr)?;
            } else {
                ws.write_with_format(row, col as u16, val.as_str(), &fmt.body)
                    .map_err(xerr)?;
            }
        }

        let base = fixed.len() as u16;
        for (wi, wr) in m.wars.iter().enumerate() {
            let col = base + wi as u16;
            let cell_fmt = match wr.category {
                WarCategory::Zero => &fmt.zero_cell,
                WarCategory::Low => &fmt.low_cell,
                _ => &fmt.body,
            };
            if matches!(wr.category, WarCategory::Excluded) {
                ws.write_with_format(row, col, "—", cell_fmt)
                    .map_err(xerr)?;
            } else {
                ws.write_with_format(row, col, wr.points as f64, cell_fmt)
                    .map_err(xerr)?;
            }
        }
    }

    Ok(())
}

fn auto_kick_sheet(
    wb: &mut Workbook,
    report: &AnalysisReport,
    fmt: &Formats,
) -> Result<(), ReportError> {
    let ws = add_sheet(wb, "Auto-Kick")?;
    let fixed_headers = &[
        "Name", "Days", "Zero", "Low", "Present", "Avg Pts", "avgE30",
    ];
    write_member_sheet(ws, report, fmt, fixed_headers, &report.auto_kick, |m| {
        vec![
            m.name.clone(),
            fmt_opt_u32(m.days),
            m.zero_count.to_string(),
            m.low_count.to_string(),
            m.present_count.to_string(),
            format!("{:.1}", m.avg_points),
            fmt_opt_f64(m.avg_e30),
        ]
    })
}

fn repeat_offenders_sheet(
    wb: &mut Workbook,
    report: &AnalysisReport,
    fmt: &Formats,
) -> Result<(), ReportError> {
    let ws = add_sheet(wb, "Repeat Offenders")?;
    let fixed_headers = &["Name", "Days", "Poor", "Zero", "Low", "Present", "Avg Pts"];
    write_member_sheet(
        ws,
        report,
        fmt,
        fixed_headers,
        &report.repeat_offenders,
        |m| {
            vec![
                m.name.clone(),
                fmt_opt_u32(m.days),
                m.poor_count.to_string(),
                m.zero_count.to_string(),
                m.low_count.to_string(),
                m.present_count.to_string(),
                format!("{:.1}", m.avg_points),
            ]
        },
    )
}

fn any_bad_war_sheet(
    wb: &mut Workbook,
    report: &AnalysisReport,
    fmt: &Formats,
) -> Result<(), ReportError> {
    let ws = add_sheet(wb, "Any Single Bad War")?;
    let fixed_headers = &[
        "Name",
        "Days",
        "Flagged Wars",
        "Zero Wars",
        "Low Wars",
        "Avg Pts",
    ];
    write_member_sheet(ws, report, fmt, fixed_headers, &report.any_bad_war, |m| {
        vec![
            m.name.clone(),
            fmt_opt_u32(m.days),
            (m.zero_count + m.low_count).to_string(),
            m.zero_count.to_string(),
            m.low_count.to_string(),
            format!("{:.1}", m.avg_points),
        ]
    })
}

fn low_activity_sheet(
    wb: &mut Workbook,
    report: &AnalysisReport,
    fmt: &Formats,
) -> Result<(), ReportError> {
    let ws = add_sheet(wb, "Low avgE30")?;
    let headers = &[
        "Name", "Days", "avgE30", "Present", "Zero", "Low", "Avg Pts",
    ];
    write_headers(ws, headers, fmt)?;

    for (rowi, name) in report.low_activity.iter().enumerate() {
        let m = match member_by_name(report, name) {
            Some(m) => m,
            None => continue,
        };
        let row = (rowi + 1) as u32;
        write_str(ws, row, 0, &m.name, &fmt.body)?;
        if let Some(d) = m.days {
            write_num(ws, row, 1, d as f64, &fmt.body)?;
        }
        if let Some(a) = m.avg_e30 {
            write_num(ws, row, 2, a, &fmt.body)?;
        }
        write_num(ws, row, 3, m.present_count as f64, &fmt.body)?;
        write_num(ws, row, 4, m.zero_count as f64, &fmt.body)?;
        write_num(ws, row, 5, m.low_count as f64, &fmt.body)?;
        write_num(ws, row, 6, m.avg_points, &fmt.body)?;
    }
    Ok(())
}

fn combined_kick_sheet(
    wb: &mut Workbook,
    report: &AnalysisReport,
    fmt: &Formats,
) -> Result<(), ReportError> {
    let ws = add_sheet(wb, "Combined Kick List")?;
    let headers = &[
        "Name",
        "Days",
        "avgE30",
        "Poor",
        "Zero",
        "Low",
        "Avg Pts",
        "Auto-Kick",
    ];
    write_headers(ws, headers, fmt)?;

    for (rowi, name) in report.combined_kick.iter().enumerate() {
        let m = match member_by_name(report, name) {
            Some(m) => m,
            None => continue,
        };
        let row = (rowi + 1) as u32;
        write_str(ws, row, 0, &m.name, &fmt.body)?;
        if let Some(d) = m.days {
            write_num(ws, row, 1, d as f64, &fmt.body)?;
        }
        if let Some(a) = m.avg_e30 {
            write_num(ws, row, 2, a, &fmt.body)?;
        }
        write_num(ws, row, 3, m.poor_count as f64, &fmt.body)?;
        write_num(ws, row, 4, m.zero_count as f64, &fmt.body)?;
        write_num(ws, row, 5, m.low_count as f64, &fmt.body)?;
        write_num(ws, row, 6, m.avg_points, &fmt.body)?;
        let auto = report.auto_kick.iter().any(|n| n == name);
        write_str(ws, row, 7, if auto { "yes" } else { "no" }, &fmt.body)?;
    }
    Ok(())
}

fn war_matrix_sheet(
    wb: &mut Workbook,
    report: &AnalysisReport,
    fmt: &Formats,
) -> Result<(), ReportError> {
    let ws = add_sheet(wb, "War Matrix")?;
    let mut headers: Vec<String> = vec!["Name".into(), "Days".into(), "avgE30".into()];
    for war in &report.wars {
        headers.push(format!("{} pts", war.display_name));
        headers.push(format!("{} hits", war.display_name));
    }
    let header_refs: Vec<&str> = headers.iter().map(|s| s.as_str()).collect();
    write_headers(ws, &header_refs, fmt)?;

    for (rowi, m) in report.members.iter().enumerate() {
        let row = (rowi + 1) as u32;
        write_str(ws, row, 0, &m.name, &fmt.body)?;
        if let Some(d) = m.days {
            write_num(ws, row, 1, d as f64, &fmt.body)?;
        }
        if let Some(a) = m.avg_e30 {
            write_num(ws, row, 2, a, &fmt.body)?;
        }
        for (wi, wr) in m.wars.iter().enumerate() {
            let col = 3 + (wi as u16 * 2);
            let cell_fmt = match wr.category {
                WarCategory::Zero => &fmt.zero_cell,
                WarCategory::Low => &fmt.low_cell,
                _ => &fmt.body,
            };
            if matches!(wr.category, WarCategory::Excluded) {
                ws.write_with_format(row, col, "—", cell_fmt)
                    .map_err(xerr)?;
                ws.write_with_format(row, col + 1, "—", cell_fmt)
                    .map_err(xerr)?;
            } else {
                ws.write_with_format(row, col, wr.points as f64, cell_fmt)
                    .map_err(xerr)?;
                ws.write_with_format(row, col + 1, wr.hits as f64, cell_fmt)
                    .map_err(xerr)?;
            }
        }
    }
    Ok(())
}

fn warnings_sheet(
    wb: &mut Workbook,
    report: &AnalysisReport,
    fmt: &Formats,
) -> Result<(), ReportError> {
    let ws = add_sheet(wb, "Warnings")?;
    let headers = &["Kind", "Source", "Context", "Detail"];
    write_headers(ws, headers, fmt)?;
    for (i, w) in report.warnings.iter().enumerate() {
        let row = (i + 1) as u32;
        write_str(ws, row, 0, &w.kind.to_string(), &fmt.body)?;
        write_str(ws, row, 1, &w.source, &fmt.body)?;
        write_str(
            ws,
            row,
            2,
            w.row_or_member.as_deref().unwrap_or(""),
            &fmt.body,
        )?;
        write_str(ws, row, 3, &w.detail, &fmt.body)?;
    }
    Ok(())
}
