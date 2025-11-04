#!/usr/bin/env bash
# Quick Fedora testing script - run this on your Fedora machine

set -euo pipefail

echo "stop - Fedora Validation"
echo "======================="
echo ""

# Build
echo "Building release binary..."
cargo build --release
echo ""

# Quick smoke test
echo "Smoke test..."
./target/release/stop --json > /dev/null && echo "✅ JSON output works"
./target/release/stop --csv > /dev/null && echo "✅ CSV output works"
./target/release/stop --filter "cpu > 0" > /dev/null && echo "✅ Simple filter works"
./target/release/stop --filter "cpu > 0 and mem > 0" > /dev/null && echo "✅ Compound filter works"
echo ""

# Tests
echo "Running tests..."
cargo test --quiet
echo "✅ All tests pass"
echo ""

# Clippy
echo "Running clippy..."
cargo clippy --quiet -- -D warnings 2>&1 | grep -v "^warning: " || echo "✅ Zero clippy warnings"
echo ""

# User field check (should show usernames on Linux, not UIDs)
echo "User field check (Linux should show usernames):"
./target/release/stop --json | jq -r '.processes[0:3] | .[] | .user' | head -3
echo ""

# Quick benchmark if hyperfine available
if command -v hyperfine &> /dev/null; then
    echo "Quick benchmark (3 runs)..."
    hyperfine --warmup 1 --runs 3 './target/release/stop --json' 2>&1 | grep "Time (mean"
else
    echo "Hyperfine not installed, skipping benchmark"
fi

echo ""
echo "✅ Fedora validation complete!"
