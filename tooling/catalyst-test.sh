#!/usr/bin/env bash
# Run the node test binaries concurrently. Each process makes its own scratch database.
set -euo pipefail
cd "$(dirname "$0")/../services/catalyst"
logs="${CARGO_TARGET_DIR:-target}/test-logs"
mkdir -p "$logs"
cargo test --locked "$@" --no-run --message-format=json-render-diagnostics | python3 -c '
import json, sys
for line in sys.stdin:
    artifact = json.loads(line)
    if artifact.get("executable") and artifact.get("profile", {}).get("test"):
        print(artifact["executable"])
' > "$logs/binaries"
cores=$(getconf _NPROCESSORS_ONLN)
xargs -P "${CATALYST_TEST_JOBS:-$(( cores < 16 ? cores : 16 ))}" -I{} sh -c '
log="$2/$(basename "$1").log"
"$1" > "$log" 2>&1 || { cat "$log"; exit 1; }
sed -n "s|^test result|$(basename "$1")|p" "$log"
' _ {} "$logs" < "$logs/binaries"
cargo test --locked "$@" --doc
