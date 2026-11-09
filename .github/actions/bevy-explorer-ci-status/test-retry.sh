#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/retry.sh"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
export RETRY_TEST_SLEEP_LOG="$WORK/sleeps"
fail() { printf 'FAIL: %s\n' "$*" >&2; exit 1; }
pass() { printf 'ok: %s\n' "$*"; }
sleep() { printf '%s\n' "$1" >> "$RETRY_TEST_SLEEP_LOG"; }

actual=''
for attempt in 1 2 3 4 5 6; do
  actual+="$(backoff_delay "$attempt" 5) "
done
[ "$actual" = '1 2 4 5 5 5 ' ] || fail "unexpected capped backoff: $actual"
pass 'backoff doubles from the common initial delay and saturates at its cap'

[ "$(backoff_delay 1 2 5)" = 2 ] || fail 'initial delay exceeded its cap'
[ "$(backoff_delay 1 0 5)" = 0 ] || fail 'zero cap did not disable the initial pause'
pass 'the cap also bounds the initial delay, including a zero cap'

sleep_for 8 3
[ "$(cat "$RETRY_TEST_SLEEP_LOG")" = 3 ] || fail 'sleep exceeded the remaining deadline'
pass 'sleep is clipped to the remaining deadline'

: > "$RETRY_TEST_SLEEP_LOG"
code=0
sleep_for 8 0 || code=$?
[ "$code" = 124 ] || fail "expired budget returned $code"
[ ! -s "$RETRY_TEST_SLEEP_LOG" ] || fail 'expired budget slept'
pass 'expired deadline returns 124 without sleeping'

retry_after_attempt 4 5 8
retry_after_attempt 5 5 8
[ "$(cat "$RETRY_TEST_SLEEP_LOG")" = 8 ] || fail 'last attempt added a wait'
pass 'attempt budget omits the wait after the final failure'

sleep() { return 17; }
code=0
retry_after_attempt 1 3 10 5 || code=$?
[ "$code" = 17 ] || fail "sleep failure was hidden as $code"
pass 'unexpected sleep failure retains its exit status'

mkdir -p "$WORK/bin"
cat > "$WORK/bin/npm" <<'STUB'
#!/usr/bin/env bash
set -euo pipefail
count=0
[ ! -f "$RETRY_TEST_CALLS" ] || count=$(cat "$RETRY_TEST_CALLS")
count=$((count + 1))
printf '%s\n' "$count" > "$RETRY_TEST_CALLS"
if [ -n "${RETRY_TEST_READY_AFTER_SECONDS:-}" ]; then
  elapsed=0
  while read -r delay; do elapsed=$((elapsed + delay)); done < "$RETRY_TEST_SLEEP_LOG"
  [ "$elapsed" -ge "$RETRY_TEST_READY_AFTER_SECONDS" ] || exit 23
else
  [ "$count" -ge "$RETRY_TEST_SUCCESS_AT" ] || exit 23
fi
printf '{"next":"1.2.3"}\n'
STUB
cat > "$WORK/bin/sleep" <<'STUB'
#!/usr/bin/env bash
set -euo pipefail
printf '%s\n' "$1" >> "$RETRY_TEST_SLEEP_LOG"
exit "${RETRY_TEST_SLEEP_EXIT:-0}"
STUB
chmod +x "$WORK/bin/npm" "$WORK/bin/sleep"
export PATH="$WORK/bin:$PATH"
export GITHUB_WORKSPACE="$(cd "$SCRIPT_DIR/../../.." && pwd)"
export VERSION=1.2.3 GITHUB_OUTPUT="$WORK/output" RETRY_TEST_CALLS="$WORK/calls"
python3 - "$GITHUB_WORKSPACE/.github/workflows/bevy-explorer-rollout.yml" "$WORK/rollout.sh" <<'PYTHON'
from pathlib import Path
import sys
import textwrap
source = Path(sys.argv[1]).read_text()
checkout = source.split('    steps:\n', 1)[1].split('    - name: Check the version is next\n', 1)[0]
assert 'ref: ${{ github.workflow_sha }}' in checkout
assert 'persist-credentials: false' in checkout
assert 'sparse-checkout: .github/actions/bevy-explorer-ci-status' in checkout
source = source.split('    - name: Check the version is next\n', 1)[1]
body = source.split('      run: |\n', 1)[1].split('    - name:', 1)[0]
Path(sys.argv[2]).write_text(textwrap.dedent(body))
PYTHON

: > "$RETRY_TEST_SLEEP_LOG"
RETRY_TEST_SUCCESS_AT=1 bash -euo pipefail "$WORK/rollout.sh"
[ "$(cat "$RETRY_TEST_CALLS")" = 1 ] || fail 'immediate success retried'
[ ! -s "$RETRY_TEST_SLEEP_LOG" ] || fail 'immediate success slept'
[ "$(cat "$GITHUB_OUTPUT")" = rollout=true ] || fail 'rollout result was lost'
pass 'actual rollout step returns immediately when its first npm read succeeds'

printf '0\n' > "$RETRY_TEST_CALLS"
: > "$RETRY_TEST_SLEEP_LOG"
RETRY_TEST_SUCCESS_AT=3 bash -euo pipefail "$WORK/rollout.sh"
[ "$(cat "$RETRY_TEST_CALLS")" = 3 ] || fail 'rollout did not make three reads'
[ "$(cat "$RETRY_TEST_SLEEP_LOG")" = $'15\n30' ] || fail 'rollout retry timing changed'
pass 'actual rollout step retries transient npm failures after 15 and 30 seconds'

printf '0\n' > "$RETRY_TEST_CALLS"
: > "$RETRY_TEST_SLEEP_LOG"
: > "$GITHUB_OUTPUT"
RETRY_TEST_READY_AFTER_SECONDS=40 bash -euo pipefail "$WORK/rollout.sh"
[ "$(cat "$RETRY_TEST_CALLS")" = 3 ] || fail 'rollout did not retry through a 40-second registry outage'
[ "$(cat "$GITHUB_OUTPUT")" = rollout=true ] || fail 'registry recovery did not allow rollout'
pass 'actual rollout retains the previous 40-second npm outage allowance'


printf '0\n' > "$RETRY_TEST_CALLS"
: > "$RETRY_TEST_SLEEP_LOG"
code=0
RETRY_TEST_SUCCESS_AT=99 bash -euo pipefail "$WORK/rollout.sh" || code=$?
[ "$code" = 1 ] || fail "exhausted rollout returned $code"
[ "$(cat "$RETRY_TEST_CALLS")" = 3 ] || fail 'exhausted rollout retried beyond its budget'
[ "$(cat "$RETRY_TEST_SLEEP_LOG")" = $'15\n30' ] || fail 'exhausted rollout slept after its final read'
pass 'actual rollout step fails after three reads without a final wait'

printf '0\n' > "$RETRY_TEST_CALLS"
: > "$RETRY_TEST_SLEEP_LOG"
code=0
RETRY_TEST_SUCCESS_AT=99 RETRY_TEST_SLEEP_EXIT=17 bash -euo pipefail "$WORK/rollout.sh" || code=$?
[ "$code" = 17 ] || fail "rollout swallowed the unexpected sleep failure as $code"
[ "$(cat "$RETRY_TEST_CALLS")" = 1 ] || fail 'rollout continued after an unexpected hard failure'
pass 'actual rollout step preserves errexit on an unexpected helper failure'
