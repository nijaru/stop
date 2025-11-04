# Profiling Results (2025-11-04)

## Baseline Performance (hyperfine)

### Current Performance
```
Benchmark 1: ./target/release/stop --json --top-n 20
  Time (mean ± σ):     232.8 ms ±   1.8 ms    [User: 5.0 ms, System: 22.9 ms]
  Range (min … max):   228.1 ms … 235.3 ms

Benchmark 2: ./target/release/stop --csv --top-n 20
  Time (mean ± σ):     232.2 ms ±   1.6 ms    [User: 4.9 ms, System: 22.4 ms]

Benchmark 3: ./target/release/stop --json --filter "cpu > 0"
  Time (mean ± σ):     230.9 ms ±   1.7 ms    [User: 4.8 ms, System: 22.3 ms]
```

### Analysis

**🚨 CRITICAL FINDING: 200ms hardcoded sleep**

Line 71 in src/main.rs:
```rust
std::thread::sleep(std::time::Duration::from_millis(200));
sys.refresh_all();
```

**Breakdown:**
- Total time: ~232ms
- Sleep time: 200ms (86% of total!)
- User CPU: ~5ms (2%)
- System CPU: ~23ms (10%)
- Overhead: ~4ms (2%)

**Real overhead: ~32ms** (5ms user + 23ms system + 4ms misc)

**Impact:**
- Single snapshot: Acceptable (need sleep for CPU% calculation)
- Watch mode with 0.5s interval: 40% of time spent sleeping!
- Watch mode with 0.2s interval: Would be 100% sleep + overflow!

## Issues Identified

### 1. Sleep Duration (HIGH PRIORITY)
- 200ms is arbitrary and not configurable
- Blocks entire thread (can't do other work)
- In watch mode, this compounds with interval setting
- sysinfo docs recommend MINIMUM_CPU_UPDATE_INTERVAL (200ms on some platforms)

### 2. open_files() Cost (UNKNOWN)
- Doc says "computed every time called"
- Need to measure actual impact
- Called for EVERY process (could be expensive × process count)

### 3. Allocation Patterns (NEED PROFILING)
- Fresh Vec<ProcessInfo> every snapshot
- String allocations for every field
- CSV escaping allocates per field

### 4. Error Handling
- Many unwrap_or() / unwrap_or_else() calls
- No graceful handling if sysinfo fails
- Broken pipe panic in watch mode (not handled)

## Memory Usage (macOS /usr/bin/time -l)

```
10043392  maximum resident set size (10MB)
 8324288  peak memory footprint (8.3MB)
```

Single snapshot uses ~10MB memory. Reasonable for CLI tool.

## Key Findings Summary

### Performance is Actually Good
- Real overhead is ~32ms (without sleep)
- 200ms sleep is for sysinfo CPU calculation (documented requirement)
- JSON/CSV output and filtering add negligible overhead (<1ms)
- Memory usage is low (~10MB for single snapshot)

### Areas to Investigate
1. **open_files()** - unknown impact, called for every process
2. **Watch mode** - memory growth over time?
3. **Edge cases** - untested error conditions
4. **Error handling** - many unwrap() calls in production paths

## open_files() Impact Benchmark

Tested with 500 processes:

```
With open_files():    233.5 ms ± 5.2 ms
Without open_files(): 230.6 ms ± 1.7 ms
Difference:            ~3ms (1.2% slower)
Per-process cost:      ~6 microseconds
```

**Verdict**: Impact is minimal. The 3ms overhead for 500 processes is acceptable.
Note: Higher std deviation with open_files (5.2ms vs 1.7ms) suggests variable performance based on system state.

## Watch Mode Memory Usage Test

Ran watch mode with `--interval 0.5 --top-n 100` for 5 minutes:

```
Time     RSS    Growth   CPU%
0s        3MB     -      0%
30s      51MB   +48MB    2.0%
60s      58MB    +7MB    2.2%
90s      61MB    +3MB    2.0%
120s     62MB    +1MB    2.0%
150s     63MB    +1MB    1.6%
180s     64MB    +1MB    1.3%
210s     64MB     0MB    1.3%
240s     64MB     0MB    1.3%
270s     64MB     0MB    0.9%
```

**Findings:**
- Memory stabilizes at ~64MB after 3 minutes
- No continued memory leak detected
- Growth pattern: rapid initial (+48MB in 30s), then tapers off
- CPU usage stabilizes around 1-1.3%
- VSZ (virtual memory) stable at ~412GB (macOS memory management)

**Verdict**: Watch mode is stable. Initial memory spike is likely from sysinfo caching process info. No memory leak detected.

## Next Steps

1. ~~Profile with flamegraph~~ - baseline is already good
2. ~~Measure open_files() impact~~ - 3ms for 500 processes, acceptable
3. ~~Test watch mode memory~~ - stable at 64MB, no leak detected
4. **Add edge case test suite** - HIGH PRIORITY (filter parsing, CSV escaping, error conditions)
5. ~~Consider making open_files optional~~ - not needed, impact is minimal
6. Add proper error handling (replace unwrap() calls in production code)

## Summary

**Performance is excellent**:
- Single snapshot: ~32ms real overhead (200ms total with required sleep)
- open_files() adds only 3ms for 500 processes (~6μs per process)
- Watch mode: stable at 64MB memory, 1-1.3% CPU, no leaks
- JSON/CSV output: negligible overhead (<1ms)

**No optimizations needed** for performance. Focus should be on:
- Edge case testing and error handling
- Code quality (remove unwrap() calls)
- Documentation of performance characteristics
