#!/usr/bin/env bash
# Run the node test binaries concurrently; tests create their own scratch databases.
# The socket binaries (archipelago_*, pulse_*) keep wall-clock budgets, so they run with at
# most one test thread per core: after the others when the pool would oversubscribe the cores.
# CATALYST_TEST_SHARD=i/n runs every n-th binary from the i-th; without it every binary runs.
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root/services/catalyst"
receipts="$root/.cache/dev"
logs="${CARGO_TARGET_DIR:-target}/test-logs"
mkdir -p "$logs" "$receipts"
rm -f "$receipts/stats-engine-receipt.log"
# Every failing binary adds one line here, so the last lines of a failed run name what failed. The
# summary goes to stdout, the stream of the dumped logs, so it follows them.
failures="$logs/failures"
: > "$failures"
phase="building the test binaries"
report() {
  status=$?
  [ "$status" -ne 0 ] || return 0
  echo "catalyst-test: failed with status $status while $phase"
  if [ -s "$failures" ]; then
    cat "$failures"
  elif [ "$phase" != "building the test binaries" ] && [ "$phase" != "selecting the test binaries" ] \
    && [ "$phase" != "running the doc-tests" ]; then
    echo "catalyst-test: a runner shell failed without naming a binary (see the output above)"
  fi
  if [ "${GITHUB_ACTIONS:-}" = true ]; then
    while IFS= read -r line; do echo "::error title=Rust test binary failed::$line"; done < "$failures"
  fi
}
trap report EXIT
cargo test --locked "$@" --no-run --message-format=json-render-diagnostics | python3 -c '
import json, os, sys
for line in sys.stdin:
    artifact = json.loads(line)
    if artifact.get("executable") and artifact.get("profile", {}).get("test"):
        target = artifact["target"]
        print(os.path.relpath(target["src_path"]), ",".join(target["kind"]), artifact["executable"], sep="\t")
' > "$logs/binaries"
phase="selecting the test binaries"
[ -s "$logs/binaries" ] || { echo "catalyst-test: no test binaries found" >&2; exit 1; }
shard="${CATALYST_TEST_SHARD:-1/1}"
index="${shard%/*}" total="${shard#*/}"
[[ "$shard" =~ ^[1-9][0-9]*/[1-9][0-9]*$ ]] && [ "$index" -le "$total" ] || { echo "catalyst-test: CATALYST_TEST_SHARD must be i/n with i from 1 to n" >&2; exit 1; }
# Every shard must cut the same list. The path of a binary holds a build hash and the order of plain
# sort follows the locale, so the list is in byte order of the source file and kind of each target.
same=$(cut -f1,2 "$logs/binaries" | LC_ALL=C sort | LC_ALL=C uniq -d)
[ -z "$same" ] || { echo "catalyst-test: two test binaries have one source file and kind: $same" >&2; exit 1; }
LC_ALL=C sort "$logs/binaries" | cut -f3- | awk -v i="$index" -v n="$total" 'NR % n == i % n' > "$logs/shard"
cores=$(nproc 2>/dev/null || getconf _NPROCESSORS_ONLN)
jobs="${CATALYST_TEST_JOBS:-$(( cores < 16 ? cores : 16 ))}"
[ "$jobs" -ge 1 ] || { echo "catalyst-test: CATALYST_TEST_JOBS must be at least 1" >&2; exit 1; }
# jobs * RUST_TEST_THREADS tests run at once, each with up to 10 of the cluster's 400 connections.
export RUST_TEST_THREADS="${RUST_TEST_THREADS:-4}"
socket_jobs=$(( cores / RUST_TEST_THREADS ))
[ "$socket_jobs" -ge 1 ] || socket_jobs=1
[ "$socket_jobs" -le "$jobs" ] || socket_jobs="$jobs"
grep -E '/(archipelago|pulse)_[^/]*$' "$logs/shard" > "$logs/sockets" || [ $? -eq 1 ]
grep -v -E '/(archipelago|pulse)_[^/]*$' "$logs/shard" > "$logs/others" || [ $? -eq 1 ]
run() {
  xargs -P "$1" -I{} sh -c '
name=$(basename "$1")
log="$2/$name.log"
status=0
"$1" > "$log" 2>&1 || status=$?
if [ "$status" -ne 0 ]; then
  cat "$log"
  if [ "$status" -gt 128 ]; then why="was killed by signal $((status - 128))"; else why="exited with status $status"; fi
  failed=$(sed -n "s/^test \(.*\) \.\.\. FAILED\$/\1/p" "$log" | tr "\n" " " | sed "s/ \$//")
  line="catalyst-test: $name $why; failed tests: ${failed:-none reported, see the end of its log above}"
  echo "$line"
  echo "$line" >> "$2/failures"; exit 1
fi
sed -n "s|^test result|$name|p" "$log"
case "$name" in
  stats_metrics_engine-*)
    sed -n "/^STATS_ENGINE_RECEIPT /p" "$log" > "$3/stats-engine-receipt.log" || exit 1
    cat "$3/stats-engine-receipt.log" ;;
esac
' _ {} "$logs" "$receipts"
}
if [ "$socket_jobs" -eq "$jobs" ]; then
  phase="running the test binaries"
  run "$jobs" < "$logs/shard"
else
  phase="running the test binaries"
  run "$jobs" < "$logs/others"
  phase="running the socket test binaries"
  run "$socket_jobs" < "$logs/sockets"
fi
phase="running the doc-tests"
cargo test --locked "$@" --doc
