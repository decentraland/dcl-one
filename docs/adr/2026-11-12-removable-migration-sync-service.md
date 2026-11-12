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

The initial executable supports isolated rehearsals of normalized Places
favorites transactions. Source-only changes can apply; duplicate-command
transactions stop until transactional backend receipts and execution/repair
arbitration are implemented. The process has no source database connection,
public ingress or provider credentials. Live capture, snapshot export,
gateway dispatch and backend write/job fencing are separate prerequisites.

## Ownership and retirement

The sync worker may write only its own metadata and the declared destination
mapping. The initial mapping updates `explore.user_favorites` and derives
`explore.place_interaction_counts`; catalogue data remains owned by Explore.
No production-to-target flow may silently become target-to-production writes.
Normal API/jobs must be inert for the mapped rehearsal scope until explicit
follower controls exist.

At eventual ownership transfer, fence production writers, drain commands and
effects, synchronize through final source boundaries and validate before
enabling independent target writes. Stop dispatch/capture and the worker only
when their pending state is accounted for. Retain metadata for the agreed
recovery period; retiring replication slots and deleting metadata are explicit
operator actions, not shutdown behavior. After independent target writes,
rollback requires preserving those writes through a separately tested process.
