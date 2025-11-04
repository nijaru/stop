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

🚧 **Early Development** - v0.0.1

Basic structure in place. Core functionality in development.

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
# Basic system snapshot (JSON)
stop --json

# Human-readable output (default)
stop

# Filter high CPU processes
stop --filter "cpu > 10" --json

# Top 10 processes by memory
stop --top-n 10 --sort-by mem

# Continuous monitoring (1s refresh)
stop --watch --json
```

## Planned Features

### Core Monitoring
- Process list (PID, name, CPU%, memory%, user, command)
- System metrics (CPU, memory, disk I/O, network)
- Thread information
- Open files/connections

### Query & Filter
- Filter by CPU/memory thresholds
- Sort by any metric
- Limit output (top N processes)
- Process name/user filtering

### Output Modes
- JSON (default for `--json` flag)
- Human-readable table
- CSV output
- Continuous watch mode

### Cross-Platform
- macOS (primary)
- Linux
- Windows (if feasible)

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
# Run tests
cargo test

# Run with debug logging
RUST_LOG=debug cargo run

# Build release
cargo build --release

# Install locally
cargo install --path .
```

## Implementation Roadmap

### Phase 1: MVP (v0.1.0)
- [ ] Basic process list (PID, name, CPU%, memory%)
- [ ] System metrics (CPU, memory)
- [ ] JSON output
- [ ] Human-readable table output
- [ ] Basic filtering (--filter flag)
- [ ] Sorting (--sort-by flag)

### Phase 2: Query & Filter (v0.2.0)
- [ ] Top N processes (--top-n)
- [ ] Multiple filter conditions
- [ ] Process name/user filtering
- [ ] CSV output

### Phase 3: Advanced Monitoring (v0.3.0)
- [ ] Disk I/O metrics
- [ ] Network metrics
- [ ] Thread information
- [ ] Open files/connections

### Phase 4: Polish (v0.4.0)
- [ ] Watch mode (--watch)
- [ ] Cross-platform testing
- [ ] Performance optimization
- [ ] Comprehensive documentation

### Phase 5: Production (v1.0.0)
- [ ] Stable API
- [ ] Full test coverage
- [ ] Windows support
- [ ] Published to crates.io

## Contributing

Early stage project. Not yet accepting contributions - focusing on core implementation first.

## License

MIT

## Acknowledgments

Inspired by:
- **top/htop** - The classic monitoring tools
- **ps** - Process information standard
- **rg/fd/sy** - Modern CLI tools with clean output
