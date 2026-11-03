# Event clients and deployment

## Public URLs and image paths

Set `PUBLIC_URL` to the externally reachable node base URL, including any deployment prefix. The default bind-address fallback is for local use; a host such as `0.0.0.0` is not a public client destination. `EVENTS_BASE_URL` identifies the events website for sitemaps and does not select the backend asset origin.

Generated defaults are stored as `/assets/event-default.webp` and expanded in responses and notifications to `PUBLIC_URL/events/assets/event-default.webp`. Both horizontal and vertical defaults use this projection. Changing `PUBLIC_URL` does not require rewriting canonical stored paths.

Uploaded posters retain their existing router-local paths, `/poster/<file>` and `/poster-vertical/<file>`. Clients using those paths prepend the backend mount: for `PUBLIC_URL=https://node.example.org/prefix`, `/poster/example.webp` is fetched from `https://node.example.org/prefix/events/poster/example.webp`. Preserve the deployment prefix and `/events` mount; resolving a leading-slash path against a URL with `new URL(path, base)` drops both. Absolute external images are used as supplied.

## Generated wallet and bearer request types

The normal PATCH request body references the strict `UpdateEventBody` wallet schema. Generic OpenAPI generators do not automatically switch schemas based on authentication and may ignore `x-bearer-request-schema`. Bearer callers explicitly use the exported `AdminEventPatch` component when constructing their body. This example is placed at the repository root; adjust the relative import for its consumer:

```ts
import type { components, operations } from './library/catalyst-api/openapi/events'

type BearerPatch = components['schemas']['AdminEventPatch']
type PatchBody = operations['patch_event']['requestBody']['content']['application/json']

const adminBody: BearerPatch = {
  action: 'reject',
  reason: 'Duplicate event',
  legacy_client_field: true
}
const requestBody: PatchBody = adminBody
```

Pass that typed `requestBody` through the generated PATCH operation with bearer authentication. This uses structural assignment without a cast; fresh wallet body literals still reject unknown fields and bearer-only control keys. The server remains authoritative: signed-wallet requests reject unknown fields, while authenticated bearer requests retain their documented legacy extras.

Bearer requests and the console's `event_moderate` command accept reason-only patches such as `{"reason":"Updated explanation"}` for rejected local events and mirrored events with an explicit local rejection. These edits preserve the current local verdict, rejection actor and independent feature flags, and create no new rejection notification. For a mirrored rejection owned by the upstream, a reason edit returns 409 unless the request explicitly makes a local rejection decision (resolving to `rejected=true`). Approving or clearing rejection can still proceed normally; setting only `approved=false` does not authorize editing the upstream's reason. This also applies when the reason accompanies feature fields: clarifying an upstream reason must not silently freeze the upstream verdict. An overlay containing only feature decisions does not make the rejection locally owned.

A non-null reason included with other admin fields, for example `{"highlighted":true,"reason":"Updated explanation"}`, updates the explanation when the resulting event is rejected, subject to the same ownership rule. If the resulting event is not rejected, those other fields are applied and the reason is ignored. A standalone reason on an event that is not rejected returns 400 without changing it.

Both `reason` and `rejection_reason` are accepted; an explicit `reason` key takes precedence. A standalone null reason is not an edit. To clear a reason, include the rejection decision, for example `{"rejected":true,"reason":null}`. This is an explicit re-rejection: it records the current actor in `rejected_by` and, on a mirrored event, creates or updates the local rejection decision.

## Conditional repair for an early staging default

No automatic historical rewrite is performed. This repair is relevant only if an earlier build that stored the website's absolute default was actually deployed. Check the affected deployment and take a database backup before any authorized maintenance.

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

## Optional upcoming-event Slack summaries

`EVENTS_UPCOMING_SUMMARIES_ENABLED=true` explicitly enables aggregate summaries
through `SLACK_WEBHOOK` (or `SLACK_WEBHOOK_FILE`) when `OPS_WRITES_ENABLED=true`.
It defaults to false, independently of moderation alerts. A missing or blank
webhook disables summary scanning and delivery.

Each minute, a leased worker scans up to 100 approved native occurrences starting
within the next hour with local going RSVPs. Repeated scans include late approvals,
new RSVPs and rescheduled occurrences; retained occurrence keys prevent duplicates.
At most ten deliveries run per tick. Payloads contain the current public name,
event link and current aggregate RSVP count, never attendee identities or private
author fields. Rejected, deleted, rescheduled and expired occurrences are rechecked
before delivery. A temporary loss of approval or RSVPs removes the unsent intent;
it can be captured again if eligibility returns before the occurrence starts.
Successfully delivered keys remain retained to prevent another summary.
HTTP delivery is at least once. Unfinished work is a dead letter when retries
are exhausted or the occurrence has started, including after downtime:

```sql
SELECT event_id, occurrence_start, attempts
FROM explore.event_upcoming_hooks
WHERE completed_at IS NULL
  AND (attempts >= 6 OR occurrence_start <= timezone('UTC', now()));
```

Occurrences older than thirty days are cleaned in bounded batches, including after
summary opt-out while operator jobs remain enabled. Downtime does not replay past
occurrences.
