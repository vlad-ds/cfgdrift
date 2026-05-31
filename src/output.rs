use std::collections::BTreeMap;

use anyhow::Result;
use serde::Serialize;
use serde_json::Value;

use crate::cli::OutputFormat;
use crate::diff::{Drift, DriftKind, DriftReport};

pub fn render_report(
    report: &DriftReport,
    format: OutputFormat,
    summary_only: bool,
) -> Result<String> {
    match format {
        OutputFormat::Human => Ok(render_human(report, summary_only)),
        OutputFormat::Json => {
            if summary_only {
                Ok(serde_json::to_string_pretty(&ReportSummary::from(report))?)
            } else {
                Ok(serde_json::to_string_pretty(report)?)
            }
        }
        OutputFormat::Markdown => Ok(render_markdown(report, summary_only)),
    }
}

fn render_human(report: &DriftReport, summary_only: bool) -> String {
    if !report.has_drift() {
        return "No config drift found.".to_string();
    }

    let mut output = format!(
        "cfgdrift found {} between {} and {}",
        pluralize(report.drift_count, "drift"),
        report.baseline,
        report.candidate
    );

    if summary_only {
        output.push('\n');
        output.push_str(&render_human_summary(report));
        return output;
    }

    for drift in &report.drifts {
        output.push_str("\n\n");
        output.push_str(&render_human_drift(drift));
    }

    output
}

fn render_human_summary(report: &DriftReport) -> String {
    report
        .counts_by_kind()
        .into_iter()
        .map(|(kind, count)| format!("{}: {count}", kind_label(kind)))
        .collect::<Vec<_>>()
        .join("\n")
}

fn render_human_drift(drift: &Drift) -> String {
    match drift.kind {
        DriftKind::Added => format!(
            "+ {} = {}",
            drift.location,
            display_optional_value(drift.candidate.as_ref())
        ),
        DriftKind::Removed => format!(
            "- {} = {}",
            drift.location,
            display_optional_value(drift.baseline.as_ref())
        ),
        DriftKind::Changed | DriftKind::TypeChanged => format!(
            "{} {}\n  baseline:  {}\n  candidate: {}",
            if drift.kind == DriftKind::TypeChanged {
                "!"
            } else {
                "~"
            },
            drift.location,
            display_optional_value(drift.baseline.as_ref()),
            display_optional_value(drift.candidate.as_ref())
        ),
        DriftKind::FileAdded => format!("+ {}", drift.location),
        DriftKind::FileRemoved => format!("- {}", drift.location),
    }
}

fn render_markdown(report: &DriftReport, summary_only: bool) -> String {
    if !report.has_drift() {
        return "No config drift found.".to_string();
    }

    if summary_only {
        let mut output = format!(
            "cfgdrift found {} between `{}` and `{}`.\n\n",
            pluralize(report.drift_count, "drift"),
            escape_markdown(&report.baseline),
            escape_markdown(&report.candidate)
        );
        output.push_str("| Type | Count |\n| --- | ---: |\n");
        for (kind, count) in report.counts_by_kind() {
            output.push_str(&format!("| {} | {count} |\n", kind_label(kind)));
        }
        return output.trim_end().to_string();
    }

    let mut output = format!(
        "cfgdrift found {} between `{}` and `{}`.\n\n",
        pluralize(report.drift_count, "drift"),
        escape_markdown(&report.baseline),
        escape_markdown(&report.candidate)
    );
    output.push_str("| Type | Location | Baseline | Candidate |\n");
    output.push_str("| --- | --- | --- | --- |\n");

    for drift in &report.drifts {
        output.push_str(&format!(
            "| {} | `{}` | {} | {} |\n",
            kind_label(drift.kind),
            escape_markdown(&drift.location),
            markdown_value(drift.baseline.as_ref()),
            markdown_value(drift.candidate.as_ref())
        ));
    }

    output.trim_end().to_string()
}

fn markdown_value(value: Option<&Value>) -> String {
    let Some(value) = value else {
        return String::new();
    };

    format!("`{}`", escape_markdown(&display_value(value)))
}

fn display_optional_value(value: Option<&Value>) -> String {
    value
        .map(display_value)
        .unwrap_or_else(|| "<missing>".to_string())
}

fn display_value(value: &Value) -> String {
    match value {
        Value::String(value) => {
            if value == "<redacted>" {
                value.clone()
            } else {
                serde_json::to_string(value).unwrap_or_else(|_| "\"?\"".to_string())
            }
        }
        _ => serde_json::to_string(value).unwrap_or_else(|_| "<unprintable>".to_string()),
    }
}

fn escape_markdown(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('`', "\\`")
        .replace('|', "\\|")
        .replace('\n', " ")
}

fn kind_label(kind: DriftKind) -> &'static str {
    match kind {
        DriftKind::Added => "added",
        DriftKind::Removed => "removed",
        DriftKind::Changed => "changed",
        DriftKind::TypeChanged => "type_changed",
        DriftKind::FileAdded => "file_added",
        DriftKind::FileRemoved => "file_removed",
    }
}

fn pluralize(count: usize, noun: &str) -> String {
    if count == 1 {
        format!("1 {noun}")
    } else {
        format!("{count} {noun}s")
    }
}

#[derive(Debug, Serialize)]
struct ReportSummary<'a> {
    baseline: &'a str,
    candidate: &'a str,
    drift_count: usize,
    by_type: BTreeMap<DriftKind, usize>,
}

impl<'a> From<&'a DriftReport> for ReportSummary<'a> {
    fn from(report: &'a DriftReport) -> Self {
        Self {
            baseline: &report.baseline,
            candidate: &report.candidate,
            drift_count: report.drift_count,
            by_type: report.counts_by_kind(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn json_summary_contains_counts() {
        let report = DriftReport {
            baseline: "base".to_string(),
            candidate: "candidate".to_string(),
            drift_count: 1,
            drifts: vec![Drift {
                kind: DriftKind::Changed,
                file: "app.toml".to_string(),
                path: "database.pool".to_string(),
                location: "app.toml:database.pool".to_string(),
                baseline: Some(json!(8)),
                candidate: Some(json!(16)),
            }],
        };

        let rendered = render_report(&report, OutputFormat::Json, true).unwrap();
        assert!(rendered.contains("\"changed\": 1"));
    }
}
