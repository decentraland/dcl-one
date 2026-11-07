# Explore API parity contract with the old places and events services

- Status: Accepted
- Date: 2026-11-05

The explore crate answers `/api` for the Explorer, the old places website and the old
events website. The reference is the pinned source of the services it replaces,
`decentraland/places@e660fbc2` and `decentraland/events@b60d5bbd`, with dated production
captures where the source leaves behaviour to data. `GET /api/places` is the exception:
its reference is `decentraland/discovery-server@81f0dcae`, the service that serves the
route in production. Parity is strict: a request shaped as those clients send it gets the
answer the pinned source gives, status, envelope, field set and date format included.
dcl-one answers more only where this document records it.

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
  value is `like_score`. `most_active` reads the places with players, at most 100, best
  liked first, sorts them by headcount and pages that set; `total` is the size of the set.
  Without a presence snapshot the mirrored headcount stands in for the live one.
- `owner` matches the row's own owner, lowercased, or a parcel the owner holds or
  operates. `sdk` matches the major or a `major.` prefix as given.

Known differences: the row keeps the fields the pinned places source adds
(`connected_addresses`, `deployment_id`, `exclude_from_ranking`, `live`, `live_event_name`,
`rank`, `realms_detail`, `textsearch`) and the `with_realms_detail` parameter;
`content_rating`, `created_at`, `deployed_at`, `updated_at` and `user_count` are nullable
where the discovery server's columns are not; the 400 for too many values carries dcl-one's
error envelope (`error` holds the message and `code` is `bad_request`; the discovery
server's `error` holds the class name).

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
  unsupported admin event fields (400).
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

## Deployment posture kept

- `DATA_WRITES_ENABLED` and `OPS_WRITES_ENABLED` answer 503 with `Retry-After` when off.
  Before a mirror's first complete pass the routes answer from the database, a static copy
  that has not landed yet answers 503 with `Retry-After`, and owner edits of a mirrored
  event answer 409 while the node mirrors.
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

## Moderation outbox

Slack-compatible notices of the events routes (`SLACK_WEBHOOK`) and of the places routes
(`PLACES_MODERATION_WEBHOOK`) are captured in the writing transaction as rows of one
table, `explore.moderation_hooks`, told apart by a `channel` column, and posted by one
leased job with bounded retries and retention. The job delivers each notice at least
once. A crash after the Slack request and before the row completes sends the notice
again, and Slack has no idempotency key that drops the copy. While the database has the
table `explore.event_moderation_hooks`, every pass of the job first moves its pending
rows into the outbox.

## Like rates

The mirror brings the old places service's aggregates, not its voters. A local vote is a
fact with the wallet's voting power at vote time, and the served rate combines that
service's qualified votes reconstructed from the mirrored aggregates with this node's
qualified votes, by the same Wilson bound. The old places service's own `user_likes` rows,
imported once from its database export with `catalyst explore import-likes`, are the
ground truth: an entity with imported rows takes its rates from the fact table alone, and
a wallet's later local vote replaces its imported one, so nobody counts twice.
[Places](../../services/catalyst/docs/places.md) describes the import.

## Open

- Which supersets stay as explicit exceptions: case-insensitive world ids on lookups,
  `order_by=most_active` on `GET /api/worlds`,
  `pointer` without `names` narrowing destinations to places, the `/api/status` extras, the `PATCH /api/places/{id}/disable` route with a
  free-text reason, a custom `url` kept on owner edits,
  poster filenames by content hash with the asset-URL guard and the profile requirement,
  and the old places service's double HTML escaping reproduced in social cards. Until
  decided, the `/api/status` extras are the two status entries in `known_gaps.json`.
- The create-event 503 wording when no blob store is configured: the old events service
  names its bucket variable, dcl-one has no bucket.
- Whether places moderation posts to two Slack channels as the old places service does.
- The admin bearer's default audit actor (`approved_by`, `rejected_by`, `deleted_by`) is
  `admin`; the old events service recorded the name of its internal bot.
- The 409 on owner edits of mirrored events and the read gate after a cut-over from
  mirroring.
- The old websites' front-end code is not in the pinned repositories, so the caller
  dimension of parity stays unproven until their repositories and commits are pinned.
