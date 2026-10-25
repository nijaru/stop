#!/usr/bin/env bash
# Compare current collection modes. Install hyperfine separately if needed.
set -euo pipefail
cd "$(dirname "$0")"

if ! command -v hyperfine >/dev/null 2>&1; then
    echo "hyperfine is required (https://github.com/sharkdp/hyperfine)" >&2
    exit 1
fi
cargo build --release
binary=./target/release/stop

hyperfine --warmup 3 \
    --command-name inventory "$binary list --json" \
    --command-name measured-inventory "$binary list --cpu --json" \
    --command-name memory-ranking "$binary top --sort mem --json" \
    --command-name pid-inspection "$binary inspect $$ --json"

echo "CPU measurement includes a 250ms warm-up; other commands have no deliberate wait."
"$binary" list --json | python3 -c 'import json,sys; print("Observed processes:", json.load(sys.stdin)["total_processes"])'
