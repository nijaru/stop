# Comprehensive Testing & Optimization Plan (2025-11-04)

## Goal
Thoroughly test, profile, and optimize stop before claiming production-ready status.

## Testing Strategy

### 1. Edge Case Testing

**Filter Parsing:**
- Empty expressions
- Special characters in strings (quotes, newlines, tabs)
- Very long expressions (>1000 chars)
- Malformed expressions
- Unicode in process names/commands
- Type coercion edge cases (name that looks like number)
- AND/OR precedence with many clauses

**CSV Output:**
- Process names with quotes, commas, newlines
- Commands with all CSV special chars
- Very long command lines (>10KB)
- Unicode characters
- Empty fields

**JSON Output:**
- Non-UTF8 process names
- Control characters
- Very large snapshots (>10MB)

**Error Conditions:**
- Process disappears during collection
- Permission denied (non-root on protected processes)
- sysinfo refresh failures
- Disk full (for CSV/JSON output)
- Invalid file descriptors

### 2. Performance Testing

**Baseline Metrics:**
- Time to collect snapshot with varying process counts (10, 100, 1000, 5000)
- Memory usage for same
- Impact of open_files() on collection time
- Filter parsing speed with complex expressions
- CSV vs JSON output performance

**Watch Mode:**
- Stability over 1 hour with 0.1s interval
- Memory growth over time (leak detection)
- CPU usage patterns
- Impact on system (are we causing load?)

**Scale Testing:**
- 10,000+ processes (stress test)
- Very large process trees
- Processes with 1000+ threads
- Processes with 10,000+ open files

### 3. Profiling Plan

**Tools:**
- `cargo flamegraph` - CPU profiling
- `cargo bloat` - Binary size analysis
- `valgrind` (Linux) - Memory profiling
- `/usr/bin/time -l` (macOS) - Memory/time
- `hyperfine` - Benchmarking

**Scenarios to Profile:**
1. Single snapshot collection (baseline)
2. Watch mode (10s with 0.1s interval)
3. Filter parsing (complex AND/OR)
4. CSV output (1000 processes)
5. open_files() vs without

## Known Optimization Opportunities

### Potential Bottlenecks:
1. `open_files()` - "computed every time" per docs (expensive?)
2. CSV field escaping - allocates string per field
3. Watch mode - no object pooling, allocates fresh every cycle
4. Filter evaluation - no short-circuit optimization?
5. No lazy evaluation of metrics (collect everything even if filtered)

### Optimization Ideas:
1. Make open_files optional flag (skip if not needed)
2. Reuse allocations in watch mode (object pool)
3. Short-circuit filter evaluation (AND can bail early)
4. Lazy metric collection (only collect what's needed for filter/output)
5. Zero-copy string handling where possible
6. Parallel process collection (rayon?)

## Test Implementation Order

1. **Setup profiling tools** (flamegraph, hyperfine)
2. **Baseline performance metrics** (document current state)
3. **Profile single snapshot** (find bottlenecks)
4. **Profile watch mode** (memory/CPU patterns)
5. **Edge case testing** (create test suite)
6. **Optimize based on findings** (data-driven)
7. **Re-profile to verify improvements**
8. **Document performance characteristics**

## Success Criteria

**Performance:**
- Single snapshot: <50ms for 1000 processes
- Watch mode: <100ms per cycle, <1MB memory growth/hour
- Filter parsing: <1ms for complex expressions
- CSV output: <10ms additional overhead

**Reliability:**
- Zero panics on edge cases
- Graceful error handling (no unwrap() in prod paths)
- No memory leaks in watch mode
- Handles process churn gracefully

**Documentation:**
- Performance characteristics documented
- Known limitations documented
- Benchmarks reproducible
- Edge cases covered in tests
