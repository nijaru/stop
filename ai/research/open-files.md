# Open Files Research (2025-11-04)

## Goal
Add open file descriptor count per process to ProcessInfo.

## Sysinfo API Investigation

**Finding**: sysinfo 0.37 Process HAS `open_files()` method ✅

```rust
pub fn open_files(&self) -> Option<usize>
```

- Returns number of open files in the current process
- Returns `None` if retrieval fails or system not supported
- **Important**: Computed every time called (may have performance cost)

## Implementation Plan

1. Add `open_files: Option<usize>` to ProcessInfo struct
2. Use `process.open_files()` directly (already returns Option)
3. Serialize as nullable field in JSON
4. CSV output: use empty string or "N/A" for None values

## Platform Support

- Linux: ✅ Supported via /proc
- macOS: ✅ Supported via lsof
- Windows: Need to verify

## Performance Consideration

Documentation notes: "this information is computed every time this function is called"

This may be slower than other metrics (thread_count, disk_usage). Consider:
- Acceptable for single snapshots
- Watch mode may see performance impact with many processes
- Can optimize later if needed
