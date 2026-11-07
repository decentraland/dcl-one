# Explore parity fixtures

What the old places and events services answer, per request, derived from their
pinned source: `github.com/decentraland/places@e660fbc2` and
`github.com/decentraland/events@b60d5bbd` (both equal to production on
2026-11-04) plus the `decentraland-gatsby@9.0.1` framework they run on. The
`GET /api/places` entries derive from
`github.com/decentraland/discovery-server@81f0dcae` instead, the service that
serves the route in production. The
harness `tests/explore_parity.rs` sends every entry to the explore router over a
seeded scratch database and compares the answer. Dated production captures
exist for the same requests, but they stay private; nothing here comes from a
capture, so every expectation can be re-read in the pinned source.

## Running

```sh
cd services/catalyst
CATALYST_TEST_DATABASE_URL=postgresql://user:pass@localhost:5432/postgres \
  cargo test -p catalyst --test explore_parity -- --nocapture
```

Without `CATALYST_TEST_DATABASE_URL` the test skips. Every entry runs against a
freshly reseeded database (the seed below), so entries are independent and their
order does not matter. The summary line names the skipped ids; on failure the
test prints one line per entry: `PASS`, `FAIL` (fails, not listed), `GAP`
(listed and failing as recorded), `CHANGED` (listed, failing differently),
`FIXED` (listed and passing), `SKIP`, `BAD` (a defect in the fixture itself:
unknown seed or placeholder, bad header, unexpected skip, `source.kind` not
`source`; never a gap).

`known_gaps.json` is the ratchet: the fixture ids the router does not answer like
the pinned source yet, each with the observed difference (uuids, timestamps and long
numbers masked). The test fails on an entry that fails and is not listed, on a
listed entry that passes (remove it), and on a listed entry whose observed
difference changed (check it did not get worse, then refresh). Fixture ids that
fail and are not listed are never added by hand:

- `EXPLORE_PARITY_WRITE_GAPS=1` rewrites the list from the run: it drops the
  ids that pass, refreshes the observed differences and keeps everything else;
  it never adds an id, and the run still fails on unlisted failures.
- `EXPLORE_PARITY_WRITE_GAPS=1 EXPLORE_PARITY_ALLOW_NEW_GAPS=1` also adds the
  unlisted failures and prints `added <id>: <observed>` for each; every added id
  is recorded under "Known gaps" below with the pinned source lines it diverges
  from and the answer the router gives (a fixture defect is fixed, never
  listed). Use it only when a change knowingly surfaces gaps (new fixtures,
  stricter comparison).

The flags compare to the value `1`; `EXPLORE_PARITY_WRITE_GAPS=0` is off.

### Known gaps

The listed entries, each with the pinned source lines it diverges from and the
answer the router gives:

- `events.status.envelope` and `places.status.shape`: the old services mount
  the framework's `status()` without a handler of their own
  (`events@b60d5bbd src/server.ts:48-58`, `places@e660fbc2 src/server.ts:78`,
  `decentraland-gatsby@9.0.1 dist/entities/Route/routes/status.js:9-26`), so
  `data` is exactly `{image, version, timestamp}`. The router's `status`
  handler (`explore/src/lib.rs`) answers those three keys plus `catalogue`,
  `configured_mirrors` and `mirrors`, the node's mirror state. Whether the
  extras stay on `/api/status` or move to `/v1/status` is open in the parity
  ADR (`docs/adr/2026-11-05-explore-api-parity-contract.md`).
- `events.create.image_bucket_unconfigured`: without a poster store the old
  events service answers 503 `Poster uploads are unavailable because
  AWS_BUCKET_URL is not configured` (`src/entities/Event/utils.ts:837-843`).
  The router answers the same status and code, `service_unavailable`, with
  `Poster uploads are unavailable because this node has no blob store
  configured` (the poster guard in `explore/src/events/writes.rs`): posters
  live in the blob store and no bucket variable exists. Whether the text or
  only the status and code are the contract is open in the parity ADR.

### Skipped and accepted entries

The entries whose id contains `.$defs.` are definitions, not requests: the
shared row key sets (`expected.body.item_keys`, named by other entries'
`item_keys`), the auth vocabulary and the validation envelope. They and
`worlds.put_ranking.data_team_race` (a lost-update race that cannot be staged)
carry `harness.skip`; any other skip is `BAD` (`SKIPPED` in
`tests/explore_parity.rs`).

Twelve entries record dcl-one's accepted answer rather than the old service's
(`ACCEPTED_ENTRIES` in the harness). Four are where the old events service 500s, which
dcl-one never reproduces: `events.sitemap.events_page_negative`
(`page=-1`), `events.get_schedule.not_uuid`,
`events.create.missing_categories_500` (201 with `categories: []`) and
`events.attendees.post_duplicate` (the idempotent 201 list). Two more are a
fractional integer field: the old schema types `x`, `y` and `duration` as `number`
(`Event/schemas.ts:580-583,622-631`) and the INTEGER and BIGINT columns then fail the
write (`migrations/1591117940223_add-position-props-to-events.ts:9-16`,
`1592405071787_update-duration-prop-on-events.ts:8-10`), another 500; dcl-one answers
the ajv `type: integer` error (`events.create.x_decimal`,
`events.patch.duration_decimal`). One is a schedule `image` with a `javascript:` or
`data:` scheme, which the old `uri` format admits (`Schedule/schema.ts:103-107`);
dcl-one requires the event poster's `^https?://` pattern (`Event/schemas.ts:562-567`) by
design and answers the `pattern` keyword (`events.schedules.post_image_not_http`). Two are the rating
code `RP`, which the old places service stores and answers but refuses in a PUT body
(`places.put_rating.rp_accepted`, `worlds.put_rating.rp_accepted`: dcl-one
accepts `PR` and `RP` and answers the code verbatim). Three are the old `edit_any_profile`
delegation (`ProfileSettings/utils.ts:4-11`): dcl-one keeps private profile reads and
permission grants moderator-only, so a grantee is 403 on `GET /api/profiles/settings` and on
`GET` and `PATCH /api/profiles/{id}/settings` (`events.profiles.list_edit_any_profile`,
`events.profiles.get_by_id_edit_any_profile`, `events.profiles.patch_by_id_edit_any_profile`).
They are checked like
every other entry but are excluded from the ratchet: they can never appear in
`known_gaps.json`, so a failure there is a regression, not a gap. Every other
entry must have `source.kind: "source"`.

The seeded events start in 2031 and the seeded schedule ends in 2036; the
time-dependent list entries (`events.get_events.default_anonymous`,
`list_live`, `list_upcoming`, `events.get_schedules.list`) hold until then.

## Files

| File | Content |
|---|---|
| `places.writes.fixture.json` | places and worlds interactions, admin PUT/DELETE, destinations ranking, report; `places.$defs.place_write_row` and `worlds.$defs.world_write_row` define the admin write answers |
| `events.writes.fixture.json` | events create/patch/delete, admin PATCH, attendees, posters, schedules, profiles, subscriptions; the `events.$defs.*` entries define the shared records, the auth vocabulary and the validation envelope |
| `places.reads.fixture.json` | places, categories, map, destinations, worlds, world names, social cards, unknown route, batch POST status codes; the `places.$defs.*` entries define the shared rows |
| `events.reads.fixture.json` | events list validation and visibility, event by id, attendees, categories, schedules, sitemaps, unknown route |
| `known_gaps.json` | the ratchet |

## Entry schema

```json
{
  "id": "places.get_places.invalid_limit",
  "area": "places",
  "request": {"method": "GET", "path": "/api/places", "query": {"limit": "abc"}, "headers": {}, "body": null, "auth": "none"},
  "expected": {
    "status": 400,
    "content_type": "application/json",
    "body": {"ok": false, "error": "...", "code": "bad_request"},
    "body_match": "subset",
    "shape": {"envelope": [], "data_type": "array", "item_keys": [], "optional_keys": [],
              "data_subset": {}, "invariants": [], "allow_empty": false, "root": "urlset"},
    "headers": {"link": "<https://decentraland.org/places/place/?position=10.10>; rel=canonical"}
  },
  "source": {"kind": "source", "repo": "decentraland/places@e660fbc2", "refs": ["src/entities/Place/schemas.ts:17-33"]},
  "harness": {"place_id": "disabled"},
  "notes": "free text"
}
```

- `query`, `headers`, `body` and `auth` may be omitted: the defaults are empty,
  empty, none and `none`. `python3 tooling/explore-fixtures-format.py` rewrites
  the files in their canonical layout (`--check` only reports).
- `query` values are strings; an array parameter uses the bracket key
  (`"positions[]": "10,10"`); several values go in the path's query string.
- `headers` are sent as given; a value starting with `<` is a description and is
  not sent. `auth` adds the identity headers (below) on top.
- `body`: JSON is sent as `application/json`; `null` sends no body; a string
  starting with `<` is a description (posters use `harness.multipart` instead).
  Object keys are sent in sorted order (the catalyst workspace forbids
  `serde_json`'s `preserve_order`, and the router parses bodies the same way), so
  where the old service's answer depends on key order (`Unsupported admin event fields:
  …` joins keys in request order), write the body's keys sorted and expect the
  sorted list.
- `body_match`:
  - `exact`: the whole JSON body equals `expected.body`; a string body compares as text.
  - `subset`: every key in `expected.body` is present with the same value,
    recursively; arrays compare whole; a string `"<...>"` is a wildcard, and a
    string with wildcards inside it, such as `"Invalid signature: <verifier
    message>"` or `"poster/<uuid>.png"`, matches when its literal pieces appear
    in order (the first as a prefix, the last as a suffix).
  - `shape`: `envelope` is the exact top-level key set; `data_type` says whether
    `data` is an array or an object; `item_keys` is the exact key set of `data`
    (object) or of every item (array), or the id of the `$defs` entry that holds
    it; `optional_keys` may be present or absent;
    `data_subset` is a subset match on `data` (or on every item); `root` is the
    XML root element. Every key of `body` other than `data` (`"ok": true`,
    `total`, ...) must be present and equal; `body.data`, when given and not a
    wildcard, is a `subset` match on `data` (its wildcards and patterns as
    above), except that an empty array `[]` only says "an array", whose items
    `item_keys`, `data_subset` and the invariants describe. An empty `data`
    array fails unless `allow_empty` is true or an invariant pins it
    (`data.length == 0`, `total == 0`): a list fixture must see rows, or say
    that empty is the answer. `invariants` are strings in this grammar; a string
    outside it fails the entry (a mistyped check never passes silently), so
    documentation goes in `notes` or starts with `doc:`:
    - `data.length <op> N` and `total <op> N`, `<op>` one of `==`, `!=`, `>=`, `<=`, `<`, `>`;
    - `data.<path> <op> <literal>`, `<op>` one of `==`, `===`, `!=`, `!==`,
      several joined by ` && `; `<path>` is dotted keys, the literal
      `true|false|null|number|'text'` or JSON; a missing path fails;
    - `every item.<path> <op> <literal>` (same operators) over every item of `data`;
    - `every item.<path> non-increasing`: the numbers at `<path>` never rise down
      `data` (`null` sorts last, as `NULLS LAST`);
    - `'<key>' in data`, `'<key>' not in data`, several joined by ` and ` or `, `;
    - `data.<path> keys: a,b,c` (exact key set);
    - `body contains '<text>'` (the raw answer text).
    A trailing ` (note)` is ignored.
- `content_type` compares by prefix; `expected.headers` compare exactly (no
  wildcards: the canonical `link` value is written out in full).
- `text/html` answers are reduced to `{"title", "meta": {name-or-property: content}}`
  before the body match; the `<title>` element may carry attributes (gatsby
  stamps `data-react-helmet="true"` on it).
- A `text/plain` or `text/html` answer that is not JSON (express's redirect page,
  for one) is never parsed: with `subset` a `null` expected body matches and a
  string must occur in the text; with `shape` only the invariants run, and
  `body contains '<text>'` reads the text.
- Every entry's `source.refs` are `file:line` ranges in the pinned repository
  (or `decentraland-gatsby@9.0.1 dist/...`) that fix the expectation. Behaviours
  of the old services that dcl-one must not reproduce (the `stack` field in error
  bodies, the sitemap `page=-1` 500, the create-without-categories 500) are marked
  in `notes` or invariants; such entries stay in `known_gaps.json` on purpose.

## Identities (`request.auth`)

| Value | Headers | Seeded as |
|---|---|---|
| `none` | none | |
| `signed:user` | signed fetch (`x-identity-*`, crypto-middleware 6 payload) | a wallet without permissions |
| `signed:owner` | signed fetch | the creator (`user`) of every seeded event |
| `signed:admin-wallet` | signed fetch | `PLACES_ADMIN_ADDRESSES` and `explore.moderators` |
| `signed:edit-any`, `signed:approve-any`, `signed:approve-own`, `signed:edit-any-schedule`, `signed:edit-any-profile` | signed fetch | `explore.event_profile_settings` rows with that permission granted; for `signed:approve-own` the seeded events belong to this wallet |
| `signed:no-profile` | signed fetch | a wallet without a profile deployment, for the `withAuthProfile()` 404 (`Not found profile for "<user>"`); every other signing wallet has one |
| `bearer:places-admin`, `bearer:data-team`, `bearer:events-admin` | `Authorization: Bearer <configured token>` | `PLACES_ADMIN_AUTH_TOKEN`, `DATA_TEAM_AUTH_TOKEN`, `EVENTS_ADMIN_AUTH_TOKEN` |
| `bearer:wrong` | `Authorization: Bearer wrong` (unless `headers` sets one) | |
| `bearer:unconfigured` | `Authorization: Bearer wrong` against a router with no admin tokens | |

Signed fetches sign `method:path:timestamp:{}` with deterministic development
keys; `harness.signature: "invalid"` signs a different payload so the chain is
well-formed but does not verify. The seed holds a profile deployment
(`content.deployments` + `content.active_pointers`, a `profile` entity whose
pointer is the lowercase address, empty snapshots) for every signing wallet but
`signed:no-profile`, as gatsby's `withAuthProfile()` wants one; the seed deletes
every profile deployment before writing them, so entries stay order-independent.

## Seed and placeholders

Placeholders in `path`, `query`, `body` and `expected` resolve to seeded rows.
`{place_id:disabled}` names a row explicitly; `{place_id}` takes
`harness.place_id` (default first).

| Placeholder | `harness` key | Rows |
|---|---|---|
| `{place_id}` | `place_id` | `enabled` (`6b7c1d2e-…0001`, "Harness Plaza", base "10,10", positions 10,10 and 10,11, categories art/poi, rating T), `disabled` (`…0002`, base 20,20, reason moderation), `world_backed` (`…0003`, world `harbour.dcl.eth`), `highlighted` (`…0004`, base 30,30, ranking 100), `excluded` (`…0005`, base 40,40), `missing` (`…0009`) |
| `{world_id}` | `world_id` | `listed` (`harbour.dcl.eth`), `highlighted` (`lighthouse.dcl.eth`), `excluded` (`quarry.dcl.eth`), `missing` (`nowhere.dcl.eth`) |
| `{event_id}` | `event_id` | `approved` (`9a1e0000-…0001`, 2031-01-01, at 10,10), `pending` (`…0002`), `rejected` (`…0003`, reason "Spam"), `deleted` (`…0004`, by its owner), `past` (`…0005`, 2020), `attended` (`…0006`, the `signed:user` wallet RSVPed), `recurrent` (`…0007`, weekly, four dates, at 30,30), `community` (`…0008`), `missing` (`…0009`); all owned by the owner wallet, created on this node |
| `{schedule_id}` | `schedule_id` | `existing` (`5c3d0000-…0001`, "Harness Season", 2026 to 2036), `missing` (`…0009`) |
| `{address}` | `address` | `subject` (a wallet; `subject_with_row` names the same wallet, which has a settings row), `signer` (the requesting wallet) |
| `{entity_id}` | | an unknown uuid |

Other `harness` keys: `skip` (reason; the entry is documentation only and its
id must be in the harness's `SKIPPED` list, see "Skipped and accepted entries"),
`state: "unconfigured"` (router without admin tokens or a blob store),
`viewer_state` (`favorited`, `liked`, `liked_qualified`, `settings_row`,
`subscribed`: facts seeded for the requesting wallet; the place facts land on
the `harness.place_id` row, and `liked_qualified` is a like with voting power
`MIN_USER_ACTIVITY` written as the PATCH writes it, so the recounted
`like_score` the place answers is on its counter row), `body`
(`ranking_entries:N` generates N ranking entries),
`multipart` (`{"field": "poster", "kind": png|jpeg|gif|oversize|garbage|empty|none|two_files}`).

## Adding an entry

1. Open the route, its schema and the model query at the pinned commit; write the
   expectation from them and list the `file:line` ranges in `source.refs`.
2. Choose `body_match`: `exact` or `subset` for errors and fixed answers, `shape`
   for records and lists (exact key sets).
3. Pick the seeded rows with `harness`, or add a row to the seed in
   `tests/explore_parity.rs` and document it here.
4. Run the suite. A `BAD` line is a defect in the entry; fix it. If the entry
   fails because of a real gap, add it with `EXPLORE_PARITY_WRITE_GAPS=1
   EXPLORE_PARITY_ALLOW_NEW_GAPS=1` (which prints the id and the observed
   difference) and record it under "Known gaps" above with its evidence.
