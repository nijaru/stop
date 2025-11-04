# Research

<!-- Template:

## Topic (YYYY-MM-DD)
**Sources**: [links, books, docs]
**Key Finding**: [main takeaway]
**Decision**: [action]
→ Details: ai/research/topic.md

## Open Questions
- [ ] Question needing research
-->

## Filter Syntax Design (2025-01-04)
**Sources**: jq manual, Process Monitor, CLI tools survey
**Key Finding**: Simple `field op value` syntax best for AI agents - predictable, easy to generate programmatically
**Decision**: Hand-rolled parser (simple regex split) - no need for pest/nom complexity
→ Details: ai/research/filter-syntax.md

## Watch Mode Implementation (2025-11-04)
**Sources**: Real-Time System Monitor in Rust Terminal, watchexec patterns
**Key Finding**: 2s default interval provides best balance of responsiveness and CPU efficiency
**Decision**: NDJSON for JSON watch mode, screen clearing for human-readable
→ Details: ai/research/watch-mode.md

## Compound Filter Expressions (2025-11-04)
**Sources**: SQL syntax, boolean algebra, CLI tool patterns
**Key Finding**: SQL-style AND/OR keywords are most readable and AI-friendly
**Decision**: Case-insensitive keywords with standard precedence (AND before OR)
→ Details: ai/research/compound-filters.md

## Performance Benchmarking (2025-11-04)
**Sources**: hyperfine, Rust performance best practices
**Key Finding**: 29ms overhead on ~500 processes (87% is mandatory sleep, only 13% actual work)
**Decision**: Current performance meets goals, no optimization needed
→ Details: ai/research/performance-benchmarks.md

## Cross-Platform Validation (2025-11-04)
**Sources**: Fedora testing, sysinfo docs
**Key Finding**: Identical functionality on macOS and Linux, user field shows UIDs on both platforms
**Decision**: Document UID limitation, don't block on it (sysinfo constraint)
→ Details: ai/research/fedora-results.md

## Resolved Questions
- [x] CSV vs TSV for command field escaping → RFC 4180 CSV with proper quoting
