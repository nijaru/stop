# Agent-first `stop` redesign

This branch explores a ground-up design for `stop` as a local observation CLI that is pleasant for humans and deterministic for agents.

## Product boundary

`stop` answers questions about live local machine state. It is intentionally observational: no signals, kill, renice, freeze, or other mutation operations belong in the core tool.

The CLI is the primary interface. Human-readable text is the default. `--json` is the stable machine representation, and `--jsonl` is reserved for repeated or streaming observations.

A future TUI, if any, must be only another renderer/client of the same core query and result model. It must not own collection or semantics.

## Core operations

The conceptual API is deliberately small:

- `list` — what exists?
- `inspect` — what is this?
- `sample` — what is it doing over an interval?
- `snapshot` — record state.
- `diff` — what changed between observations?
- `wait` — wait until a state predicate becomes true.

Human-oriented views may sit on top of those operations:

- `top` — ranked/limited process list.
- `tree` — process relationship view.

Views are not separate core abstractions.

## CLI rules

1. `stop` with no subcommand should remain useful and human-friendly; the intended default is the `top` view.
2. Human-readable text is the default output.
3. `--json` always emits JSON regardless of TTY/PTY state.
4. `--jsonl` is used only for streams/repeated results.
5. stdout contains requested results. stderr contains diagnostics.
6. Machine output never contains ANSI escapes, banners, spinners, or prose warnings.
7. Explicit typed selectors are canonical (`--pid`, `--name`, `--port`, `--cwd`, `--exe`, `--user`). Positional inference may be added only as convenience sugar.
8. Selector and field names retain the same meaning across commands.
9. `list` is exhaustive unless the caller explicitly supplies a limit. `top` is intentionally ranked and limited.
10. Durations and byte sizes use explicit units (`250ms`, `5s`, `500MiB`).

## Core data model

A process is not just a row. The long-term model is a graph of observable resources and relationships:

```text
Process
  -> parent / children -> Process
  -> executable        -> File
  -> cwd               -> Directory
  -> open files        -> File
  -> listening sockets -> Socket
  -> connections       -> Endpoint
  -> container/service -> Runtime identity
```

Process identity must be stronger than PID alone. The canonical identity is at least `(pid, start_time)` so PID reuse cannot silently refer to a different process.

## Result semantics

Structured responses must make completeness explicit. A caller must be able to distinguish:

- no values exist,
- values could not be observed because of permissions,
- the platform does not support the field,
- the result was intentionally truncated.

Machine responses will converge on a versioned envelope with observation metadata such as:

```json
{
  "schema": "stop/1",
  "observed_at": "...",
  "result": {},
  "meta": {
    "complete": true,
    "matched": 1,
    "returned": 1,
    "truncated": false,
    "sample_window_ms": 200
  }
}
```

## Collection architecture

The public model must not be defined by one portability crate. Platform collectors should eventually live behind our own contract:

```text
collector/
  linux
  macos
  windows
```

`sysinfo` can remain useful during bootstrap, but native collectors may replace or supplement it where correctness, identity, sockets, files, or provenance require platform-specific APIs.

Collection should be progressive:

- cheap process summary fields by default,
- expensive relationships/enrichments only when requested.

## Temporal state

Time is a first-class part of the design rather than a watch-mode afterthought.

`sample` measures rates/deltas over a defined interval.

`snapshot` captures an inspectable observation.

`diff` compares observations semantically: process started/exited, resource deltas, sockets opened/closed, and other relationship changes.

`wait` replaces shell polling loops for conditions such as process exit or a port becoming available.

Temporal comparison is expected to be one of the primary differentiators from conventional process-table tools.

## Security

Read-only does not mean non-sensitive. Full command lines, environment variables, paths, and peer endpoints can contain secrets.

Sensitive enrichments should therefore be explicit and independently permissionable/redactable. Environment collection should not be part of a default process summary.

## Initial implementation strategy

Do not rewrite every feature at once.

1. Introduce the new typed observation model and selector layer alongside the current implementation.
2. Add an experimental CLI client using the new model while keeping the existing `stop` binary intact.
3. Establish deterministic text/JSON rendering and exhaustive-vs-ranked semantics.
4. Add process identity/context fields and native collector seams.
5. Implement relationships (parent/child first, then ports/files).
6. Implement `sample`, `snapshot`, `diff`, and `wait` against the same model.
7. Replace the legacy binary only after the new path has task-level tests and parity for the behavior we want to keep.

## Evaluation

Features should be justified against agent tasks, not monitoring-tool feature counts. Benchmark at least:

- tool calls required,
- bytes/tokens returned,
- latency,
- ability to detect incomplete/ambiguous results,
- correctness of process identity,
- cross-platform semantic consistency.

The target competitors are native `ps`/`pgrep`/`lsof`, `pidstat`, `procs`, `proc`, and `osquery`. `stop` should not try to out-feature all of them. It should minimize the work required to answer common local-state questions safely and correctly.
