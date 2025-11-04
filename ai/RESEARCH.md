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

## Open Questions
- [ ] Watch mode CPU overhead testing (1s vs 2s refresh rate)
- [ ] CSV vs TSV for command field escaping
