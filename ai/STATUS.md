# Status

## Current State

**Version**: v0.0.1
**Phase**: 1 - Core features complete
**Last Updated**: 2025-11-04

## What Works

**Core Features:**
- ✅ Process listing with system metrics
- ✅ Filter functionality (`--filter` flag)
  - Simple `field op value` syntax
  - Compound expressions with `and`/`or` logic (case-insensitive)
  - Proper precedence (AND before OR)
  - Fields: cpu, mem, pid, name, user
  - Operators: >, >=, <, <=, ==, !=
  - AI-friendly JSON error messages
- ✅ Sorting (`--sort-by` flag: cpu, mem, pid, name)
- ✅ Top-N limiting (`--top-n` flag, default 20)

**Output Modes:**
- ✅ JSON output (`--json` flag)
- ✅ CSV output (`--csv` flag) - RFC 4180 compliant
- ✅ Human-readable table with colors and system summary
- ✅ Watch mode (`--watch` flag) - continuous updates with configurable interval
  - NDJSON for JSON mode (stream-friendly)
  - Screen clearing for human-readable mode
  - Default 2s interval, configurable with `--interval`

**Code Quality:**
- ✅ 29 tests (16 unit + 13 integration), all passing
- ✅ Zero clippy warnings
- ✅ Proper error handling with thiserror
- ✅ Type-safe filter module with parse-time validation
- ✅ Cross-platform data collection (sysinfo)

## What Doesn't Work Yet

- ❌ Parentheses for complex filter grouping (Phase 3)
- ❌ Disk I/O metrics
- ❌ Network metrics
- ❌ Thread information

## Recent Changes

**Cross-platform validation (2025-11-04)**:
- ✅ Tested on Fedora Linux (i9-13900KF, 32GB DDR5)
- ✅ All 29 tests pass on Linux
- ✅ Fixed clippy warnings for stricter Rust versions
- ✅ Identical functionality on macOS and Linux
- ⚠️ User field shows UIDs on both platforms (sysinfo limitation)

**Compound filter expressions (2025-11-04)**:
- ✅ Implemented AND/OR logic with SQL-style keywords (case-insensitive)
- ✅ Proper precedence: AND before OR (standard boolean algebra)
- ✅ Recursive descent parser with whole-word keyword matching
- ✅ Short-circuit evaluation for performance
- ✅ 8 new unit tests for compound expressions (29 tests total)
- ✅ Updated README with comprehensive examples
- ✅ End-to-end testing verified

**Phase 1 complete (2025-11-04)**:
- ✅ Implemented filter module with type-safe parsing and evaluation
- ✅ Added thiserror for proper error handling
- ✅ Implemented sort-by functionality (cpu, mem, pid, name)
- ✅ Implemented top-n limiting (works for all output modes)
- ✅ Added CSV output with RFC 4180 escaping
- ✅ Color-coded human-readable output (owo-colors)
- ✅ 21 comprehensive tests (8 unit + 13 integration)
- ✅ Updated README with examples and filter syntax docs
- ✅ Zero clippy warnings, all tests passing

**Initial implementation (bd0d51c)**:
- Created basic MVP with sysinfo 0.37
- System metrics: CPU usage, memory total/used/percent
- Process info: PID, name, CPU%, memory%, user, command
- Both JSON and human-readable output modes
- CLI scaffolding with clap 4.5

## Next Steps

**Completed:**
1. ✅ Performance benchmarks (29ms overhead, well under 100ms goal)
2. ✅ Cross-platform testing on Fedora (all tests pass)
3. ✅ All documentation updated

**Next: Phase 3 Features**
- Disk I/O metrics per process (read/write bytes)
- Network metrics per process (rx/tx bytes)
- Thread information (thread count)
- Open file descriptors/handles
- Parentheses for explicit filter grouping (optional enhancement)
- Windows support testing

## Known Issues

- **User field shows UIDs on both macOS and Linux** (e.g., "501" on macOS, "1000" on Linux) instead of usernames - sysinfo crate limitation, affects both platforms equally
- Process commands may be empty for kernel threads (expected behavior)
- 200ms sleep in collect_snapshot for accurate CPU readings (acceptable, documented)

## Performance

- Collection time: ~200ms (includes mandatory sleep for CPU accuracy)
- Memory usage: Minimal (processes collected once)
- Output size: ~500 bytes per process in JSON

## Development Environment

- Rust: 1.91.0 (edition 2024)
- Key dependencies: sysinfo 0.37, clap 4.5, serde 1.0, chrono 0.4
- Platforms tested: macOS (M3 Max), Fedora (x86_64)
