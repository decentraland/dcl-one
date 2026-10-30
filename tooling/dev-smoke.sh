#!/usr/bin/env bash
# Run in a fresh checkout: nix develop --command bash tooling/dev-smoke.sh
set -euo pipefail
export FOUNDATION_DEV_PROFILE=offline
trap 'foundation-dev down' EXIT
foundation-dev up
foundation-dev wait --timeout "${FOUNDATION_DEV_STARTUP_TIMEOUT:-3600}"
eval "$(foundation-dev env)"
get() { curl --fail --silent --show-error --max-time 10 "$1"; }
places() { get "$1/places/api/places?limit=1" | jq -e '.ok == true' > /dev/null; }
# A SQL-backed API read, directly and through the Sites dev proxy, and a rendered page.
places "$FOUNDATION_DEV_API_URL"
# The content routes order by the en_US.utf8 collation under search_path=content,meta.
get "$FOUNDATION_DEV_API_URL/content/deployments?limit=1" | jq -e '.deployments | type == "array"' > /dev/null
places "$FOUNDATION_DEV_SITES_URL"
page="$(get "$FOUNDATION_DEV_SITES_URL/")"
grep -q '<title>Decentraland</title>' <<< "$page"
# The web Explorer: the shell document and the pinned engine artifact behind it.
get "$FOUNDATION_DEV_SITES_URL/play" > /dev/null
get "$FOUNDATION_DEV_SITES_URL/play/engine/pkg/manifest.json" | jq -e '.wasmSize > 0' > /dev/null
# The API keeps answering across a database-only restart.
foundation-dev restart postgres
foundation-dev wait --timeout 120
places "$FOUNDATION_DEV_API_URL"
# Repeated shell entry must reuse the running stack and its database.
foundation-dev up
foundation-dev status
# PostgreSQL and the applied schema must also survive a complete stack restart.
foundation-dev down
foundation-dev up
foundation-dev wait --timeout 120
places "$FOUNDATION_DEV_SITES_URL"
