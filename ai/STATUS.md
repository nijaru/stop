# Status

## Current State

**Version**: v0.0.1 (working toward v0.1.0)
**Phase**: 1 (MVP → v0.1.0) - Core functionality complete
**Last Updated**: 2025-11-04

## What Works

- ✅ Basic process listing with system metrics
- ✅ JSON output (`--json` flag)
- ✅ Human-readable table output with summary info
- ✅ CLI argument parsing (clap)
- ✅ Cross-platform data collection (sysinfo)
- ✅ **Filter functionality** (`--filter` flag)
  - Simple `field op value` syntax
  - Fields: cpu, mem, pid, name, user
  - Operators: >, >=, <, <=, ==, !=
  - AI-friendly JSON error messages
- ✅ **Sorting** (`--sort-by` flag: cpu, mem, pid, name)
- ✅ **Top-N limiting** (`--top-n` flag)
- ✅ **Unit tests** (8 tests, all passing)
- ✅ **Zero clippy warnings**
- ✅ Proper error handling with thiserror
- ✅ Type-safe filter module with comprehensive validation

## What Doesn't Work Yet

- ❌ `--watch` flag (parsed but not implemented)
- ❌ CSV output mode
- ❌ Multiple filter conditions (AND/OR logic)
- ❌ Disk I/O metrics
- ❌ Network metrics
- ❌ Integration tests

## Recent Changes

**Phase 1 core features (2025-11-04)**:
- ✅ Implemented filter module with type-safe parsing and evaluation
- ✅ Added thiserror for proper error handling
- ✅ Implemented sort-by functionality (cpu, mem, pid, name)
- ✅ Implemented top-n limiting (works for both JSON and human output)
- ✅ Added 8 unit tests covering filter parsing edge cases
- ✅ AI-friendly JSON error messages for invalid filters
- ✅ Zero clippy warnings, all tests passing

**Initial implementation (bd0d51c)**:
- Created basic MVP with sysinfo 0.37
- System metrics: CPU usage, memory total/used/percent
- Process info: PID, name, CPU%, memory%, user, command
- Both JSON and human-readable output modes
- CLI scaffolding with clap 4.5

## Next Steps

Remaining for Phase 1 completion (v0.1.0):
1. Integration tests (CLI end-to-end)
2. CSV output mode
3. Human-readable output improvements (colors, better formatting)
4. Documentation updates (README examples, filter syntax docs)

## Known Issues

- User field shows UID numbers on macOS (e.g., "501") instead of usernames - sysinfo limitation
- Process commands may be empty for kernel threads (expected)
- 200ms sleep in collect_snapshot for accurate CPU readings (acceptable)

## Performance

- Collection time: ~200ms (includes mandatory sleep for CPU accuracy)
- Memory usage: Minimal (processes collected once)
- Output size: ~500 bytes per process in JSON

## Development Environment

- Rust: 1.91.0 (edition 2024)
- Key dependencies: sysinfo 0.37, clap 4.5, serde 1.0, chrono 0.4
- Platforms tested: macOS (M3 Max), Fedora (x86_64)
