# stop

**stop** (structured top) is a process monitoring tool built for AI agents and automation: JSON-first process data instead of human-oriented text parsing.

```
$ stop list --json | jq '.processes[0]'
{
  "pid": 1431,
  "start_time": 1787355876,
  "ppid": 1,
  "name": "ghostty",
  "exe": "/Applications/Ghostty.app/Contents/MacOS/ghostty",
  "cmdline": ["/Applications/Ghostty.app/Contents/MacOS/ghostty"],
  "cwd": "/",
  "state": "sleep",
  "user": "nick",
  "uid": "501",
  "cpu_percent": 10.4,
  "rss_bytes": 648085504,
  "virtual_bytes": 448247283712,
  "threads": null,
  "io_read_bytes": 68861952,
  "io_written_bytes": 364544
}
```

## Install

```bash
cargo install --path .
```

Requires Rust stable. Tested on macOS and Linux.

## Usage

### `stop list` — query the process table

```bash
stop list                          # human-readable table, sorted by CPU
stop list --name postgres          # case-insensitive substring match
stop list --user nick              # by username or raw UID
stop list --sort mem --limit 20    # top 20 by memory
stop list --json                   # full JSON envelope
stop list --fast                   # ~24ms instead of ~240ms (see below)
```

JSON envelope:

```json
{
  "collected_at": "2026-08-26T08:56:41+00:00",
  "total_processes": 924,
  "matched": 924,
  "returned": 924,
  "truncated": false,
  "processes": [ ... ]
}
```

`truncated` is true only when a limit cut rows from `matched`.

### Fast mode

Accurate CPU percentages require two refreshes separated by ~200 ms, so a
default run costs ~240 ms wall time. Pass `--fast` (on `list` and `top`) to
skip the warm-up and collect in ~25 ms — but every `cpu_percent`, including
the system summary, is reported as `null` rather than a wrong number, and
`--sort cpu` ordering is meaningless in that mode.

### `stop inspect` — inspect a process or port

```bash
stop inspect 1431                  # by PID
stop inspect ghostty               # by exact name (case-insensitive)
stop inspect ghost                 # substring fallback
stop inspect ghostty --json        # machine-readable record
stop inspect --port 3000           # visible TCP listeners and UDP bindings
stop inspect --port 3000 --json    # machine-readable ownership report
```

Numeric targets are PIDs; otherwise the target matches names — exact match first, then substring. If several processes match, exit code 3 reports candidate identities so you can retry by PID:

```json
{"code":"ambiguous","message":"3 processes match 'node'; disambiguate by PID","target":"node","candidates":[{"pid":37259,"start_time":1787722914,"name":"node","user":"nick"}]}
```

The `--port` form reports all visible TCP listeners and UDP bindings on the
requested port. TCP connections that are not listening are excluded. Port
ownership can be incomplete without sufficient permissions; JSON reports this
as `visibility: "partial"`, along with inaccessible processes and
unattributed sockets. Port 0 is not a valid query. The JSON shape is:

```json
{"port":3000,"visibility":"complete","inaccessible_processes":0,"unattributed_sockets":0,"owners":[{"process":{...},"sockets":[{"protocol":"tcp","local_address":"127.0.0.1","local_port":3000,"state":"listen"}]}]}
```

### `stop top` — system summary + ranked processes

```bash
stop top                           # system metrics header + top 10 by CPU
stop top --sort mem --limit 25     # rank by memory
stop top --json                    # metrics and snapshot in one document
```

### `stop tree` — parent/child hierarchy

```bash
stop tree                           # full process forest
stop tree 1431                      # subtree rooted at PID 1431
stop tree ghostty                   # subtree rooted at that process (name rules like inspect)
stop tree --json                    # nested JSON: { collected_at, total_processes, roots: [...] }
```

Every collected process appears exactly once. Roots are processes whose
parent is absent (exited or unavailable); cycles from PID reuse are broken
at the first back-edge, with cycle members re-rooted rather than dropped.
With a name target, ambiguity exits 3 with candidates, like `inspect`.

## Process fields (P0 model)

| Field | Notes |
|---|---|
| `pid`, `start_time` | Together they form process identity; `start_time` (unix seconds) guards against PID reuse |
| `ppid` | Parent PID, `null` when unavailable or parent exited |
| `name`, `exe`, `cmdline`, `cwd` | Location facts; unavailable values are `null` |
| `state` | One of `idle`, `run`, `sleep`, `stop`, `zombie`, `tracing`, `dead`, `wakekill`, `waking`, `parked`, `lock_blocked`, `disk_sleep`, `unknown` |
| `user`, `uid` | Resolved username and raw ID |
| `cpu_percent` | Total across cores, can exceed 100; **null under `--fast`** |
| `rss_bytes`, `virtual_bytes` | Raw byte counts |
| `threads` | Thread count (`null` on macOS) |
| `io_read_bytes`, `io_written_bytes` | Cumulative totals |

## Exit codes

| Code | Meaning |
|---|---|
| 0 | Success, at least one process returned |
| 1 | Operational error |
| 2 | No match |
| 3 | Target ambiguous for `inspect`/`tree` (candidates in error payload) |

Enables shell checks like `stop inspect nginx && echo running`.

## Design goals

1. **AI-first** — JSON primary, humans secondary
2. **Cross-platform** — same contracts on macOS and Linux
3. **Honest metadata** — truncation and availability are explicit, never silent
4. **Small surface** — few commands, well-defined semantics

## Development

Requires Rust ≥ 1.95 (`rust-version` is declared, driven by sysinfo 0.39).

```bash
cargo test      # unit + integration suite
cargo clippy    # zero-warning policy
cargo fmt
cargo build --release
```

Collection includes a mandatory ~200 ms warm-up so CPU percentages reflect real deltas.

## Known limitations

- **Threads**: not reported on macOS (sysinfo limitation); `null`
- **Disk I/O**: often zero on macOS for idle processes
- **Windows**: untested; port ownership is currently unsupported
- Port ownership is implemented on Linux and macOS; permissions can make
  attribution partial
- Per-process network metrics: not available via sysinfo

## Roadmap

Planned next on this architecture: port/socket ownership (`stop inspect --port 3000`), time-series sampling, snapshots + diff, and wait-for-process.

## License

MIT
