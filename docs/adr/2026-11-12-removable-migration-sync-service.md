# ADR: Removable production-to-dcl-one synchronization service

- Status: Accepted
- Date: 2026-11-12

## Decision

`services/migration-sync` is a separately deployed migration worker in the
monorepo, built as `dcl-migration-sync`. It belongs to the existing Rust workspace
for dependency locking and verification, but the Catalyst binary does not link
or start it. Its independent deployment and retirement lifecycle justify the
separate service crate. It is not a Catalyst domain module or a library crate.

Production remains authoritative during one-way coexistence. The worker owns
its inbox, stream positions and mapping provenance in `migration_sync` inside
the destination database. These tables join the single `schemas/` bootstrap;
there is no private application schema installer. Target facts and their apply
receipts commit together. Removing the worker does not remove business facts.

The executable supports isolated rehearsals of explicitly scoped Genesis Places
favorites. Source-local transactional outbox capture records changes and command
receipts without contacting dcl-one. A repeatable-read snapshot and capture boundary
bootstrap the destination before activation. Source-only changes apply through the
mapping; duplicate commands run through a dedicated authenticated follower listener,
with native mutations and receipts in one transaction. The ordered coordinator
verifies those receipts without applying the same domain changes again.

Capture and apply run as separately supervised commands with different database
roles. Source installation is an explicit operator command; runtime capture needs
read access only. The minimal `catalyst migration-follower` process starts no normal
API, jobs, mirrors or providers. Its private verification read is not a public API
replacement. Normal `serve`/`index` startup and registration share an advisory guard;
old binaries and independent writers still require deployment-level fencing.

The source outbox is a bounded fallback, not a claim that production logical CDC,
throughput or failover has been verified. The [pilot contract](../../services/migration-sync/pilot-contract.json)
and [runbook](../../services/migration-sync/README.md) define the supported mapping,
recovery controls and remaining activation gates.

## Ownership and retirement

The sync worker may write only its own metadata and the declared destination
mapping. The initial mapping updates `explore.user_favorites` and derives
`explore.place_interaction_counts`; catalogue data remains owned by Explore.
No production-to-target flow may silently become target-to-production writes.
Normal API/jobs must stay off; only the dedicated follower may serve the pilot.

At eventual ownership transfer, fence production writers, drain commands and
effects, synchronize through final source boundaries and validate before
enabling independent target writes. Stop dispatch/capture and the worker only
when their pending state is accounted for. Retain metadata for the agreed
recovery period; retiring replication slots and deleting metadata are explicit
operator actions, not shutdown behavior. After independent target writes,
rollback requires preserving those writes through a separately tested process.
