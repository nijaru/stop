//! Real-process lifecycle checks: streaming, pipe closure, signals and exec.

use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Child, ChildStdout, Command, ExitStatus, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use serde_json::Value;
use stop::collector::{Request, Sampler, TargetObservation};

struct OwnedChild(Child);
impl Drop for OwnedChild {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn sampler(format: &str) -> OwnedChild {
    OwnedChild(
        Command::new(env!("CARGO_BIN_EXE_stop"))
            .args([
                "sample",
                "--pid",
                &std::process::id().to_string(),
                "--count",
                "1000",
                "--interval",
                "1h",
                format,
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap(),
    )
}

fn first_point(child: &mut OwnedChild) -> (Value, BufReader<ChildStdout>) {
    let mut reader = BufReader::new(child.0.stdout.take().unwrap());
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        let _ = tx.send((line, reader));
    });
    let (line, reader) = rx
        .recv_timeout(Duration::from_secs(10))
        .expect("first point must stream before the next one-hour interval");
    (serde_json::from_str(&line).unwrap(), reader)
}

fn finish(child: &mut OwnedChild) -> ExitStatus {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        if let Some(status) = child.0.try_wait().unwrap() {
            return status;
        }
        assert!(Instant::now() < deadline, "sampler did not stop promptly");
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn jsonl_streams_and_closed_consumer_stops_long_interval_sampling() {
    let mut child = sampler("--jsonl");
    let (point, reader) = first_point(&mut child);
    assert_eq!(point["index"], 1);
    assert_eq!(point["requested_count"], 1000);
    assert_eq!(point["processes"][0]["pid"], std::process::id());
    drop(reader);
    assert_eq!(finish(&mut child).code(), Some(0));
}

#[cfg(unix)]
#[test]
fn sigint_preserves_streamed_points_and_exits_130() {
    let mut child = sampler("--jsonl");
    let (point, mut reader) = first_point(&mut child);
    assert_eq!(point["index"], 1);
    // SAFETY: this is our live child, synchronized past handler installation by its output.
    assert_eq!(
        unsafe { libc::kill(child.0.id().try_into().unwrap(), libc::SIGINT) },
        0
    );
    assert_eq!(finish(&mut child).code(), Some(130));
    let mut tail = String::new();
    reader.read_to_string(&mut tail).unwrap();
    assert!(tail.is_empty(), "no partial JSON line on interruption");
    let mut error = String::new();
    child
        .0
        .stderr
        .take()
        .unwrap()
        .read_to_string(&mut error)
        .unwrap();
    let error: Value = serde_json::from_str(&error).unwrap();
    assert_eq!(error["code"], "interrupted");
}

#[cfg(unix)]
#[test]
fn sampling_refreshes_exec_names_before_filtering() {
    // A pipe controls exec, rather than relying on a sleep racing the first collection.
    let mut child = OwnedChild(
        Command::new("/bin/sh")
            .args(["-c", "read line; exec sleep 30"])
            .stdin(Stdio::piped())
            .spawn()
            .unwrap(),
    );
    let mut sampler = Sampler::new(Request {
        pid: Some(child.0.id()),
        name: Some("sleep".into()),
        ..Request::default()
    });
    let first = sampler.sample();
    assert!(first.processes.is_empty());
    assert!(
        matches!(first.target, TargetObservation::Observed(_)),
        "PID binding must precede name filtering"
    );
    child.0.stdin.take().unwrap().write_all(b"go\n").unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let data = sampler.sample();
        if let Some(process) = data.processes.first() {
            assert_eq!(process.name, "sleep");
            assert_eq!(process.pid, child.0.id());
            return;
        }
        assert!(Instant::now() < deadline, "exec name remained stale");
        std::thread::sleep(Duration::from_millis(10));
    }
}
