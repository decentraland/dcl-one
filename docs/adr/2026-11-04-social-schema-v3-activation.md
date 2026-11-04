# ADR: Activate schema version 3 and retire historical step 0020

- Status: Proposed
- Date: 2026-11-04

## Context

The Social runtime keys community voice START notifications by the source's
exact raw identifier and allows several pending and media private calls per
wallet. Version 2 stores the notification on `social.communities` (historical
step `0020_social_voice_notifications`) and enforces one call per caller and
callee with unique constraints. Changing that layout drops constraints and
columns, which the [database contract](../../DATABASE.md) allows only in a
reviewed release. The [dormant release ADR](2026-11-04-dormant-schema-release-definitions.md)
keeps the canonical definitions in `schemas/releases/v3/` and defers
activation. Two version-2 origins exist: shipped main without step 0020, and
databases prepared at the captured build with its genuine receipt.

## Decision

The catalyst binary requires schema version 3. Its fresh bootstrap enrolls
`schemas/releases/v3/41_social_voice_notifications.sql` and
`42_social_private_voice_indexes.sql` once, in dependency order. An existing
version-2 database moves with the transactional release
`schemas/releases/v3-private-voice-pending.sql`, run with `psql` while every
writer is stopped, followed by `catalyst schema migrate` and
`catalyst schema status`.

From version 3, the exact historical step 0020 is retired: apply and migrate
never execute it and never invent its receipt. Status and bootstrap still
verify a genuine receipt's hash and refuse the five retired columns,
constraint and index if they reappear. Version-2 binaries refuse version 3,
and version-3 binaries refuse version 2.

## Consequences

- Cutover requires stopped writers, a verified backup restore, the release,
  the migrator and a clean status, in that order.
- Steps 0021 to 0023 run as additive migrations after the release; on shipped
  main, 0021 rewrites `social.referral_progress` (measured in METRICS.md).
- Returning to version 2 is a restore, not a reverse script: version 3 can hold
  several pending or media calls per wallet that the old constraints reject.
- Owned upgrade tests cover both genuine origins with populated rows,
  interruption at each release boundary, retry and the real migrator.

## Alternatives

- Keep version 2 and emulate the new behaviour on the old layout: the unique
  constraints forbid concurrent pending and media calls the source allows.
- Ship the layout change as an additive migration: it drops constraints and
  columns, which only a reviewed release may do.
- Delete or rewrite historical step 0020: breaks the genuine receipts of
  prepared databases, which migrate refuses by design.
