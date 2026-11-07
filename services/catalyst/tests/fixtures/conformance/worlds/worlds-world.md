# Worlds content server: world metadata reads and settings

Area: the routes that describe a world and edit its settings: `GET /world/:world_name/about`,
`GET`/`POST /world/:world_name/scenes`, `GET`/`PUT /world/:world_name/settings`,
`GET /world/:world_name/manifest`, `GET /world/:world_name/limits`, `GET /worlds`,
`GET /wallet/:wallet/stats`, `GET /wallet/contribute`.

Pinned revisions:

- Upstream: `decentraland/worlds-content-server` @ `10af5255bb04a1a28e0c26bd190317c26a6015a9` (2026-10-21).
- Packages read from their published tarballs, at the versions `yarn.lock` resolves:
  `@dcl/http-commons` 2.0.0 (`errorHandler`, `getPaginationParams`),
  `@dcl/schema-validator-component` 1.0.1, `@dcl/crypto-middleware` 6.3.0, `@dcl/http-server` 2.3.0 (`cors.js`).
  `@dcl/schemas` (`EthAddress`, `IPFSv2`, `SceneParcels`), `@dcl/catalyst-contracts` and
  `@dcl/catalyst-api-specs` were not read; rows that depend on them say so.
- dcl-one: `dcl-one` @ `2da8b8aea` (origin/main), Rust workspace `services/catalyst`.

Everything below is from reading the code and the upstream tests. Nothing was built, run or captured,
and no line is "verified".

Files read:

- Upstream:
  - `src/controllers/routes.ts`
  - `src/controllers/handlers/{world-about,scenes,world-settings,world-manifest,world-limits,worlds,wallet-stats,contributor}-handler.ts`
  - `src/controllers/schemas/scenes-query-schemas.ts`
  - `src/controllers/multipart/{middleware,limits,errors}.ts`, `src/controllers/upload-error-handler.ts`,
    `src/controllers/source-upload-admission.ts`
  - `src/adapters/worlds-manager.ts` (`getMetadataForWorld`, `getWorldScenes`, `getWorlds`,
    `getContributableDomains`, `getWorldSettings`, `updateWorldSettings`, `getOccupiedParcels`,
    `getWorldBoundingRectangle`)
  - `src/adapters/{limits-manager/component,limits-manager/errors,wallet-stats,name-deny-list-checker,status}.ts`,
    `src/adapters/content-locks/{component,errors}.ts`, `src/adapters/source-upload-limits/errors.ts`
  - `src/logic/{settings/component,settings/errors,world-settings-policy/component,world-settings-policy/types,coordinates/component,thumbnails/component,form-data,world-runtime-metadata-utils}.ts`,
    `src/logic/worlds/component.ts` (`isWorldBlocked`, `getWorldManifest`), `src/logic/validations/parcels.ts`
  - `src/types.ts`, `src/components.ts`, `src/migrations/{0017,0018}_*.ts`
  - Tests: `test/integration/{world-about-handler,scenes-handler,world-settings-handler,world-manifest-handler,world-limits-handler,worlds-handler,wallet-stats-handler,contributor-handler}.spec.ts`,
    the matching parts of `world-manager-adapter.spec.ts`, `limits-manager-adapter.spec.ts`,
    `wallet-stats-adapter.spec.ts`, and `test/unit/{settings-logic,settings-content-lock,world-limits-handler,world-limits-parity,name-deny-list-checker}.spec.ts`.
- dcl-one (`services/catalyst/`):
  - `worlds/src/lib.rs` (`router`, `app_router`, `Settings::from_config`, `fill_world`)
  - `worlds/src/about.rs`, `worlds/src/scenes.rs`, `worlds/src/queries.rs` (`world_about`, `push_scene_filters`, `list_scenes_full`)
  - `worlds/src/core/mod.rs` (`router`, `root_router`, `upload_routes`, `timed_routes`)
  - `worlds/src/core/{settings,settings_policy,manifest,limits,wallet,names,quota,http,store}.rs`
  - `worlds/src/writes.rs`
  - `comms/src/worlds_envelope.rs`, `comms/src/signed_fetch.rs`, `comms/src/world_identity.rs` (`Parcel`)
  - `shared/src/signed_fetch.rs`, `shared/src/identity.rs` (`Address`), `shared/src/cors.rs`
  - `content/src/errors.rs`, `content/src/lib.rs` (`content_public_url`)
  - `schemas/20_worlds.sql` (repo root)
  - `docs/worlds-compatibility.md`

Common notes:

- **Mount points.** Upstream serves these routes at the root of its own host. dcl-one serves every route in
  this area at the root and again under `/worlds-content-server`: the reads from `worlds::router`
  (`worlds/src/lib.rs:342-357`) and the rest from `core::root_router` / `core::router`
  (`worlds/src/core/mod.rs:588-628`, `worlds/src/lib.rs:394-434`). The two mounts answer the same.
- **Error envelope.** Upstream's `errorHandler` (`@dcl/http-commons` 2.0.0) answers
  `400 {"error":"Bad request","message":…}`, `404 {"error":"Not Found","message":…}`,
  `401 {"error":"Not Authorized","message":…}` and `500 {"error":"Internal Server Error"}`. Handlers that build
  their own response (settings) answer `{"error":<message>}` with no `message`. dcl-one's worlds envelope
  (`comms/src/worlds_envelope.rs:123-140`) reproduces the http-commons labels and serves `/about` too (WW-02,
  fixed); the settings handlers answer their own `{"error":<message>}` (`core/settings.rs` `SettingsError`,
  WW-17, WW-19, WW-20, fixed).
- **Signed fetch.** Upstream's strict `signedFetchMiddleware` guards `PUT /world/:world_name/settings` and
  `GET /wallet/contribute` (`src/controllers/routes.ts:98,222,252`): current payload only,
  `rejectIfSigner('decentraland-kernel-scene')`, failures answer
  `{"error":<detail>,"message":"This endpoint requires a signed fetch request. See ADR-44."}` with the
  detail's status (400 `Invalid Auth Chain`, 401 `Invalid signature…` / `Expired signature…`).
  dcl-one calls `require_verified(…, &[], …)` (no legacy fallback, the scene gate included,
  `comms/src/signed_fetch.rs`), so acceptance matches, and failures answer the same body (WW-24, fixed). The signed-fetch verifier itself belongs to the auth area.
- **Pagination.** `getPaginationParams` 2.0.0: limit 1..100 else 100, offset `parseInt` ≥ 0 else 0, no
  offset cap (the 100,000 cap arrived in 2.0.1, not the locked version). dcl-one `pagination_of`
  (`worlds/src/core/http.rs:27-41`) is the same, except that an offset beyond `i64` saturates (upstream
  passes it to Postgres, which answers a 500).
- **CORS.** Upstream `@dcl/http-server` answers `Access-Control-Allow-Origin: *` with methods
  `GET, HEAD, OPTIONS, DELETE, POST, PUT` and max-age 86400 (`src/components.ts:101-104`). dcl-one echoes the
  origin with `Access-Control-Allow-Credentials: true`, reflects requested headers and adds `PATCH`
  (`shared/src/cors.rs:14-79`). Same cross-cutting row as the content-lambdas CORS theme (WW-37).
- **Writes switch.** `PUT /world/:world_name/settings` sits behind `writes::guard`; while
  `WORLDS_WRITES_ENABLED` is off (the default) it answers 503
  `{"ok":false,"error":"Service Unavailable","message":…}` with `Retry-After: 3600`
  (`worlds/src/writes.rs:29-75`). Every other route in this area is a read and stays open (WW-27,
  documented).
- **Deny list.** Both trees read the DAO `banned-names` list (upstream `DCL_LISTS_URL`, one-hour cache;
  dcl-one `explore_api.lists`, one-hour cache, `worlds/src/core/names.rs:16-78`) and normalize the name the
  same way (`toLowerCase().replace('.eth','').replace('.dcl','')`, first occurrence only).

## Endpoint inventory

| # | method | upstream path | dcl-one route(s) | status |
|---|---|---|---|---|
| 1 | GET | `/world/:world_name/about` | `worlds/src/about.rs` `get_about` (route `worlds/src/lib.rs:344`) | DIVERGENT (intended only) |
| 2 | GET | `/world/:world_name/scenes` | `worlds/src/scenes.rs:49` `list_scenes` (route `lib.rs:345-348`) | MATCH |
| 3 | POST | `/world/:world_name/scenes` | `worlds/src/scenes.rs` `post_list_scenes` (same route) | MATCH |
| 4 | GET | `/world/:world_name/settings` | `worlds/src/core/settings.rs` `get_world_settings` (route `core/mod.rs:669-672`) | DIVERGENT (intended only) |
| 5 | PUT | `/world/:world_name/settings` | `worlds/src/core/settings.rs` `update_world_settings` (route `core/mod.rs:604-612`) | DIVERGENT (intended only; the signed-fetch body with #383) |
| 6 | GET | `/world/:world_name/manifest` | `worlds/src/core/manifest.rs:19` (route `core/mod.rs:637-640`) | DIVERGENT (intended only) |
| 7 | GET | `/world/:world_name/limits` | `worlds/src/core/limits.rs:50` (route `core/mod.rs:673`) | DIVERGENT (intended only) |
| 8 | GET | `/worlds` | `worlds/src/core/manifest.rs` `get_worlds` (route `core/mod.rs:636`) | DIVERGENT (intended only) |
| 9 | GET | `/wallet/:wallet/stats` | `worlds/src/core/wallet.rs:119` (route `core/mod.rs:679`) | DIVERGENT (intended only) |
| 10 | GET | `/wallet/contribute` | `worlds/src/core/wallet.rs:42` (route `core/mod.rs:678`) | DIVERGENT (intended only; the signed-fetch body with #383) |

Counts: 10 endpoints. MATCH 2, GAP 0, DIVERGENT 8 (all on intended differences only, two once #383's
signed-fetch bodies land), MISSING 0.
Every route exists at both dcl-one mounts.

---

## 1. GET /world/:world_name/about

The explorer's realm handshake for a world: it reads `configurations.scenesUrn` to load scenes and
`comms.adapter` to join comms.

### Request

Path parameter only. Upstream handler `src/controllers/handlers/world-about-handler.ts:11-125`; metadata
from `worldsManager.getMetadataForWorld` (`src/adapters/worlds-manager.ts:384-435`), runtime metadata from
`buildWorldRuntimeMetadata` (`src/logic/world-runtime-metadata-utils.ts:5-92`). dcl-one
`worlds/src/about.rs:121-301`, data from `queries::world_about` (`worlds/src/queries.rs:78-115`).

### Responses

| trigger | status | body | upstream | dcl-one | verdict |
|---|---|---|---|---|---|
| Deny-listed name | 404 | `{"error":"Not Found","message":"World \"<name>\" has no scene deployed."}` in both | `world-about-handler.ts:22-24` | `about.rs` `get_about` | MATCH (fixed, WW-02) |
| No world row, or no scenes | 404 | the same, message `World "<name>" has no scenes deployed.` | `:26-29` | `about.rs` `build_about` | MATCH (fixed, WW-02). dcl-one first tries a federation fill (`lib.rs:273`, dcl-one only) |
| Owner quota-blocked over 2 days | 401 | `{"error":"Not Authorized","message":"World \"<name>\" has been blocked since <Date.toString() in UTC> as it exceeded its allowed storage space."}` in both | `:31-35`, `logic/worlds/component.ts:52-60` | `about.rs` `build_about` | MATCH (fixed, WW-02, WW-03) for a UTC upstream. The grace matches; the live-holder rule and operator disable are intended (WW-13) |
| `ETH_NETWORK` not in `l1Contracts` | 500 | `{"error":"Internal Server Error"}` | `:46-50` | `lib.rs:122-127` maps anything but `sepolia` to 1 | DIVERGENT, config only, intended (WW-14) |
| Success | 200 | example below | `:121-124` | `about.rs:258-300` | DIVERGENT (fields below) |

Upstream success body for one scene (from `world-about-handler.spec.ts:106-145`, which uses `toEqual`):

```json
{
  "healthy": true,
  "acceptingUsers": true,
  "spawnCoordinates": "20,24",
  "configurations": {
    "networkId": 1,
    "globalScenesUrn": [],
    "scenesUrn": ["urn:decentraland:entity:<entityId>?=&baseUrl=http://0.0.0.0:3000/contents/"],
    "minimap": {"enabled": false, "dataImage": "http://0.0.0.0:3000/contents/<hash>"},
    "map": {"minimapEnabled": false, "sizes": []},
    "skybox": {"textures": ["http://0.0.0.0:3000/contents/<hash>"]},
    "realmName": "<worldName>"
  },
  "content": {"healthy": true, "publicUrl": "https://peer.com/content", "synchronizationStatus": "Syncing"},
  "lambdas": {"healthy": true, "publicUrl": "https://peer.com/lambdas"},
  "comms": {"healthy": true, "protocol": "v3", "adapter": "fixed-adapter:signed-login:http://0.0.0.0:3000/worlds/<worldname>/comms"}
}
```

**Fields** (upstream / dcl-one):

| field | upstream | dcl-one | verdict |
|---|---|---|---|
| `scenesUrn` | the most recently deployed scene only: `getWorldScenes(…, {limit: 1, orderBy: created_at, desc})` (`worlds-manager.ts:403-407`); the spec pins length 1 (`world-about-handler.spec.ts:541-554`) | the newest scene only (`about.rs` `build_about`, `.take(1)` of the newest-first rows) | MATCH (fixed, WW-01) |
| `baseUrl` in URNs, file URLs, adapter | `HTTP_BASE_URL`, else the request's host | `Settings.public_url` (`lib.rs:136-139`) | MATCH when `HTTP_BASE_URL` is the public URL |
| `spawnCoordinates` | `worlds.spawn_coordinates`, `null` when unset | the same | MATCH (fixed, WW-07) |
| `realmName` | `worldConfiguration.name` (after the `dclName` migration) `|| worldName` | the same (`about.rs` `RuntimeMeta::from_scene`), then the ENS strip and override (`resolve_realm_name`) | MATCH (fixed, WW-05, WW-06); strip intended (WW-12) |
| `minimap.enabled` | `miniMapConfig.visible ?? false`, where a truthy legacy `minimapVisible` replaces the whole `miniMapConfig` | the same, read as a boolean (JS truthiness) | MATCH (fixed, WW-05) |
| `minimap.dataImage` / `estateImage` | present when enabled or set; the file URL, else `https://api.decentraland.org/v1/minimap.png` / `estatemap.png` | same rule; defaults `map_parcel_view_url` (this node's `/api/v1/minimap.png` unless configured, `config/src/config.rs:570`) and `map_estate_view_url` | DIVERGENT: default URLs, intended (WW-09) |
| `skybox.fixedHour` | `worlds.skybox_time` when not NULL, else `skyboxConfig.fixedTime` (or legacy `skybox`); absent when neither | the same; the number as stored or deployed | MATCH (fixed, WW-04, WW-05) |
| `skybox.textures` | each texture resolved to `<baseUrl>/contents/<hash>`; an unresolved file becomes `""`; `[]` when unset | the same | MATCH (fixed, WW-06) |
| `globalScenesUrn` | `GLOBAL_SCENES_URN.split(' ')` (two spaces give an empty element) | `split_whitespace` (`lib.rs:166-173`) | DIVERGENT, config only, intended (WW-14) |
| `networkId` | `l1Contracts[ETH_NETWORK].chainId` | `network_id_for` (`lib.rs:122-127`) | MATCH for mainnet and sepolia |
| `map` | `{minimapEnabled:false, sizes:[]}` | same | MATCH |
| `healthy`, `acceptingUsers`, `content.healthy`, `lambdas.healthy` | `GET <CONTENT_URL>/status` reports `Syncing`, and `GET <LAMBDAS_URL>/status` answers, cached 5 min (`adapters/status.ts`) | false only when this node's content sync failed; lambdas always true (`about.rs:93-118,262-276`) | DIVERGENT, intended (WW-10) |
| `content.publicUrl`, `lambdas.publicUrl` | `new URL(CONTENT_URL).toString()` (`https://peer.com/content`, no trailing slash added) | `CONTENT_URL`/`LAMBDAS_URL` as set, else `<public_url>/content` and `/lambdas` (`lib.rs` `Settings::from_config`) | MATCH (fixed, WW-08) |
| `comms.adapter` | `fixed-adapter:offline:offline` when `fixedAdapter` is `offline:offline` or `single_player`; otherwise `fixed-adapter:signed-login:<baseUrl>/worlds/<lowercase name>/comms` (`world-about-handler.ts:127-134`) | the same, and offline too while LiveKit has no credentials or the SFU probe fails (`about.rs:320-333`) | DIVERGENT, intended (WW-11) |
| `comms.healthy`, `protocol` | `true`, `"v3"` | same | MATCH |
| `content.synchronizationStatus` | `"Syncing"` | same | MATCH |

### Data dependency

The about route reads `worlds.worlds` and `worlds.world_scenes` (both trees), the quota block
(dcl-one: `worlds.quota_blocks` for the live NAME holder, `about.rs:136-145`), the deny list, and the comms
state (dcl-one: `cfg.comms_online && cfg.sfu.is_alive()`).

**Dependency on the comms area.** The adapter string points the explorer at
`POST <public_url>/worlds/<name>/comms`, which dcl-one serves from the comms gatekeeper, not the worlds crate.
Whether that URL answers at both mounts, and with the upstream token shape, is the comms area's row. The
offline fallback (WW-11) reads the shared SFU liveness that comms maintains.

---

## 2. GET /world/:world_name/scenes

### Request

| param | upstream (`scenes-handler.ts:18-85`) | dcl-one (`scenes.rs:49-223`) | verdict |
|---|---|---|---|
| `limit`, `offset` | `getPaginationParams` | `pagination` | MATCH (common notes) |
| `x1, x2, y1, y2` | present, non-empty, `Number()`-valid values; one to three of them is 400 `Bounding box requires all of x1, x2, y1, y2 to be provided.`; the box is min/max of each pair | `bounding_box` and `js_number` (`scenes.rs:101-148`) | MATCH |
| `authorized_deployer` | `EthAddress.validate` or 400 `Invalid authorized_deployer address: <v>. Must be a valid Ethereum address.`; owner or world `deployment` grant | `Address::parse_untrimmed` behind a `0x` check (`scenes.rs:157-170`); same SQL (`queries.rs:165-177`) | MATCH (assuming the `@dcl/schemas` regex `^0x[a-fA-F0-9]{40}$`) |
| `entity_id` | present (even empty) and not `IPFSv2` is 400 `Invalid entity_id: <v>. Must be a valid IPFS CIDv1.` | `is_cid_v1` (`scenes.rs:151-153,171-178`) | MATCH (depends on `IPFSv2` being `^ba[a-zA-Z0-9]{57}$`) |
| repeated keys | `searchParams.get` reads the first | `query_param` reads the first | MATCH |

### Responses

| trigger | status | body | upstream | dcl-one | verdict |
|---|---|---|---|---|---|
| Success | 200 | `{"scenes":[…],"total":<n>}` | `scenes-handler.ts:73-84` | `scenes.rs:187-223` | MATCH |
| Unknown world | 200 | `{"scenes":[],"total":0}` | `:73` | same (after a federation fill attempt, dcl-one only) | MATCH |
| Filter errors | 400 | `{"error":"Bad request","message":…}` | above | above | MATCH |
| Database error | 500 | `{"error":"Internal Server Error"}` | `errorHandler` | `ApiError::Internal` | MATCH |

Scene objects: upstream spreads `WorldScene` (`worlds-manager.ts:60-73`) with `size` as a string:
`worldName, deployer, entityId, deploymentAuthChain, entity, parcels, size, status, createdAt, updatedAt`.
dcl-one has the same keys and values (`scenes.rs:22-35`), with `status` always `"DEPLOYED"` (retired scenes
live in `worlds.retired_world_scenes`, so `world_scenes` holds deployed rows only), and timestamps as
RFC 3339 with milliseconds and `Z`, as `Date.toJSON()` writes them. Key order differs (`entityId` after
`entity`); no client reads key order. Order: upstream `ORDER BY created_at ASC` with no tie-break;
dcl-one adds `entity_id` (`queries.rs:195`), a deterministic subset of the upstream order. No deny-list
check in either tree.

---

## 3. POST /world/:world_name/scenes

### Request

Upstream: `schemaValidator.withSchemaValidatorMiddleware(getWorldScenesSchema)` then the GET handler with
`coordinates` from the body (`routes.ts:204-208`). Schema (`schemas/scenes-query-schemas.ts:10-26`):
object, `additionalProperties:false`, required `coordinates`: array of 1..500 unique strings matching
`^-?\d+,-?\d+$`. Coordinates are canonicalized before the `parcels && $1` match.
dcl-one: `schema_coordinates` (`scenes.rs:75-97`) checks the same rules and canonicalizes with
`Parcel::parse_strict` (`comms/src/world_identity.rs:348-360`).

### Responses

| trigger | status | upstream body | dcl-one body | verdict |
|---|---|---|---|---|
| `Content-Type` not JSON (`application/json` or `type/sub+json`) | 415 | `{"ok":false,"message":"Content-Type must be application/json"}` | the same (`scenes.rs` `is_json_content_type`) | MATCH (fixed, WW-15) |
| Body is not JSON | 400 | `{"ok":false,"message":"<JSON.parse message>"}` | the same, V8's text for the captured cases (`content::validation::json_parse_message`), else serde's | MATCH (fixed, WW-15) |
| Schema failure | 400 | `{"ok":false,"message":"Invalid JSON body","data":[<AJV errors>]}` | the same, the first AJV 8.17.1 error (`scenes.rs` `schema_coordinates`) | MATCH (fixed, WW-15); the spec asserts only `message` (`scenes-handler.spec.ts:153-156`) |
| A coordinate past the i32 range (`"99999999999,0"`) | 200 (matches the pattern; no scene) | the same | | MATCH (fixed, WW-16) |
| Success, query filters, errors | as GET | as GET | | MATCH |

---

## 4. GET /world/:world_name/settings

### Responses

| trigger | status | upstream body | dcl-one body | verdict |
|---|---|---|---|---|
| Name that is not a valid world key | upstream 404 or 200 | dcl-one 400 `{"error":"Bad request","message":"Invalid world name: …"}` (`permissions::parse_world`) | | DIVERGENT, intended (WW-29) |
| No `worlds` row | 404 | `{"error":"World \"<name>\" not found or has no settings configured."}` (`world-settings-handler.ts:168-174`, `logic/settings/errors.ts:8-13`) | the same (`settings.rs` `get_world_settings`) | MATCH (fixed, WW-17); the spec asserts `error` (`world-settings-handler.spec.ts:233-242`) |
| Success | 200 | `toSnakeCaseSettings` (`world-settings-handler.ts:24-38`) | `settings_json` | MATCH plus two keys, intended (WW-18) |

Success fields:

| key | upstream (`worlds-manager.ts:1236-1262`) | dcl-one | verdict |
|---|---|---|---|
| `title`, `description`, `content_rating`, `spawn_coordinates`, `thumbnail_hash` | the value, or **absent** when NULL or empty | the same | MATCH (fixed, WW-18) |
| `categories` | the array (`[]` kept), or absent when NULL | the same | MATCH (fixed, WW-18) |
| `skybox_time` | number or `null` | same | MATCH |
| `single_player`, `show_in_places` | booleans, NULL read as `false` / `true` | NOT NULL columns with those defaults (`schemas/20_worlds.sql:85-86`) | MATCH |
| `access_type` | `access.type` | `access_type` column | MATCH |
| `settings_version` | number; bumped by every settings write that sets a column and by every access change, not by a spawn-only write (`world-settings-handler.spec.ts:96-199`) | same (`store.rs:1783`, `core/tests.rs:2308-2390`) | MATCH |
| `realm_name_override`, `preview_wearable_urns` | not present | dcl-one only, always present | DIVERGENT, superset, intended (WW-18) |

No deny-list check in either tree.

---

## 5. PUT /world/:world_name/settings

### Request

Upstream chain (`routes.ts:214-233`): `stampRequestArrival`, per-source upload admission (2 MiB), strict signed
fetch, multipart parser (2 MiB payload cap, wire cap 2 MiB + 10 MiB, `repeatableFields: ['categories']`,
busboy limits 100 fields of 1 MiB), then the handler under the shared content lock.

dcl-one order (`settings.rs` `update_world_settings`): world-name parse, signed fetch, `Content-Length` check
against the 12 MiB wire cap, per-source admission, a process-wide concurrent-upload slot, then the body: the
content-type check and the multipart parse under the upload timeout, then authorization, thumbnail store and
the update under the processing deadline. The body is not read before the signed fetch (WW-24).

Fields (upstream `parseMultipartInput`, `world-settings-handler.ts:40-153`; dcl-one `parse_multipart`,
`settings.rs:222-470`):

| field | upstream | dcl-one | verdict |
|---|---|---|---|
| `title` | first value; 3..100 UTF-16 units; empty or out of range is 400 `Invalid title: <v or "title cannot be empty">. Expected between 3 and 100 characters.` | same (`text_len` counts UTF-16 units) | MATCH |
| `description` | same with 3..1000 | same | MATCH |
| `content_rating` | defined (every value non-empty) and in `RP, E, T, A, R`, else 400 `Invalid content rating: <v>. Expected one of: RP, E, T, A, R` | same | MATCH |
| `spawn_coordinates` | defined; `parseCoordinate` (`split(',')` into exactly two, `parseInt`, bounds ±150), else 400 `Invalid spawnCoordinates format: "<v>".`; then inside the row lock: no deployed scene is 400 `Invalid spawnCoordinates "<v>". The world has no deployed scenes.`, outside the shape is 400 `…It must be within the world shape rectangle: (x,y) to (x,y).` | `Parcel::parse` plus bounds (`settings.rs` `parse_coordinate`), same messages (`store.rs:1734,1742`), same lock | DIVERGENT: `parseInt` leniency, intended (WW-25) |
| `skybox_time` | `"null"` clears; else `Number(v)` must be an integer in the i32 range, else 400 `Invalid skybox_time: <v>. Expected an integer between -2147483648 and 2147483647, or null.` | same (`settings.rs:390-400`) | MATCH |
| `categories` | repeatable; `["null"]` clears to `[]`; more than 20 is 400 `Invalid categories: <n> items. Expected at most 20` | same (`settings.rs:418-431`) | MATCH |
| `single_player`, `show_in_places` | defined; `=== 'true'` | same | MATCH |
| `thumbnail` file | over 1 MiB is 400 `Invalid thumbnail: size <n> bytes exceeds maximum of 1048576 bytes (1MB).`; PNG/JPEG/GIF87a/GIF89a/WebP magic, else 400 `Invalid thumbnail: expected a PNG, JPEG, GIF or WebP image.`; stored as SHA-256 hex | same checks (`settings.rs:293-301,455-467`, `settings_policy.rs`); stored as CIDv1 after authorization | MATCH on checks; hash intended (WW-28) |
| a non-repeatable field sent twice | 400 `Duplicate form field '<name>'` (`multipart/middleware.ts:233-240`) | the same (`categories` and the dcl-one `preview_wearable_urns` repeat) | MATCH (fixed, WW-22) |
| `realm_name_override`, `preview_wearable_urns` | ignored | dcl-one only fields | dcl-one superset |

Authorization: upstream `namePermissionChecker.checkPermission` (the NAME owner), else a world-wide
`deployment` grant (`logic/settings/component.ts:49-57`); parcel-scoped grants are refused. dcl-one
`policy.is_owner`, else `has_world_wide_permission(Deployment)` (`settings.rs` `update_world_settings`); an owner lookup
that cannot be decided answers 503 (`ApiError::from(undetermined)`), where upstream's checker throws a 500.
MATCH on the decision.

### Responses

| trigger | status | upstream body | dcl-one body | verdict |
|---|---|---|---|---|
| Unsigned / malformed chain | 400 | `{"error":"Invalid Auth Chain","message":"This endpoint requires a signed fetch request. See ADR-44."}` (`world-settings-handler.spec.ts:849-876`) | the same; the body is read only after the signed fetch | MATCH (WW-24) |
| Validation failure (fields above, and the two spawn refusals from `store::update_world_settings`) | 400 | `{"error":"<message>"}` | the same (`SettingsError::Handler`; `core/tests.rs` `settings_empty_values_follow_the_upstream_handler`) | MATCH (fixed, WW-19) |
| Not owner, no world-wide grant | 403 | `{"error":"Unauthorized. You do not have permission to update settings for this world."}` | the same | MATCH (fixed, WW-20) |
| Body too large (declared `Content-Length` over the wire cap, payload over 2 MiB, a file over 2 MiB, a field over 1 MiB, too many parts or fields) | 413 | `{"error":"Payload Too Large","message":…}` (`multipart/middleware.ts:159-200`, `upload-error-handler.ts:49-51`); the spec pins 413 `An uploaded file exceeds the maximum allowed size.` (`world-settings-handler.spec.ts:1045-1080`) | the same statuses and texts (`settings.rs` `too_large`) | MATCH (fixed, WW-21) |
| Not `multipart/form-data` (after the signed fetch) | 400 | busboy 1.6.0: `{"error":"Bad request","message":"Missing Content-Type"` / `"Unsupported content type: <ct>"` / `"Multipart: Boundary not found"}`; busboy also reads `application/x-www-form-urlencoded` | the same three texts; a urlencoded form is `Unsupported content type` | MATCH; urlencoded intended (worlds-compatibility.md, "Settings uploads") |
| Invalid `Content-Length` | 400 | `{"error":"Bad request","message":"Invalid Content-Length header."}` | same | MATCH |
| Source over its share | 429 + `Retry-After: 5` | `{"error":"Too Many Requests","message":"Too many uploads in progress from this client, please retry shortly."}` | same (`settings.rs:120-126`; keyed on `X-Real-IP`, intended in worlds-compatibility.md) | MATCH |
| In-flight byte or file budget exhausted | 503 + `Retry-After: 5` | `{"error":"Service Unavailable","message":"Server is buffering too many uploads…"}` / `…upload files…` | same messages | MATCH |
| Process-wide concurrent uploads exceeded | dcl-one 503 + `Retry-After: 5` `Server is handling too many concurrent uploads, please retry shortly.` | upstream has no such cap on this route | | DIVERGENT, intended (WW-23) |
| Body not received in time | 408 | `{"error":"Request Timeout","message":"The upload did not finish within <s> s: received …"}` (`describeUploadTimeout`) | the same text over the bytes received (`settings.rs` `describe_upload_timeout`) | MATCH (fixed, WW-23) |
| Processing over the deadline | dcl-one 408 `Deployment processing exceeded the <n>ms deadline.` | upstream has no processing deadline here | | DIVERGENT, intended (WW-23) |
| Writes switch off | dcl-one 503 `{"ok":false,"error":"Service Unavailable","message":…}`, `Retry-After: 3600` | not upstream | | DIVERGENT, intended (WW-27) |
| Success | 200 | `{"message":"World settings updated successfully","settings":<as GET>}` | the same, with the two extra keys (WW-18) | MATCH plus two keys, intended (WW-18) |

Side effects: upstream publishes `WorldSettingsChangedEvent` and, when the spawn changed,
`WorldSpawnCoordinateSetEvent` to SNS (`logic/settings/component.ts:102-134`); dcl-one publishes neither
(WW-26, intended). Upstream holds the shared content lock so a running GC cannot delete a just-stored thumbnail
before its row lands (`routes.ts:223`, `unit/settings-content-lock.spec.ts`); dcl-one's collector has an age
floor (`core/admin.rs:7,66-108`), which closes the same race (WW-30).

---

## 6. GET /world/:world_name/manifest

| trigger | status | body | upstream | dcl-one | verdict |
|---|---|---|---|---|---|
| Invalid world key | dcl-one 400 `Invalid world name: …` | | | `manifest.rs:23` | DIVERGENT, intended (WW-29) |
| Deny-listed | 404 | `{"error":"Not Found","message":"World \"<name>\" not found."}` | `world-manifest-handler.ts:32-35` | `manifest.rs:24-32` | MATCH |
| No occupied parcel | 404 | `…"message":"World \"<name>\" has no scenes deployed."` | `:38-42` | `:33-37` | MATCH |
| Success | 200 | `{"occupied":[…],"spawn_coordinate":{"x":"<x>","y":"<y>"},"total":<n>}` | `:44-58`, `logic/worlds/component.ts:122-141`, `worlds-manager.ts:1552-1600` | `manifest.rs:38-46`, `store.rs:1812-1843` | MATCH: distinct parcels sorted by integer x then y, first 500, `total` the distinct count, spawn `"0,0"` when unset |

---

## 7. GET /world/:world_name/limits

| trigger | status | body | upstream | dcl-one | verdict |
|---|---|---|---|---|---|
| Invalid world key | dcl-one 400 | | | `limits.rs:55` | DIVERGENT, intended (WW-29) |
| `parcels` present but not unique canonical pointers (`?parcels=` included) | 400 | `{"error":"Bad request","message":"Parcels must be unique canonical \"x,y\" coordinates, as in a scene's parcels."}` | `world-limits-handler.ts:38-42` (`SceneParcels.validate`, `@dcl/schemas`) | `limits.rs:28-48`, `deploy_validate.rs:45-61` | MATCH (depends on `SceneParcels`) |
| A parcel out of ±150 | 400 | `…"message":"Coordinate X value <x> is out of bounds. Must be between -150 and 150."` | `:43-49` | `deploy_validate.rs:63-79` | MATCH |
| Deny-listed (checked after the parcels) | 404 | `…"message":"World \"<lowercased>\" not found."` | `:54-56` | `limits.rs:60-69` | MATCH |
| Owner cannot be determined | 404 | `…"message":"Could not determine owner for world <lowercased>"` | `:69-71`, `limits-manager/errors.ts` | `limits.rs:72-91` | MATCH |
| Allowance unavailable | dcl-one 503 `The world's storage allowance could not be read right now` | upstream counts a failed holdings fetch as an error (500) after retries | | `limits.rs:91-96` | DIVERGENT, intended (WW-35) |
| Query over the header limit | upstream 431 from Node's 16 KB header limit (`world-limits-handler.spec.ts:49-62`) | dcl-one: whatever the gateway and hyper allow | | | DIVERGENT, edge (WW-35) |
| Success | 200 | `{"maxSceneSizeInBytes":"<decimal>","maxParcels":<n>}` | `:58-67`, `limits-manager/component.ts:58-87` | `limits.rs:98-101` | MATCH in shape; the size comes from the intended allowance rules (WW-35) |

---

## 8. GET /worlds

### Request

| param | upstream (`worlds-handler.ts:49-104`) | dcl-one (`manifest.rs:51-126`) | verdict |
|---|---|---|---|
| `limit`, `offset` | `getPaginationParams` | `pagination_of` | MATCH |
| `authorized_deployer` | `EthAddress.validate` (no trim, `0x` only) | `Address::parse_untrimmed` behind a `0x` check, as `scenes.rs` | MATCH (fixed, WW-32) |
| `search` | trimmed; empty is absent; over 64 UTF-16 units is 400 `Invalid search parameter: must be at most 64 characters.` | the same (`text_len`) | MATCH (fixed, WW-32) |
| `has_deployed_scenes` | present: `=== 'true'` | same | MATCH |
| `sort` | `name` (default) or `last_deployed_at`, else 400 `Invalid sort parameter: <v>. Valid values are: name, last_deployed_at` | same | MATCH |
| `order` | `asc` (default) or `desc`, else 400 `Invalid order parameter: <v>. Valid values are: asc, desc` | same | MATCH |
| a repeated key | first value | the same (`QueryPairs`, `query_param`) | MATCH (fixed, WW-32) |

### Responses

| trigger | status | body | upstream | dcl-one | verdict |
|---|---|---|---|---|---|
| Success | 200 | `{"worlds":[…],"total":<n>}` | `worlds-handler.ts:97-103`, `worlds-manager.ts:1366-1550` | `manifest.rs:123-125`, `store.rs:1880-1997` | DIVERGENT (fields below) |
| Validation errors | 400 | `{"error":"Bad request","message":…}` | above | above | MATCH |

World objects, key order `name, owner, title, description, shape, content_rating, spawn_coordinates,
skybox_time, categories, single_player, show_in_places, thumbnail_hash, last_deployed_at, blocked_since,
deployed_scenes` in both (`worlds-handler.ts:29-47`, `manifest.rs:133-157`).

| field / behaviour | upstream | dcl-one | verdict |
|---|---|---|---|
| Deny-listed names | `w.name != ALL(<banned names with .eth/.dcl stripped>)` (`worlds-manager.ts:1409-1413`); since a stored name keeps its suffix, this hides only bare-name worlds | the same filter on the page and the count (`store.rs` `list_worlds_public`, `BannedNames::banned_names`) | MATCH (fixed, WW-31) |
| `shape`, `deployed_scenes`, `last_deployed_at` | denormalized columns kept by `recalculateWorldSceneStats`; `null` / `0` / `null` with no scene | computed from `world_scenes` (`store.rs:1894-1905`): the same values | MATCH |
| `blocked_since` | any `blocked` row of the owner, no grace (`worlds-manager.ts:1404`) | `quota_blocks` row only for quota-blockable local DCL NAME and ENS worlds (`store.rs:1907`, `quota.rs:39-45`) | DIVERGENT, intended (WW-33) |
| `owner` | NOT NULL string | `null` for worlds with no owner (bare names, mirrors) | DIVERGENT, intended (WW-33) |
| `single_player`, `show_in_places` | `COALESCE(…, false/true)` | NOT NULL columns | MATCH |
| `categories` | the column (`null` when unset) | same | MATCH |
| timestamps | `Date.toISOString()` | RFC 3339 ms `Z` | MATCH |
| Search | `search_vector` (weighted per column) `@@ plainto_tsquery`, OR `ILIKE '%term%'` unescaped, OR trigram > 0.3 | on-the-fly `to_tsvector(name || title || description)`, escaped ILIKE, trigram only with `pg_trgm` (`store.rs:1846-1878`) | DIVERGENT, intended (worlds-compatibility.md, "`GET /worlds` search"); match sets agree except a literal `%`/`_` |
| Ordering | `ORDER BY w.name` / `last_deployed_at IS NULL, last_deployed_at <dir>, name` | same (`store.rs:1928-1933`) | MATCH (collation of the two databases not compared) |

---

## 9. GET /wallet/:wallet/stats

| trigger | status | body | upstream | dcl-one | verdict |
|---|---|---|---|---|---|
| Not `^0x[a-fA-F0-9]{40}$` | 400 | `{"error":"Bad request","message":"Invalid request. Missing or invalid wallet in request url param."}` | `wallet-stats-handler.ts:11-13` | `wallet.rs:114-130` | MATCH |
| Success | 200 | `{"wallet":<as sent>,"dclNames":[{"name","size"}],"ensNames":[…],"usedSpace":"<n>","maxAllowedSpace":"<n>","blockedSince"?:<ISO>}` | `:15-33`, `adapters/wallet-stats.ts:77-106` | `wallet.rs:119-185`, `quota.rs:119-164` | DIVERGENT, intended (WW-34) |

Shape, ordering (`ORDER BY w.name`), the `wallet` echo and `blockedSince` (absent when not blocked) match.
Which worlds are listed and `maxAllowedSpace` follow the intended allowance rules: upstream lists every world
whose stored owner is the wallet, splitting on the `.dcl.eth` suffix (bare names fall into `ensNames`), and
reads `spaceAllowance` from `DCL_NAME_STATS_URL` (`Number.MAX_SAFE_INTEGER` MiB without it). dcl-one lists the
NAMEs the wallet holds now, drops mirrored, personal and bare-name worlds, and computes the allowance in process
(WW-34).

---

## 10. GET /wallet/contribute

| trigger | status | body | upstream | dcl-one | verdict |
|---|---|---|---|---|---|
| Signed-fetch failure | 400/401 | crypto-middleware shape | `routes.ts:252` | `wallet.rs:48-56` | MATCH (fixed, WW-24) |
| Success | 200 | `{"domains":[{"name","user_permissions":[…],"size":"<n>","owner","parcelCount":<n>}],"count":<n>}` | `contributor-handler.ts:5-16`, `worlds-manager.ts:893-939` | `wallet.rs:42-91` | MATCH except `owner` (WW-36) |

Both select the worlds where the signer holds a grant, with `user_permissions` sorted distinct,
`size` the deployed size as text, and `parcelCount` 0 when any grant is world-wide, else the smallest grant.
dcl-one restricts `permission IN ('deployment','streaming')`, the only two types the table holds in either
tree, and orders by name (upstream: group order). `owner` is `null` for worlds with no stored owner.

---

## Backlog (WW-)

1. **WW-01 · `/about` · `scenesUrn` lists every deployed scene.** Status: MATCH (fixed).
   - Upstream: the newest scene only (`decentraland/worlds-content-server:src/adapters/worlds-manager.ts:403-407`,
     `src/logic/world-runtime-metadata-utils.ts:76-84`); the spec pins one URN for a two-scene world
     (`test/integration/world-about-handler.spec.ts:500-555`). Runtime metadata (realm name, minimap, skybox,
     `fixedAdapter`) comes from that same newest scene.
   - dcl-one: every row of `world_scenes`, newest first (`services/catalyst/worlds/src/queries.rs:88-92`,
     `services/catalyst/worlds/src/about.rs:183-191`); runtime metadata from `scenes[0]`, the newest, as upstream.
   - Fix: in `about.rs::build_about` build `scenes_urn` from `scenes[0]` only (or `LIMIT 1` the scene side of
     `queries::world_about`, keeping `is_locally_published` over all rows). If multi-scene loading is wanted,
     record it in `docs/worlds-compatibility.md` instead.
   - Fixed: `about.rs` `build_about` builds `scenesUrn` from the newest scene only; `is_locally_published` still reads every row. Pinned by `about.rs` `scenes_urn_names_the_newest_scene_only` and `tests/worlds_mirror.rs`.
   - Severity: P1.

2. **WW-02 · `/about` · error envelope.** Status: MATCH (fixed).
   - Upstream: `NotFoundError`/`NotAuthorizedError` through http-commons: `{"error":"Not Found"|"Not Authorized","message":…}`;
     the specs assert `message` (`world-about-handler.spec.ts:30-37,571-578`) and `error: 'Not Authorized'` (`:604-614`).
   - dcl-one: content-server `AppError` → `{"error":<message>}` (`services/catalyst/content/src/errors.rs:74-76`);
     dcl-one's own test pins it (`about.rs:788-821`).
   - Fix: return `core::http::ApiError` from `get_about`/`build_about` (`not_found`, `unauthorized`); update the test.
   - Fixed: `get_about`/`build_about` answer `core::http::ApiError` (`Not Found`, `Not Authorized`); pinned by `about.rs` `a_deny_listed_world_is_404_before_any_read`.
   - Severity: P2 (statuses match).

3. **WW-03 · `/about` · blocked message date.** Status: MATCH (fixed).
   - Upstream: `${blockedSince}` is `Date.prototype.toString()`, e.g. `Tue Oct 06 2026 10:00:00 GMT+0000 (Coordinated Universal Time)`.
   - dcl-one: RFC 3339 with milliseconds, and `by an operator` for an operator disable (`about.rs:163-180`).
   - Fix: format the quota case as `%a %b %d %Y %H:%M:%S GMT+0000 (Coordinated Universal Time)`; keep the operator text (WW-13).
   - Fixed: both block messages format the date as `Date.prototype.toString()` in UTC (`%a %b %d %Y %H:%M:%S GMT+0000 (Coordinated Universal Time)`); the operator text keeps `by an operator` (WW-13). Upstream's text follows its process time zone; production runs in UTC.
   - Severity: P2.

4. **WW-04 · `/about` · `skybox.fixedHour` null and float.** Status: MATCH (fixed).
   - Upstream: the key is absent when no fixed time is set (the spec's `toEqual` body has no `fixedHour`); a set value is a JSON integer.
   - dcl-one: `fixed_hour: Option<f64>` without `skip_serializing_if`, so `null`, and `36000.0` when set (`about.rs:29,255`).
   - Fix: `#[serde(skip_serializing_if = "Option::is_none")]` and serialize an integral value as an integer
     (store it as `serde_json::Number`).
   - Fixed: `SkyboxConfig.fixed_hour` is a `serde_json::Number`, left out when unset; `library/catalyst-api/worlds/SkyboxConfig.ts` and the sites zod schema follow.
   - Severity: P2.

5. **WW-05 · `/about` · legacy `worldConfiguration` keys.** Status: MATCH (fixed).
   - Upstream `migrateConfiguration` (`world-runtime-metadata-utils.ts:5-33`): `dclName` replaces `name`; a truthy
     `minimapVisible` replaces the whole `miniMapConfig` with `{visible}` (dropping `dataImage`/`estateImage`); a
     truthy `skybox` replaces `skyboxConfig` with `{fixedTime: skybox}`.
   - dcl-one `RuntimeMeta::from_scene` (`about.rs:346-411`): no `dclName`, no `skybox`; `miniMapConfig.visible` wins over `minimapVisible`.
   - Fix: port the three rules into `from_scene`. Old SDK6 world deployments carry these keys.
   - Fixed: `RuntimeMeta::from_scene` applies the three `migrateConfiguration` rules (truthy `dclName`, `minimapVisible`, `skybox`); pinned by `about.rs` `runtime_metadata_follows_upstream_migrate_configuration`.
   - Severity: P2.

6. **WW-06 · `/about` · unresolved textures and an empty name.** Status: MATCH (fixed).
   - Upstream: an unresolved texture is `""` in `textures`; `name || worldName` treats `""` as missing.
   - dcl-one: unresolved textures are dropped (`about.rs:388-396`); a `""` name is kept (`about.rs:350-355`).
   - Fix: map unresolved textures to `""`; filter an empty name.
   - Fixed: an unresolved (or non-string, or empty) texture links as `""`; an empty name falls back to the world name. Same test as WW-05.
   - Severity: P2.

7. **WW-07 · `/about` · `spawnCoordinates` fallback.** Status: MATCH (fixed).
   - Upstream: `worlds.spawn_coordinates`, `null` when unset (`worlds-manager.ts:420-427`).
   - dcl-one: falls back to the newest scene's `metadata.scene.base` (`about.rs:212-219`).
   - Fix: drop the fallback, or keep it and record it (deploys set the column in both trees, so it only shows for rows written without it).
   - Fixed: the fallback is dropped; `spawnCoordinates` is the column, `null` when unset.
   - Severity: P2.

8. **WW-08 · `/about` · `publicUrl` trailing slash.** Status: MATCH (fixed).
   - Upstream: `new URL(CONTENT_URL).toString()`: `https://peer.com/content` (spec `world-about-handler.spec.ts:136-139`).
   - dcl-one: `CONTENT_URL`/`LAMBDAS_URL` verbatim, else `<public_url>/content/` and `/lambdas/` (`services/catalyst/content/src/lib.rs:244-247`).
   - Fix: give the worlds `Settings` its own `content_public_url`/`lambdas_public_url` without the trailing slash
     (the content `/about` may keep its own). Clients join paths onto it.
   - Fixed in the worlds `Settings::from_config`: a set `CONTENT_URL`/`LAMBDAS_URL` as configured, this node's own without the trailing slash (the content `/about` keeps its own).
   - Severity: P2.

9. **WW-09 · `/about` · default minimap images.** Status: kept: intended (worlds-compatibility.md, "`/about` configuration").
   - Upstream: `https://api.decentraland.org/v1/minimap.png` and `/v1/estatemap.png`.
   - dcl-one: this node's `/api/v1/minimap.png` unless `MAP_PARCEL_VIEW_URL` is set (`services/catalyst/config/src/config.rs:570-572`),
     and `map_estate_view_url` (`worlds/src/lib.rs:148`, overridden in `serve.rs:511`).
   - Fix: record it in `docs/worlds-compatibility.md` (a self-contained node serves its own map), or default to the upstream URLs.
   - Documented under "`/about` configuration": set `MAP_PARCEL_VIEW_URL` / `MAP_ESTATE_VIEW_URL` to the upstream URLs for a byte-equal answer.
   - Severity: P2.

10. **WW-10 · `/about` · health fields.** Status: Kept by decision, intended (worlds-compatibility.md, "`/about` health").
    - Upstream polls `CONTENT_URL/status` and `LAMBDAS_URL/status`; dcl-one reads its own sync state and reports lambdas healthy.
    - Drop-in: a stock explorer refuses a realm with `acceptingUsers:false`; dcl-one reports false less often, never more. No break.
    - Severity: P2 (by design).

11. **WW-11 · `/about` · offline adapter while the SFU is down.** Status: Kept by decision, intended (worlds-compatibility.md, "Offline comms when the SFU is down").
    - dcl-one: `comms_online && sfu.is_alive()` (`about.rs:226-231,320-333`).
    - Drop-in: a client gets an offline realm instead of a handshake that fails. No break. Depends on the comms area (SFU liveness, the `/worlds/<name>/comms` mint).
    - Severity: P2 (by design).

12. **WW-12 · `/about` · `realmName` strip and override.** Status: Kept by decision, intended (worlds-compatibility.md, "Realm name").
    - dcl-one strips `.dcl.eth` for locally published worlds (`WORLDS_REALM_NAME_STRIP_ENS`, default true) and applies `realm_name_override` (`about.rs:279-318`).
    - Drop-in: a client that compares `realmName` with the world name gets a different string; set
      `WORLDS_REALM_NAME_STRIP_ENS=false` for a byte-equal answer.
    - Severity: P2 (by design).

13. **WW-13 · `/about` · operator disable and live-holder quota block.** Status: Kept by decision, intended (worlds-compatibility.md, "Operator disables apply at once").
    - Severity: P2 (by design).

14. **WW-14 · `/about` · configuration edges.** Status: kept: intended (worlds-compatibility.md, "`/about` configuration").
    - `GLOBAL_SCENES_URN` with repeated spaces: upstream keeps empty elements (`split(' ')`), dcl-one does not (`lib.rs:166-173`).
    - An `ETH_NETWORK` other than mainnet/sepolia: upstream 500, dcl-one `networkId: 1` (`lib.rs:122-127`).
    - Fix: none needed for production values; note in `docs/worlds-compatibility.md` if kept.
    - Documented under "`/about` configuration".
    - Severity: P2.

15. **WW-15 · `POST /scenes` · schema-validator bodies.** Status: MATCH (fixed).
    - Upstream (`@dcl/schema-validator-component` 1.0.1): 415 `{"ok":false,"message":"Content-Type must be application/json"}`;
      400 `{"ok":false,"message":<JSON.parse error>}`; 400 `{"ok":false,"message":"Invalid JSON body","data":[<AJV errors>]}`.
    - dcl-one: the worlds envelope for all three (`services/catalyst/worlds/src/scenes.rs:64-97`).
    - Fix: answer the validator shape from `post_list_scenes`, and build `data` with the AJV-style builder the
      content crate already uses for `POST /entities/active` (content-lambdas CR-2).
    - Fixed: `post_list_scenes` answers the validator's three bodies, keys in its order (`content::validation::ordered_refusal_json`), the parse message from `content::validation::json_parse_message` and the first AJV error from `scenes.rs` `schema_coordinates`, checked against AJV 8.17.1 (`schema_errors_are_the_first_ajv_error`, `core/tests.rs` scenes test). The content crate is layer 6, worlds 9 (`docs/module-layers.toml`).
    - Severity: P2 (the upstream spec checks only `message`).

16. **WW-16 · `POST /scenes` · coordinates past i32.** Status: MATCH (fixed).
    - Upstream: the pattern accepts any digit count; such a coordinate matches no scene (200).
    - dcl-one: `Parcel::parse` refuses it, so 400 (`comms/src/world_identity.rs:337-360`).
    - Fix: in `schema_coordinates`, check the pattern only and keep the raw string when it does not fit i32.
    - Fixed: the pattern alone decides; a coordinate past `i32` is kept as sent and matches nothing (200).
    - Severity: P2.

17. **WW-17 · `GET /settings` · 404 body.** Status: MATCH (fixed).
    - Upstream: `404 {"error":"World \"<name>\" not found or has no settings configured."}`
      (`src/logic/settings/errors.ts:8-13`, `world-settings-handler.ts:168-174`).
    - dcl-one: `{"error":"Not Found","message":"World \"<name>\" not found."}` (`services/catalyst/worlds/src/core/settings.rs:58-61`).
    - Fix: answer `{"error": "<upstream message>"}` (a dedicated response, not `ApiError::not_found`).
    - Fixed: `SettingsError::Handler` answers `{error}` only; pinned in `core/tests.rs` (the `nope.dcl.eth` read).
    - Severity: P2.

18. **WW-18 · `GET`/`PUT /settings` · null vs absent, and extra keys.** Status: MATCH (fixed); the two extra keys kept: intended (worlds-compatibility.md, "Settings superset").
    - Upstream omits `title`, `description`, `content_rating`, `spawn_coordinates`, `thumbnail_hash` when NULL or
      empty, and `categories` when NULL (`worlds-manager.ts:1236-1262`).
    - dcl-one writes `null`, and adds `realm_name_override` and `preview_wearable_urns` (`settings.rs:65-81`).
    - Fix: skip `None` fields in `settings_json`; record the two extra keys as a superset in `docs/worlds-compatibility.md`.
    - Fixed: `settings_json` leaves NULL or empty text and NULL categories out. The sites settings reader (`apps/sites/src/data/lib/catalyst/creator-hub/world-settings.ts`) accepts absent keys.
    - Severity: P2.

19. **WW-19 · `PUT /settings` · 400 body.** Status: MATCH (fixed).
    - Upstream: every validation failure is `400 {"error":"<message>"}` (`world-settings-handler.ts:208-213`); the spec asserts `error`
      for each message (`world-settings-handler.spec.ts:332-1160`).
    - dcl-one: `{"error":"Bad request","message":"<message>"}`; a client that shows `error`, as upstream's shape invites, shows "Bad request".
    - Fix: map the settings-validation `ApiError::bad_request`s (field checks and the two spawn errors from
      `store::update_world_settings`) to `{"error": msg, "message": msg}` in `update_world_settings`; keep http-commons
      400s for the middleware-stage errors (Content-Length) as upstream does.
    - Fixed: the field validations and the two spawn refusals of `store::update_world_settings` answer `400 {error: <message>}`; the parser's refusals (Content-Length, duplicates, content type) keep the http-commons shape, as upstream. Pinned by `core/tests.rs` `settings_empty_values_follow_the_upstream_handler`.
    - Severity: P1 (the settings form's error text).

20. **WW-20 · `PUT /settings` · 403 text.** Status: MATCH (fixed).
    - Upstream: `Unauthorized. You do not have permission to update settings for this world.` in `error` only.
    - dcl-one: `You are not authorized to update the settings of this world.` in `error` and `message` (`settings.rs:167-169`).
    - Fix: use the upstream string.
    - Fixed: `403 {error: "Unauthorized. You do not have permission to update settings for this world."}`.
    - Severity: P2.

21. **WW-21 · `PUT /settings` · size limits answer 400 instead of 413.** Status: MATCH (fixed).
    - Upstream: a declared length over the wire cap, a payload, file or field over its limit, too many parts or fields:
      `413 {"error":"Payload Too Large","message":…}`; a field over 1 MiB says
      `Field '<name>' is too large. The maximum allowed size per field is 1048576 bytes.`
      (`src/controllers/multipart/middleware.ts:159-200,330-336`).
    - dcl-one: `400 {"error":"Bad request",…}` with `The multipart request is too large.` (`settings.rs:105-117,203-205,271-274,313-318`).
    - Fix: answer 413 with the upstream label and messages from these branches (the same helper `POST /entities` would use).
    - Fixed: every size and count branch answers 413 `Payload Too Large` with upstream's text, the field cap naming the field and the fields cap its maximum.
    - Severity: P2.

22. **WW-22 · `PUT /settings` · duplicate fields.** Status: MATCH (fixed).
    - Upstream: a second `title` (any field but `categories`, or any file name seen twice) is
      `400 {"error":"Bad request","message":"Duplicate form field 'title'"}`.
    - dcl-one: keeps every value and reads the first (`settings.rs:334-337,472-474`).
    - Fix: reject a repeated name other than `categories` (and the dcl-one `preview_wearable_urns`) in `parse_multipart`.
    - Fixed: `parse_multipart` refuses a name seen before unless both parts are fields of `categories` or `preview_wearable_urns`; pinned by `settings.rs` `a_repeated_form_field_is_refused`.
    - Severity: P2.

23. **WW-23 · `PUT /settings` · timeouts and the concurrency slot.** Status: MATCH (fixed) for the timeout text; the slot and the processing deadline kept: intended (worlds-compatibility.md, "Settings uploads").
    - Upstream: 408 with `describeUploadTimeout`'s text (`multipart/errors.ts:32-50`); no processing deadline and no
      process-wide upload count on this route.
    - dcl-one: 408 `The multipart upload timed out.`; a processing deadline (408 `Deployment processing exceeded…`) and
      `try_acquire_upload_slot` (503 `Server is handling too many concurrent uploads…`) (`settings.rs:127-186`).
    - Fix: reuse the upstream timeout message; drop the slot and the processing deadline from this route or record them.
    - Fixed: the 408 is `describeUploadTimeout`'s text over the body bytes received and the declared length (`settings.rs` `describe_upload_timeout`, local to the route).
    - Severity: P2.

24. **WW-24 · `PUT /settings`, `/wallet/contribute` · signed-fetch failure bodies.** Status: open, bodies fixed.
    - Upstream: `{"error":<detail>,"message":"This endpoint requires a signed fetch request. See ADR-44."}`, detail
      e.g. `Invalid Auth Chain` (`src/controllers/routes.ts:89-92`, spec `world-settings-handler.spec.ts:849-876`).
    - dcl-one: `map_auth_error_api` → worlds envelope `{"error":"Bad request","message":"Invalid Auth Chain"}`
      (`services/catalyst/comms/src/signed_fetch.rs:187-190`, `shared/src/signed_fetch.rs:76-87`).
    - Fix: one change in `map_auth_error_api` for every strict worlds route; owned by the auth/permissions area.
    - Fixed here: the handler takes the raw request and reads the body only after the signed fetch, so an unsigned request of any content type reaches the signed-fetch refusal. Pinned by `core/tests.rs` `an_unsigned_settings_put_is_refused_by_the_signed_fetch_before_the_body` (status and the signed-fetch text, not the exact body).
    - Severity: P2. Depends on: auth (signed fetch).

25. **WW-25 · `PUT /settings` · `spawn_coordinates` parsing.** Status: kept: intended (worlds-compatibility.md, "Settings uploads").
    - Upstream `parseCoordinate` splits on `,` and uses `parseInt`, so `5abc,3`, `5.9,3` and ` 5,3` pass and are stored as sent.
    - dcl-one: `Parcel::parse` refuses trailing garbage and fractions (`settings.rs:381-388,492-500`).
    - Fix: keep (refusing garbage is safer) and record it, or port `parseInt` semantics.
    - Documented under "Settings uploads".
    - Severity: P2.

26. **WW-26 · `PUT /settings` · no settings events.** Status: kept: intended (worlds-compatibility.md, "Settings events").
    - Upstream publishes `WorldSettingsChangedEvent` and `WorldSpawnCoordinateSetEvent` to SNS (`src/logic/settings/component.ts:102-134`).
    - dcl-one publishes nothing; its explore crate reads the worlds tables directly.
    - Fix: record under "Notifications" in `docs/worlds-compatibility.md`; emit on NATS only if an external consumer needs it.
    - Documented under Notifications, "Settings events".
    - Severity: P2. Depends on: notifications/explore.

27. **WW-27 · `PUT /settings` · the writes switch is off by default.** Status: kept: intended (worlds-compatibility.md, "Writes switch").
    - dcl-one: 503 `{"ok":false,"error":"Service Unavailable","message":…}` with `Retry-After: 3600` until
      `WORLDS_WRITES_ENABLED` is on (`services/catalyst/worlds/src/writes.rs:29-75`).
    - Drop-in: a node started with defaults refuses every settings edit.
    - Fix: list the switch in `docs/worlds-compatibility.md`; operators replacing upstream set it after the restore drill.
    - Documented under "Writes switch", once for every worlds write route (WD-03 too).
    - Severity: P1 (configuration).

28. **WW-28 · `PUT /settings` · thumbnail hash is CIDv1.** Status: Kept by decision, intended (worlds-compatibility.md, "Thumbnails").
    - Upstream stores SHA-256 hex (spec `world-settings-handler.spec.ts:424-447`); dcl-one CIDv1 (`settings.rs:170-178`). Both serve it from `/contents/<hash>`.
    - Severity: P2 (by design).

29. **WW-29 · settings, manifest, limits · invalid world name is 400.** Status: Kept by decision, intended (worlds-compatibility.md, "Invalid world names").
    - Severity: P2 (by design).

30. **WW-30 · `PUT /settings` · content lock.** Status: MATCH, no work.
    - Upstream holds the shared content lock across the thumbnail write and the row update.
    - dcl-one's collector skips blobs younger than its age floor (`services/catalyst/worlds/src/core/admin.rs:7,66-108`), so a just-stored thumbnail cannot be collected before its row lands.
    - Severity: none (closed).

31. **WW-31 · `/worlds` · deny-listed names are listed.** Status: MATCH (fixed).
    - Upstream: `w.name != ALL(getBannedNames())` (`worlds-manager.ts:1409-1413`); with suffixes stripped from the list, this hides bare-name worlds.
    - dcl-one: no filter (`store.rs:1880-1997`).
    - Fix: pass `names.snapshot().banned_names()` to `list_worlds_public` and add `AND w.name <> ALL($n)` to both the page and the count.
      (Hiding `<banned>.dcl.eth` too would be stricter than upstream.)
    - Fixed: `list_worlds_public` adds `w.name <> ALL($5)` to the page and the count with `BannedNames::banned_names()`; pinned by `core/tests.rs` `worlds_listing_reads_the_query_like_url_search_params_and_hides_banned_bare_names`.
    - Severity: P2.

32. **WW-32 · `/worlds` · query parsing.** Status: MATCH (fixed).
    - Repeated keys: serde `Query` answers axum's plain-text 400; upstream reads the first.
    - `authorized_deployer`: `Address::parse` trims and accepts `0X`; upstream's `EthAddress` refuses both.
    - `search`: the 64 limit counts bytes; upstream counts UTF-16 units, so a 30-character non-Latin term is refused here.
    - Fix: in `manifest.rs::get_worlds` read `QueryPairs` with `query_param`, validate with `parse_untrimmed` behind a `0x` check
      (as `scenes.rs` does), and use `settings_policy::text_len` for the search limit.
    - Fixed as proposed; same test as WW-31.
    - Severity: P2.

33. **WW-33 · `/worlds` · `blocked_since` and `owner`.** Status: kept: intended (worlds-compatibility.md, "World listings").
    - Upstream: `blocked_since` is any block of the stored owner; `owner` is never null.
    - dcl-one: only quota-blockable local DCL NAME and ENS worlds show a block (`store.rs:1907`, `quota.rs:39-45`); `owner` is `null` for ownerless worlds.
    - Fix: record both in `docs/worlds-compatibility.md` next to "Operator disables apply at once".
    - Documented under "World listings".
    - Severity: P2.

34. **WW-34 · `/wallet/:wallet/stats` · allowance and listed worlds.** Status: Kept by decision, intended (worlds-compatibility.md, "Wallet storage allowance", bare-name line added).
    - Not in the page: bare-name worlds, which upstream lists under `ensNames`, are dropped (`quota.rs:156-161`).
    - Fix: add the bare-name line to `docs/worlds-compatibility.md`.
    - Severity: P2 (by design).

35. **WW-35 · `/limits` · allowance sources and the header limit.** Status: Kept by decision, intended (worlds-compatibility.md, "Wallet storage allowance", "Per-world limits", "Unowned names").
    - dcl-one answers 503 while the allowance or owner cannot be read; upstream retries the stats service and then fails.
    - The 431 upstream gets from Node's 16 KB header limit for very long `parcels` lists depends on the gateway and hyper here; not compared.
    - Severity: P2 (by design).

36. **WW-36 · `/wallet/contribute` · `owner` null.** Status: kept: intended (worlds-compatibility.md, "World listings").
    - dcl-one returns `owner: null` for worlds with no stored owner (`wallet.rs:23-31`); upstream's column is NOT NULL.
    - Fix: none for owned worlds; record it with WW-33.
    - Documented under "World listings".
    - Severity: P2.

37. **WW-37 · all routes · CORS.** Status: kept: intended (worlds-compatibility.md, "CORS", added by #386).
    - Upstream `Access-Control-Allow-Origin: *`; dcl-one echoes the origin with credentials (`services/catalyst/shared/src/cors.rs:72-79`).
    - Fix: the shared CORS decision (content-lambdas theme "CORS"); no per-route work.
    - No worlds-only change: the CORS layer is shared with the content server; #386 records the echoed origin.
    - Severity: P2. Depends on: shared CORS.

38. **WW-38 · paged routes · offset past i64.** Status: Kept by decision.
    - Upstream (http-commons 2.0.0) passes it to Postgres, which answers 500; dcl-one saturates and answers an empty page (`core/http.rs:31-41`).
    - Severity: P2 (by design).

## Endpoints dcl-one serves in this area that upstream does not

| method | path | handler | note |
|---|---|---|---|
| GET | `/world/{world_name}/preview-wearables` | `worlds/src/preview_wearables.rs` (route `worlds/src/lib.rs:349-352`) | reads the `preview_wearable_urns` setting |
| GET | `/worlds/status` | `worlds/src/status.rs` (route `lib.rs:354`) | worlds status under the root mount |
| GET | `/admin/worlds`, `/admin/worlds/{world_name}`, `…/ban-status`; POST `…/enable`, `…/disable` | `worlds/src/core/admin.rs` (routes `core/mod.rs:680-693`) | operator world reads and the disable that WW-13 refers to |
| GET | `/openapi.json` (under `/worlds-content-server`) | `worlds/src/lib.rs:419-422` | the worlds OpenAPI document |
| PUT fields | `realm_name_override`, `preview_wearable_urns` on `/world/{world_name}/settings` | `core/settings.rs:402-446` | dcl-one-only settings (WW-18) |
