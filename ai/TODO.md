# TODO

## v0.0.1 Status: Core Features Complete

### ✅ Completed (Phases 1, 2, 4)

**Core Functionality**:
- [x] Filter expressions (simple + compound AND/OR logic)
- [x] Sort by cpu, mem, pid, name
- [x] Top-N limiting (default 20)
- [x] Watch mode with configurable interval
- [x] JSON, CSV (RFC 4180), human-readable output
- [x] Color-coded terminal output

**Testing**:
- [x] 29 tests (16 unit + 13 integration)
- [x] Zero clippy warnings
- [x] Cross-platform tested (macOS, Linux)
- [x] Performance validated (29ms overhead)

**Documentation**:
- [x] README with examples and filter syntax
- [x] ai/ context files (PLAN, STATUS, DECISIONS, RESEARCH)
- [x] Research docs (filter syntax, watch mode, compound filters, benchmarks)

## Current Priority: Validation & Traction

### Real-World Usage Testing
- [ ] Use stop for actual tasks this week (see ai/research/real-world-usage.md)
- [ ] Document what works well vs. what's missing
- [ ] Identify killer features or lack thereof
- [ ] **Decision**: Proceed to v0.1.0, pivot, or shelve based on utility

### Documentation for Traction
- [ ] Add usage examples to README (more practical scenarios)
- [ ] Create "Why stop?" section with specific use cases
- [ ] Add comparison with ps/top/htop (when to use each)
- [ ] Consider adding animated GIFs/screenshots to README
- [ ] Write blog post or announcement (if proven useful)

### Potential Improvements (Only if Validated as Useful)
- [ ] Fix deprecated test warnings (Command::cargo_bin → cargo_bin_cmd!)
- [ ] Shell completion scripts (bash, zsh, fish)
- [ ] Better help text with examples (--help output)
- [ ] Man page generation
- [ ] GitHub Actions CI (test on Linux/macOS)

## Phase 3: Advanced Monitoring (Future - If Justified)

**Only proceed if tool proves useful in current form.**

- [ ] Disk I/O metrics per process
- [ ] Network metrics per process
- [ ] Thread information
- [ ] Open file descriptors/handles
- [ ] Windows support testing

## Phase 5: Production (Future - If Widely Used)

- [ ] Publish to crates.io
- [ ] Set up GitHub Actions CI
- [ ] Version 0.1.0 release (lock JSON schema)
- [ ] Stable API documentation
- [ ] Consider homebrew formula (macOS)
- [ ] Consider packaging for Linux distros

## Non-Goals (Out of Scope)

- Real-time alerting system (users can build on top)
- Historical data storage (not a metrics DB)
- Process control/killing (security implications)
- Container-specific metrics (scope creep)
- Plugin system (premature complexity)
- Interactive TUI (conflicts with AI-first goal)

## Notes

- **Staying in 0.0.x until real-world validation**
- Focus on proving utility before adding features
- Good documentation > more features right now
- Don't optimize for hypothetical use cases
