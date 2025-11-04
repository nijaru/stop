use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::Value;

#[test]
fn test_help_flag() {
    let mut cmd = Command::cargo_bin("stop").unwrap();
    cmd.arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Structured process and system monitoring",
        ));
}

#[test]
fn test_version_flag() {
    let mut cmd = Command::cargo_bin("stop").unwrap();
    cmd.arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("0.0.1"));
}

#[test]
fn test_json_output() {
    let mut cmd = Command::cargo_bin("stop").unwrap();
    let output = cmd.arg("--json").assert().success();

    let stdout = String::from_utf8(output.get_output().stdout.clone()).unwrap();
    let json: Value = serde_json::from_str(&stdout).expect("Valid JSON output");

    // Check structure
    assert!(json.get("timestamp").is_some());
    assert!(json.get("system").is_some());
    assert!(json.get("processes").is_some());

    // Check system metrics
    let system = &json["system"];
    assert!(system.get("cpu_usage").is_some());
    assert!(system.get("memory_total").is_some());
    assert!(system.get("memory_used").is_some());
    assert!(system.get("memory_percent").is_some());
}

#[test]
fn test_csv_output() {
    let mut cmd = Command::cargo_bin("stop").unwrap();
    cmd.arg("--csv")
        .arg("--top-n")
        .arg("2")
        .assert()
        .success()
        .stdout(predicate::str::contains("timestamp,cpu_usage"))
        .stdout(predicate::str::contains("pid,name"));
}

#[test]
fn test_filter_cpu() {
    let mut cmd = Command::cargo_bin("stop").unwrap();
    cmd.arg("--filter")
        .arg("cpu > 0")
        .arg("--json")
        .assert()
        .success();

    // Should succeed and return valid JSON
}

#[test]
fn test_filter_invalid_field() {
    let mut cmd = Command::cargo_bin("stop").unwrap();
    cmd.arg("--filter")
        .arg("invalid > 10")
        .arg("--json")
        .assert()
        .failure()
        .stdout(predicate::str::contains("FilterError"))
        .stdout(predicate::str::contains("Unknown field"));
}

#[test]
fn test_filter_invalid_operator() {
    let mut cmd = Command::cargo_bin("stop").unwrap();
    cmd.arg("--filter")
        .arg("cpu >> 10")
        .arg("--json")
        .assert()
        .failure()
        .stdout(predicate::str::contains("FilterError"));
}

#[test]
fn test_filter_type_mismatch() {
    let mut cmd = Command::cargo_bin("stop").unwrap();
    cmd.arg("--filter")
        .arg("name > 10")
        .arg("--json")
        .assert()
        .failure()
        .stdout(predicate::str::contains("Type mismatch"));
}

#[test]
fn test_sort_by_mem() {
    let mut cmd = Command::cargo_bin("stop").unwrap();
    let output = cmd
        .arg("--sort-by")
        .arg("mem")
        .arg("--json")
        .arg("--top-n")
        .arg("5")
        .assert()
        .success();

    let stdout = String::from_utf8(output.get_output().stdout.clone()).unwrap();
    let json: Value = serde_json::from_str(&stdout).expect("Valid JSON");

    let processes = json["processes"].as_array().unwrap();
    if processes.len() >= 2 {
        let first_mem = processes[0]["memory_percent"].as_f64().unwrap();
        let second_mem = processes[1]["memory_percent"].as_f64().unwrap();
        assert!(
            first_mem >= second_mem,
            "Memory should be sorted descending"
        );
    }
}

#[test]
fn test_sort_by_cpu() {
    let mut cmd = Command::cargo_bin("stop").unwrap();
    let output = cmd
        .arg("--sort-by")
        .arg("cpu")
        .arg("--json")
        .arg("--top-n")
        .arg("5")
        .assert()
        .success();

    let stdout = String::from_utf8(output.get_output().stdout.clone()).unwrap();
    let json: Value = serde_json::from_str(&stdout).expect("Valid JSON");

    let processes = json["processes"].as_array().unwrap();
    if processes.len() >= 2 {
        let first_cpu = processes[0]["cpu_percent"].as_f64().unwrap();
        let second_cpu = processes[1]["cpu_percent"].as_f64().unwrap();
        assert!(first_cpu >= second_cpu, "CPU should be sorted descending");
    }
}

#[test]
fn test_top_n_limit() {
    let mut cmd = Command::cargo_bin("stop").unwrap();
    let output = cmd.arg("--json").arg("--top-n").arg("3").assert().success();

    let stdout = String::from_utf8(output.get_output().stdout.clone()).unwrap();
    let json: Value = serde_json::from_str(&stdout).expect("Valid JSON");

    let processes = json["processes"].as_array().unwrap();
    assert!(
        processes.len() <= 3,
        "Should return at most 3 processes, got {}",
        processes.len()
    );
}

#[test]
fn test_combined_filter_sort_topn() {
    let mut cmd = Command::cargo_bin("stop").unwrap();
    let output = cmd
        .arg("--filter")
        .arg("cpu >= 0")
        .arg("--sort-by")
        .arg("mem")
        .arg("--top-n")
        .arg("5")
        .arg("--json")
        .assert()
        .success();

    let stdout = String::from_utf8(output.get_output().stdout.clone()).unwrap();
    let json: Value = serde_json::from_str(&stdout).expect("Valid JSON");

    let processes = json["processes"].as_array().unwrap();
    assert!(processes.len() <= 5);
}

#[test]
fn test_human_readable_output() {
    let mut cmd = Command::cargo_bin("stop").unwrap();
    cmd.arg("--top-n")
        .arg("3")
        .assert()
        .success()
        .stdout(predicate::str::contains("stop"))
        .stdout(predicate::str::contains("v0.0.1"))
        .stdout(predicate::str::contains("System:"))
        .stdout(predicate::str::contains("CPU:"))
        .stdout(predicate::str::contains("Memory:"));
}
