# Event clients and deployment

## Public URLs and image paths

Set `PUBLIC_URL` to the externally reachable node base URL, including its port and any deployment prefix. Stored poster keys use the `/events-assets-099ac00` mount on the configured public base. The bind address is used for local serving. An unset `PUBLIC_URL` leaves mounted poster links relative for same-origin clients. `EVENTS_SITE_URL` selects the Sites origin for notification page links (`/events`, `/events?tab=my` and `/events/edit-event/<id>`), defaulting to `https://decentraland.org`. `EVENTS_BASE_URL` retains the legacy event website origin for sitemaps, world-event default posters and Slack links, defaulting to `https://events.decentraland.org`. Neither selects the backend asset origin. Reminder, start and approval jump links keep `JUMP_IN_SITE_URL`.

An event created without a poster gets the old service's default (`createEvent.ts:209-224`): the parcel map `https://api.decentraland.org/v1/parcels/<x>/<y>/map.png` (the estate map `/v1/estates/<id>/map.png` once the scene's estate is known) for a land event, `EVENTS_BASE_URL/images/event-default.jpg` for a world event. The default is regenerated when the event moves and when a world event's poster is cleared; an organizer's own poster is never replaced. A row whose image is the local default `/assets/event-default.webp` expands it to `PUBLIC_URL/events/assets/event-default.webp` in responses and notifications.

Uploaded posters and copied event artwork use `/events-assets-099ac00/poster/<file>` or `/events-assets-099ac00/poster-vertical/<file>`. `POST /api/poster` and `POST /api/poster-vertical` return the existing `{ok,data:{filename,url,size,type}}` envelope. Upload responses and event reads of stored local poster keys use absolute URLs under `PUBLIC_URL`, including on Decentraland domains, IP addresses and localhost. Without `PUBLIC_URL` they use root-relative links for clients on the node's origin. Clients can submit either form as returned. Stored upstream poster URLs use the node mount only when the public domain qualifies for the upstream rewrite. Upload filenames use the content's SHA-256. Create stores indexed poster keys as router-local paths, including copies from the configured bucket. A changed poster on edit requires a local upload. An unchanged edit of a projected poster or jump link retains its stored value. Third-party poster URLs require a supported source or a local upload.

A new poster read-through validates the file signature and records the detected media type. Existing cache entries keep their indexed content type, including generic media types.

The upload needs a signed fetch by a wallet whose profile this node holds or reads through (the old `withAuthProfile()`: 404 `Not found profile for "<user>"` otherwise), `DATA_WRITES_ENABLED`, and a blob store (503 `Service unavailable` without one). The refusals keep the old texts: 400 `Poster param is required`, `Multiple files are not allowed`, `Empty files are not allowed`, `Invalid file content; expected one of <types>`; 413 `File size limit has been reached` (horizontal) and `File size limit has been reached (max 500 KB)` (vertical). The size limit is checked before the content type, as the old upload middleware did.

## Schedules, profile settings, subscriptions and avatars

These routes answer the old events service (`decentraland/events@b60d5bbd`) for its website and other signed-fetch clients. "isAdmin" is a wallet in `explore.moderators`; the `edit_any_schedule` and `edit_any_profile` permissions count once granted (`permissions_granted`): imported, or set through `PATCH /api/profiles/{id}/settings`. A plain signed-fetch body needs no `x-identity-metadata`; when the client signs `body_sha256` the body must match it (400), and when it signs a `nonce` a replay answers 409. Validation failures answer the gatsby validator's text, `Invalid data was sent to the server`. A profile this node lacks is read through `SYNC_SOURCE` when `READ_THROUGH_ENABLED` is on, as the old service read the catalyst's `/lambdas/profiles`; a profile found nowhere or denylisted, a source that fails, every fill slot busy, a fill past its 8 s wait, a pointer whose fill failed in the last 60 s, or `SYNC_ENTITY_TYPES` without `profile`, is a miss (404 `Not found profile for "<user>"`, or the default avatar).

- `GET /api/schedules` lists the schedules whose `active_until` is still ahead; `GET /api/schedules/{id}` and `/events/sitemap.schedules.xml` read every row, expired ones included.
- `POST /api/schedules` requires `name`, `active_since` and `active_until`, validates `image` as an http(s) URL or null (the `uri` format plus the event poster's `^https?://` pattern) and `theme` against `null|mvmf_2022|mvfw_2023|pride_2023`, ignores unknown keys, stores `theme: null` and answers the whole record. The body is validated before the permission check (403 `Forbidden`). A signed `ScheduleUpsert` envelope passes the same `image` and `theme` checks after the permission check.
- `PATCH /api/schedules/{id}` merges the stored row with the body's keys and answers only the eight schema keys (`name`, `description`, `image`, `theme`, `background`, `active`, `active_since`, `active_until`); an absent schedule is 404 `Schedule "<id>" not found`. The `ScheduleUpsert` federation envelope keeps its full-record answer.
- `GET /api/profiles/settings` lists the rows holding permissions, emails included, for a moderator or an `edit_any_profile` grantee (`listProfileSettings.ts:11`); anyone else is 403. The same two read and write other profiles: `GET` and `PATCH /api/profiles/{id}/settings` (`getProfileSettings.ts:14`). A stored `edit_any_profile` without the grant counts for nothing. `GET /api/profiles/{id}/settings` answers the stored row or `data: null`; a non-address is 404 `Not found "<id>" profile: invalid address`. Settings carry `email_verified_at` and `email_updated_at`; the signer's own `GET /api/profiles/me/settings` adds its `subscriptions` endpoints.
- `PATCH /api/profiles/me/settings` takes `email` (RFC-style format), `email_verified`, `use_local_time`, `notify_by_email`, `notify_by_browser` and nothing else. `email_verified: true` is ignored; a changed email is stored unverified with `notify_by_email` off and `email_updated_at` set; `email_verified: false` on the same email un-verifies it the same way.
- `PATCH /api/profiles/{id}/settings` takes `permissions` only, any of the five, so a grantee may grant or withdraw `edit_any_profile`, its own included (`updateProfileSettings.ts:19-50`); a changed set queues the Slack moderation notice through the same outbox and switch as the event notices (`OPS_WRITES_ENABLED` and `SLACK_WEBHOOK`). The `ProfileSettingsUpdate` envelope, which may also set the target's own fields, stays a moderator's.
- `GET`/`POST`/`DELETE /api/profiles/subscriptions` keep the deprecated web-push rows (`explore.event_profile_subscriptions`, body `{endpoint,p256dh,auth}`) and the owner's `notify_by_browser` flag in step; both mutations answer `true`. Nothing sends pushes.
- `GET /api/profile/{user}/face.png` and `/body.png` answer 302 with `Cache-Control: max-age=86400` to the avatar snapshot of the profile this node holds or reads through (a stored URL as is, a content hash as this node's `profile-images/entities/<id>/{face,body}.png`), else to `https://decentraland.org/images/male.png`.

## Events: lists, reads, create, edit, delete and RSVPs

These routes answer the old events service (`decentraland/events@b60d5bbd`) for the Explorer and the old events website. Statuses, messages and record shapes follow its sources; every rule below names its source file.

### Stored read columns

`explore.event` stores the reads of the raw document that every list filters by: `rejected`, `deleted`, `world`, `place_id_lower`, `server_lower`, `created_at_ts`, the `recurrence_dates` array (each valid `recurrent_dates` entry as a UTC timestamp, ascending), `live_until` (the last instant the effective occurrence is live; the hash-named `event_live_until` function, one pass over the entries) and the `textsearch` vector. The Active, Live and Upcoming lists and `/api/events/attending` read `live_until > now` from `event_live_until_idx`; the destination, creator and sitemap predicates and the search read their own indexes. The columns are generated, so the mirror and the local writers do not write them. A list sorts, counts and cuts its keys in a `page` CTE and reads the columns of the page alone. A mirrored `duration` is stored bounded to a thousand years, so a value the old service never checked cannot overflow `live_until`; the document keeps its value.

### Who sees what

- Anonymous and plain signed readers list approved, non-rejected, non-deleted events and read any approved event by id, soft-deleted ones included (`getEvent.ts:61-79` has no deletion check; lists hide deleted rows). A signed reader also lists their own pending events and reads their own pending and rejected ones by id; their rejected events appear in a list only under `owner=true` (`Event/model.ts:183-190,238-241`).
- A moderator (`explore.moderators`, gatsby's `isAdmin`) or a wallet granted `edit_any_event` or `approve_any_event` gets `allow_pending` (`getEventList.ts:166-171`): pending events of every creator in lists, pending and rejected ones by id; rejected rows stay out of their lists and the `approved`/`rejected`/`deleted` query filters are not honoured for them.
- The admin bearer (`EVENTS_ADMIN_AUTH_TOKEN`; a signed chain wins over it, a non-`Bearer` header is anonymous, a `Bearer` while the token is unset is 503, a wrong one 401) lists pending and rejected rows by default, honours `approved`, `rejected` and `deleted` (`?deleted=true` lists the soft-deleted rows), and its reads by id and its PATCH/DELETE answers carry `contact` and `details` (`adminServiceProfile`, `admin.ts:37-42`) with `attending: false`; its lists carry neither. It is not honoured on `POST /api/events`, `GET /api/events/attending` or the attendee routes (401).
- `GET /api/events/attending` answers every event that has not finished, is not rejected and not deleted, pending events of other creators included, each with the signer's `attending` flag (`Event/model.ts:461-480`).
- Records carry the old service's keys: `contact`/`details` only in the owner's and the bearer's views, `connected_addresses` on every item when `with_connected_users` is set (an empty array when the room is unknown), no `previous_place_id` on `/api/*` (the `/v1/*` routes keep it). `recurrent_dates` lists the next occurrences that have not ended, at most ten (`MAX_EVENT_RECURRENT`); `/v1/events?with_recurrent_dates=true` answers the stored window. `recurrent_interval` defaults to 1.
- `user_name` answers `Decentraland Foundation` for the wallets the feature flag `events-decentraland-foundation-addresses` lists in its variant payload (`modules/decentralandFoundationAddresses.ts`). The list is read every five minutes from the `events` feature-flags document in `explore_api.feature_flags`, which the explorer API's feature-flags mirror fetches beside the explorer document (`events.json` next to the configured `EXPLORER_FEATURE_FLAGS_SOURCE_URL`), and kept when the row is absent.
- Text search weighs name `A`, creator name `B`, estate name `B`, description `C` (`Event/model.ts:35-48`).

### Create (`POST /api/events`, `createEvent.ts`)

A signed fetch by a wallet whose profile this node holds or reads through (`withAuthProfile()`: 404 `Not found profile for "<user>"`), behind `DATA_WRITES_ENABLED`. An empty body is 400 `Empty event data`. A falsy `image`, `image_vertical`, `server` or `featured_item` is null; a falsy `url` is the generated jump-in link `https://decentraland.org/jump/events?position=<x>%2C<y>[&realm=<server>]`. The defaulted body is validated against `newEventSchema` (400 `Invalid data was sent to the server` with `data.errors`, `data.messages` and the defaulted body; unknown fields refused), then in the old service's order: a custom `url` must be a public http(s) link (`Invalid event url <url>; only public HTTP(S) URLs are allowed`), a poster must be one this node serves (`Invalid image url <url>, please upload the image through the upload poster endpoint (POST /poster)`; 503 without a blob store), the coordinates must be inside the world limits for every event, world events included (`Event is outside the world limits`), a recurrence whose expansion from `start_at` to now exceeds 50 000 steps is refused, the end must be ahead (`The event end date is already in the past`, `The recurrence end date (Until) must be in the future`), the duration at most 24 hours (`Maximum allowed duration 24Hrs`), the categories known (`Invalid category tag supplied`) and a `community_id` one the signer owns or moderates in `social.communities` (`Failed to validate community ownership: Community "<id>" not found or you don't have access to it`). Any signer may set `url`; `schedules` are always stored as `[]`; a missing `categories` is `[]` (the old service answered 500 there, not reproduced). `estate_id`, `estate_name`, `scene_name` and `place_id` come from this node's `explore.place_indexed` (the scene's title stands for the Land tile name the old service read); `user_name` from the signer's profile. The event is pending approval; the answer is the owner's view.

`count` and `until` may both bound a recurrence; the stored series holds at most 1 000 occurrences from one day before the present on (the reads project only unfinished occurrences; the extra day lets the community metrics job record an occurrence that ended since its last pass), a local series bounded by `until` (not by `count`) is expanded again from the present by the hourly `explore-event-recurrence` job when its stored window holds 1 000 occurrences and ends within 7 days and before `until` (a shorter window is the whole series and is not read again); the job reads each row again under its lock, so a moderation write is never written over. The reads project the old service's ten. `x`, `y` and `duration` must be whole numbers: a fraction answers the ajv `type: integer` error, and a whole float such as `11.0` is read as `11`.

### Edit (`PATCH /api/events/{id}`)

A signed wallet (`updateEvent.ts:159-595`): the event is read as `GET /api/events/{id}` does (404 for a pending or rejected event of another creator), then 403 `You don't have permission to edit this event` unless the signer is the owner, a moderator or an `edit_any_event` grant. The body is merged through the old service's attribute sets (unknown fields are ignored): the owner's `image`, `image_vertical`, `rejected`, `name`, `description`, `start_at`, `duration`, `all_day`, `x`, `y`, `server`, the `recurrent*` fields, `categories`, `world`, `featured_item`, `contact`, `details` and `community_id` (plus `approved` with `approve_own_event`); an `edit_any_event` grant's `highlighted`, `trending`, `schedules` and `url`; an `approve_any_event` grant's `approved` and `rejection_reason`. The merged event is validated whole against `newEventSchema`; then the url (when sent), the recurrence cap (when a recurrence field is sent), the posters (when changed), the schedules (deduplicated; `Schedule not found: <ids>`), the duration (at most the stored one or 24 hours), the world limits, the categories, the location's place and the recurrence; a date change must leave the end ahead. An empty or `https://decentraland.org/jump`-prefixed url is regenerated from the location; a custom url is kept. A rejection clears `approved`, `highlighted` and `trending`; a changed moderated field (`name`, `description`, the images, `x`, `y`, `server`, `categories`, `world`, `featured_item`) sends an approved event back to pending with `highlighted` and `trending` cleared unless the editor may approve; `approved_by`/`rejected_by` record the actor of a transition. An owner's merged `community_id` (the stored one when the body does not send it) must be a community the signer still manages (`...not found or you don't have permission to access it`; `...to detach this event from it` when the body sets it null). Another wallet's edit is an operator write (`OPS_WRITES_ENABLED`). A plain signed fetch needs no `x-identity-metadata`; a signed `body_sha256` must match the body (400) and a signed `nonce` refuses its replay (409 `Event request already applied`). An owner's `rejected: false` sends a rejected event back to pending whoever rejected it, leaving `rejected_by` and `rejection_reason` as stored. dcl-one keeps one rule the old service lacks: a mirrored event answers 409 naming the node it mirrors.

The admin bearer (`admin.ts:253-387`), behind `OPS_WRITES_ENABLED`: the event first (400 for a non-UUID, 404 when missing; a soft-deleted event is still patched). A body whose keys are a subset of `actor`, `approved`, `rejected` and `reason` is a state change: `approved`/`rejected` must be booleans and never both (`approved and rejected cannot be set in the same request`); `approved: true` approves (clearing a rejection; an approved event is answered unchanged), `approved: false` unapproves, `rejected: true` rejects with the required `reason` (`rejection reason is required`, `...cannot be empty`, `...must be 500 characters or less`, `...must be a string`) clearing `highlighted` and `trending` (the same reason again is answered unchanged), `rejected: false` unrejects; `actor` (default `admin`, at most 42 characters) is the recorded `approved_by`/`rejected_by`. Any other body is a content edit with the editable attributes above (minus `rejected`) and `actor`; another key is 400 `Unsupported admin event fields: <keys>`, an empty body `At least one admin event field is required`, `actor` alone `approved or rejected field is required`. That is the whole HTTP vocabulary: `action` and `rejection_reason` are unsupported admin event fields. The console's `events/moderate` command has its own body (`moderation::AdminPatch`), which takes `action` and `reason`, spelled `rejection_reason` too.

### Delete (`DELETE /api/events/{id}`)

A signed owner soft-deletes their event (`deleted_by_user`, `deleted_by` the wallet; the body is ignored), an `edit_any_event` grant or moderator another's (`deleted_by_admin`, `deleted_by` the wallet, the body's `reason` trimmed, blank as null, at most 500 characters: `deleted_reason must be a string`, `deleted_reason must be 500 characters or less`); otherwise 403 `You don't have permission to delete this event` (`deleteEvent.ts:22-92`). The admin bearer deletes as its `actor` (default `admin`) with the same reason rule (`admin.ts:230-251,359-365`). An already deleted event is answered unchanged. A mirrored event is hidden by an operator's delete and refuses its owner's (409).

### Attendees

`GET /api/events/{id}/attendees` reads the event as `GET /api/events/{id}` does for the optional signer (400 for a non-UUID, 404 when hidden; a `Bearer` header is anonymous here) and answers the newest 500 RSVPs. `POST` needs a signed fetch by a wallet whose profile this node holds or reads through (404 `Not found profile for "<user>"`), whose name is the row's `user_name`; `DELETE` a signed fetch; both read the event the same way and answer the list. A repeated RSVP is answered idempotently (the old service's duplicate insert error is not reproduced). `total_attendees` and `latest_attendees` (the ten newest) are derived at read time. An RSVP made at the old service is known here only while it is among the event's `latest_attendees` or was made here; `attending`, `only_attendee` and the attendee list see no other.

## Mirror and cut-over

With `EVENTS_MIRROR_UPSTREAM=true` the `events-mirror` job copies the old events service's catalogue from `EVENTS_UPSTREAM_URL` every `EVENTS_MIRROR_INTERVAL_SECS`; rows it writes carry `origin = 'upstream'`, and an owner's edit or delete of such a row answers 409 naming the upstream (the admin's moderation, the admin's delete and RSVPs are local rows on it). The cut-over is an explicit opt-in once the old service stops writing: a start with `EVENTS_MIRROR_UPSTREAM=false` and `EVENTS_MIRROR_ADOPT=true` adopts every row the mirror wrote as `origin = 'local'` (`events/mirror.rs`, `adopt_upstream`), once per database: `explore.mirror_state.adopted_at` records it, so a later start changes nothing with or without the flag, and it runs as the `events-mirror` job under that job's lease, so a replica still mirroring finishes first and the rows it wrote are the ones adopted. After it their owners edit and delete them and their RSVPs and moderation rows stay (`docs/adr/2026-11-05-explore-api-parity-contract.md`, "Deployment posture kept"). The adoption is one-way: a mirror turned on again skips the adopted rows as local and reports their ids as collisions.

## Background jobs and notifications

The old events service ran four cron jobs (`Event/cron.ts`) and posted Slack notices inline (`Slack/utils.ts`). This node runs them as leased periodic jobs over the same database; the texts, links and audiences are the old service's, the delivery is durable.

- **Reminders** (`notifyUpcomingEvents`, `cron.ts:51-83`): the social `event-notifications` job (every 60 s) queues one `events_starts_soon` intent per approved occurrence starting within the next hour that has a local RSVP (`going`), and delivers it to those attendees as "Event starts in an hour" / "The event <name> starts in an hour." with the link `JUMP_IN_SITE_URL/events?id=<id>&position=<x>,<y>[&realm=<server>]` (`Notifications/index.ts:30-68`). The old service's window was `[last run + 1 h, now + 1 h)` over a cursor; here the occurrence key is the checkpoint, so occurrence scanning is durable across retries. Difference kept: an event approved with less than an hour to go is still reminded once (the old service's cursor skipped it).
- **Started** (`notifyStartedEvents`, `cron.ts:85-148`): `events_started` to the attendees ("Event started" / "The event <name> has begun!") and, for a local event of a local community, to its members ("Community Event starting" / "A <community> event is about to start."), deduplicated; same link. A mirrored event notifies its local RSVPs only.
- **Ended** (`notifyEndedEvents`, `cron.ts:150-192`): the old service published an `EVENT_ENDED` message with `totalAttendees` and the `communityId`; its consumer kept the community event metrics. Here the durable `social.event_ended_occurrences` row is that message (event, occurrence, attendees = current RSVPs, community) and `social.community_event_metrics` is updated in the same commit; each ending is also logged (`event ended`, `total_attendees`). Local events only.
- **Foundation names** (`refreshFoundationAddresses`, `modules/decentralandFoundationAddresses.ts`): every five minutes the events reads refresh the `events-decentraland-foundation-addresses` flag payload from the mirrored feature flags and answer "Decentraland Foundation" as `user_name` for those wallets.
- **Recurrence** (`updateNextStartAt`): occurrences are materialised at write time and `next_start_at`/`live` are computed per read; the hourly `explore-event-recurrence` job expands a local `until`-bounded series again from the present when its stored window holds 1 000 occurrences and ends within 7 days (500 rows per pass; rules over the past-iterations cap, `count` rules and mirrored rows are left alone). A re-anchored series keeps the ended occurrences of its last 24 hours (`ENDED_OCCURRENCE_GRACE_HOURS`, `recurrence.rs`): reads project only the unfinished ones, and the ended-occurrence pass of the `event-notifications` job (every 60 s) still finds an occurrence that ended since its last pass. The grace assumes that job does not stall for a day; an occurrence that ended earlier than the grace before the next pass is not counted in the community metrics.

Slack moderation notices (`SLACK_WEBHOOK`, only with `OPS_WRITES_ENABLED`) are captured in the authoring transaction as rows of the moderation outbox `explore.moderation_hooks` (channel `events`; the places moderation notices of `PLACES_MODERATION_WEBHOOK` share the table on channel `places`) and posted by the `moderation-hook` job (every 60 s, at least once, six attempts with backoff, 30-day retention). While the database has the table `explore.event_moderation_hooks`, every pass of the job first moves its due pending rows into the outbox (`adopt_legacy_rows`, one `to_regclass` check per pass; a row an older worker claimed waits for its retry minute, so a notice in flight is not posted twice). The release `schemas/releases/explore-retired-hook-tables.sql` moves the pending rows the same way, then drops that table, its activation row and the unused `explore.event_upcoming_hooks`; run it once with `psql` after every replica runs this binary and `catalyst schema migrate` recorded step `0029_explore_parity`. The messages are the old service's: `:tada: New event submitted` with the review link `EVENTS_BASE_URL/whats-on/admin/pending-events?id=<id>`, the poster and the place line (`Slack/utils.ts:33-81`); `:white_check_mark: new event approved`, `:x: new event rejected` (with the reason) and `:wastebasket: event deleted by admin` with their actor and the self-moderation suffix (`:83-188`); `:pencil2: user <name> just edited his event`, at most once per minute per event (`:190-220`); `:runner: <id> is about to start` with the reminder count, queued by the reminder job only with `EVENTS_UPCOMING_SUMMARIES_ENABLED` (`:222-257`, `cron.ts:76`; below); `:magic_wand: user <wallet> has new permissions` with the permission names (`:322-358`). The edit and upcoming links are `EVENTS_BASE_URL/whats-on/edit-hangout/<id>?openPreview` (the old service hard-coded `https://decentraland.org` there). User content is Slack-escaped in `mrkdwn` blocks. The old service's `notifyEventError` had no caller and is not ported. A dcl-one extra: edits are also capped at two notices per minute per creator.

Reminder, start, community-created, approval, rejection and deletion notices each keep one inbox row per event, type and recipient. A newer occurrence or moderation transition refreshes that row; an older source timestamp cannot overwrite it, and equal source times keep the first payload. Email is queued only on the first insertion, including for approval and rejection. Intent cleanup and email-delivery pruning do not permit another email. Native history keeps its existing ID when adopted; existing historical duplicates are not consolidated.

Event rows honor in-app preferences at their first insertion. A hidden row is read and stays hidden after later moderation or occurrences, even if the recipient turns in-app back on; a visible row stays visible after in-app is turned off. Email preferences remain independent. Imported RSVPs use the same attendance table and reminder eligibility as locally recorded RSVPs. The import reconciles upstream cancellations only for rows whose imported values remain unchanged; later local cancellations and re-RSVPs take precedence. See the Events import mapping for the historical cutover requirements.

Every newly stored event notice also enters `social.notification_deliveries` for the `email` channel, governed by the recipient's social subscription (`ignore_all_email`, per-type settings, community opt-outs), not by the legacy `notify_by_email` profile flag; the old service's own email and web-push code was dead. See `docs/adr/2026-11-01-event-notification-delivery.md`.

## Subscribers of the legacy events topic

The legacy events service publishes its notices to the platform's shared SNS bus (the `event-driven-sns` stack, read from `AWS_SNS_ARN`), not to a topic of its own: `type` `event` with subtype `event-created`, `event-approved`, `event-rejected`, `event-deleted`, `event-starts-soon`, `event-started` or `event-ended`. This node publishes nothing to that bus, so once the legacy service stops, every subscriber whose filter admits `type = event` stops receiving these messages.

| Subscriber | Receives | Uses them for | On this node |
| --- | --- | --- | --- |
| notifications-workers (`notifications-processor` queue) | every `event` subtype | inbox notices | Reproduced: the social `event-notifications` job and the notification intents (above). |
| engagement-workers | the `notifications-processor` and `badges-processor` queues (it owns no subscription) | the same inbox notices | Reproduced, as for notifications-workers. |
| social-service-ea | `event-ended` | community event metrics | Reproduced: `social.event_ended_occurrences` and `social.community_event_metrics`. |
| badges processor (`badges-processor` queue) | every `event` subtype (its filter excludes only `badge`, `rewards`, `asset-bundle` and `credits-service`) | nothing: no observer handles `type = event` | Drop: no feed needed. |

The other subscriptions to the bus filter `type = event` out: asset-bundle-registry and its abgen instance, the abgen converter lambda, bot-detection-server, credits-server, discovery-server, lods-generator, multiplayer-server, places, police-server, profile-images-worker, world-storage-service, worlds-content-server and wearable-validator. The asset-bundle converters and lod-generator-unity declare filtered queues but no subscription to the bus. discovery-server is also wired to publish `event-starts-soon`, `event-started` and `event-ended` to the bus; stopping the legacy service does not stop those.

Of the subscriptions declared in infrastructure code, none outside the reproduced consumers needs a feed. That inventory comes from the `definitions` repository at `268fa33` (`src/services/*.ts`, `src/jobs/wearable-validator.ts`) and the `ops-lambdas` repository at `6de6eda` (`abgen.ts`); a search of the other Decentraland service and infrastructure repositories found no further subscription to the bus. The event types are those the legacy service publishes at `decentraland/events@2436e598` (`src/entities/Notifications/index.ts`). Subscriptions created outside infrastructure code, including any the `event-driven-sns` stack declares itself, are not covered: before the legacy service stops, list the bus's live subscriptions (`aws sns list-subscriptions-by-topic`) and check any not named above.

## Conditional repair for an early staging default

No automatic historical rewrite is performed. This repair applies only to a deployment whose rows store the website's absolute default. Check the affected deployment and take a database backup before any authorized maintenance.

First inspect up to 100 matching local, generated images:

```sql
SELECT id
FROM explore.event
WHERE origin = 'local'
  AND raw->>'_image_source' = 'generated'
  AND raw->>'image' = 'https://events.decentraland.org/assets/event-default.webp'
ORDER BY id
LIMIT 100;
```

If those rows are confirmed to be the generated defaults from that build, preview a bounded repair transaction. Its default `ROLLBACK` leaves the database unchanged. Only an authorized operator replaces it with `COMMIT` after checking the returned IDs; repeat batches as needed.

```sql
BEGIN;
SET LOCAL lock_timeout = '2s';
SET LOCAL statement_timeout = '5s';
WITH affected AS (
  SELECT id
  FROM explore.event
  WHERE origin = 'local'
    AND raw->>'_image_source' = 'generated'
    AND raw->>'image' = 'https://events.decentraland.org/assets/event-default.webp'
  ORDER BY id
  LIMIT 100
  FOR UPDATE SKIP LOCKED
)
UPDATE explore.event AS event
SET raw = jsonb_set(event.raw, '{image}', '"/assets/event-default.webp"'::jsonb)
FROM affected
WHERE event.id = affected.id
RETURNING event.id;
ROLLBACK;
```

The exact URL and generated-provenance guards leave explicit organizer images, mirrored events and other image URLs untouched. This repairs representation only; it does not change event lifecycle timestamps, moderation or notification state.

## Optional upcoming-event Slack notices

`EVENTS_UPCOMING_SUMMARIES_ENABLED=true` turns on the `:runner: <id> is about to start`
notice of the moderation outbox above. It defaults to false and needs
`OPS_WRITES_ENABLED=true` and a non-blank `SLACK_WEBHOOK` (or `SLACK_WEBHOOK_FILE`);
the moderation alerts do not depend on it. The reminder job (`event-notifications`,
above) is the only source of that notice: when an occurrence's reminders complete it
queues one notice for the occurrence, with the count of its local `going` RSVPs, in the
transaction that finishes the reminders, so an occurrence is announced at most once and
a crash repeats neither. Delivery, retries and retention are the outbox's
(`moderation-hook` job). Off, the reminders still go out and no Slack notice is queued.

## Activation with imported event history

Keep event jobs stopped on every target process until imported notification
identity has been checked and repaired. Keep target `catalyst serve` processes
stopped, or configure every target process with `FLEET_JOBS_ENABLED=false` during
the cutover; this stands down all fleet jobs, not only event notifications.
Disabling Social writes alone does not establish this prerequisite.

The deployment owner must confirm whether the database previously imported
notification history or attendance without event identity or import provenance.
For notification history, freeze source writers and run a full
`run notifications-workers --step notification --restart` import before enabling
jobs. An incremental import cannot repair older rows outside its watermark.
Verify that affected event rows carry their exact event identity. Complete the
Events attendance import and reconciliation against the authoritative source,
retaining later local changes.

If required source records no longer exist, an earlier authoritative export is
needed to identify the exact historical rows. The current database alone cannot
prove that coverage. Keep jobs stopped until that evidence is reconciled or the
deployment owner explicitly accepts the remaining historical limitation. A
database that never ran the legacy import does not require an import repair.
This prerequisite is operational; the worker does not enforce an import marker.
