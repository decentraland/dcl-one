# Segment webhooks replace the telemetry sink

- Status: Accepted
- Date: 2026-11-07

The node stops being a telemetry sink. The sites and the Explorer post their
analytics to Segment with a public write key, as the old websites and the old
Explorer did, and the node serves the events-notifier's Segment destination
webhooks instead, pinned at `decentraland/events-notifier@1550a6e8`. What
Segment forwards lands in one table of the node's own database, where the
consumers the notifier fed over SNS read it next.

## Context

- `services/catalyst/src/ops/telemetry` ingested the sites' tracking client and
  the Explorer's Sentry-shaped reports into the `telemetry` schema
  (`schemas/60_ops.sql`). Nothing read it: no badge, presence or moderation
  consumer, no dashboard.
- Decentraland's analytics pipeline is Segment. The old websites and the old
  Explorer post to it with a public write key; the events-notifier receives
  Segment's webhook deliveries, verifies them with a shared signing key, and
  publishes the parsed events to SNS for the badges, notifications and
  multi-accounting consumers (`events-notifier/src/controllers/handlers/forward-explorer-events.ts`,
  `forward-web-events.ts`, `forward-onboarding-checkpoint.ts`,
  `src/logic/event-parser.ts`).
- The node has no SNS and no separate consumers: every backend area runs in
  one `catalyst` binary over one database (`AGENTS.md`, "Scope").

## Decision

- The node is not a Sentry or Segment sink. The `telemetry` schema is retired;
  the removal of the ingest, the sites' tracking client and the schema lands
  separately with its release script.
- The sites and the Explorer post to Segment with a public write key. The node
  keeps no analytics write path of its own.
- The notifier's webhooks are served under `/events-notifier`
  (`POST /forward`, `POST /forward-web`, `POST /forward-onboarding-checkpoint`)
  by `services/catalyst/social/src/segment_webhooks`, nested by `serve` behind
  the social writes guard (`services/catalyst/social/src/writes.rs`) and routed
  by the gateway as an absorbed service (`infra/gateway/routes.nix`). The
  contract is the notifier's: `x-signature` is the hex HMAC-SHA1 of the body
  under `SEGMENT_SIGNING_KEY` (`services/catalyst/shared/src/segment_signature.rs`),
  a refusal answers `{ok: false, error}` with the notifier's statuses and
  messages, and an accepted delivery answers `{ok: true}` (`validationPassed`
  on the web route). An unset key answers 503; the writes switch answers 503
  with `Retry-After`.
- A forwarded event is one row of `social.client_events`
  (`schemas/40_social.sql`, `DATABASE.md`, "Segment webhook events workload")
  instead of an SNS message: append-only, keyed by Segment's `messageId` so a
  redelivery inserts nothing, the owner address proved by the auth chain
  (client) or `traits.ethAddress` (web), the sub-type metadata the notifier
  published. A leased job purges rows older than
  `CLIENT_EVENTS_RETENTION_DAYS`. The badges, presence and multi-accounting
  consumers read this table next.
- A forwarded onboarding checkpoint is recorded in the explore onboarding
  state (`services/catalyst/explore/src/api/onboarding.rs`) by a local call,
  where the notifier posted it to the auth server; a refusal is a warning, as
  the notifier's fire-and-forget call was.
- The parity gate is `services/catalyst/tests/notifier_parity.rs` over
  `services/catalyst/tests/fixtures/notifier_parity`, the notifier's own
  fixture bodies and unit-spec cases; `known_gaps.json` is its ratchet and
  starts empty.
- Accepted differences from the notifier, recorded as the gate's accepted
  entries (fixtures README, "Accepted entries"):
  - a client event's auth chain is verified, signatures and ephemeral expiry
    included, besides the owner check; the notifier checked the chain's shape
    and owner only, so an expired chain is 401 `Invalid AuthChain` here;
  - a client event without a string `messageId` or without a parsable
    `receivedAt` and `timestamp` is 400 `Invalid event`; the notifier published
    it with an undefined key or `NaN` timestamps;
  - a web identify without `traits` is 200 `validationPassed: 0` without a
    row, its unit spec's intent; the notifier's source threw, a 500;
  - a web identify without `context.page` is 400 `Invalid web event`; the
    notifier's source threw, a 500;
  - a web identify whose wallet is missing or not an address stores no row;
    the explorer's `sentAt` is not stored; the IP and anonymous-id
    multi-accounting clusters and the credits-server flags are not kept (the
    denylist refusal is); the two delay metrics are histograms.

## Consequences

- One sink: a consumer of client events reads `social.client_events` by
  direct SQL inside the binary, with no topic, queue or second service, and
  records that its cursor stays above the purge horizon.
- The signing key is a secret of the deployment (`Config::secret_slots`); a
  node without it is not a Segment destination and says so with a 503.
- `SEGMENT_WALLET_ALLOWLIST` has no consumer and no setting; it returns with
  the multi-accounting detector.
- Next work: `POST /forward-blockchain` (tips received) in the same router;
  the multi-accounting detector over the stored rows; the badges and presence
  consumers; the retirement of the `telemetry` schema with its release script.
