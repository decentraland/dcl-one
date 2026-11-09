# ADR: Native social events

- Status: Accepted
- Date: 2026-11-04
- Status changed: 2026-11-04
- Decided by: user, through the explicit instruction to remove SQS/SNS and retain NATS.
- Supersedes: [Bounded referral Slack effects](2026-11-04-referral-slack-effect-budget.md), for native login delivery ordering; its provider time and retry bounds remain.
- Decision: Replace internal SQS/SNS transport with native monorepo integration; retain NATS and preserve external client contracts.

## Context

Compatibility with `decentraland/social-service-ea` at
`b6b2cefefdf727b2ed47ddb887e77d226a41eea6` concerns client-visible HTTP/RPC,
authorization, notification content and referral behavior. AWS queue semantics,
topic envelopes, SDK retries and transport-specific failure ordering are internal
implementation details. Reproducing them would retain infrastructure that the
unified application does not need.

Social, comms, worlds and discovery run together in `catalyst serve`. Native
ranking producers already write local facts, and social owns the notification
inbox and email outbox. Pulse and serving replicas retain their existing NATS
channels. Core NATS is transient and does not replace durable referral state.

## Decision

Remove the Social SQS consumer, SNS publishers, AWS SDK dependencies and their
runtime configuration. In-process effects use domain calls and the existing SQL
pool. Notifications are persisted locally with their email delivery records;
community and friendship audiences retain their existing subscription rules.
Ranking continues through native photo finalization, stream completion and
ended-event jobs. No replacement generic event bus or separate service is added.

A login is identity completion in the owned Explorer, including restored and
imported identities. Social RPC is lazy and therefore cannot be the only login
producer; reconnects and room admissions are not treated as new logins.
The client submits `POST /social-api/v1/client-login` (or the native
`/v1/client-login` mount) with ADR-44 V6 signed metadata containing `loginId`
(UUID) and `occurredAt` (epoch milliseconds). The verified signer identifies
the wallet. A successful local commit returns 204; no request body chooses
another wallet. Retried submissions keep the same event identity. The
server persists its receipt and local referral effects before acknowledging it.
UTC processing dates and the existing threshold-before-append transition are
preserved; a duplicate request cannot supply an extra qualifying transition.

The client normally saves each event before sending it. After three storage
failures it exposes a memory-only mode, keeps unsaved changes and retries
storage recovery every minute. That mode preserves live-session delivery but
cannot retain events across process/page shutdown. Corrupt stores are
quarantined, and valid records survive malformed neighbors. Missing endpoints
(404/405) retain observations with a 150–300 second retry delay, bounded by the
same 24-hour event lifetime, so client-first deployments do not discard logins.

Login recording performs no reward-provider request. The existing bounded,
fleet-leased reward worker consumes committed intent. It persists a claim before
the non-idempotent provider POST and a confirmed response before local completion.
The reward image, notification/email outbox and clearing of pending state commit
atomically. An uncertain provider outcome remains quarantined; SQL retries do
not authorize another provider POST. Optional Slack alerts remain bounded,
best-effort post-commit effects. Attribution timing/IP alerts remain awaited;
only the native login milestone is detached. Background admission allows at most eight
concurrent milestone tasks per adapter before spawning; saturation drops this optional alert without
undoing a committed grant. Each admitted effect retains the ten-second total
budget, three-attempt limit and two-second Retry-After cap. This supersedes
the earlier proposal's rejection of detached milestone delivery; it does not
claim durable Slack delivery or preserve the old consumer's awaited ordering.

## Consequences

The runtime needs no SQS queue, SNS topic or AWS credentials for Social events.
Existing HTTP/RPC contracts remain; the owned client uses an additive login
endpoint. An unchanged third-party client that does not report native logins
needs its own producer integration before referral-login coverage can be claimed.
Existing NATS subjects and delivery guarantees are unchanged.

Run `catalyst schema migrate` before rolling out the new server; bootstrap
requires the login receipt relation. Remove `AWS_SQS_ENDPOINT`,
`AWS_SQS_QUEUE_URL`, `AWS_SNS_ENDPOINT` and `AWS_SNS_ARN` from Social deployment
environments and secret/config templates; the runtime no longer reads them.
This deployment supports the owned
Explorer producer. It is not a drop-in referral replacement for unchanged
third-party clients: their referral-progress operations remain a
known gap until those clients submit native signed login observations. Do not
expand the rollout to those clients on the strength of HTTP route compatibility.

There is no legacy SQS importer. Queued messages lack the user signature
required by the native endpoint; undrained observations are lost to referral
progress at cutover. An archive alone does not make them replayable.

The monorepo has no SNS subscription inventory. Native inbox/email and direct
comms effects replace the owned paths; delivery to existing external push or
other topic subscribers is not implemented or verified. Before replacing an
existing topic-backed deployment, its operator must identify those consumers
and integrate any required external effect. This does not require restoring
AWS transport in the native runtime.

Compatibility evidence tests external responses and native effects. Frozen
upstream captures remain source evidence, including captures of internals, but
AWS request batches and queue acknowledgements are not acceptance criteria for
this implementation. Provider availability, deployment configuration and live
client evidence remain separate from local regression results.

Revisit this decision if an independently deployed consumer needs a durable
inter-process event contract. Such a consumer must justify its channel and
recovery semantics rather than inheriting an AWS topology by default.
