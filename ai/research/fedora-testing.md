# Fedora Testing Instructions

## Goal
Validate cross-platform compatibility and performance on Linux (Fedora x86_64).

## Prerequisites

Fedora machine accessible via Tailscale: `nick@fedora`

## Testing Steps

### 1. Copy Project to Fedora

```bash
# From Mac
rsync -av --exclude target --exclude .git /Users/nick/github/nijaru/stop/ nick@fedora:~/stop/
```

### 2. Build on Fedora

```bash
# SSH to Fedora
ssh nick@fedora

cd ~/stop

# Ensure Rust is installed
rustc --version  # Should be 1.91.0 or later

# Build release
cargo build --release

# Run tests
cargo test
cargo clippy -- -D warnings
```

### 3. Functional Testing

```bash
# Basic output
./target/release/stop

# JSON output
./target/release/stop --json | jq .

# CSV output
./target/release/stop --csv | head

# Filtering
./target/release/stop --filter "cpu > 1"
./target/release/stop --filter "cpu > 5 and mem > 1"
./target/release/stop --filter "name == systemd or name == chrome"

# Sorting and limiting
./target/release/stop --sort-by mem --top-n 10
./target/release/stop --filter "cpu > 1" --sort-by cpu --top-n 5 --json

# Watch mode (Ctrl+C to exit)
timeout 10 ./target/release/stop --watch
timeout 10 ./target/release/stop --watch --json --interval 1
```

### 4. Performance Benchmarking

```bash
# Install hyperfine if needed
cargo install hyperfine

# Run benchmark
./bench.sh
```

Expected results:
- Total time: ~230ms (including 200ms sleep)
- Overhead: <50ms (Linux may be faster than macOS)
- Filter operations: <1ms overhead

### 5. Platform-Specific Checks

**User field**: On Linux, should show actual usernames (not UIDs like macOS)
```bash
./target/release/stop --json | jq '.processes[0].user'
# Expected: "nick", "root", etc. (not "1000", "0")
```

**System metrics**: Verify accuracy
```bash
# Compare with standard tools
free -m  # Memory
top -bn1 | head -5  # CPU
./target/release/stop --json | jq '.system'
```

**Process listing**: Check completeness
```bash
# Count processes
ps aux | wc -l
./target/release/stop --json | jq '.processes | length'
# Should be similar (within ~10%)
```

### 6. Edge Cases

```bash
# High process count (if applicable)
./target/release/stop --top-n 1000 --json | jq '.processes | length'

# System processes
./target/release/stop --filter "pid < 1000" --sort-by pid

# Watch mode stress test
timeout 30 ./target/release/stop --watch --interval 0.5 > /dev/null
# Should not crash, CPU usage should be reasonable
```

## Expected Differences from macOS

| Metric | macOS | Linux | Reason |
|--------|-------|-------|--------|
| User field | UID (e.g., "501") | Username (e.g., "nick") | sysinfo implementation |
| Process count | ~500 | Varies | System-dependent |
| Performance | ~229ms | ~220-250ms | Hardware/kernel differences |

## Known Issues to Verify

- [ ] User field shows usernames (not UIDs)
- [ ] All 29 tests pass
- [ ] Zero clippy warnings
- [ ] Performance <250ms total time
- [ ] Watch mode works correctly
- [ ] CSV escaping handles Linux process names

## Reporting Results

Create `ai/research/fedora-results.md` with:

```markdown
# Fedora Testing Results

## Environment
- Fedora version: X
- Kernel: X
- Rust version: X
- CPU: i9-13900KF
- RAM: 32GB DDR5

## Test Results
- [ ] All tests pass
- [ ] Zero clippy warnings
- [ ] Performance: Xms (overhead: Xms)
- [ ] User field shows usernames: Yes/No
- [ ] Watch mode: Working
- [ ] CSV output: Valid

## Issues Found
(List any issues or differences from macOS)

## Benchmarks
(Paste hyperfine output)
```

## Next Steps After Testing

If all tests pass:
- Update README.md with "Tested on: macOS, Linux"
- Update STATUS.md with Fedora results
- Consider bumping to v0.0.2 or v0.1.0
- Decide: Phase 3 features or field testing?
