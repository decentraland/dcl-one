#!/usr/bin/env bash
# Run the node test binaries concurrently; tests create their own scratch databases.
# The node's integration tests link as a few grouped binaries (services/catalyst/Cargo.toml), so
# the node_* groups run with twice the test threads of a crate's binary unless RUST_TEST_THREADS
# is set (tooling/dev-test.sh sets 4, so in CI every binary runs 4). The socket groups
# (sockets_*) keep wall-clock budgets: they run after the others, sharing one test thread per core.
# An explicit RUST_TEST_THREADS caps every binary, groups included.
# CATALYST_TEST_SHARD=i/n runs the i-th of n shards, cut by the recorded times in
# tooling/catalyst-test-durations.tsv; without it every binary runs. Each shard starts its longest
# binaries first.
# CATALYST_TEST_DOC_SHARD=j runs the doc-tests in shard j only; without it every shard runs them.
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
durations="$root/tooling/catalyst-test-durations.tsv"
cd "$root/services/catalyst"
logs="${CARGO_TARGET_DIR:-target}/test-logs"
mkdir -p "$logs"
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
# The regression guard builds its inventory and this shard together so Cargo can schedule
# their common prerequisites and links in one invocation.
inventory=
targets=(--lib --bins)
if [ "${1:-}" = "--write-inventory" ]; then
  usage='usage: catalyst-test.sh --write-inventory FILE [TEST ...] -- [CARGO_ARGS ...]'
  [ -n "${2:-}" ] && [ "$2" != "--" ] || { echo "$usage" >&2; exit 2; }
  inventory="$2"
  shift 2
  while [ "$#" -gt 0 ] && [ "$1" != "--" ]; do
    targets+=(--test "$1")
    shift
  done
  [ "$#" -gt 0 ] || { echo "$usage" >&2; exit 2; }
  shift
fi
build_targets() {
  cargo test --locked "$@" --no-run --message-format=json-render-diagnostics | tee "${inventory:-/dev/null}" | python3 -c '
import json, os, sys
for line in sys.stdin:
    artifact = json.loads(line)
    if artifact.get("executable") and artifact.get("profile", {}).get("test"):
        target = artifact["target"]
        print(os.path.relpath(target["src_path"]), ",".join(target["kind"]), artifact["executable"], sep="\t")
'
}
planned=false
if [ "${CATALYST_TEST_SHARD:-1/1}" != 1/1 ]; then
  python3 "$root/tooling/catalyst-test-targets.py" plan "$@" > "$logs/binaries"
  if [ -s "$logs/binaries" ]; then planned=true; fi
fi
if [ "$planned" = false ]; then build_targets "$@" > "$logs/binaries"; fi
phase="selecting the test binaries"
[ -s "$logs/binaries" ] || { echo "catalyst-test: no test binaries found" >&2; exit 1; }
shard="${CATALYST_TEST_SHARD:-1/1}"
index="${shard%/*}" total="${shard#*/}"
[[ "$shard" =~ ^[1-9][0-9]*/[1-9][0-9]*$ ]] && [ "$index" -le "$total" ] || { echo "catalyst-test: CATALYST_TEST_SHARD must be i/n with i from 1 to n" >&2; exit 1; }
# A value outside 1 to n fails every shard, so no setting makes all shards skip the doc-tests.
doc="${CATALYST_TEST_DOC_SHARD:-$index}"
[[ "$doc" =~ ^[1-9][0-9]*$ ]] && [ "$doc" -le "$total" ] || { echo "catalyst-test: CATALYST_TEST_DOC_SHARD must name one shard from 1 to $total" >&2; exit 1; }
# Every shard must cut the same list. The path of a binary holds a build hash and the order of plain
# sort follows the locale, so the list is in byte order of the source file and kind of each target.
same=$(cut -f1,2 "$logs/binaries" | LC_ALL=C sort | LC_ALL=C uniq -d)
[ -z "$same" ] || { echo "catalyst-test: two test binaries have one source file and kind: $same" >&2; exit 1; }
# The cut reads the list, recorded times and n. Socket groups are assigned first, then other
# binaries, longest first within each phase. The cost models four concurrent binaries followed
# by the socket phase. Each node_* and sockets_* group is spread across shards.
# A binary without a time keeps the round-robin place
# (every n-th from the i-th) and runs first, as it may be long.
LC_ALL=C sort "$logs/binaries" | python3 -c '
import heapq, re, sys
index, total, recorded = int(sys.argv[1]), int(sys.argv[2]), {}
for line in open(sys.argv[3], encoding="utf-8"):
    if line.strip() and not line.startswith("#"):
        source, kind, seconds = line.rstrip("\n").split("\t")
        recorded[source, kind] = int(seconds)
rows = [line.rstrip("\n").split("\t", 2) for line in sys.stdin]
time = lambda row: recorded.get((row[0], row[1]))
socket = lambda row: re.search("/sockets_[^/]*$", row[2]) is not None
group = lambda row: re.match("tests/(node|sockets)_", row[0]) and row[0].split("_")[0]
order = lambda shard: sorted(shard, key=lambda row: (time(row) is not None, -(time(row) or 0)))
SLOTS = 4  # binaries at once on the 4-core Linux runners, which bound the job
def cost(shard):
    slots = [0] * SLOTS
    for row in order(shard):
        if not socket(row):
            heapq.heappush(slots, heapq.heappop(slots) + (time(row) or 0))
    return max(slots) + sum(time(row) or 0 for row in shard if socket(row))
missing = sorted(row[0] for row in rows if time(row) is None)
if missing:
    print("catalyst-test: no recorded time, cut round-robin (refresh tooling/catalyst-test-durations.tsv): "
          + " ".join(missing), file=sys.stderr)
shards = [[] for _ in range(total)]
for number, row in enumerate(rows):
    if time(row) is None:
        shards[number % total].append(row)
# Reserve the serial socket phase before filling the four concurrent slots.
for row in sorted((row for row in rows if time(row) is not None), key=lambda row: (not socket(row), -time(row))):
    taken = [sum(group(other) == group(row) for other in shard) if group(row) else 0 for shard in shards]
    best = min((i for i in range(total) if taken[i] == min(taken)), key=lambda i: (cost(shards[i] + [row]), i))
    shards[best].append(row)
for row in order(shards[index - 1]):
    print(row[2])
' "$index" "$total" "$durations" > "$logs/shard"
if [ "$planned" = true ]; then
  phase="building the test binaries"
  while IFS= read -r target; do
    case "$target" in test/*) targets+=(--test "${target#test/}");; esac
  done < "$logs/shard"
  build_targets "$@" "${targets[@]}" > "$logs/artifacts"
  python3 "$root/tooling/catalyst-test-targets.py" resolve "$logs/shard" "$logs/binaries" "$logs/artifacts" > "$logs/executables"
  mv "$logs/executables" "$logs/shard"
fi
if [ -n "$inventory" ]; then exit 0; fi
cores=$(nproc 2>/dev/null || getconf _NPROCESSORS_ONLN)
jobs="${CATALYST_TEST_JOBS:-$(( cores < 16 ? cores : 16 ))}"
[ "$jobs" -ge 1 ] || { echo "catalyst-test: CATALYST_TEST_JOBS must be at least 1" >&2; exit 1; }
# Up to jobs * RUST_TEST_THREADS tests run at once (a node_* group counting twice), each with up
# to 10 of the cluster's 400 connections.
cap="${RUST_TEST_THREADS:-}"
export RUST_TEST_THREADS="${RUST_TEST_THREADS:-4}"
group_threads=$(( ${cap:-$(( RUST_TEST_THREADS * 2 ))} ))
grep -E '/sockets_[^/]*$' "$logs/shard" > "$logs/sockets" || [ $? -eq 1 ]
grep -v -E '/sockets_[^/]*$' "$logs/shard" > "$logs/others" || [ $? -eq 1 ]
socket_jobs=$(( $(wc -l < "$logs/sockets") ))
[ "$socket_jobs" -ge 1 ] || socket_jobs=1
[ "$socket_jobs" -le "$jobs" ] || socket_jobs="$jobs"
socket_threads=$(( cores / socket_jobs ))
[ "$socket_threads" -ge 1 ] || socket_threads=1
[ -z "$cap" ] || [ "$socket_threads" -le "$cap" ] || socket_threads="$cap"
run() {
  xargs -P "$1" -I{} sh -c '
name=$(basename "$1")
log="$2/$name.log"
case "$name" in
  node_*) RUST_TEST_THREADS="$3"; export RUST_TEST_THREADS ;;
  sockets_*) RUST_TEST_THREADS="$4"; export RUST_TEST_THREADS ;;
esac
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
' _ {} "$logs" "$group_threads" "$socket_threads"
}
phase="running the test binaries"
run "$jobs" < "$logs/others"
phase="running the socket test binaries"
run "$socket_jobs" < "$logs/sockets"
phase="running the doc-tests"
if [ "$doc" -eq "$index" ]; then
  cargo test --locked "$@" --doc
fi
