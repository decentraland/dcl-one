# Asset-bundle registry: content-related endpoints

- Area: every route of the asset-bundle registry (ABR), split into what a Catalyst replacement must
  answer from its own content (profiles, the profile metadata projection, world manifests, the
  denylist read, and the content fields of `entities/active` and `entities/status`) and what stays
  asset-bundle state or ABR administration (bundle and version state, build queues, re-ingest, cache
  flush, denylist writes).
- Upstream ABR: `decentraland/asset-bundle-registry` @ `f9578d4ab1ab9fdff4576ba6d052b966bd0fd5e9`
  (2026-10-21). Production served `/status` with that `commitHash` and `version` 2.5.3 on 2026-11-05
  ([`captures/abr/AB-15-status.json`](captures/abr/AB-15-status.json)); the zone registry served
  `d932a02eaa90345f846b068f5c8c2b9ab388deaa`, `version` `Unknown`
  ([`captures/abr/AB-15-zone-status.json`](captures/abr/AB-15-zone-status.json)).
- Upstream content server (it consumes the ABR denylist): `decentraland/catalyst` @
  `a416565db1e705cff9523540a2578af642598a29` (2026-10-21).
- Upstream lambdas (production owner of `/lambdas/profiles/:id`): `decentraland/lamb2` @
  `02a37f5d88fb895339c20e84c2470be68cdf3f57` (2026-10-21).
- dcl-one: `main` @ `20037334` (2026-11-05), workspace `services/catalyst`; open pull request
  `decentraland/dcl-one#257` (branch `feat/catalyst-compact-profiles`) read from its diff.
- Method: static reading, plus the captures under [`captures/abr/`](captures/abr/) taken on 2026-11-05
  from `peer.decentraland.org`, `peer.decentraland.zone`, `asset-bundle-registry.decentraland.org` and
  `asset-bundle-registry.decentraland.zone` (format and sanitization in
  [`captures/README.md`](captures/README.md)). A row citing a capture states what that service
  answered; every other row was read. Nothing in this tree was built or run.
- ABR behaviour that lives in npm packages missing from the clone (`@dcl/http-server` routing, 404,
  405 and body limits; `@dcl/http-commons` `errorHandler` bodies; `@dcl/crypto-middleware`) is taken
  from the captures; rows that no capture covers say "not vendored".

Files read (ABR): `src/controllers/routes.ts`, `src/controllers/handlers/{get-service-status,get-active-entities,get-entity-versions,get-entity-status,get-world-manifest,get-profiles,get-profiles-metadata,get-denylist,post-denylist-entry}.ts`,
`src/controllers/schemas/{profiles,worlds}.ts`, `src/logic/profile-retriever.ts`, `src/logic/sync/{profile-sanitizer,ownership-validator-job}.ts`, `src/logic/registry-parser.ts`,
`src/logic/coordinates/component.ts`, `src/utils/{webgl-compat,pointers}.ts`, `src/adapters/{db,catalyst}.ts`, `src/types/types.ts`, `src/migrations/1763329911778_profiles.ts`, `.env.default`.
Upstream catalyst: `src/Environment.ts`, `src/adapters/denylist/component.ts`, `src/components.ts`, `src/controllers/handlers/get-content-handler.ts`, `src/controllers/utils.ts`.

Files read (dcl-one): `explore/src/api/{registry,mod,denylist}.rs`, `config/src/{posture,explore,ops}.rs`, `content/src/{denylist,queries,active_entities}.rs`,
`shop/src/market/{mod,profiles}.rs`, `shared/src/http_util.rs`, `worlds/src/{lib,queries}.rs`, `src/serve.rs`, `../../infra/gateway/{root-aliases.nix,gateway.py}`,
and the `decentraland/dcl-one#257` diff (`shop/src/market/{mod,profiles,tests}.rs`).

## 1. Which production URLs reach ABR

None of the Catalyst URLs probed on 2026-11-05 is redirected (no 3xx, no `Location`) or proxied to
ABR, on the production peer or on the zone peer. Each answer came from lamb2, the content server or
the gateway in front of them, never with an ABR-shaped body:

| request | production and zone answer | layer | capture |
|---|---|---|---|
| `POST /lambdas/profiles/metadata` | `405`, empty body, `Allow: HEAD, GET` (lamb2's `GET /profiles/:id`) | lamb2 | `AB-14-{peer,zone}-post-lambdas-profiles-metadata.json` |
| `GET /lambdas/profiles/metadata` | `404 {"error":"Not Found","message":"Profile not found"}` (a profile id `metadata`) | lamb2 | `AB-14-{peer,zone}-get-lambdas-profiles-metadata.json` |
| `OPTIONS /lambdas/profiles/metadata` | `204`, `access-control-allow-origin: *`, max-age 36000 (lamb2's CORS, not ABR's 86400) | lamb2 | `AB-14-{peer,zone}-preflight-lambdas-profiles-metadata.json` |
| `POST /profiles/metadata`, `POST /profiles`, `POST /entities/versions`, `GET /entities/status/0,0`, `GET /worlds/<name>/manifest`, `GET /denylist`, `GET /queues/status`, `GET /asset-bundle-registry/status` (root paths) | `404 {"ok":false,"error":"404 path not found"}`, `application/octet-stream` | gateway | `AB-14-{peer,zone}-*-root-*.json`, `AB-14-{peer,zone}-get-abr-prefix-status.json` |
| `POST /content/entities/versions` | `405`, `Allow: HEAD, GET` (content's `GET /entities/:type`) | content server | `AB-14-{peer,zone}-post-content-entities-versions.json` |
| `GET /content/entities/versions` | `400 {"error":"Unrecognized type: versions"}` | content server | `AB-14-{peer,zone}-get-content-entities-versions.json` |
| `GET /content/entities/status/0,0`, `GET /content/worlds/<name>/manifest`, `GET /content/denylist` | `404 Not found` (`text/plain`) | content server | `AB-14-{peer,zone}-get-content-*.json` |
| `POST /lambdas/entities/versions` | nginx `404` HTML | gateway | `AB-14-{peer,zone}-post-lambdas-entities-versions.json` |
| `POST /content/entities/active {"pointers":["0,0"]}` | the content server's entity (keys `id, type, pointers, content, version, timestamp, metadata` on the peer), no `deployer`, `status`, `bundles` or `versions` | content server | `AB-4-peer-content-entities-active.json`, `AB-4-zone-content-entities-active.json` |

The one production dependency of a Catalyst on ABR is the denylist: the content server reads
`https://asset-bundle-registry.decentraland.org/denylist` by default
(`decentraland/catalyst:src/Environment.ts:13`, `:513-514`) at boot and every 120 s
(`decentraland/catalyst:src/components.ts:134-137`), and drops every listed `entity_id` from its reads
(`decentraland/catalyst:src/adapters/denylist/component.ts:59-86`). On production, the first entry of
ABR's list answers `404` to `GET /content/contents/<id>` with `If-None-Match: *`, where a hash that is
not listed answers `304`; the denylist check runs before the `If-None-Match` check
(`decentraland/catalyst:src/controllers/handlers/get-content-handler.ts:22-24`,
`decentraland/catalyst:src/controllers/utils.ts:102-104`)
([`AB-10-peer-denylisted-hash.json`](captures/abr/AB-10-peer-denylisted-hash.json),
[`AB-10-peer-valid-hash.json`](captures/abr/AB-10-peer-valid-hash.json)). ABR still reports that entity
as converted ([`AB-6-denylisted-id.json`](captures/abr/AB-6-denylisted-id.json)). The zone peer
answered `404` to both requests, so the zone result does not show whether it applies the list
([`AB-10-zone-*.json`](captures/abr/)).

Clients call ABR at its own host (`asset-bundle-registry.<domain>`); in dcl-one that host and the
`/asset-bundle-registry` prefix reach the read-through router (`services/catalyst/explore/src/api/mod.rs:697-741`,
`services/catalyst/config/src/posture.rs:398-402`).

## 2. Endpoint inventory

| method | ABR path | class | production URL prefix | dcl-one route(s) | status |
|---|---|---|---|---|---|
| GET | `/status` | service status | `asset-bundle-registry.<domain>` | `/asset-bundle-registry/status` `services/catalyst/explore/src/api/registry.rs:86` | MATCH |
| POST | `/entities/active` | mixed: content fields + AB state | ABR host | `/asset-bundle-registry/entities/active` `services/catalyst/explore/src/api/registry.rs:82` (read-through) | GAP |
| POST | `/entities/versions` | AB state (only `pointers` is content) | ABR host | `/asset-bundle-registry/entities/versions` `services/catalyst/explore/src/api/registry.rs:83` (read-through) | DIVERGENT |
| GET | `/entities/status/:id` | mixed: `entityId`, `catalyst` + AB state | ABR host | none | MISSING |
| GET | `/entities/status` (signed fetch) | mixed: per deployer | ABR host | none | MISSING |
| GET | `/queues/status` | AB only | ABR host | none (root `/queues/` is aliased to `/asset-bundle-registry/queues/status`, which has no route: `infra/gateway/root-aliases.nix:506-513`) | MISSING (out of parity scope) |
| GET | `/worlds/:worldName/manifest` | content | ABR host | none | MISSING |
| POST | `/profiles` | content | ABR host | `/asset-bundle-registry/profiles` `services/catalyst/explore/src/api/registry.rs:84` (read-through) | GAP |
| POST | `/profiles/metadata` | content | ABR host | `/asset-bundle-registry/profiles/metadata` `services/catalyst/explore/src/api/registry.rs:85` (read-through); `decentraland/dcl-one#257` adds `/lambdas/profiles/metadata` from the node's profiles | GAP |
| GET | `/denylist` | content (moderation list) | ABR host | none (root `/denylist` is aliased to `/asset-bundle-registry/denylist`, which has no route: `infra/gateway/root-aliases.nix:497-504`) | MISSING |
| POST | `/denylist/:entityId` (signed fetch, moderators) | ABR admin | ABR host | none; the operator console edits `content.denylist` (`services/catalyst/src/ops/admin/create.rs:546`) | MISSING (out of parity scope) |
| DELETE | `/denylist/:entityId` (signed fetch, moderators) | ABR admin | ABR host | none; console `services/catalyst/src/ops/admin/create.rs:574` | MISSING (out of parity scope) |
| POST | `/registry` (bearer) | ABR admin | ABR host | none; the console calls the external abgen runtime (`services/catalyst/config/src/ops.rs:124-133`) | MISSING (out of parity scope) |
| DELETE | `/flush-cache` (bearer) | ABR admin | ABR host | none; same console bridge | MISSING (out of parity scope) |

Counts: 14 endpoints. MATCH 1, GAP 3, DIVERGENT 1, MISSING 9 (5 of them ABR admin or build-queue
routes outside catalyst parity). Content-related: 7 (`entities/active`, `entities/status/:id`,
`entities/status`, `worlds/:worldName/manifest`, `profiles`, `profiles/metadata`, `GET /denylist`).

Upstream routes: `decentraland/asset-bundle-registry:src/controllers/routes.ts:34-58`. `/profiles` and
`/profiles/metadata` are registered only when `DISABLE_PROFILES` is not `true`
(`decentraland/asset-bundle-registry:src/controllers/routes.ts:44-48`; `.env.default` sets `false`).
`/registry` and `/flush-cache` exist only with `API_ADMIN_TOKEN` (`:54-58`).

### Field classification of the mixed routes

| route | content fields (parity target) | asset-bundle fields (stay with ABR / the read-through) |
|---|---|---|
| `POST /entities/active` | `id`, `type`, `timestamp`, `pointers`, `content`, `metadata`, `deployer` (from the deployment's auth chain) | `status` (`complete`/`fallback` only are served), `bundles.{assets,lods}.{mac,windows,webgl}`, `versions.assets.{mac,windows,webgl}.{version,buildDate}` |
| `POST /entities/versions` | `pointers` | `versions`, `bundles`, `status` |
| `GET /entities/status/:id` | `entityId`; `catalyst` (always `"complete"` when a registry exists); the id/pointer resolution (current id, historical id, or newest entity on the pointer, `world_name` aware) | `complete`, `assetBundles.{mac,windows}`, `lods.{mac,windows}` |
| `GET /entities/status` | the deployer's entity ids (current and historical) | per-entity AB state as above |
| `GET /worlds/:worldName/manifest` | `occupied`, `spawn_coordinate`, `total` | only the filter: ABR counts parcels of registries in `complete`/`fallback` (converted) state |

### Cross-cutting behaviour of the ABR host

| aspect | ABR | dcl-one read-through | verdict |
|---|---|---|---|
| 400 from `InvalidRequestError` | `{"error":"Bad request","message":<msg>}` ([`AB-3-missing-ids.json`](captures/abr/AB-3-missing-ids.json)) | `{"error":<msg>}` `services/catalyst/explore/src/api/mod.rs:770-772` | DIVERGENT |
| handler 400/404 | `{"ok":false,"message":<msg>}` (`decentraland/asset-bundle-registry:src/controllers/handlers/get-active-entities.ts:13-21`) | `{"error":<msg>}` | DIVERGENT |
| unhandled error (unparseable JSON, empty body) | `500 {"error":"Internal Server Error"}` ([`AB-3-not-json.json`](captures/abr/AB-3-not-json.json), [`AB-3-empty-body.json`](captures/abr/AB-3-empty-body.json)) | axum `Json` rejection: `400` `text/plain` | DIVERGENT |
| `Content-Type` | ignored: `text/plain` with a JSON body answers `200` ([`AB-3-text-plain.json`](captures/abr/AB-3-text-plain.json)) | axum `Json`: `415` `text/plain` | DIVERGENT |
| unknown route | `404 Not found` `text/plain; charset=utf-8` ([`AB-12-unknown-route.json`](captures/abr/AB-12-unknown-route.json)) | falls through to the content 404 (`text/plain` `Not found`) | MATCH |
| wrong method | `405`, empty body, `Allow: POST` ([`AB-1-get.json`](captures/abr/AB-1-get.json), [`AB-1-put.json`](captures/abr/AB-1-put.json)) | axum `405`, empty body, `Allow: POST` | MATCH |
| trailing slash | `POST /profiles/metadata/` answers as without it ([`AB-1-trailing-slash.json`](captures/abr/AB-1-trailing-slash.json)) | trimmed for the whole app (`services/catalyst/src/serve.rs:1355`) | MATCH |
| CORS preflight | `204`, `allow-origin: *`, `allow-methods: GET,HEAD,OPTIONS,DELETE,POST,PUT`, `allow-headers` reflected, `max-age: 86400`, `vary: Access-Control-Request-Headers` ([`AB-12-preflight.json`](captures/abr/AB-12-preflight.json)) | `CorsLayer::permissive()` (`services/catalyst/explore/src/api/mod.rs:741`): any origin, any method, any header, expose any header, no max-age | DIVERGENT |
| upstream failure | n/a | `502 {"error":"asset-bundle registry upstream: <route> failed"}` `services/catalyst/explore/src/api/registry.rs:149-155` | dcl-one only |
| no source configured | n/a | every POST answers `200 []` `services/catalyst/explore/src/api/registry.rs:165-167`; `EXPLORER_REGISTRY_SOURCE_URL` is set only by `NODE_POSTURE=mirror` (`services/catalyst/config/src/posture.rs:398-402`) | dcl-one only |
| rate limits | none in ABR | gateway limit by IP (README "Rate limits") | as the other areas |

---

## 3. GET /status

**Request:** none.

| trigger | status | body | headers | upstream | dcl-one | verdict |
|---|---|---|---|---|---|---|
| always | 200 | `{"data":{"version":"2.5.3","currentTime":1793883010167,"commitHash":"f9578d4a…"}}` ([`AB-15-status.json`](captures/abr/AB-15-status.json)); `version`/`commitHash` default to `""` | `content-type: application/json` | `decentraland/asset-bundle-registry:src/controllers/handlers/get-service-status.ts:11-30` | same keys and order, the node's `version` and `commit_hash` `services/catalyst/explore/src/api/registry.rs:459-468` | MATCH (values are the node's) |

**Data dependency:** none.

## 4. POST /entities/active

**Request:** body `{"pointers": string[]}`; `ids` is not read. Query `world_name` (case-insensitive):
with it only entities whose `metadata.worldConfiguration.name` matches; without it world entities are
excluded. No cap on the number of pointers, no auth. Pointers are lowercased
(`decentraland/asset-bundle-registry:src/adapters/db.ts:62-119`).

| trigger | status | body | upstream | dcl-one | verdict |
|---|---|---|---|---|---|
| `pointers` missing, not truthy or empty (also `{"ids":[...]}`) | 400 | `{"ok":false,"message":"No pointers provided"}` ([`AB-4-empty-pointers.json`](captures/abr/AB-4-empty-pointers.json)) | `get-active-entities.ts:13-21` | `pointers` and `ids` both accepted; neither, both, or empty → `400 {"error":"ids or pointers must be present in the request body, and only one of them"}` `services/catalyst/explore/src/api/registry.rs:106-119`; `ids` are forwarded and ABR answers 400, read here as `502` | DIVERGENT |
| more than 1000 pointers | 200 (no cap) | entity array | `get-active-entities.ts:10-48` | `400 {"error":"at most 1000 ids or pointers per request"}` `registry.rs:120-125` | DIVERGENT |
| `pointers` not an array (e.g. a string) | the string is passed to SQL as an array literal; not vendored | — | `get-active-entities.ts:11` | axum `422` `text/plain` | needs capture |
| found | 200 | one element per distinct pointer set, `status` in `complete`/`fallback`, denylisted ids excluded; keys in order `id, type, timestamp, deployer, pointers, content, metadata, status, bundles, versions`; `timestamp` is a JSON **string** (Postgres `bigint`); `webgl` re-added to `bundles` and `versions` (`src/utils/webgl-compat.ts:46-81`). Example: `{"id":"bafkrei…","type":"scene","timestamp":"1791215990965","deployer":"0x…0b02","pointers":["-1,-4",…],"content":[{"file":"main.crdt","hash":"bafkrei…"},…],"metadata":{…},"status":"fallback","bundles":{"assets":{"mac":"complete","windows":"complete","webgl":"pending"},"lods":{"mac":"pending","windows":"pending","webgl":"pending"}},"versions":{"assets":{"mac":{"version":"v49","buildDate":"2026-10-05T16:08:08.914Z"},…}}}` ([`AB-4-pointer-0-0.json`](captures/abr/AB-4-pointer-0-0.json)) | `get-active-entities.ts:29-48`, `src/logic/registry-parser.ts:1-17` | ABR's bytes, cached 300 s per key and per entity id `registry.rs:191-299` | MATCH for entities ABR knows |
| order | 200 | grouped by the whole `pointers` array in database row order; the query has no `ORDER BY` for this route, so the element kept per group is the first row returned | `db.ts:113-115`, `registry-parser.ts:1-17` | request-key order, each entity once `registry.rs:281-298` | DIVERGENT (ABR order unspecified) |
| entity deployed to this node, not converted by Decentraland's ABR | 200 | absent (ABR has no registry for it) | — | absent: the read-through never reads this node's entities | GAP (see AB-4) |
| entity on this node's `content.denylist` | — | ABR applies its own list | `db.ts:105-111` | the read-through does not consult `content.denylist` (`services/catalyst/content/src/queries.rs:78` is applied only on the content routes) | GAP (AB-5) |

**Data dependency:** content fields: the node's content DB (active entities, `deployer_address`),
which dcl-one holds. AB fields: Decentraland's ABR (read-through) or a local abgen.

## 5. POST /entities/versions

Same request rules as `entities/active` (`pointers` only, no cap, `world_name`); no denylist filter
(`decentraland/asset-bundle-registry:src/controllers/handlers/get-entity-versions.ts:10-44`).

| trigger | status | body | dcl-one | verdict |
|---|---|---|---|---|
| no pointers | 400 | `{"ok":false,"message":"No pointers provided"}` ([`AB-11-missing-pointers.json`](captures/abr/AB-11-missing-pointers.json)) | `{"error":"ids or pointers …"}` `registry.rs:106-119` | DIVERGENT |
| found | 200 | `[{"pointers":[…],"versions":{"assets":{"mac":{…},"windows":{…},"webgl":{"version":"","buildDate":""}}},"bundles":{"assets":{…},"lods":{…}},"status":"fallback"}]` ([`AB-11-pointer-0-0.json`](captures/abr/AB-11-pointer-0-0.json)) | ABR's bytes, cached per sorted key set; the pointers are sent sorted, not in request order `registry.rs:303-336` | MATCH (body is ABR's) |

**Data dependency:** AB state only.

## 6. GET /entities/status/:id

**Request:** `:id` is an entity id (current registry, then historical registry, compared lowercased)
or a pointer; query `world_name`. No auth.

| trigger | status | body | upstream | dcl-one | verdict |
|---|---|---|---|---|---|
| found | 200 | `{"entityId":"bafkrei…","catalyst":"complete","complete":false,"assetBundles":{"mac":"pending","windows":"pending"},"lods":{"mac":"pending","windows":"pending"}}` ([`AB-6-by-pointer.json`](captures/abr/AB-6-by-pointer.json)); a pointer resolves to the newest registry of any status; missing AB values read `pending`; `lods` always present (defaults) | `get-entity-status.ts:5-54` | no route | MISSING |
| denylisted entity | 200 | answered as any other ([`AB-6-denylisted-id.json`](captures/abr/AB-6-denylisted-id.json)) | `db.ts:121-130` (no denylist filter) | no route | MISSING |
| not found | 404 | `{"ok":false,"message":"No active entity found for the provided id or pointer"}` ([`AB-6-unknown-id.json`](captures/abr/AB-6-unknown-id.json)) | `get-entity-status.ts:56-65` | no route | MISSING |

The body is built with `JSON.stringify` and sent as a string with `Content-Type: application/json`.

**Data dependency:** content DB (entity by id, overwritten entities, newest entity on a pointer, worlds
by name). dcl-one holds all of it; the AB fields come from the read-through or default to `pending`.

## 7. GET /entities/status (signed fetch)

**Request:** ADR-44 signed fetch, required; scene-signed requests (`signer: decentraland-kernel-scene`)
are refused (`decentraland/asset-bundle-registry:src/controllers/routes.ts:22-32`).

| trigger | status | body | upstream | dcl-one | verdict |
|---|---|---|---|---|---|
| unsigned | 400 | `{"error":"Invalid Auth Chain","message":"This endpoint requires a signed fetch request. See ADR-44."}` ([`AB-7-unsigned.json`](captures/abr/AB-7-unsigned.json)) | `routes.ts:22-32` | no route | MISSING |
| signer not an address | 400 | `{"ok":false,"message":"Invalid user address"}` | `get-entity-status.ts:78-84` | no route | MISSING |
| signed | 200 | array of the entity-status object above for every registry, then every historical registry, whose `deployer` is the signer (lowercased) | `get-entity-status.ts:86-100` | no route | MISSING |

**Data dependency:** content DB by deployer (`deployer_address`), including overwritten deployments.

## 8. GET /worlds/:worldName/manifest

| trigger | status | body | upstream | dcl-one | verdict |
|---|---|---|---|---|---|
| name not matching `^[a-zA-Z0-9-]+(\.dcl)?\.eth$` | 400 | `{"ok":false,"message":"A valid world name is required"}`, no `Content-Type` set by the handler (the captured answer is `application/json`) ([`AB-8-invalid-name.json`](captures/abr/AB-8-invalid-name.json)) | `get-world-manifest.ts:24-46`, `schemas/worlds.ts:1-3` | no route | MISSING |
| unknown or empty world | 200 | `{"occupied":[],"spawn_coordinate":{"x":0,"y":0},"total":0}` ([`AB-8-unknown-world.json`](captures/abr/AB-8-unknown-world.json)) | `coordinates/component.ts:286-288` | no route | MISSING |
| world with scenes | 200 | `occupied`: distinct `"x,y"` strings of the world's converted, non-denylisted scenes, unordered (`SELECT DISTINCT unnest(pointers)`); `spawn_coordinate`: the stored spawn (`world_spawn_coordinates`) if inside the bounding box, else the occupied parcel nearest the centroid; `x` and `y` are numbers; `total` = `occupied.length` | `coordinates/component.ts:277-310`, `:57-80`, `db.ts:887-901` | no route | MISSING |
| database failure | 500 | `{"ok":false,"message":<error message or "Failed to get world manifest">}` | `get-world-manifest.ts:48-57` | no route | MISSING |

**Data dependency:** the worlds store: scenes and their parcels per world and the world's
`spawn_coordinates` (`services/catalyst/worlds/src/queries.rs:12-30`), which dcl-one holds.

## 9. POST /profiles

**Request:** as `/profiles/metadata` (section 10): `ids` must be an array of non-empty strings, may be
empty, no cap.

| trigger | status | body | upstream | dcl-one read-through | verdict |
|---|---|---|---|---|---|
| found | 200 | `[{"timestamp":<number>,"avatars":[…]}]`; each avatar keeps the stored keys and order; `avatar.snapshots` is replaced by `{"face256":"<PROFILE_IMAGES_URL>/entities/<entityId>/face.png","body":"…/body.png"}` ([`AB-2-one-known.json`](captures/abr/AB-2-one-known.json)). `timestamp` is a number on a cache hit or a Catalyst fallback and the raw Postgres `bigint` (a string) when the profile was read from ABR's database in that request | `get-profiles.ts:11-22`, `profile-sanitizer.ts:118-152`, `profile-retriever.ts:32-54`, `src/migrations/1763329911778_profiles.ts:10` | ABR's documents, cached 60 s per address `registry.rs:364-442` | MATCH for profiles ABR holds |
| profile deployed to this node only | 200 | ABR hydrates a miss from a Catalyst and persists it (`profile-retriever.ts:110-124`) | — | absent; an `island` node answers `[]` | GAP |
| errors | | as section 10 | | as section 10 | DIVERGENT |

**Data dependency:** the node's profiles (`/lambdas/profiles` already builds them:
`services/catalyst/shop/src/market/profiles.rs:665-724`).

## 10. POST /profiles/metadata

**Request** (`decentraland/asset-bundle-registry:src/controllers/schemas/profiles.ts:9-29`):
- Body read with `request.json()` regardless of `Content-Type`.
- `ids` must be an array whose every element is a non-empty string; an empty array passes. No upper
  bound ("deliberately not capped", `:5-7`). Body limit: `@dcl/http-server`, not vendored; 1001 ids
  (about 45 KB) answered `200`.
- No auth, no rate limit.
- Ids are lowercased and deduplicated before the lookup (`profile-retriever.ts:77`); not trimmed.

**Lookup** (`decentraland/asset-bundle-registry:src/logic/profile-retriever.ts:75-148`): L1 cache
(lowercased keys), then `profiles` table (`pointer = ANY(lowercased)`), then a Catalyst
`/lambdas/profiles` fallback whose results are persisted; profiles for pointers not asked for are
dropped. Database and Catalyst failures are logged and read as "not found": the answer stays `200`.

**Projection** (`decentraland/asset-bundle-registry:src/logic/sync/profile-sanitizer.ts:107-116`,
type `src/types/types.ts:35-41`): from `avatars[0]`:

```json
{"pointer":"0x…0b00","hasClaimedName":false,"name":"profile-name-1","nameColor":{"r":1,"g":0.5,"b":0},"thumbnailUrl":"https://profile-images.decentraland.org/entities/<entity-id>/face.png"}
```

- `pointer`: the stored entity pointer (`pointers[0]`), lowercased at ingest.
- `hasClaimedName`, `name`, `nameColor`: copied from the stored avatar. A key whose value is
  `undefined` is dropped by `JSON.stringify`; a stored `null` is sent as `null`.
- `thumbnailUrl`: always `${PROFILE_IMAGES_URL}/entities/<entity id>/face.png`, built from the entity
  id, never from the deployed snapshot.
- Stored profiles are the lamb2-sanitized document (`hasClaimedName` checked against name ownership),
  refreshed by the ownership-validator job every `PROFILES_OWNERSHIP_VALIDATION_INTERVAL_MS` (default
  10 min) (`src/logic/sync/ownership-validator-job.ts:16-24`); `userId`/`ethAddress` are forced to the
  pointer (`profile-sanitizer.ts:48-65`); profiles without avatars are discarded at ingest (`:71-98`).

| trigger | status | body | headers | capture / upstream | dcl-one read-through (`registry.rs:364-453`) | `/lambdas/profiles/metadata` in `decentraland/dcl-one#257` |
|---|---|---|---|---|---|---|
| one known id | 200 | one object, keys `pointer, hasClaimedName, name, thumbnailUrl` (`nameColor` absent when unset) | `content-type: application/json` | `AB-1-one-known.json` | ABR's bytes | own profile; keys in alphabetical order (`hasClaimedName, name, nameColor, pointer, thumbnailUrl`): `serde_json` without `preserve_order` |
| two known ids, warm cache | 200 | request order, both orders kept | | `AB-1-two-known.json`, `AB-1-two-known-reversed.json` | request order | request order |
| order on a cold read | 200 | cache hits first in request order, then database rows in row order (no `ORDER BY`), then Catalyst fallbacks | | `profile-retriever.ts:81-121`, `db.ts:596-613` | request order | request order |
| same address twice, different case | 200 | one element | | `AB-1-case-duplicate.json` | one element (lowercased, deduplicated) | one element |
| unknown id | 200 | `[]` | | `AB-1-unknown.json` | `[]`, remembered 60 s | `[]` |
| `ids: []` | 200 | `[]` | | `AB-3-empty-array.json` | `400 {"error":"ids must be a non-empty array"}` | `400 {"error":"Bad request","message":"No profile ids were specified. Expected ids:string[] in body"}` |
| `{}` (no `ids`) | 400 | `{"error":"Bad request","message":"The ids property must be an array of non-empty strings"}` | `application/json` | `AB-3-missing-ids.json` | `400 {"error":"ids must be a non-empty array"}` | `400`, same envelope, message `No profile ids were specified. Expected ids:string[] in body` |
| `ids` not an array | 400 | same as above | | `AB-3-ids-not-array.json` | axum `422` `text/plain` | `400`, message `Failed to parse the request body as JSON: invalid type: string …` |
| non-string element | 400 | same as above | | `AB-3-non-string-element.json` | axum `422` `text/plain` | `400`, message `Failed to parse the request body as JSON: invalid type: integer …` |
| empty-string element | 400 | same as above | | `AB-3-empty-string-element.json` | trimmed to `""`, forwarded; ABR answers 400, read as `502` | accepted; `""` finds nothing: `200 []` |
| more than 1000 ids | 200 | (1001 unknown ids) `[]` | | `AB-3-1001-ids.json` | `400 {"error":"at most 1000 ids per request"}` | `400 … "Too many profile ids were specified. Maximum allowed is 1000"` |
| body not JSON | 500 | `{"error":"Internal Server Error"}` | | `AB-3-not-json.json` | axum `400` `text/plain` | `400 … "Failed to parse the request body as JSON: …"` |
| empty body | 500 | `{"error":"Internal Server Error"}` | | `AB-3-empty-body.json` | axum `400` `text/plain` | `400 … "Failed to parse …"` |
| `Content-Type: text/plain` | 200 | answered as JSON | | `AB-3-text-plain.json` | axum `415` | accepted (reads raw bytes): MATCH |
| body over the limit | not vendored | | | | axum default 2 MiB | `413` above 256 KiB (about 5,800 addresses) |
| `GET` / `PUT` | 405 | empty, `Allow: POST` | | `AB-1-get.json`, `AB-1-put.json` | axum 405, `Allow: POST` | axum 405 `Allow: POST` (production lamb2 answers `GET` with `404 Profile not found`: `AB-14-peer-get-lambdas-profiles-metadata.json`) |
| a lookup layer fails | 200 | what was found | | `profile-retriever.ts:50-53`, `:69-72` | `502` | `500` (database) or `503 ownership readers unavailable` (`services/catalyst/shop/src/market/profiles.rs:509-524`) |

**Data dependency:** the node's own profiles (content DB) and the name-ownership reader that sets
`hasClaimedName`; dcl-one holds both for `/lambdas/profiles`.

### `decentraland/dcl-one#257` against ABR, field by field

| aspect | ABR | `profile_metadata` in the diff | verdict |
|---|---|---|---|
| `pointer` | stored `pointers[0]` | `avatars[0].ethAddress` (else `userId`), lowercased; equal to the pointer after `apply_pointer_identity` (`services/catalyst/shop/src/market/profiles.rs:322-329`) | MATCH for address pointers |
| `hasClaimedName` | stored value; key dropped if absent | value, default `false` | DIVERGENT only for a profile without the key |
| `name` | stored value; key dropped if absent | value, default `""` | DIVERGENT only for a profile without the key |
| `nameColor` | present when the key exists, `null` kept | present only when non-null | DIVERGENT for a stored `null` |
| `thumbnailUrl` | `${PROFILE_IMAGES_URL}/entities/<id>/face.png` | the processed `avatar.snapshots.face256`: `<PROFILE_CDN_BASE_URL>/entities/<id>/face.png`, else `<profile-images>/entities/<id>/face.png`; a deployed absolute `face256` on another host is kept verbatim; an avatar without an `avatar` object gives `""` (`rewrite_snapshot_urls`, `services/catalyst/shop/src/market/profiles.rs:204-268`) | MATCH for the common case; DIVERGENT in the two edge cases |
| key order | `pointer, hasClaimedName, name, nameColor, thumbnailUrl` | alphabetical | DIVERGENT (P2) |
| order | see the table above | request order | compatible |
| dedup | case-insensitive | case-insensitive | MATCH |
| validation and errors | see the table above | lamb2's `/profiles` rules | DIVERGENT |
| GET | 405 | 405 | MATCH (ABR) |
| path | `asset-bundle-registry.<domain>/profiles/metadata` | `/lambdas/profiles/metadata` only; the ABR-host path stays the read-through | GAP |

---

## 11. GET /denylist

| trigger | status | body | upstream | dcl-one | verdict |
|---|---|---|---|---|---|
| always | 200 | array ordered by `created_at` descending; each entry `{"entity_id":string,"reason":string\|null,"created_by":string,"created_at":string,"updated_at":string}` (millisecond `bigint`s arrive as strings); `entity_id` and `created_by` lowercased at insert ([`AB-9-denylist.json`](captures/abr/AB-9-denylist.json): 16 entries) | `get-denylist.ts:3-13`, `db.ts:1456-1481`, `src/types/types.ts:140-148` | no route; `content.denylist` holds `id`, `added_by`, `added_at` (`services/catalyst/content/src/denylist.rs:47`, `:256`) | MISSING |

No `Content-Type` is set by the handler; the captured answer is `application/json`.

**Data dependency:** the node's `content.denylist`. Decentraland's moderation list itself is in ABR's
database; dcl-one reads it through `DENYLIST_URLS` (AB-10).

## 12. Admin and build-queue routes (outside catalyst parity)

- `GET /queues/status`: `{"windowsPendingJobs":[ids],"macPendingJobs":[ids],"webglPendingJobs":[]}`
  (`get-entity-status.ts:103-128`; [`AB-16-queues-status.json`](captures/abr/AB-16-queues-status.json):
  1466 ids each).
- `POST`/`DELETE /denylist/:entityId`: signed fetch by a moderator in the `user_moderators` feature
  list; `201` with the stored entry, `403 {"ok":false,"message":"Forbidden: signer is not an authorized moderator"}`
  (`post-denylist-entry.ts:4-41`). Not probed (writes).
- `POST /registry`, `DELETE /flush-cache`: bearer `API_ADMIN_TOKEN`. Not probed.

In dcl-one the operator console reaches the external abgen runtime for these
(`services/catalyst/config/src/ops.rs:124-133`), and the gateway aliases the root paths `/registry`,
`/denylist`, `/queues/`, `/flush-cache` to `/asset-bundle-registry$uri`, where no route answers
(`infra/gateway/root-aliases.nix:488-523`).

---

## 13. Backlog

1. **AB-1** `POST /profiles/metadata` (ABR host and `/lambdas/profiles/metadata`). The ABR-host route
   reads only Decentraland's registry, so a profile deployed to this node is absent and an `island`
   node answers `[]`; `decentraland/dcl-one#257` serves the node's profiles at `/lambdas/profiles/metadata`
   only. Expected: the ABR contract (section 10, `decentraland/asset-bundle-registry:src/controllers/handlers/get-profiles-metadata.ts:11-21`,
   `src/logic/sync/profile-sanitizer.ts:107-116`) from the node's own profiles at both paths; the
   read-through can stay as the source for profiles this node does not hold. Change:
   `services/catalyst/explore/src/api/registry.rs:448-453` (route to the profiles handler or merge),
   `services/catalyst/shop/src/market/profiles.rs`. Severity: P1. Review items for
   `decentraland/dcl-one#257`:
   1. `ids: []` answers `400`; ABR answers `200 []` ([`AB-3-empty-array.json`](captures/abr/AB-3-empty-array.json)). The new test `profiles_metadata_rejects_missing_and_too_many_ids` pins the 400 for `{}` with lamb2's message.
   2. More than 1000 ids answers `400`; ABR has no cap (`schemas/profiles.ts:5-7`, [`AB-3-1001-ids.json`](captures/abr/AB-3-1001-ids.json)). The 256 KiB body limit adds a `413` from about 5,800 addresses.
   3. `{}`, `ids` not an array and a non-string element answer `400` with lamb2 or serde messages; ABR answers `400 {"error":"Bad request","message":"The ids property must be an array of non-empty strings"}` for all three (same envelope as `bad_request`, `services/catalyst/shared/src/http_util.rs:14-16`).
   4. An empty-string element is accepted; ABR rejects it with the 400 above ([`AB-3-empty-string-element.json`](captures/abr/AB-3-empty-string-element.json)).
   5. A body that is not JSON, or empty, answers `400`; ABR answers `500 {"error":"Internal Server Error"}` ([`AB-3-not-json.json`](captures/abr/AB-3-not-json.json), [`AB-3-empty-body.json`](captures/abr/AB-3-empty-body.json)). Matching it means answering a client error with 500; record which one dcl-one keeps.
   6. Keys serialize alphabetically; ABR's order is `pointer, hasClaimedName, name, nameColor, thumbnailUrl`. Building the object in that order needs an ordered map (the workspace keeps `preserve_order` off, `services/catalyst/Cargo.toml:21-23`).
   7. `thumbnailUrl` is read from the processed snapshot. ABR always builds `<PROFILE_IMAGES_URL>/entities/<entity id>/face.png` from the entity id; building it from the entity id removes the two edge cases (a foreign absolute `face256`, an avatar without `avatar`).
   8. Missing `hasClaimedName`/`name` default to `false`/`""` where ABR drops the key; a stored `nameColor: null` is dropped where ABR sends `null`.
   9. A database or ownership-reader failure answers `500`/`503`; ABR logs and answers `200` with what it found.
   10. Only `/lambdas/profiles/metadata` is added; `asset-bundle-registry.<domain>/profiles/metadata` and `/asset-bundle-registry/profiles/metadata` still read Decentraland's registry, which the explorer calls on a Decentraland realm.
   11. `GET /lambdas/profiles/metadata` becomes `405` where production lamb2 answers `404 {"error":"Not Found","message":"Profile not found"}` ([`AB-14-peer-get-lambdas-profiles-metadata.json`](captures/abr/AB-14-peer-get-lambdas-profiles-metadata.json)); `POST` is a dcl-one extension at a lamb2 path (production answers `405 Allow: HEAD, GET`). State both in the description.
   12. CORS on `/lambdas/*` is the market router's (origin echoed with credentials); ABR answers `*` with `max-age: 86400` ([`AB-12-preflight.json`](captures/abr/AB-12-preflight.json)).
2. **AB-2** `POST /profiles` (ABR host). Same gap as AB-1 for the full document: expected
   `[{"timestamp":number,"avatars":[…]}]` with snapshots rewritten to
   `<PROFILE_IMAGES_URL>/entities/<id>/{face,body}.png` (`decentraland/asset-bundle-registry:src/logic/sync/profile-sanitizer.ts:118-152`)
   from the node's profiles. Change: `services/catalyst/explore/src/api/registry.rs:444-446`. The module
   comment says ABR's `/profiles` has a string `timestamp` (`registry.rs:9`); the captured one is a
   number, and a string only on a database-layer read. Severity: P1.
3. **AB-3** profile request validation on the ABR host (both profile routes). Expected: the rules of
   `decentraland/asset-bundle-registry:src/controllers/schemas/profiles.ts:9-29` and the captured
   statuses (section 10): empty array `200 []`, no cap, 400 with the http-commons envelope and the one
   message, `500` for unparseable bodies, `Content-Type` ignored, no trimming. Change:
   `services/catalyst/explore/src/api/registry.rs:364-376`, `:444-453` (take raw bytes, not `Json`).
   Severity: P1.
4. **AB-4** `POST /entities/active` content part. Expected: `pointers` only (`ids` → `400
   {"ok":false,"message":"No pointers provided"}`), no cap, `world_name` filter, entities with
   `timestamp` as a string and `deployer` (`decentraland/asset-bundle-registry:src/controllers/handlers/get-active-entities.ts:10-48`);
   the content fields of an entity this node holds and ABR does not must come from the node, with the
   AB fields left to the read-through (what to answer for an entity no registry has converted is an
   owner decision: ABR answers nothing for it today). Change: `services/catalyst/explore/src/api/registry.rs:106-132`, `:191-299`.
   Severity: P1.
5. **AB-5** `POST /entities/active` and `/entities/versions` read-through ignore `content.denylist`.
   Expected: an id on the node's denylist is not served (ABR excludes its own list,
   `decentraland/asset-bundle-registry:src/adapters/db.ts:105-111`). Change: `registry.rs:281-298`,
   using `services/catalyst/content/src/queries.rs:78`'s projection. Severity: P1.
6. **AB-6** `GET /entities/status/:id` missing. Expected: section 6
   (`decentraland/asset-bundle-registry:src/controllers/handlers/get-entity-status.ts:5-66`), `entityId`
   and `catalyst` from the node's content (current id, overwritten id, newest entity on a pointer,
   `world_name`), AB fields from the read-through or `pending`, 404 body `{"ok":false,"message":"No active entity found for the provided id or pointer"}`.
   Change: `services/catalyst/explore/src/api/registry.rs:80-88`. Severity: P1.
7. **AB-7** `GET /entities/status` (signed fetch) missing. Expected: section 7
   (`get-entity-status.ts:68-101`, `routes.ts:22-38`): signer's current and overwritten entities, the
   captured unsigned 400 body. Change: `registry.rs:80-88` with the node's signed-fetch verifier.
   Severity: P2 (creator tooling).
8. **AB-8** `GET /worlds/:worldName/manifest` missing. Expected: section 8
   (`decentraland/asset-bundle-registry:src/controllers/handlers/get-world-manifest.ts:13-59`,
   `src/logic/coordinates/component.ts:277-310`) from the worlds store: occupied parcels of the world's
   non-denylisted scenes, stored spawn when inside the bounds else the parcel nearest the centroid,
   numeric `x`/`y`, `total`; unknown world `200 {"occupied":[],"spawn_coordinate":{"x":0,"y":0},"total":0}`;
   invalid name 400. Change: `registry.rs:80-88` reading `services/catalyst/worlds/src/queries.rs`.
   Severity: P1.
9. **AB-9** `GET /denylist` missing. Expected: section 11 (`decentraland/asset-bundle-registry:src/controllers/handlers/get-denylist.ts:3-13`):
   the node's list as `[{entity_id,reason,created_by,created_at,updated_at}]`, timestamps as strings,
   newest first. Change: `registry.rs:80-88` over `services/catalyst/content/src/denylist.rs`; the
   gateway's root `/denylist` alias already points at `/asset-bundle-registry/denylist`
   (`infra/gateway/root-aliases.nix:497-504`). Severity: P1.
10. **AB-10** Status: MATCH, no work. `DENYLIST_URLS` (upstream's name) is fetched through the outbound
    guard at boot and every 120 s (`services/catalyst/content/src/denylist.rs` `reload_remote`,
    `services/catalyst/src/serve.rs`). `NODE_POSTURE=mirror` defaults it to
    `https://asset-bundle-registry.decentraland.org/denylist`; island and journey leave it empty
    (`services/catalyst/config/src/posture.rs`). The ids feed the same projection as `content.denylist`
    (`services/catalyst/storage/src/denylist.rs`), so `/contents`, the active-entity reads, `/deployments`,
    `/pointer-changes`, `/available-content`, erc721 and the thumbnails and images hide them; audit stays
    unfiltered and snapshots do not filter, as upstream. A failed or garbage fetch keeps the URL's last
    good list and never the operator's. Differences from upstream: an id removed from the remote list is
    lifted on the next good fetch (upstream: at restart), a non-2xx answer counts as a failure, and a local
    deploy naming a listed entity id or content hash is refused (upstream stores it hidden).
11. **AB-11** `POST /entities/versions` error bodies and selector: `{"ok":false,"message":"No pointers provided"}`,
    `pointers` only, no cap (`get-entity-versions.ts:10-21`). Change: `registry.rs:303-336`. Severity: P2.
12. **AB-12** CORS on the ABR host: expected the captured preflight (`allow-origin: *`, `allow-methods:
    GET,HEAD,OPTIONS,DELETE,POST,PUT`, reflected `allow-headers`, `max-age: 86400`). Change:
    `services/catalyst/explore/src/api/mod.rs:741`. Severity: P2.
13. **AB-13** error envelopes on the ABR host: `{"ok":false,"message":…}` from handlers, the
    http-commons `{"error":"Bad request","message":…}` for invalid requests,
    `500 {"error":"Internal Server Error"}` for unhandled errors, `Content-Type` ignored (section 2,
    cross-cutting table). Change: `services/catalyst/explore/src/api/mod.rs:770-772`, `registry.rs`.
    Severity: P2.
14. **AB-14** production routing. No production or zone Catalyst URL redirects or proxies to ABR
    (section 1). Status: MATCH, no work: a Catalyst replacement needs no ABR path under `/content`,
    `/lambdas` or the root to match production; `POST /lambdas/profiles/metadata` is a dcl-one
    extension (AB-1 item 11).
15. **AB-15** `GET /status`. Status: MATCH, no work.
16. **AB-16** `GET /queues/status`, denylist writes, `/registry`, `/flush-cache`: ABR administration and
    build state, outside catalyst parity. Status: out of scope, no work.

## 14. Routes dcl-one serves here that ABR does not

- `GET /asset-bundle-registry/health` → `{"status":"ok"}` (`services/catalyst/explore/src/api/registry.rs:455-457`).
- `POST /entities/active` and `/entities/versions` with `{"ids":[…]}` (forwarded; ABR answers 400).
- Every route again under the `/asset-bundle-registry` prefix on the main host; production answers that
  prefix with the gateway 404 ([`AB-14-peer-get-abr-prefix-status.json`](captures/abr/AB-14-peer-get-abr-prefix-status.json)).
