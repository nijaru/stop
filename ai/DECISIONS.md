# Decisions

## Architectural Choices

### Language & Runtime
**Rust** - Chosen for:
- Cross-platform system APIs
- Memory safety without GC pauses
- Excellent CLI ecosystem (clap)
- Strong serde integration for JSON
- Similar to sy (keeps tooling consistent)

### Core Dependencies

**sysinfo 0.37** - System monitoring
- Rationale: Cross-platform, actively maintained, comprehensive API
- Alternatives considered: procfs (Linux-only), libc (too low-level)
- Trade-off: UID instead of username on some platforms (acceptable)

**clap 4.5** - CLI parsing
- Rationale: Derive API is clean, great error messages, widely used
- Alternatives: structopt (deprecated), argh (less features)

**serde + serde_json** - Serialization
- Rationale: Standard Rust JSON library, zero-copy where possible
- No alternatives seriously considered

**chrono 0.4** - Timestamps
- Rationale: RFC3339 format, timezone support
- Alternative: std::time (insufficient formatting)

### Design Patterns

**Single snapshot model** - Collect all data once, then process
- Rationale: Simpler than streaming, sufficient for most use cases
- Trade-off: Higher initial memory for large process lists (acceptable up to ~10k processes)
- Future: Could add streaming for watch mode

**CLI-first design** - No library/API separation yet
- Rationale: YAGNI - focus on tool usefulness first
- Future: Could extract core logic to library crate if needed

**JSON as primary output** - AI/automation first, humans second
- Rationale: Project goal is AI-friendly tooling
- Human-readable mode is secondary (but still good UX)

### Output Format

**NDJSON for future streaming** - One JSON object per line
- Rationale: Enables piping to jq, grep, etc.
- Current: Single JSON object (simpler for v0.1)
- Future: Multiple snapshots in watch mode = NDJSON

**Process sorting default: CPU descending**
- Rationale: Most common use case (find high CPU processes)
- User can override with `--sort-by`

## Rejected Alternatives

**Go** - Easier deployment (single binary)
- Rejected: Rust has similar deployment story, better performance, consistent with sy

**Python with psutil** - Faster development
- Rejected: Slower runtime, requires Python install, not self-contained

**C/C++** - Maximum performance
- Rejected: Development velocity too low, memory safety issues

**Terminal UI (like htop)** - More human-friendly
- Rejected: Conflicts with AI-first goal, adds complexity

## Filter Implementation Decisions (2025-11-04)

**Filter expression syntax** - Chose option #1: Simple `field op value`
- **Decision**: Implemented `cpu > 10` style syntax
- **Rationale**:
  - Easiest for AI agents to construct programmatically
  - Clear error messages possible
  - Extensible to AND/OR in Phase 2 without breaking changes
  - jq-like comparison operators (familiar pattern)
- **Implementation**: Type-safe enums + hand-rolled parser (no pest/nom needed)
- **Validation**: Comprehensive type checking at parse time, not eval time

**Error handling approach** - thiserror crate
- **Decision**: Use thiserror for structured error types
- **Rationale**:
  - SOTA Rust error handling pattern
  - Clear error variants (UnknownField, TypeMismatch, InvalidValue)
  - AI-friendly: JSON serializable error messages
- **Example**: `{"error": "FilterError", "message": "...", "expression": "..."}`

**String matching semantics**
- **Decision**:
  - `name == chrome` → case-insensitive contains (matches "Chrome", "chromium")
  - `user == root` → exact match (case-sensitive)
- **Rationale**: Name matching is fuzzy (user intent), user is precise (security)
- **Trade-off**: Different semantics for different fields (documented in research/)

## Open Questions

**Watch mode refresh rate** - Default TBD
- Options: 1s (like top), 2s, user-configurable
- Need to test CPU overhead before deciding

**CSV field escaping** - How to handle commands with commas/quotes?
- Standard RFC 4180 (quote fields, escape quotes with double-quotes)
- Alternative: Use TSV instead (simpler, no escaping needed)

## Migration Notes

No breaking changes yet (v0.0.x series).

When bumping to v0.1.0:
- Lock JSON schema format
- Document stability guarantees
- Any CLI changes after this require deprecation warnings
