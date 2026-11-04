# ADR: Social reward recovery and replica state

- Status: Proposed
- Date: 2026-11-04
- Status changed: 2026-11-04

## Context

The compatibility baseline is `decentraland/social-service-ea` at
`b6b2cefefdf727b2ed47ddb887e77d226a41eea6`. Its standalone deployment, forwarding
headers, provider initialization and failure timing do not directly define a
safe configuration for every replica of a combined backend. Referral writes
also cross PostgreSQL, a reward provider and notification delivery; a committed
grant cannot make those external effects atomic.

This proposal records the current implementation and the choices requiring
acceptance. Implementation and local regression coverage do not establish
source equivalence, acceptance of these differences or deployment readiness.
The [parity backlog](../../services/catalyst/tests/fixtures/conformance/social/PARITY.md)
keeps those distinctions explicit.

## Related proposals and scope

The existing proposals define [trusted proxy ingress](2026-11-04-referral-trusted-proxy-ingress.md),
[bounded Slack effects](2026-11-04-referral-slack-effect-budget.md) and
[schema-v3 activation](2026-11-04-social-schema-v3-activation.md). All remain
Proposed. This document supplements them with reward recovery, provider
activation, replica freshness and native moderation ordering; it does not
supersede those proposals or record acceptance.

## Current behavior and proposal

| Boundary | Pinned baseline or retained local state | Current implementation and proposed choice |
| --- | --- | --- |
| Provider bootstrap | Source standalone components require their settings at initialization. | Permit an unconfigured monolith. Analytics-specific settings activate validation of context, URL, token and `ENV`; shared `ENV` alone, including with an empty posture URL, leaves analytics inactive. Partial configured providers fail boot validation. |
| Referral denylist | The source obtains its configured denylist during referral checks. | Share Explorer's merged mirror/local-overlay snapshot with referral readers. Every serving process reloads from PostgreSQL every 30 seconds with a five-second timeout, independently of mirror and fleet-job flags. Serialize reloads within each process and retain the previous snapshot after failure. |
| Reward recovery | The source commits a grant before reward effects and does not retry failed reward work. A historical pending tier without a paid receipt does not prove whether its reward POST succeeded. | Commit a request claim before the non-idempotent POST and persist its confirmed response before downstream effects. Resume only unclaimed or confirmed-response work automatically; quarantine ambiguous claims for provider reconciliation. |
| Native signed moderation | Native federation is a separate contract from source client mutations. Role actions carry `(signed_at, signature_hash)` ordering. | Ban and unban supersession ignores target-issued self actions, so an allowed future self-join cannot postpone moderation. Ordinary role changes still compare the complete action order, including joins and leaves. |

The [analytics constructor](../../services/catalyst/social/src/analytics.rs)
and [provider bootstrap validation](../../services/catalyst/config/src/social.rs)
keep optional operation separate from incomplete configuration. Without an
OpenAI key, the monolith skips compliance classification; the
standalone source refuses to start. Partial compliance-specific configuration
is still rejected by bootstrap validation.

The [denylist refresh](../../services/catalyst/explore/src/api/denylist.rs)
adds one merged-set database read per process per interval, with no upstream
request per referral. Propagation is eventual. Database failure can extend
staleness beyond the normal interval; retaining a last-known snapshot does not
provide immediate cross-process revocation or a fail-closed freshness policy.

The [native signed-action ordering](../../services/catalyst/social/src/communities/fed.rs)
keeps moderator-issued actions ordered by timestamp and signature hash. Only
ban/unban supersession excludes actions signed by their own target; promotions
and demotions retain ordering against self-joins and self-leaves. This describes
the native federation contract, not equivalence to the source client API.

## Reward issuance and reconciliation

[`Referral::reward` and `drain_pending_rewards`](../../services/catalyst/social/src/communities/referral.rs)
coordinate attempts with a per-invited-user PostgreSQL advisory guard. The
guard's connection closes on drop, including cancellation. The worker records
`reward_requested_at` before posting and records `reward_response` before
notification/image completion. The `reward_paid` receipt retains its tier,
provider response and confirmed notification completion, so an image retry
skips a notification already recorded as delivered. A claimed request lacking a
confirmed response is excluded from the automatic drain, including HTTP failures, malformed
responses, interrupted calls and interrupted response persistence. This can
hold a reward that the provider never issued; automatically clearing the claim
would risk issuing one that it already granted.

With social writes enabled, `serve` registers the fleet-leased
`social-referral-rewards` job. The drain checks the live restore-stamp hold
before selecting work and before each attempt. Each pass selects at most 64 due rows and the
next pass waits 60 seconds. Attempts defer eligibility by one minute; a later
ban also blocks retry. Missing campaign configuration fails before claiming,
so configuration can be repaired without issuing an empty campaign key.
Confirmed responses resume downstream work without another login or reward
POST. The pending tier and paid receipt clear only after notification and image
work succeeds; the claim and provider response remain as history.
Independent effects can survive a sibling failure, and external notification
publication may repeat after an ambiguous failure; this is not exactly-once
external delivery.

The [additive receipt migration](../../schemas/migrations/0024_social_referral_reward_receipt.sql)
imports a historical paid receipt into the claim/response fields and preserves
its notification marker. It quarantines preexisting pending tiers without a
paid receipt because their provider outcome is unknown.
Provider evidence must establish whether to retain the claim and supply a
confirmed response or authorize a new attempt. No automatic reconciliation
endpoint or provider idempotency guarantee is assumed.

## Schema activation and deployment gates

The [schema-v3 activation proposal](2026-11-04-social-schema-v3-activation.md)
and [release procedure](../../schemas/releases/v3/README.md) cover the compatible
binary, genuine version-2 origins and retained receipts/facts. The dormant definition
proposal remains Proposed too. Merging definitions or recovery code does not
apply a release or authorize deployment.

Acceptance and cutover require the following evidence:

- Check real proxy provisioning, overwritten headers and client-IP distribution;
  accepting the changed trust boundary is distinct from matching source headers.
- Exercise configured providers, queue policies and notification delivery,
  including repeated downstream publication and reconciliation of quarantined
  rewards. Local provider stubs do not establish these outcomes.
- Run PostgreSQL-backed upgrade and runtime regressions without skipped tests,
  covering both genuine version-2 origins, nonempty retained facts, immutable
  historical receipts, refused layouts and repeatable additive migration.
- Stop every writer, queue consumer and administrative mutation before release;
  establish backup restoration, apply the reviewed release, run the compatible
  migrator and inspect schema status and runtime before resuming traffic.
  An old version-2 binary refuses version 3; deleting or deduplicating retained
  facts to reinstall old constraints is not a rollback.

## Alternatives and tradeoffs

Retrying every pending reward could repeat a non-idempotent issuance; never retrying any confirmed-response work would
leave recoverable notification/image failures stranded. Per-request denylist
reads would have different latency and database cost, while local-only cache
invalidation would leave replicas stale indefinitely. These alternatives are
not the current implementation.

Revisit the proposal if deployment trust changes, measured refresh costs or
revocation requirements demand a different cache policy, or the reward provider
offers a durable idempotency/reconciliation contract. No acceptance or rollout
approval is recorded here.
