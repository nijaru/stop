//! Integration tests: CLI behavior, JSON contracts, and exit codes.
//!
//! Exit codes under contract:
//! 0 = success (at least one process returned)
//! 1 = operational error
//! 2 = no match
//! 3 = ambiguous inspect target

use assert_cmd::Command;
use serde_json::Value;

fn stop() -> Command {
    Command::cargo_bin("stop").unwrap()
}

/// Returns the PID of the oldest live process from a fresh listing.
///
/// Tests that re-resolve a PID in a second invocation must not pick
/// short-lived processes (they die between runs on busy or quiet
/// machines alike); the oldest process is stable at test timescales.
fn oldest_pid(v: &Value) -> u64 {
    let procs = v["processes"].as_array().unwrap();
    let oldest_start = procs
        .iter()
        .map(|p| p["start_time"].as_u64().unwrap())
        .min()
        .expect("at least one process");
    procs
        .iter()
        .find(|p| p["start_time"].as_u64().unwrap() == oldest_start)
        .unwrap()["pid"]
        .as_u64()
        .unwrap()
}

/// Returns the parsed JSON envelope from a `stop list --json` run.
fn list_json(extra_args: &[&str]) -> Value {
    let output = stop()
        .args(["list", "--json"])
        .args(extra_args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "list --json {extra_args:?} exited {:?}: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("valid JSON on stdout")
}

#[test]
fn help_lists_subcommands() {
    let cases: [&[&str]; 4] = [
        &["--help"],
        &["list", "--help"],
        &["inspect", "--help"],
        &["top", "--help"],
    ];
    for args in cases {
        let output = stop().args(args).assert().success();
        let stdout = output.get_output().stdout.clone();
        let text = String::from_utf8(stdout).unwrap();
        if args == ["--help"] {
            for sub in ["list", "inspect", "top"] {
                assert!(text.contains(sub), "help should mention '{sub}'");
            }
        }
    }
}

#[test]
fn list_json_envelope_contract() {
    let v = list_json(&[]);
    for key in [
        "collected_at",
        "total_processes",
        "matched",
        "returned",
        "truncated",
        "processes",
    ] {
        assert!(v.get(key).is_some(), "missing envelope key '{key}'");
    }
    assert!(v["total_processes"].as_u64().unwrap() > 0);
    assert_eq!(
        v["matched"].as_u64().unwrap(),
        v["processes"].as_array().unwrap().len() as u64
    );
}

#[test]
fn list_json_process_fields_present() {
    let v = list_json(&[]);
    let procs = v["processes"].as_array().unwrap();
    assert!(!procs.is_empty(), "expected live processes");

    for p in procs {
        // P0 identity fields must always be present; pid > 0, name non-empty.
        assert!(p["pid"].as_u64().unwrap() > 0);
        assert!(p["start_time"].is_u64(), "start_time missing or not u64");
        assert!(!p["name"].as_str().unwrap().is_empty());
        for key in [
            "ppid",
            "exe",
            "cmdline",
            "cwd",
            "state",
            "user",
            "uid",
            "cpu_percent",
            "rss_bytes",
            "virtual_bytes",
            "threads",
            "io_read_bytes",
            "io_written_bytes",
        ] {
            assert!(p.get(key).is_some(), "process record missing '{key}'");
        }
    }

    // At least one process should carry a resolvable state from the vocabulary.
    let states: Vec<&str> = procs.iter().map(|p| p["state"].as_str().unwrap()).collect();
    assert!(
        states.iter().any(|s| ["run", "sleep", "idle"].contains(s)),
        "unexpected state vocabulary: {states:?}"
    );
}

#[test]
fn list_respects_limit_and_reports_truncation() {
    let full = list_json(&[]);
    let total_matched = full["matched"].as_u64().unwrap();

    let limited = list_json(&["--limit", "1"]);
    assert_eq!(limited["returned"].as_u64().unwrap(), 1);
    if total_matched > 1 {
        assert!(limited["truncated"].as_bool().unwrap());
        assert_eq!(limited["processes"].as_array().unwrap().len(), 1);
    }
}

#[test]
fn list_human_output_exits_zero() {
    stop().args(["list"]).assert().success();
}

#[test]
fn list_no_match_exits_two_with_empty_processes() {
    let output = stop()
        .args([
            "list",
            "--json",
            "--name",
            "zz-definitely-not-a-real-process-xyz",
        ])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    let v: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(v["returned"].as_u64().unwrap(), 0);
    assert_eq!(v["processes"].as_array().unwrap().len(), 0);
}

#[test]
fn inspect_by_pid_from_list_output() {
    let v = list_json(&[]);
    let pid = oldest_pid(&v);

    let output = stop()
        .args(["inspect", &pid.to_string(), "--json"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0));
    let got: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(got["pid"].as_u64().unwrap(), pid);
    let expected_start = v["processes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["pid"].as_u64().unwrap() == pid)
        .unwrap()["start_time"]
        .as_u64()
        .unwrap();
    assert_eq!(got["start_time"].as_u64().unwrap(), expected_start);
}

#[test]
fn inspect_unknown_pid_exits_two() {
    let output = stop()
        .args(["inspect", "4294967295", "--json"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    // Error payload goes to stderr as JSON when --json is set.
    let err: Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(err["code"].as_str().unwrap(), "not_found");
}

#[test]
fn inspect_unknown_name_exits_two() {
    let output = stop()
        .args(["inspect", "zz-no-such-process-name-xyz"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
}

#[test]
fn inspect_ambiguous_name_exits_three_with_candidates() {
    // Two live children share the name "sleep", making the target ambiguous.
    let mut children: Vec<std::process::Child> = (0..2)
        .map(|_| {
            std::process::Command::new("sleep")
                .arg("10")
                .spawn()
                .expect("spawn sleep")
        })
        .collect();

    let result = stop()
        .args(["inspect", "sleep", "--json"])
        .output()
        .unwrap();

    for child in &mut children {
        let _ = child.kill();
        let _ = child.wait();
    }

    assert_eq!(result.status.code(), Some(3));
    let err: Value = serde_json::from_slice(&result.stderr).unwrap();
    assert_eq!(err["code"].as_str().unwrap(), "ambiguous");
    let candidates = err["candidates"].as_array().unwrap();
    assert!(candidates.len() >= 2, "expected >=2 candidates");
    for c in candidates {
        assert!(c["pid"].as_u64().unwrap() > 0);
        assert!(c["start_time"].is_u64());
    }
}

#[test]
fn inspect_human_detail_includes_identity_fields() {
    let v = list_json(&[]);
    let pid = oldest_pid(&v);

    let output = stop().args(["inspect", &pid.to_string()]).output().unwrap();
    assert_eq!(output.status.code(), Some(0));
    let text = String::from_utf8(output.stdout).unwrap();
    for field in ["pid:", "start_time:", "state:", "rss:"] {
        assert!(text.contains(field), "human detail missing '{field}'");
    }
}

#[test]
fn top_json_has_system_metrics() {
    let output = stop().args(["top", "--json"]).output().unwrap();
    assert_eq!(output.status.code(), Some(0));
    let v: Value = serde_json::from_slice(&output.stdout).unwrap();
    for key in [
        "collected_at",
        "cpu_percent",
        "memory_total_bytes",
        "memory_used_bytes",
        "memory_used_percent",
        "processes",
    ] {
        assert!(v.get(key).is_some(), "top JSON missing '{key}'");
    }
    let limit = v["returned"].as_u64().unwrap();
    assert_eq!(limit, 10.min(v["total_processes"].as_u64().unwrap()));
    assert_eq!(
        v["truncated"].as_bool().unwrap(),
        v["matched"].as_u64().unwrap() > v["returned"].as_u64().unwrap()
    );
}

#[test]
fn top_human_output_shows_header() {
    let output = stop().args(["top", "--limit", "5"]).output().unwrap();
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("PID"), "table header missing");
}
