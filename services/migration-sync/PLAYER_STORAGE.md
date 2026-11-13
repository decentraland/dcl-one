# Player storage replay rehearsal

The `player` lane captures accepted commands inside World Storage, including
multiplayer-origin calls that do not use the public gateway. It replays set,
per-key delete, player clear and scene clear through the same native SQL
operation used by the target's storage handlers. It does not run scenes, copy
unreceipted rows, or enable world/env storage replication.

The decoded accepted value serialization is bounded to 1 MiB; the command envelope
allows worst-case JSON string escaping plus bounded metadata. Oversized envelopes
are rejected in the source query before transferring their payload to the worker.
JSON nesting is validated by PostgreSQL rather than a second, stricter Rust parser.

Production remains the authorization/quota authority. Replay verifies the
accepted command's shape and original resolved world/place/scene binding; it
does not reuse expired delegations or recalculate later production policy.
Target mutation, receipt and applied cursor commit in one transaction. The
source generation plus transaction position is the retry identity. Unknown
source writes or changed identities stop the ordered stream.

Source acceptance integration: [World Storage #130](https://github.com/decentraland/world-storage-service/pull/130). Its opt-in adapter and the bundled source SQL must use the same contract version.

## Scope and preparation

Use only explicit lowercase `.eth` worlds with a nonnil Places UUID and a fixed
active scene entity. Genesis/legacy/many-to-one namespace mappings are excluded.
Source and target scenes must remain fixed during this rehearsal. Source scope
is sealed at installation; changing it requires a separately reviewed baseline.
Normal target `serve` and `index` startup is fenced while either player or Worlds
streams are registered, including paused streams. Stop older binaries and
independent writers too; the guard is not a database-role permission boundary.

Stop source writers before installation and deploy the matching opt-in World
Storage code before restarting any writer. Mixed old/new writer lock ordering
is unsupported. Install with a dedicated owner; the runtime app must not own or
alter capture tables, functions or triggers.

Create a scope file (operator input, not a credential):

```json
[
  {
    "world_name": "example.dcl.eth",
    "place_id": "11111111-1111-4111-8111-111111111111",
    "scene_id": "<accepted-scene-entity-id>"
  }
]
```

Configure `MIGRATION_SYNC_DATABASE_URL` for the isolated target and
`MIGRATION_SYNC_SOURCE_DATABASE_URL` for the World Storage source via supervisor
secrets. Apply the normal target schema migrations first. Then:

```sh
dcl-migration-sync install-player-source --scope-file scopes.json
```

The installer runs [player.sql](source/player.sql), inserts the scope and seals
it atomically. Reinstallation requires exactly the same sealed scope and version.
Give the source runtime role schema usage and execution only on
`migration_player_capture.lock_capture()` and
`migration_player_capture.apply_command(text,text,uuid,text,text,text,text)`;
retain its existing business-table rights. Give the capture reader schema usage
and SELECT on `control`, `scope` and `transactions`. Bootstrap additionally needs SELECT on `public.player_storage`; use a separate baseline role or temporarily grant it, then revoke it for polling. Do not grant either role
capture-table modification or execution of internal helpers. Enable source
`MIGRATION_PLAYER_CAPTURE_ENABLED=true` only after this installation and grants.

## Baseline and operation

Run the existing Worlds/content/place/storage baseline migration with source
writers stopped. Player bootstrap **verifies**, rather than overwrites, the
selected imported rows against a source snapshot established under the capture
lock. Values, sizes and original timestamps must match exactly. It also rejects
missing, retired, replaced or mirrored target scene bindings. A mismatch leaves
no registered stream. The bounded verification permits at most 100,000 rows and
64 MiB of JSON text per selected scene; larger scopes require a reviewed
streaming bootstrap before use.

```sh
dcl-migration-sync player bootstrap --stream player-storage
# Supervise independently with the read-only source role:
dcl-migration-sync player capture --stream player-storage --poll-ms 1000
# Explicit isolated rehearsal acknowledgment:
dcl-migration-sync player resume --stream player-storage --rehearsal
dcl-migration-sync player run --stream player-storage --rehearsal --poll-ms 1000
dcl-migration-sync player status --stream player-storage
dcl-migration-sync player pause --stream player-storage
```

Capture and run support `--once`. Capture continues while apply is paused. Status
reports generation, baseline/captured/applied positions, last source observation
and blocked count without player values. Commands and receipts contain player
values: restrict database access and establish retention before production use.
There is no automatic deletion or cursor skip. A blocked head needs investigation
and a reviewed recovery procedure; don't edit inbox payloads to force progress.

This lane needs no new gateway route or externally reachable replay endpoint.
The worker calls the shared database operation on the isolated target. There
are no live target storage caches during rehearsal; regular handlers retain
native cache invalidation. Public serving, moving scene identities, failure/load
budgets and multi-day source retention remain rollout gates.
