# Development Context

> Context from initial development session (2025-01-04)

## Why stop Was Created

**Problem**: Traditional system monitoring tools (top, htop, ps) output human-readable text that's difficult for AI agents to parse. Modern automation and AI-driven workflows need structured, machine-readable data.

**Solution**: stop provides JSON-first output with consistent cross-platform format.

## Origin Story

During a review of modern CLI tools for AI agents, we identified a gap:
- Code search: `rg` (structured, fast)
- File finding: `fd` (simple, predictable)
- JSON processing: `jq` (proper manipulation)
- **System monitoring**: ❌ No structured alternative to top/ps

The name "stop" follows the pattern: **s**tructured **top**.

## Design Philosophy

1. **AI-first, humans second**: JSON is the primary output format
2. **Structured data, not pretty text**: No colors/formatting in machine output
3. **Cross-platform consistency**: Same JSON schema on macOS, Linux, Windows
4. **Part of modern CLI ecosystem**: Listed alongside rg, fd, jq, yq in global tooling recommendations

## Technical Decisions Made

### Tech Stack
- **Rust**: Cross-platform, memory safe, consistent with existing tools (sy, ast-grep, sd)
- **sysinfo 0.37**: Latest stable, cross-platform system APIs
- **clap 4.5**: Modern CLI parsing with derive API
- **edition = "2024"**: Latest stable Rust edition (following version selection guidelines)

### Architecture
- Single-snapshot model (collect once, process)
- No streaming yet (YAGNI principle)
- CLI-first (no library separation yet)
- Simple filter syntax planned: `cpu > 10` (extensible later)

### Repository Setup
- GitHub: github.com/nijaru/stop
- Installed on both Mac and Fedora
- Added to Modern CLI Tools list in global CLAUDE.md
- Marked as "experimental" (by us, not well-tested yet)

## Current State (v0.0.1)

**Working**:
- System metrics: CPU usage, memory total/used/percent
- Process list: PID, name, CPU%, memory%, user, command
- JSON output: `stop --json`
- Human-readable table: `stop` (default, top 20 by CPU)

**Not implemented**:
- CLI flags parsed but not functional: `--filter`, `--sort-by`, `--top-n`, `--watch`
- CSV output mode
- Tests
- Disk I/O metrics
- Network metrics

**Known issues**:
- User field shows UID on macOS (e.g., "501") - sysinfo limitation
- 200ms collection time (includes sleep for accurate CPU readings)

## Development Context

### Version Selection
- Always use latest stable: Check `cargo search <pkg>` before suggesting versions
- Use ranges: `serde = "1.0"` not `serde = "1.0.150"`
- Rust edition: Use "2024" not "2021" for new projects
- This was a problem initially - suggested sysinfo 0.33 (outdated), corrected to 0.37

### Related Tools
- **sy**: Modern rsync (2-11x faster, JSON output, parallel) - by us, similar tech stack
- **rg/fd/ast-grep/sd**: Rust CLI tools with structured output
- **stop**: Fills the "structured system monitoring" gap

### Integration
- Added to `~/.claude/CLAUDE.md` Modern CLI Tools table
- Installed via `cargo install --path .` on both machines
- Will be updated by `up` command (via cargo-update)

## Next Steps (Phase 1 → v0.1.0)

Priority order:
1. Implement `--filter` flag (parse expressions like "cpu > 10")
2. Implement `--sort-by` flag (cpu, mem, pid, name)
3. Implement `--top-n` flag (limit output)
4. Add test suite (unit + integration)
5. Improve human-readable output (colors, formatting)
6. CSV output mode

See `ai/TODO.md` for detailed checklist.

## How to Proceed

**Starting fresh in Claude Code:**
1. Read: AGENTS.md → ai/STATUS.md → ai/TODO.md → ai/DECISIONS.md
2. Pick a task from TODO.md (recommend: start with `--filter` implementation)
3. Implement, test as you go: `cargo run -- --json`
4. Update ai/STATUS.md with progress
5. Commit frequently with descriptive messages (no AI attribution)

**Testing approach:**
```bash
# Basic testing
cargo run -- --json | jq '.system.cpu_usage'
cargo run -- --top-n 5
cargo run -- --filter "cpu > 10"  # After implementing

# Build and install
cargo build --release
cargo install --path .
```

**Code location:**
- Main logic: `src/main.rs` (single file MVP)
- Will need refactoring as features grow (suggest: `src/filter.rs`, `src/output.rs`, etc.)

## Important Reminders

- NO AI attribution in commits/PRs
- Use latest stable versions (check crates.io)
- Test before committing
- Update ai/STATUS.md after each session
- Follow existing code style (rustfmt, no clippy warnings)
- Cross-platform: Test on both Mac and Linux when possible

## References

- Repository: https://github.com/nijaru/stop
- Similar project (for inspiration): sy (github.com/nijaru/sy)
- Agent contexts: github.com/nijaru/agent-contexts
- Modern CLI tools guide: ~/.claude/CLAUDE.md
