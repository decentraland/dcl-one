# ADR: Opt-in AI conversation sharing and analysis

- Status: Proposed
- Date: 2026-11-12

The Electron Creator Hub's AI assistant drives the creator's own agent CLI
(Claude, Codex, Cursor, Gemini) to build scenes. Creators who opt in share
each turn: what they asked, what the agent answered, the tools it used, the
agent and model, and the outcome. The node stores those turns, keyed by the
creator's wallet, serves them back to that wallet, and erases them on request
or after a retention period.

The point of collecting them is to make the assistant better for creators.
Every day the node summarises each finished conversation with a language
model (what the creator was trying to do, whether they got there, what went
wrong and what went right), then writes a daily report: counts sliced by
Creator Hub release, agent and model, and findings with proposed changes to
the system prompt, skills, tools and docs, each backed by conversations from
several creators. People on the team review the findings and decide what to
change; the node changes nothing itself.

This is a named exception to
[Segment webhooks replace the telemetry sink](2026-11-07-segment-webhooks-replace-the-telemetry-sink.md),
which says the node keeps no analytics write path of its own. Its purpose is
analytics: improving a product. The exception is bounded to one route
family, one schema, one data class (text a creator chose to share, under a
consent the node holds and enforces) and one outbound processor (the model
provider that writes the summaries).

## Context

- A turn's text already reaches the creator's chosen agent provider, which
  runs the model. None of it reaches Decentraland: the `AI Turn Completed`
  Segment event carries the provider, model, project id, counts and timings,
  no text.
- A shared turn is up to 128 KB of text a creator wrote or caused, about
  their own scene, under a consent they can withdraw. Segment documents a
  32 KB limit per call, and the sites post to it with a public write key, so
  it has no way to refuse an event from a wallet that has not consented. The
  node verifies the signer and holds the consent, so it can.
- The team's question is not "what is in row N" but "what are creators
  trying to build, where does the assistant fail them, and did last week's
  change help". Reading turns one by one does not scale and puts more text
  in front of more people; counting columns in SQL cannot tell a
  misunderstood request from a missing skill. A model reading whole
  conversations can, and SQL then counts what the model labelled.
- `camera_reel` is the nearest precedent for storage: a wallet's own
  records, signed writes, owner-only delete, behind `CREATE_WRITES_ENABLED`.
- Community compliance (`services/catalyst/social/src/communities/compliance.rs`)
  is the precedent for calling a model provider from the node: settings with
  a redacted `Debug`, a whole-call time budget (`EFFECT_TIMEOUT`), bounded
  retries that honour `Retry-After`, and structured JSON output checked
  against a schema. Its key is listed in `THIRD_PARTY_SECRETS` with its base
  URL as an `upstream` (`services/catalyst/config/src/posture.rs`), so a
  loopback or mirror posture refuses a remote provider.
- ADR-44 signed fetch covers `method:path:timestamp:metadata`, has no replay
  cache, and accepts a request for ±5 minutes
  (`services/catalyst/shared/src/signed_fetch.rs`). Explore's
  `record_signed_request` (`services/catalyst/explore/src/events/writes.rs`)
  checks a signed `body_sha256` and refuses a replayed signed `nonce`, which
  it makes optional; its log, `explore.signed_actions`, is never purged.
- The writes gate (`services/catalyst/shared/src/writes.rs`) keeps a write
  open while writes are off by path alone: every mutating method on a listed
  path passes. `CREATE_WRITES_ENABLED` is one switch for the whole creator
  plane (`services/catalyst/builder/src/writes.rs`), and the restore stamps
  behind it (`shared/src/writes/restore.rs`) prove that backups and restore
  drills are fresh, not that a restore has happened.

## Decision

### Where it lives

- A module `ai_sharing` of the `builder` crate
  (`services/catalyst/builder/src/ai_sharing/`), with its own schema
  `ai_sharing` in a new file `schemas/51_ai_sharing.sql`. No second consumer
  or build requirement calls for a crate. The schema stays separate from
  `builder`'s so its retention, erasure and readers are visible on their own,
  as `chat` is beside `social`. The object pass creates the schema and its
  tables on an existing database; no migration step is needed.
- The analysis lives in the same module (`ai_sharing/analysis/`). Its two
  prompts and their output schemas are files beside the code (`digest.md`,
  `report.md`), so a prompt change is reviewed like any other change.
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
| `GET /turns?limit&cursor` | `{records, next}`: the wallet's own records, newest first, each with its `rating` (`up`, `down` or `null`); `limit` 1–20 (default 10); `next` is an opaque cursor or `null`. |
| `GET /digests?limit&cursor` | `{digests, next}`: the summaries the node made of the wallet's own conversations, newest first, paged as `GET /turns`. |
| `PUT /ratings` `{recordId, rating}` | `{recordId, rating}`. `rating` is `up`, `down`, or `null` to clear it. 404 when the wallet has no such record; 403 as for `POST /turns`. |
| `POST /erase` `{decidedAt, recordId?}` | `{deleted}`. With `recordId`, deletes that record of the wallet and everything derived from it. Without it, deletes every record of the wallet and everything derived from them, and appends a withdrawal dated `decidedAt`. |

Statuses besides 200: 400 for a malformed body, metadata or signature
header; 401 for a signature that does not verify; 409 as described under
"Consent" and for a reused nonce; 413 for a body over 1 MiB (the router's
`DefaultBodyLimit` and the gateway's `client_max_body_size`); 429 with
`Retry-After` for a quota; 503 with `Retry-After: 3600` while writes are off,
and 503 when the EIP-1654 check cannot reach its RPC.

`POST /turns` takes at most 20 records. The byte cap decides: Creator Hub
packs records up to 900,000 bytes per request.

A rating is the one signal of success the creator gives directly; the rest
are inferred. It carries no text, so it adds no new kind of personal data.
Ratings live in `ai_sharing.ratings` keyed by `(wallet, record_id)`, deleted
with their record.

### Consent

- `AI_SHARING_CONSENT_VERSION` in the node's config is the current consent
  version. Creator Hub ships the same number with the consent text it shows.
- Version 1 is the first text Creator Hub ships with the feature turned on.
  It says, in plain words, that shared conversations are read by an AI model
  from a provider working for Decentraland, which keeps no copy and does not
  train on them; that the summaries it writes are kept and deleted with the
  conversations; and that patterns found across many creators' conversations
  may be kept by the team to improve the assistant. The privacy policy names
  the provider, so changing provider does not change the consent text.
  Creator Hub's current pre-release text does not yet say this; it is
  updated before the flag is turned on, so no turn is ever stored under a
  text without it.
- `AI_SHARING_ANALYSIS_MIN_CONSENT_VERSION` (default 1) is the oldest
  consent version whose turns may be analysed. A later text that narrows
  what is done with the turns raises it.
- `decidedAt` is the creator's clock when they decided. It is stored as sent
  and used for one comparison only, below.
- A wallet's decisions are ordered by the node's receive time; the latest
  received is the wallet's state.
- A withdrawal (`granted: false`, or `POST /erase` without `recordId`)
  always applies. It stops new turns and stops analysis of the wallet: in
  its own transaction it deletes the wallet's digests and everything
  derived from them (see "Writes and erasure"). The turns themselves stay
  until retention, or until the creator erases them, as the consent text
  says.
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

- `POST /turns`, `PUT /ratings`, `PUT /consent` and `POST /erase` take a
  per-wallet transaction-scoped advisory lock and do their consent read,
  quota checks and writes in that one transaction. A withdrawal or erasure
  that commits is never followed by a turn accepted under the consent it
  ended.
- Turns are immutable and keyed by `(wallet, record_id)`. A record id this
  wallet already stored is counted as accepted, changes nothing and costs no
  quota, so a retried upload is harmless. One wallet cannot touch another's
  rows.
- Erasing a turn, by request or by the retention purge, deletes in the same
  transaction its rating and the digest of its conversation. Deleting a
  digest, for any reason (replacing it with a newer one included), deletes
  in the same transaction every report finding whose evidence includes it.
  A finding's evidence is every digest whose notes were sent to the call
  that wrote it. Nothing in the node that was
  written from a creator's text outlives that text. A finding's title and
  text may already have been copied out of the node by then; see "The daily
  report" for why that is allowed and what keeps it safe.
- The analysis jobs write under the same rules: a digest is stored only in
  a transaction that holds the wallet's lock and finds every turn it covers
  still stored and the wallet's latest decision still a grant; a finding is
  stored only in a transaction that locks every digest in its evidence
  (`FOR KEY SHARE`, so a concurrent erasure waits; it takes no wallet
  locks, so the two cannot deadlock) and finds them all still there.
  Otherwise the result is discarded. An erasure that commits while a call to
  the provider is under way is never followed by a result built from what
  it erased.
- Every withdrawal and erasure is first written to the erasure log (see
  "Retention and backups"), then committed. When the entry cannot be
  written, the request answers 503 and changes nothing; Creator Hub retries
  withdrawals and erasures until they succeed.
- `CREATE_WRITES_ENABLED` gates the module through its own `writes` gate
  (`blobs: false`). Because the gate opens by path, erasure and consent have
  paths of their own: `/erase` and `/consent` are on the open-while-disabled
  list, `/turns` and `/ratings` are not. While writes are off, `PUT /consent`
  applies a withdrawal and answers a grant with 503.
- `AI_SHARING_ENABLED` (default false) switches the module's uploads and
  ratings on beneath `CREATE_WRITES_ENABLED`, so this module can stay off
  while the rest of the creator plane runs. Erasure, withdrawal and the
  reads answer whatever either switch says; erasure and withdrawal need only
  the erasure log.

### Quotas

Counted in SQL inside the write transaction, so they hold across replicas
and restarts:

- per wallet: 200 accepted turns in any 24 hours and 3,000 stored;
- per client address, read only from a trusted proxy as the referral quota
  reads it (`HTTP_TRUSTED_PROXY_CIDRS`): 1,000 accepted turns a day. The
  count lives in `ai_sharing.address_counts (day, address_hash, n)`, keyed
  by an HMAC of the address under `AI_SHARING_OPERATOR_KEY`, never the
  address, and is purged after two days;
- in total: `AI_SHARING_DAILY_TURNS` a UTC day (default 20,000), counted in
  a single counter row updated with
  `SET n = n + $k WHERE day = $today AND n + $k <= $cap`, since per-wallet
  locks do not serialize a cross-wallet count.

The gateway also rate-limits `/ai-sharing` writes per client address, with a
`limit_req` zone like its others. A request that would exceed a quota is 429
with `Retry-After` and stores nothing; Creator Hub keeps the records queued
and retries.

The attack these bound: fresh wallets cost nothing, so one actor can fill
the global day and refuse every creator's turns until it rolls over, or
store junk. The per-address limit and the gateway zone bound one source; the
global cap bounds storage. Together with the 128,000-byte record cap, the
caps hold storage to at most 2.56 GB a day and 461 GB at the 180-day
retention. Junk that gets in also reaches the analysis; see "The daily
report" for what that means for findings.

### Analysis

Two leased periodic jobs (`jobs.spawn_periodic_leased`), both hourly, both
off unless `AI_SHARING_ANALYSIS_ENABLED` is set and the provider is
configured, and both idle while the module is off (`AI_SHARING_ENABLED` or
`CREATE_WRITES_ENABLED` false) or the erasure log holds entries the
database has not applied (see "Retention and backups").

#### What the model is told about the assistant

Each Creator Hub release that changes the assistant's system prompt, skills
or tools adds a manifest to the module
(`ai_sharing/analysis/manifests/<systemPromptHash>.json`): the release, the
system prompt, and each skill and tool with its name, description and
version. Creator Hub's release generates it. Both prompts get the manifest
for the hashes they cover, so the model can tell a missing skill from one
that exists and was ignored, and the report labels each hash with its
release. A hash with no manifest is reported as such and analysed without
one.

#### Conversation digests

`ai-sharing-digest` takes settled conversations, oldest first. A
conversation is settled when its newest turn arrived more than six hours
ago. It belongs to the UTC day its newest turn arrived. It needs a digest
when it has none or has turns newer than the ones its digest covers, and it
qualifies only when:

- the wallet's latest decision is a grant;
- every turn was accepted under a consent version at or above
  `AI_SHARING_ANALYSIS_MIN_CONSENT_VERSION`;
- none of its turns is within seven days of `AI_SHARING_RETENTION_DAYS`, so
  conversations about to be purged are not re-sent as they age out.

For each one the node:

1. Computes the facts from the stored records, without a model: turn count,
   agents and models used, `systemPromptHash` and app versions, outcomes and
   error kinds per turn, tool names and counts, mutations, tokens, duration,
   and ratings.
2. Sends the conversation to the model with `digest.md` as the
   instructions and the manifest as context: every turn in order, with its
   prompt, response, tool names and details, outcome, error kind and
   rating. The instructions treat the conversation as data to describe,
   never as instructions to follow. Prompts are kept whole: they carry the
   creator's intent. When the input would exceed 300,000 code points, tool
   details are dropped first, then responses are cut to their first 2,000
   code points, then turns from the middle are left out and replaced by a
   marker that names their indices.
3. Asks for structured output against the schema in `digest.md`:
   - `goal`: what the creator was trying to make, in general terms;
   - `outcome`: `achieved`, `partial`, `not_achieved`, `abandoned` or
     `unclear`;
   - `frictions`: each `{kind, target, subject?, turnIndex, note,
     suggestion?}`, with `kind` one of `misunderstood_request`,
     `wrong_sdk_knowledge`, `tool_failed`, `tool_missing`, `skill_missing`,
     `skill_ignored`, `asset_not_found`, `scene_broke`, `slow_or_stuck`,
     `creator_corrected_agent`, `creator_stopped_reply`,
     `setup_or_sign_in` or `other`; `target` the part of the assistant it
     points at, one of `system_prompt`, `skill`, `tool`, `docs` or `app`;
     `subject` a skill or tool name from the manifest; and `suggestion` what
     might have prevented it;
   - `wentWell`: short notes.

   `digest.md` sets the counts and lengths; the node enforces them.
4. Checks the output for copying: no run of 40 or more characters, compared
   case-folded with whitespace collapsed, may also appear in the
   conversation's prompts, responses or tool details. The node asks once
   more with the rule restated; if the second answer copies too, the digest
   is stored with its enums, subjects and turn indices and without its
   notes. A secret-check hit (as for records) drops the notes the same way.
5. Stores the digest in `ai_sharing.conversation_digests`, keyed by
   `(wallet, conversation_id)`, with the facts, the output, the newest turn
   it covers, `digest_version` (a constant bumped whenever `digest.md`
   changes) and the model that wrote it, under the rules in "Writes and
   erasure".

Digests are per creator and can paraphrase what they asked, so they are
treated like the turns: keyed by wallet, readable by their wallet
(`GET /digests`), erased with any turn they cover and on withdrawal, shown
to operators only with a hashed wallet, and never copied out of the node.

#### The daily report

`ai-sharing-report` recomputes, on each pass, the counts of every UTC day in
the last three whose digests changed. Counts are final once they are three
days old; until then the report says how many of its day's conversations
still wait for a digest. Findings are written once a day, by the first pass
after 06:00 UTC (the pass reads the clock; the job itself only has an
interval), over the seven days ending yesterday. A digest that arrives
later is picked up by the next day's window.

- **Counts**, computed in SQL from the day's digests: conversations, turns
  and creators; outcome rates; friction kinds; ratings; error kinds;
  tokens. Each is sliced by Creator Hub release (through the manifest),
  agent and model, and set against the trailing 28 days. A slice shows
  rates only when it has at least 20 conversations from at least
  `AI_SHARING_MIN_WALLETS` wallets, and counts alone otherwise. Comparing
  releases is how a change to the prompt, skills or tools is judged after it
  ships.
- **Findings**, written by the model with `report.md` from the digests of
  those seven days. The model sees the digests' enums, subjects, notes and
  facts, never turns. SQL first groups the frictions by key: `(target,
  subject)` when the friction names a subject, `(target, kind)` when it
  does not. It keeps the 15 keys with the most wallets. Each key is one
  call to the model, with the group's counts, the manifest, and the notes
  of at most 40 of its digests, sampled one per wallet first. Each call
  returns at most one finding: `{title, problem, proposal?}`, where
  `problem` describes the pattern and `proposal` the change to make. Its
  evidence is the digests sent to the call.
- The key gives a finding a stable identity from day to day. Its status
  (`open`, `accepted`, `dismissed`, `shipped`), the wallet count when it was
  last set, and a link to the change that answered it live in
  `ai_sharing.finding_status`, keyed by the key and holding no creator
  text. A dismissed key is not reported again for 30 days unless its wallet
  count reaches double the stored one.
- Findings leave the node: people copy their titles and text into issues,
  pull requests and team channels, and the erasure cascade cannot follow
  them there. So before a finding is stored the node holds it to three
  rules, and drops it if any fails:
  - its evidence spans at least `AI_SHARING_MIN_WALLETS` distinct wallets
    (default 3);
  - it passes the copy check against the prompts, responses and tool
    details of every conversation in its evidence, and the secret check;
  - its words are shared. The text is split into tokens at every character
    that is not a letter or digit and at camelCase boundaries, so
    `my_scene.glb` gives `my`, `scene`, `glb`, and `GltfContainer` gives
    `gltf`, `container`. Every token of three or more characters must be a
    common English word (a bundled list), a term in a bundled glossary of
    the SDK's components, functions and concepts, a name in the manifest,
    or a token that appears in conversations of at least
    `AI_SHARING_MIN_WALLETS` of the wallets in its evidence.

  For the last two rules the node reads the turns of the conversations in
  the evidence; the model never does. The rules make a finding describe a
  pattern across creators and keep out most details that belong to one
  creator, such as a scene, file or person's name. They cannot catch a
  name that is also a common word or a two-letter token, and they do not
  make a finding trustworthy: fresh wallets cost nothing,
  so one actor can supply the wallets a finding needs, and a creator's text
  can try to steer the model. The control for both is people: every finding
  is reviewed in the console before anyone acts on it, and it shows its
  evidence.

Reports live in `ai_sharing.reports` (counts, keyed by day) and
`ai_sharing.report_findings` with `ai_sharing.finding_evidence`
(`(finding_id, wallet, conversation_id)`). Counts carry no wallet and no
text and are kept for two years. A finding is deleted with any digest in
its evidence; the next day's run may write it again from the digests that
remain.

#### The model provider

- The provider is configured by `AI_SHARING_ANALYSIS_BASE_URL` (no default;
  analysis is off without it, as `ANALYTICS_API_URL` is for social
  analytics), `AI_SHARING_ANALYSIS_API_KEY` (also readable from `…_FILE`) and
  `AI_SHARING_ANALYSIS_MODEL`. The key joins `THIRD_PARTY_SECRETS` with the
  base URL as its redirect key, and the base URL joins the `upstream` list,
  so loopback and mirror postures refuse a remote provider as they do for
  community compliance. Settings print the key as `[redacted]`.
- Launch takes a provider under a written agreement that it keeps no copy of
  inputs or outputs beyond the call and does not train on them, and the
  privacy policy names it. The client calls one documented API with
  structured output; adding a second provider is a code change, not a
  setting.
- What it receives: one conversation per digest call; for a report call,
  notes and facts from the digests of up to 40 wallets, with no wallet.
- Each call has a 120-second budget, at most three attempts, and waits on
  `Retry-After` up to 30 seconds. Redirects are refused. A conversation
  whose digest fails is tried again on the next pass; after three failed
  passes it is marked `failed` with a reason code and skipped until a
  re-run.
- A job that loses its lease during a call discards the result: before it
  stores anything it checks the job's cancellation token, which the jobs
  runtime trips when the lease's session lock is lost.
- Jobs log ids, status codes, reason codes and token counts. A provider's
  error body can echo its input, so it is never logged.

#### Cost and scale

- `AI_SHARING_ANALYSIS_DAILY_CONVERSATIONS` (default 2,000) and
  `AI_SHARING_ANALYSIS_DAILY_INPUT_TOKENS` (default 50,000,000) cap a UTC
  day, counted in one counter row as the turn cap is. A call reserves its
  estimated input tokens before it is sent and the counter is corrected to
  the provider's count after it returns, so a failed or discarded call still
  counts. Past either cap, the remaining conversations wait for the next
  day, oldest first, and the report states the backlog.
- At the planning figures under "Workload" (10,000 turns and about 1,500
  conversations a day), digests read about 20 million input tokens a day;
  the findings' 15 calls a day read under 2 million. The cost follows the model and
  is recorded in `DATABASE.md` with the workload once the provider is
  chosen.

#### Changing the prompts

`digest_version` and `report_version` are stored with what they produced.
Old digests stay as they are. `POST /admin/api/ai-sharing/digests/rerun
{from, to}` re-digests the conversations settled in a range under the
current version, within the daily caps; it is a console write, so it is
refused while `OPS_WRITES_ENABLED` is off or the module is idle, and it is
audited as `ai_sharing.rerun`. A report counts only digests of its own
version, and says so.

### Retention and backups

- A leased periodic job, `ai-sharing-purge` (`jobs.spawn_periodic_leased`,
  as `client-events-purge`), deletes turns older than
  `AI_SHARING_RETENTION_DAYS` (default 180) in batches through a
  `(received_at, wallet, record_id)` index, with everything derived from
  them as under "Writes and erasure", and nonces older than fifteen minutes.
- Report counts and finding statuses are deleted after two years.
- Consent history is the record of what a wallet agreed to and when it
  stopped. It is kept while the wallet's latest decision is a grant or it has
  stored turns; once neither holds, the purge deletes it
  `AI_SHARING_RETENTION_DAYS` after the latest decision.
- Backups that hold `ai_sharing` expire within `AI_SHARING_BACKUP_DAYS`,
  which the deployment sets and the privacy policy states.
- A restore would bring back turns, digests and findings that creators have
  since erased. The node keeps the erasure log to undo that.
  - Each withdrawal and erasure is one entry: a file named by its time and a
    random id, holding the wallet, `decidedAt` and the `recordId` if any (no
    content), written and flushed to disk in `AI_SHARING_ERASURE_LOG_DIR`
    before the database transaction commits. Every replica writes its own
    entries; there is no shared sequence. The directory is shared by every
    replica and lives on storage that a database restore does not roll
    back. The module stays off while it is unset.
  - The database records the id of each entry it applied, in
    `ai_sharing.erasure_applied`, in the transaction that applied it.
  - Replay applies every entry in the log that the database has not
    recorded, oldest first. Applying one twice changes nothing, so an entry
    whose own transaction failed after it was written is simply applied:
    the creator asked for it. Replay runs under a global advisory lock, so
    replicas never race, at every replica's startup and before every pass
    of either job. Until a replica's startup replay has finished, it
    answers uploads and ratings with 503.
  - A restore restarts the node, so the startup replay re-applies every
    erasure received since the backup before any upload or analysis runs.
  - Entries older than `AI_SHARING_BACKUP_DAYS` are deleted from the log
    and from `ai_sharing.erasure_applied`, since no backup that old
    remains.

### Who reads it

- The wallet itself, through `GET /turns`, `GET /digests` and
  `GET /consent`.
- The model provider, as under "The model provider".
- Operators, through the admin console behind `AdminSession` and a second
  allowlist, `AI_SHARING_READER_ADDRESSES`. Today `ADMIN_ADDRESSES` is the
  console's only allowlist; this is the first per-area one.
  - `GET /admin/api/ai-sharing/reports` and `…/reports/{day}`: counts and
    findings, with each finding's evidence. This is where the team works,
    and where it sets a finding's status (`PUT …/findings/{key}/status`, a
    console write under `OPS_WRITES_ENABLED`, audited as
    `ai_sharing.triage`).
  - `GET /admin/api/ai-sharing/digests` (filters: day range, outcome,
    friction kind, subject, release, agent, model; paginated): to check a
    finding against its evidence.
  - `GET /admin/api/ai-sharing/turns` (filters: time range, provider,
    outcome, conversation; paginated): to check a digest against its
    conversation.
  - Operators see a keyed hash of the wallet (`AI_SHARING_OPERATOR_KEY`),
    never the address. Filtering by a wallet takes the wallet and a stated
    reason (an erasure check, a creator's own request), and the reason is
    audited.
- Every operator read writes an `ops.admin_audit` row with domain `create`
  and action `ai_sharing.read` through `shared::audit::record`: the filters
  (a wallet filter as its hash), the reason when there is one, and the
  returned ids, never content. The audit row is not a console write, so it
  is written while `OPS_WRITES_ENABLED` is off; a read whose audit row
  cannot be written answers 503 and returns nothing. These are the table's
  first rows for reads; `shared/src/audit.rs`, which describes it as one row
  per operator mutation, is updated to say so. Audit rows follow the
  audit table's own retention; the ids in them point at nothing once
  erased.
- Building an evaluation or training set from turns or digests, and any
  copy of them outside the node, is a new decision.
- Database superusers and backups can read the schema; the console is the
  only application path to it.
- No database role is added.

### Logs and metrics

Handlers and jobs log ids, counts, sizes, status and reason codes, never a
record's fields, a digest's or finding's text, a request or provider body,
or a parser message. Metrics count accepted and rejected records by reason,
quota refusals by quota, bytes stored, digests written, failed and stripped
by the copy check, findings dropped by each rule, erasure-log replays, and
provider tokens.

### Workload

To be recorded in `DATABASE.md` before the tables are final. The planning
figures are assumptions, to be replaced with the `AI Turn Completed` counts
once Creator Hub ships:

- writers: 500 opted-in creators, 20 turns each a day, 8 KB typical record:
  10,000 rows and 80 MB a day; 1.8 M rows and 14 GB at 180 days. About
  1,500 digests a day at about 3 KB: 4.5 MB a day, 0.8 GB at 180 days. One
  report a day with up to 30 findings;
- readers: `GET /turns` and `GET /digests` by their own wallet (rare), the
  digest job (hourly, one conversation's turns at a time), the report job
  (hourly counts over three days of digests; once a day, grouped notes over
  seven days of digests plus the turns of the conversations behind each
  finding, for its checks), the erasure-log replay (each startup and job
  pass),
  the operator console (a few people, paged), the purge (hourly);
- invariants: the ones under "Consent", "Writes and erasure" and
  "Analysis".

## Consequences

- The node gains one write path for analytics-purpose data and one outbound
  processor for it, bounded as above. The telemetry ADR gains a line
  pointing here when this one is accepted.
- The team gets, every morning, what creators tried to build the day
  before, how often they got there, where the assistant failed them, and
  proposed fixes backed by several creators, each tracked from open to
  shipped, with counts by release to judge each change after it ships. At
  the planning figures most slices are small; rates become meaningful as
  more creators opt in.
- A creator can stop sharing in Creator Hub, which also stops the analysis
  and deletes its summaries; read and rate what they shared and read the
  summaries made of it; erase one turn or all of it with `POST /erase`,
  which erases every summary and finding made from it; and withdraw and
  erase while writes are off.
- The consent version is a contract between Creator Hub and the node. A new
  consent text bumps it in both; grants of the old version stop admitting
  turns, and a Creator Hub too old to show the new text stops offering
  sharing.
- Launch needs, outside this repo: the privacy policy and consent wording
  approved (lawful basis: consent; withdrawal as easy as granting; the
  model provider named in the policy; the AI analysis and the kept
  cross-creator patterns stated in the consent text; `AI_SHARING_BACKUP_DAYS`
  stated); the provider's agreement signed; the retention period confirmed;
  storage for the erasure log that a database restore does not roll back;
  Creator Hub's consent text updated as under "Consent" before its flag is
  turned on; Creator Hub's release writing the manifest; Creator Hub's view
  of what a creator shared (`GET /turns`, `GET /digests`) and its rating
  control (`PUT /ratings`; the analysis works without ratings and uses them
  once they arrive); its `AI_SHARING_API_URL` set and its feature flag
  turned on.
- Next work: the module, `schemas/51_ai_sharing.sql`, the `DATABASE.md`
  workload section, the two prompts and their schemas, the common-word list
  and the SDK glossary,
  tests on the scratch database (the lock, quotas, replay, cross-wallet ids,
  consent ordering, erasure while writes are off, erasure and withdrawal
  reaching digests and findings, an erasure racing a provider call, the
  erasure-log replay after a simulated restore, the three finding rules,
  analysis staying idle while the module is off, the secret backstop), a
  provider test double for the jobs, `config/src/create.rs`,
  `config/src/posture.rs`, `src/serve.rs`, `src/ops/admin/create.rs`, the
  console pages, the gateway zone, and `.github/CODEOWNERS` regenerated with
  `tooling/owners.py` if `ownership.json` changes.

## Alternatives

- **Segment.** Rejected: its documented 32 KB per-call limit is below a
  record's size, and it has no way to refuse an event from a wallet that has
  not consented.
- **A crate of its own.** Rejected: nothing but this module would depend on
  it, and the layout rule asks for an independent consumer or a build
  requirement.
- **Raw turns in object storage or a data warehouse, analysed there.**
  Rejected: erasing one creator means finding and rewriting files or
  partitions, and every copy outside the node is one more place an erasure
  has to reach. The volume is small enough for Postgres, and the analysis
  needs one conversation at a time, not scans over all of them.
- **Analysis outside the node** (a notebook or a separate service reading
  an export). Rejected: it needs the export this ADR keeps out, and its
  outputs would escape the erasure cascade and the finding rules.
- **Summarising each turn as it arrives.** Rejected: a turn alone rarely
  shows whether the creator got what they wanted, and it ties uploads to the
  provider's latency and outages.
- **Only counts, no per-conversation digests.** Rejected: SQL cannot tell a
  misunderstood request from a missing skill, and a finding with no digests
  behind it cannot be checked.
- **One report call over all the week's digests.** Rejected: seven days of
  digests are tens of megabytes, far past one call. Grouping in SQL first
  keeps each call small and gives findings a stable key.
- **Findings from any single conversation.** Rejected: a finding is copied
  out of the node, and one built on one creator can describe that creator.
- **Operators reading turns to find problems.** Rejected as the main path:
  it does not scale and puts more text in front of more people. Reading a
  conversation stays possible, audited, to check a digest.
- **Relying on the restore stamps after a restore.** Rejected: they prove
  backups and drills are fresh, not that erasures since the backup were
  re-applied, and they gate the whole creator plane, not this module.
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
- **A model-based personal-data filter on upload.** Not decided here. The
  node refuses the two highest-risk secrets; the client removes keys, seed
  phrases, emails, phone numbers, IP addresses and home-folder paths. A
  model-based pass is a separate decision, taken if the digests or a sample
  of stored records show the client rules miss things.
