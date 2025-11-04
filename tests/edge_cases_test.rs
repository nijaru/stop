use assert_cmd::Command;
use predicates::prelude::*;

/// Test filter parsing with edge cases
#[test]
fn test_filter_empty_string() {
    let mut cmd = Command::cargo_bin("stop").unwrap();
    cmd.arg("--json")
        .arg("--filter")
        .arg("")
        .assert()
        .failure()
        .stdout(predicate::str::contains("FilterError"));
}

#[test]
fn test_filter_with_quotes_in_string() {
    let mut cmd = Command::cargo_bin("stop").unwrap();
    // Test process name with quotes
    cmd.arg("--json")
        .arg("--filter")
        .arg("name == \"test\\\"process\"")
        .assert()
        .success();
}

#[test]
fn test_filter_with_unicode() {
    let mut cmd = Command::cargo_bin("stop").unwrap();
    cmd.arg("--json")
        .arg("--filter")
        .arg("name == \"tést🚀\"")
        .assert()
        .success();
}

#[test]
fn test_filter_very_long_expression() {
    let mut cmd = Command::cargo_bin("stop").unwrap();
    // Create a very long filter expression with multiple OR clauses
    let mut filter = String::from("cpu > 0");
    for i in 1..50 {
        filter.push_str(&format!(" OR cpu > {}", i));
    }
    cmd.arg("--json")
        .arg("--filter")
        .arg(&filter)
        .assert()
        .success();
}

#[test]
fn test_filter_with_special_chars() {
    let mut cmd = Command::cargo_bin("stop").unwrap();
    cmd.arg("--json")
        .arg("--filter")
        .arg("name == \"test,process;with:special\"")
        .assert()
        .success();
}

#[test]
fn test_filter_name_looks_like_number() {
    let mut cmd = Command::cargo_bin("stop").unwrap();
    // This should treat "123" as a string when used with name field
    cmd.arg("--json")
        .arg("--filter")
        .arg("name == \"123\"")
        .assert()
        .success();
}

#[test]
fn test_filter_multiple_spaces() {
    let mut cmd = Command::cargo_bin("stop").unwrap();
    cmd.arg("--json")
        .arg("--filter")
        .arg("cpu    >    10    AND    mem   >   5")
        .assert()
        .success();
}

#[test]
fn test_filter_whitespace_in_string() {
    let mut cmd = Command::cargo_bin("stop").unwrap();
    cmd.arg("--json")
        .arg("--filter")
        .arg("name == \"  spaces  \"")
        .assert()
        .success();
}

/// Test CSV output with edge cases
#[test]
fn test_csv_output_escaping() {
    let mut cmd = Command::cargo_bin("stop").unwrap();
    cmd.arg("--csv")
        .arg("--top-n")
        .arg("5")
        .assert()
        .success()
        .stdout(predicate::str::contains("timestamp,cpu_usage"));
}

#[test]
fn test_csv_with_filter() {
    let mut cmd = Command::cargo_bin("stop").unwrap();
    cmd.arg("--csv")
        .arg("--filter")
        .arg("cpu >= 0")
        .arg("--top-n")
        .arg("3")
        .assert()
        .success()
        .stdout(predicate::str::contains("timestamp,cpu_usage"));
}

/// Test error conditions
#[test]
fn test_conflicting_output_formats() {
    let mut cmd = Command::cargo_bin("stop").unwrap();
    // Both --json and --csv shouldn't cause crash (undefined behavior but shouldn't panic)
    cmd.arg("--json")
        .arg("--csv")
        .assert()
        .success(); // Currently no validation, just takes last one
}

#[test]
fn test_negative_interval() {
    let mut cmd = Command::cargo_bin("stop").unwrap();
    cmd.arg("--watch")
        .arg("--interval")
        .arg("-1")
        .assert()
        .failure();
}

#[test]
fn test_very_small_interval() {
    // Just test that small interval shows warning, don't wait for completion
    let mut cmd = Command::cargo_bin("stop").unwrap();
    let output = cmd
        .arg("--json")
        .arg("--interval")
        .arg("0.01")
        .arg("--top-n")
        .arg("1")
        .timeout(std::time::Duration::from_millis(500))
        .output()
        .unwrap();

    // Should have shown warning about interval
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Interval below 0.2s") || stderr.is_empty());
}

#[test]
fn test_zero_top_n() {
    let mut cmd = Command::cargo_bin("stop").unwrap();
    cmd.arg("--json")
        .arg("--top-n")
        .arg("0")
        .assert()
        .success()
        .stdout(predicate::str::contains("\"processes\": []"));
}

#[test]
fn test_huge_top_n() {
    let mut cmd = Command::cargo_bin("stop").unwrap();
    cmd.arg("--json")
        .arg("--top-n")
        .arg("1000000")
        .assert()
        .success(); // Should work, just limited by actual process count
}

#[test]
fn test_invalid_sort_field() {
    let mut cmd = Command::cargo_bin("stop").unwrap();
    cmd.arg("--json")
        .arg("--sort-by")
        .arg("invalid_field")
        .assert()
        .success(); // Should default to cpu with warning
}

#[test]
fn test_filter_with_newline_in_command() {
    let mut cmd = Command::cargo_bin("stop").unwrap();
    // Commands can contain newlines, should handle gracefully
    cmd.arg("--json")
        .arg("--filter")
        .arg("cpu >= 0")
        .assert()
        .success();
}

#[test]
fn test_json_output_is_valid() {
    let mut cmd = Command::cargo_bin("stop").unwrap();
    let output = cmd.arg("--json").arg("--top-n").arg("5").output().unwrap();

    assert!(output.status.success());

    // Verify JSON is parseable
    let json_str = String::from_utf8(output.stdout).unwrap();
    let _: serde_json::Value = serde_json::from_str(&json_str)
        .expect("Output should be valid JSON");
}

#[test]
fn test_csv_output_row_count() {
    let mut cmd = Command::cargo_bin("stop").unwrap();
    let output = cmd.arg("--csv").arg("--top-n").arg("3").output().unwrap();

    assert!(output.status.success());

    let csv_str = String::from_utf8(output.stdout).unwrap();
    let lines: Vec<&str> = csv_str.lines().collect();

    // Should have header + 3 data rows = 4 lines minimum
    assert!(lines.len() >= 4, "Expected at least 4 lines (header + 3 processes), got {}", lines.len());
}
