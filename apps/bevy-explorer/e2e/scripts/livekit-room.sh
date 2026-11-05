#!/usr/bin/env bash
# Runs the pinned LiveKit scenario against a local room: starts `livekit-server --dev`
# (keys devkey/secret, port 7880), mints a viewer token with `lk`, points the standalone at
# the room through the preview-mode scene room override, and runs the spec, which publishes
# with `lk room join --publish-demo` itself. Needs `brew install livekit livekit-cli`,
# DCL_E2E_NATIVE_BIN set to a standalone with the UUAV runtime set staged next to it, and the
# scenes built (`npm run build-scenes`).
set -euo pipefail
cd "$(dirname "$0")/.."

: "${DCL_E2E_NATIVE_BIN:?set DCL_E2E_NATIVE_BIN to the standalone binary}"
command -v livekit-server >/dev/null || { echo "livekit-server is not on PATH (brew install livekit)" >&2; exit 1; }
command -v lk >/dev/null || { echo "lk is not on PATH (brew install livekit-cli)" >&2; exit 1; }

PORT="${DCL_E2E_LIVEKIT_PORT:-7880}"
ROOM="${DCL_E2E_LIVEKIT_ROOM:-e2e}"
URL="ws://127.0.0.1:${PORT}"

livekit-server --dev --bind 127.0.0.1 --port "$PORT" >"${TMPDIR:-/tmp}/livekit-server.log" 2>&1 &
SERVER=$!
trap 'kill $SERVER 2>/dev/null || true' EXIT
for _ in $(seq 1 50); do
    curl -sf "http://127.0.0.1:${PORT}/" >/dev/null 2>&1 && break
    sleep 0.2
done

TOKEN=$(lk token create --api-key devkey --api-secret secret --room "$ROOM" --identity viewer --join --valid-for 2h 2>/dev/null | tail -1)
[ -n "$TOKEN" ] || { echo "lk token create printed no token" >&2; exit 1; }

export DCL_E2E_TRANSPORT=native
export DCL_E2E_LIVEKIT_URL="$URL"
export DCL_E2E_LIVEKIT_ROOM="$ROOM"
export DCL_SCENE_ROOM_ADAPTER="livekit:${URL}?access_token=${TOKEN}"
export DCL_E2E_NATIVE_ARGS="${DCL_E2E_NATIVE_ARGS:-} --preview"
npm run e2e -- --visual --filter 'pinned participant'
