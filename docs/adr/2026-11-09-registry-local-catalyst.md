# Registry API over local Catalyst data and native conversion state

- Status: Accepted
- Date: 2026-11-09

The Registry surface is composed under `/asset-bundle-registry` in the existing
Catalyst HTTP server. Profiles use Catalyst's shared entity lookup and profile
processing directly. The remaining Registry reads go through a gateway without
a response cache. No Registry profile or entity response cache is added.

`POST /profiles` and `POST /profiles/metadata` share the marketplace state, content
denylist, ownership readers and profile transformations used by Lambdas. They
accept an empty `ids` array and, as the Registry, any number of ids: the only
bound is the 256 KiB request body (about 5,800 addresses), over which the answer
is a 413 in the Registry's JSON error envelope.
They resolve case-insensitive pointers once and project the
processed results into Registry response shapes, in request order.
Non-profile entities and profiles without a usable avatar are omitted; an entity
missing its id, pointer or timestamp is logged and omitted. As the Registry, a
failed lookup layer is logged and answers 200 with what was found: a failed entity
lookup answers `[]`, and the ownership pass runs per 128 entities, so a failed pass
leaves out its chunk only. Unvalidated metadata is never served. Full profiles carry the entity
timestamp as a JSON number and both answers keep the Registry's key order, as the
2026-11-05 captures `AB-1-one-known.json` and `AB-2-one-known.json` show. Snapshots and compact thumbnails point to the configured local
profile-images service; each full-profile avatar carries exactly the `face256` and
`body` snapshots, as the Registry's sanitizer writes them. The Registry projection
does not use Lambdas' profile response cache, so a subsequent read sees changes in
the local entity view.

The Explore Registry gateway serves these reads from the backend named by
`EXPLORER_REGISTRY_SOURCE_URL`:

| Method | Paths |
| --- | --- |
| POST | `/entities/active`, `/entities/versions` |
| GET | `/entities/status`, `/entities/status/{id}`, `/worlds/{world_name}/manifest` |
| GET | `/denylist`, `/queues/status` |

The variable is the only selector; `ABGEN_ADMIN_URL` does not enable the gateway,
because an abgen build without native Registry state answers these routes
differently. The source is an `http(s)` base URL without credentials, query or
fragment, and startup refuses any other value, naming the variable.

- A loopback host (`127.0.0.0/8`, `::1`, `localhost`, the `ABGEN_ADMIN_URL` rule)
  is the local abgen runtime. It must implement the native Registry routes
  (decentraland/abgen#145 or a compatible release). The gateway forwards the
  signed-fetch `x-identity-*` headers and overwrites `x-original-path` with the
  path Catalyst received, without the query. The native verifier binds that path
  to the forwarded route and accepts only the canonical service path and its single
  Registry-prefix alias.
- Any other host is a public Registry, for example the mirror posture default
  `https://asset-bundle-registry.decentraland.org`. It passes the same SSRF and
  own-host checks as the other explorer sources, at startup and on each request,
  and receives no identity or path headers. Signed `GET /entities/status` calls
  therefore fail in this mode with the Registry's 400 `Invalid Auth Chain`;
  signed reads need the local runtime.
- Empty, the island posture, answers `/entities/active` and `/entities/versions`
  with `200 []`, `/denylist` with the node's own rows, and the other reads with 503.

`GET /denylist` merges the source's list with the node's `content.denylist` rows,
newest first by `created_at`. An id both lists name (compared case-insensitively,
as the Registry lowercases at insert) keeps the source's entry; an operator row
reads `{entity_id, reason: null, created_by, created_at, updated_at}` with
`updated_at` equal to `created_at`, in milliseconds as strings. A source that fails
or answers anything but a 200 JSON array fails the read with its status, rather
than a shorter list: a content server polling the list keeps its last good copy on
an error, and would lift the source's ids on a 200.

Whatever the source, `/entities/active` and `/entities/versions` first apply the
Registry's request rule: a body without a non-empty `pointers` (`ids` is not read)
answers `400 {"ok":false,"message":"No pointers provided"}` and a body that is not
JSON `500 {"error":"Internal Server Error"}`, with no cap on the pointers. A
successful `/entities/active` answer then drops every entity whose id is on the
node's content denylist projection, the one the content reads apply.

Every case forwards the method, raw query, body, content type and `accept`, and
never `authorization`; conditional headers are forwarded except on
`/entities/active` and `/denylist`, whose answers the gateway may rewrite. Registry writes (`/denylist/{id}`
POST/DELETE, `/queues/retry|pause|resume`, `/registry`, `/flush-cache`) are not
routed: the abgen admin bearer stays on the host and those operations remain with
the ops console, behind its sign-in, write switch and audit log.

Nothing is cached, so every read, including each explorer boot read, is a
backend call. At most 64 backend requests run at once, bounding buffered
responses at 512 MiB; a read waits up to 2 seconds for a free slot and is then
refused with 503 and `Retry-After: 1`. A request times out after 30 seconds, does
not follow redirects and buffers at most 8 MiB; a failed request or a larger
response returns 502. A
path the URL parser would normalize, such as an encoded `..`, returns 400.
Responses preserve status, body, content type, cache directives, validators,
`Allow` and `Retry-After`. Local `/status`, `/health` and the status-check probes
(`/health/live`, `/health/ready`, `/health/startup`) describe the Catalyst
gateway; they do not establish native worker readiness. Every path under the
prefix, routed by either half or not, answers the Registry's CORS: a 204
preflight with origin `*`, its method list, the requested headers and a one-day
max-age. Behind `infra/gateway`, a response without
`Access-Control-Allow-Origin`, such as the answer to a request without
`Origin`, gets `Access-Control-Allow-Origin: *` from the gateway. The existing
gateway routes Registry traffic to Catalyst, so no additional Nginx upstream or new
service is introduced here, and this change does not select an abgen release.

The alternative of retaining an external Registry cache would duplicate the
entity/profile view and obscure native worker failures. Reimplementing conversion
state inside Catalyst would split the converter's state ownership. The chosen
boundary reuses Catalyst's existing implementations and keeps conversion state
with its existing service.

Compatibility expectations are scoped to Registry source
`63c9ef2bd7365ac7312d8bd23b6759515ca999e5` and explicit checked-in captures.
The profile input error tests use the 2026-11-05 captures at Registry
`f9578d4ab1ab9fdff4576ba6d052b966bd0fd5e9`: malformed JSON returns 500, while a
valid JSON value with invalid `ids` returns 400. The CORS preflight follows the
2026-11-05 capture `AB-12-preflight.json`. Full transport equivalence, including
all failure envelopes, is not implied by sharing the profile implementation.
