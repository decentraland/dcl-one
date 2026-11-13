# One-way migration service

A separately deployable, removable **production → dcl-one** coordinator. Production
remains the write authority and owns jobs and external effects. The gateway returns
production's result. Its production transaction records the command durably; the
coordinator executes the equivalent native target command in source order. Residual mappings are POI membership and bounded Worlds owner/quota outcomes; neither has an equivalent compatible inbound request. Unrecognized changes block progress. Compatible operations use native execution, never continuous interaction-row copying.

## Supported rehearsals

Favorites, tri-state votes and trusted POI-job membership for explicitly selected Genesis Places UUIDs. Shop and credits are
excluded for security reasons, including marketplace imports. Additional opt-in rehearsals cover accepted World Storage player commands and Worlds owner/quota outcomes. World/env storage, deployments, social/voice, notifications, other aggregates and arbitrary SQL replication are not implemented.
The asset-bundle rewriter is unrelated and unchanged.

The Places [executable scope contract](pilot-contract.json) lists the command scope and activation gates.
The migration decision document under [`sync-migration`](../../sync-migration/README.md) owns strategy and exceptions.

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

## Build and prerequisites

```sh
cargo build --locked --manifest-path services/catalyst/Cargo.toml -p dcl-migration-sync
nix build .#migration-sync
nix build .#image-migration-sync
```

Use the normal schema migration procedure to install
[worker state](../../schemas/70_migration_sync.sql) and
[capture progress](../../schemas/71_migration_capture.sql). Migrate the canonical
Genesis catalogue before bootstrap. Bootstrap imports a consistent scoped favorite/vote snapshot and aligns the POI bit. It rejects existing target interactions in that scope; omit that scoped
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
# Imports interactions and POI membership at a consistent capture boundary and registers a paused stream.
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

Set worker `MIGRATION_SYNC_FOLLOWER_URL` to the complete URL ending in
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

Enable the Places application's `MIGRATION_CAPTURE_ENABLED=true` only after source
installation. Its authenticated favorite/vote handlers require `x-dcl-operation-id` for captured commands. Stable client IDs remain the default; retries reuse that ID and payload. The gateway can opt into per-attempt IDs for these setters only, preserving each actual source acceptance without deduplicating separate client retries. Enable `MIGRATION_POI_CAPTURE_ENABLED=true` separately for trusted POI-job results after installing version 4; see the [POI contract](../../sync-migration/places-poi-residual.md).
Configure the alb-router hybrid allowlist to match the installed UUID scope. Writes
without that protocol are captured but block the ordered head with
`unreceipted_mutation`; they are never copied into target business tables. Direct independent target writes stay denied.

## Ordering and failure handling

- A command receipt, interaction mutation and source outbox entry commit together.
  Conflicting operation-ID reuse fails; an identical retry returns the stored result.
- Capture persists each complete transaction before advancing its durable offset.
  Snapshot-covered commands never replay. A source generation change or history gap
  stops capture rather than silently starting over.
- One stream lock coordinates native target execution, approved POI outcomes and progress.
  Native state and receipt commit together. The coordinator validates that receipt
  and advances without applying those captured interaction rows again.
- A timeout may hide success. Retry the same pointer and resolve the durable target
  receipt. Later deletes cannot be overtaken by a delayed earlier command.
- Unreceipted mutations or native invariant failures block the head. No later transaction skips it.
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
checkpoints or domain data. Revalidation can block it again. Retrying an unreceipted
mutation cannot approve it or manufacture a command receipt. Resolving that gap
requires a reviewed recovery decision or new consistent baseline. There is no skip,
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
Reconciliation compares scoped favorite/vote rows, favorite counts and POI membership, and rejects a moving source or target boundary. It never repairs data; catalogue/effects/API equivalence still
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
Those remain activation gates. Add a residual mapping only after documenting the
specific writer, why equivalent requests cannot reproduce its result, its exact
fields and side effects, and its validation. A job or internal call is a candidate
for investigation, not automatic permission to copy its tables.

See the [deployment ADR](../../docs/adr/2026-11-12-removable-migration-sync-service.md).

## Worlds and player storage

The new streams use separate `player` and `worlds` CLI subcommands and immutable source scopes. They verify an already imported baseline at a consistent source capture boundary, retain committed source transactions, then apply them with atomic progress. Normal server/index startup is forbidden while any migration stream is registered. No public target storage endpoint is enabled by these commands.

Player replay uses the same native SQL mutation as normal storage handlers; production acceptance supplies authorization/quota evidence. World/player namespace identity must remain fixed. The source application change is necessary because direct multiplayer storage requests bypass the gateway. No client or multiplayer-service change is required for this path. Source capture is disabled by default and must be installed and granted explicitly before application opt-in.

Worlds source installation uses triggers on owner/quota facts and requires an opt-in source lock-order hook before existing transaction row/advisory locks. Enable it on every writer while writes are paused; mixed old/new ordering is unsupported. It includes post-deployment unblocks and physical quota deletions. The apply worker has no SNS, name-resolution or quota-provider clients. Neither new stream implements eviction or runs GC; normal target jobs remain fenced during rehearsal.

See the [player runbook](PLAYER_STORAGE.md), [Worlds installation](source/worlds.md) and [operation contract](../../sync-migration/worlds-background-contract.md) for commands, scope, source-change rationale and activation limits. Run `dcl-migration-sync player --help`, `worlds --help`, `install-player-source --help` and `install-worlds-source --help` for the commands. Keep source installation credentials separate from read-only capture credentials and the isolated target writer.
