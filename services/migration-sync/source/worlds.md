# Worlds owner and quota capture

This opt-in rehearsal adapter copies only committed owner changes and wallet quota block facts. It does not execute ownership lookups, quota calculations, SNS publication, deployment, scene eviction or gameplay. All scoped source writers are observed, including SQL changes and unblocks after deployment/undeployment.

## Workload and invariants

One source-local statement lock orders writes to `worlds` and `blocked`; each transaction with scoped changes gets one contiguous position. The target applies the complete transaction with its cursor, or records a blocker without changing either. Capture and apply are independently restartable. The source never contacts the target. Lock contention, storage growth and recovery throughput require staging measurements; no production capacity claim is made.

The first scope is 1–1,000 canonical world names and 1–1,000 canonical wallet addresses. Every scoped world must already exist, with an owner in the wallet allowlist. Include possible transfer recipients explicitly. Transfer outside that list, world rename/deletion and noncanonical keys stop apply. A target wallet must not own worlds outside the approved target world scope. Scope/generation changes require a new reviewed baseline, never editing a running capture generation.

Source transactions above 1,000 changes or 1 MiB stop capture before progress advances. A physical `TRUNCATE` is refused while triggers are installed. Ordinary outside-scope changes create no captured facts, but share the statement lock. Source DDL, trigger disabling and privileged manual writes are operator-controlled boundaries, not solved by this adapter.

## Install and bootstrap

Use the existing worker's `MIGRATION_SYNC_SOURCE_DATABASE_URL` and `MIGRATION_SYNC_DATABASE_URL` secret environment variables. The CLI integration exposes installation and the `worlds` command group; consult `dcl-migration-sync --help` and `dcl-migration-sync worlds --help` for argument names.

1. Stop normal target services, jobs, indexers and old binaries. The common migration startup lock fences current binaries, not arbitrary SQL or older releases.
2. Run `catalyst schema migrate` with the target administrative role. `72_worlds_migration.sql` introduces only new objects, which the additive schema migrator creates; it changes no existing business table.
3. Install `source/worlds.sql` through the worker's source installation operation with the explicit world/wallet allowlists. Installation is one source transaction; repeating it requires exactly the same scope. Do not run raw installation SQL against an existing capture schema.
4. Complete the initial Worlds migration. Bootstrap verifies exact source/target owners and quota timestamps at a consistent source boundary and registers a paused stream. It does not overwrite existing target business facts. If concurrent writes prevent an exact match, quiesce source writers for the initial import/bootstrap; do not force a mismatched cursor.
5. Start capture with read-only source credentials. Resume target application with `--rehearsal`. Keep normal target jobs/providers off throughout coexistence. Status reports baseline/capture/apply positions, pause state, blocker and source contact age.

With credentials supplied by the supervisor, a bounded example is:

```sh
dcl-migration-sync install-worlds-source --world example.dcl.eth --wallet 0x1111111111111111111111111111111111111111
dcl-migration-sync worlds bootstrap --stream worlds
dcl-migration-sync worlds capture --stream worlds --poll-ms 1000
dcl-migration-sync worlds resume --stream worlds --rehearsal
dcl-migration-sync worlds run --stream worlds --rehearsal --poll-ms 1000
dcl-migration-sync worlds status --stream worlds
dcl-migration-sync worlds pause --stream worlds
```

Run capture and apply as separate supervised processes; `--once` supports rehearsal assertions. The example identities must be replaced with the reviewed source scope.

Capture continues while application is paused. A transient database/network failure can be retried; position and generation checks prevent skipping missing history. A projection conflict records `worlds_projection_conflict` and pauses the stream. Resume cannot clear that blocker. Investigate and reconcile or establish a new reviewed baseline; never skip a transaction by editing progress.

## Roles

Installation requires an owner/DDL-capable source role to create the private schema and triggers on `public.worlds` and `public.blocked`. Trigger functions use that owner with a fixed search path and fully qualified relations. PUBLIC has no capture-schema, table, sequence or function privileges. Existing application DML invokes the installed triggers without needing access to the private outbox.

The runtime source reader needs `CONNECT`, schema `USAGE` on `public` and `migration_worlds_capture`, and `SELECT` on:

- `public.worlds` and `public.blocked` for baseline verification;
- `migration_worlds_capture.control`, `world_scope`, `wallet_scope`, `transactions` and `changes` for capture.

It needs no DML, DDL, sequence or outbox-function execution grants. Bootstrap's advisory lock is session/transaction coordination, not a row write. Set finite statement and lock timeouts. The target worker needs the target migration tables plus narrowly scoped reads/owner updates on `worlds.worlds` and CRUD on `worlds.quota_blocks`; it needs no provider secrets or platform-ban privileges. DB role grants remain deployment configuration, not automatic installation grants.

## Retention and shutdown

No outbox pruning is implemented. Retain source history and target inbox for the recovery horizon; alert on growth. Stop capture/apply without deleting business facts. Before final handover, fence source writers, capture and apply the final position, reconcile owners/quota blocks, and explicitly retire migration ownership before allowing normal target jobs. Source trigger removal and evidence archival are separate operator steps after that boundary.

Block creation timestamps preserve the original grace period; refresh only changes the recorded update timestamp. Deletes and delete/reblock episodes remain distinct. No quota outcome is ever written to the platform-wide ban table.
