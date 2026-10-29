#!/usr/bin/env bash
# Run the suites that need a database or a running node against the development stack:
#   nix develop --command bash tooling/dev-test.sh [rust] [e2e]
# The Rust tests create scratch databases in the development cluster; the Sites
# end-to-end suite seeds the development database and reads through the API.
set -euo pipefail
export FOUNDATION_DEV_PROFILE=offline
suites=("$@")
[ $# -eq 0 ] && suites=(rust e2e)
for suite in "${suites[@]}"; do
  case "$suite" in rust|e2e) ;; *) echo "unknown suite: $suite (rust|e2e)" >&2; exit 2 ;; esac
done
startup=()
if [ "${suites[*]}" = rust ]; then startup=(--postgres-only); fi
trap 'foundation-dev down' EXIT
foundation-dev up ${startup[@]+"${startup[@]}"}
foundation-dev wait ${startup[@]+"${startup[@]}"} --timeout "${FOUNDATION_DEV_STARTUP_TIMEOUT:-3600}"
eval "$(foundation-dev env)"
for suite in "${suites[@]}"; do
  case "$suite" in
    # Each test holds a scratch database with its own pools; the cluster allows 40 connections.
    rust)
      test_database_url="${FOUNDATION_DEV_TEST_DATABASE_URL:-}"
      if [ -z "${test_database_url//[[:space:]]/}" ]; then
        echo "FOUNDATION_DEV_TEST_DATABASE_URL must name disposable PostgreSQL" >&2
        exit 1
      fi
      nats_binary="$(command -v nats-server)" || { echo "nats-server executable missing" >&2; exit 1; }
      [ -x "$nats_binary" ] || { echo "nats-server must be executable" >&2; exit 1; }
      "$nats_binary" --version
      export PULSE_NATS_SERVER_BIN="$nats_binary" ARCHIPELAGO_NATS_SERVER_BIN="$nats_binary"
      export CATALYST_TEST_DATABASE_URL="$test_database_url"
      export RUST_TEST_THREADS="${RUST_TEST_THREADS:-4}"
      cd "$FOUNDATION_DEV_ROOT"
      python3 -m unittest discover -s tooling/tests -p 'test_dev_regressions.py'
      python3 -m unittest discover -s tooling/tests -p 'test_enet_provenance.py'
      python3 library/vendor/rusty_enet/verify.py
      python3 tooling/dev-regressions.py
      cargo check --locked --manifest-path library/vendor/rusty_enet/Cargo.toml --all-features --all-targets
      cargo test --locked --manifest-path library/vendor/rusty_enet/Cargo.toml --lib
      make -C "$FOUNDATION_DEV_ROOT" catalyst-test ;;
    e2e) SITES_E2E_NODE_URL="$FOUNDATION_DEV_API_URL" SITES_E2E_PG_URL="$FOUNDATION_DEV_DATABASE_URL" \
         pnpm --dir "$FOUNDATION_DEV_ROOT/apps/sites" test:e2e ;;
    *) echo "unknown suite: $suite (rust|e2e)" >&2; exit 2 ;;
  esac
done
