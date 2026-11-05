# ADR: Referral reward recovery

- Status: Proposed
- Date: 2026-11-04
- Status changed: 2026-11-04

## Context and scope

The baseline is `decentraland/social-service-ea` at
`b6b2cefefdf727b2ed47ddb887e77d226a41eea6`. Referral grants commit before
external rewards and notification effects. The rewards API supplies no durable
idempotency key, so a lost answer cannot establish whether issuance happened.

This proposal covers reward recovery only. The proposed
[trusted proxy ingress](2026-11-04-referral-trusted-proxy-ingress.md),
[Slack budget](2026-11-04-referral-slack-effect-budget.md) and
[schema-v3 activation](2026-11-04-social-schema-v3-activation.md) decisions remain
separate. Implementation and regression tests do not establish acceptance,
source equivalence or deployment readiness.

## Current implementation and proposed choice

`Referral::reward` commits `reward_requested_at` before the non-idempotent POST,
then stores the confirmed `reward_response` before notification/image effects.
A per-invitee advisory guard serializes attempts; its connection closes on drop,
including cancellation. `reward_paid` records the tier, result and known completed
notification while effects remain pending. Completion clears `reward_tier` and
`reward_paid`; the claim and response remain as history.

A claimed request without a response is quarantined. HTTP failures, malformed
replies, cancellation and failure to persist a successful response cannot trigger
an automatic second POST. Configuration failures before claiming remain retryable.
The source commits the grant and does not retry failed rewards; recovery here is
an intentional difference, recorded in the [parity backlog](../../services/catalyst/tests/fixtures/conformance/social/PARITY.md).

With social writes enabled, the fleet-leased `social-referral-rewards` job selects
at most 64 due rows every 60 seconds. It checks the live restore-stamp write hold
before selecting work and before each attempt. Failed attempts defer eligibility
by one minute; bans also block retry. Confirmed responses resume downstream work
without another login or provider POST. Local image, notification/email outbox and pending-state completion now share
one transaction, as specified by the accepted [native events decision](2026-11-04-native-social-events.md). Imported receipts marked notified skip another notice.
Exactly-once external delivery is not claimed.

Migration 0024 imports object-valued historical paid responses, preserving their
notification marker even if image metadata is missing or invalid. Such a receipt
is confirmed; metadata repair may be required to finish image delivery. Older
pending tiers without a confirmed response are quarantined for provider
reconciliation. The normal schema object pass creates the partial due index.
Migration 0023 and its receipts remain unchanged.

## Mandatory deployment boundary

**Do not roll this change out alongside pre-0024 binaries.** Both implementations
use schema version 3, but an older binary interprets non-null `reward_tier` as an
owed POST and ignores the new claim and advisory lock. It can repeat a reward
that the new code has claimed, confirmed or quarantined.

Before migrating 0024, pause referral/login ingress; stop every login consumer,
retry worker and administrative writer; and drain or stop in-flight reward work.
Stop all pre-0024 binaries sharing the database and disable their restart or
rollback before running the compatible `catalyst schema migrate` and schema
status. Resume login delivery only on compatible binaries after
inspecting the migration and quarantined rows. A fleet lease alone does not
prevent an older login consumer from issuing a reward.

This process gate is required even on an already-version-3 database. The migrator
does not enforce it automatically. Do not restart pre-0024 code on the migrated
database; use a reviewed forward fix or coordinated restore and provider
reconciliation. SQL restore cannot reverse an external reward. No assertion is
made that pre-0024 code has never run in production. See [DATABASE.md](../../DATABASE.md)
for the writer-stopped sequence.

## Tradeoffs and validation boundaries

Quarantine can hold a reward the provider never issued. Clearing claims without
provider evidence would risk duplicate issuance; confirmed-response retries avoid
stranding recoverable downstream failures. No automatic reconciliation endpoint
or provider idempotency guarantee is assumed. Revisit this choice if the provider
adds a durable idempotency or reconciliation contract.

Local tests cover concurrency, restart, cancellation, unknown responses,
notification/image failures, restore holds and populated additive upgrades.
Deployment still requires verified backups/restores, configured providers, reconciliation of quarantined rewards and evidence that all old
processes stopped. No acceptance or rollout approval is recorded here.
