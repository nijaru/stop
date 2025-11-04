# TODO

## Phase 1: MVP → v0.1.0

### Core Functionality ✅ COMPLETE
- [x] Implement `--filter` flag
  - [x] Parse filter expressions (e.g., "cpu > 10", "mem > 50")
  - [x] Apply filters to process list
  - [x] Error handling for invalid expressions
  - [ ] Support multiple conditions (AND logic) - deferred to Phase 2
- [x] Implement `--sort-by` flag
  - [x] Support: cpu, mem, pid, name
  - [x] Default: cpu descending
  - [x] Add tests for each sort option
- [x] Implement `--top-n` flag
  - [x] Limit output to N processes
  - [x] Default: 20
  - [x] Verify with sorting and filtering

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
- [x] Unit tests
  - [x] Filter parsing and evaluation (8 tests)
  - [x] Sort comparison functions
  - [x] Output formatting
- [ ] Integration tests
  - [ ] CLI argument parsing end-to-end
  - [ ] JSON output structure validation
  - [ ] CSV output format validation
  - [ ] Error cases (invalid args)
- [ ] Property-based tests (optional, may defer)
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
