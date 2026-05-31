use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::Value;
use tempfile::TempDir;

#[test]
fn reports_structural_drift_and_redacts_secrets() {
    let temp = TempDir::new().unwrap();
    let baseline = temp.path().join("baseline.toml");
    let candidate = temp.path().join("candidate.toml");

    std::fs::write(
        &baseline,
        r#"
[database]
pool = 8
password = "old-secret"
"#,
    )
    .unwrap();
    std::fs::write(
        &candidate,
        r#"
[database]
pool = 16
password = "new-secret"
"#,
    )
    .unwrap();

    Command::cargo_bin("cfgdrift")
        .unwrap()
        .args([
            "diff",
            baseline.to_str().unwrap(),
            candidate.to_str().unwrap(),
        ])
        .assert()
        .code(1)
        .stdout(predicate::str::contains("database.pool"))
        .stdout(predicate::str::contains("<redacted>"))
        .stdout(predicate::str::contains("old-secret").not());
}

#[test]
fn emits_valid_json_output() {
    let temp = TempDir::new().unwrap();
    let baseline = temp.path().join("baseline.json");
    let candidate = temp.path().join("candidate.json");

    std::fs::write(&baseline, r#"{"feature":{"enabled":false}}"#).unwrap();
    std::fs::write(&candidate, r#"{"feature":{"enabled":true}}"#).unwrap();

    let output = Command::cargo_bin("cfgdrift")
        .unwrap()
        .args([
            "diff",
            baseline.to_str().unwrap(),
            candidate.to_str().unwrap(),
            "--format",
            "json",
            "--exit-zero",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let parsed: Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(parsed["drift_count"], 1);
    assert_eq!(
        parsed["drifts"][0]["location"],
        "baseline.json -> candidate.json:feature.enabled"
    );
}

#[test]
fn ignore_file_suppresses_matching_drift() {
    let temp = TempDir::new().unwrap();
    let baseline = temp.path().join("baseline");
    let candidate = temp.path().join("candidate");
    std::fs::create_dir_all(&baseline).unwrap();
    std::fs::create_dir_all(&candidate).unwrap();
    std::fs::write(baseline.join("app.yaml"), "database:\n  pool: 8\n").unwrap();
    std::fs::write(candidate.join("app.yaml"), "database:\n  pool: 16\n").unwrap();
    let ignore_file = temp.path().join(".cfgdriftignore");
    std::fs::write(&ignore_file, "database.pool\n").unwrap();

    Command::cargo_bin("cfgdrift")
        .unwrap()
        .args([
            "diff",
            baseline.to_str().unwrap(),
            candidate.to_str().unwrap(),
            "--ignore-file",
            ignore_file.to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("No config drift found."));
}
