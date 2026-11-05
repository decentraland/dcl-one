# ADR: Native social events

- Status: Accepted
- Date: 2026-11-04
- Status changed: 2026-11-04
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

Login recording performs no reward-provider request. The existing bounded,
fleet-leased reward worker consumes committed intent. It persists a claim before
the non-idempotent provider POST and a confirmed response before local completion.
The reward image, notification/email outbox and clearing of pending state commit
atomically. An uncertain provider outcome remains quarantined; SQL retries do
not authorize another provider POST. Optional Slack alerts remain bounded,
best-effort post-commit effects.

## Consequences

The runtime needs no SQS queue, SNS topic or AWS credentials for Social events.
Existing HTTP/RPC contracts remain; the owned client uses an additive login
endpoint. An unchanged third-party client that does not report native logins
needs its own producer integration before referral-login coverage can be claimed.
Existing NATS subjects and delivery guarantees are unchanged.

Compatibility evidence tests external responses and native effects. Frozen
upstream captures remain source evidence, including captures of internals, but
AWS request batches and queue acknowledgements are not acceptance criteria for
this implementation. Provider availability, deployment configuration and live
client evidence remain separate from local regression results.

Revisit this decision if an independently deployed consumer needs a durable
inter-process event contract. Such a consumer must justify its channel and
recovery semantics rather than inheriting an AWS topology by default.
