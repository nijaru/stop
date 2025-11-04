# Status

## Current State

**Version**: v0.0.1
**Phase**: 1 (MVP → v0.1.0)
**Commit**: bd0d51c (initial commit)

## What Works

- ✅ Basic process listing with system metrics
- ✅ JSON output (`--json` flag)
- ✅ Human-readable table output (default, top 20 by CPU)
- ✅ CLI argument parsing (clap)
- ✅ Cross-platform data collection (sysinfo)
- ✅ Compiles with zero warnings
- ✅ Installed on both Mac and Fedora

## What Doesn't Work Yet

- ❌ `--filter` flag (parsed but not implemented)
- ❌ `--sort-by` flag (parsed but not implemented)
- ❌ `--top-n` flag (parsed but not implemented)
- ❌ `--watch` flag (parsed but not implemented)
- ❌ CSV output mode
- ❌ Tests (no test suite yet)
- ❌ Disk I/O metrics
- ❌ Network metrics

## Recent Changes

**Initial implementation (bd0d51c)**:
- Created basic MVP with sysinfo 0.37
- System metrics: CPU usage, memory total/used/percent
- Process info: PID, name, CPU%, memory%, user, command
- Both JSON and human-readable output modes
- CLI scaffolding with clap 4.5

## Next Steps

See `TODO.md` for prioritized task list. Focus on Phase 1 completion:
1. Implement filtering logic
2. Implement sorting options
3. Implement top-N limiting
4. Add comprehensive tests
5. Improve human-readable output formatting

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
