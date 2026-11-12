# Migration sync worker

A separately deployable, removable worker for **production → dcl-one**
synchronization. It does not run inside the gateway or the Catalyst API process.
Build it from this monorepo; the serving backend does not depend on it.

## Implemented scope

- Durable transaction inbox with immutable stream generation and snapshot boundary.
- Strictly ordered apply through predecessor positions; missing predecessors wait.
- Identical replay is a no-op; conflicting replay, forks and stale generations fail.
- Places favorites upsert/delete for canonical wallet and Genesis-place UUID keys.
- Fact changes, deletion/version history, derived counts and checkpoints commit atomically.
- Multiple worker processes serialize on the stream row. Failed application rolls
  back the whole domain transaction and retains a blocked head.
- Pause, resume, status and SIGINT/SIGTERM shutdown. Pausing waits for in-flight
  application; shutdown never deletes history or advances an unfinished transaction.

This increment is **for isolated rehearsals only**. `--rehearsal` acknowledges
that prerequisite; it does not disable another process's writers. Keep the mapped
Catalyst APIs, jobs and mirrors off and use isolated storage/providers. There is
no live WAL connector, consistent-snapshot exporter, gateway dispatcher, backend
receipt protocol or automatic follower-mode enforcement yet. Envelopes marked
`dual` are durably blocked with `dual_receipts_required`, never assumed equivalent.

Shop, credits, marketplace favorites, worlds and arbitrary table/SQL replication
are outside this executable's mapping. Only one Places stream can own the scope.

## Build and schema

From the repository root:

```sh
cargo build --locked --manifest-path services/catalyst/Cargo.toml -p dcl-migration-sync
nix build .#migration-sync         # separate package
nix build .#image-migration-sync   # separate image; not started automatically
```

Use the normal Catalyst fresh-schema or additive schema migration procedure to
install `schemas/70_migration_sync.sql` with the rest of the destination schema.
The worker does not create tables on startup. For existing databases the schema
migrator's object pass creates these new objects; no existing table is altered.
The baseline must already contain the pilot's canonical, non-mirrored Genesis
catalogue and migrated facts. Registration records an operator-established
boundary; it does not prove a source snapshot or capture stream is complete.

## Run an isolated rehearsal

Provide a destination-only connection using `MIGRATION_SYNC_DATABASE_URL`.
Use a role restricted to `migration_sync`, reading the mapped Explore catalogue
and likes, and changing favorites and interaction counts. It needs no production
credentials, DDL permissions or access to financial tables. Runtime credentials
are not built into the package/image.

```sh
export MIGRATION_SYNC_DATABASE_URL='postgres://USER:PASSWORD@HOST/ISOLATED_DB'
dcl-migration-sync register --stream places --generation 11111111-1111-4111-8111-111111111111 --baseline-lsn 0/10
dcl-migration-sync enqueue transaction.json
dcl-migration-sync status --stream places
dcl-migration-sync resume --stream places --rehearsal
dcl-migration-sync run --stream places --rehearsal --once
# Omit --once for the separately supervised polling worker:
dcl-migration-sync run --stream places --rehearsal --poll-ms 1000
# From another process:
dcl-migration-sync pause --stream places
```

Streams start paused. `run --once` processes at most one source transaction;
it reports failure for a blocked head. `status` reports positions, queue counts
and a non-sensitive error code, not payloads. Paused/empty/gapped streams are idle;
use status to distinguish those states. A database error stops the process;
restart safely after resolving it. Resume does not clear blocked transactions,
skip missing predecessors or change the snapshot generation.

## Input contract

`enqueue` reads one JSON file, at most 1 MiB and 1,000 ordered mutations. This is
trusted normalized capture input, not raw WAL and not a public request body.
`previous_lsn` is the prior captured transaction in this stream, not an assumption
that PostgreSQL LSNs are consecutive. The future connector must establish that
chain, preserve transaction boundaries and map source identities correctly.
An empty transaction may advance the captured boundary when explicitly accounted for.

```json
{
  "version": 1,
  "source": "places",
  "stream": "places",
  "generation": "11111111-1111-4111-8111-111111111111",
  "previous_lsn": "0/10",
  "commit_lsn": "0/20",
  "mode": "source_only",
  "changes": [{
    "op": "upsert",
    "user": "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    "entity_id": "22222222-2222-4222-8222-222222222222",
    "user_activity": 0,
    "created_at": "2026-11-12T00:00:00Z"
  }]
}
```

The example entity must exist in the imported catalogue. A delete carries only
`op`, `user` and `entity_id`; it also works after its catalogue entry was removed.
Counters are recomputed from accepted favorite/like facts, never incremented
again from a captured counter. Canonical keys and source-only provenance must be
established by the capture adapter: its mode field is not automatic writer detection.

All transaction payloads remain retained for replay conflict detection. Restrict
access and account for their storage/privacy cost. No automatic pruning or stream
reset is implemented. Retiring/rebuilding a generation requires an explicit,
validated baseline transition; old envelopes must never replay over a newer baseline.

## Verification

```sh
cargo test --locked --manifest-path services/catalyst/Cargo.toml -p dcl-migration-sync
# Opt into DB tests using a disposable server with CREATEDB permission:
MIGRATION_SYNC_TEST_DATABASE_URL='postgres://USER:PASSWORD@HOST/postgres' \
  cargo test --locked --manifest-path services/catalyst/Cargo.toml -p dcl-migration-sync
```

The PostgreSQL test creates and removes its own uniquely named database. Without
the explicit test URL, only unit checks run locally; CI requires the URL. Tests
cover replay, gaps, deletion ordering, fork rejection, immutable baselines,
transaction rollback, duplicate-mode refusal and competing worker instances.
They do not establish live CDC continuity, production load budgets or end-to-end
hybrid readiness.

See the [deployment decision](../../docs/adr/2026-11-12-removable-migration-sync-service.md)
for ownership and retirement. No production services are started by installing
this package.
