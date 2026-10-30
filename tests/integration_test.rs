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
        .filter_map(|p| p["start_time"].as_u64())
        .min()
        .expect("at least one process");
    procs
        .iter()
        .find(|p| p["start_time"].as_u64() == Some(oldest_start))
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
    let cases: [&[&str]; 6] = [
        &["--help"],
        &["list", "--help"],
        &["inspect", "--help"],
        &["top", "--help"],
        &["tree", "--help"],
        &["sample", "--help"],
    ];
    for args in cases {
        let output = stop().args(args).assert().success();
        let stdout = output.get_output().stdout.clone();
        let text = String::from_utf8(stdout).unwrap();
        if args == ["--help"] {
            for sub in ["list", "inspect", "top", "tree", "sample"] {
                assert!(text.contains(sub), "help should mention '{sub}'");
            }
        }
    }
}

#[test]
fn help_only_advertises_applicable_options() {
    for command in ["list", "inspect", "top", "tree", "sample"] {
        let result = stop().args([command, "--help"]).output().unwrap();
        assert!(result.status.success());
        let text = String::from_utf8(result.stdout).unwrap();
        assert_eq!(text.contains("--jsonl"), command == "sample", "{text}");
        assert_eq!(text.contains("--cpu"), command != "sample", "{text}");
        if command == "top" {
            assert!(text.contains("[possible values: cpu, mem]"), "{text}");
        }
    }
}

#[test]
fn usage_errors_keep_layout_and_escape_input() {
    let output = stop()
        .args(["list", "--bogus\n\x1b[31m\u{202e}"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    let text = String::from_utf8(output.stderr).unwrap();
    assert!(text.starts_with("error: unexpected argument"));
    assert!(
        text.contains("\n\nUsage: stop list [OPTIONS]\n\n"),
        "{text}"
    );
    assert!(text.contains("--bogus\\n\\u{1b}[31m\\u{202e}"), "{text}");
    assert!(!text.contains('\x1b') && !text.contains('\u{202e}'));
}

#[test]
fn sample_count_errors_identify_the_right_option() {
    for count in ["0", "1001", "bad"] {
        let output = stop().args(["sample", "--count", count]).output().unwrap();
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        let text = String::from_utf8(output.stderr).unwrap();
        assert!(text.contains("count must be between 1 and 1000"), "{text}");
    }
}

#[test]
fn sample_globals_work_before_or_after_the_command() {
    for args in [
        vec!["--jsonl", "sample", "--count", "1"],
        vec!["sample", "--jsonl", "--count", "1"],
    ] {
        let result = stop()
            .args(args)
            .args(["--pid", &std::process::id().to_string()])
            .output()
            .unwrap();
        assert!(result.status.success(), "{result:?}");
        let point: Value = serde_json::from_slice(&result.stdout).unwrap();
        assert_eq!(point["index"], 1);
        assert_eq!(point["processes"][0]["pid"], std::process::id());
    }
    for args in [vec!["--cpu", "sample"], vec!["sample", "--cpu"]] {
        let result = stop().args(args).output().unwrap();
        assert_eq!(result.status.code(), Some(1));
        assert!(
            String::from_utf8(result.stderr)
                .unwrap()
                .contains("remove --cpu")
        );
    }
}

#[test]
fn conflicting_formats_are_rejected_across_command_boundaries() {
    for document in ["--json", "--pretty"] {
        for args in [
            vec![document, "sample", "--jsonl"],
            vec!["--jsonl", "sample", document],
            vec!["sample", document, "--jsonl"],
            vec![document, "--jsonl", "sample"],
        ] {
            let output = stop().args(&args).args(["--count", "1"]).output().unwrap();
            assert_eq!(output.status.code(), Some(1), "{args:?}: {output:?}");
            assert!(output.stdout.is_empty());
            let error: Value = serde_json::from_slice(&output.stderr).unwrap();
            assert_eq!(error["code"], "usage");
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
        for field in ["start_time", "rss_bytes", "virtual_bytes"] {
            assert!(
                p[field].is_u64() || (p[field].is_null() && p["unavailable"][field] == "unknown"),
                "unexplained missing {field}"
            );
        }
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
    assert_eq!(got["process"]["pid"].as_u64().unwrap(), pid);
    if got["process"]["birth_id"].is_null() {
        // Protected system processes may expose basic facts but no native generation.
        assert_eq!(got["process"]["unavailable"]["birth_id"], "unknown");
    } else {
        assert_eq!(
            got["process"]["birth_id"],
            v["processes"]
                .as_array()
                .unwrap()
                .iter()
                .find(|p| p["pid"] == pid)
                .unwrap()["birth_id"]
        );
    }
    assert!(got["process"]["cpu_percent"].is_null());
    let expected_start = v["processes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["pid"].as_u64().unwrap() == pid)
        .unwrap()["start_time"]
        .as_u64()
        .unwrap();
    assert_eq!(
        got["process"]["start_time"].as_u64().unwrap(),
        expected_start
    );
}

#[test]
fn inspect_unknown_pid_exits_two() {
    let output = stop()
        .args(["inspect", "4294967295", "--json"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stderr.is_empty());
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(report["process"].is_null());
    // A requested but absent PID is not an observed inaccessible candidate.
    assert_eq!(report["inaccessible_processes"], 0);
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
fn ambiguous_names_report_candidates_in_both_formats() {
    // Allow for slow CLI launches on loaded CI hosts, rather than letting the
    // fixture expire before name resolution. Always reap it before assertions.
    let mut children: Vec<std::process::Child> = (0..2)
        .map(|_| {
            std::process::Command::new("sleep")
                .arg("60")
                .spawn()
                .unwrap()
        })
        .collect();
    let pids: Vec<_> = children.iter().map(|child| child.id()).collect();
    let results: Vec<_> = ["inspect", "tree"]
        .into_iter()
        .map(|command| {
            let machine = stop().args([command, "sleep", "--json"]).output().unwrap();
            let human = stop()
                .args([command, "sleep"])
                .env("TZ", "UTC")
                .output()
                .unwrap();
            (machine, human)
        })
        .collect();
    for child in &mut children {
        let _ = child.kill();
        let _ = child.wait();
    }
    for (machine, human) in results {
        assert_eq!(machine.status.code(), Some(3));
        assert_eq!(human.status.code(), Some(3));
        assert!(machine.stdout.is_empty() && human.stdout.is_empty());
        let err: Value = serde_json::from_slice(&machine.stderr).unwrap();
        assert_eq!(err["code"], "ambiguous");
        let candidates = err["candidates"].as_array().unwrap();
        let text = String::from_utf8(human.stderr).unwrap();
        for header in ["PID", "USER", "NAME", "STARTED", "PATH"] {
            assert!(text.contains(header), "{text}");
        }
        for pid in &pids {
            let candidate = candidates
                .iter()
                .find(|c| c["pid"] == *pid)
                .expect("live child is a candidate");
            let started =
                chrono::DateTime::from_timestamp(candidate["start_time"].as_i64().unwrap(), 0)
                    .unwrap();
            assert!(text.contains(&pid.to_string()));
            assert!(
                text.contains(&started.format("%Y-%m-%d %H:%M:%S +00:00").to_string()),
                "{text}"
            );
        }
    }
}

#[test]
fn inspect_human_detail_includes_identity_fields() {
    let v = list_json(&[]);
    let pid = oldest_pid(&v);

    let output = stop().args(["inspect", &pid.to_string()]).output().unwrap();
    assert_eq!(output.status.code(), Some(0));
    let text = String::from_utf8(output.stdout).unwrap();
    for field in ["PID", "Started", "State", "RSS", "Executable", "Arguments"] {
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
    let returned = v["returned"].as_u64().unwrap();
    // PIDs may disappear between inventory and enrichment; limit collected matches.
    assert_eq!(returned, 10.min(v["matched"].as_u64().unwrap()));
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

/// Collects every pid in the tree forest and asserts the spanning-forest
/// invariant: count matches total_processes and no pid repeats.
fn assert_spanning_forest(v: &Value) {
    let roots = v["roots"].as_array().expect("roots array");
    let mut pids: Vec<u64> = Vec::new();
    fn walk(nodes: &[Value], pids: &mut Vec<u64>) {
        for n in nodes {
            pids.push(n["process"]["pid"].as_u64().expect("node pid"));
            let children = n["children"].as_array().expect("children array");
            walk(children, pids);
        }
    }
    walk(roots, &mut pids);

    let total = v["total_processes"].as_u64().expect("total_processes");
    assert_eq!(
        pids.len() as u64,
        total,
        "every process appears exactly once"
    );
    let unique = pids.iter().collect::<std::collections::HashSet<_>>();
    assert_eq!(
        unique.len(),
        pids.len(),
        "no pid repeated across the forest"
    );
}

#[test]
fn tree_json_is_spanning_forest() {
    let output = stop().args(["tree", "--json"]).output().unwrap();
    assert_eq!(output.status.code(), Some(0));
    let v: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(!v["roots"].as_array().unwrap().is_empty());
    assert_spanning_forest(&v);
}

#[test]
fn tree_json_subtree_roots_at_pid() {
    let pid = oldest_pid(&list_json(&[]));
    let output = stop()
        .args(["tree", "--json", &pid.to_string()])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0));
    let v: Value = serde_json::from_slice(&output.stdout).unwrap();
    let roots = v["roots"].as_array().unwrap();
    assert_eq!(roots.len(), 1, "subtree view has a single root");
    assert_eq!(roots[0]["process"]["pid"].as_u64().unwrap(), pid);
}

#[test]
fn tree_no_match_exits_two() {
    let output = stop()
        .args(["tree", "--json", "definitely-not-a-process-xyz"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
}

#[test]
fn tree_human_output_shows_hierarchy() {
    let pid = oldest_pid(&list_json(&[]));
    let output = stop().args(["tree", &pid.to_string()]).output().unwrap();
    assert_eq!(output.status.code(), Some(0));
    let text = String::from_utf8(output.stdout).unwrap();
    let first_line = text.lines().next().expect("at least one line");
    assert!(
        first_line.starts_with(&pid.to_string()),
        "first line must be the target root, got {first_line:?}"
    );
}

#[test]
fn inspect_port_reports_tcp_owner() {
    let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let port = listener.local_addr().unwrap().port().to_string();

    let output = stop()
        .args(["inspect", "--port", &port, "--json"])
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(0));
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["port"].as_u64(), Some(port.parse().unwrap()));
    assert!(matches!(
        report["visibility"].as_str(),
        Some("complete") | Some("partial")
    ));
    let owners = report["owners"].as_array().unwrap();
    let sockets: Vec<&Value> = owners
        .iter()
        .flat_map(|owner| owner["sockets"].as_array().unwrap())
        .collect();
    assert!(
        sockets.iter().any(|socket| {
            socket["protocol"] == "tcp"
                && socket["local_port"].as_u64() == Some(port.parse().unwrap())
                && socket["state"] == "listen"
        }),
        "listener was not attributed: {report}"
    );
}

#[test]
fn inspect_port_reports_udp_owner() {
    let socket = std::net::UdpSocket::bind(("127.0.0.1", 0)).unwrap();
    let port = socket.local_addr().unwrap().port().to_string();

    let output = stop()
        .args(["inspect", "--port", &port, "--json"])
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(0));
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    let owners = report["owners"].as_array().unwrap();
    assert!(
        owners
            .iter()
            .flat_map(|owner| owner["sockets"].as_array().unwrap())
            .any(|socket| {
                socket["protocol"] == "udp"
                    && socket["local_port"].as_u64() == Some(port.parse().unwrap())
                    && socket["state"] == "bound"
            })
    );
}

#[test]
fn inspect_port_no_match_exits_two_with_report() {
    let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let port = listener.local_addr().unwrap().port().to_string();
    drop(listener);

    let output = stop()
        .args(["inspect", "--port", &port, "--json"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["port"].as_u64(), Some(port.parse().unwrap()));
    assert!(report["owners"].as_array().unwrap().is_empty());
}

#[test]
fn inspect_port_zero_is_usage_error() {
    let output = stop()
        .args(["inspect", "--port", "0", "--json"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
}

#[test]
fn sample_json_has_bounded_filtered_cpu_series() {
    let pid = std::process::id().to_string();
    let output = stop()
        .args([
            "sample",
            "--pid",
            &pid,
            "--count",
            "2",
            "--interval",
            "200ms",
            "--json",
        ])
        .output()
        .unwrap();
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["schema_version"], 1);
    assert!(report["started_at"].is_string());
    assert_eq!(report["interval_ms"], 200);
    assert_eq!(report["count"], 2);
    assert_eq!(report["requested_count"], 2);
    assert_eq!(report["completed"], true);
    let samples = report["samples"].as_array().unwrap();
    assert_eq!(samples.len(), 2);
    for sample in samples {
        assert!(sample["collected_at"].is_string());
        assert!(sample["cpu_interval_ms"].as_u64().unwrap() >= 200);
        assert!(sample["cpu_percent"].is_number());
        assert_eq!(sample["processes"].as_array().unwrap().len(), 1);
        assert_eq!(sample["processes"][0]["pid"], std::process::id());
        assert!(sample["processes"][0]["cpu_percent"].is_number());
        assert_eq!(sample["target_status"], "observed");
    }
    assert_eq!(
        samples[0]["processes"][0]["birth_id"],
        samples[1]["processes"][0]["birth_id"]
    );
}

#[test]
fn sample_rate_normalizes_to_interval() {
    let output = stop()
        .args(["sample", "--count", "1", "--rate", "2", "--json"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0));
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["interval_ms"], 500);
}

#[test]
fn invalid_requests_are_structured_usage_errors() {
    let cases: &[&[&str]] = &[
        &["sample", "--interval", "1s", "--rate", "2"],
        &["sample", "--interval", "1ms"],
        &["sample", "--rate", "1000"],
        &["sample", "--cpu"],
        &["list", "--fast"],
        &["list", "--limit", "0"],
        &["list", "--cpu=1ms"],
        &["top", "--sort", "pid"],
        &["inspect", "sleep", "--name", "sleep"],
        &["list", "--jsonl"],
    ];
    for args in cases {
        let output = stop().arg("--json").args(*args).output().unwrap();
        assert_eq!(output.status.code(), Some(1), "{args:?}");
        assert!(output.stdout.is_empty());
        let error: Value = serde_json::from_slice(&output.stderr).unwrap();
        assert_eq!(error["code"], "usage");
    }
}

#[test]
fn human_tables_only_show_requested_cpu_and_sampling_is_bounded() {
    let plain = stop().args(["list", "--limit", "1"]).output().unwrap();
    let text = String::from_utf8(plain.stdout).unwrap();
    assert!(text.contains("RSS") && !text.contains("CPU%"));
    let sample = stop()
        .args(["sample", "--count", "1", "--limit", "1"])
        .output()
        .unwrap();
    assert!(sample.status.success());
    let text = String::from_utf8(sample.stdout).unwrap();
    assert!(text.contains("Sample 1/1") && text.contains("CPU%"));
    assert!(text.contains("1 of"));
}

#[test]
fn inventory_is_pid_ordered_without_cpu_but_can_request_measurement() {
    let plain = list_json(&[]);
    let rows = plain["processes"].as_array().unwrap();
    assert!(
        rows.windows(2)
            .all(|pair| pair[0]["pid"].as_u64() < pair[1]["pid"].as_u64())
    );
    assert!(
        rows.iter()
            .all(|p| p["cpu_percent"].is_null()
                && p["unavailable"]["cpu_percent"] == "not_requested")
    );
    assert!(plain["cpu_interval_ms"].is_null());
    let measured = list_json(&["--pid", &std::process::id().to_string(), "--cpu=250ms"]);
    assert!(measured["cpu_interval_ms"].as_u64().unwrap() >= 250);
    assert!(measured["processes"][0]["cpu_percent"].is_number());
}

#[test]
fn bare_stop_is_top_and_pretty_implies_json() {
    let output = stop().arg("--pretty").output().unwrap();
    assert!(output.status.success());
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(report["memory_total_bytes"].is_number());
    assert!(report["cpu_percent"].is_number());
    assert!(report["returned"].as_u64().unwrap() <= 10);
}

#[test]
fn memory_ranking_is_immediate_and_filterable() {
    let output = stop()
        .args([
            "top",
            "--sort",
            "mem",
            "--pid",
            &std::process::id().to_string(),
            "--json",
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(report["cpu_percent"].is_null());
    assert!(report["cpu_interval_ms"].is_null());
    assert_eq!(report["processes"].as_array().unwrap().len(), 1);
}

#[test]
fn inspect_and_tree_do_not_silently_broaden_names() {
    let mut child = std::process::Command::new("sleep")
        .arg("10")
        .spawn()
        .unwrap();
    let results = ["inspect", "tree"]
        .map(|command| stop().args([command, "--name", "slee", "--json"]).output());
    child.kill().unwrap();
    child.wait().unwrap();
    for output in results {
        let output = output.unwrap();
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stderr.is_empty());
        let _: Value = serde_json::from_slice(&output.stdout).unwrap();
    }
}

#[test]
fn sample_no_match_still_emits_every_point() {
    let output = stop()
        .args([
            "sample",
            "--name",
            "zz-no-process-xyz",
            "--count",
            "2",
            "--interval",
            "200ms",
            "--jsonl",
        ])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    let points: Vec<Value> = String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(points.len(), 2);
    assert!(points.iter().all(|p| p["returned"] == 0));
}
