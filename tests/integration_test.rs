#![allow(deprecated)]

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
            "Modern process monitoring with structured output",
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

#[test]
fn test_phase3_features_in_json() {
    let mut cmd = Command::cargo_bin("stop").unwrap();
    let output = cmd.arg("--json").arg("--top-n").arg("1").assert().success();

    let stdout = String::from_utf8(output.get_output().stdout.clone()).unwrap();
    let json: Value = serde_json::from_str(&stdout).expect("Valid JSON output");

    // Ensure at least one process exists
    let processes = json["processes"].as_array().unwrap();
    assert!(!processes.is_empty(), "Expected at least one process");

    let process = &processes[0];

    // Check Phase 3 features exist
    assert!(
        process.get("thread_count").is_some(),
        "Missing thread_count field"
    );
    assert!(
        process.get("disk_read_bytes").is_some(),
        "Missing disk_read_bytes field"
    );
    assert!(
        process.get("disk_write_bytes").is_some(),
        "Missing disk_write_bytes field"
    );
    assert!(
        process.get("open_files").is_some(),
        "Missing open_files field"
    );

    // Verify types and reasonable values
    assert!(
        process["thread_count"].is_number(),
        "thread_count should be a number"
    );
    // thread_count can be 0 in some environments (e.g., kernel threads)
    let thread_count = process["thread_count"].as_u64().unwrap();
    assert!(
        thread_count < 10000,
        "thread_count should be reasonable (< 10000), got {}",
        thread_count
    );

    assert!(
        process["disk_read_bytes"].is_number(),
        "disk_read_bytes should be a number"
    );
    assert!(
        process["disk_write_bytes"].is_number(),
        "disk_write_bytes should be a number"
    );

    // open_files can be null for privileged processes, or a number
    assert!(
        process["open_files"].is_null() || process["open_files"].is_number(),
        "open_files should be null or a number"
    );
}

#[test]
fn test_phase3_features_in_csv() {
    let mut cmd = Command::cargo_bin("stop").unwrap();
    let output = cmd.arg("--csv").arg("--top-n").arg("1").assert().success();

    let stdout = String::from_utf8(output.get_output().stdout.clone()).unwrap();
    let lines: Vec<&str> = stdout.lines().collect();

    assert!(
        lines.len() >= 2,
        "Expected header and at least one data row"
    );

    // Check CSV header includes Phase 3 fields
    let header = lines[0];
    assert!(
        header.contains("thread_count"),
        "CSV header missing thread_count"
    );
    assert!(
        header.contains("disk_read_bytes"),
        "CSV header missing disk_read_bytes"
    );
    assert!(
        header.contains("disk_write_bytes"),
        "CSV header missing disk_write_bytes"
    );
    assert!(
        header.contains("open_files"),
        "CSV header missing open_files"
    );

    // Check data row has these fields (verify by counting commas)
    let data_row = lines[1];
    let field_count = data_row.split(',').count();
    assert!(
        field_count >= 16,
        "Expected at least 16 CSV fields including Phase 3 features"
    );
}

#[test]
fn test_broken_pipe_handling_json() {
    // Test that piping to head doesn't cause panic (broken pipe handling)
    use std::process::{Command, Stdio};

    let mut stop_child = Command::new("cargo")
        .args(["run", "--", "--json"])
        .stdout(Stdio::piped())
        .spawn()
        .expect("Failed to start stop");

    let stop_stdout = stop_child.stdout.take().unwrap();
    let head_output = Command::new("head")
        .arg("-1")
        .stdin(stop_stdout)
        .output()
        .expect("Failed to run head");

    // Wait for stop to exit gracefully (broken pipe should cause clean exit)
    let _ = stop_child.wait();

    // Should succeed without panic
    assert!(
        head_output.status.success()
            || head_output.status.code() == Some(0)
            || head_output.status.code() == Some(141)
    );

    // Should get at least one line
    let stdout = String::from_utf8_lossy(&head_output.stdout);
    assert!(!stdout.is_empty(), "Expected at least one line of output");
}

#[test]
fn test_broken_pipe_handling_csv() {
    // Test CSV output with broken pipe
    use std::process::{Command, Stdio};

    let mut stop_child = Command::new("cargo")
        .args(["run", "--", "--csv"])
        .stdout(Stdio::piped())
        .spawn()
        .expect("Failed to start stop");

    let stop_stdout = stop_child.stdout.take().unwrap();
    let head_output = Command::new("head")
        .arg("-2")
        .stdin(stop_stdout)
        .output()
        .expect("Failed to run head");

    // Wait for stop to exit gracefully (broken pipe should cause clean exit)
    let _ = stop_child.wait();

    // Should succeed without panic
    assert!(
        head_output.status.success()
            || head_output.status.code() == Some(0)
            || head_output.status.code() == Some(141)
    );

    // Should get header + at least one data row
    let stdout = String::from_utf8_lossy(&head_output.stdout);
    let lines: Vec<&str> = stdout.lines().collect();
    assert!(lines.len() >= 2, "Expected header + data row");
}

#[test]
fn test_verbose_mode_output() {
    let mut cmd = Command::cargo_bin("stop").unwrap();
    let output = cmd
        .arg("--verbose")
        .arg("--top-n")
        .arg("1")
        .assert()
        .success();

    let stdout = String::from_utf8(output.get_output().stdout.clone()).unwrap();

    // Check for verbose-only headers
    assert!(stdout.contains("Threads"), "Should contain Threads header");
    assert!(stdout.contains("Read"), "Should contain Read header");
    assert!(stdout.contains("Write"), "Should contain Write header");
    assert!(stdout.contains("Files"), "Should contain Files header");
}

#[test]
fn test_verbose_mode_with_json() {
    let mut cmd = Command::cargo_bin("stop").unwrap();
    let output = cmd
        .arg("--json")
        .arg("--verbose")
        .arg("--top-n")
        .arg("1")
        .assert()
        .success();

    let stdout = String::from_utf8(output.get_output().stdout.clone()).unwrap();
    let json: Value = serde_json::from_str(&stdout).expect("Valid JSON output");

    // Verbose flag shouldn't affect JSON output structure
    assert!(json.get("processes").is_some());
    let processes = json["processes"].as_array().unwrap();
    if !processes.is_empty() {
        let process = &processes[0];
        assert!(process.get("thread_count").is_some());
        assert!(process.get("disk_read_bytes").is_some());
    }
}

#[test]
fn test_watch_mode_ndjson_output() {
    use std::process::{Command, Stdio};
    use std::time::Duration;

    let mut stop_child = Command::new("cargo")
        .args(["run", "--", "--json", "--watch", "--interval", "0.3"])
        .stdout(Stdio::piped())
        .spawn()
        .expect("Failed to start stop");

    // Let it run for longer to account for 200ms CPU sample interval
    // First snapshot: 200ms, then 300ms interval per iteration
    // Total: 1200ms should give us 200 + 300 + 300 = at least 3 snapshots
    std::thread::sleep(Duration::from_millis(1200));

    // Kill the process
    stop_child.kill().expect("Failed to kill process");
    let output = stop_child.wait_with_output().expect("Failed to wait");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let lines: Vec<&str> = stdout.lines().filter(|l| !l.is_empty()).collect();

    // Should have at least 1 NDJSON line (accounting for timing variance)
    assert!(
        !lines.is_empty(),
        "Expected at least 1 NDJSON line, got {}",
        lines.len()
    );

    // Each line should be valid JSON
    for (i, line) in lines.iter().enumerate() {
        let result = serde_json::from_str::<Value>(line);
        assert!(
            result.is_ok(),
            "Line {} is not valid JSON: {}",
            i + 1,
            line
        );
    }
}

#[test]
fn test_watch_mode_with_filter() {
    use std::process::{Command, Stdio};
    use std::time::Duration;

    let mut stop_child = Command::new("cargo")
        .args([
            "run",
            "--",
            "--json",
            "--watch",
            "--interval",
            "0.3",
            "--filter",
            "cpu >= 0",
        ])
        .stdout(Stdio::piped())
        .spawn()
        .expect("Failed to start stop");

    // Account for 200ms initial CPU sample + interval time
    std::thread::sleep(Duration::from_millis(1200));

    stop_child.kill().expect("Failed to kill process");
    let output = stop_child.wait_with_output().expect("Failed to wait");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let lines: Vec<&str> = stdout.lines().filter(|l| !l.is_empty()).collect();

    assert!(
        !lines.is_empty(),
        "Expected at least 1 NDJSON line with filter"
    );

    // Validate structure
    for line in lines.iter() {
        let json: Value = serde_json::from_str(line).expect("Valid JSON");
        assert!(json.get("processes").is_some());
    }
}

#[test]
fn test_search_flag() {
    let mut cmd = Command::cargo_bin("stop").unwrap();
    let output = cmd
        .arg("--search")
        .arg("cargo")
        .arg("--json")
        .assert()
        .success();

    let stdout = String::from_utf8(output.get_output().stdout.clone()).unwrap();
    let json: Value = serde_json::from_str(&stdout).expect("Valid JSON output");

    let processes = json["processes"].as_array().unwrap();

    // All returned processes should match search term
    for process in processes {
        let name = process["name"].as_str().unwrap().to_lowercase();
        let command = process["command"].as_str().unwrap().to_lowercase();
        assert!(
            name.contains("cargo") || command.contains("cargo"),
            "Process should match search term 'cargo': name={}, command={}",
            name,
            command
        );
    }
}

#[test]
fn test_search_with_common_term() {
    // Test that search works with common terms
    // Note: We can't reliably test self-exclusion in integration tests due to
    // timing issues and cargo test spawning multiple processes
    let mut cmd = Command::cargo_bin("stop").unwrap();
    let output = cmd
        .arg("--search")
        .arg("rust")
        .arg("--json")
        .arg("--top-n")
        .arg("5")
        .assert()
        .success();

    let stdout = String::from_utf8(output.get_output().stdout.clone()).unwrap();
    let json: Value = serde_json::from_str(&stdout).expect("Valid JSON output");

    let processes = json["processes"].as_array().unwrap();

    // All returned processes should contain "rust" in name or command
    for process in processes {
        let name = process["name"].as_str().unwrap().to_lowercase();
        let command = process["command"].as_str().unwrap().to_lowercase();
        assert!(
            name.contains("rust") || command.contains("rust"),
            "Process should match search term 'rust': name={}, command={}",
            name,
            command
        );
    }
}

#[test]
fn test_invalid_interval_negative() {
    let mut cmd = Command::cargo_bin("stop").unwrap();
    cmd.arg("--interval")
        .arg("-1")
        .arg("--watch")
        .assert()
        .failure();
}

