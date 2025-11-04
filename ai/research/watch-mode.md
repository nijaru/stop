# Watch Mode Research (2025-11-04)

## Goal
Implement continuous monitoring with configurable refresh rate.

## Research Sources
- Building Real-Time System Monitor in Rust Terminal (The New Stack)
- watchexec and cargo-watch patterns
- Production monitoring tools (top, htop, glances)

## Key Findings

### Refresh Rate
- **1s** - Standard for `top`, feels responsive
- **2s** - Better CPU efficiency, smoother (46ms stddev vs 216ms at 1s)
- **User-configurable** - Let users choose based on needs

**Decision**: Default to 2s, allow configuration via `--interval` flag

### Output Modes in Watch

| Mode | Watch Behavior |
|------|----------------|
| Human-readable | Clear screen + redraw (like `top`) |
| JSON | NDJSON (newline-delimited JSON, one snapshot per line) |
| CSV | Headers once, then rows (no repeated headers) |

### NDJSON Rationale
- Each line is valid JSON object
- Stream-friendly: `stop --watch --json | jq '.system.cpu_usage'`
- AI agents can process line-by-line
- Standard format (jsonlines.org)

### Signal Handling
- Ctrl+C should exit cleanly
- No raw terminal mode needed (we're not interactive)
- Use ctrlc crate for cross-platform signal handling

### Screen Clearing
- ANSI escape codes: `\x1B[2J\x1B[H` (clear + home cursor)
- Alternative: `crossterm` crate (more robust, cross-platform)
- Decision: Use crossterm (small dependency, handles Windows)

## Implementation Plan

```rust
// Add --interval flag (default 2s)
#[arg(long, default_value_t = 2.0)]
interval: f64,

// Watch loop
loop {
    let snapshot = collect_snapshot()?;
    // ... apply filter/sort/limit

    if human_readable {
        clear_screen();
        print_snapshot(&snapshot);
    } else if json {
        println!("{}", serde_json::to_string(&snapshot)?); // NDJSON
    } else if csv {
        if first_iteration {
            print_csv_header();
        }
        print_csv_rows(&snapshot);
    }

    thread::sleep(Duration::from_secs_f64(interval));
}
```

## Edge Cases

1. **Filter matches nothing**: Still show system metrics, empty process list
2. **Terminal resize**: Let it flow naturally (no handling needed for simple mode)
3. **Very fast interval (<200ms)**: Warn user about CPU overhead
4. **Non-TTY output**: Watch mode should work (e.g., piping to file)

## Performance Considerations

- Collection time: ~200ms (sysinfo sleep for accurate CPU readings)
- Minimum practical interval: ~0.5s (below that, mostly waiting for sysinfo)
- Memory: Single snapshot at a time, no history tracking

## Testing Strategy

- Unit test: None needed (integration of existing functions)
- Integration test: Spawn with `--watch`, verify first output, kill process
- Manual test: Visual inspection of refresh behavior

## Dependencies

**Add to Cargo.toml:**
- `crossterm = "0.28"` - Terminal control (clear screen)
- `ctrlc = "3.4"` - Signal handling (optional, can use stdlib)

**Alternative**: No new deps, use ANSI codes directly (simpler, less robust on Windows)

**Decision**: Use crossterm (better UX, small cost)
