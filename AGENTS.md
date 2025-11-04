# stop - AI Agent Instructions

> Structured process and system monitoring with JSON output

**Reference**: github.com/nijaru/agent-contexts

## Project Overview

**stop** (structured top) is a modern system monitoring tool designed for AI agents and automation. Provides structured JSON output instead of human-readable text formatting.

**Status**: Early development (v0.0.1) - MVP implemented, Phase 1 in progress

**Tech Stack**: Rust, sysinfo crate, clap CLI, serde JSON

## Project Structure

- Documentation: README.md, docs/
- AI working context: ai/
  - PLAN.md — Strategic roadmap (Phase 1 → v1.0.0)
  - STATUS.md — Current state (read first)
  - TODO.md — Next steps
  - DECISIONS.md — Architectural choices
  - RESEARCH.md — Research notes
  - research/ — Detailed research documents
- Source: src/main.rs (MVP - single file)
- Dependencies: Cargo.toml (sysinfo 0.37, clap 4.5, serde, chrono)

## Current Implementation

**Working features (v0.0.1):**
- System metrics (CPU, memory)
- Process list (PID, name, CPU%, memory%, user, command)
- JSON output (`--json` flag)
- Human-readable table output (default)
- CLI args parsed (filter, sort-by, top-n, watch) - not implemented yet

**Code structure:**
- `SystemSnapshot` - Top-level struct (serializable to JSON)
- `SystemMetrics` - CPU, memory totals
- `ProcessInfo` - Per-process data (PID, name, CPU%, mem%, user, cmd)
- `collect_snapshot()` - Uses sysinfo to gather data
- `main()` - CLI parsing and output formatting

## Technology Stack

- **Language**: Rust (edition 2024)
- **Package Manager**: cargo
- **Core Dependencies**:
  - sysinfo 0.37 — Cross-platform system metrics
  - clap 4.5 — CLI parsing (derive API)
  - serde 1.0 + serde_json — JSON serialization
  - chrono 0.4 — Timestamps (RFC3339)

## Development Commands

```bash
# Build
cargo build
cargo build --release

# Test
cargo test                    # Run test suite (when implemented)
cargo clippy                  # Lint checks
cargo fmt                     # Format code

# Run
cargo run -- --json          # JSON output
cargo run -- --top-n 5       # Human-readable, top 5
cargo run                    # Default output (top 20 by CPU)

# Install
cargo install --path .       # Install locally
```

## Development Workflow

1. Read: `ai/PLAN.md` → `ai/STATUS.md` → `ai/TODO.md` → `ai/DECISIONS.md`
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

## Current Focus

**Phase 1: MVP → v0.1.0** (January 2025)

See ai/TODO.md for detailed task list. Priority:
1. Implement `--filter` flag (expressions like "cpu > 10")
2. Implement `--sort-by` flag (cpu, mem, pid, name)
3. Implement `--top-n` flag (limit output)
4. Add comprehensive test suite
5. Improve human-readable output

For full roadmap (v0.1.0 → v1.0.0): See ai/PLAN.md

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
