#!/usr/bin/env bash
# Verify the archive users install, without publishing or replacing local binaries.
set -euo pipefail
cd "$(dirname "$0")/.."
python3 --version >/dev/null
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
# Package verification builds the same crate identity from target/package.
# Isolate those artifacts so later workspace builds cannot reuse packaged sources.
export CARGO_TARGET_DIR="$work/target"

cargo package --locked
archive=$(cargo metadata --locked --no-deps --format-version 1 | python3 -c '
import json, pathlib, sys
metadata = json.load(sys.stdin)
pkg = metadata["packages"][0]
print(pathlib.Path(metadata["target_directory"]) / "package" / (pkg["name"] + "-" + pkg["version"] + ".crate"))
')
package=$(basename "$archive" .crate)
version=${package#stop-cli-}

tar -xzf "$archive" -C "$work"
test -f "$work/$package/Cargo.lock"
cargo install --locked --path "$work/$package" --root "$work/install" --target-dir "$work/target"
binary="$work/install/bin/stop"
test "$("$binary" --version)" = "stop $version"
"$binary" --help >/dev/null
"$binary" list --pid "$$" --json | python3 -c '
import json, sys
report = json.load(sys.stdin)
assert report["schema_version"] == 1
assert len(report["processes"]) == 1
process = report["processes"][0]
assert process["pid"] == int(sys.argv[1])
assert process["birth_id"] is not None
' "$$"
printf 'Packaged stop %s passed the install smoke test.\n' "$version"
