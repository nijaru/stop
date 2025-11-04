# TODO

## Phase 1: MVP → v0.1.0

### Core Functionality
- [ ] Implement `--filter` flag
  - [ ] Parse filter expressions (e.g., "cpu > 10", "mem > 50")
  - [ ] Apply filters to process list
  - [ ] Support multiple conditions (AND logic)
  - [ ] Error handling for invalid expressions
- [ ] Implement `--sort-by` flag
  - [ ] Support: cpu, mem, pid, name
  - [ ] Default: cpu descending
  - [ ] Add tests for each sort option
- [ ] Implement `--top-n` flag
  - [ ] Limit output to N processes
  - [ ] Default: 20 (already partially working)
  - [ ] Verify with sorting

### Output Improvements
- [ ] Human-readable table
  - [ ] Better column alignment
  - [ ] Show command path truncation indicator (...)
  - [ ] Add color coding (high CPU = red, etc.)
  - [ ] Header with system summary
- [ ] CSV output mode
  - [ ] Add `--csv` flag
  - [ ] Proper escaping for command fields
  - [ ] Header row with column names

### Testing
- [ ] Unit tests
  - [ ] Filter parsing and evaluation
  - [ ] Sort comparison functions
  - [ ] Output formatting
- [ ] Integration tests
  - [ ] CLI argument parsing
  - [ ] JSON output structure validation
  - [ ] CSV output format validation
  - [ ] Error cases (invalid args)
- [ ] Property-based tests (optional)
  - [ ] Filter expressions always return subset
  - [ ] Sort order is stable

### Documentation
- [ ] Add inline docs for public functions
- [ ] Update README with implemented features
- [ ] Add CONTRIBUTING.md
- [ ] Document filter expression syntax

### Release Prep
- [ ] Run clippy and fix all warnings
- [ ] Run rustfmt
- [ ] Bump version to 0.1.0
- [ ] Update CHANGELOG
- [ ] Tag release

## Phase 2: Query & Filter (v0.2.0)

Deferred to after Phase 1:
- [ ] Process name/user filtering
- [ ] Multiple filter conditions (OR logic)
- [ ] Regular expression matching
- [ ] CSV output

## Phase 3: Advanced Monitoring (v0.3.0)

Future work:
- [ ] Disk I/O metrics
- [ ] Network metrics
- [ ] Thread information
- [ ] Open files/connections

## Notes

- Focus on Phase 1 completion before moving to Phase 2
- Keep commits small and focused on single features
- Test as you implement, don't batch testing at the end
- Update STATUS.md after completing each major item
