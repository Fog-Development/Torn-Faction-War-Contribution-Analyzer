//! Markdown summary report writer.

use std::fmt::Write as _;
use std::fs::File;
use std::io::Write as _;
use std::path::{Path, PathBuf};

use chrono::{SecondsFormat, Utc};

use twr_core::{AnalysisReport, MemberSummary};

use crate::ReportError;

const METHODOLOGY: &str = "\
A member is considered **present** for a given war when their tenure in the faction (`Days`) \
is strictly greater than the number of integer days between the war's UTC start time and the \
report's reference time. For each war we compute a **low-points threshold** as the configured \
percentile (default 20th) of the points scored by present members who scored at least one point, \
using linear interpolation. A member's war-participation is then classified as one of:

- **Zero** — present but scored 0 points
- **Low** — present and scored at or below the low threshold
- **Ok** — present and scored above the low threshold
- **Excluded** — not present (insufficient tenure or absent from the war CSV)

Rollup lists are derived from the per-member counts and the optional activity feed.";

pub fn write(report: &AnalysisReport, out_dir: &Path) -> Result<PathBuf, ReportError> {
    std::fs::create_dir_all(out_dir).map_err(|e| ReportError::Io {
        path: out_dir.display().to_string(),
        source: e,
    })?;
    let path = out_dir.join("summary.md");
    let mut f = File::create(&path).map_err(|e| ReportError::Io {
        path: path.display().to_string(),
        source: e,
    })?;
    let body = render(report);
    f.write_all(body.as_bytes()).map_err(|e| ReportError::Io {
        path: path.display().to_string(),
        source: e,
    })?;
    Ok(path)
}

/// Render the markdown report to a string. Deterministic given a report.
pub fn render(report: &AnalysisReport) -> String {
    let mut s = String::new();
    let generated = Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);
    let _ = writeln!(s, "# Faction War Contribution Report");
    let _ = writeln!(s);
    let _ = writeln!(s, "Generated: {generated}");
    let _ = writeln!(
        s,
        "Reference time: {}",
        report
            .reference_time
            .to_rfc3339_opts(SecondsFormat::Secs, true)
    );
    let _ = writeln!(s);

    // --- Wars analysed ---
    let _ = writeln!(s, "## Wars analyzed");
    let _ = writeln!(s);
    let _ = writeln!(
        s,
        "| War | Date (UTC) | Days ago | Participants | Low threshold |"
    );
    let _ = writeln!(s, "|---|---|---|---|---|");
    for (i, war) in report.wars.iter().enumerate() {
        let days_ago = (report.reference_time - war.start_utc).num_days();
        let _ = writeln!(
            s,
            "| {} | {} | {} | {} | {:.1} |",
            war.display_name,
            war.start_utc.to_rfc3339_opts(SecondsFormat::Secs, true),
            days_ago,
            war.participants.len(),
            report.war_thresholds[i]
        );
    }
    let _ = writeln!(s);

    // --- Methodology ---
    let _ = writeln!(s, "## Methodology");
    let _ = writeln!(s);
    let _ = writeln!(s, "{METHODOLOGY}");
    let _ = writeln!(s);

    // --- Warnings ---
    if !report.warnings.is_empty() {
        let _ = writeln!(s, "## ⚠ Warnings ({})", report.warnings.len());
        let _ = writeln!(s);
        let _ = writeln!(s, "| Kind | Source | Context | Detail |");
        let _ = writeln!(s, "|---|---|---|---|");
        for w in &report.warnings {
            let _ = writeln!(
                s,
                "| {} | {} | {} | {} |",
                w.kind,
                escape_md(&w.source),
                escape_md(w.row_or_member.as_deref().unwrap_or("")),
                escape_md(&w.detail),
            );
        }
        let _ = writeln!(s);
    }

    // --- Auto-Kick ---
    let _ = writeln!(
        s,
        "## 🚨 Auto-Kick List ({} members)",
        report.auto_kick.len()
    );
    let _ = writeln!(s);
    render_kick_table(&mut s, report, &report.auto_kick);
    let _ = writeln!(s);

    // --- Repeat Offenders ---
    let _ = writeln!(
        s,
        "## Repeat Offenders ({} members)",
        report.repeat_offenders.len()
    );
    let _ = writeln!(s);
    render_kick_table(&mut s, report, &report.repeat_offenders);
    let _ = writeln!(s);

    // --- Low Activity ---
    let _ = writeln!(
        s,
        "## Low Activity — avgE30 < {} ({} members)",
        report.config.analysis.activity_threshold,
        report.low_activity.len()
    );
    let _ = writeln!(s);
    render_low_activity(&mut s, report);
    let _ = writeln!(s);

    // --- Combined Kick ---
    let _ = writeln!(
        s,
        "## 🚨 Combined Kick List ({} members)",
        report.combined_kick.len()
    );
    let _ = writeln!(s);
    render_combined(&mut s, report);

    s
}

fn render_kick_table(s: &mut String, report: &AnalysisReport, names: &[String]) {
    if names.is_empty() {
        let _ = writeln!(s, "_(none)_");
        return;
    }
    let _ = writeln!(
        s,
        "| Member | Days | Present | Zero | Low | Poor | Avg Pts | avgE30 |"
    );
    let _ = writeln!(s, "|---|---|---|---|---|---|---|---|");
    for name in names {
        if let Some(m) = lookup(report, name) {
            let _ = writeln!(
                s,
                "| {} | {} | {} | {} | {} | {} | {} | {} |",
                escape_md(&m.name),
                fmt_opt_u32(m.days),
                m.present_count,
                m.zero_count,
                m.low_count,
                m.poor_count,
                fmt_avg(m.avg_points),
                fmt_opt_f64(m.avg_e30),
            );
        }
    }
}

fn render_low_activity(s: &mut String, report: &AnalysisReport) {
    if report.low_activity.is_empty() {
        let _ = writeln!(s, "_(none)_");
        return;
    }
    let _ = writeln!(
        s,
        "| Member | Days | avgE30 | Present | Zero | Low | Avg Pts |"
    );
    let _ = writeln!(s, "|---|---|---|---|---|---|---|");
    for name in &report.low_activity {
        if let Some(m) = lookup(report, name) {
            let _ = writeln!(
                s,
                "| {} | {} | {} | {} | {} | {} | {} |",
                escape_md(&m.name),
                fmt_opt_u32(m.days),
                fmt_opt_f64(m.avg_e30),
                m.present_count,
                m.zero_count,
                m.low_count,
                fmt_avg(m.avg_points),
            );
        }
    }
}

fn render_combined(s: &mut String, report: &AnalysisReport) {
    if report.combined_kick.is_empty() {
        let _ = writeln!(s, "_(none)_");
        return;
    }
    let _ = writeln!(
        s,
        "| Member | Days | avgE30 | Poor | Zero | Low | Avg Pts | Auto-Kick |"
    );
    let _ = writeln!(s, "|---|---|---|---|---|---|---|---|");
    for name in &report.combined_kick {
        if let Some(m) = lookup(report, name) {
            let auto = report.auto_kick.iter().any(|n| n == name);
            let _ = writeln!(
                s,
                "| {} | {} | {} | {} | {} | {} | {} | {} |",
                escape_md(&m.name),
                fmt_opt_u32(m.days),
                fmt_opt_f64(m.avg_e30),
                m.poor_count,
                m.zero_count,
                m.low_count,
                fmt_avg(m.avg_points),
                if auto { "yes" } else { "no" },
            );
        }
    }
}

fn lookup<'a>(report: &'a AnalysisReport, name: &str) -> Option<&'a MemberSummary> {
    report.members.iter().find(|m| m.name == name)
}

fn fmt_avg(p: f64) -> String {
    if p == 0.0 {
        "0".to_string()
    } else {
        format!("{:.1}", p)
    }
}

fn fmt_opt_u32(v: Option<u32>) -> String {
    v.map(|x| x.to_string()).unwrap_or_else(|| "—".into())
}

fn fmt_opt_f64(v: Option<f64>) -> String {
    v.map(|x| format!("{:.1}", x)).unwrap_or_else(|| "—".into())
}

fn escape_md(s: &str) -> String {
    s.replace('|', "\\|")
}

/// A variant of `render` that uses a fixed "Generated:" timestamp — used by snapshot tests
/// so the output is deterministic.
pub fn render_for_snapshot(report: &AnalysisReport) -> String {
    let real = render(report);
    // Replace the "Generated: ..." line with a stable token.
    let mut out = String::with_capacity(real.len());
    for line in real.lines() {
        if let Some(rest) = line.strip_prefix("Generated: ") {
            let _ = (rest,);
            out.push_str("Generated: <FIXED>");
        } else {
            out.push_str(line);
        }
        out.push('\n');
    }
    out
}
