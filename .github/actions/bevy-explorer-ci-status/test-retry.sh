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
