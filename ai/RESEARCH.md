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

## Open Questions
- [ ] CSV vs TSV for command field escaping
