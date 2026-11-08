# Worlds content server: content reads, status and admin

Area: the read and operator routes of the worlds content server outside deploys, permissions,
settings and comms: `GET`/`HEAD /contents/:hashId`, `GET`/`HEAD /ipfs/:hashId`,
`GET /available-content`, `POST /entities/active`, `GET /index` (deprecated), `GET /status`,
`POST /reprocess-ab` and `POST /gc`. Also the cross-cutting rules: the unknown-route 404 and the
405, the `errorHandler` envelope of `@dcl/http-commons`, CORS, and how this tree's root and
`/worlds-content-server` mounts meet the catalyst content routes of the same name.

Pinned revisions:

- Upstream: `decentraland/worlds-content-server` @ `10af5255bb04a1a28e0c26bd190317c26a6015a9` (2026-10-21).
- Upstream packages read from their published tarballs (`npm pack`, versions from `yarn.lock`):
  `@dcl/http-commons@2.0.0`, `@dcl/http-server@2.3.0`, `@dcl/schema-validator-component@1.0.1`,
  `@dcl/schemas@27.1.0`. `file-type@16` and `@dcl/catalyst-storage` were not read; rows that depend
  on them say so.
- dcl-one: origin main `2da8b8aea`, Rust workspace `services/catalyst`.

Everything below is from reading the code. Nothing was built, run or captured, and no line is
"verified".

Files read:

- Upstream worlds-content-server:
  - `src/controllers/routes.ts`, `src/service.ts`, `src/components.ts` (server and CORS options)
  - `src/controllers/handlers/{content-file-handler,active-entities,index-handler,status-handler,reprocess-ab-handler,garbage-collection}.ts`
  - `src/controllers/schemas/reprocess-ab-schemas.ts`
  - `src/adapters/worlds-manager.ts` (`getRawWorldRecords`, `getDeployedWorldCount`, `getEntityForWorlds`, `getWorldScenes`)
  - `src/adapters/worlds-indexer.ts`, `src/adapters/comms-adapter.ts`, `src/adapters/content-references.ts`
  - `src/adapters/content-locks/errors.ts`, `src/logic/content-gc/{component,types}.ts`
  - `src/adapters/name-deny-list-checker.ts`
  - `.env.default`
  - tests: `test/integration/{get-endpoints,active-entities-handler,content-references,index-handler,garbage-collection,reprocess-ab-handler,startup}.spec.ts`,
    `test/unit/{content-file-handler,garbage-collection-handler,routes}.spec.ts`
- Upstream packages: `@dcl/http-commons` `src/controllers/handlers/{error-handler,bearer-token-middleware}.ts`,
  `src/errors.ts`; `@dcl/http-server` `dist/{cors,logic,router,server,status-checks}.js`;
  `@dcl/schema-validator-component` `dist/src/component.js`; `@dcl/schemas` `dist/misc/index.js` (`IPFSv2`).
- dcl-one (`services/catalyst/`):
  - `worlds/src/lib.rs` (`router`, `app_router`)
  - `worlds/src/core/mod.rs` (`router`, `root_router`, route tables)
  - `worlds/src/core/{active,admin,auth,http,names,store,partial}.rs`, `worlds/src/writes.rs`
  - `worlds/src/{index,status,available_content,queries,about}.rs`
  - `comms/src/worlds_envelope.rs`, `comms/src/world_identity.rs`
  - `content/src/{get_content,formatters,available_content,errors,routing,status}.rs`, `content/src/lib.rs` (`content_routes`, `build_router`)
  - `shared/src/{cors,health,query_params}.rs`
  - `storage/src/gc.rs` (reference list)
  - `src/serve.rs` (app composition), `../../library/dcl-auth/src/{admin_bearer,hash}.rs`
  - `../../infra/gateway/gateway.py`, `../../infra/gateway/smoke.py`
  - `docs/worlds-compatibility.md`
  - tests: `worlds/src/core/tests.rs` (`available_content_under_the_worlds_prefix_reports_deployed_files`,
    `worlds_contents_sniff_and_answer_legacy_sha256_keys`, the admin and `/gc` cases near line 4240),
    `content/src/get_content.rs` tests, `src/cross_tests/served_routes_tests.rs`.

## Common notes

- **Mounts.** Upstream serves every route at the root of its own host
  (`https://worlds-content-server.decentraland.org`). dcl-one composes one app
  (`src/serve.rs:1311-1372`):
  - `content::build_router` first, which owns `/contents/{hashId}`, `/available-content`,
    `POST /entities/active` and `/status` at the root and under `/content`
    (`content/src/lib.rs:741-813`, `817-856`);
  - `worlds::app_router` (`worlds/src/lib.rs:388-439`): the worlds reads (`/world/{n}/about`,
    `/world/{n}/scenes`, `/index`, `/worlds/status`) and `core::root_router` at the root, and the
    full worlds surface nested under `/worlds-content-server` (`/status`, `/contents/{hashId}`,
    `/ipfs/{hashId}`, `/available-content`, `/openapi.json`, `/health/*`, and `core::router`, which
    adds `/entities`, `/entities/active`, `/entities/{name}`, `/gc` and `/admin/gc`)
    (`worlds/src/core/mod.rs:582-603`).
  - At the root, `/contents`, `/available-content`, `/entities/active` and `/status` are the
    catalyst content server's; `/ipfs/{hashId}` and `/gc` do not exist at the root; `/index` is the
    worlds index.
- **How a worlds client reaches the worlds mount.** The gateway maps the subdomain
  `worlds-content-server.<domain>` onto the `/worlds-content-server` prefix
  (`infra/gateway/gateway.py:280-289`; `smoke.py:247` drives `/contents/test` on that vhost), so a
  client pointed at that host gets the worlds handlers for every route in this file. A client
  pointed at the node's root host gets the catalyst routes for the four colliding paths. Without the
  gateway, only the path prefix works.
- **Links this tree builds.** The worlds `Settings.public_url` is the node's `PUBLIC_URL`
  (`worlds/src/lib.rs:134-139`). `/about` (`scenesUrn` `baseUrl`, skybox textures, thumbnail,
  `worlds/src/about.rs:182-188,223,259`) and `/index` (`thumbnail`, `worlds/src/index.rs:135,188`)
  link `<PUBLIC_URL>/contents/<hash>`, the root catalyst handler, not the worlds mount. Upstream
  links `HTTP_BASE_URL` (its own host) and so its own handler (WR-12).
- **Error envelope.** Upstream installs `errorHandler` first (`src/controllers/routes.ts:168`):
  `InvalidRequestError` → 400 `{"error":"Bad request","message":…}`, `NotFoundError` → 404
  `{"error":"Not Found",…}`, `NotAuthorizedError` → 401 `{"error":"Not Authorized",…}`, anything
  else → 500 `{"error":"Internal Server Error"}` (`@dcl/http-commons`
  `src/controllers/handlers/error-handler.ts`). The worlds routes here answer the same labels
  (`comms/src/worlds_envelope.rs:120-140`). The routes this tree serves from the content crate under
  the prefix (`/contents`, `/ipfs`, `/available-content` after its own checks, `/status`, `/index`)
  answer the content envelope `{"error": message}` (`content/src/errors.rs:68-76`); a 500 is
  `{"error":"Internal Server Error"}` in both.
- **Unknown route, 405.** Upstream: `router.middleware()` then `router.allowedMethods()`
  (`src/service.ts:13-17`). An unmatched path answers `@dcl/http-server`'s default 404 `Not found`,
  `text/plain; charset=utf-8` (`dist/logic.js:315-317`, `respondString`); a matched path with
  another method answers 405 with `Allow: <methods joined by ", ">` and no body
  (`dist/router.js:205-265`). dcl-one: the content router's fallback answers 404 `Not found`
  `text/plain; charset=utf-8` for every path no router claims, `/worlds-content-server/*`
  included (`content/src/lib.rs:728-735,841`). A 405 is axum's, `Allow` spelled `GET,HEAD`;
  `content::routing::upstream_allow` rewrites it to upstream's spelling under `/content/` only
  (`content/src/routing.rs:140-170`) (WR-26).
- **CORS.** Upstream configures `@dcl/http-server` with `{methods: [GET,HEAD,OPTIONS,DELETE,POST,PUT],
  maxAge: 86400}` (`src/components.ts:96-107`): any `OPTIONS`, on any path, answers 204 before
  routing with `Access-Control-Allow-Origin: *`, the methods, the requested headers reflected (none
  sent, none answered) and `Access-Control-Max-Age: 86400`; an actual response to a request with
  `Origin` gets `Access-Control-Allow-Origin: *`, no credentials, and no expose list beyond what a
  handler sets (`dist/cors.js:51-160`). dcl-one's worlds routers use `shared::cors`: the origin
  reflected with `Access-Control-Allow-Credentials: true` and `Vary: Origin`, methods
  `GET,HEAD,POST,PUT,DELETE,PATCH,OPTIONS`, the requested headers reflected or a fixed list, max-age
  86400, `Access-Control-Expose-Headers: *` when the handler set none, and `X-Content-Type-Options:
  nosniff` (`shared/src/cors.rs:15-85`). A layer reaches only the routes added before it
  (axum `Router::layer`), and `/worlds-content-server/{status,contents,ipfs,openapi.json}` are added
  after `router()` applied its layers (`worlds/src/lib.rs:394-422`; `/available-content` and
  `/health/*` carry their own), so those four answer with no CORS headers at all (WR-06).
- **Rate limits.** Upstream applies none to the routes in this file. This tree's gateway limits
  `/worlds-content-server/(contents|ipfs)/` with `zone=content` (100 r/s, burst 1000) and every other
  path with the server-wide `zone=read` (30 r/s, burst 60), answering nginx's HTML 429
  (`infra/gateway/gateway.py:194,367`). The root `/contents/{hashId}` also runs the content server's
  GET quota (`content/src/lib.rs:759-765`, `content_get_limit`).
- **Writes switch.** `core::router` sits behind `worlds::writes::guard`; with
  `WORLDS_WRITES_ENABLED=false` (the default) `POST /gc` answers 503 `{ok:false, error:"Service
  Unavailable", message}` with `Retry-After: 3600`; `POST /entities/active` stays open
  (`worlds/src/writes.rs:30-75`).
- **Service startup** (`test/integration/startup.spec.ts`: database, then migrations, before any
  component) is not a wire contract; dcl-one applies its schema with `catalyst schema migrate`. No row.

## Endpoint inventory

| # | method | upstream path | dcl-one route(s) | status |
|---|---|---|---|---|
| 1 | GET | `/contents/:hashId` | `/worlds-content-server/contents/{hashId}` → `content/src/get_content.rs:147` (`get_world_content`; route `worlds/src/lib.rs:399-404`); root `/contents/{hashId}` is the catalyst handler (`get_content.rs:133`) | DIVERGENT |
| 2 | HEAD | `/contents/:hashId` | same handler | DIVERGENT |
| 3 | GET | `/ipfs/:hashId` | `/worlds-content-server/ipfs/{hashId}` → same handler (`worlds/src/lib.rs:413-418`); no root route | DIVERGENT |
| 4 | HEAD | `/ipfs/:hashId` | same handler | DIVERGENT |
| 5 | GET | `/available-content` | `/worlds-content-server/available-content` → `worlds/src/available_content.rs:31` then `content/src/available_content.rs:14`; root is the catalyst handler | DIVERGENT |
| 6 | POST | `/entities/active` | `/worlds-content-server/entities/active` → `worlds/src/core/active.rs:19` (route `core/mod.rs:623-626`); root is the catalyst handler | DIVERGENT |
| 7 | GET | `/index` | `/index` and `/worlds-content-server/index` → `worlds/src/index.rs:87` | GAP |
| 8 | GET | `/status` | `/worlds-content-server/status` and root `/worlds/status` → `worlds/src/status.rs:146`; root `/status` is the catalyst status | DIVERGENT |
| 9 | POST | `/reprocess-ab` | none | MISSING |
| 10 | POST | `/gc` | `/worlds-content-server/gc` and `/worlds-content-server/admin/gc` → `worlds/src/core/admin.rs:90`; no root route | DIVERGENT |

Counts: 10 endpoints. MATCH 0, GAP 1, DIVERGENT 8, MISSING 1. The cross-cutting rules (404/405,
error envelope, CORS, mounts) are in "Common notes" and the backlog.

---

## 1–4. GET/HEAD /contents/:hashId and /ipfs/:hashId

Upstream handlers: `getContentFile` and `headContentFile`
(`src/controllers/handlers/content-file-handler.ts:123-211`), the same pair at both paths
(`src/controllers/routes.ts:245-250`). dcl-one: `get_world_content` → `serve` with the `WORLDS`
mount (`content/src/get_content.rs:95-132,147-281`) at both paths under the prefix.

### Request

| aspect | upstream | dcl-one |
|---|---|---|
| Key format | `IPFSv2.validate` (`^(ba)[a-zA-Z0-9]{57}$`, `@dcl/schemas`) or a lowercase 64-hex SHA-256 (`content-file-handler.ts:47-51`). Anything else → **400**, no body. `Qm…` CIDv0 → 400. | `is_canonical_cid` (CIDv0 `Qm…` of 46, or `ba` + base32 lowercase, ≥58) or, on this mount, a lowercase 64-hex SHA-256 read as the raw-leaf CIDv1 of that digest (`get_content.rs:163-174`, `library/dcl-auth/src/hash.rs:176-204`). Anything else → **404** (intended, `docs/worlds-compatibility.md` "Legacy content keys"). CIDv0 keys are served (WR-09). |
| SHA-256 key | The blob stored under that literal key. | The blob whose raw-leaf CIDv1 has that digest, so a file of one chunk (256 KiB) or less (intended, same doc entry). Pinned by `worlds_contents_sniff_and_answer_legacy_sha256_keys` (`worlds/src/core/tests.rs:5405`). |
| `Range` (GET) | `parseRangeHeader` (`content-file-handler.ts:85-111`): `bytes=S-E`, `bytes=S-`, `bytes=-N` by regex `^bytes=(\d*)-(\d*)$`; multi-range, other units, `bytes=-` → unsupported → full 200; `-0`, start ≥ size, end < start → 416 `Content-Range: bytes */size`. Compressed or unknown-size files ignore the range. | `parse_range_header` (`content/src/formatters.rs:41-83`): the same outcomes for the cases upstream tests (`test/unit/content-file-handler.spec.ts:12-186`, `test/integration/get-endpoints.spec.ts:63-229`). Edge differences in WR-08. |
| `Range` (HEAD) | Ignored: `headContentFile` answers 200 with the full headers (`content-file-handler.ts:200-211`). | Honoured: 206 with the window's `Content-Length` and `Content-Range` (`get_content.rs:209-237`) (WR-07). |
| `If-None-Match` | Ignored. | A match answers 304 with `ETag`, `Cache-Control`, `Access-Control-Expose-Headers: ETag` (`get_content.rs:180-184`, `formatters.rs:9-33`) (WR-10). |
| Denylist | None. | The content denylist answers 404 (`get_content.rs:175-177`) (WR-09). |
| Missing blob | 404. | 404, unless the content read-through is on and fills it from the sync sources first (`get_content.rs:186-197`) (WR-09). |

### Responses

| case | upstream | dcl-one |
|---|---|---|
| 200 GET | `Content-Type` sniffed by `file-type` `fromStream` on the raw stream (4100-byte sample) else `application/octet-stream`; `ETag: "<hashId>"`; `Access-Control-Expose-Headers: ETag, Accept-Ranges, Content-Range`; `Cache-Control: public,max-age=31536000,s-maxage=31536000,immutable`; `Accept-Ranges: bytes` (or `Content-Encoding` when compressed); `Content-Length` when known (`content-file-handler.ts:27-69`). | `content_file_headers` (`formatters.rs:85-102`): `Content-Type` sniffed from the first 32 bytes by `detect_content_type` plus a JSON check (`get_content.rs:24-66,105-118`); `ETag: "<hashId>"` (the client's spelling, also for a SHA-256 key); `Access-Control-Expose-Headers: ETag, Content-Range, Accept-Ranges, Content-Length`; the same `Cache-Control`; `Accept-Ranges: bytes`; `Content-Length`. Sniffing differs (WR-04). |
| 200 HEAD | Same headers, no body (`get-endpoints.spec.ts:27-40`). | Same headers, sniffed, no body (`get_content.rs:248-261`). MATCH. |
| 206 | The sniff reads the file's first 4100 bytes (a failed read falls back to octet-stream); `Content-Length` of the window; `Content-Range: bytes S-E/size` (`content-file-handler.ts:176-197`). | The sniff reads bytes 0–31 of the file; same `Content-Length` and `Content-Range` (`get_content.rs:209-237`). MATCH except the sniffer (WR-04). |
| 416 | `Content-Range: bytes */size` only. | `Content-Range: bytes */size` and `Access-Control-Expose-Headers: Content-Range` (`get_content.rs:198-208`). MATCH (one extra header). |
| 400 | Invalid key: empty body. | Not answered: an invalid key is a 404 (WR-02). |
| 404 | Missing: empty body. | `{"error":"No content found with hash <key>"}` (`get_content.rs:80-82`, `content/src/errors.rs:74-76`) (WR-03). |
| 500 | A storage error other than `RangeError` rethrows → `{"error":"Internal Server Error"}` (`content-file-handler.ts:163-173`). | A storage fault → 500 `{"error":"Internal Server Error"}` (`content/src/errors.rs:56-61`). MATCH. |
| Full GET with `STORAGE_X_ACCEL_BASE` | n/a | The app answers 200 with `X-Accel-Redirect` and nginx serves the file, adding its own `Cache-Control: public, max-age=31536000, immutable` and `Access-Control-Allow-Origin: *` (`get_content.rs:263-275`, `infra/gateway/gateway.py:221-229`). Which `Cache-Control` the client sees needs a capture (WR-25). |
| CORS | `Access-Control-Allow-Origin: *` on a request with `Origin`. | None from the app (WR-06); behind the gateway, its fallback adds `*` (`gateway.py:262-265,352`). |

### Data dependency

Both trees read one hash-addressed store. Upstream's holds only world files; dcl-one's holds every
domain's files (`docs/worlds-compatibility.md` "One shared blob store"), so a worlds-mount read can
return any blob this node holds. The entity file of a scene (`/contents/<entityId>`,
`get-endpoints.spec.ts:20-24`) is among the files a deploy stores (`worlds/src/core/deploy.rs:899-905`);
there is no `<entityId>.auth` blob (intended, `worlds-compatibility.md` "No `<entityId>.auth` blob").

---

## 5. GET /available-content

Upstream: `availableContentHandler` (`content-file-handler.ts:213-242`). dcl-one: the worlds checks
(`worlds/src/available_content.rs:31-50`), then the content handler
(`content/src/available_content.rs:14-49`).

| aspect | upstream | dcl-one |
|---|---|---|
| Parsing | `URLSearchParams.getAll('cid')`: only the literal key `cid`. | `parse_query_string` + `qs_get_array` (`shared/src/query_params.rs:16-38`): `cid`, `cid[]` and `cid[0]` all count; the first 2000 pairs are read (WR-24). |
| No cid | 400 `{"error":"Bad request","message":"At least one cid query parameter is required."}` | Same (`available_content.rs:36-38`; test `available_content_under_the_worlds_prefix_reports_deployed_files`). MATCH. |
| > 500 cids | 400 `Too many cid query parameters. Maximum allowed is 500.` | Same (`available_content.rs:39-43`). MATCH. |
| Invalid cid | Any cid failing `IPFSv2.validate` (an empty value included) → 400 `Invalid cid format.` | `is_ipfs_v2`, the same regex (`available_content.rs:18-21,44-46`). MATCH. |
| Body | `[{cid, available}]` from `storage.existMultiple(cids)`, a `Map`, so a repeated cid is listed once (`test/unit/content-file-handler.spec.ts:651-668`). Order: the map's (`@dcl/catalyst-storage`, not read). | `[{cid, available}]`, first occurrence order, repeats dropped (`content/src/available_content.rs:30-46`). A denylisted cid is dropped from the list (WR-11). |
| Storage fault | Throws → 500 `{"error":"Internal Server Error"}`. | 500 `{"error":"Internal Server Error"}`. MATCH. |
| CORS | `*`. | `shared::cors` on this route (`worlds/src/lib.rs:406-412`). See the CORS note. |

The root `/available-content` is the catalyst handler: 400 `{"error":"Please set at least one
cid."}`, a 1000-cid cap and no format check (`content/src/available_content.rs:21-28`). A deploy
client on the root host gets those answers (WR-12 for the mount, WR-23 for the documentation).

---

## 6. POST /entities/active

Upstream: `activeEntitiesHandler` (`src/controllers/handlers/active-entities.ts:7-48`) and
`worldsManager.getEntityForWorlds` (`src/adapters/worlds-manager.ts:822-862`). dcl-one:
`active::active_entities` (`worlds/src/core/active.rs:19-54`) and `store::entities_for_worlds`
(`worlds/src/core/store.rs:1141-1177`).

| aspect | upstream | dcl-one |
|---|---|---|
| Body read | `request.json()`, no content-type check, no body cap (`@dcl/http-server` is built without `maxBodySize`). Invalid or empty JSON throws a `SyntaxError` → 500 `{"error":"Internal Server Error"}`. | Raw bytes, no content-type check, 256 KiB cap (`core/mod.rs:623-626`) under the gateway's 1 MiB (`gateway.py:366`). Invalid JSON → 400 `Invalid request. Request body is not valid` (`active.rs:23-24`) (WR-13, WR-16). |
| Shape | Not an object, or `pointers` not an array → 400 `{"error":"Bad request","message":"Invalid request. Request body is not valid"}` (`active-entities-handler.spec.ts:17-28`). | Same message and envelope (`active.rs:25-29`). MATCH. |
| Pointers | Non-empty strings kept; more than 50 → 400 `Maximum 50 pointers allowed per request`; lowercased and deduplicated (`active-entities.ts:17-29`). | Same filter, same cap and message, deduplicated by lowercase key (`active.rs:31-47`). Names are also trimmed and must parse as a `WorldName` (no inner whitespace, `/` or control character) (`comms/src/world_identity.rs:63-80`) (WR-14). |
| Deny list | `checkNameDenyList`: the name lowercased, first `.eth` and first `.dcl` removed, compared case-insensitively (`src/adapters/name-deny-list-checker.ts`). | The same normalization over the `banned-names` list (`worlds/src/core/names.rs:93-99`; source: the explorer-api mirror instead of `DCL_LISTS_URL`). MATCH. |
| Query | `DISTINCT ON (world_name)` the newest `DEPLOYED` scene, `ORDER BY world_name, created_at DESC`. | The same `DISTINCT ON` over `worlds.world_scenes` (which keeps only live scenes; retired ones move to `worlds.retired_world_scenes`). MATCH. |
| Body | `[{...entity, id, metadata: {...metadata, owner}}]`: the stored entity's keys in their jsonb order, `id` and `metadata` at their places, `owner` last in `metadata` (`worlds-manager.ts:854-861`). | `id` inserted, `metadata.owner` inserted when the world has an owner; `serde_json` without `preserve_order`, so keys come out sorted (`store.rs:1159-1176`) (WR-15). |
| Order | `world_name` order. | `world_name` order. MATCH. |

Upstream tests matched: empty `[]` for an undeployed world, `[]` for empty `pointers`, the 50 cap,
three worlds, a missing world, duplicates and case-insensitive duplicates
(`test/integration/active-entities-handler.spec.ts:4-168`). The root `POST /entities/active` is the
catalyst content server's, which resolves catalyst pointers and answers `[]` for world names
(WR-12).

---

## 7. GET /index (deprecated upstream)

Upstream: `getIndexHandler` (`src/controllers/handlers/index-handler.ts:10-36`) over
`worldsIndexer.getIndex` (`src/adapters/worlds-indexer.ts:19-55`). dcl-one: `index::get_index`
(`worlds/src/index.rs:87-203`) over `queries::list_index_scenes` (`worlds/src/queries.rs:222-259`).

| aspect | upstream | dcl-one |
|---|---|---|
| `limit`, `offset` | `parseInt`; a positive limit is clamped to 10,000, else 10,000; a non-negative offset, else 0. | `js_parse_int`, the same bounds (`index.rs:165-172`, tests `index_limit_clamps_at_max`, `index_offset_parses_and_defaults`). MATCH. |
| Paged set | `getRawWorldRecords({}, {limit, offset})`: every `worlds` row ordered by name, banned names excluded, blocked wallets not excluded; then a world with no deployed scene is skipped, so a page can be shorter than `limit` (`worlds-manager.ts:322-382`, `worlds-indexer.ts:20-32`). | Worlds that have scenes, `blocked_since IS NULL`, ordered by name, paged in SQL (`queries.rs:228-235`). The deny list is not applied; operator-blocked worlds are left out (WR-17, WR-19). |
| Scenes per world | All deployed scenes, newest first. | All scenes, newest first. MATCH. |
| Scene fields | `id`; `title: display.title \|\| ''`; `description: display.description \|\| ''`; `thumbnail: <base>/contents/<hash>` or **absent**; `pointers: parcels`; `runtimeVersion` or **absent**; `timestamp: entity.timestamp \|\| Date.now()` (`worlds-indexer.ts:34-46`). | `title`, `description` `null` when missing; `thumbnail` `null` when none; `runtimeVersion` `null` when missing; `timestamp` `null` when missing (`index.rs:54-67,174-203`; test `scene_summary_runtime_version_is_null_when_absent`) (WR-18). |
| Thumbnail base | `HTTP_BASE_URL`, else the request's origin (`index-handler.ts:15`). | `PUBLIC_URL` (the node root), so the link reaches the catalyst `/contents` (WR-12). |
| `lastUpdated` | ISO time of this request's build. | ISO time of the memoized page's build, up to 30 s old; `ETag` and `If-None-Match` → 304 (`index.rs:18-31,97-131`) (WR-20). |
| Errors | 500 `{"error":"Internal Server Error"}`. | Same (content envelope). MATCH. |

Upstream test `test/integration/index-handler.spec.ts:86` expects a world without a thumbnail to have
`thumbnail` undefined; this tree answers `null`.

---

## 8. GET /status

Upstream: `statusHandler` (`src/controllers/handlers/status-handler.ts:14-39`). dcl-one:
`status::get_status` (`worlds/src/status.rs:121-189`).

| field | upstream | dcl-one |
|---|---|---|
| `content.commitHash` | `COMMIT_HASH` or `"unknown"`. | `commit_hash` or `"unknown"` (`status.rs:164-168`). MATCH. |
| `content.worldsCount` | `{ens, dcl}` over worlds with `deployed_scene_count > 0`, split on the `.dcl.eth` suffix (`worlds-manager.ts:802-820`). | Distinct `world_name` of `worlds.world_scenes`, split on `LIKE '%.dcl.eth'` (`queries.rs:263-275`). Mirrored worlds count as deployed here. MATCH. |
| `comms` | The LiveKit adapter's status, cached 60 s: `{adapterType:"livekit", statusUrl:"https://<host>/", rooms, users, timestamp}`, `details` dropped; world rooms with players, scene rooms excluded; a LiveKit error reads as 0/0 (`comms-adapter.ts:137-172,222-245`). | `{adapterType, statusUrl, rooms, users, timestamp}` in that order from this node's presence registry, live; `adapterType:"offline"` with no `statusUrl` when no LiveKit host is set (`status.rs:121-144`) (WR-21). |
| Extra | — | `personalWorlds` (`null` when off) and `mirror` (`status.rs:88-96,152-187`). |
| CORS | `*`. | None (WR-06). |

Upstream test `test/integration/get-endpoints.spec.ts:42-61` expects `comms.rooms`/`users` from its
LiveKit mock; the equivalent here depends on the presence feed (Pulse/LiveKit webhook). The root
`/status` is the catalyst status (`content/src/status.rs:17-49`), a different shape (WR-12).

---

## 9. POST /reprocess-ab

Upstream: registered when `AUTH_SECRET` is set, which `requireString` makes mandatory at startup
(`src/controllers/routes.ts:335-344`). Chain: `bearerTokenMiddleware(AUTH_SECRET)`, the schema
validator, `reprocessABHandler` (`src/controllers/handlers/reprocess-ab-handler.ts:7-82`).

- Bearer: no `Authorization` → 401 `{"error":"Not Authorized","message":"Authorization header is
  missing"}`; anything but `Bearer <secret>` → 401 `Invalid authorization header`
  (`@dcl/http-commons` `bearer-token-middleware.ts`).
- Schema (`src/controllers/schemas/reprocess-ab-schemas.ts`): `{worlds: [{worldName:
  ^[a-zA-Z0-9-]+\.(dcl\.eth|eth)$, entityIds?: string[]}] (minItems 1)}`, no extra properties.
  Non-JSON `Content-Type` → 415 `{"ok":false,"message":"Content-Type must be application/json"}`;
  unparsable body → 400 `{"ok":false,"message":<parser message>}`; invalid → 400
  `{"ok":false,"message":"Invalid JSON body","data":<ajv errors>}`
  (`@dcl/schema-validator-component` `dist/src/component.js`; tests
  `reprocess-ab-handler.spec.ts:155-218`).
- `AWS_SNS_ARN` unset → 500 `{"error":"Internal Server Error"}` (`reprocess-ab-handler.spec.ts:220-242`).
- Every deployed scene of the named worlds (optionally only `entityIds`), minus banned names; none →
  400 `{"error":"Bad request","message":"No scenes found for reprocessing"}`.
- Publishes one `WorldDeploymentEvent` per scene to SNS; 200 `{baseUrl, batch: [{entity: {entityId,
  authChain}, contentServerUrls: [baseUrl]}], successful, failed}` with `baseUrl` from `HTTP_BASE_URL`
  or the request origin (`reprocess-ab-handler.spec.ts:38-62`).

dcl-one: no route at the root or under the prefix; the request falls to the 404 `Not found`. This
tree publishes no SNS events (`docs/worlds-compatibility.md` "Delivery"); its asset-bundle tooling is
reached through the operator console (`ABGEN_ADMIN_URL`, `src/serve.rs:1214-1223`) (WR-22).

---

## 10. POST /gc

Upstream: `bearerTokenMiddleware(AUTH_SECRET)` then `garbageCollectionHandler`
(`src/controllers/handlers/garbage-collection.ts:14-42`) over `contentGc.collectGarbage`
(`src/logic/content-gc/component.ts:51-73`). dcl-one: `admin::garbage_collect`
(`worlds/src/core/admin.rs:88-188`) over `catalyst_storage::gc::collect`.

| aspect | upstream | dcl-one |
|---|---|---|
| Path | `/gc` at the host root. | `/worlds-content-server/gc` and `/worlds-content-server/admin/gc`; no root `/gc` (`core/mod.rs:588-603`) (WR-27). |
| Auth | `AUTH_SECRET`; missing header 401 `Authorization header is missing`, else 401 `Invalid authorization header`; the token is the second space-separated word (`header.split(' ')`). | `CATALYRST_WORLDS_ADMIN_TOKEN`; the same two 401 bodies (`worlds/src/core/auth.rs:31-39`); the token is everything after `Bearer ` (`library/dcl-auth/src/admin_bearer.rs:37-43`); unset refuses every request with 401 (WR-28). |
| Writes switch | None. | 503 while `WORLDS_WRITES_ENABLED=false` (WR-29). |
| What is removed | Every stored key not referenced by a world scene (soft-deleted ones until eviction included), its entity file and `.auth`, a world thumbnail, or a live pending upload; no age floor; then the expired-upload cleanup (`content-gc/component.ts:37-73`, `src/adapters/content-references.ts:46-69`; `test/integration/garbage-collection.spec.ts`, `content-references.spec.ts`). | Store-wide: every key no domain of the shared store references (world scenes, retired scenes, pending scenes and their files, thumbnails, and the other domains' tables, `storage/src/gc.rs:105-145`), older than the age floor (at least an hour and the upload window), then the expired-upload cleanup (`admin.rs:96-138`). Intended (`worlds-compatibility.md` "`/gc`"). |
| 200 body | `{message: "Garbage collection removed N unused keys.", expiredUploads: {outcome, removed, failed}}` (`test/unit/garbage-collection-handler.spec.ts:73-81`). | `{message, dryRun, removed, removedBytes, failed, candidates, scanned, activeKeys, minAgeSeconds, expiredUploads}`; same `message` and `expiredUploads` shape; `expiredUploads: null` on a dry run (`admin.rs:168-187`). Intended (same doc entry). |
| Query | Ignored. | `dry_run`, `min_age_secs`, `allow_empty_active_set`, typed: `?dry_run=yes` is axum's plain-text 400 (`admin.rs:61-69`) (WR-30). |
| Lock busy | 503, `Retry-After: 60`, `{"error":"Service Unavailable","message":"The content lock is still busy, please retry shortly."}` (`garbage-collection.ts:29-36`, `content-locks/errors.ts`). | 503, `Retry-After: 60`, same keys, message `Garbage collection deferred: deployments kept the content lock busy.`; a second concurrent run answers 503 `A garbage collection run is already in progress.` (`admin.rs:71-86,111-116,148-153`) (WR-31). |
| Empty reference set | n/a | 400 `Refusing to collect: …` unless `allow_empty_active_set=true` (`admin.rs:154-159`). |
| Other failure | Rethrown → 500 `{"error":"Internal Server Error"}`. | 500 `{"error":"Internal Server Error"}` (`admin.rs:160-166`, `worlds_envelope.rs:175-180`). MATCH. |

---

## Mounts: what a worlds client sees on each host

| request | upstream (its host) | dcl-one, `worlds-content-server.<domain>` vhost or `/worlds-content-server` prefix | dcl-one, node root |
|---|---|---|---|
| `GET /contents/<cid>` | worlds handler | worlds handler (no app CORS) | catalyst handler: no sniff without `?includeMimeType`, no SHA-256 keys, GET quota |
| `GET /ipfs/<cid>` | worlds handler | worlds handler (no app CORS) | 404 `Not found` |
| `GET /available-content` | worlds checks | worlds checks | catalyst checks (`Please set at least one cid.`, 1000 cap) |
| `POST /entities/active` | world names | world names | catalyst pointers (`[]` for a world name) |
| `GET /index` | worlds index | worlds index | worlds index |
| `GET /status` | worlds status | worlds status (no app CORS) | catalyst status; worlds status at `/worlds/status` |
| `POST /reprocess-ab` | admin | 404 | 404 |
| `POST /gc` | admin | admin | 404 |

The links `/about` and `/index` build point at the root column for `/contents` (WR-12).

## Backlog (WR-)

1. **WR-01 · `/contents`, `/ipfs` · range outcomes, headers and HEAD body.** Status: MATCH, no work.
   - Upstream: `parseRangeHeader` and the header set (`content-file-handler.ts:53-111`); tests
     `test/unit/content-file-handler.spec.ts:12-186,288-345`, `test/integration/get-endpoints.spec.ts:27-229`.
   - dcl-one: `parse_range_header` and `content_file_headers` (`content/src/formatters.rs:41-102`)
     give the same 200/206/416 outcomes, `ETag`, `Cache-Control`, `Accept-Ranges` and
     `Content-Length` for every tested case; HEAD has no body (`get_content.rs:248-261`).
   - Severity: none (closed).

2. **WR-02 · `/contents`, `/ipfs` · an invalid key answers 404, upstream 400.** Status: intended (worlds-compatibility.md).
   - Upstream: 400, empty body (`content-file-handler.ts:126,203`; `get-endpoints.spec.ts:199-212`,
     `content-file-handler.spec.ts:548-567`).
   - dcl-one: 404 (`get_content.rs:163-174`).
   - Drop-in: no known client branches on 400 versus 404 for a hash it built; a test that probes a
     bad key sees a different status. Does not break replacement.
   - Proposed fix: keep: intended, the shared handler treats every unknown key as absent.
   - Severity: P2.

3. **WR-03 · `/contents`, `/ipfs` · the 404 has a JSON body.**
   - Upstream: `{status: 404}`, empty body (`content-file-handler.ts:118,135,174,207`).
   - dcl-one: `{"error":"No content found with hash <key>"}` (`get_content.rs:80-82`).
   - Proposed fix: in `content/src/get_content.rs` `serve`, answer a bare 404 when `mount` is
     `WORLDS` (the root keeps the catalyst body its fixtures pin).
   - Severity: P2.

4. **WR-04 · `/contents`, `/ipfs` · the Content-Type sniffer.**
   - Upstream: `file-type@16` `fromStream` over a 4100-byte sample; JSON and other text are
     `application/octet-stream` (`content-file-handler.ts:14-41`; `file-type` not read, its JSON
     non-detection is the comment's statement at lines 18-21). It also tells `ftyp` brands apart
     (`M4A` audio, `qt` QuickTime) and detects fonts, AVIF, BMP, FLAC, WebAssembly and many more.
   - dcl-one: 14 signatures over 32 bytes; every `ftyp` file is `video/mp4`; a body opening with
     `{` or `[` is `application/json` (`get_content.rs:24-66,105-118`). The JSON rule is listed as
     intended ("Content types come from the catalyst's sniffer, so JSON is `application/json`"),
     but the code comment at `get_content.rs:99-102` says the worlds mount answers JSON "as
     worlds-content-server does", which the upstream handler does not.
   - Drop-in: browsers and engines read these files by extension from the scene manifest; a client
     that keys on `Content-Type` for fonts, audio or `.mov` sees `application/octet-stream` or
     `video/mp4` instead.
   - Proposed fix: add the `file-type` signatures scene files use (woff/woff2/ttf/otf, AVIF, BMP,
     FLAC, WebAssembly, the `M4A`/`qt` brands) to `detect_content_type`, read the head up to 4100
     bytes on the worlds mount, and correct the comment in `content/src/get_content.rs`. Keep the
     JSON rule (documented).
   - Severity: P2.

5. **WR-05 · `/contents`, `/ipfs` · HEAD with a `Range` header.**
   - Upstream: `headContentFile` ignores `Range` and answers 200 with the full length.
   - dcl-one: 206 with the window length and `Content-Range` (`get_content.rs:209-237`).
   - Proposed fix: in `serve`, skip range parsing for `HEAD` on the `WORLDS` mount. Size S.
   - Severity: P2.

6. **WR-06 · `/worlds-content-server/{status,contents,ipfs,openapi.json}` · no CORS headers.**
   - Upstream: the server-wide CORS middleware answers every request with `Origin`
     (`Access-Control-Allow-Origin: *`) and every preflight with 204 (`src/components.ts:96-107`).
   - dcl-one: these four routes are added to `content_server` after `router(worlds)` applied
     `cors_middleware` and `nul_guard_middleware`, so they carry neither (`worlds/src/lib.rs:394-422`).
     `/available-content` and `/health/*` add their own layers. A preflight to one of the four is a
     405 from axum. Behind the gateway, `$cors_fallback` adds `Access-Control-Allow-Origin: *` to a
     response without one (`gateway.py:262-265,352`), so plain GETs read; a preflighted request still
     fails, and a node served without the gateway gives browsers no CORS at all.
   - Proposed fix: in `worlds/src/lib.rs` `app_router`, apply `nul_guard_middleware` and
     `cors_middleware` to `content_server` after its last route (or as `route_layer`s on these four),
     and drop the per-route layers on `/available-content`; add a cross-origin GET and a preflight
     case to `worlds_contents_sniff_and_answer_legacy_sha256_keys`. Size S.
   - Severity: P1 (a browser client of a node without the gateway cannot read world content).

7. **WR-07 · `/contents`, `/ipfs` · `If-None-Match` answers 304.** Status: kept (superset).
   - Upstream: always 200.
   - dcl-one: 304 on a matching ETag (`get_content.rs:180-184`).
   - Proposed fix: keep: a conditional request is only sent by a cache that holds the immutable
     body. Record it in `docs/worlds-compatibility.md`.
   - Severity: P2.

8. **WR-08 · `/contents`, `/ipfs` · `Range` edge spellings.**
   - Upstream: `^bytes=(\d*)-(\d*)$`; a signed number (`bytes=+5-`) is unsupported → 200; an
     overflowing number parses as a large integer → 416.
   - dcl-one: `u64::parse` accepts `+5` → 206; an overflow fails the parse → 200
     (`formatters.rs:48-79`).
   - Proposed fix: accept ASCII digits only and treat an overflow as unsatisfiable in
     `parse_range_header` (shared with the root mount; check the content-reads fixtures first). Size S.
   - Severity: P2.

9. **WR-09 · `/contents`, `/ipfs` · CIDv0 keys, the denylist and the read-through.** Status: kept (shared store).
   - Upstream: `Qm…` → 400; no denylist; a missing key is 404.
   - dcl-one: `Qm…` keys are served; denylisted keys answer 404; with `READ_THROUGH_ENABLED` a
     missing key is fetched from the sync sources before the 404 (`get_content.rs:163-197`).
   - Proposed fix: keep: a consequence of the one blob store; add the CIDv0 and denylist rules to
     `docs/worlds-compatibility.md` "Legacy content keys".
   - Severity: P2.

10. **WR-10 · `/contents` · the extra `Access-Control-Expose-Headers` entry and 416 header.** Status: MATCH, no work.
    - Upstream: `ETag, Accept-Ranges, Content-Range`.
    - dcl-one: `ETag, Content-Range, Accept-Ranges, Content-Length`; a 416 also exposes
      `Content-Range` (`formatters.rs:89-92`, `get_content.rs:198-208`). A superset that exposes
      nothing sensitive.
    - Severity: none.

11. **WR-11 · `/available-content` · a denylisted cid is left out of the answer.**
    - Upstream: every requested cid (deduplicated) is listed.
    - dcl-one: the content handler drops denylisted cids before the lookup
      (`content/src/available_content.rs:32-36`), so the worlds answer can be shorter than the
      request.
    - Proposed fix: in `worlds/src/available_content.rs`, list a denylisted cid as
      `{cid, available: false}` (the deploy client then uploads it, and the deploy refuses it as it
      does now), keeping the content handler's rule for the root.
    - Severity: P2.

12. **WR-12 · `/contents`, `/available-content`, `/entities/active`, `/status` · the root is the catalyst's, and worlds links point at it.**
    - Upstream: one host; `/about` and `/index` link `HTTP_BASE_URL/contents/…`, which is that host
      (`index-handler.ts:15,27`).
    - dcl-one: on the root host these four paths are the catalyst content server's
      (`content/src/lib.rs:741-813`); the worlds versions answer under `/worlds-content-server` and on
      the `worlds-content-server.<domain>` vhost (`gateway.py:280-289`). `/about` and `/index` build
      their links from `PUBLIC_URL`, the root (`worlds/src/about.rs:182-188,223,259`,
      `worlds/src/index.rs:135,188`), so world files are fetched through the catalyst handler: no
      sniffed `Content-Type` (everything `application/octet-stream`), no SHA-256 keys (a mirrored
      world whose thumbnail is a legacy key gets a 404 link), the catalyst GET quota.
    - Drop-in: a client configured with the vhost or the prefix gets the worlds answers for its own
      requests, but follows links that leave that mount.
    - Proposed fix: give the worlds read routes a worlds base URL (`<PUBLIC_URL>/worlds-content-server`
      unless a `WORLDS_PUBLIC_URL` overrides it) in `worlds::Settings` (`worlds/src/lib.rs:134-139`)
      and use it for every `/contents/` link in `about.rs` and `index.rs`; keep `PUBLIC_URL` for the
      comms mint, which is mounted at both. Size M. Coordinate with the about area.
    - Severity: P1.

13. **WR-13 · `/entities/active` · unparsable JSON answers 400, upstream 500.**
    - Upstream: `request.json()` throws outside `InvalidRequestError` → 500
      `{"error":"Internal Server Error"}`.
    - dcl-one: 400 `Invalid request. Request body is not valid` (`active.rs:23-24`).
    - Proposed fix: keep (a client error is a 400) and list it in `docs/worlds-compatibility.md`.
    - Severity: P2.

14. **WR-14 · `/entities/active` · pointers are trimmed and parsed as world names.**
    - Upstream: the raw string, lowercased; `" foo.dcl.eth"` finds nothing.
    - dcl-one: `WorldName::parse` trims, so `" foo.dcl.eth"` finds `foo.dcl.eth`; a name with inner
      whitespace, `/` or a control character is dropped, which also finds nothing upstream
      (`comms/src/world_identity.rs:63-80`, `active.rs:44-49`).
    - Proposed fix: skip a pointer that differs from its trim in `active.rs` (or keep and document).
      Size S.
    - Severity: P2.

15. **WR-15 · `/entities/active` · key order and a world without an owner.**
    - Upstream: the entity's jsonb key order with `id` and `metadata` in place; `metadata.owner` is
      always set (to the row's `owner`).
    - dcl-one: keys sorted (`serde_json` without `preserve_order`); `metadata.owner` absent when the
      world row has no owner (`store.rs:1159-1176`).
    - Proposed fix: write `"owner": null` when the row has none; leave key order (JSON clients do not
      read it). Size S.
    - Severity: P2.

16. **WR-16 · `/entities/active` · body cap.** Status: kept.
    - Upstream: no cap.
    - dcl-one: 256 KiB (axum's 413 text), under the gateway's server-wide 1 MiB
      (`core/mod.rs:623-626`, `gateway.py:366`). Fifty world names fit in a few KiB.
    - Proposed fix: keep.
    - Severity: P2.

17. **WR-17 · `/index` · the name deny list is not applied.**
    - Upstream: `getRawWorldRecords` excludes banned names before paging (`worlds-manager.ts:326-349`),
      so a banned world is never listed.
    - dcl-one: `list_index_scenes` has no deny-list filter (`queries.rs:228-235`); a banned world
      that still has scene rows is listed, and the index's consumers (mirrors, asset-bundle
      snapshot jobs) process it.
    - Proposed fix: pass `state.names.snapshot(&state.pool)`'s normalized names into
      `list_index_scenes` (`worlds/src/queries.rs`) and exclude them with the deny list's
      normalization (strip `.eth`/`.dcl`, lowercase) in SQL; extend the memo key or clear the memo
      when the list changes. Size S.
    - Severity: P1 (moderation: a banned world is published to downstream consumers).

18. **WR-18 · `/index` · `null` where upstream answers `''` or leaves the field out.**
    - Upstream: `title`/`description` `''` when missing; `thumbnail` and `runtimeVersion` absent
      when missing; `timestamp` falls back to `Date.now()` (`worlds-indexer.ts:38-46`; test
      `index-handler.spec.ts:86` expects `thumbnail` undefined).
    - dcl-one: `null` for each (`index.rs:54-67,174-203`).
    - Proposed fix: in `worlds/src/index.rs` `IndexSceneSummary`, make `title` and `description`
      `String` (`unwrap_or_default`), add `skip_serializing_if = "Option::is_none"` to `thumbnail`
      and `runtime_version`, and default `timestamp` to the build time; check that this tree's
      mirror (`worlds/src/mirror.rs`) reads the upstream shape. Size S.
    - Severity: P1 (a consumer that reads `title.length` or tests `thumbnail === undefined` breaks).

19. **WR-19 · `/index` · paging set and blocked worlds.**
    - Upstream: pages over every `worlds` row and skips scene-less ones afterwards, so `limit=N` can
      return fewer than N and offsets count scene-less rows; blocked wallets are not filtered.
    - dcl-one: pages over worlds with scenes; operator-blocked worlds (`blocked_since`) are left out
      (`queries.rs:228-235`). With no parameters (how the known consumers call it) the set differs
      only by blocked and banned worlds.
    - Proposed fix: keep the paging (fuller pages); list the blocked-world exclusion in
      `docs/worlds-compatibility.md` next to "Operator disables apply at once".
    - Severity: P2.

20. **WR-20 · `/index` · 30-second memo and ETag.** Status: kept.
    - Upstream: built per request, no ETag.
    - dcl-one: a page is reused for 30 s (cleared by a mirror pass), with an `ETag` and 304
      (`index.rs:18-38,97-131`); `lastUpdated` is the memo's build time.
    - Proposed fix: keep (bounded cost of a deprecated wide scan).
    - Severity: P2.

21. **WR-21 · `/status` · comms block source and extra fields.**
    - Upstream: always `adapterType: "livekit"` with `statusUrl`, counts from LiveKit's room listing
      cached 60 s; no other top-level fields.
    - dcl-one: counts from the presence registry (empty while presence is stale), live;
      `adapterType: "offline"` without `statusUrl` when no LiveKit host is set; extra `personalWorlds`
      and `mirror` (`status.rs:88-96,121-144`).
    - Proposed fix: keep the extra fields (superset); add "offline" `/status` to
      `docs/worlds-compatibility.md` "LiveKit only". Depends on the presence feed for the counts.
    - Severity: P2.

22. **WR-22 · `POST /reprocess-ab` · missing.**
    - Upstream: section 9: bearer, schema validator (415 / 400 `{ok:false,…}`), 400
      `No scenes found for reprocessing`, 500 without SNS, 200 `{baseUrl, batch, successful, failed}`.
    - dcl-one: no route; 404 `Not found`.
    - Proposed fix: add `POST /reprocess-ab` to `worlds/src/core/mod.rs` (`router`, and `root_router`
      since the root has no such path) with a handler in `worlds/src/core/admin.rs`: the admin bearer,
      the schema with the validator's 415/400 bodies, scenes from `worlds.world_scenes` filtered by
      `entityIds` and the deny list, the 400 above, and a hand-off to this tree's asset-bundle
      converter (the `ABGEN_ADMIN_URL` client in `src/ops/admin/console.rs`), answering upstream's
      200 body with `successful`/`failed` from that hand-off and a 500 when no converter is
      configured. Size M. Depends on the asset-bundle pipeline (ops/abgen).
    - Severity: P1 (operator tooling that re-converts a world gets a 404).

23. **WR-23 · mounts · the root collisions are not documented.**
    - dcl-one: `docs/worlds-compatibility.md` says the surface is served "at the root and under
      `/worlds-content-server`", but `/contents`, `/available-content`, `POST /entities/active` and
      `/status` at the root are the catalyst's, and `/ipfs`, `/gc` exist only under the prefix.
    - Proposed fix: state in `docs/worlds-compatibility.md` that worlds clients use the
      `worlds-content-server.<domain>` host or the `/worlds-content-server` prefix, and list the
      four root paths that answer as the catalyst.
    - Severity: P2.

24. **WR-24 · `/available-content` · `cid[]` and `cid[0]` are read as `cid`.**
    - Upstream: `getAll('cid')` ignores them; `?cid[]=<cid>` answers 400 `At least one cid query
      parameter is required.`
    - dcl-one: `strip_array_index` folds them into `cid` (`shared/src/query_params.rs:16-38`).
    - Proposed fix: read the literal `cid` pairs in `worlds/src/available_content.rs` (a
      `URLSearchParams` reading, as `core::http::QueryPairs`). Size S.
    - Severity: P2.

25. **WR-25 · `/contents` with `STORAGE_X_ACCEL_BASE` · headers nginx adds.** Needs capture.
    - dcl-one: a full GET is handed to nginx, whose protected-storage location adds
      `Cache-Control: public, max-age=31536000, immutable` and `Access-Control-Allow-Origin: *`
      beside the app's headers (`gateway.py:221-229`). Whether the client sees one or two
      `Cache-Control` values, and which `Content-Type`, needs a request against a gateway-fronted
      node.
    - Proposed fix: capture; if doubled, set `proxy_hide_header Cache-Control` or drop the gateway's
      `add_header` for that location.
    - Severity: P2.

26. **WR-26 · 405 · `Allow` spelling under the prefix.**
    - Upstream: `Allow: HEAD, GET` (koa-router registration order, `", "`).
    - dcl-one: axum's `GET,HEAD` under `/worlds-content-server`; `upstream_allow` rewrites only
      `/content/` (`content/src/routing.rs:140-170`).
    - Proposed fix: apply `upstream_allow_value` to `/worlds-content-server/` paths too. Size S.
    - Severity: P2.

27. **WR-27 · `POST /gc` · not at the root.**
    - Upstream: `/gc` at its host root.
    - dcl-one: only `/worlds-content-server/gc` (and `/admin/gc`); `root_router` leaves it out
      (`core/mod.rs:588-603`), so the root answers 404 although no other route uses the path. The
      vhost reaches it.
    - Proposed fix: keep (operator tooling targets the worlds host) and say so in WR-23's
      documentation.
    - Severity: P2.

28. **WR-28 · `POST /gc` · the token variable and bearer parsing.**
    - Upstream: `AUTH_SECRET` (required at startup); `split(' ')` takes the second word, so
      `Bearer <secret> extra` passes.
    - dcl-one: `CATALYRST_WORLDS_ADMIN_TOKEN` (unset: every request 401); the whole remainder after
      `Bearer ` must match (`admin_bearer.rs:37-43`, `config/src/worlds.rs:96-99`).
    - Proposed fix: accept `AUTH_SECRET` as a second env name for the token in
      `config/src/worlds.rs`, so an operator's existing secret works; keep the strict parse. Size S.
    - Severity: P2.

29. **WR-29 · `POST /gc` · refused while worlds writes are off.** Status: intended.
    - dcl-one: 503 `{ok:false,error:"Service Unavailable",message}` with `Retry-After: 3600` until a
      restore is demonstrated (`worlds/src/writes.rs:1-75`, DATABASE.md). Upstream has no such gate.
    - Proposed fix: keep: intended; list `/gc` among the gated routes in `docs/worlds-compatibility.md`.
    - Severity: P2.

30. **WR-30 · `POST /gc` · typed query parameters.**
    - Upstream: the query is ignored.
    - dcl-one: `dry_run`, `min_age_secs`, `allow_empty_active_set` are typed; an unparsable value is
      axum's plain-text 400 (`admin.rs:61-69`).
    - Proposed fix: read them leniently (a non-boolean `dry_run` is false) or answer the worlds 400
      envelope, in `worlds/src/core/admin.rs`. Size S.
    - Severity: P2.

31. **WR-31 · `POST /gc` · the deferred messages.**
    - Upstream: one 503 message, `The content lock is still busy, please retry shortly.`
      (`content-locks/errors.ts`; test `garbage-collection.spec.ts:222-242`).
    - dcl-one: `Garbage collection deferred: deployments kept the content lock busy.` for a lock
      timeout, and `A garbage collection run is already in progress.` for a concurrent run
      (`admin.rs:74,111-116,148-153`).
    - Proposed fix: answer upstream's message for `GcError::LockTimeout` in `admin.rs`; keep the
      in-progress message (upstream has no run lock). Size S.
    - Severity: P2.

32. **WR-32 · CORS shape on the worlds routes.** Status: kept (decision in `shared/src/cors.rs`).
    - Upstream: `*`, no credentials, methods `GET, HEAD, OPTIONS, DELETE, POST, PUT`, no default
      expose list.
    - dcl-one: the origin reflected with credentials and `Vary: Origin`, `PATCH` among the methods,
      `Access-Control-Expose-Headers: *` by default, `nosniff` (`shared/src/cors.rs:15-85`). Every
      request upstream admits is admitted.
    - Proposed fix: keep (documented decision 2026-07-31); add it to `docs/worlds-compatibility.md`.
    - Severity: P2.

33. **WR-33 · error envelope on the worlds routes.** Status: MATCH, no work.
    - Upstream: `errorHandler` labels (`@dcl/http-commons` `error-handler.ts`).
    - dcl-one: `envelope` (`comms/src/worlds_envelope.rs:120-140`); the 400s of `/available-content`
      and `/entities/active` and the 401s of `/gc` carry upstream's exact messages.
    - Severity: none.

34. **WR-34 · unknown route · 404 body.** Status: MATCH, no work.
    - Upstream: 404 `Not found`, `text/plain; charset=utf-8`.
    - dcl-one: the content fallback answers the same for any path, under the prefix included
      (`content/src/lib.rs:728-735,841`).
    - Severity: none.

## Endpoints dcl-one serves in this area that upstream does not

- `GET /worlds/status` at the root (`worlds/src/lib.rs:354`): the worlds status of section 8.
- `POST /worlds-content-server/admin/gc` (`worlds/src/core/mod.rs:592`): the same handler as `/gc`.
- `GET /worlds-content-server/openapi.json` (`worlds/src/lib.rs:419-422`): the worlds OpenAPI
  document (no CORS, WR-06).
- `GET /worlds-content-server/health/{live,ready,startup}` (`shared/src/health.rs:12-26`): upstream
  has them through `@dcl/http-server`'s status checks (`dist/status-checks.js`), where `ready` and
  `startup` report component checks; here they always pass.
- `GET /index` at the root as well as under the prefix.
- `?dry_run`, `?min_age_secs`, `?allow_empty_active_set` on `/gc` (WR-30).
- `If-None-Match` → 304 on `/contents`, `/ipfs` and `/index` (WR-07, WR-20).
