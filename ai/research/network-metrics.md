# Network Metrics Research (2025-11-04)

## Goal
Add network RX/TX bytes per process to ProcessInfo.

## Sysinfo API Investigation

### Process-level network metrics
Checking if `Process` has network-related methods:
- `disk_usage()` ✅ exists (we used this for Phase 3.2)
- `network_usage()` or similar? Need to verify

### System-level network metrics
sysinfo has `Networks` type for system-wide network stats, but need per-process.

## Platform-Specific Challenges

### Linux
- `/proc/<pid>/net/dev` - Per-process network stats
- May not be available for all processes
- Requires parsing text files

### macOS
- No built-in per-process network stats in procfs
- Would need `lsof -i` or private APIs
- More complex than Linux

### Windows
- Performance counters or WMI
- Outside scope for now (not tested yet)

## Decision Points

**Option 1**: Use sysinfo if available
- Cleanest, cross-platform
- May not exist for per-process metrics

**Option 2**: Platform-specific code
- Linux: Parse /proc
- macOS: Use lsof or skip
- More complex, maintenance burden

**Option 3**: Defer to Phase 4 or document as limitation
- Focus on what sysinfo supports natively
- Document that network per-process is platform-dependent

## Research Steps

1. ✅ Check sysinfo Process methods for network_*
2. ✅ If not available, check if it's a known limitation
3. ✅ Decide: Implement platform-specific or document limitation
4. If implementing, Linux first (easier), macOS second

## Research Results

**Finding**: sysinfo 0.37 Process does NOT have per-process network metrics.
- Only system-wide `Networks` type available
- Would require platform-specific implementation:
  - Linux: Parse `/proc/<pid>/net/dev` (may not be available for all processes)
  - macOS: Use `lsof -i` or private APIs (significantly more complex)
  - Windows: Performance counters or WMI

**Decision**: Document as known limitation, defer to future release
- **Rationale**:
  - Requires platform-specific code (breaks cross-platform abstraction)
  - More maintenance burden than thread count/disk I/O features
  - Per-process network stats not universally available on all platforms
  - Can be added later if there's real-world demand (staying lean for 0.0.x)

**Action**: Mark Phase 3.3 as deferred, move to Phase 3.4 (open file descriptors)
