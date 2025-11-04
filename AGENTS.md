# stop - AI Agent Instructions

> Structured process and system monitoring with JSON output

**Reference**: github.com/nijaru/agent-contexts

## Project Overview

**stop** (structured top) is a modern system monitoring tool designed for AI agents and automation. Provides structured JSON output instead of human-readable text formatting.

**Status**: v0.0.1 - Core features complete, Phases 1-2-4 done, tested on macOS and Linux

**Tech Stack**: Rust 2024, sysinfo 0.37, clap 4.5, serde, thiserror, owo-colors, crossterm

## Project Structure

- Documentation: README.md, docs/
- AI working context: ai/
  - PLAN.md — Strategic roadmap (Phase 1 → v1.0.0)
  - STATUS.md — Current state (read first)
  - TODO.md — Next steps
  - DECISIONS.md — Architectural choices
  - RESEARCH.md — Research notes
  - research/ — Detailed research documents
- Source: src/ (main.rs, filter.rs, watch.rs)
- Tests: tests/ (29 tests: 16 unit + 13 integration)
- Dependencies: Cargo.toml (sysinfo 0.37, clap 4.5, serde, chrono, thiserror, owo-colors, crossterm)

## Current Implementation

**Working features (v0.0.1):**
- System metrics (CPU, memory)
- Process list (PID, name, CPU%, memory%, user, command)
- **Filtering**: Simple and compound expressions (AND/OR logic, case-insensitive keywords)
- **Sorting**: By cpu, mem, pid, name
- **Limiting**: Top-N processes (default 20)
- **Watch mode**: Continuous monitoring with configurable interval
- **Output formats**: JSON, CSV (RFC 4180), human-readable with colors
- **Performance**: 29ms overhead (< 100ms goal)
- **Cross-platform**: Tested on macOS and Linux (Fedora)

**Code structure:**
- `src/main.rs` - CLI, data collection, output formatting
  - `SystemSnapshot`, `SystemMetrics`, `ProcessInfo` structs
  - `collect_snapshot()` - Uses sysinfo to gather data
  - `sort_processes()`, `output_*()` functions
- `src/filter.rs` - Type-safe filter parsing and evaluation
  - `FilterExpr` enum (Simple/And/Or)
  - `Filter`, `FilterField`, `FilterOp`, `FilterValue` types
  - Parse-time validation with `thiserror` errors
- `src/watch.rs` - Continuous monitoring mode
  - Screen clearing for human output
  - NDJSON streaming for JSON output
- `tests/integration_test.rs` - 13 CLI integration tests

## Technology Stack

- **Language**: Rust (edition 2024)
- **Package Manager**: cargo
- **Core Dependencies**:
  - sysinfo 0.37 — Cross-platform system metrics
  - clap 4.5 — CLI parsing (derive API)
  - serde 1.0 + serde_json — JSON serialization
  - chrono 0.4 — Timestamps (RFC3339)
  - thiserror 2.0 — Structured error handling
  - owo-colors 4.1 — Terminal colors
  - crossterm 0.28 — Terminal control (watch mode)

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

**v0.0.1** - Staying in 0.0.x for real-world validation

**Completed (Phases 1, 2, 4)**:
- ✅ All core CLI flags (filter, sort, top-n, watch, CSV, JSON)
- ✅ Compound filter expressions (AND/OR logic)
- ✅ 29 tests (16 unit + 13 integration), zero clippy warnings
- ✅ Performance validated (29ms overhead)
- ✅ Cross-platform tested (macOS, Linux)

**Current Priority**: Real-world usage validation
- Use tool for actual tasks to prove utility
- Gather feedback on what's useful vs. what's missing
- Don't add Phase 3 features without proven use case

**Decision Point**:
- If proven useful → Consider v0.1.0, publish to crates.io
- If unclear utility → Find specific niche or shelve
- See ai/research/real-world-usage.md for testing framework

For full roadmap: See ai/PLAN.md and ai/STATUS.md

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
