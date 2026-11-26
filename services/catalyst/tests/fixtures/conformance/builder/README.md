# Builder conformance

These files compare the node's builder routes with
`github.com/decentraland/builder-server` at
`98897af692f26063a088aebb2ea5bb3f683168a2`. Each capture is a request and
the answer the pinned upstream gave to it. The parity ledger records, per
operation, what the node's replay of the captures shows. The replay and its
tests are pending (see Tests).

## Files

The folder holds the ledger, the seed both sides start from, and the
captures.

| Path | Holds |
| --- | --- |
| `parity.json` | The per-operation parity ledger |
| `seed.json` | The synthetic wallets and collections both sides start from |
| `source/*.json` | Captures of the pinned upstream, written by `tooling/builder-server-capture.cjs` |
| `live/*.json` | Dated captures of the public service; none are recorded |

Captures use the shared conformance fixture format
(`services/catalyst/testing/src/conformance/fixture.rs`): `request` with
`method`, `path`, `query`, `headers` and `body`; an upload's capture carries
`multipart` parts instead of a body (each part's field `name`, `filename`,
`type` and synthetic `text`), a field the builder port adds to that format,
and the replay sends them as form data; `response` with `status`,
the kept headers and `body_json` or `body_bytes_b64`; `captured_from`,
`captured_at` and `volatile_paths`. A capture whose request made upstream
post to Slack carries `slack`: the JSON bodies upstream posted to Slack's
`chat.postMessage`, in the order they arrived. A capture that calls the forum
carries `forum`: each call's `method`, `path`, `username`, and JSON `body`
(null for a GET), in arrival order. The replay compares every forum field,
including topic and post identifiers. A capture whose request made upstream
request an automated review carries `queue`: each validation manifest it
published to its events topic, without its random `validationId`
(`collectionId` and `items`), in arrival order. A capture of the wearable
validator's call carries `validator_sign`, the defect its signature carries:
`{}` for none, an `offset` in milliseconds from the clock, or `"secret":
"wrong"` for a signature under another secret; it names no wallet signer,
`sign` or `auth`, and the replay signs it again with the node's scheme and
that defect. A capture recorded with the review flow on carries
`review_flow: true`: the capture turns upstream's
`builder-auto-curation-review-flow` on for that request only, and the replay
turns the node's on in its `builder` flags document for it. A validator's
capture that answers a running review carries `validation_of`, the
collection whose review it answers: the capture sends that collection's
newest start's `validationId` and records the body as written, with its
signature over that body, and the replay sends the node's open request of
that collection and the claim it holds instead. A capture whose request made upstream
forward a subscription to the newsletter service carries `newsletter`: each
call's `method`, `path`, `authorization`, `accept`, `content_type` and JSON
`body`, in arrival order; the replay forwards the node's to a fake newsletter
service that, as the capture's, answers 500 to `fail@example.com`. A
capture's request may carry headers other than `x-identity-` ones, such as an
`origin` or a preflight's `access-control-request-*`, which the replay sends
as recorded; a preflight (`OPTIONS` with `access-control-request-method`)
exercises the operation of the method it asks for. Upstream and the replay
run with the same `CORS_ORIGIN`. A capture's response records
`content-type`, `cache-control` and `location`, and the other headers its case
is about. A capture with `repeat` was the last of that many plus one equal
requests in a row: the capture required each earlier one to answer its case's
`repeat_status` and the last one its `status`; the replay sends it as many times first, with the signer's
review budgets fresh before and after. A volatile leaf is recorded as
`<<VOLATILE>>`, which matches any value on replay. A capture whose
volatile timestamps were equal before masking lists them in
`volatile_equal`. No other marker may appear in a builder capture: a
`<<REDACTED` string anywhere, or a `<<VOLATILE>>` that none of its
`volatile_paths` reaches, refuses the capture. A capture whose signature is the point of the case
carries `sign`, the one defect it was signed with (below); the replay
signs it again at replay time with the same defect, so the node's
refusal comes from the check the case names. A capture of literal headers
is marked `"auth": "recorded"` and replayed with its own `x-identity-*`
headers. A capture carries one or the other, `sign` must name a recorded
signer and only the fields below, and `auth` takes no other value. `sign`
names a defect: a non-zero `offset`, a `path`, a `method` or a
`sentMetadata` (with the signed `metadata`), or `untimestamped`, which
takes no `path`, `offset`, `metadata` or `sentMetadata`; `withHeaders` goes
only with `untimestamped`. The capture script refuses a case that breaks
these rules, as the ledger test refuses such a capture.

## Parity ledger

`parity.json` lists the operations of the pinned upstream that the builder
port serves; the port's route ledger, pending until the port adds it
(`services/catalyst/builder/src/tests/upstream_routes.rs`), names each one
`Routed` or `Todo`. Its schema is the social family's (`../social/parity.json`): each operation has
`id` (`api_http:<METHOD>:/v1<path>`), `owner`, `status`, `result`,
`evidence`, `gaps` and `remaining_verification`.

- `not_reviewed`: the captures it cites, if any, don't settle the
  operation.
- `known_gap`: it lists at least one open gap.
- `reviewed`: it cites at least one capture and lists no open gap: every
  difference its captures replay with is explained by a deliberate gap. It
  covers the cases those captures exercise, not every input.

`deliberate_gaps` states, once each, a difference the node keeps on
purpose, by name: its `text`, the `captures` it explains notes on (paths
relative to this folder, such as `source/92-collection-save-extra-field.json`)
and, for a gap about a refusal or a status, its `anchors`, the notes a capture
must record before the gap explains any other note there. `open_gaps` states
the differences the node doesn't keep on purpose the same way, with `closes`:
the behavior of the node that would close it. An operation lists them among
its `gaps` as `deliberate:<name>` or `open:<name>`, and lists nothing else.
Every reference must name a stated gap, and every stated gap must be
referenced. An evidence entry maps each note it records to the gap that
explains it in `explained_by` (`{"<note>": "deliberate:<name>"}` or
`"open:<name>"`); that map is the one list of the notes a gap explains on that
capture. The ledger test refuses a recorded note no gap explains, a mapping to
a gap its operation doesn't list, a mapping to a gap that doesn't list the
capture, a note whose gap's anchors the capture doesn't record, a mapping of a
note the capture doesn't record, a gap that lists a file that isn't a capture,
and a gap that lists a capture none of whose notes it explains. The capture
script refuses to run when a gap lists a `source/` capture none of its cases
records, with the ledger test's message.

The ledger guarantees this for every operation. Each note a cited capture
records is attributed to exactly one gap that the operation lists and that
lists that capture, and every capture a gap lists records at least one note
attributed to it, so a gap explains differences only on the captures it names
and adding a capture to a gap is an edit of that gap. A gap about a refusal or
a status explains a note only on a capture that also records its anchors.
`reviewed` lists no open gap, `known_gap` lists at least one, and every open
gap says what closes it. The replay reproduces exactly the recorded notes. The
ledger doesn't check that a gap's text describes the notes attributed to it,
or that a gap labeled deliberate is kept on purpose; review decides both.

An evidence entry whose path is in this folder must be a capture directly in
`source/` or `live/` whose request method and path match the operation (a
`:name` segment matches one non-empty segment; a `/builder-api` prefix, a
query string and a trailing slash are ignored), and must carry `replay`: the
exact notes the replay produces for it, empty for a match. A capture whose
request matches more than one in-scope operation is refused. A capture's
`volatile_paths` may only end in a field named in `volatile_fields`
(`created_at`, `updated_at`, `last_activity_at`, `reviewed_at` and `id`, for a
row the database names), and the recorded value there must be
`<<VOLATILE>>`. A note is `status: ...`,
`header ...`, a body path with `missing`, `extra` or `value`, `body bytes
differ: ...`, `candidate body is not JSON: ...`, `slack: ...` or `newsletter: ...`, as
`social::test_fixtures::replay_signed` words them.

An evidence entry outside this folder names a node test for what no capture
holds, such as a bound: `path` is the source file and `test` the test
function in it, which the ledger test requires to be there.

## Tests

Pending: the tests that hold the ledger to the captures and replay every
capture against the node are `services/catalyst/tests/builder_conformance.rs`,
which arrives with the builder port, and the ledger's evidence entries outside
this folder name builder tests that arrive with the port too. Until then the
statuses in `parity.json` record the review of the captures against the port,
not a replay, and no test checks them against the node. What this file says
the ledger test and the replay test refuse or require describes those tests.
`node --test tooling/tests/builder-server-capture.test.cjs` checks this folder
now: every capture parses into the shape the replay reads and is a case of the
capture script, the seed holds only the fields both seeders apply, and every
`source/` capture a gap lists is a case.

`seed.json` accepts only the fields both seeders apply; any other field
fails the replay test with its name. `collections` are the builder's rows,
each with the contract address its draft computed, and a third-party one with
its provider id, URN suffix and, optionally, its linked contract address and
network; a collection may carry its `forum_link` and `forum_id`. The capture stores a third-party row with an empty `eth_address`, as
upstream stores one; the replay stores the owner the seed names, which
decides nothing on the node. `items` are the builder's item rows,
with their `contents` (file to hash), `rarity` and, for a third-party item, the
item part of its URN (`urn_suffix`; the replay stores the full URN the node
keeps) and, optionally, its `local_content_hash` (64 lowercase hex digits).
`collection_curations` are a collection's curation rows (status, the
assignee's wallet role or null, and their dates) and `item_curations` an
item's (status, a 64-digit lowercase hex content hash, the mapping flag or
null, and their dates); both sides insert them as they are.
`third_parties` are virtual third parties, each with its name and managers:
both sides insert them as virtual third party rows (`tp:1:<name>:`).
`registry` names the providers the third-party registry lists, each with its
managers, whether it is programmatic and, with `approved: false`, an
unapproved one (approved otherwise): the capture serves them from its fake
third-party subgraph to upstream, and the replay from a loopback registry to
the node. A provider can be in both.
`receipts` are the slot cheques the registry already consumed, each with its
provider, quantity, salt and signing wallet role: the capture serves them as
the third-party subgraph's `receipts`, and the replay records them in the
node's registry index under their provider, which the registry must list.
`entities` are the catalyst's active entities, each with its entity id, type,
pointer and metadata: the capture's fake answers an active-entities read with
the seeded ones whose pointers it asks for, and the replay deploys them in the
node's content tables. The
`admin` wallet is upstream's `ADMIN_ADDRESSES`. `chain` names, once, what
the Polygon collections subgraph holds: the committee's roles, its
collections (contract, name, creator and owner roles, approval, manager and
minter roles and unix timestamps), their items, and the rarities (name, USD
`price`, `max_supply`, and the `mana_price` the rarities-with-oracle
contract answers). The capture serves `chain` from its fake subgraph and its
fake Polygon JSON-RPC to upstream; the replay writes the same collections and
items into the node's copy of that subgraph (`shop.collections` and
`shop.items`), records a fresh complete pass of both mirrors in
`shop.mirror_state`, and serves the committee (as the accounts marked
`isCommitteeMember`) and the rarities to the node's readers from a loopback
fake subgraph, and
the oracle prices, Polygon's chain id, code at each `chain`
collection's contract and that contract's item count from a loopback fake
JSON-RPC, so a collection is published, a wallet is on the committee and a
rarity has its prices on both sides exactly when `chain` says so. The node's
clock, which sets and compares a collection's lock, is the capture's fixed
clock.

## Capturing the pinned upstream

`tooling/builder-server-capture.cjs` runs upstream's own Express app and
records the cases listed in its `CASES` table. It needs Node 24, matching
the major version upstream's `Dockerfile` pins. It shares its checkout and
database-URL checks with the social captures
(`tooling/social-capture-common.cjs`).

The script refuses a checkout that isn't the Git working tree root, isn't
at the pinned revision or has staged, unstaged or untracked changes. It
installs upstream's locked dependencies with `npm ci --ignore-scripts`, which
replaces whatever `node_modules` held. Upstream and its migration process
run with the capture's settings and only `PATH`, `HOME`, `TMPDIR` and `TZ`
of the shell's environment, and each runs in a new empty directory;
upstream's settings are loaded into its `decentraland-commons`
env, so no `.env` file is read. Then it:

1. Creates its own database, `builder_capture_<pid>_<random>`, on the
   cluster `CATALYST_TEST_DATABASE_URL` names, and drops it at the end of
   the run, also when the capture or its teardown fails, and on `SIGINT` or
   `SIGTERM`, including one that arrives while the database is being
   created; a second signal waits for the same drop. An uncaught error is
   reported redacted after the same drop. Upstream runs with that URL's credentials, so use a role that
   can create databases and owns nothing else. The cluster must be on
   localhost, a loopback address or a Unix socket (a `host=/path`
   parameter or a URL-encoded path as the host): the script refuses any
   other host before it installs anything, since the offline guard would
   refuse the connection.
2. Runs upstream's migrations, stopping the run if they take more than
   five minutes, and inserts the rows of `seed.json`.
3. Starts a loopback fake for the collections and third-party subgraphs,
   which answers the collections subgraph's `accounts`, `collections` and
   `items` fields with the `seed.json` `chain` rows their `where` argument
   selects (by `id`, `id_in`, `id_gt`, `creator`, `managers_contains`,
   `minters_contains`, `collection` or `isCommitteeMember`; ids, creators and
   collections compare exactly, as The Graph does, and a `first` above 1,000
   or a `skip` above 5,000 answers The Graph's error; a collection carries its
   items), its `rarities` field with the `chain` rarities (`id`, `name`,
   `price` and `maxSupply`, ordered by id) and the third-party subgraph's `thirdParties` with
   the `seed.json` `registry` rows (by `id`, `managers_contains` or
   `managers_contains_nocase`; with no `__typename`, which The Graph answers
   only when a query selects it). It answers the catalyst's active-entities read with the
   `seed.json` `entities` whose pointers it names, and the Polygon JSON-RPC's `eth_chainId`, `net_version`, `eth_getCode`
   (code only at a `chain` collection's contract) and the `eth_call` of the
   rarities-with-oracle contract's `getRarityByName` (a `chain` rarity's
   `max_supply` and `mana_price`; a revert for another name), records the
   events upstream forwards to the warehouse (a `POST` with its token), and
   stores, copies and deletes the objects an upload sends to S3. It answers
   the peer's profile read with no profile, so upstream names a person by
   its shortened address, and answers Slack's `chat.postMessage` as Slack
   does: a message with the configured bot token, a channel and a text is
   recorded and answered `ok: true`, and anything else is a refusal that
   fails the capture. The forum fake starts with the seeded collections'
   topics, reads a post's topic with `GET /posts/{id}.json`, and creates
   topics or replies with `POST /posts.json`. It checks the API key and
   records the path, account, and body of every call. The configured forum
   origin is `https://forum.example.org`; requests to that host and
   `slack.com` reach their fakes over loopback. The other peer, RPC,
   warehouse, S3, SNS, and OpenSea settings point at the fake directly. The
   events topic's fake answers a `Publish` of a validation request on the
   configured topic, records its manifest, and refuses anything else. The
   bot token is `xoxb-example123`, the channel `curation-example`, and the
   feature flags are set through their `FF_*` overrides as production has
   them: `builder-auto-curation` on, its review flow and the alternative
   collections API off. A case waits until every automated review request
   upstream started in the background has ended: in a message on the events
   topic, a curators' notice that none was requested, or no changed item.
4. Loads `tooling/builder-server-offline.cjs` into its own process and,
   with `--require`, into the migration process. The guard refuses to load
   while `HTTP_PROXY`, `HTTPS_PROXY`, `ALL_PROXY` or `NODE_USE_ENV_PROXY` is
   set in either spelling, so no proxy configured before it can carry a
   request. It refuses a TCP connection or a UDP datagram to anything but
   localhost or a loopback address (one that brings its own lookup function: to
   anything but a loopback address), a lookup (`dns.lookup` and
   `dns.promises.lookup`) of any other name, and every nameserver query (the
   `resolve*` and `reverse` functions of `dns`, `dns.promises` and their
   resolvers), whatever its name. The
   capture fails if anything was refused, if upstream asked the fake for
   another subgraph field or filtered by a `where` key the fake doesn't
   apply, called another JSON-RPC method, or called a fake route other than the subgraphs, the JSON-RPC, the
   active-entities read, the profile read, the warehouse, Slack, forum, the
   events topic, and S3 object writes. A case
   that needs another answer adds it to the fake first. The guard covers
   `net` connections and `dns.lookup` on the main thread of the capture
   process and on every thread of the migration process. It
   doesn't cover worker threads the capture process starts, other child
   processes or native code.
5. Fixes the clock at `2026-01-01T00:00:00.000Z`, so timestamps and
   signatures are the same on every run. Values the database clock sets are
   listed in the case's `volatile` paths. A volatile path must end in a
   field named in `parity.json` `volatile_fields` and match a recorded
   value; the replay test holds committed captures to the same rule.
   Upstream sends its Slack notices without waiting for them, so after each
   answer the capture waits until every notice upstream started has
   finished and records the messages Slack received meanwhile. The
   capture fails if upstream's answer or the request already holds a
   marker, and records which volatile values were equal before masking.
6. Signs a case with a `signer` over the timestamped payload and its
   `metadata` (`{}` by default), with the one defect its `sign` names:
   `offset` moves the timestamp by that many milliseconds, `path` or
   `method` signs another one, `sentMetadata` sends other metadata than was
   signed, and `untimestamped` signs only the method and the path below
   `/v1`, with no timestamp or metadata header, or with both when
   `withHeaders` is set. The capture keeps `sign`, with the signed
   `metadata` for a timestamped case. A case's `headers` sends literal
   headers and marks the capture `"auth": "recorded"`. The wallets are
   derived from the public labels
   `dcl-one builder conformance <role>` and `... <role> ephemeral`
   (keccak-256 of the label is the private key). `seed.json` records each
   role's address and both the script and the replay check it. Anyone can
   derive these keys: never configure the addresses as an admin or committee
   member anywhere.
7. Writes the captures to a staging folder beside the output folder and,
   only after every check passed, moves the output folder aside, moves the
   staging folder into its place and then deletes the old one, so a failed
   run leaves the output untouched and a passing one leaves no capture of a
   removed or renamed case behind. It refuses an output folder that holds
   anything but `.json` files.

To capture, check upstream out at the pin in a scratch folder and write into
a folder of its own:

```sh
git clone --no-checkout https://github.com/decentraland/builder-server.git "$SCRATCH/builder-server"
git -C "$SCRATCH/builder-server" checkout --detach 98897af692f26063a088aebb2ea5bb3f683168a2
CATALYST_TEST_DATABASE_URL=<disposable-cluster-url> \
  node tooling/builder-server-capture.cjs "$SCRATCH/builder-server" "$SCRATCH/out"
diff -r "$SCRATCH/out" services/catalyst/tests/fixtures/conformance/builder/source
```

A run reproduces the committed captures byte for byte. Compare its output
with `source/` before replacing anything.

The script's own tests run on any Node version, in the `Social source
captures` workflow:

```sh
node --test tooling/tests/builder-server-capture.test.cjs
```

### Adding a capture

A new case goes through the script, the ledger and the replay in that
order.

1. Add the case to `CASES` in `tooling/builder-server-capture.cjs`: a
   `name` with the next number prefix, a `description`, `method`, `path`,
   optional `query` (each value a string, or a list of strings for a
   repeated parameter), `signer` (a `seed.json` role), `metadata`, `sign`,
   `headers`, `body` or `multipart`, and `volatile`. Write a `body`'s keys in
   sorted order at every level: the replay re-serializes it with sorted keys,
   and an item's content hash depends on the order of its documents' keys.
   Keep `multipart` text small and synthetic. Both sides run the captures in the order of
   their numbers against one database, so a capture sees what the writes
   before it left; the script and the ledger test refuse two captures with
   one number, and a case listed before a lower number.
   Add any rows it needs to `seed.json`, and teach both seeders (the
   script's `seedDatabase` and `SEED_KEYS`, the test's `Seed` and `seed`)
   the new field. If upstream asks the fake for a new subgraph field, add
   its answer to the fake.
2. Run the capture and copy the new file into `source/`.
3. Run the tests. The ledger test names the uncited capture and the replay
   test prints the notes it produces.
4. Cite the capture in its operation's `evidence` with a `scope`, the
   `replay` notes and `explained_by` for each note, then set the status:
   `reviewed` when the operation's cases are covered and no open gap
   explains a note, `known_gap` otherwise.

### Marking a gap

Record a difference where the replay shows it.

1. Copy the replay's notes into the capture's `replay` list unchanged.
2. Map each note to the gap that explains it in `explained_by`, and add the
   capture to that gap's `captures`. A difference the node keeps on purpose is a
   `deliberate_gaps` entry; any other is an `open_gaps` entry stating what
   upstream answers, what the node answers and what closes it. The operation
   lists the gap, and is `known_gap` while it lists an open one.
3. When a change makes the node match, the replay test fails on that
   capture; empty its `replay` list and its `explained_by`, drop the capture
   from the gap's `captures`, remove a gap left explaining nothing that no
   operation needs, and update the
   status in the same change.

## Dated captures of the public service

A capture of `builder-api.decentraland.org` goes in `live/` in the same
format, with `captured_from` set to the full URL and `captured_at` to the
UTC time of the request. Capture only unsigned reads of public data, one
request at a time with a pause between them, and replace every address and
name that identifies a person with a synthetic one before committing. Mark
fields that depend on production data as volatile. The replay test reads
`live/` like `source/`, so each dated capture is cited by one operation
with its `replay` notes.
