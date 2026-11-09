# Explore API parity contract with the old places and events services

- Status: Accepted
- Date: 2026-11-05

The explore crate answers `/api` for the Explorer, the old places website and the old
events website. The reference is the pinned source of the services it replaces,
`decentraland/places@e660fbc2` and `decentraland/events@b60d5bbd`, with dated production
captures where the source leaves behaviour to data. Production served exactly those two
commits on 2026-11-07 (`/api/status` of each service names its image). `GET /api/places`
is the exception: its query parsing follows `decentraland/discovery-server@81f0dcae`, the
merged rewrite of the route, which is not deployed; the inputs on which the two parsers
differ (`limit=abc`, `offset=-1`, `order_by=user_visits`, `only_highlighted=1`,
`search=a,b`) are sent by none of the pinned callers below. Parity is strict: a request
shaped as those clients send it gets the answer the pinned source gives, status, envelope,
field set and date format included. dcl-one answers more only where this document records
it. `/v1` is dcl-one's own surface, outside this contract: its gate is the OpenAPI
contract in `library/catalyst-api` and the sites tests.

## The gate

`services/catalyst/tests/explore_parity.rs` replays
`services/catalyst/tests/fixtures/explore_parity/*.fixture.json` against a scratch database.
Every entry cites the pinned source lines it derives from. `known_gaps.json` is the ratchet:
it lists the entries the router does not answer like the pinned source yet and may only
shrink.
Accepted entries record dcl-one's answer where the old service fails and can never become
gaps; a failure there is a regression. The fixtures README documents the grammar and both lists.

## `GET /api/places` follows the discovery server

- A flag (`only_highlighted`, `only_favorites`, `with_realms_detail`) is true only as
  `true`.
- `limit` and `offset` are JavaScript non-negative integers; any other value is the
  default (100 and 0). `limit` is capped at 100.
- `categories` and `names` split on commas and drop blank items. More than 1000 positions,
  categories or names answers 400. `names` turns `positions` off.
- No value has a pattern check: an owner, a creator address or a position that matches no
  row answers an empty list.
- `search` is read only when it is not empty; fewer than three UTF-16 units answers an
  empty list. Each run of letters and digits is a prefix term; the terms are joined with
  `&`.
- `order_by` accepts `like_score`, `updated_at`, `created_at` and `most_active`; any other
  value is `like_score`. `most_active` reads 100 places, those with players first and the
  rest best liked first, sorts them by headcount and pages that set; `total` is its size, at
  most 100. `order=asc` lists the least liked and the emptiest first. An `offset` past the
  places with players answers an empty list, `total` 0. Without a presence snapshot the
  mirrored headcount stands in for the live one.
- `owner` matches the row's own owner, lowercased, the creator address where the row has
  no owner, or a parcel the owner holds or operates. The creator fallback is dcl-one's
  (recorded below). `sdk` matches the major or a `major.` prefix as given.

Known differences: the row keeps the fields the pinned places source adds
(`connected_addresses`, `deployment_id`, `exclude_from_ranking`, `live`, `live_event_name`,
`rank`, `realms_detail`, `textsearch`) and the `with_realms_detail` parameter;
`content_rating`, `created_at`, `deployed_at`, `updated_at` and `user_count` are nullable
where the discovery server's columns are not; the 400 for too many values carries dcl-one's
error envelope (`error` holds the message and `code` is `bad_request`; the discovery
server's `error` holds the class name).

## Recorded differences on production data

Measured on 2026-11-07 against the mirrored production catalogue with the queries the
pinned callers build. Each is a known difference, not an approval.

- `owner`: 93 places have a creator and no owner (23 121 have neither; none has an owner
  that differs from its creator). The node answers them for `?owner=<creator>`
  (`places/query.rs`), serves `owner` as the creator (`places/rows.rs`) and
  `creator_address` lowercased; the old places service answers none, serves `owner` null
  and `creator_address` as stored. `?owner=` with the zero address answers 54 rows here
  and none there.
- `GET /api/destinations` with several `categories`: the totals differ (906 against 847
  for `art` and `music`); each category alone, and `GET /api/places` with both, agree.
- `previous_place_id`: the old events service serves it on `/api` lists; the node omits it
  (`events/handlers.rs`; the fixtures list the key as optional).
- An event the old service deleted stays readable as approved until the mirror's grace
  sweep marks it (`events/mirror.rs`).

## Contract as the pinned source gives it

- An error body is `{ok: false, error, code}`: `error` the message, `code` the
  snake-cased status, and `data` only when the refusal carries detail
  (`decentraland-gatsby@9.0.1 dist/entities/Route/error.js:8-24` on the events routes,
  `dist/entities/Route/wkc/response/ErrorResponse.js:9-26` on the places routes).
  `ApiErrorBody` in `library/catalyst-api` has no `message` and never a `stack`.
- A category row is `{name, count, i18n: {en}}` (`places Category/model.ts:20-29`,
  `Category/routes.ts:29-38`): `CategoryOut` has no `active`.
- The admin bearer's `PATCH /api/events/{id}` body is the old events service's: a body
  whose keys are a subset of `actor`, `approved`, `rejected` and `reason` is a state
  change (`events Event/routes/admin.ts:63`, `:253-257`, `:274-286`), any other body a
  content edit of the editable attributes; `action` and `rejection_reason` are
  unsupported admin event fields (400). The admin bearer's audit actor (`approved_by`,
  `rejected_by`, `deleted_by`) is `admin`; the old events service recorded the name of
  its internal bot.
- POST, PUT and PATCH on the events routes answer 201
  (`decentraland-gatsby@9.0.1 dist/entities/Route/handle/utils.js:4-15`), PUT and POST
  on the places routes too (`dist/entities/Route/wkc/routes/utils.js:13-24`). A router
  layer keyed on method and path turns a handler's 200 into it, so the handlers stay
  free of it; the fixtures pin every route's status.
- `GET /api/events` validates its query against the old events service's list schema
  (`events Event/schemas.ts:23-171`, `additionalProperties: false`), whose `list` is one
  of `all`, `active`, `live`, `upcoming` and `highlight` (`:71-97`): `list=relevance`,
  `recurrent`, `with_recurrent_dates` and `allow_deleted` answer 400 there, and dcl-one
  serves them on `/v1/events` only.

## Accepted answers where the old services error or are inconsistent

- The old events service answers 500 on a duplicate RSVP, on an event created without
  `categories`, on `page=-1` of the events sitemap and on a non-UUID schedule id, and
  leaks stack traces in its error bodies. dcl-one answers an idempotent 201 attendee
  list, a 201 with `categories: []`, a plain error status for the other two, and never a
  stack trace. It also 500s on a fractional `x`, `y` or `duration` (typed `number`, stored
  in INTEGER and BIGINT columns); dcl-one answers the ajv `type: integer` error.
- A schedule `image` must be an http(s) URL: the old `uri` format admits `javascript:` and
  `data:` values, dcl-one applies the event poster's `^https?://` pattern.
- The rating codes `PR` and `RP` are both accepted on rating writes, stored as sent and
  answered verbatim: the old places service stores both while its PUT schema takes only
  `PR`.
- Profile settings never bump `updated_at`: the epoch forever, as the old events service
  does. Place, world and event records serialise dates as node's `Date#toJSON` writes
  them (`2026-01-01T00:00:00.000Z`).
- dcl-one's own extras live under `/v1`: `POST /v1/destinations` takes an object body and
  answers `kind`, `live_event` and `next_event`. `/api/destinations` keeps the old places
  service's contract.
- `GET /api/map` and `GET /api/map/places` with `order_by=most_active` answer the full
  headcount-ordered set with its real `total`, where the old places service pages twice
  and reports the page length as `total` (`getMapPlacesMostActive.ts:60-98`,
  `getAllPlacesMostActiveList.ts:63-115`).

## Deployment posture kept

- `DATA_WRITES_ENABLED` and `OPS_WRITES_ENABLED` answer 503 with `Retry-After` when off.
  Before a mirror's first complete pass the routes answer from the database, a static copy
  that has not landed yet answers 503 with `Retry-After`, and owner edits of a mirrored
  event answer 409 while the node mirrors. The 409 follows the row's `origin`
  (`events/writes.rs`), and cut-over is an explicit opt-in: a node that starts with
  `EVENTS_MIRROR_UPSTREAM` off and `EVENTS_MIRROR_ADOPT` on adopts every row the mirror
  wrote as `local`, once per database (`events/mirror.rs`, `adopt_upstream`, recorded in
  `explore.mirror_state` and run under the mirror job's lease), so their owners edit and
  delete them, their RSVPs and moderation stay, and the read gate is open
  (`tests/explore_mirrors.rs`).
- Browser origins are the old services' per-router allow-lists rather than the reflecting
  CORS the shared crate offers. `EXPLORE_CORS_ORIGINS` appends a deployment's own origins
  to every list.
- Private profile reads and permission grants are moderator-only. The old events service
  also admits a wallet holding `edit_any_profile`; dcl-one stores and lists that grant
  without effect, so no grantee reads emails or passes the grant on (accepted entries
  `events.profiles.*_edit_any_profile`).
- The `:runner:` upcoming-event Slack notice is off by default; `EVENTS_UPCOMING_SUMMARIES_ENABLED=true`
  queues one per reminded occurrence through the outbox. The old events service posted it
  whenever a webhook was configured.
- `POST /api/events` without a blob store answers 503 in dcl-one's words
  (`known_gaps.json`, `events.create.image_bucket_unconfigured`): the old events service
  named its bucket variable, dcl-one has no bucket.

## Moderation outbox

Slack-compatible notices of the events routes (`SLACK_WEBHOOK`) and of the places routes
(`PLACES_MODERATION_WEBHOOK`) are captured in the writing transaction as rows of one
table, `explore.moderation_hooks`, told apart by a `channel` column, and posted by one
leased job with bounded retries and retention. The job delivers each notice at least
once. A crash after the Slack request and before the row completes sends the notice
again, and Slack has no idempotency key that drops the copy. Places notices go to the one
`PLACES_MODERATION_WEBHOOK`; the old places service posted to two channels. While the database has the
table `explore.event_moderation_hooks`, every pass of the job first moves its pending
rows into the outbox; the release `schemas/releases/explore-retired-hook-tables.sql`
drops that table and the unused `explore.event_upcoming_hooks` once every replica
runs this binary.

## Like rates

The mirror brings the old places service's aggregates, not its voters. A local vote is a
fact with the wallet's voting power at vote time, and the served rate combines that
service's qualified votes reconstructed from the mirrored aggregates with this node's
qualified votes, by the same Wilson bound. The old places service's own `user_likes` rows,
imported once from its database export with `catalyst explore import-likes`, are the
ground truth: an entity with imported rows takes its rates from the fact table alone, and
a wallet's later local vote replaces its imported one, so nobody counts twice.
[Places](../../services/catalyst/docs/places.md) describes the import.

## Supersets kept

dcl-one answers more than the pinned source in these places; each stays as a recorded
exception:

- world ids are matched case-insensitively on lookups;
- `order_by=most_active` on `GET /api/worlds`;
- `pointer` without `names` narrows destinations to places;
- `GET /api/status` adds `catalogue`, `configured_mirrors` and `mirrors` to the three
  upstream keys (the two status entries are accepted entries of the gate);
- `PATCH /api/places/{id}/disable` with a free-text reason;
- a custom `url` any signer sets on create and owner edits keep;
- poster filenames by content hash, with the asset-URL guard and the profile requirement;
- the old places service's double HTML escaping is reproduced in social cards.

## Callers

The supported caller contract resolves `/api/places`, `/api/worlds`, `/api/destinations`,
`/api/map` and `/api/events` on the places and events hosts, or `/places/api/…` and
`/events/api/…` through the gateway; it does not use `/v1`.
Explorer requests use integers, `true` flags, repeated `categories`, `positions` and `names`,
`order_by` with `order`, `owner`, `only_sdk7`, `with_live_events`, `with_connected_users`,
and on events `list=live` or `highlight`, `positions[]`, `places_ids[]`, `community_id`,
`from` and `to`, and `POST /api/events/search`. The sites send `only_highlighted=true`,
`order_by=most_active`, `search` of three or more characters, `owner`,
`with_realms_detail` and repeated `categories`. Twenty-three of their queries replayed
against production and this node on 2026-11-07 agree, apart from the recorded differences.
