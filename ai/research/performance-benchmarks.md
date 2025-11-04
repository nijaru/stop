# Performance Benchmarks (2025-11-04)

## Environment

- **Platform**: macOS (M3 Max)
- **Process count**: ~500 processes
- **Tool**: hyperfine 1.19
- **Binary**: Release build

## Results

### JSON Output (Most Common for AI Agents)

```
Benchmark 1: ./target/release/stop --json
  Time (mean ± σ):     229.2 ms ±   3.1 ms    [User: 4.6 ms, System: 21.1 ms]
  Range (min … max):   226.2 ms … 237.6 ms    13 runs

Benchmark 2: ./target/release/stop --json --filter 'cpu > 1'
  Time (mean ± σ):     228.8 ms ±   2.4 ms    [User: 4.5 ms, System: 20.6 ms]
  Range (min … max):   225.6 ms … 232.2 ms    13 runs

Benchmark 3: ./target/release/stop --json --filter 'cpu > 1 and mem > 0.1'
  Time (mean ± σ):     229.7 ms ±   1.8 ms    [User: 4.6 ms, System: 20.6 ms]
  Range (min … max):   226.0 ms … 231.7 ms    12 runs
```

## Analysis

**Total time**: ~229ms average
- Mandatory sleep (CPU accuracy): 200ms
- Actual overhead: **~29ms**

**Filter performance**:
- Simple filter overhead: <1ms (within measurement error)
- Compound filter overhead: <1ms (within measurement error)

**Breakdown**:
- User CPU time: ~4.6ms
- System CPU time: ~21ms
- Sleep time: 200ms
- Overhead: ~3ms

## Conclusion

✅ **Performance goal met**: 29ms overhead << 100ms target

The tool is extremely efficient:
- Filter operations add negligible overhead
- Compound filters (AND/OR) have same performance as simple filters
- 87% of time is mandatory sleep for CPU accuracy
- Only 13% is actual collection/processing

## Cross-Platform Testing

**TODO**: Test on Linux (Fedora) and Windows to verify consistent performance.

## Future Optimizations

Not needed currently, but potential improvements:
- Parallel process enumeration (likely no benefit due to sysinfo API)
- Reduce sleep time for use cases where CPU accuracy isn't critical (flag?)
- Zero-copy JSON serialization (marginal gains)
