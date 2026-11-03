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

Bearer reason-only patches such as `{"reason":"Updated explanation"}` update an already rejected event without changing its verdict, rejection actor or independent feature flags. They create no new rejection notification. A reason-only request on an event that is not rejected returns 400. Both `reason` and `rejection_reason` are accepted; an explicit `reason` key takes precedence. A standalone null reason is not an edit: to clear it, include the rejection decision, for example `{"rejected":true,"reason":null}`.

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
