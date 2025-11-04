# stop

> Structured process and system monitoring with JSON output

**stop** (structured top) is a modern system monitoring tool designed for AI agents and automation. Unlike traditional `top`/`htop`/`ps` which output human-readable text, `stop` provides structured JSON data for easy parsing and integration.

## Why stop?

**AI agents need structured data, not pretty text.**

Traditional monitoring tools are designed for humans:
- `top`/`htop` - Interactive TUI with colors and formatting
- `ps` - Text output that needs complex parsing
- `iostat`/`vmstat` - Inconsistent formats across platforms

**stop** provides:
- Clean JSON output for programmatic consumption
- Consistent format across platforms (macOS, Linux, Windows)
- Query and filter capabilities
- One-shot or continuous monitoring modes

## Status

**v0.0.1** - Core functionality complete

✅ Filter, sort, limit processes
✅ JSON/CSV/human-readable output
✅ 21 tests passing
✅ Zero clippy warnings

## Installation

```bash
# From source
git clone https://github.com/nijaru/stop.git
cd stop
cargo install --path .

# Verify installation
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

Simple `field op value` expressions:

**Fields:**
- `cpu` - CPU percentage (float)
- `mem` - Memory percentage (float)
- `pid` - Process ID (integer)
- `name` - Process name (case-insensitive contains)
- `user` - User name/ID (exact match)

**Operators:**
- `>`, `>=`, `<`, `<=` - Numeric comparisons
- `==`, `!=` - Equality (works with all fields)

**Examples:**
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

## Features

### ✅ Implemented (v0.0.1)

**Output Modes:**
- JSON - Structured data for AI agents
- CSV - RFC 4180 compliant with proper escaping
- Human-readable - Color-coded table with system summary

**Filtering:**
- Simple `field op value` syntax
- Fields: cpu, mem, pid, name, user
- Operators: `>`, `>=`, `<`, `<=`, `==`, `!=`
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

### 🚧 Planned

- Multiple filter conditions (AND/OR logic)
- Disk I/O metrics
- Network metrics
- Thread information
- Windows support

## Example Output

```json
{
  "timestamp": "2025-01-04T12:34:56Z",
  "system": {
    "cpu_usage": 45.2,
    "memory_total": 137438953472,
    "memory_used": 89478485332,
    "memory_percent": 65.1,
    "disk_read_bytes": 12345678,
    "disk_write_bytes": 87654321,
    "network_rx_bytes": 9876543210,
    "network_tx_bytes": 1234567890
  },
  "processes": [
    {
      "pid": 1234,
      "name": "chrome",
      "cpu_percent": 12.5,
      "memory_bytes": 2147483648,
      "memory_percent": 1.6,
      "user": "nick",
      "command": "/Applications/Chrome.app/Contents/MacOS/Chrome"
    }
  ]
}
```

## Use Cases

**AI Agents:**
- Debug performance issues
- Check if processes are running
- Monitor resource usage
- Automate system management

**Automation:**
- Alert on high resource usage
- Kill processes exceeding thresholds
- Log system metrics for analysis
- Integration with monitoring systems

**Scripting:**
```bash
# Kill processes using >50% CPU
stop --filter "cpu > 50" --json | jq -r '.processes[].pid' | xargs kill

# Alert if memory usage >80%
if [ $(stop --json | jq '.system.memory_percent') -gt 80 ]; then
  echo "High memory usage!"
fi
```

## Design Goals

1. **Structured Output** - JSON by default for AI/automation
2. **Cross-Platform** - Consistent behavior across OS
3. **Fast** - Minimal overhead, efficient data collection
4. **Simple** - Easy to use, clear output format
5. **Reliable** - Production-ready error handling

## Comparison

| Feature | top/htop | ps | stop |
|---------|----------|-----|------|
| JSON output | ❌ | ❌ | ✅ |
| Filtering | Limited | Complex | Simple flags |
| Cross-platform | ⚠️ Varies | ⚠️ Varies | ✅ Consistent |
| AI-friendly | ❌ TUI | ❌ Text parsing | ✅ Structured |
| Watch mode | ✅ | ❌ | ✅ |
| One-shot | ❌ | ✅ | ✅ |

## Development

```bash
# Run tests (21 tests: 8 unit + 13 integration)
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

**Tech Stack:**
- Rust 2024 edition
- sysinfo 0.37 - Cross-platform system metrics
- clap 4.5 - CLI parsing
- thiserror 2.0 - Structured error handling
- owo-colors 4.1 - Terminal colors

**Architecture:**
- Type-safe filter module with comprehensive validation
- Parse-time error checking (not eval-time)
- Zero-copy processing where possible
- Minimal allocations in hot paths

**Testing:**
- 8 unit tests (filter parsing, edge cases)
- 13 integration tests (CLI, output formats, errors)
- All tests passing, zero clippy warnings

## Roadmap

See `ai/PLAN.md` for detailed 5-phase roadmap.

**Next up:**
- Watch mode (continuous monitoring)
- Multiple filter conditions (AND/OR)
- Disk I/O and network metrics
- Windows support
- Publish to crates.io

## Contributing

Early stage project. Not yet accepting contributions - focusing on core implementation first.

## License

MIT

## Acknowledgments

Inspired by:
- **top/htop** - The classic monitoring tools
- **ps** - Process information standard
- **rg/fd/sy** - Modern CLI tools with clean output
