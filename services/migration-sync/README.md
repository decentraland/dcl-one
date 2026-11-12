# One-way migration service

A separately deployable, removable **production → dcl-one** coordinator. Production
remains the write authority and owns jobs and external effects. Existing clients
continue calling production. The coordinator captures committed favorite and
like/dislike changes and applies them to dcl-one in source order. An optional
receipt-bearing favorite command can exercise native target execution instead;
its captured favorite rows are then verified rather than applied twice.

## Supported pilot

Favorites, likes and dislikes for explicitly selected Genesis Places UUIDs. Shop and credits are
excluded for security reasons, including marketplace imports. Worlds, social/voice,
notifications, other aggregates and arbitrary SQL replication are not implemented.
The asset-bundle rewriter is unrelated and unchanged.

The [executable scope contract](pilot-contract.json) lists mappings and activation gates.

The target's dedicated `catalyst migration-follower` listener starts no normal
services, consumers, mirrors or scheduled jobs. Its authenticated read endpoint is
an internal verification API, **not** a compatible replacement for the public
Places API. Do not send public read traffic to it yet.

Source capture is an opt-in **transactional outbox**, not logical WAL replication.
Triggers write locally in the same production transaction, including direct SQL
writes. A source-local transaction lock establishes commit order; production never
waits for dcl-one, but contending source writers do serialize. Measure that cost
before rollout. Outbox ordinals use the envelope's hexadecimal cursor format;
they are not actual PostgreSQL WAL positions.

## Unchanged-client synchronization

The default rollout path is source-only synchronization. Existing clients call
production normally, with no operation header. No Places application change or
hybrid gateway mode is needed: leave `MIGRATION_CAPTURE_ENABLED` disabled and use
the gateway's ordinary upstream routing, if present. The source database capture
installation is still an explicit change requiring reviewed privileges and load.

The worker captures favorite and like/dislike inserts, updates and deletes,
including internal SQL, in one ordered transaction stream. Target counts are derived
from those facts. Production alone runs scheduled jobs; capture of an actual
scheduled-job dataset beyond these tables remains unproven.

Native favorite replay is optional. Its handler, header and follower replay
credentials are needed only when explicitly exercising native duplicate commands.
For source-only operation, omit `MIGRATION_SYNC_FOLLOWER_URL` and
`MIGRATION_FOLLOWER_TOKEN` from the apply worker. The private follower listener may
still be used separately for verification reads.

This coverage requires `places_outbox_v2` and a new consistent interaction baseline.
A favorites-only v1 source/stream is not upgraded or expanded in place. Retain its
history, stop the rehearsal and establish a reviewed clean v2 baseline. The target
schema migration permits both adapter identities without relabeling old rows.

## Build and prerequisites

```sh
cargo build --locked --manifest-path services/catalyst/Cargo.toml -p dcl-migration-sync
nix build .#migration-sync
nix build .#image-migration-sync
```

Use the normal schema migration procedure to install
[worker state](../../schemas/70_migration_sync.sql) and
[capture progress](../../schemas/71_migration_capture.sql) and
[like version history](../../schemas/72_migration_like_versions.sql). Apply the normal
additive schema migration for existing databases too. Migrate the canonical
Genesis catalogue before bootstrap. Bootstrap imports a consistent scoped favorite/like
snapshot and rejects existing target interactions in that scope; omit that scoped
interaction batch from the initial import or use a clean rehearsal destination.
Catalogue changes are not continuously mapped by this pilot: freeze reviewed
catalogue identities or stop the pilot when dependencies change.

Stop all normal destination API/job/mirror processes. New `serve` and `index`
commands share a startup advisory guard with registration/bootstrap; old binaries,
independent SQL writers and lost guard sessions are **not** fenced by that guard.
Keep the destination dedicated, restrict DB roles and ingress, and supervise these
processes separately. `--rehearsal` remains required until rollout gates are proven.

## Start a rehearsal

Credentials belong in the supervisor's secret environment, not CLI arguments or
images. `MIGRATION_SYNC_DATABASE_URL` targets dcl-one. Source commands additionally
use `MIGRATION_SYNC_SOURCE_DATABASE_URL`. Installation uses a separate DDL-capable
source role; subsequent capture uses a read-only source role. See
[source installation and grants](source/README.md).

```sh
# Explicit source mutation: install only in the approved source rehearsal DB.
dcl-migration-sync install-source --entity 22222222-2222-4222-8222-222222222222
# Imports favorites at a consistent capture boundary and registers a paused stream.
dcl-migration-sync bootstrap --stream places
# Copy committed source transactions; supervise separately from application.
dcl-migration-sync capture --stream places --poll-ms 1000
```

Bootstrap prints the immutable generation. Start the dedicated target listener
with that value and its own `MIGRATION_FOLLOWER_DATABASE_URL` plus a secret
`MIGRATION_FOLLOWER_TOKEN` of at least 32 bytes:

```sh
catalyst migration-follower --host 127.0.0.1 --port 5213 --stream places --generation GENERATION
```

For optional native favorite replay, set worker `MIGRATION_SYNC_FOLLOWER_URL` to the complete URL ending in
`/internal/migration/favorites/replay`, and the same bearer token. HTTPS is required
except for loopback rehearsal. Redirects are disabled. The replay body contains
only stream, generation and cursor; the target reads the immutable command from
its captured inbox. Original client signatures are never forged or retained.

```sh
dcl-migration-sync resume --stream places --rehearsal
dcl-migration-sync run --stream places --rehearsal --poll-ms 1000
dcl-migration-sync status --stream places
dcl-migration-sync check --stream places --max-age-seconds 30
dcl-migration-sync reconcile --stream places
dcl-migration-sync metrics --stream places
```

For optional native favorite replay only, enable the Places application's `MIGRATION_CAPTURE_ENABLED=true` only after source
installation. Its authenticated favorite handler requires a stable client-generated
`x-dcl-operation-id` for captured commands. Client retries reuse that ID and payload.
Configure the alb-router hybrid allowlist to match the installed UUID scope. Writes
without that protocol still produce source-only changes through capture; they do
not prove native command compatibility. Direct independent target writes stay denied.

## Ordering and failure handling

- A command receipt, favorite mutation and source outbox entry commit together.
  Conflicting operation-ID reuse fails; an identical retry returns the stored result.
- Capture persists each complete transaction before advancing its durable offset.
  Snapshot-covered commands never replay. A source generation change or history gap
  stops capture rather than silently starting over.
- One stream lock coordinates native target execution, transformed SQL and progress.
  Native state and receipt commit together. The coordinator validates that receipt
  and advances without applying those captured favorite rows again.
- A timeout may hide success. Retry the same pointer and resolve the durable target
  receipt. Later deletes cannot be overtaken by a delayed earlier command.
- Mapping or native invariant failures block the head. No later transaction skips it.
  Transport failures leave the head pending. Never fall back to independent target
  execution if production is unavailable.

Pause waits for the current application transaction. Capture can continue while
application is paused. SIGINT/SIGTERM stops each worker without deleting history.
After fixing the cause, an operator can retry the exact immutable blocked head:

```sh
dcl-migration-sync pause --stream places
dcl-migration-sync status --stream places
dcl-migration-sync retry-blocked --stream places --expected-cursor 0/2
dcl-migration-sync resume --stream places --rehearsal
```

Retry changes only its blocked status; it does not alter payloads, receipts,
checkpoints or domain data. Revalidation can block it again. There is no skip,
automatic repair, destructive reset or automatic history pruning.

## Operating and retiring

`status` separates observed source, durable capture and applied positions, pending
receipts, blocked rows and oldest unapplied age. `metrics` emits Prometheus text for
a supervised textfile collector. Export atomically; alert on missing collector
samples too. Example rehearsal alert rules are in [alerts.yml](alerts.yml); their
thresholds are examples, not measured production budgets.

Readiness requires current source contact, matching generation, fully caught-up
capture/application, no blocked head and an unpaused stream. Idle capture polls
refresh contact. Readiness is sampled, not linearizable cross-database consistency.
Reconciliation compares scoped favorite/like rows and target interaction counts derived
from source facts, and rejects a moving source or target boundary. It never repairs data; catalogue/effects/API equivalence still
need separate verification.

For a repeatable stable-window soak, run `scripts/soak.sh places 1440 60` with
the worker binary and read-only source credentials in the environment. It records
status, metrics, health checks and boundary reconciliation each minute and stops on failure;
it does not start writers or claim a production workload was exercised.

For retirement, disable hybrid ingress, preserve production authority, fence source
writers at final cutover, capture through the final boundary, drain and reconcile.
Stop follower ingress and both workers before transferring write/job ownership.
Archive receipts and tombstones for the agreed retry/outage horizon. Removing
capture triggers or deleting history is an explicit operator migration after that
proof; this service does not do it automatically. Once dcl-one accepts independent
writes, rollback to production needs a separate preservation plan.

## Verification and remaining gates

```sh
MIGRATION_SYNC_TEST_DATABASE_URL='postgres://USER:PASSWORD@HOST/postgres' \
  cargo test --locked --manifest-path services/catalyst/Cargo.toml -p dcl-migration-sync
CATALYST_TEST_DATABASE_URL='postgres://USER:PASSWORD@HOST/postgres' \
  cargo test --locked --manifest-path services/catalyst/Cargo.toml -p catalyst-explore migration_follower
```

DB tests create uniquely named disposable databases; the test role needs CREATEDB.
Without the explicit URL, DB tests are skipped locally. Local tests do not establish
production restart/failover behavior, deployed role grants, source load budgets,
public read compatibility, real scheduled-job coverage or a multi-day soak.
Those remain activation gates. Expand table mappings only after an actual job-owned
and coupled permission/notification workflow passes equivalent fault tests.

See the [deployment ADR](../../docs/adr/2026-11-12-removable-migration-sync-service.md).
