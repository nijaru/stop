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
    read_point(child.0.stdout.take().unwrap())
}

fn read_point<R: Read + Send + 'static>(stream: R) -> (Value, BufReader<R>) {
    let mut reader = BufReader::new(stream);
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
fn fill_socket(producer: &mut std::os::unix::net::UnixStream) {
    producer.set_nonblocking(true).unwrap();
    loop {
        match producer.write(&[b' '; 4096]) {
            Ok(0) => panic!("socket closed during setup"),
            Ok(_) => {}
            Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => break,
            Err(err) => panic!("cannot fill socket: {err}"),
        }
    }
    producer.set_nonblocking(false).unwrap();
}

#[cfg(unix)]
#[test]
fn help_and_parse_errors_are_interruptible_without_leaking_stdout_flags() {
    use std::os::{
        fd::{AsRawFd, OwnedFd},
        unix::net::UnixStream,
    };
    for arg in ["--help", "--not-an-option"] {
        let (_unread_consumer, mut producer) = UnixStream::pair().unwrap();
        fill_socket(&mut producer);
        let mut child = OwnedChild(
            Command::new(env!("CARGO_BIN_EXE_stop"))
                .arg(arg)
                .stdout(Stdio::from(OwnedFd::from(producer.try_clone().unwrap())))
                .stderr(Stdio::from(OwnedFd::from(producer.try_clone().unwrap())))
                .spawn()
                .unwrap(),
        );
        std::thread::sleep(Duration::from_millis(250));
        // SAFETY: the live child is ours; only its standard streams are blocked.
        assert_eq!(
            unsafe { libc::kill(child.0.id().try_into().unwrap(), libc::SIGINT) },
            0
        );
        assert_eq!(finish(&mut child).code(), Some(130));
        // SAFETY: our sending handle stays open after child termination.
        let flags = unsafe { libc::fcntl(producer.as_raw_fd(), libc::F_GETFL) };
        assert!(flags >= 0);
        assert_eq!(flags & libc::O_NONBLOCK, 0, "flags leaked by {arg}");
    }
}

#[cfg(unix)]
#[test]
fn sigint_stops_a_blocked_writer_and_restores_shared_stdout_flags() {
    use std::os::{
        fd::{AsRawFd, OwnedFd},
        unix::net::UnixStream,
    };

    for merged_stderr in [false, true] {
        // Keep the receiving end open but stop reading after the first point. The
        // extra sending handle lets us fill the buffer without depending on the
        // machine's process count or waiting for hundreds of sample points.
        let (consumer, mut producer) = UnixStream::pair().unwrap();
        // SAFETY: producer owns this live descriptor; F_GETFL takes no pointer.
        let original = unsafe { libc::fcntl(producer.as_raw_fd(), libc::F_GETFL) };
        assert!(original >= 0);
        let mut child = OwnedChild(
            Command::new(env!("CARGO_BIN_EXE_stop"))
                .args([
                    "sample",
                    "--pid",
                    &std::process::id().to_string(),
                    "--count",
                    "1000",
                    "--interval",
                    "200ms",
                    "--jsonl",
                ])
                .stdout(Stdio::from(OwnedFd::from(producer.try_clone().unwrap())))
                .stderr(if merged_stderr {
                    Stdio::from(OwnedFd::from(producer.try_clone().unwrap()))
                } else {
                    Stdio::piped()
                })
                .spawn()
                .unwrap(),
        );
        let (point, _unread_consumer) = read_point(consumer);
        assert_eq!(point["index"], 1);
        let pid = child.0.id().try_into().unwrap();
        // Pause our child while filling: descriptor flags are shared, so it must
        // not restore its output mode concurrently with the parent's setup.
        let mut status = 0;
        // SAFETY: our owned, live child PID and a valid wait-status pointer.
        unsafe {
            assert_eq!(libc::kill(pid, libc::SIGSTOP), 0);
            assert_eq!(libc::waitpid(pid, &mut status, libc::WUNTRACED), pid);
        }
        assert!(libc::WIFSTOPPED(status));
        fill_socket(&mut producer);
        // SAFETY: resume only our owned, stopped child.
        assert_eq!(unsafe { libc::kill(pid, libc::SIGCONT) }, 0);
        // Let the next 200ms point reach the full output buffer before interruption.
        std::thread::sleep(Duration::from_millis(600));
        // SAFETY: our live child has installed its handler before emitting point 1.
        assert_eq!(
            unsafe { libc::kill(child.0.id().try_into().unwrap(), libc::SIGINT) },
            0
        );
        assert_eq!(finish(&mut child).code(), Some(130));
        // Check the flag we change, not OS-maintained bits (Darwin records writes).
        // SAFETY: the producer is still owned and open after the child exits.
        let restored = unsafe { libc::fcntl(producer.as_raw_fd(), libc::F_GETFL) };
        assert!(restored >= 0);
        assert_eq!(restored & libc::O_NONBLOCK, original & libc::O_NONBLOCK);
    }
}

#[cfg(unix)]
#[test]
fn inspect_preserves_empty_and_whitespace_arguments() {
    let program = "/bin//sh";
    let long = "a long argument with 日本語 and 👩‍💻 ".repeat(20);
    let args = [
        "-c",
        "read line",
        "stop-argv-check",
        "two words",
        "",
        " \t padded \n ",
        "Unicode-λ",
        &long,
        "",
    ];
    let child = OwnedChild(
        Command::new(program)
            .args(args)
            .env_clear()
            .env("STOP_ARGS_ENV", "not_an_argument")
            .stdin(Stdio::piped())
            .spawn()
            .unwrap(),
    );
    let output = Command::new(env!("CARGO_BIN_EXE_stop"))
        .args(["inspect", &child.0.id().to_string(), "--json"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    let expected: Vec<_> = std::iter::once(program).chain(args).collect();
    assert_eq!(report["process"]["cmdline"], serde_json::json!(expected));

    let human = Command::new(env!("CARGO_BIN_EXE_stop"))
        .args(["inspect", &child.0.id().to_string()])
        .output()
        .unwrap();
    assert!(human.status.success());
    let text = String::from_utf8(human.stdout).unwrap();
    let mut quoted = Vec::<String>::new();
    let mut indent = 0;
    for line in text
        .lines()
        .skip_while(|line| *line != "Arguments")
        .skip(1)
        .take_while(|line| line.starts_with("  "))
    {
        if let Some(entry) = line.strip_prefix("  [") {
            let (index, value) = entry.split_once("] ").unwrap();
            assert_eq!(index.parse::<usize>().unwrap(), quoted.len());
            indent = line.len() - value.len();
            quoted.push(value.to_owned());
        } else {
            quoted.last_mut().unwrap().push_str(&line[indent..]);
        }
    }
    assert_eq!(
        quoted,
        expected
            .iter()
            .map(|arg| format!("{arg:?}"))
            .collect::<Vec<_>>(),
        "human inspection did not preserve argv:\n{text}"
    );
}

#[cfg(target_os = "linux")]
#[test]
fn inspect_thread_count_includes_the_main_thread() {
    let child = OwnedChild(
        Command::new("/bin/sh")
            .args(["-c", "read line"])
            .stdin(Stdio::piped())
            .spawn()
            .unwrap(),
    );
    let output = Command::new(env!("CARGO_BIN_EXE_stop"))
        .args(["inspect", &child.0.id().to_string(), "--json"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["process"]["threads"], 1);
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
