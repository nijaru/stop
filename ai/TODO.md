# TODO

## Current Priority: Fix Critical Issues

### 🔴 Critical (Blocking)

From code review (2025-11-20):

- [ ] Fix UTF-8 string slicing panic (`src/main.rs:533, 542, 548`)
  - Uses byte index instead of char boundaries
  - Will panic on non-ASCII process names (emoji, Chinese, etc.)
  - Fix: Use `char_indices()` or safe `.get()`

- [ ] Fix division by zero (`src/main.rs:205, 229`)
  - No check for `total_memory == 0`
  - Creates `Infinity` in JSON output
  - Fix: Guard division with zero check

- [ ] Fix interval validation (`src/main.rs:500`)
  - Doesn't check NaN or Infinity
  - Can cause infinite loops
  - Fix: Use `is_finite()` check

- [ ] Add UTF-8 edge case tests
  - Process names with emoji/Chinese/combining chars
  - Truncation edge cases

### 🟡 High Priority (Soon)

- [ ] Optimize process command allocation (`src/main.rs:214-218`)
- [ ] Optimize CSV escaping to only call replace() when needed
- [ ] Cache lowercased search term for watch mode
- [ ] Add NaN handling in sort comparisons

### 🟠 Medium Priority (Polish)

- [ ] Extract shared filtering code (watch.rs duplicates main.rs)
- [ ] Add constants for magic table widths
- [ ] Add CSV format documentation to README
- [ ] Flush CSV writer on watch mode exit

### 🔵 Low Priority (Nice to Have)

- [ ] Reuse System object in watch mode
- [ ] Validate top_n range (0 invalid, very large warn)
- [ ] Add error context hints

## Completed This Session (2025-11-20)

- [x] Fixed `is_none_or` for stable Rust compatibility
- [x] Added BufWriter for CSV output performance
- [x] Optimized CSV escaping with `bytes().any()`
- [x] Pre-allocated process vector capacity
- [x] Extracted byte size constants to module level
- [x] Added `#[must_use]` attributes
- [x] Created custom StopError type
- [x] Added interval validation (partial - needs NaN fix)
- [x] Added module-level documentation
- [x] Improved filter error messages with suggestions
- [x] Added doc comment examples
- [x] Added 7 new integration tests (watch, verbose, search)
- [x] Comprehensive code review (found 18 issues)

## Previous Completions (Reference)

**v0.0.1-alpha Release**:
- Published to crates.io as `stop-cli`
- 59 tests passing (16 unit + 24 integration + 19 edge case)
- CI/CD pipeline (GitHub Actions)

## Non-Goals (Out of Scope)

- Real-time alerting (users build on top)
- Historical storage (not a metrics DB)
- Process control/killing (security)
- Container-specific metrics
- Interactive TUI
