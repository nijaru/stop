#!/usr/bin/env bash
# Simple benchmark for stop collection overhead using hyperfine

set -euo pipefail

BINARY="./target/release/stop"

echo "stop Performance Benchmark"
echo "=========================="
echo ""

# Build release binary if needed
if [ ! -f "$BINARY" ]; then
    echo "Building release binary..."
    cargo build --release
    echo ""
fi

# Check if hyperfine is available
if ! command -v hyperfine &> /dev/null; then
    echo "hyperfine not found. Installing..."
    cargo install hyperfine
    echo ""
fi

echo "Benchmarking JSON output (most common for AI agents)..."
hyperfine --warmup 3 \
    "$BINARY --json" \
    "$BINARY --json --filter 'cpu > 1'" \
    "$BINARY --json --filter 'cpu > 1 and mem > 0.1'"

echo ""
echo "Note: Collection includes mandatory 200ms sleep for CPU accuracy."
echo "Overhead = Total time - 200ms"

# Get process count
process_count=$($BINARY --json | jq '.processes | length')
echo "Process count tested: $process_count"
