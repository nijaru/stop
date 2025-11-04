# stop - AI Agent Instructions

> Structured process and system monitoring with JSON output

**Reference**: github.com/nijaru/agent-contexts

## Project Overview

**stop** (structured top) is a modern system monitoring tool designed for AI agents and automation. Provides structured JSON output instead of human-readable text formatting.

**Status**: Early development (v0.0.1) - MVP implemented, Phase 1 in progress

**Tech Stack**: Rust, sysinfo crate, clap CLI, serde JSON

## Architecture

```
stop/
├── src/
│   └── main.rs          # MVP implementation (process list, system metrics, JSON output)
├── Cargo.toml           # Dependencies: sysinfo 0.37, clap 4.5, serde, chrono
├── README.md            # User-facing docs with roadmap
└── ai/                  # Agent working context
    ├── STATUS.md        # Current state
    ├── TODO.md          # Active tasks
    └── DECISIONS.md     # Architectural choices
```

## Current Implementation

**Working features (v0.0.1):**
- System metrics (CPU, memory)
- Process list (PID, name, CPU%, memory%, user, command)
- JSON output (`--json` flag)
- Human-readable table output (default)
- CLI args parsed (filter, sort-by, top-n, watch) - not implemented yet

**Code structure:**
- `SystemSnapshot` - Top-level struct
- `SystemMetrics` - CPU, memory totals
- `ProcessInfo` - Per-process data
- `collect_snapshot()` - Uses sysinfo to gather data
- `main()` - CLI parsing and output formatting

## Development Workflow

1. Read: `ai/STATUS.md` → `ai/TODO.md` → `ai/DECISIONS.md`
2. Implement features from Phase 1 roadmap
3. Update `ai/STATUS.md` with progress
4. Test as you go: `cargo test`, `cargo run`
5. Commit frequently with descriptive messages

## Testing

```bash
# Build and run
cargo build
cargo run -- --json

# Test basic functionality
cargo run -- --json | jq '.system.cpu_usage'
cargo run -- --top-n 5

# Install locally
cargo install --path .
```

## Phase 1 Goals (v0.1.0)

See `ai/TODO.md` for current task list. Focus:
- [ ] Implement filtering (`--filter` flag)
- [ ] Implement sorting (`--sort-by` flag)
- [ ] Add tests (unit + integration)
- [ ] Human-readable table improvements
- [ ] CSV output mode

## Key Design Principles

1. **AI-first**: JSON output is primary, human output is secondary
2. **Cross-platform**: macOS and Linux support (Windows nice-to-have)
3. **Fast**: Minimal overhead, efficient data collection
4. **Simple**: Easy to use, clear output format
5. **Production-ready**: Error handling, logging, validation

## Important Notes

- Use latest stable Rust (edition = "2024")
- Follow existing code style (rustfmt, no clippy warnings)
- Update README.md for user-facing changes
- Update ai/STATUS.md for agent context
- NO AI attribution in commits
- Test before committing
