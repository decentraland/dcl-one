# ADR: Opt-in AI conversation sharing

- Status: Proposed
- Date: 2026-11-12

The Electron Creator Hub's AI assistant drives the creator's own agent CLI
(Claude, Codex, Cursor, Gemini) to build scenes. Creators who opt in share
each turn: what they asked, what the agent answered, the tools it used, the
agent and model, and the outcome. The node stores those turns, keyed by the
creator's wallet, serves them back to that wallet, erases them on request or
after a retention period, and lets allowlisted operators read them, with
every read audited, to improve the assistant's prompt, tools and skills.

This is a named exception to
[Segment webhooks replace the telemetry sink](2026-11-07-segment-webhooks-replace-the-telemetry-sink.md),
which says the node keeps no analytics write path of its own. Its purpose is
analytics: improving a product. The exception is bounded to one route
family, one schema and one data class: text a creator chose to share, under
a consent the node holds and enforces.

## Context

- A turn's text already reaches the creator's chosen agent provider, which
  runs the model. None of it reaches Decentraland: the `AI Turn Completed`
  Segment event carries the provider, model, project id, counts and timings,
  no text.
- A shared turn is up to 128 KB of text a creator wrote or caused, about
  their own scene, under a consent they can withdraw. Segment's documented
  per-call limit is 32 KB (external), and the sites post to it with a public
  write key, so it has no way to refuse an event from a wallet that has not
  consented. The node verifies the signer and holds the consent, so it can.
- `camera_reel` is the nearest precedent: a wallet's own records, signed
  writes, owner-only delete, behind `CREATE_WRITES_ENABLED`.
- ADR-44 signed fetch covers `method:path:timestamp:metadata`, has no replay
  cache, and accepts a request for ±5 minutes
  (`services/catalyst/shared/src/signed_fetch.rs`). Explore's
  `record_signed_request` (`services/catalyst/explore/src/events/writes.rs`)
  checks a signed `body_sha256` and refuses a replayed signed `nonce`, which
  it makes optional; its log, `explore.signed_actions`, is never purged.
- The writes gate (`services/catalyst/shared/src/writes.rs`) keeps a write
  open while writes are off by path alone: every mutating method on a listed
  path passes.

## Decision

### Where it lives

- A module `ai_sharing` of the `builder` crate
  (`services/catalyst/builder/src/ai_sharing/`), with its own schema
  `ai_sharing` in a new file `schemas/51_ai_sharing.sql`. No second consumer
  or build requirement calls for a crate. The schema stays separate from
  `builder`'s so its retention, erasure and readers are visible on their own,
  as `chat` is beside `social`. The object pass creates the schema and its
  tables on an existing database; no migration step is needed.
- `serve` nests its router under `/ai-sharing`. The gateway reaches it
  through `location /`; no route entry is needed. Handlers verify the
  signature against the original URI (`SignedPath::from_uris`); the gate sees
  the path with `/ai-sharing` stripped.

### Authentication and replay

- Every route takes an ADR-44 signed request:
  `shared::signed_fetch::require_signer_at` with `PayloadPolicy::V6Only`.
  Creator Hub signs with `decentraland-crypto-fetch` 3.x, which lowercases
  the method and path and binds the metadata bytes verbatim.
- Every write carries signed metadata `{"nonce": <canonical UUID>}`, plus
  `"body_sha256": <hex SHA-256 of the exact body bytes>` when it has a body.
  A missing or mismatched hash is 400, a missing or non-canonical nonce is
  400, and a nonce this wallet already used is 409 with nothing applied.
  Unlike explore's check, the nonce is required. Nonces are recorded in
  `ai_sharing.signed_requests` (primary key `(wallet, nonce)`, with
  `received_at`) in the write's own transaction, and purged after fifteen
  minutes: a request signed five minutes ahead stays valid for ten minutes
  after it arrives. The check moves to `shared` when a second module needs it.

### Contract

Paths are under `/ai-sharing`. Errors answer `{message}`, as `camera_reel`
does.

| Route | Answers |
| --- | --- |
| `GET /consent` | `{granted, version, decidedAt}` of the wallet's latest decision; `{granted: false, version: null, decidedAt: null}` when it has none. |
| `PUT /consent` `{granted, version, decidedAt}` | The wallet's state after the write. |
| `POST /turns` `{records}` | `200 {accepted, rejected: [{recordId, reason}]}`. Each record is validated on its own; one that fails is listed in `rejected` and not stored. 403 `{message, currentVersion}` when the wallet's latest decision is not a grant of the current consent version; nothing is stored. |
| `GET /turns?limit&cursor` | `{records, next}`: the wallet's own records, newest first; `limit` 1–20 (default 10); `next` is an opaque cursor or `null`. |
| `POST /erase` `{decidedAt, recordId?}` | `{deleted}`. With `recordId`, deletes that record of the wallet. Without it, deletes every record of the wallet and appends a withdrawal dated `decidedAt`. |

Statuses besides 200: 400 for a malformed body, metadata or signature
header; 401 for a signature that does not verify; 409 as described under
"Consent" and for a reused nonce; 413 for a body over 1 MiB (the router's
`DefaultBodyLimit` and the gateway's `client_max_body_size`); 429 with
`Retry-After` for a quota; 503 with `Retry-After: 3600` while writes are off,
and 503 when the EIP-1654 check cannot reach its RPC.

`POST /turns` takes at most 20 records. The byte cap decides: Creator Hub
packs records up to 900,000 bytes per request.

### Consent

- `AI_SHARING_CONSENT_VERSION` in the node's config is the current consent
  version. Creator Hub ships the same number with the consent text it shows.
- `decidedAt` is the creator's clock when they decided. It is stored as sent
  and used for one comparison only, below.
- A wallet's decisions are ordered by the node's receive time; the latest
  received is the wallet's state.
- A withdrawal (`granted: false`, or `POST /erase` without `recordId`)
  always applies.
- A grant is 409 `{message, currentVersion}` when its `version` is not the
  current one, or when its `decidedAt` is earlier than the `decidedAt` of a
  withdrawal already stored for the wallet. The second case is an offline
  machine delivering a grant made before the creator withdrew on another
  one. Both dates come from creators' own clocks.
- A decision equal to the latest stored one writes nothing.
- Turns accepted under an older consent version stay until retention or
  erasure; a version change stops new ones only.

### The record

Schema version 1. A field marked optional may be absent or `null`.

- `schemaVersion` (1), `recordId` (UUID), `consentVersion`,
  `address` (lowercase wallet), `conversationId`, `turnId`, `turnIndex`,
  `projectId` (optional; a random UUID, never a path or scene name).
- `startedAt` (ISO 8601), `durationMs`, `app: {version, os}`,
  `agent: {provider, requestedModel, actualModel?, cliVersion?}`,
  `systemPromptHash` (hex SHA-256 of Creator Hub's system prompt),
  `usage?: {inputTokens, outputTokens, costUsd?}`.
- `prompt` and `response`, and `tools: [{name, detail}]` (at most 200), each
  text as `{text, originalLength, truncated, redactions}`, where
  `redactions` maps a kind (`api_key`, `seed_phrase`, `email`, …) to how
  many matches the client removed.
- `context: {selectedEntityCount, mentionedAssetCount, attachments: [{kind,
  ext}]}`: counts and file extensions, never content or paths.
- `outcome` (`ok`, `error`, `cancelled`), `errorKind` (`start_failed`,
  `sign_in`, `no_reply`, `exit_code`, `signal`, or `null`; never error
  text), `mutations`.

Limits count Unicode code points, a truncation marker included: `prompt.text`
16,000, `response.text` 32,000, a tool `name` 120, a tool `detail.text` 500.
A record is at most 128,000 bytes of JSON as received.

A record is rejected with a fixed `reason` code, never a parser message
(which can quote the value): `malformed`, `missing_field`, `too_long`,
`too_large`, `address_mismatch`, `consent_version`, `secret_detected`. The
node checks shape and size, and refuses (`secret_detected`) any record
whose text holds one of the two highest-risk secrets: a 64-hex-digit
private key, with or without `0x` (wallets export it without), or a run of
twelve or more BIP-39 English words, however separated (spaces, commas,
quotes and brackets of a JSON array, list numbering). Every other secret and
personal detail is removed by the client alone, before upload; a hex SHA-256
has a private key's shape and is refused too.

### Writes and erasure

- `POST /turns`, `PUT /consent` and `POST /erase` take a per-wallet
  transaction-scoped advisory lock and do their consent read, quota checks
  and writes in that one transaction. A withdrawal or erasure that commits is
  never followed by a turn accepted under the consent it ended.
- Turns are immutable and keyed by `(wallet, record_id)`. A record id this
  wallet already stored is counted as accepted, changes nothing and costs no
  quota, so a retried upload is harmless. One wallet cannot touch another's
  rows.
- `CREATE_WRITES_ENABLED` gates the module through its own `writes` gate
  (`blobs: false`). Because the gate opens by path, erasure and consent have
  paths of their own: `/erase` and `/consent` are on the open-while-disabled
  list, `/turns` is not. While writes are off, `PUT /consent` applies a
  withdrawal and answers a grant with 503.

### Quotas

Counted in SQL inside the write transaction, so they hold across replicas
and restarts:

- per wallet: 200 accepted turns in any 24 hours and 3,000 stored;
- per client address, read only from a trusted proxy as the referral quota
  reads it (`HTTP_TRUSTED_PROXY_CIDRS`): 1,000 accepted turns a day;
- in total: `AI_SHARING_DAILY_TURNS` a UTC day (default 20,000), counted in
  a single counter row updated with
  `SET n = n + $k WHERE day = $today AND n + $k <= $cap`, since per-wallet
  locks do not serialize a cross-wallet count.

The gateway also rate-limits `/ai-sharing` writes per client address, with a
`limit_req` zone like its others. A request that would pass a quota is 429
with `Retry-After` and stores nothing; Creator Hub keeps the records queued
and retries.

The attack these bound: fresh wallets cost nothing, so one actor can fill
the global day and refuse every creator's turns until it rolls over, or
store junk. The per-address limit and the gateway zone bound one source; the
global cap bounds storage. Together with the 128,000-byte record cap, the
caps hold storage to at most 2.56 GB a day and 461 GB at the 180-day
retention.

### Retention and backups

- A leased periodic job, `ai-sharing-purge` (`jobs.spawn_periodic_leased`,
  as `client-events-purge`), deletes turns older than
  `AI_SHARING_RETENTION_DAYS` (default 180) in batches through a
  `(received_at, wallet, record_id)` index, and nonces older than fifteen
  minutes.
- Consent history is the record of what a wallet agreed to and when it
  stopped. It is kept while the wallet's latest decision is a grant or it has
  stored turns; once neither holds, the purge deletes it
  `AI_SHARING_RETENTION_DAYS` after the latest decision.
- Backups that hold `ai_sharing` expire within `AI_SHARING_BACKUP_DAYS`,
  which the deployment sets and the privacy policy states. A restored
  database keeps the module's writes off until the deployment has re-applied
  the erasures and withdrawals received after the backup was taken, from a
  log the deployment keeps outside the database (wallet, time, `recordId`,
  no content). Writes for this module stay off until that log exists.

### Who reads it

- The wallet itself, through `GET /turns` and `GET /consent`.
- Operators, through `GET /admin/api/ai-sharing/turns` (filters: time range,
  provider, outcome; paginated) behind `AdminSession` and a second allowlist,
  `AI_SHARING_READER_ADDRESSES`. Today `ADMIN_ADDRESSES` is the console's only
  allowlist; this is the first per-area one. Operators see a keyed hash of
  the wallet (`AI_SHARING_OPERATOR_KEY`), never the address. Filtering by a
  wallet takes the wallet and a stated reason (an erasure check, a creator's
  own request), and the reason is audited.
- Every operator request writes an `ops.admin_audit` row with domain
  `create` and action `ai_sharing.read`: the filters, the reason when there is
  one, and the returned `recordId`s, never content. Rows follow the audit
  table's own retention.
- Analysis outputs quote no record text. Building an evaluation set from
  records, and any export or copy outside the node, is a new decision.
- Database superusers and backups can read the schema; the console is the
  only application path to it.
- No database role is added.

### Logs and metrics

Handlers log ids, counts, sizes and reason codes, never a record's fields, a
request body or a parser message. Metrics count accepted and rejected
records by reason, quota refusals by quota, and bytes stored.

### Workload

To be recorded in `DATABASE.md` before the tables are final. The planning
figures are assumptions, to be replaced with the `AI Turn Completed` counts
once Creator Hub ships:

- writers: 500 opted-in creators, 20 turns each a day, 8 KB typical record:
  10,000 rows and 80 MB a day; 1.8 M rows and 14 GB at 180 days;
- readers: `GET /turns` by its own wallet (rare), the operator read (a few
  analysts, paged), the purge (hourly);
- invariants: the ones under "Consent" and "Writes and erasure".

## Consequences

- The node gains one write path for analytics-purpose data, bounded as
  above. The telemetry ADR gains a line pointing here when this one is
  accepted.
- A creator can stop sharing in Creator Hub, read what they shared with
  `GET /turns`, erase one turn or all of it with `POST /erase`, and withdraw
  and erase while writes are off.
- The consent version is a contract between Creator Hub and the node. A new
  consent text bumps it in both; grants of the old version stop admitting
  turns, and a Creator Hub too old to show the new text stops offering
  sharing.
- Launch needs, outside this repo: the privacy policy and consent wording
  approved (lawful basis: consent; withdrawal as easy as granting;
  `AI_SHARING_BACKUP_DAYS` stated), the retention period confirmed, the
  deployment's erasure log, Creator Hub's view of what a creator shared
  (`GET /turns`), its `AI_SHARING_API_URL` set, and its feature flag turned
  on.
- Next work: the module, `schemas/51_ai_sharing.sql`, the `DATABASE.md`
  workload section, tests on the scratch database (the lock, quotas,
  replay, cross-wallet ids, consent ordering, erasure while writes are off,
  the secret backstop), `config/src/create.rs`, `config/src/posture.rs`,
  `src/serve.rs`, `src/ops/admin/create.rs`, the gateway zone, and
  `.github/CODEOWNERS` regenerated with `tooling/owners.py` if
  `ownership.json` changes.

## Alternatives

- **Segment.** Rejected: a 32 KB per-call limit (external) below a record's
  size, and no way to refuse an event from a wallet that has not consented.
- **A crate of its own.** Rejected: nothing but this module would depend on
  it, and the layout rule asks for an independent consumer or a build
  requirement.
- **An analysis database role.** Rejected: every role that can connect may
  `LISTEN` on `social.client_events` and `NOTIFY` on `credits.wallet_flags`
  (`DATABASE.md`), so a reader role would also receive wallets and IP
  addresses and could flag wallets.
- **`DELETE /turns` for erasure.** Rejected: the gate opens by path, so
  keeping erasure open while writes are off would open uploads too.
- **Ordering consent by `decidedAt` alone.** Rejected: it compares clocks
  of different machines with the node's and can drop a creator's latest
  click.
- **An in-process `comms::limits::RateLimiter`.** Rejected for the quotas: it
  is per replica, resets on restart, and clears every bucket at 10,000 keys,
  which fresh wallets reach for free.
- **Unsigned uploads with a client id.** Rejected: consent and erasure need
  an owner the node can verify.
- **A model-based personal-data filter on the node.** Not decided here. The
  node refuses the two highest-risk secrets; the client removes keys, seed
  phrases, emails, phone numbers, IP addresses and home-folder paths. A
  model-based pass is a separate decision, taken if a sample of stored
  records shows the client rules miss things.
