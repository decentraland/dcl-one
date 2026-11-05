# ADR: Bounded referral Slack effects

- Status: Proposed
- Date: 2026-11-04

## Context

Referral attribution posts Slack messages (suspicious timing, IP rejection and
the hundred-invite milestone). The pinned `social-service-ea` uses
`@slack/web-api`'s `WebClient` with its default retry policy, which can retry
for about thirty minutes and waits out any `Retry-After`. Those effects are
awaited inside referral HTTP requests and inside the login-event consumer,
whose SQS visibility timeout is 60 seconds.

## Decision

Each referral Slack effect keeps the source's payloads, endpoint, form encoding
and empty-token behaviour, but runs within one absolute ten-second budget
(`EFFECT_TIMEOUT` in `referral_slack.rs`). That budget covers the shared
provider permit, a rate-limit pause, at most three attempts and `Retry-After`
waits capped at two seconds. Redirects are refused and the token is redacted
from errors. A failed or timed-out effect keeps the committed referral state
and is logged at the boundary the source uses; it is not retried later.

## Consequences

- A Slack outage cannot hold one effect beyond ten seconds. This bounds the
  Slack contribution to request/consumer latency, not a complete handler or
  queue batch; other work can still exceed the queue's visibility window.
- A message the source would have delivered after a long outage or a long
  `Retry-After` is dropped here. Slack delivery is a notification, not durable
  state.
- The same whole-effect pattern bounds community AI compliance (60 seconds).

## Alternatives

- Keep the SDK's retry policy: blocks HTTP requests and the consumer for up to
  half an hour and lets messages be redelivered while still being handled.
- Detach the effect into a background task: loses the source's awaited
  ordering and error boundary without making delivery durable.
- Persist an outbox and retry it: a new table and worker for a notification
  that the source does not persist either.
