#!/usr/bin/env bash
# Functional tests for resolve-comment-pr.sh against a stubbed gh that serves a canned
# commit-to-pull-requests response from a file: no network, no repo. Run from anywhere:
#   bash .github/actions/bevy-explorer-ci-status/test-resolve-comment-pr.sh
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
RESOLVER="$SCRIPT_DIR/resolve-comment-pr.sh"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

# --- gh stub ---------------------------------------------------------------------------------
# `gh api repos/<repo>/commits/<sha>/pulls` serves $SCENARIO/pulls.json, fails when
# $SCENARIO/fail exists, and records that it was called so a scenario can assert it was not.
mkdir -p "$WORK/bin"
cat > "$WORK/bin/gh" <<'STUB'
#!/usr/bin/env bash
set -euo pipefail
[ "$1" = api ] && [[ "$2" == repos/*/commits/*/pulls* ]] || { echo "gh stub: unsupported command $*" >&2; exit 64; }
touch "$SCENARIO/.called"
[ -f "$SCENARIO/fail" ] && { echo "gh stub: HTTP 502" >&2; exit 1; }
cat "$SCENARIO/pulls.json"
STUB
chmod +x "$WORK/bin/gh"
export PATH="$WORK/bin:$PATH"

# --- helpers ---------------------------------------------------------------------------------
FAILURES=0
PASSES=0
SHA=1111111111111111111111111111111111111111
OTHER=2222222222222222222222222222222222222222

pr_run() { # pr_run <number> <head-ref>
  jq -nc --argjson n "$1" --arg h "$2" --arg sha "$SHA" \
    '{event:"pull_request", head_branch:$h, head_sha:$sha,
      pull_requests:[{number:$n, head:{ref:$h, sha:$sha, repo:{id:7}}, base:{ref:"main", repo:{id:7}}}]}'
}
branch_run() { # branch_run <event> <branch> <sha>
  jq -nc --arg e "$1" --arg b "$2" --arg sha "$3" '{event:$e, head_branch:$b, head_sha:$sha, pull_requests:[]}'
}
pulls() { # pulls '<number>:<state>:<head-ref>:<head-sha>:<head-repo>' ...
  local out='[' first=1 spec n state ref sha repo
  for spec in "$@"; do
    IFS=: read -r n state ref sha repo <<< "$spec"
    [ $first -eq 1 ] || out+=','
    first=0
    out+="{\"number\":$n,\"state\":\"$state\",\"head\":{\"ref\":\"$ref\",\"sha\":\"$sha\",\"repo\":{\"full_name\":\"$repo\"}}}"
  done
  echo "$out]"
}

new_scenario() {
  SCENARIO="$WORK/$1"
  mkdir -p "$SCENARIO"
  export SCENARIO
}

# expect <name> <event-json> <expected-stdout>
expect() {
  local name="$1" event="$2" want="$3" got code=0
  got=$(WORKFLOW_RUN_EVENT_OBJ="$event" REPO=o/r bash "$RESOLVER" 2>"$SCENARIO/stderr") || code=$?
  if [ "$got" = "$want" ] && [ "$code" -eq 0 ]; then
    echo "  ok   $name"
    PASSES=$((PASSES + 1))
  else
    echo "  FAIL $name: exit=$code"
    echo "       want:"; sed 's/^/         /' <<< "$want"
    echo "       got:";  sed 's/^/         /' <<< "$got"
    sed 's/^/         /' "$SCENARIO/stderr"
    FAILURES=$((FAILURES + 1))
  fi
}
expect_not_called() {
  if [ -f "$SCENARIO/.called" ]; then
    echo "  FAIL $1: gh was called"
    FAILURES=$((FAILURES + 1))
  else
    echo "  ok   $1 (gh not called)"
    PASSES=$((PASSES + 1))
  fi
}
out() { printf 'pr-number=%s\nhead-ref=%s\nhead-sha=%s' "$@"; }

# --- scenarios -------------------------------------------------------------------------------
echo "resolve-comment-pr.sh"

new_scenario pr-run
expect "a pull request's run resolves from the payload" \
  "$(pr_run 10 feat/thing)" "$(out 10 feat/thing "$SHA")"
expect_not_called "a pull request's run needs no lookup"

new_scenario branch-run-with-pr
pulls "20:open:feat/other:$OTHER:o/r" "21:open:feat/thing:$SHA:o/r" > "$SCENARIO/pulls.json"
expect "a branch run whose commit heads an open pull request resolves to it" \
  "$(branch_run push feat/thing "$SHA")" "$(out 21 feat/thing "$SHA")"

new_scenario branch-run-closed-pr
pulls "22:closed:feat/thing:$SHA:o/r" > "$SCENARIO/pulls.json"
expect "a closed pull request does not count" \
  "$(branch_run push feat/thing "$SHA")" "$(out "" "" "$SHA")"

new_scenario branch-run-fork-pr
pulls "23:open:feat/thing:$SHA:someone/r" > "$SCENARIO/pulls.json"
expect "a fork's pull request does not count" \
  "$(branch_run pull_request feat/thing "$SHA")" "$(out "" "" "$SHA")"

new_scenario branch-run-older-commit
pulls "24:open:feat/thing:$OTHER:o/r" > "$SCENARIO/pulls.json"
expect "a pull request whose head moved past the run's commit does not count" \
  "$(branch_run push feat/thing "$SHA")" "$(out "" "" "$SHA")"

new_scenario main-push
expect "a push run on main reports nowhere" \
  "$(branch_run push main "$SHA")" "$(out "" "" "$SHA")"
expect_not_called "a push on main needs no lookup"

new_scenario lookup-fails
touch "$SCENARIO/fail"
expect "a failed lookup reports nowhere without failing" \
  "$(branch_run push feat/thing "$SHA")" "$(out "" "" "$SHA")"
grep -q '::warning::' "$SCENARIO/stderr" && echo "  ok   lookup failure is warned about" && PASSES=$((PASSES + 1)) \
  || { echo "  FAIL lookup failure should warn"; FAILURES=$((FAILURES + 1)); }

echo
echo "$PASSES passed, $FAILURES failed"
[ "$FAILURES" -eq 0 ]
