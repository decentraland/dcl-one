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
  redelivery inserts nothing, the owner of the auth chain the event carried
  (client: the chain proves that the wallet once delegated to a key, not
  that it sent this event, so a chain replayed in a forged event writes
  under its owner) or `traits.ethAddress` (web), the sub-type metadata the notifier
  published. A leased job purges rows older than
  `CLIENT_EVENTS_RETENTION_DAYS`. The badges and presence consumers read this
  table next; a trigger announces every stored row on the
  `social.client_events` channel, and wallet flags notified on the
  `credits.wallet_flags` channel are applied to the credits.
- The explorer's CP7 checkpoints are recorded in the explore onboarding state
  (`services/catalyst/explore/src/api/onboarding.rs`) by a local call, where
  the notifier posted them to the auth server; a refusal is a warning, as the
  notifier's fire-and-forget call was. A Segment event is accepted as a
  checkpoint only from `/forward` after its auth chain verified: the chain's
  owner is the checkpoint's user identifier, the OTP event's own
  email is dropped (the onboarding state resolves the wallet to the email
  the auth server bound to it, so a client-chosen address reaches no
  nudge, and keeps no row for a wallet without one, so throwaway wallets
  evict no user's row), and no metadata is kept.
  `/forward-onboarding-checkpoint`, whose body names any user, answers as the
  notifier did and records nothing. The onboarding state holds at most
  `MAX_CHECKPOINTS` rows (the oldest evicted first) and `MAX_METADATA_BYTES`
  of metadata per checkpoint.
- The parity gate is `services/catalyst/tests/notifier_parity.rs` over
  `services/catalyst/tests/fixtures/notifier_parity`, the notifier's own
  fixture bodies and unit-spec cases; `known_gaps.json` is its ratchet and
  starts empty.
- Accepted differences from the notifier, recorded as the gate's accepted
  entries (fixtures README, "Accepted entries"):
  - a client event's auth chain is verified, signatures and ephemeral expiry
    included, besides the owner check, and only the explorer's login chain
    (`[SIGNER, ECDSA_EPHEMERAL]`) is accepted; the notifier checked the
    chain's shape and owner only, so an expired chain, or a deployment chain
    a content server publishes, is 401 `Invalid AuthChain` here;
  - a client event without a string `messageId` or without a parsable
    `receivedAt` and `timestamp` is 400 `Invalid event`; the notifier published
    it with an undefined key or `NaN` timestamps;
  - a web identify without `traits` is 200 `validationPassed: 0` without a
    row, its unit spec's intent; the notifier's source threw, a 500;
  - a web identify without `context.page` is 400 `Invalid web event`; the
    notifier's source threw, a 500;
  - a web identify whose wallet is missing or not an address stores no row;
    the explorer's `sentAt` is not stored; the two delay metrics are
    histograms.
  - the OTP checkpoint is recorded by the chain owner, without metadata, and
    only once the chain verified (401 `Invalid AuthChain` otherwise);
    `/forward-onboarding-checkpoint` records nothing; the notifier recorded
    both by the identifier the body carried, before any check.
- The notifier's `POST /forward-blockchain` (a tip reported by its sender) is
  not this router's: the shop serves it under the same prefix
  (`services/catalyst/shop/src/market/tips.rs`), the claim checked against
  the node's own MANA transfer index instead of an RPC read.
- The multi-accounting detector (`event-parser.ts:110-179`), its anonymous-id
  denylist (`client-denylister.ts`) and its `/admin/anonids` routes are not
  part of the node: their lists and limits are operator policy over
  unsigned client input (`context.ip`, the anonymous id). The node keeps
  two neutral mechanisms and the flag's effect. A trigger announces every
  stored `social.client_events` row on the `social.client_events` channel
  (the identity columns, never the metadata). The leased
  `credits-wallet-flags` listener (`shop/src/credits/wallet_flags.rs`,
  where `OPS_WRITES_ENABLED` is on) applies `{action: "flag", addresses,
  source, ip}` (the credits server's `/flag` body) or `{action: "unflag",
  address}` notified on `credits.wallet_flags` to `credits.wallet_flags`
  (`DATABASE.md`, "Credits wallet flags workload") through the admin store,
  audited under `pg:credits.wallet_flags`. A flag blocks the credits claim
  only (the spend and the reservation read the operator's block alone); the
  operator's `GET`, `POST` and `DELETE /credits/admin/flags` behind
  `CATALYRST_CREDITS_ADMIN_TOKEN` list, raise and release one by hand, and
  the unblock (`user.unblock`) deletes it too. A notification reaches the
  listeners connected when it commits and nobody else: Postgres grants no
  privilege on `NOTIFY` or `LISTEN`, so every role that can connect to the
  database can raise or release a flag on `credits.wallet_flags` and
  receives the identity columns announced on `social.client_events`; both
  channels trust the database's roles, the node's own and the flagging
  service's, and a flag raised while no listener is connected is not
  applied. A queue table under `GRANT INSERT` to one role, or a `SECURITY
  DEFINER` writer, would name the flag's writer and keep the flags raised
  while no listener is up: the next step if the database gains other roles.
  No event is refused for its anonymous id, and `/forward-web` answers
  `validationPassed: 1` for every stored identify.

## Consequences

- One sink: a consumer of client events reads `social.client_events` by
  direct SQL inside the binary, with no topic, queue or second service, and
  records that its cursor stays above the purge horizon.
- The signing key is a secret of the deployment (`Config::secret_slots`); a
  node without it is not a Segment destination and says so with a 503.
- A flagged wallet is refused the credits claim by the backend, not by the
  UI, and keeps the credits it holds; the operator turns flagging on per
  deployment and releases a wallet with an unblock.
- Next work: the badges and presence consumers; the retirement of the
  `telemetry` schema with its release script.
