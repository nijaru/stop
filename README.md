# stop

> Modern process monitoring with structured output

**stop** (structured top) is a process and system monitoring tool that outputs JSON, CSV, or human-readable formats. Get system metrics and process information in a format that's easy to parse, script, and automate.

## Why stop?

Traditional monitoring tools output formatted text that's hard to parse:
- `top`/`htop` - Interactive TUI, not script-friendly
- `ps` - Text output requires complex parsing (awk/grep/sed)
- `iostat`/`vmstat` - Inconsistent formats across platforms

**stop** provides:
- **Structured output** - JSON and CSV for easy parsing
- **Powerful filtering** - Simple syntax with AND/OR logic
- **Multiple formats** - JSON for scripts, CSV for analysis, human-readable for terminals
- **Cross-platform** - Consistent behavior across macOS and Linux
- **Watch mode** - Continuous monitoring with configurable intervals

## Status

**v0.0.1** - Phases 1-4 complete

✅ Filter, sort, limit processes (with AND/OR logic)
✅ JSON/CSV/human-readable output
✅ Advanced metrics: thread count, disk I/O, open file descriptors
✅ 52 tests passing (16 unit + 19 edge case + 17 integration)
✅ Zero clippy warnings
✅ CI/CD pipeline (GitHub Actions on Ubuntu and macOS)
✅ Tested on macOS and Linux (Fedora)
✅ Performance optimized (86% allocation reduction in watch mode)

## Installation

```bash
cargo install stop-cli
```

Or from source:
```bash
git clone https://github.com/nijaru/stop.git
cd stop
cargo install --path .
```

Verify:
```bash
stop --version
```

## Quick Start

```bash
# Human-readable output with colors (default)
stop

# JSON output
stop --json

# CSV output
stop --csv

# Filter high CPU processes
stop --filter "cpu > 10"

# Top 10 processes by memory
stop --sort-by mem --top-n 10

# Combined: filter, sort, limit
stop --filter "mem >= 1" --sort-by cpu --top-n 5 --json

# Watch mode (continuous monitoring)
stop --watch                              # Updates every 2s (default)
stop --watch --interval 1                 # Custom interval
stop --watch --json | jq '.system.cpu_usage'  # NDJSON stream
```

## Filter Syntax

Build filter expressions with `field op value` and combine them with `and`/`or`:

**Fields:**
- `cpu` - CPU percentage (float)
- `mem` - Memory percentage (float)
- `pid` - Process ID (integer)
- `name` - Process name (case-insensitive contains)
- `user` - User name/ID (exact match)

**Operators:**
- `>`, `>=`, `<`, `<=` - Numeric comparisons
- `==`, `!=` - Equality (works with all fields)
- `and`, `or` - Combine conditions (case-insensitive)

**Precedence:** `and` has higher precedence than `or` (standard boolean logic)

**Simple Examples:**
```bash
# High CPU processes
stop --filter "cpu > 50"

# Memory hogs
stop --filter "mem >= 5.0"

# System processes (low PIDs)
stop --filter "pid < 1000"

# Find Chrome processes
stop --filter "name == chrome"

# Processes by specific user
stop --filter "user == root"
```

**Compound Examples:**
```bash
# High CPU AND high memory
stop --filter "cpu > 50 and mem > 10"

# Chrome OR Firefox processes
stop --filter "name == chrome or name == firefox"

# High resource usage (either CPU or memory)
stop --filter "cpu > 50 or mem > 10"

# System processes with high CPU
stop --filter "pid < 1000 and cpu > 5"

# Case-insensitive keywords (all equivalent)
stop --filter "cpu > 10 AND mem > 5"
stop --filter "cpu > 10 and mem > 5"
stop --filter "cpu > 10 And mem > 5"

# Mixed AND/OR with precedence: (cpu > 50 AND mem > 10) OR name == chrome
stop --filter "cpu > 50 and mem > 10 or name == chrome"
```

## Features

### ✅ Implemented (v0.0.1)

**Output Modes:**
- JSON - Structured data for AI agents
- CSV - RFC 4180 compliant with proper escaping
- Human-readable - Color-coded table with system summary

**Filtering:**
- Simple `field op value` syntax
- Compound expressions with `and`/`or` logic
- Fields: cpu, mem, pid, name, user
- Operators: `>`, `>=`, `<`, `<=`, `==`, `!=`
- Proper precedence (AND before OR)
- AI-friendly JSON error messages

**Sorting:**
- Sort by: cpu, mem, pid, name
- Default: CPU descending

**Limiting:**
- `--top-n` flag to show top N processes
- Default: 20 processes

**Watch Mode:**
- Continuous monitoring with `--watch` flag
- Configurable interval with `--interval` (default: 2s)
- NDJSON output for JSON mode (stream-friendly)
- Screen clearing for human-readable mode
- Works with all filters, sorting, and output modes
- Graceful broken pipe handling (e.g., piping to `head`)

**Advanced Metrics (Phase 3):**
- Thread count per process
- Disk I/O (read/write bytes) per process
- Open file descriptors per process (when available)

### 🚧 Planned

- Per-process network metrics (sysinfo doesn't support yet)
- Windows support
- Parentheses for complex filter grouping
- Publish to crates.io

## Example Output

```json
{
  "timestamp": "2025-11-05T00:23:56.037549+00:00",
  "system": {
    "cpu_usage": 6.5,
    "memory_total": 137438953472,
    "memory_used": 73425764352,
    "memory_percent": 53.4
  },
  "processes": [
    {
      "pid": 1234,
      "name": "chrome",
      "cpu_percent": 12.5,
      "memory_bytes": 2147483648,
      "memory_percent": 1.6,
      "user": "501",
      "command": "/Applications/Chrome.app/Contents/MacOS/Chrome",
      "thread_count": 15,
      "disk_read_bytes": 12345678,
      "disk_write_bytes": 8765432,
      "open_files": 120
    }
  ]
}
```

## Use Cases

**Scripting & Automation:**
```bash
# Kill processes using >50% CPU
stop --filter "cpu > 50" --json | jq -r '.processes[].pid' | xargs kill

# Alert if memory usage >80%
if [ $(stop --json | jq '.system.memory_percent') -gt 80 ]; then
  echo "High memory usage!"
fi

# Log metrics for analysis
stop --csv --interval 5 > metrics.csv
```

**Monitoring & Alerting:**
- Track resource usage over time
- Trigger alerts on thresholds
- Export data for analysis
- Integration with monitoring systems

**Development & Debugging:**
- Find resource-intensive processes
- Monitor application behavior
- Track I/O and thread usage
- Verify processes are running

## Practical Examples

### Monitoring Specific Applications

```bash
# Watch Chrome memory usage in real-time
stop --watch --filter "name == chrome" --sort-by mem

# Find all Electron apps (VS Code, Slack, etc.)
stop --filter "name == electron" --json | jq '.processes[].name'

# Monitor Docker containers
stop --filter "name == docker" --top-n 20
```

### Resource Analysis

```bash
# Find memory leaks (processes with high memory, many open files)
stop --filter "mem > 5" --sort-by mem --json | \
  jq '.processes[] | select(.open_files > 100) | {name, memory_percent, open_files}'

# Identify I/O-heavy processes
stop --json | jq '.processes | sort_by(.disk_write_bytes) | reverse | .[:5]'

# Find multi-threaded processes
stop --json | jq '.processes | sort_by(.thread_count) | reverse | .[:10]'
```

### System Health Checks

```bash
# Quick system overview
stop --top-n 5

# Export system state for analysis
stop --json > system-snapshot-$(date +%Y%m%d-%H%M%S).json

# Monitor system during load test
stop --watch --interval 1 --json | tee load-test-metrics.jsonl

# Check if specific service is running
stop --filter "name == nginx" --json | jq -e '.processes | length > 0' && \
  echo "nginx is running" || echo "nginx is NOT running"
```

### Performance Debugging

```bash
# Find processes causing high CPU during investigation
stop --watch --filter "cpu > 10" --interval 0.5

# Compare disk I/O before and after optimization
stop --filter "name == myapp" --json | jq '.processes[].disk_write_bytes'

# Monitor resource usage with CSV for spreadsheet analysis
stop --watch --csv --interval 5 > metrics.csv
# Analyze in Excel/Google Sheets later
```

### Automation & Alerting

```bash
# Alert on resource spikes (monitoring script)
while true; do
  CPU=$(stop --json | jq '.system.cpu_usage')
  if (( $(echo "$CPU > 90" | bc -l) )); then
    echo "ALERT: CPU at ${CPU}%" | mail -s "High CPU" admin@example.com
  fi
  sleep 60
done

# Log top 10 processes every hour (cron job)
0 * * * * /usr/local/bin/stop --top-n 10 --json >> /var/log/process-metrics.jsonl

# Kill runaway processes automatically
stop --filter "cpu > 80 and name == myapp" --json | \
  jq -r '.processes[].pid' | \
  xargs -I {} sh -c 'echo "Killing PID {}"; kill {}'
```

## Design Goals

1. **Structured Output** - JSON and CSV for easy parsing and automation
2. **Cross-Platform** - Consistent behavior across operating systems
3. **Fast** - Minimal overhead, efficient data collection
4. **Simple** - Easy to use, clear syntax
5. **Reliable** - Production-ready error handling

## Comparison

| Feature | top/htop | ps | stop |
|---------|----------|-----|------|
| JSON output | ❌ | ❌ | ✅ |
| CSV output | ❌ | ❌ | ✅ |
| Filtering | Limited | Complex | Simple syntax |
| Cross-platform | ⚠️ Varies | ⚠️ Varies | ✅ Consistent |
| Structured data | ❌ TUI | ❌ Text parsing | ✅ JSON/CSV |
| Watch mode | ✅ | ❌ | ✅ |
| One-shot | ❌ | ✅ | ✅ |

## Development

```bash
# Run tests (52 tests: 16 unit + 19 edge case + 17 integration)
cargo test

# Check code quality
cargo clippy
cargo fmt

# Build release
cargo build --release

# Install locally
cargo install --path .
```

## Implementation Details

**Architecture:**
- Type-safe filter module with comprehensive validation
- Parse-time error checking (not eval-time)
- Efficient data collection with minimal overhead
- Cross-platform system metrics via sysinfo

**Testing:**
- 52 tests: 16 unit + 19 edge case + 17 integration
- Continuous integration on Ubuntu and macOS
- All tests passing, zero warnings
- Performance: <30ms overhead, optimized for watch mode

## Known Limitations

- **User field**: Shows UIDs (e.g., "501", "1000") instead of usernames on both macOS and Linux - sysinfo crate limitation
- **Open files**: Returns `null` for privileged processes and kernel threads (expected behavior)
- **Network metrics**: Per-process network metrics not available - sysinfo crate limitation
- **Collection time**: Includes mandatory 200ms sleep for accurate CPU readings
- **Windows**: Not yet tested (planned)

## Roadmap

See `ai/PLAN.md` for detailed roadmap.

**Completed (v0.0.1):**
- ✅ Phases 1-4: Filter, sort, watch, CSV output, advanced metrics
- ✅ Thread count, disk I/O, open file descriptors
- ✅ Comprehensive testing and optimization

**Next up:**
- Field testing and real-world validation
- Windows support
- Parentheses for complex filter grouping
- Publish to crates.io (after field testing)

## Contributing

Early stage project. Not yet accepting contributions - focusing on core implementation first.

## License

MIT

## Acknowledgments

Inspired by:
- **top/htop** - The classic monitoring tools
- **ps** - Process information standard
- **rg/fd/sy** - Modern CLI tools with clean output
