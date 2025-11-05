# TODO

## v0.0.1-alpha Status: Released & Field Testing

### ✅ Completed (All Core Phases)

**Release**:
- [x] Published to crates.io as `stop-cli`
- [x] GitHub release v0.0.1-alpha (prerelease)
- [x] Git tag created
- [x] CI/CD pipeline (GitHub Actions on Ubuntu and macOS)

**Core Functionality**:
- [x] Filter expressions (simple + compound AND/OR logic)
- [x] Sort by cpu, mem, pid, name
- [x] Top-N limiting (default 20)
- [x] Watch mode with configurable interval
- [x] JSON, CSV (RFC 4180), human-readable output
- [x] Color-coded terminal output
- [x] Advanced metrics: threads, disk I/O, open files

**Testing**:
- [x] 52 tests (16 unit + 17 integration + 19 edge case)
- [x] Zero clippy warnings
- [x] CI passing on Ubuntu and macOS
- [x] Cross-platform tested (macOS, Linux)
- [x] Performance validated (<30ms overhead)
- [x] Broken pipe handling

**Documentation**:
- [x] README with examples and filter syntax
- [x] Repositioned as general-purpose tool (not AI-first)
- [x] Installation instructions (crates.io)
- [x] ai/ context files (PLAN, STATUS, DECISIONS, RESEARCH)

## Current Priority: Field Testing

**Monitor**:
- [ ] User feedback on GitHub issues
- [ ] Bug reports
- [ ] Feature requests
- [ ] Real-world usage patterns

**Track**:
- Downloads from crates.io
- GitHub stars/watchers
- Issue velocity

## Post-Field Testing (v0.1.0)

**Before Stable Release**:
- [ ] Address critical bugs (if any)
- [ ] Implement highly-requested features (if any)
- [ ] Windows testing and validation
- [ ] Version bump to 0.1.0
- [ ] Lock JSON schema format

## Potential Improvements (Low Priority)

**Only if requested by users**:
- [ ] Additional output formats (YAML, TSV, plain text)
- [ ] Parentheses for complex filter grouping
- [ ] Shell completion scripts (bash, zsh, fish)
- [ ] Better help text with examples
- [ ] Man page generation
- [ ] Homebrew formula (macOS)
- [ ] Linux distro packaging

**Documentation**:
- [ ] More practical examples (if users request)
- [ ] Animated GIFs/screenshots (if needed for traction)
- [ ] Blog post or announcement (if proven useful)

## Non-Goals (Out of Scope)

- Real-time alerting system (users can build on top)
- Historical data storage (not a metrics DB)
- Process control/killing (security implications)
- Container-specific metrics (scope creep)
- Plugin system (premature complexity)
- Interactive TUI (conflicts with design goals)

## Notes

- **Currently in alpha**: v0.0.1-alpha released for field testing
- **No version bump until feedback**: Staying in alpha until validated
- **User-driven development**: Let real-world usage guide next features
- **Focus on stability**: Fix bugs before adding features
