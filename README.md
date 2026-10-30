# stop

Inspect processes without leaving the shell. **stop** prints one-shot reports
and finite samples—not a full-screen monitor. Human-readable output is the
default; use `--json` for scripts or `--jsonl` for streaming samples.

Supported and tested on Linux and macOS. Despite its name, stop does not
terminate processes. There is no daemon, automatic pager, or unbounded watch
mode.

## Install

With Rust 1.95 or newer:

```bash
cargo install stop-cli --locked
stop --version
```

The crate is named `stop-cli`; the installed command is `stop`. To build the
latest source instead:

```bash
git clone https://github.com/nijaru/stop.git
cd stop
cargo install --path .
```

## Quick start

```bash
stop                                  # system summary + top 10 by CPU
stop list --name postgres              # immediate process inventory
stop inspect 1431                      # identity, arguments, paths and resources
stop top --sort mem --limit 20         # rank by RSS, without a CPU warm-up
stop sample --name node --count 5      # five filtered reports as they arrive
```

## Commands

### Inventory and ranking

```bash
stop list                             # PID order, no implicit row limit
stop list --pid 1431
stop list --name postgres --user nick
stop list --sort mem --limit 20
stop list --sort cpu --limit 10        # requests a CPU measurement
stop list --cpu=500ms --json           # PID order, with measured CPU
stop top                              # same report as bare stop
stop top --sort mem --name node
```

`list`, `top`, and `sample` share `--pid`, `--name`, `--user`, `--sort`, and
`--limit`. Filters combine with AND. Names use case-insensitive substring
search; owners match a username or raw UID. Sorting is deterministic, with
PID as the tie-breaker and unavailable measurements last. `list` and `sample`
accept `cpu`, `mem`, `pid`, or `name`; `top` accepts `cpu` or `mem`.

`top` defaults to CPU ranking and ten rows. `list` defaults to PID order and
all matching rows. Counts are measured before limiting: `matched` is the
number of collected matches, `returned` is the number emitted, and `truncated`
is true only when a limit removed rows.

### Inspect a process or port

```bash
stop inspect 1431
stop inspect postgres                 # exact name, case-insensitive
stop inspect --name 123               # a numeric process name, not PID 123
stop inspect 1431 --cpu
stop inspect --port 3000 --json
```

A canonical decimal positional target is a PID; other targets are exact
names. There is no substring fallback. Search with `stop list --name ...`,
then inspect a PID. Multiple exact matches return exit 3 with candidate
identities on stderr. Human details quote argument boundaries; JSON preserves
the argument array. Arguments and paths can contain secrets—take care before
sharing an inspection report.

Port inspection reports visible TCP listeners and UDP bindings. Established
TCP connections are excluded. Ownership is joined by PID **and native birth
ID**, never PID alone. `visibility: "partial"`, `inaccessible_processes`, and
`unattributed_sockets` describe known attribution gaps. Even a complete report
is limited to the observed process/network scope; it is not proof that a port
is universally free. Linux socket discovery uses the caller's network namespace.
Port inspection is unsupported on other platforms.

### Process trees

```bash
stop tree                             # full observed forest
stop tree 1431                        # subtree rooted at this PID
stop tree --name postgres --json
```

Name resolution follows `inspect`. Every collected process appears once in
the full forest. Missing parents become roots; self-links, known later-born
parents, and cycles cannot hide nodes. These are observed parent relationships,
not a historical or atomic lineage record.

### Finite sampling

```bash
stop sample                           # five points, 1s period, top 10 by CPU
stop sample --pid 1431 --count 10 --interval 500ms
stop sample --user nick --sort mem --limit 5 --rate 2 --jsonl
stop sample --name node --count 3 --json
```

`--count` accepts 1–1000. `--interval` accepts durations with `ms`, `s`, `m`,
or `h` suffixes, from 200ms to 24h. `--rate` is an alternative in Hz, up to
5 Hz, and cannot be combined with `--interval`.

Sampling primes CPU once, waits for the first measurement, then follows a
monotonic start-to-start schedule. It waits for fresh counters and skips
elapsed slots when collection, rendering, or output backpressure overruns the
schedule; it never emits catch-up bursts. `skipped_intervals` counts skipped
slots before the current point. Requested periods are not timing guarantees:
use acquisition timestamps and `cpu_interval_ms` for actual observations.

`--pid` binds the first observed native identity, before other filters. A
replacement process cannot take over the sample. `target_status` distinguishes
`not_observed_yet`, `observed`, and `no_longer_observed`; an observed identity
may still fail an additional name/owner filter. If a PID is visible but its
native identity is unavailable, sampling it fails rather than binding weakly.
Name and owner queries are reevaluated at each point, including after exec.
Empty points are still emitted. A completed run exits 0 if any point matched,
or 2 if none did.

Human output and `--jsonl` stream each point immediately. `--json` retains
points and emits one final document with `requested_count`, `count`,
`completed`, `interrupted`, and `samples`. These flags describe collection,
before final JSON delivery; interrupting delivery can exit 130 even if
collection had already completed.

Handled Ctrl-C exits 130. With a responsive consumer, interruption during
collection leaves a valid incomplete JSON document or complete JSONL points.
If a pipe consumer stops reading, stop allows a short flush grace period,
then exits even if the current record or document is unfinished; earlier
complete JSONL records remain valid. Closing a pipe consumer stops sampling
successfully, including during long waits. Forced termination cannot guarantee
valid output.

## CPU and availability

Inventory, inspection, trees, port lookup, and memory ranking do not measure
CPU unless requested. `--cpu` adds a default 250ms measurement window;
`--cpu=500ms` chooses another window (200ms–24h). The equals sign is required
for an explicit duration. CPU sorting requests the default window automatically.
Sampling always measures CPU over its own cadence and rejects `--cpu`.

Process CPU is percent of **one core**, so it can exceed 100%. System CPU is
normalized across cores. A new process, unavailable counter, or invalid
baseline yields `null`, not a fabricated zero. Human reports omit unrequested
CPU columns and show `n/a` for unavailable values (`not supported` for fields
the platform does not provide). Memory columns use RSS and explicit binary
units such as MiB; JSON uses raw bytes.

Unavailable scalar facts are `null`. The sparse `unavailable` map explains
missing fields with `not_requested`, `missing_baseline`, `unknown`, or
`unsupported`. Inventory does not request command lines, executable paths,
working directories, thread counts, or cumulative I/O; `inspect` requests them.
An empty `cmdline` is explained in the same map. The backend cannot distinguish
some unreadable zero counters from real zero: ambiguous zero memory/I/O facts
and zero start timestamps are reported as unknown. Thread counts are currently
unsupported on macOS. Human output escapes terminal controls in process text;
`NO_COLOR` disables color, which is otherwise terminal-only.

Human reports use local time; process start times include the UTC offset.
Sample headings show the local date and time to the millisecond. Detailed acquisition
windows and CPU measurement intervals are available in JSON. Process tables
shorten names and usernames to fit the terminal, without shortening numbers.
A short notice identifies incomplete process data or an incomplete port
search. An empty report means nothing was found, not that a process or port
is guaranteed absent.

## JSON contract

All machine formats are explicit; piping does not change the format. `--pretty`
implies `--json`. `--jsonl` is available only for `sample`. Reports use
`schema_version: 1` (also on each sample point), snake_case keys, and additive
availability metadata. This schema version is independent of the crate version.

Collection metadata includes:

- `collection_started_at`, `collected_at`, `collection_duration_ms`: the
  acquisition window, not an atomic snapshot.
- `cpu_interval_ms`: the actual system CPU baseline interval, or `null` when
  CPU was not measured. Per-process deltas span the corresponding process reads.
- `scope_id`: an opaque boot/PID-numbering scope, or `null` if unavailable.
- `scope_pid`: a PID-scoped collection, or `null` for the visible inventory.
- `inaccessible_processes`: observed candidates with limited basic facts or
  lost during enrichment. It cannot count processes hidden from enumeration.

Process correlation requires **`scope_id` + `pid` + `birth_id`**. Birth IDs are
opaque native tokens, not timestamps. `start_time` is a human Unix-seconds
field, not a reuse-safe identity. If scope or birth ID is unavailable, do not
infer identity continuity across invocations.

Report shapes (metadata omitted here):

| Command | Payload |
|---|---|
| `list` | `total_processes`, `matched`, `returned`, `truncated`, `processes` |
| `top` / bare `stop` | List payload plus system CPU and memory metrics |
| `inspect <target>` | `target`, `process` (object or `null`) |
| `tree` | `total_processes`, `roots` (nodes with `process` and `children`) |
| `inspect --port` | `port`, `visibility`, `inaccessible_processes`, `unattributed_sockets`, `owners` |
| `sample --json` | `started_at`, `interval_ms`, `requested_count`, `count`, `completed`, `interrupted`, `samples` |
| `sample --jsonl` | One self-contained point per line: index/count/period, skipped slots, collection metadata, list payload, system metrics, optional target status |

`total_processes` counts the visible inventory within the requested PID scope,
before query filtering; a full tree counts successfully collected nodes.
Port reports include `scope_id`, process acquisition metadata in
`process_collection`, and separate `scan_started_at`, `scanned_at`, and
`scan_duration_ms` fields. Each owner has `process` and `sockets`; socket
records contain `protocol`, `local_address`, `local_port`, and `state`.

Successful and empty reports go to stdout. Ambiguity, usage, and operational
errors go to stderr, as JSON when a machine format was selected. An empty
result means **no match observed**, not universal absence.

| Exit | Meaning |
|---|---|
| 0 | Success (or a consumer closed the output pipe) |
| 1 | Usage or operational error |
| 2 | No match observed |
| 3 | Ambiguous `inspect`/`tree` target; candidates reported |
| 130 | Handled interruption |

## Upgrading from 0.0.x

0.1.0 changes the CLI and JSON contracts. Update scripts rather than treating
this as a presentation-only upgrade:

- Replace `--watch` with finite `sample --count N --interval 1s`. Use `--jsonl`
  for streaming output; there is no unbounded watch mode.
- Replace `--sort-by mem --top-n 5` with `top --sort mem --limit 5`.
- Replace name, user, and PID `--filter` expressions with explicit selectors
  on `list`, `top`, or `sample`. For numeric predicates, filter JSON with a
  tool such as `jq`. CSV output is no longer supported.
- Bare `stop` now shows a system summary and the top ten processes by CPU.
  Use `list` for immediate PID-ordered inventory without a CPU warm-up.
- Inspect/tree names resolve exactly; use `list --name` for substring search.
- Machine reports use the envelopes, nullable facts, native identity, timing,
  and exit codes documented above. `--pretty` selects JSON on its own, and
  empty results go to stdout with exit 2.

## Development

```bash
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt --check
cargo build --release
```

To verify the published package from a clean working tree:

```bash
bash scripts/check-package.sh
```

This check needs Python 3. It builds and verifies the crate archive, installs
it into a temporary prefix, and exercises that installed binary. It does
not publish anything or replace your installed `stop`.

Development tracks stable Rust through `rust-toolchain.toml`. CI tests both
stable and the Rust 1.95 minimum on Linux and macOS. `Cargo.lock` is
intentionally ignored; each CI job resolves dependencies freshly, then uses
that graph consistently for its checks. Published crates include a generated
lockfile; `cargo install --locked` uses that release's dependency graph.
`bench.sh` compares collection modes using an already installed `hyperfine`.

## License

MIT
