# Events schedules synchronization

## Decision and scope

Synchronize a reviewed UUID allowlist from the Events database's `public.schedule`
table into `explore.schedules_local`, production → dcl-one only. This is a bounded
schedule metadata mapping, not synchronization of events, attendance, profiles,
notifications or all future schedules. Production remains the only request and job
authority. Clients and Events HTTP handlers stay unchanged.

The source implementation was inspected at
[Events `770e43f8baaf47844bf4ff5768e5493a380508bc`](https://github.com/decentraland/events/tree/770e43f8baaf47844bf4ff5768e5493a380508bc).
The [source migration](https://github.com/decentraland/events/blob/770e43f8baaf47844bf4ff5768e5493a380508bc/src/migrations/1654102204538_schedules-to-db.ts),
[nullable-field change](https://github.com/decentraland/events/blob/770e43f8baaf47844bf4ff5768e5493a380508bc/src/migrations/1667215766493_make-props-optional-on-schedules.ts)
and [theme column](https://github.com/decentraland/events/blob/770e43f8baaf47844bf4ff5768e5493a380508bc/src/migrations/1667216550571_add-theme-prop-to-schedule.ts)
define the captured fields. Deployed source versions remain unverified.

## Why this subset

Schedules have direct UUID identities and no external effects in their reviewed
create/PATCH handlers. The [existing batch mapping](../tooling/migration/sources/events/schedule.load.sql)
already establishes the field conversion. The continuous path additionally needs
commit ordering, deletes and a shared snapshot boundary; a timestamp-based batch
refresh cannot provide those guarantees.

The reviewed [PATCH handler](https://github.com/decentraland/events/blob/770e43f8baaf47844bf4ff5768e5493a380508bc/src/entities/Schedule/routes.ts)
does not explicitly advance `updated_at`. Therefore synchronization uses committed
outbox positions, preserves the source timestamps verbatim and does not reject an
otherwise newer committed change merely because its timestamp is unchanged.

## Mapping and invariants

| Source | Target | Reason |
| --- | --- | --- |
| UUID `id` | Canonical UUID text `id` | Preserve identity; do not generate a target ID. |
| Name, nullable description/image/theme | Same semantic fields | Preserve nulls and stored theme values rather than inventing defaults. |
| `varchar(30)[]` background | JSON array of strings | Match the established source-to-target representation. |
| Four `timestamptz` fields | UTC `timestamp without time zone` | Preserve the source instant and microsecond precision. |
| Active flag and active interval | Same values | Do not reinterpret expiration or re-run source validation. |
| Row DELETE or primary-key change | Explicit deletion; delete/upsert in source order | Preserve removals and prevent old keys being left behind. |

Validation rejects noncanonical/nil UUIDs, values exceeding source column bounds,
background arrays over the bounded capture limit, unknown fields and timestamps
more precise than the source database. Unsupported input stops the stream rather
than silently truncating data. Source array null elements are unsupported and must
be resolved explicitly if found during baseline validation.

Snapshot reads check scoped source array cardinality in PostgreSQL before fetching
rows: `varchar(30)[]` limits each string but does not limit the number of elements.
The limit is 1,000 elements. Target reconciliation also checks JSON array shape,
element count and a 1 MiB serialized-background ceiling before decoding, because
the target JSON column does not impose the source string bounds. Callers must keep
each check and its row fetch in the same consistent database snapshot so a
concurrent write cannot bypass the preflight. Oversized data is rejected for review.

The [typed mapping](../services/migration-sync/src/events_schedules.rs) applies facts
inside the coordinator's transaction. It does not mutate event membership arrays,
produce notification intents, execute HTTP handlers or start jobs. Delete/recreate
ordering and per-key versions belong to the coordinator, not a wall-clock rule.

## Capture and deployment

The [source installation](../services/migration-sync/source/events_schedules.sql)
creates `migration_capture_events` with adapter `events_schedules_outbox_v1`.
Statement triggers serialize schedule writes locally, row triggers append scoped
changes transactionally, and TRUNCATE is refused. Mixed updates share their source
transaction. No source function calls dcl-one.

The runtime source role reads the outbox and scoped facts; installation needs DDL
privileges, and each source writer needs explicit schema/function/table/sequence
grants. Functions run as the caller. Missing grants can reject production writes,
so installation and permissions must be coordinated before traffic resumes.
Source lock contention affects all schedule writers, including out-of-scope rows;
capacity remains a staging measurement.

The UUID allowlist is immutable for a generation. The source POST handler generates
new random UUIDs, so an existing allowlist does not automatically include arbitrary
new schedules. Scope expansion needs a reviewed baseline. Capturing selected
existing schedules and their deletion/recreation is intentional pilot coverage,
not a claim that every future schedule will synchronize.

## Why a PR in Events is not required now

No Events application PR is required for source-only row capture. Existing source
authentication and schedule create/PATCH behavior remain authoritative. The dcl-one
PR contains the mapping and capture installer; operators must still review and
apply the source database change. No client operation ID or optional duplicate
command receipt is needed, because schedules are not natively replayed.

An Events application PR would need a distinct reason: for example, expanding to
native command replay would require an atomic acceptance/response receipt and
compatible retry identity. That is not part of this mapping. Do not add generic
synchronization calls to every Events writer.

## Validation and limits

The mapping tests cover typed payload validation, nullable fields, UTC conversion,
source updates with unchanged `updated_at`, false active values, rollback, key
changes, deletes, TRUNCATE refusal and absence of generated notification intents.
The integration test uses the real source column types and current target schema in
disposable databases. Generic stream tests must additionally cover scoped bootstrap,
ordering, restart/replay and cross-source isolation before activation.

No scheduled writer of `public.schedule` was identified in the inspected source;
this does not establish deployed writer inventory. In particular, event recurrence
and notification jobs concern other data and are not covered by this adapter.
Event references to schedule IDs and complete event API compatibility remain a
separate mapping/rehearsal. Do not expose dependent target workflows merely because
schedule rows match.
