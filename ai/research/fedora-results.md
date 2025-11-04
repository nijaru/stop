# Fedora Testing Results (2025-11-04)

## Environment
- **OS**: Fedora (via Tailscale at nick@fedora)
- **CPU**: i9-13900KF
- **RAM**: 32GB DDR5
- **Rust**: Latest stable (via cargo)

## Test Results Summary

✅ **All tests pass**: 29 tests (16 unit + 13 integration)
✅ **Zero clippy warnings**: After fixing format string issues
✅ **All smoke tests pass**: JSON, CSV, simple filters, compound filters
✅ **Build successful**: Release binary compiled in ~5s

## Issues Found & Fixed

### 1. Clippy Warnings (Fixed)
**Issue**: Fedora's Rust version flagged 10 `uninlined_format_args` warnings

**Examples**:
```rust
// Old (warned on Fedora)
format!("{:.1}%", cpu_value)
eprintln!("Error: {}", e);

// Fixed
format!("{cpu_value:.1}%")
eprintln!("Error: {e}");
```

**Status**: ✅ Fixed in both `src/main.rs` and `src/watch.rs`

### 2. User Field Shows UIDs (Not Fixed - Limitation)
**Issue**: User field shows UIDs ("1000", "0") instead of usernames ("nick", "root")

**Examples from output**:
```
1000  (should be "nick")
0     (should be "root")
1000  (repeated)
```

**Root cause**: sysinfo crate limitation on both macOS and Linux

**Impact**: Medium - makes output less readable but doesn't affect functionality

**Workaround**: Users can map UIDs to usernames externally if needed

**Decision**: Document as known limitation, don't block on this

## Platform Comparison

| Metric | macOS (M3 Max) | Linux (Fedora) | Notes |
|--------|----------------|----------------|-------|
| **Tests** | 29 passed | 29 passed | ✅ Identical |
| **Clippy** | Clean | Clean (after fixes) | ✅ Fixed format strings |
| **Build time** | ~3.5s | ~5s | ✅ Acceptable |
| **User field** | UIDs (e.g., "501") | UIDs (e.g., "1000") | ⚠️ Both affected |
| **Functionality** | All features work | All features work | ✅ Identical |

## Performance

No hyperfine on Fedora machine, but build/test performance is good:
- Release build: ~5s
- Test suite: ~0.42s
- Expected runtime: Similar to macOS (~230ms including 200ms sleep)

## Known Limitations

### User Field (Both Platforms)
- **macOS**: Shows UID like "501" instead of "nick"
- **Linux**: Shows UID like "1000" instead of "nick"
- **Cause**: sysinfo crate doesn't resolve UIDs to usernames
- **Workaround**: Can parse `/etc/passwd` externally if needed
- **Impact**: Makes output less user-friendly, but doesn't affect filtering/sorting

## Verdict

✅ **Cross-platform validation successful**

**Fedora support is production-ready:**
- All features work identically to macOS
- No Linux-specific bugs found
- Performance is acceptable
- Only issue is cosmetic (user field) and affects both platforms

**Recommendation**: Update README to claim "Tested on macOS and Linux"

## Next Steps

1. ✅ Fix clippy warnings - DONE
2. ✅ Verify tests pass - DONE
3. Document user field limitation in README/STATUS
4. Decide: v0.1.0 or stay at v0.0.x pending real-world usage validation
