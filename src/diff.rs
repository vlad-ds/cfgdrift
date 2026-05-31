use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use anyhow::{Context, Result, anyhow, bail};
use serde::Serialize;
use serde_json::Value;

use crate::ignore::IgnoreMatcher;
use crate::{load, redact};

#[derive(Debug, Clone)]
pub struct CompareOptions {
    pub redact: bool,
    pub ignore: IgnoreMatcher,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DriftKind {
    Added,
    Removed,
    Changed,
    TypeChanged,
    FileAdded,
    FileRemoved,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
pub struct Drift {
    #[serde(rename = "type")]
    pub kind: DriftKind,
    pub file: String,
    pub path: String,
    pub location: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub baseline: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub candidate: Option<Value>,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
pub struct DriftReport {
    pub baseline: String,
    pub candidate: String,
    pub drift_count: usize,
    pub drifts: Vec<Drift>,
}

impl DriftReport {
    pub fn has_drift(&self) -> bool {
        self.drift_count > 0
    }

    pub fn counts_by_kind(&self) -> BTreeMap<DriftKind, usize> {
        let mut counts = BTreeMap::new();
        for drift in &self.drifts {
            *counts.entry(drift.kind).or_insert(0) += 1;
        }
        counts
    }
}

pub fn compare_paths(
    baseline: &Path,
    candidate: &Path,
    options: CompareOptions,
) -> Result<DriftReport> {
    let baseline_meta = fs::metadata(baseline)
        .with_context(|| format!("could not read baseline path {}", baseline.display()))?;
    let candidate_meta = fs::metadata(candidate)
        .with_context(|| format!("could not read candidate path {}", candidate.display()))?;

    let mut drifts = match (baseline_meta.is_file(), candidate_meta.is_file()) {
        (true, true) => compare_file_pair(baseline, candidate, &options)?,
        (false, false) if baseline_meta.is_dir() && candidate_meta.is_dir() => {
            compare_directory_pair(baseline, candidate, &options)?
        }
        _ => bail!(
            "baseline and candidate must both be files or both be directories: {} vs {}",
            baseline.display(),
            candidate.display()
        ),
    };

    drifts.sort_by(|left, right| {
        left.file
            .cmp(&right.file)
            .then(left.path.cmp(&right.path))
            .then(left.kind.cmp(&right.kind))
    });

    Ok(DriftReport {
        baseline: baseline.display().to_string(),
        candidate: candidate.display().to_string(),
        drift_count: drifts.len(),
        drifts,
    })
}

fn compare_file_pair(
    baseline: &Path,
    candidate: &Path,
    options: &CompareOptions,
) -> Result<Vec<Drift>> {
    let baseline_value = load::load_config_file(baseline)?;
    let candidate_value = load::load_config_file(candidate)?;
    let file = file_pair_label(baseline, candidate);

    let mut drifts = Vec::new();
    diff_values(
        &file,
        &[],
        &baseline_value,
        &candidate_value,
        options,
        &mut drifts,
    );
    Ok(drifts)
}

fn compare_directory_pair(
    baseline: &Path,
    candidate: &Path,
    options: &CompareOptions,
) -> Result<Vec<Drift>> {
    let baseline_docs = load::load_config_dir(baseline)?;
    let candidate_docs = load::load_config_dir(candidate)?;
    let files = baseline_docs
        .keys()
        .chain(candidate_docs.keys())
        .cloned()
        .collect::<BTreeSet<_>>();

    let mut drifts = Vec::new();
    for file in files {
        match (baseline_docs.get(&file), candidate_docs.get(&file)) {
            (Some(baseline_value), Some(candidate_value)) => diff_values(
                &file,
                &[],
                baseline_value,
                candidate_value,
                options,
                &mut drifts,
            ),
            (Some(value), None) => push_drift(
                DriftInput {
                    kind: DriftKind::FileRemoved,
                    file: &file,
                    path: &[],
                    baseline: Some(value),
                    candidate: None,
                },
                options,
                &mut drifts,
            ),
            (None, Some(value)) => push_drift(
                DriftInput {
                    kind: DriftKind::FileAdded,
                    file: &file,
                    path: &[],
                    baseline: None,
                    candidate: Some(value),
                },
                options,
                &mut drifts,
            ),
            (None, None) => return Err(anyhow!("unreachable file union state for {file}")),
        }
    }

    Ok(drifts)
}

fn diff_values(
    file: &str,
    path: &[PathPart],
    baseline: &Value,
    candidate: &Value,
    options: &CompareOptions,
    drifts: &mut Vec<Drift>,
) {
    match (baseline, candidate) {
        (Value::Object(baseline_map), Value::Object(candidate_map)) => {
            let keys = baseline_map
                .keys()
                .chain(candidate_map.keys())
                .cloned()
                .collect::<BTreeSet<_>>();

            for key in keys {
                let mut next_path = path.to_vec();
                next_path.push(PathPart::Key(key.clone()));
                match (baseline_map.get(&key), candidate_map.get(&key)) {
                    (Some(left), Some(right)) => {
                        diff_values(file, &next_path, left, right, options, drifts);
                    }
                    (Some(left), None) => push_drift(
                        DriftInput {
                            kind: DriftKind::Removed,
                            file,
                            path: &next_path,
                            baseline: Some(left),
                            candidate: None,
                        },
                        options,
                        drifts,
                    ),
                    (None, Some(right)) => push_drift(
                        DriftInput {
                            kind: DriftKind::Added,
                            file,
                            path: &next_path,
                            baseline: None,
                            candidate: Some(right),
                        },
                        options,
                        drifts,
                    ),
                    (None, None) => {}
                }
            }
        }
        (Value::Array(baseline_array), Value::Array(candidate_array)) => {
            let len = baseline_array.len().max(candidate_array.len());
            for index in 0..len {
                let mut next_path = path.to_vec();
                next_path.push(PathPart::Index(index));
                match (baseline_array.get(index), candidate_array.get(index)) {
                    (Some(left), Some(right)) => {
                        diff_values(file, &next_path, left, right, options, drifts);
                    }
                    (Some(left), None) => push_drift(
                        DriftInput {
                            kind: DriftKind::Removed,
                            file,
                            path: &next_path,
                            baseline: Some(left),
                            candidate: None,
                        },
                        options,
                        drifts,
                    ),
                    (None, Some(right)) => push_drift(
                        DriftInput {
                            kind: DriftKind::Added,
                            file,
                            path: &next_path,
                            baseline: None,
                            candidate: Some(right),
                        },
                        options,
                        drifts,
                    ),
                    (None, None) => {}
                }
            }
        }
        _ if baseline == candidate => {}
        _ => push_drift(
            DriftInput {
                kind: changed_kind(baseline, candidate),
                file,
                path,
                baseline: Some(baseline),
                candidate: Some(candidate),
            },
            options,
            drifts,
        ),
    }
}

struct DriftInput<'a> {
    kind: DriftKind,
    file: &'a str,
    path: &'a [PathPart],
    baseline: Option<&'a Value>,
    candidate: Option<&'a Value>,
}

fn push_drift(input: DriftInput<'_>, options: &CompareOptions, drifts: &mut Vec<Drift>) {
    let path = display_path(input.path);
    let location = display_location(input.file, &path);
    if options.ignore.is_match(input.file, &path, &location) {
        return;
    }

    drifts.push(Drift {
        kind: input.kind,
        file: input.file.to_string(),
        path,
        location,
        baseline: input
            .baseline
            .map(|value| redact::maybe_redact(&display_path(input.path), value, options.redact)),
        candidate: input
            .candidate
            .map(|value| redact::maybe_redact(&display_path(input.path), value, options.redact)),
    });
}

fn changed_kind(baseline: &Value, candidate: &Value) -> DriftKind {
    if value_type(baseline) == value_type(candidate) {
        DriftKind::Changed
    } else {
        DriftKind::TypeChanged
    }
}

fn value_type(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "bool",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

fn file_pair_label(baseline: &Path, candidate: &Path) -> String {
    let baseline_name = baseline.file_name().and_then(|name| name.to_str());
    let candidate_name = candidate.file_name().and_then(|name| name.to_str());

    match (baseline_name, candidate_name) {
        (Some(left), Some(right)) if left == right => left.to_string(),
        (Some(left), Some(right)) => format!("{left} -> {right}"),
        _ => format!("{} -> {}", baseline.display(), candidate.display()),
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
enum PathPart {
    Key(String),
    Index(usize),
}

fn display_location(file: &str, path: &str) -> String {
    if path.is_empty() {
        file.to_string()
    } else {
        format!("{file}:{path}")
    }
}

fn display_path(parts: &[PathPart]) -> String {
    let mut output = String::new();

    for part in parts {
        match part {
            PathPart::Key(key) => {
                if !output.is_empty() {
                    output.push('.');
                }
                output.push_str(&display_key(key));
            }
            PathPart::Index(index) => {
                output.push('[');
                output.push_str(&index.to_string());
                output.push(']');
            }
        }
    }

    output
}

fn display_key(key: &str) -> String {
    if key
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
    {
        key.to_string()
    } else {
        format!(
            "[{}]",
            serde_json::to_string(key).unwrap_or_else(|_| "\"?\"".to_string())
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn changed_scalar_is_reported_with_path() {
        let mut drifts = Vec::new();
        diff_values(
            "app.toml",
            &[],
            &json!({"database": {"pool": 8}}),
            &json!({"database": {"pool": 16}}),
            &CompareOptions {
                redact: true,
                ignore: IgnoreMatcher::empty(),
            },
            &mut drifts,
        );

        assert_eq!(drifts.len(), 1);
        assert_eq!(drifts[0].kind, DriftKind::Changed);
        assert_eq!(drifts[0].location, "app.toml:database.pool");
    }

    #[test]
    fn ignores_nested_suffix_pattern() {
        let mut drifts = Vec::new();
        diff_values(
            "app.toml",
            &[],
            &json!({"database": {"pool": 8, "password": "old"}}),
            &json!({"database": {"pool": 16, "password": "new"}}),
            &CompareOptions {
                redact: true,
                ignore: IgnoreMatcher::from_patterns(&["database.password".to_string()]).unwrap(),
            },
            &mut drifts,
        );

        assert_eq!(drifts.len(), 1);
        assert_eq!(drifts[0].location, "app.toml:database.pool");
    }

    #[test]
    fn redacts_sensitive_values() {
        let mut drifts = Vec::new();
        diff_values(
            "app.toml",
            &[],
            &json!({"auth": {"token": "old-token"}}),
            &json!({"auth": {"token": "new-token"}}),
            &CompareOptions {
                redact: true,
                ignore: IgnoreMatcher::empty(),
            },
            &mut drifts,
        );

        assert_eq!(drifts[0].baseline, Some(json!("<redacted>")));
        assert_eq!(drifts[0].candidate, Some(json!("<redacted>")));
    }
}
