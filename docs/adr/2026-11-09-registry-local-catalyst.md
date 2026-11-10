# Registry API over local Catalyst data and native conversion state

- Status: Accepted
- Date: 2026-11-09

The Registry surface is composed under `/asset-bundle-registry` in the existing
Catalyst HTTP server. Profiles use Catalyst's shared entity lookup and profile
processing directly. Registry operations that need conversion state use the
existing abgen service. No Registry profile or entity response cache is added.

`POST /profiles` and `POST /profiles/metadata` share the marketplace state, content
denylist, ownership readers and profile transformations used by Lambdas. They
accept an empty `ids` array, resolve case-insensitive pointers once, and project
the processed results into Registry response shapes. Non-profile entities and
profiles without a usable avatar are omitted. Lookup and ownership processing
failures are logged; the Registry wrapper returns only available validated
profiles, or `200 []` when the shared batch cannot produce any. Lambdas retains
its own failure envelopes. Full profiles use an entity
timestamp string. Snapshots and compact thumbnails point to the configured local
profile-images service. The Registry projection does not use Lambdas' profile
response cache, so a subsequent read sees changes in the local entity view.

The Explore Registry facade forwards these explicitly registered routes to abgen:

| Method | Paths |
| --- | --- |
| POST | `/entities/active`, `/entities/versions` |
| GET | `/entities/status`, `/entities/status/{id}` |
| GET | `/worlds/{world_name}/manifest`, `/denylist`, `/queues/status` |
| POST, DELETE | `/denylist/{entity_id}` |
| POST | `/queues/retry`, `/queues/pause`, `/queues/resume`, `/registry` |
| DELETE | `/flush-cache` |

The facade preserves the HTTP method, raw query, request body, content type,
bearer credentials and signed-fetch headers. It overwrites `x-original-path`
with the actual request URI received by Catalyst, which may include a
gateway-added Registry prefix. The native verifier must bind that path to the
forwarded route and accept only the canonical service path and its single known
Registry-prefix alias for signing. Client-supplied path headers do not establish
authority. Authorization, moderator policy, queue
controls and conditional admin-route registration belong to native abgen.

The backend is `EXPLORER_REGISTRY_SOURCE_URL`, falling back to `ABGEN_ADMIN_URL`.
The URL must use HTTP and a loopback/private IP address or the configured service
names admitted by the facade (`localhost` and `catalyst-abgen`), without credentials,
query or fragment. It is an operator-configured service boundary, independent of
mirror posture. An absent backend returns 503; a failed request or response over
64 MiB returns 502. Requests time out after 60 seconds. Redirects are not followed.
Responses preserve status, body, content type, cache directives, validators,
`Allow` and `Retry-After`. Local `/status` and `/health` describe the Catalyst
facade; they do not establish native worker readiness.

Deployments require a compatible native Registry implementation in
`decentraland/abgen`, configured to read this deployment's Catalyst and Worlds
APIs and to persist its own Registry state. This change does not select a new
abgen release or provide a converter deployment. A backend exposing only the
older conversion API cannot satisfy the newly exposed Registry operations.
The existing gateway routes Registry traffic to Catalyst, so no additional
Nginx upstream or new service is introduced here.

The alternative of retaining an external Registry cache would duplicate the
entity/profile view and obscure native worker failures. Reimplementing conversion
state inside Catalyst would split the converter's state ownership. The chosen
boundary reuses Catalyst's existing implementations and keeps conversion state
with its existing service.

Compatibility expectations are scoped to Registry source
`63c9ef2bd7365ac7312d8bd23b6759515ca999e5` and explicit checked-in captures.
The profile input error tests use the 2026-11-05 captures at Registry
`f9578d4ab1ab9fdff4576ba6d052b966bd0fd5e9`: malformed JSON returns 500, while a
valid JSON value with invalid `ids` returns 400. Full transport equivalence,
including exact OPTIONS status/headers and all failure envelopes, is not implied
by sharing the profile implementation.
