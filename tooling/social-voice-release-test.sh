#!/usr/bin/env bash
# Run from the locked nix develop shell. This proof owns its entire disposable cluster.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
for binary in postgres initdb pg_ctl psql cargo; do
  command -v "$binary" >/dev/null || { echo "Release proof requires $binary from nix develop" >&2; exit 1; }
done
if [[ "$(postgres --version)" != 'postgres (PostgreSQL) 18.6' ]]; then
  echo 'Release proof requires the locked PostgreSQL 18.6 toolchain' >&2
  exit 1
fi
export PSQL="$(command -v psql)"

# Ignore ambient connections. All clients receive the owned socket URL below.
unset DATABASE_URL CATALYST_TEST_DATABASE_URL PGHOST PGHOSTADDR PGPORT PGDATABASE PGUSER
unset PGPASSWORD PGPASSFILE PGSERVICE PGSERVICEFILE PGOPTIONS PGSSLMODE
work="$(mktemp -d /tmp/catalyst-release.XXXXXX)"
export PGDATA="$work/data"
socket="$work/socket"
cleanup() {
  result=$?
  trap - EXIT HUP INT TERM
  if [[ -s "$PGDATA/postmaster.pid" ]]; then
    if ! pg_ctl -D "$PGDATA" -m immediate -w -t 30 stop; then
      echo "Unable to stop the owned release-proof cluster; preserving $work" >&2
      exit 1
    fi
  fi
  if [[ $result -ne 0 && -f "$work/postgres.log" ]]; then
    tail -n 100 "$work/postgres.log" >&2
  fi
  rm -rf -- "$work"
  exit "$result"
}
trap cleanup EXIT
trap 'exit 129' HUP
trap 'exit 130' INT
trap 'exit 143' TERM

mkdir "$socket"
initdb -D "$PGDATA" --username=release_test --auth-local=trust --auth-host=reject --encoding=UTF8 --locale=C --no-instructions
cat >> "$PGDATA/postgresql.conf" <<EOF
listen_addresses = ''
unix_socket_directories = '$socket'
unix_socket_permissions = 0700
max_connections = 40
# Captured schemas acquire thousands of object locks in one transaction.
max_locks_per_transaction = 1024
log_min_error_statement = panic
EOF
pg_ctl -D "$PGDATA" -l "$work/postgres.log" -w -t 30 start
export CATALYST_TEST_DATABASE_URL="postgresql://release_test@localhost/postgres?host=$socket"
touch "$work/pgpass"
chmod 600 "$work/pgpass"
export PGCONNECT_TIMEOUT=10 PGPASSFILE="$work/pgpass"
export RUST_TEST_THREADS="${RUST_TEST_THREADS:-2}"
version="$("$PSQL" -X --no-password "$CATALYST_TEST_DATABASE_URL" -v ON_ERROR_STOP=1 -Atqc 'SHOW server_version_num')"
if [[ "$version" != 180006 ]]; then
  echo "Release proof requires PostgreSQL 18.6; server returned $version" >&2
  exit 1
fi
# Scratch databases inherit template1. Match the existing development harness's
# Darwin alias for the collation required by the production bootstrap check.
"$PSQL" -X --no-password "postgresql://release_test@localhost/template1?host=$socket" -v ON_ERROR_STOP=1 \
  -c "CREATE COLLATION IF NOT EXISTS pg_catalog.\"en_US.utf8\" (provider=libc, locale='en_US.UTF-8')"
# Availability and scratch-database privileges must hold before any test can run.
"$PSQL" -X --no-password "$CATALYST_TEST_DATABASE_URL" -v ON_ERROR_STOP=1 \
  -c 'CREATE DATABASE catalyst_release_ci_probe' -c 'DROP DATABASE catalyst_release_ci_probe'

cd "$root"
cargo test --manifest-path services/catalyst/Cargo.toml --locked -p catalyst-db --test social_voice_release
