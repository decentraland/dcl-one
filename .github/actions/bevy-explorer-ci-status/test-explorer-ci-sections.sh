#!/usr/bin/env bash
# Functional tests for explorer-ci-sections.sh on canned job and artifact lists: no network, no
# repo. Run from anywhere:
#   bash .github/actions/bevy-explorer-ci-status/test-explorer-ci-sections.sh
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
COMPOSER="$SCRIPT_DIR/explorer-ci-sections.sh"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

FAILURES=0
PASSES=0
SHA=1111111111111111111111111111111111111111

# job '<name>' '<conclusion>' [seconds]: one job object; an empty conclusion is null.
job() {
  local completed=""
  [ -n "${3:-}" ] && completed=$(date -u -d "@$((1793181600 + $3))" +%Y-%m-%dT%H:%M:%SZ)
  jq -nc --arg n "$1" --arg c "$2" --arg e "$completed" '
    {name:$n, html_url:("https://github.com/o/r/actions/runs/1/job/" + ($n | gsub("[^A-Za-z0-9]"; "-"))),
     conclusion:(if $c == "" then null else $c end)}
    + (if $e == "" then {} else {started_at:"2026-10-28T10:00:00Z", completed_at:$e} end)'
}
jobs() { printf '%s\n' "$@" | jq -sc '{jobs: .}' > "$WORK/jobs.json"; }
artifacts() { # artifacts '<name>:<id>[:expired]' ...
  local out='{"artifacts":[' first=1 spec name id expired
  for spec in "$@"; do
    IFS=: read -r name id expired <<< "$spec"
    [ $first -eq 1 ] || out+=','
    first=0
    out+="{\"name\":\"$name\",\"id\":$id,\"expired\":${expired:-false},\"created_at\":\"2026-10-28T10:00:00Z\"}"
  done
  echo "$out]}" > "$WORK/artifacts.json"
}
compose() {
  (cd "$WORK" && REPO=o/r RUN_ID=1 RUN_URL=https://github.com/o/r/actions/runs/1 HEAD_SHA="$SHA" \
    JOBS_FILE=jobs.json ARTIFACTS_FILE=artifacts.json bash "$COMPOSER")
}
check() { # check <name> <file> <pattern>
  if grep -qF -- "$3" "$WORK/$2"; then
    echo "  ok   $1"
    PASSES=$((PASSES + 1))
  else
    echo "  FAIL $1: '$3' not in $2:"
    sed 's/^/         /' "$WORK/$2"
    FAILURES=$((FAILURES + 1))
  fi
}
check_absent() {
  if grep -qF -- "$3" "$WORK/$2"; then
    echo "  FAIL $1: '$3' found in $2:"
    sed 's/^/         /' "$WORK/$2"
    FAILURES=$((FAILURES + 1))
  else
    echo "  ok   $1"
    PASSES=$((PASSES + 1))
  fi
}

echo "test-explorer-ci-sections.sh"

# 1. Everything ran and passed, with every product's artifact.
jobs "$(job 'Integration guards' success 30)" "$(job Changes success 40)" \
  "$(job 'Test Suite (windows-2025)' success 2700)" "$(job 'Test Suite (ubuntu-24.04)' success 1500)" \
  "$(job 'Test Suite (macos-15)' success 3700)" "$(job 'Headless Smoke' success 900)" \
  "$(job 'Web engine, HUD and bridge scene' success 1300)" "$(job Rustfmt success 20)"
artifacts "bevy-explorer-web-$SHA:11" "bevy-headless-linux-x64-$SHA:12" \
  "bevy-explorer-headless-linux-amd64-$SHA:13" "bevy-explorer-benchmark-linux-amd64-$SHA:14" \
  "bevy-explorer-benchmark-windows-amd64-$SHA:15" "bevy-explorer-benchmark-macos-arm64-$SHA:16" \
  "bevy-explorer-native-timings-Windows-X64-$SHA:17"
compose
check "green build badge" build.md 'Build-Success!-3fb950'
check "web app row links the job, the result and the artifact" build.md \
  '| [Web app](https://github.com/o/r/actions/runs/1/job/Web-engine--HUD-and-bridge-scene) | ✅ Built | 21m 40s | [web app](https://github.com/o/r/actions/runs/1/artifacts/11) |'
check "headless row lists both artifacts" build.md \
  '| ✅ Built | 15m 0s | [npm package](https://github.com/o/r/actions/runs/1/artifacts/12) · [engine](https://github.com/o/r/actions/runs/1/artifacts/13) |'
check "benchmark runners by platform, sorted" build.md \
  'Benchmark runner: [linux-amd64](https://github.com/o/r/actions/runs/1/artifacts/14) · [macos-arm64](https://github.com/o/r/actions/runs/1/artifacts/16) · [windows-amd64](https://github.com/o/r/actions/runs/1/artifacts/15)'
check_absent "timings artifacts are not products" build.md 'native-timings'
check "green tests badge" tests.md 'Tests-Passed!-3fb950'
check "test rows by platform with job time" tests.md '| [Test Suite (Windows)](https://github.com/o/r/actions/runs/1/job/Test-Suite--windows-2025-) | ✅ Passed | 45m 0s |'
check "macOS row" tests.md '| [Test Suite (macOS)]('
check "an hour-long job" tests.md '| ✅ Passed | 1h 1m |'
check "headless smoke is a test row" tests.md '| [Headless Smoke](https://github.com/o/r/actions/runs/1/job/Headless-Smoke) | ✅ Passed | 15m 0s |'
check_absent "rustfmt is not a test row" tests.md 'Rustfmt'
check "footer names the run and the commit" tests.md '<sub>[Run summary](https://github.com/o/r/actions/runs/1) · commit `1111111`</sub>'

# 2. A failed Linux suite, a cancelled Windows one, a web build that failed with no artifact.
jobs "$(job Changes success 40)" "$(job 'Test Suite (windows-2025)' cancelled 100)" \
  "$(job 'Test Suite (ubuntu-24.04)' failure 1500)" "$(job 'Web engine, HUD and bridge scene' failure 300)" \
  "$(job 'Headless Smoke' skipped)"
artifacts "bevy-explorer-benchmark-linux-amd64-$SHA:14"
compose
check "red build badge" build.md 'Build-Failed!-ff0000'
check "a failed web build has no download" build.md '| ❌ Failed | 5m 0s | — |'
check "a skipped product is not selected" build.md '| — Not selected | — | — |'
check "red tests badge" tests.md 'Tests-Failed!-ff0000'
check "a cancelled suite" tests.md '| ⚠️ Cancelled | 1m 40s |'
check "a failed suite" tests.md '| ❌ Failed | 25m 0s |'
check "a skipped suite" tests.md '| [Headless Smoke](https://github.com/o/r/actions/runs/1/job/Headless-Smoke) | — Not selected | — |'

# 3. Only the web build was selected: the tests section says so.
jobs "$(job Changes success 40)" "$(job 'Test Suite' skipped)" "$(job 'Headless Smoke' skipped)" \
  "$(job 'Web engine, HUD and bridge scene' success 1300)"
artifacts "bevy-explorer-web-$SHA:11"
compose
check "green build badge with only the web app" build.md 'Build-Success!-3fb950'
check "tests not selected badge" tests.md 'Tests-Not%20selected-lightgrey'
check "tests headline" tests.md 'No test job was selected: the test inputs are unchanged.'

# 4. Only the tests were selected, and an expired artifact is no download.
jobs "$(job Changes success 40)" "$(job 'Test Suite (ubuntu-24.04)' success 1500)" \
  "$(job 'Web engine, HUD and bridge scene' skipped)" "$(job 'Headless Smoke' skipped)"
artifacts "bevy-explorer-web-$SHA:11:true" "bevy-explorer-benchmark-linux-amd64-$SHA:14"
compose
check "build not selected badge" build.md 'Build-Not%20selected-lightgrey'
check_absent "an expired artifact is no download" build.md 'artifacts/11'
check "green tests badge with one suite" tests.md 'Tests-Passed!-3fb950'

# 5. The run was cancelled: both sections say to refresh.
jobs "$(job Changes success 40)" "$(job 'Test Suite (ubuntu-24.04)' cancelled 10)" \
  "$(job 'Web engine, HUD and bridge scene' cancelled 10)"
artifacts
compose
check "grey build badge" build.md 'Build-Cancelled-lightgrey'
check "grey tests badge" tests.md 'Tests-Cancelled-lightgrey'
check "the cancelled headline" tests.md 'Push a new commit or re-run the workflow to refresh this section.'

# 7. A PR preview: linked beside the web app on the zone services, only for a built web app, and
#    never with a version that is not a plain npm version.
jobs "$(job Changes success 40)" "$(job 'Web engine, HUD and bridge scene' success 1300)"
artifacts "bevy-explorer-web-$SHA:11"
PREVIEW_VERSION=0.1.0-1.commit-abc1234 compose
check "the preview is linked after the web app" build.md \
  '[web app](https://github.com/o/r/actions/runs/1/artifacts/11) · [preview](https://cdn.decentraland.org/@dcl-regenesislabs/bevy-explorer-web/0.1.0-1.commit-abc1234/index.html?position=0%2C-2&baseDomain=decentraland.zone) |'
PREVIEW_VERSION='0.1.0)[x](https://evil' compose
check_absent "a malformed version is not linked" build.md 'preview'
jobs "$(job Changes success 40)" "$(job 'Web engine, HUD and bridge scene' failure 300)"
PREVIEW_VERSION=0.1.0-1.commit-abc1234 compose
check_absent "a failed web build has no preview" build.md 'preview'

# 6. A job name with table furniture renders as text.
jobs "$(job Changes success 40)" "$(job 'Test Suite (ubuntu|24.04)' success 10)"
artifacts
compose
check "pipes are stripped from a suite name" tests.md '| [Test Suite (Linux)]('

echo
if [ "$FAILURES" -gt 0 ]; then
  echo "$FAILURES failed, $PASSES passed"
  exit 1
fi
echo "all $PASSES passed"
