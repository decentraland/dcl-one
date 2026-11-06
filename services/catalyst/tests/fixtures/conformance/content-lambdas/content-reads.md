# Content server: entity and file reads (CR)

Area: the content server's entity reads, file reads and their audit/availability probes, and the
cross-cutting HTTP behaviour of the upstream router (404, 405, CORS, trailing slash, prefix).

## Pinned revisions

- Upstream content server: `decentraland/catalyst` @ `a416565db1e705cff9523540a2578af642598a29` (2026-10-21).
- dcl-one: `dcl-one-conformance` @ `37e91aed` (origin/main, 2026-11-04), Rust workspace `services/catalyst`.
- This tree's gateway, for the rate limits in front of the content routes: `infra/gateway/gateway.py`.

All verdicts below are from reading code (static), except where a row cites a capture under
[`captures/content-reads/`](captures/content-reads/): those state what the production peer answered on
2026-11-04 ([`captures/README.md`](captures/README.md)). Nothing was built or run; no item is
"verified". Library behaviour that is not in the pinned upstream tree (`@dcl/http-server`,
`@dcl/schema-validator-component`, `@dcl/rate-limiter-component`, `@dcl/catalyst-storage`,
`@dcl/schemas`, `@dcl/urn-resolver`, `file-type`) is stated only as far as the upstream code and
its tests pin it; the rest is marked "needs a capture".

## Files read

Upstream (`decentraland/catalyst:`):
- `src/controllers/routes.ts`, `src/service.ts`, `src/components.ts` (CORS 489-506, rate limiter 540-595)
- `src/controllers/handlers/get-entities-handler.ts`, `filter-by-urn-handler.ts`, `active-entities-handler.ts`,
  `get-content-handler.ts`, `get-active-entities-by-deployment-hash-handler.ts`,
  `get-available-content-handler.ts`, `get-audit-handler.ts`
- `src/controllers/utils.ts`, `errors.ts`, `middlewares.ts`, `content-get-rate-limit.ts`,
  `sync-peer-exemption.ts`, `client-source.ts`
- `src/logic/active-entities/component.ts`, `src/logic/deployments/component.ts`,
  `src/logic/query-params/component.ts`, `src/types.ts`, `src/Environment.ts`
- `src/adapters/deployments-repository/component.ts`, `src/adapters/content-files-repository/component.ts`,
  `src/adapters/pointers-repository/component.ts`, `src/migrations/scripts/1764186875999_update-third-parties-materialized-view.ts`
- Tests: `test/integration/controller/active-entities.spec.ts`, `cors.spec.ts`, `entity-type-parsing.spec.ts`,
  `content.spec.ts`, `test/integration/Server.spec.ts`, `test/unit/controllers/filter-by-urn-handler.spec.ts`,
  `content-get-rate-limit.spec.ts`, `utils.spec.ts`

dcl-one (`services/catalyst/`):
- `content/src/lib.rs`, `active_entities.rs`, `get_content.rs`, `available_content.rs`, `audit.rs`,
  `queries.rs`, `formatters.rs`, `validation.rs`, `errors.rs`, `wire_types.rs`, `read_through.rs`, `connection.rs`
- `storage/src/content.rs`; `shared/src/cors.rs`, `shared/src/query_params.rs`, `shared/src/nul_guard.rs`
- `src/serve.rs` (1170-1300); `config/src/node.rs`; `library/dcl-auth/src/hash.rs:176` (outside the workspace)
- Fixtures: `tests/fixtures/content/*.json`

## Endpoint inventory

Upstream production serves every route below under `/content/*` (nginx strips the prefix). dcl-one
serves the same router at the root and under `/content`
(`services/catalyst/content/src/lib.rs:710-711`).

| # | method | upstream path | prod prefix | dcl-one route(s) | status |
|---|---|---|---|---|---|
| 1 | GET (and implicit HEAD) | `/entities/:type` | `/content` | `services/catalyst/content/src/lib.rs:630` -> `active_entities.rs:123` | DIVERGENT |
| 2 | GET | `/entities/active/collections/:collectionUrn` | `/content` | `services/catalyst/content/src/lib.rs:631-634` -> `active_entities.rs:252` | DIVERGENT |
| 3 | POST | `/entities/active` | `/content` | `services/catalyst/content/src/lib.rs:635-638` -> `active_entities.rs:98` | DIVERGENT |
| 4 | GET | `/contents/:hashId` | `/content` | `services/catalyst/content/src/lib.rs:639-642` -> `get_content.rs:90` | GAP |
| 5 | HEAD | `/contents/:hashId` | `/content` | `services/catalyst/content/src/lib.rs:641` -> `get_content.rs:90` | GAP |
| 6 | GET | `/contents/:hashId/active-entities` | `/content` | `services/catalyst/content/src/lib.rs:643-646` -> `active_entities.rs:169` | MATCH |
| 7 | GET | `/available-content` | `/content` | `services/catalyst/content/src/lib.rs:647-650` -> `available_content.rs:14` | DIVERGENT |
| 8 | GET | `/audit/:type/:entityId` | `/content` | `services/catalyst/content/src/lib.rs:651` -> `audit.rs:13` | DIVERGENT |
| X1 | any | unknown path (router miss) | `/content` | `services/catalyst/content/src/lib.rs:623-625,712` | MATCH |
| X2 | wrong method on a known path | any | `/content` | axum `MethodRouter` default | DIVERGENT |
| X3 | OPTIONS (preflight) and CORS on every response | any | `/content` | `services/catalyst/shared/src/cors.rs:20-70` | DIVERGENT |

Counts: 8 endpoints: MATCH 1, GAP 2, DIVERGENT 5, MISSING 0. Cross-cutting: MATCH 1, DIVERGENT 2.

---

## 1. GET /entities/:type

Deprecated upstream in favour of `POST /entities/active` (comment at `decentraland/catalyst:src/controllers/handlers/get-entities-handler.ts:17-20`), still served.

**Request**
- Path `:type`: `parseEntityType` = `trim().toLowerCase()` then an exact lookup in a map of every
  `@dcl/schemas` `EntityType` value plus `value + "s"` for values not ending in `s`
  (`decentraland/catalyst:src/types.ts:213-221`). Accepted: `scene(s)`, `profile(s)`, `wearable(s)`,
  `store(s)`, `emote(s)`, `outfits` (pinned by `test/integration/controller/entity-type-parsing.spec.ts:19-56`:
  `outfits` and `scenes` 200, `outfitss` 400). The type does not filter the result: ids/pointers of
  any type are returned.
- Query (parsed with `qs`, `parameterLimit: 2000`, `arrayLimit: 100`, `depth: 5`,
  `decentraland/catalyst:src/logic/query-params/component.ts:14-19`):
  - `pointer` (repeatable): lowercased; a missing or empty single value (`?pointer=`) counts as absent;
    non-string elements dropped (`component.ts:21-31`). Empty strings inside a multi-value array are kept.
  - `id` (repeatable): as-is.
  - `fields`: comma list of `content|pointers|metadata`, case-insensitive, trimmed; unknown names become
    `undefined` entries; an empty string means no mask (`get-entities-handler.ts:59-63`).
  - Exactly one of `id`/`pointer`; at most 1000 of either (`get-entities-handler.ts:15,44-57`).
- Auth: none. Rate limit: none at the app.

**Responses**

| trigger | status | body | headers | upstream | dcl-one | verdict |
|---|---|---|---|---|---|---|
| type not recognised | 400 | `{"error":"Unrecognized type: <raw :type>"}` | `Content-Type: application/json` | `get-entities-handler.ts:37-42` | `active_entities.rs:130-134` | DIVERGENT: dcl-one rejects `outfits` (`services/catalyst/shared/src/query_params.rs:18,66-72`); upstream accepts it |
| both or neither of id/pointer | 400 | `{"error":"ids or pointers must be present, but not both"}` | json | `get-entities-handler.ts:45-50` | `active_entities.rs:142-147`, `validation.rs:14` | MATCH, except a multi-value list containing an empty element (`?pointer=a&pointer=`): upstream 200, dcl-one 400 (`validation.rs:40-46`) |
| > 1000 ids or pointers | 400 | `{"error":"Too many ids or pointers; the maximum allowed is 1000"}` | json | `get-entities-handler.ts:52-57` | `validation.rs:26-28,49-51` | MATCH |
| success, no `fields` | 200 | `Entity[]` (example below); unknown ids/pointers are omitted; denylisted ids dropped | `Cache-Control: public, max-age=<ENTITIES_CACHE_CONTROL_MAX_AGE>` (default 10; 0 omits the header) | `get-entities-handler.ts:66-77`, `Environment.ts:713-717` | `active_entities.rs:151-164`, `config/src/node.rs:71-72` | MATCH on fields and values; object key order differs (see CR-14) |
| success, `fields=<list>` | 200 | `[{version,id,type,timestamp,pointers,content,metadata?}]`: unselected `pointers`/`content` are `[]`, unselected `metadata` key absent | same | `get-entities-handler.ts:80-95` | `formatters.rs:122-170` | MATCH |
| `fields=` (empty) | 200 | upstream: unmasked entities | | `get-entities-handler.ts:61` (`if (fields)`) | `active_entities.rs:64-70` (`Some("")` -> mask everything) | DIVERGENT: dcl-one returns `pointers: []`, `content: []`, no `metadata` |
| `fields` repeated (`fields=a&fields=b`) | 500 upstream (`.split` on an array) | `{"error":"Internal Server Error"}` | | `get-entities-handler.ts:34,62` | first value used | DIVERGENT (upstream bug; no backlog item) |
| DB error | 500 | `{"error":"Internal Server Error"}` | | `middlewares.ts:72-81` | `errors.rs:52-57,64-72` | MATCH |

Success example (both trees; values fake):

```json
[{"version":"v3","id":"bafkreiexampleentity000000000000000000000000000000000","type":"scene",
  "pointers":["0,0"],"timestamp":1700000000000,
  "content":[{"file":"scene.json","hash":"bafkreiexamplefile0000000000000000000000000000000000"}],
  "metadata":{"display":{"title":"Example"}}}]
```

- `content` items are `{file, hash}`; `[]` when the deployment has no files (upstream
  `decentraland/catalyst:src/logic/deployments/component.ts:192`, dcl-one `services/catalyst/content/src/queries.rs:112-127`).
- `metadata` is the `entity_metadata.v` value; the key is absent when the column is NULL (upstream
  `component.ts:276`, dcl-one `queries.rs:108-110,124-126`).
- `timestamp`: upstream sends `date_part('epoch', entity_timestamp) * 1000` as a JS number (float8);
  dcl-one truncates the same expression to `i64` (`queries.rs:121`). Equal for every value whose
  float8 product is integral.
- Order: upstream returns cache hits first, then DB rows in unspecified order
  (`logic/active-entities/component.ts:294-351`); no ORDER BY (`logic/deployments/component.ts:351-369`).
  dcl-one: ids in plan order, pointers ordered by each entity's smallest matching pointer
  (`queries.rs:179-211`). Upstream order is not a contract; no item.
- Content file order: upstream unordered `SELECT ... FROM content_files WHERE deployment = ANY(...)`
  (`decentraland/catalyst:src/adapters/content-files-repository/component.ts:18-23`); dcl-one
  `ORDER BY key COLLATE "en_US.utf8"` (`queries.rs:136-148`).

**Data dependency**: `content.deployments`, `content.active_pointers`, `content.content_files`, synced
from peers by the sync worker for the types in `SYNC_ENTITY_TYPES`; with `READ_THROUGH_ENABLED` a
miss is filled from `SYNC_SOURCE` (`services/catalyst/content/src/read_through.rs:466-506`). A node
that does not sync a type answers `[]` for it (upstream full nodes hold every type).

---

## 2. GET /entities/active/collections/:collectionUrn

**Request**
- Path: a collection URN or third-party id. Upstream `parseUrn` (`@dcl/urn-resolver`): throws ->
  "Invalid URN format"; parsed type must be `blockchain-collection-v1`, `blockchain-collection-v2`,
  `blockchain-collection-third-party-name`, or `off-chain` with registry `base-avatars`/`base-emotes`
  (`decentraland/catalyst:src/controllers/handlers/filter-by-urn-handler.ts:16-35`).
- Query: `pageSize` default 100, max 1000; `pageNum` default 1; both parsed with `parseInt(_, 10)`
  (`decentraland/catalyst:src/controllers/utils.ts:7-26`).
- Prefix match: `LIKE '<lowercased urn, \ % _ escaped>%'` on `active_pointers` (collections) or on the
  materialized view `active_third_party_collection_items_deployments_with_content` (third party),
  cached 24 h per URN (`logic/active-entities/component.ts:43-65,356-386`,
  `adapters/pointers-repository/component.ts:14-39`). The prefix has no trailing delimiter, so
  `...:collections-v2:0xabc` also matches `0xabcdef...` (both trees).

**Responses**

| trigger | status | body | upstream | dcl-one | verdict |
|---|---|---|---|---|---|
| `parseUrn` throws (e.g. `not-a-real-urn`) | 400 | `{"error":"Invalid URN format: <urn>"}` | `filter-by-urn-handler.ts:16-20` | `active_entities.rs:187-217` (scheme heuristic) | MATCH on the pinned test inputs (`test/unit/controllers/filter-by-urn-handler.spec.ts:102-110`, fixture `tests/fixtures/content/entities-by-collection-400-invalid-urn.json`) |
| parsed but not a collection (LAND, ESTATE, `collections-thirdparty:<tp>:<collection>`, off-chain other registry) | 400 | `{"error":"Invalid collection urn param, it must be a valid urn prefix of a collection or a third party id, instead: '<urn>'"}` | `filter-by-urn-handler.ts:22-35` | `active_entities.rs:191-195,229-249` | MATCH on the pinned cases (`filter-by-urn-handler.spec.ts:33-77`); the hand-written grammar (`KNOWN_NETWORKS`, `collections-v1` only on `ethereum` unless hex) is not proven equal to `@dcl/urn-resolver` for other inputs |
| `pageSize`/`pageNum` not a positive integer | 400 | `{"error":"pageSize must be a positive integer"}` / `{"error":"pageNum must be a positive integer"}` | `utils.ts:11-17` | `shared/src/query_params.rs:136-146` | DIVERGENT: an unparseable or empty value (`pageSize=abc`, `pageSize=`) is NaN -> 400 upstream; dcl-one falls back to the default (`query_params.rs:48-50,136-137`) and answers 200. `pageSize=10abc` is 10 upstream (`parseInt`), default 100 in dcl-one |
| `pageSize` > 1000 | 400 | `{"error":"max allowed pageSize is 1000"}` | `utils.ts:19-21` | `query_params.rs:159-163` | MATCH |
| success | 200 | `{"total": number, "entities": Entity[]}`; `total` counts matching pointers before the denylist filter | `filter-by-urn-handler.ts:40-55` | `active_entities.rs:264-281`, `queries.rs:277-310` | MATCH (entity shape as section 1) |
| no match | 200 | `{"total":0,"entities":[]}` | `logic/active-entities/component.ts:373-375` | `queries.rs:284-290` | MATCH |

Order: upstream pages the id list in `active_pointers` scan order and then re-orders by cache
state (`component.ts:377-380`); dcl-one orders by pointer (`queries.rs:292-300`). Not a contract.

**Data dependency**: `content.active_pointers` and deployments for wearables/emotes. Upstream third-party
lists come from a materialized view refreshed on a schedule (stale until refresh); dcl-one reads
`active_pointers` live, so it can list a fresh third-party item earlier. Both need the wearable and
emote types synced.

---

## 3. POST /entities/active

**Request**
- Body cap: `MAX_ACTIVE_ENTITIES_BODY_SIZE`, default 10 MiB, `createBodySizeLimitMiddleware`
  (`decentraland/catalyst:src/controllers/routes.ts:92`, `src/Environment.ts:79`).
- Body validated by `@dcl/schema-validator-component` with `ensureJsonContentType: false`
  (`routes.ts:41,93`): any `Content-Type` is accepted. Schema `oneOf` of
  `{ids: string[] (minLength 1 each, 1..1000 items)}` and `{pointers: ...same}`, each with
  `additionalProperties: false` (`active-entities-handler.ts:8-23`).
- No query parameters; no `fields` support; pointers lowercased in SQL (`logic/deployments/component.ts:368`).

**Responses**

| trigger | status | body | headers | upstream | dcl-one | verdict |
|---|---|---|---|---|---|---|
| body larger than the cap | 413 | `text/plain; charset=utf-8` `Payload Too Large` (capture `captures/content-reads/CR-17-body-over-10mib.json`) | | `routes.ts:92` | `lib.rs:637` (256 KiB, axum `DefaultBodyLimit`): `text/plain` "Failed to buffer the request body: length limit exceeded" | DIVERGENT: cap 256 KiB vs 10 MiB; body text |
| body not JSON | 400 | `{"ok":false,"message":<V8 JSON.parse message>}`, no `data`. Captured: empty body `Unexpected end of JSON input`; `{` `Expected property name or '}' in JSON at position 1 (line 1 column 2)`; `hello` `Unexpected token 'h', "hello" is not valid JSON` (`captures/content-reads/CR-2-empty-body.json`, `CR-2-not-json.json`, `CR-2-invalid-token.json`) | json | `routes.ts:93` | 400 `{"ok":false,"message":<text>}`, no `data`: `"Unexpected end of JSON input"` for an empty body, otherwise serde's parse message (`validation.rs:74-84`) | DIVERGENT (P2): the empty-body answer matches the capture; any other parse message is serde's wording, not V8's |
| `Content-Type` not `application/json` (e.g. `text/plain` from a bare `fetch`) | validated normally (`text/plain` with `{}` answers the same 400 as JSON `{}`: `captures/content-reads/CR-2-text-plain.json`) | | | `routes.ts:38-41` | the body is read as `Bytes` and parsed whatever the header (`active_entities.rs:98-110`) | MATCH |
| `{}`; both keys; empty array; empty string element; extra property; non-string element; `null` value; non-object body; > 1000 items | 400 | `{"ok":false,"message":"Invalid JSON body","data":<ajv errors>}`: the first error of each `oneOf` branch, then the `oneOf` error, each `{instancePath, schemaPath, keyword, params, message}` in that key order (`captures/content-reads/CR-2-*.json`, eleven cases) | json | `active-entities-handler.ts:8-23` | the same `ok`/`message`, with `data` built as AJV's first error per `oneOf` branch plus the `oneOf` error (`validation.rs:74-215`; fixtures `tests/fixtures/content/active-entities-400-*.json`) | MATCH as JSON values: the `data` arrays `validation.rs` builds, as read, equal all eleven captured arrays. Key order differs: `json!` without `preserve_order` sorts keys (`data,message,ok`; `instancePath,keyword,message,params,schemaPath`) (CR-2) |
| element containing NUL | 500 `{"error":"Internal Server Error"}` (`captures/content-reads/CR-2-nul-element.json`) | | | | 400 `{"error":"ids or pointers must be present, but not both. ..."}` (`validation.rs:103-107`) | DIVERGENT (P2) |
| success | 200 | `Entity[]` as section 1; unknown ids/pointers omitted; denylisted dropped; duplicates collapsed | no `Cache-Control` | `active-entities-handler.ts:34-47` | `active_entities.rs:98-120` | DIVERGENT: dcl-one adds `Cache-Control: public, max-age=10` (`active_entities.rs:119`) |
| success with `?fields=` | 200 | upstream ignores the query: full entities | | `active-entities-handler.ts:34-42` | `active_entities.rs:112-116` masks | DIVERGENT |
| DB error | 500 | `{"error":"Internal Server Error"}` | | `middlewares.ts:72-81` | `errors.rs:52-57` | MATCH |

Order and duplicates: upstream cache-first then DB order; a pointer answered from cache and another
pointer of the same entity answered from the DB can return that entity twice
(`logic/active-entities/component.ts:323-351`). dcl-one returns each entity once. No item.

**Data dependency**: as section 1. Read-through applies to the unmasked path only
(`active_entities.rs:48-62`); the masked path reads the DB directly (`active_entities.rs:72-93`).

---

## 4 and 5. GET and HEAD /contents/:hashId

**Request**
- `:hashId` must pass `IPFSv1.validate || IPFSv2.validate` from `@dcl/schemas`
  (`decentraland/catalyst:src/controllers/handlers/get-content-handler.ts:15-17`); dcl-one uses
  `is_canonical_cid` (`library/dcl-auth/src/hash.rs:176-192`: `Qm` + 44 base58 chars, or `ba` + >= 56
  lowercase base32 chars). Equivalence of the two regexes needs a differential test.
- `?includeMimeType` (presence) turns on MIME sniffing (`get-content-handler.ts:8`).
- Headers: `If-None-Match` (weak compare, `*`), `Range` (`bytes=a-b`, `bytes=a-`, `bytes=-n`; anything
  else is ignored and answers 200).
- Rate limit (upstream only): three buckets per client IP (IPv6 by /64 network, `CONTENT_GET_RATE_LIMIT_IPV6_PREFIX_LENGTH`):
  burst `CONTENT_GET_RATE_LIMIT_MAX` = 1000 per `CONTENT_GET_RATE_LIMIT_WINDOW_SECONDS` = 60, hourly
  `CONTENT_GET_HOURLY_QUOTA_MAX` = 3000 per 3600 s, daily `CONTENT_GET_DAILY_QUOTA_MAX` = 20000 per
  86400 s; burst runs first so a burst rejection does not drain the quotas; the client IP is read from
  `TRUSTED_CLIENT_IP_HEADER` (rightmost, one trusted proxy) else the socket; IPs in
  `TRUSTED_SYNC_PEER_IPS` bypass all three (`decentraland/catalyst:src/controllers/routes.ts:101-114`,
  `content-get-rate-limit.ts:86-121`, `sync-peer-exemption.ts:12-25`, `client-source.ts:17-26`,
  `src/Environment.ts:106-125`, `src/components.ts:545-595`). GET and HEAD share the buckets.
- Rate limit (dcl-one): none in the app. This tree's gateway limits by client IP (`$binary_remote_addr`
  after `real_ip`): `/content/*` takes `limit_req zone=content burst=1000 nodelay` at 100 r/s, and every
  other path, including the root-mounted `/contents/*`, the server-wide `zone=read` at 30 r/s with burst 60
  (`infra/gateway/gateway.py:166,257-260,292`). There is no hourly or daily quota and no exemption. Over the
  limit nginx answers 429 (`limit_req_status 429`) with its own HTML page and no `Retry-After`.

**Responses**

| trigger | status | body | headers | upstream | dcl-one | verdict |
|---|---|---|---|---|---|---|
| over burst/hourly/daily budget | 429 | `{"error":"Too many requests"}` | `Retry-After`, `RateLimit-*` (added by the limiter) | `components.ts:554`, `content-get-rate-limit.ts:99-118` | app: none. Gateway: 100 r/s burst 1000 under `/content/`, 30 r/s burst 60 at the root, nginx HTML 429 without `Retry-After` (`infra/gateway/gateway.py:166,257-260,292`) | DIVERGENT: other budgets (no hourly or daily quota), other body and headers |
| client IP in `TRUSTED_SYNC_PEER_IPS` | never limited | | | `sync-peer-exemption.ts:17-24` | none: the gateway limit applies to sync peers too | MISSING |
| hash fails validation | 404 | `{"error":"No content found with hash <hash>"}` | json | `get-content-handler.ts:15-17` | `get_content.rs:97-99` | MATCH (fixture `tests/fixtures/content/contents-404-garbage-hash.json`) |
| hash denylisted (upstream: its denylist file and `DENYLIST_URLS`, by default Decentraland's ABR list, `Environment.ts:13,513-514`; dcl-one: `content.denylist`, the camera reel's withheld blobs and `DENYLIST_URLS`, Decentraland's ABR list under `NODE_POSTURE=mirror`, `config/src/posture.rs`) | 404 | same | | `get-content-handler.ts:22-24`, `adapters/denylist/component.ts:59-86` | `get_content.rs:97-99`, `content/src/denylist.rs` (`reload_remote`), `storage/src/denylist.rs` | MATCH |
| `If-None-Match` matches `"<hash>"` or is `*` (checked before storage) | 304 | none | `ETag: "<hash>"`, `Cache-Control: public,max-age=31536000,s-maxage=31536000,immutable`, `Access-Control-Expose-Headers: ETag` | `get-content-handler.ts:34-35`, `utils.ts:88-113` | `get_content.rs:101-105`, `formatters.rs:9-33` | MATCH |
| not in storage | 404 | `{"error":"No content found with hash <hash>"}` | json | `get-content-handler.ts:40-43` | `get_content.rs:107-117` | MATCH (dcl-one first tries read-through when attached) |
| Range unsatisfiable (`start >= size`, `start > end`, `bytes=-0`, empty file) | 416 | none | `Content-Range: bytes */<size>`, `Access-Control-Expose-Headers: Content-Range` | `utils.ts:184-195` | `get_content.rs:125-135`, `formatters.rs:41-83` | MATCH |
| Range satisfiable | 206 | the slice (GET), none (HEAD) | base headers below + `Content-Range: bytes a-b/<size>`, `Content-Length: <b-a+1>` | `utils.ts:197-211`, `get-content-handler.ts:52-63` | `get_content.rs:136-159` | MATCH. Upstream `<size>` is the decompressed size of a gzip-stored file (`utils.ts:184`); dcl-one stores raw bytes, so the same number |
| plain GET | 200 | the bytes | `Content-Type: application/octet-stream` (or sniffed), `ETag: "<hash>"`, `Access-Control-Expose-Headers: ETag, Content-Range, Accept-Ranges, Content-Length`, `Accept-Ranges: bytes`, `Cache-Control: public,max-age=31536000,s-maxage=31536000,immutable`, `Content-Length` | `utils.ts:115-135` | `formatters.rs:85-102`, `get_content.rs:193-198` | DIVERGENT: upstream serves a gzip-stored file as stored, with `Content-Encoding: gzip` and the compressed `Content-Length`, regardless of `Accept-Encoding` (`utils.ts:128-133`); dcl-one never sets `Content-Encoding` (`storage/src/content.rs:333-338`). Upstream omits `Content-Length` for a 0-byte file (`if (content.size)`); dcl-one sends `0` |
| plain GET with `STORAGE_X_ACCEL_BASE` set | 200 | empty; nginx serves the file | `X-Accel-Redirect` | n/a | `get_content.rs:177-191` | deploy-dependent; headers nginx emits need a capture |
| plain HEAD | 200 | none | as plain GET | `get-content-handler.ts:62` | `get_content.rs:163-167` | MATCH, except sniffing (next row) |
| `?includeMimeType` | 200/206 | | `Content-Type` from `file-type` on the stream head, else `application/octet-stream` | `utils.ts:137-153` | `get_content.rs:20-61,137-140,169-175` | DIVERGENT: dcl-one detects only png, glb, mp4 (`ftyp`), jpeg, webp, ogg, webm, and answers `application/json` for a body starting with `{`/`[`; `file-type` returns no type for JSON (so `application/octet-stream`) and detects gif, mp3, wav, zip, ktx2 and more; HEAD in dcl-one never sniffs (returns before `get_content.rs:169`) |
| storage fault | 500 | `{"error":"Internal Server Error"}` | | `middlewares.ts:72-81` | `get_content.rs:107`, `errors.rs:95-99` | MATCH |

**Data dependency**: the blob store (`storage/src/content.rs:202-224`), filled by sync for every
content file of synced entities; with read-through a miss is fetched from `SYNC_SOURCE` and
hash-verified before it is stored (`content/src/read_through.rs:510-521`). Content of entity types
outside `SYNC_ENTITY_TYPES` is absent unless read-through fills it.

---

## 6. GET /contents/:hashId/active-entities

**Request**: `:hashId` free text (no validation). No query.

| trigger | status | body | upstream | dcl-one | verdict |
|---|---|---|---|---|---|
| no active deployment references the hash (or all are denylisted) | 404 | `{"error":"The entity was not found"}` | `decentraland/catalyst:src/controllers/handlers/get-active-entities-by-deployment-hash-handler.ts:21-23` | `active_entities.rs:176-178` | MATCH (fixture `contents-active-entities-404-unknown-hash.json`) |
| found | 200 | `string[]` of entity ids, unordered, at most 15000 | `adapters/deployments-repository/component.ts:16,243-254` | `queries.rs:313-331` | MATCH |
| DB error | 500 | `{"error":"Internal Server Error"}` | `middlewares.ts:72-81` | `errors.rs:84-93` | MATCH |

**Data dependency**: `content.content_files` joined with `content.deployments`; no read-through.

---

## 7. GET /available-content

**Request**: `cid` repeatable (`qs`); 1..1000 values (`decentraland/catalyst:src/controllers/handlers/get-available-content-handler.ts:9,24-29`).
No CID validation; denylisted cids are dropped from the answer; existence checked in chunks of 100.

| trigger | status | body | upstream | dcl-one | verdict |
|---|---|---|---|---|---|
| no `cid`, or a single empty `cid=` | 400 | `{"error":"Please set at least one cid."}` | `get-available-content-handler.ts:24-26`, `logic/query-params/component.ts:25` | `available_content.rs:21-23` | DIVERGENT for `cid=`: dcl-one gets `[""]`, answers 200 `[{"cid":"","available":false}]` |
| > 1000 cids | 400 | `{"error":"Too many cids requested; the maximum allowed is 1000."}` | `get-available-content-handler.ts:27-29` | `available_content.rs:24-28` | MATCH |
| success | 200 | `[{"cid": string, "available": boolean}]` in input order | `get-available-content-handler.ts:33-47` | `available_content.rs:30-45` | DIVERGENT: upstream builds a `Map` keyed by cid, so a repeated cid appears once; dcl-one emits one item per input value |
| storage fault | 500 | `{"error":"Internal Server Error"}` | | `available_content.rs:35`, `storage/src/content.rs:139-165` | MATCH (invalid ids are `false` in both, fixture `available-content-200-bogus-cid.json`) |

**Data dependency**: the blob store. No read-through: on a node that syncs a subset of types (or
relies on read-through), cids an upstream full node holds answer `false`.

---

## 8. GET /audit/:type/:entityId

**Request**: `:type` parsed as section 1 (`outfits` accepted). `:entityId` free text.

| trigger | status | body | upstream | dcl-one | verdict |
|---|---|---|---|---|---|
| type not recognised | 400 | `{"error":"Unrecognized type: <raw :type>"}` | `decentraland/catalyst:src/controllers/handlers/get-audit-handler.ts:17-20` | `audit.rs:17-21` | DIVERGENT for `outfits` (dcl-one 400; fixture `tests/fixtures/content/audit-400-outfits-plural-rejected.json` pins the old behaviour) |
| no deployment with that id **and that type** | 404 | `{"error":"No deployment found"}` | `get-audit-handler.ts:22-30` (filters `entityIds` and `entityTypes`, includes overwritten and denylisted) | `audit.rs:23-25`, `queries.rs:883-925` | DIVERGENT: dcl-one ignores the type (`WHERE d.entity_id = $1`, `queries.rs:905`), so `/audit/profile/<scene id>` is 200 in dcl-one, 404 upstream |
| found | 200 | `{"version":"v3","localTimestamp":number,"authChain":[{"type":string,"payload":string,"signature":string}]}`; `overwrittenBy`, `isDenylisted`, `denylistedContent` are never set by the query (`adapters/deployments-repository/component.ts:84-98` selects no `overwritten_by`), so they are absent | `get-audit-handler.ts:32-44` | `queries.rs:913-925` | MATCH on fields; dcl-one adds `localProvenance` for entities published to its write API |
| DB error | 500 | `{"error":"Internal Server Error"}` | `middlewares.ts:72-81` | `errors.rs:84-93` | MATCH |

Example (fake values): `{"version":"v3","localTimestamp":1700000000000,"authChain":[{"type":"SIGNER","payload":"0x0000000000000000000000000000000000000001","signature":""}]}`.

**Data dependency**: `content.deployments.auth_chain` and `local_timestamp`. `localTimestamp` is the
time this node stored the deployment, so it differs between any two nodes (upstream peers too).

---

## X1. Unknown route

- Upstream: no route matches -> `router.allowedMethods()` and then the `@dcl/http-server` default
  answer (`decentraland/catalyst:src/service.ts:63-65`). Captured: `404`, `Content-Type: text/plain;
  charset=utf-8`, body `Not found` (`Content-Length: 9`), no CORS headers without `Origin`
  (`captures/content-reads/CR-13-unknown-route.json`).
- dcl-one: `404 text/plain "Not found"` (`services/catalyst/content/src/lib.rs:623-625,712`); axum gives a
  `&str` body `text/plain; charset=utf-8`. It adds the CORS and `X-Content-Type-Options: nosniff` headers.
- Verdict: MATCH on status, content type and body, by the capture (CR-13).

## X2. Wrong method on a known path

- Upstream: `router.allowedMethods()` (koa-router port) answers 405 with `Allow` for a method the
  router knows, and 501 for a method it does not (`decentraland/catalyst:src/service.ts:64-65`).
  Captured: `PUT` and `PATCH /content/status` answer `405`, empty body, `Content-Length: 0`,
  `Allow: HEAD, GET`; `DELETE /content/entities/active` answers `405` with `Allow: HEAD, GET, POST`
  (the `GET` comes from `/entities/:type`) (`captures/content-reads/CR-12-*.json`). A method outside the
  router's list (`FOO`) is answered by the CDN with an empty 400 before it reaches the peer, so the 501
  is not observable on the public peer (`CR-12-unknown-method.json`).
- dcl-one: axum answers 405 with `Allow` and an empty body for any method. axum writes `Allow` as the
  registered methods joined by `,` without spaces (`GET,HEAD`), where upstream writes `HEAD, GET`.
- Path shadowing: upstream `GET /entities/active` matches `/entities/:type` and answers 400
  `{"error":"Unrecognized type: active"}` (captured: `captures/content-reads/CR-12-get-entities-active.json`); in dcl-one the static `/entities/active` route (POST only)
  wins and answers 405 (`services/catalyst/content/src/lib.rs:630-638`).
- Verdict: DIVERGENT (CR-12).

## X3. CORS

| aspect | upstream (`decentraland/catalyst:src/components.ts:489-506`, `test/integration/controller/cors.spec.ts`) | dcl-one (`services/catalyst/shared/src/cors.rs`) |
|---|---|---|
| `Access-Control-Allow-Origin` | `*` | the request `Origin` echoed; absent when the request has no `Origin` (`cors.rs:56-58,72-79`) |
| `Access-Control-Allow-Credentials` | not sent | `true` (`cors.rs:75-78`) |
| `Access-Control-Expose-Headers` | `*`; the content handler's own list wins on `/contents` (`content.spec.ts:62-70`) | `*` unless the handler set one (`cors.rs:60-65`) |
| preflight status | 204 | 204 (`cors.rs:28-52`) |
| `Access-Control-Allow-Methods` | `GET,HEAD,POST,OPTIONS` | `GET,HEAD,POST,PUT,DELETE,PATCH,OPTIONS` (`cors.rs:15`) |
| `Access-Control-Allow-Headers` | reflects `Access-Control-Request-Headers` | reflects it; a fixed list when absent (`cors.rs:17,38-41`) |
| `Access-Control-Max-Age` | `600` | `86400` (`cors.rs:18`) |
| `Vary` | `Access-Control-Request-Headers` on preflight (`cors.spec.ts:52-54`) | `Origin` (`cors.rs:46,59`) |
| extra | none from the application code; every response captured from the public peer carried `X-Content-Type-Options: nosniff`, so a layer in front of it adds the header ([`captures/README.md`](captures/README.md), Sanitization) | `X-Content-Type-Options: nosniff` on every response (`cors.rs:81-86`) |

The upstream column is confirmed by capture: the preflight for `POST /content/entities/active` answered
`204` with `access-control-allow-headers: content-type`, `access-control-allow-methods: GET,HEAD,POST,OPTIONS`,
`access-control-allow-origin: *`, `access-control-expose-headers: *`, `access-control-max-age: 600`,
`vary: Access-Control-Request-Headers`; a `GET /content/status` with `Origin` answered
`access-control-allow-origin: *` and `access-control-expose-headers: *`
(`captures/content-reads/CR-11-preflight.json`, `CR-11-get-with-origin.json`).

Verdict: DIVERGENT, browser-compatible in both directions (CR-11).

## Trailing slash, case and prefix

- Trailing slash: upstream's router (path-to-regexp, `strict: false` in the koa-router port it uses)
  matches `/entities/scene/`: captured, `GET /content/entities/scene/?pointer=0,0` answers the same bytes
  as the path without the slash (`captures/content-reads/CR-16-trailing-slash-entities.json`). dcl-one trims a trailing slash before
  routing (`services/catalyst/src/serve.rs:1355`). MATCH.
- Case: the same router port defaults to `sensitive: false`. Captured: `HEAD /content/Contents/<hash>`
  answers `200` with the file's headers, and `/content/Status` and `/content/Deployments` answer `200`
  (`captures/content-reads/CR-16-path-case-contents.json`, `CR-16-path-case-status.json`,
  `captures/content-sync/CS-23-path-case-deployments.json`). axum is case-sensitive (404). DIVERGENT (CR-16).
- Prefix: upstream production serves only `/content/*`; dcl-one answers the same routes at the root
  and under `/content` (`services/catalyst/content/src/lib.rs:710-711`). A superset.
- NUL bytes: dcl-one answers any path or query containing a NUL (raw or `%00`) with
  `400 {"error":"request path or query contains a NUL byte"}` (`services/catalyst/shared/src/nul_guard.rs:12-22`);
  upstream lets it reach the handler (404 on `/contents`, a DB error elsewhere).

---

## Backlog

1. **CR-1** POST /entities/active: any `Content-Type` is parsed as JSON. Status: MATCH, no work.
   - Upstream: the body is parsed as JSON whatever the header (`ensureJsonContentType: false`,
     `decentraland/catalyst:src/controllers/routes.ts:38-41`).
   - dcl-one: the handler takes `Bytes` and parses them itself (`services/catalyst/content/src/active_entities.rs:98-110`).
   - Severity: none (closed).

2. **CR-2** POST /entities/active: confirm the schema-validator `data` array and parse messages.
   - Gap: dcl-one answers `{"ok":false,"message":"Invalid JSON body","data":[...]}` for every schema violation, with
     `data` rebuilt from AJV's rules (first error per `oneOf` branch, then the `oneOf` error). That array is not
     confirmed by a capture. A body that is not JSON answers `{"ok":false,"message":<serde text>}` (V8's text only for an
     empty body), and an element containing NUL answers the older `{"error":...}` sentence.
   - Expected: the validator component's output (`decentraland/catalyst:src/controllers/handlers/active-entities-handler.ts:8-23`,
     `test/integration/controller/active-entities.spec.ts:28-54`).
   - Capture (`captures/content-reads/CR-2-*.json`): the eleven schema cases answer the `data` arrays that
     `validation.rs` builds, as read, value for value. What still differs: key order (upstream `ok,message,data` and
     `instancePath,schemaPath,keyword,params,message`; dcl-one sorts keys), V8's parse messages for a non-empty
     body that is not JSON (`Expected property name or '}' in JSON at position 1 (line 1 column 2)`,
     `Unexpected token 'h', "hello" is not valid JSON`), and a NUL element, which upstream answers
     `500 {"error":"Internal Server Error"}`. Pin the captured bodies in `tests/fixtures/content/active-entities-400-*.json`.
   - Place: `services/catalyst/content/src/validation.rs:74-215`.
   - Severity: P2.

3. **CR-3** GET/HEAD /contents/:hashId: add the content-get rate limit.
   - Gap: no limit in the app. The gateway applies `limit_req` per client IP to every path: 100 r/s with burst
     1000 under `/content/`, 30 r/s with burst 60 at the root (`infra/gateway/gateway.py:166,257-260,292`).
     It has no hourly or daily quota, and its 429 is nginx's HTML page without `Retry-After` or `RateLimit-*`.
   - Expected: per client IP (IPv6 per /64), burst 1000/60 s, then hourly 3000, then daily 20000, each a
     separate bucket, burst first; 429 `{"error":"Too many requests"}` with `Retry-After` and `RateLimit-*`;
     client IP from `TRUSTED_CLIENT_IP_HEADER` (rightmost) else the socket; every knob an env var with the
     upstream name (`decentraland/catalyst:src/controllers/content-get-rate-limit.ts:86-121`,
     `src/Environment.ts:106-125`).
   - Place: a layer on the two routes at `services/catalyst/content/src/lib.rs:639-642`; client IP via
     `services/catalyst/shared/src/client_ip.rs`.
   - Severity: P1.

4. **CR-4** GET/HEAD /contents/:hashId: trusted sync peer bypass.
   - Gap: no bypass exists, and the gateway already throttles a syncing peer: `zone=content` (100 r/s, burst
     1000 per IP) on `/content/contents/:hashId` and the `/content/*` sync routes, `zone=read` (30 r/s, burst 60)
     on the root mounts (`infra/gateway/gateway.py:166,257-258,292`). Upstream never limits a peer listed in
     `TRUSTED_SYNC_PEER_IPS`, and limits no sync route.
   - Expected: canonical IPs listed in `TRUSTED_SYNC_PEER_IPS` (comma list; invalid entries dropped with a
     warning) skip all three buckets (`decentraland/catalyst:src/controllers/sync-peer-exemption.ts:12-25`,
     `src/components.ts:580-595`).
   - Place: same layer as CR-3, plus a gateway exemption for the same IPs (a `geo` map that blanks the
     `limit_req_zone` key) in `infra/gateway/gateway.py:257-258`.
   - Severity: P1.

5. **CR-5** GET /entities/:type and GET /audit/:type/:entityId: accept `outfits`.
   - Gap: `DEPLOYABLE_ENTITY_TYPES` omits `outfits`, so both answer 400.
   - Expected: `outfits` accepted, `outfit` and `outfitss` rejected; `scene|profile|wearable|store|emote`
     with or without one `s` (`decentraland/catalyst:src/types.ts:212-221`,
     `test/integration/controller/entity-type-parsing.spec.ts:19-56`).
   - Place: `services/catalyst/shared/src/query_params.rs:18,66-72`. Look the raw value up in a table of the exact
     type names plus `<type>s` for each type that does not end in `s`, as upstream builds `ENTITY_TYPES_BY_NAME`.
     `parse_entity_type` (`query_params.rs:58-62`) is not that table: it strips one trailing `s` from any value, so
     it accepts `outfitss`; fix it the same way, since `/deployments` and `/pointer-changes` use it;
     `content/src/active_entities.rs:130`, `content/src/audit.rs:17`; replace fixtures
     `tests/fixtures/content/entities-400-outfits-rejected.json`, `audit-400-outfits-plural-rejected.json`.
   - Severity: P1.

6. **CR-6** GET /audit/:type/:entityId: filter by type.
   - Gap: the query ignores the type.
   - Expected: 404 `No deployment found` unless a deployment has that id and that type
     (`decentraland/catalyst:src/controllers/handlers/get-audit-handler.ts:22-30`).
   - Place: `services/catalyst/content/src/queries.rs:883-907` (add `AND d.entity_type = $2`), `content/src/audit.rs:23`.
   - Severity: P1.

7. **CR-7** GET /contents/:hashId?includeMimeType: sniff like `file-type`.
   - Gap: a short list of signatures; JSON reported as `application/json`; HEAD never sniffs.
   - Expected: the `file-type` result on the stream head (gif, mp3, wav, zip, png, jpeg, webp, glb, mp4,
     ogg, webm, ...), `application/octet-stream` when it finds none (JSON and text included), on GET and
     HEAD (`decentraland/catalyst:src/controllers/utils.ts:137-153`, `get-content-handler.ts:54`).
   - Place: `services/catalyst/content/src/get_content.rs:20-61,163-167`.
   - Severity: P1.

8. **CR-8** POST /entities/active: no `Cache-Control`, no `fields`.
   - Gap: dcl-one adds `Cache-Control: public, max-age=10` and honours `?fields=`.
   - Expected: no `Cache-Control` header; the query string ignored, full entities returned
     (`decentraland/catalyst:src/controllers/handlers/active-entities-handler.ts:36-47`).
   - Place: `services/catalyst/content/src/active_entities.rs:112-119`.
   - Severity: P2.

9. **CR-9** GET /entities/:type: `fields` and empty elements.
   - Gap: `fields=` masks everything; `pointer=a&pointer=` (or `id=a&id=`) answers 400.
   - Expected: an empty `fields` means no mask (`get-entities-handler.ts:61`). `qsGetArray` treats only a
     missing key or a single empty value (`?pointer=`) as absent; empty strings inside a multi-value list are
     kept and simply match nothing. The 400 comes only when both `id` and `pointer` are absent in that sense,
     or both are present (`decentraland/catalyst:src/logic/query-params/component.ts:21-31`).
   - Place: `services/catalyst/content/src/active_entities.rs:64-70,136-147`.
   - Severity: P2.

10. **CR-10** GET /entities/active/collections/:collectionUrn: `parseInt` pagination.
    - Gap: unparseable or empty `pageSize`/`pageNum` fall back to defaults; `10abc` is not 10.
    - Expected: present but NaN -> 400 `pageSize must be a positive integer` / `pageNum must be a positive integer`;
      a numeric prefix is used (`10abc` -> 10, `1.5` -> 1) (`decentraland/catalyst:src/controllers/utils.ts:7-26`).
    - Place: `services/catalyst/shared/src/query_params.rs:130-185` (a JS-`parseInt` reader for this route).
    - Severity: P2.

11. **CR-11** CORS headers.
    - Gap: origin echoed with credentials, wider method list, max-age 86400, `Vary: Origin`, nothing without `Origin`.
    - Expected: `Access-Control-Allow-Origin: *` on every response, no `Allow-Credentials`,
      `Access-Control-Expose-Headers: *`, preflight 204 with `Allow-Methods: GET,HEAD,POST,OPTIONS`,
      reflected `Allow-Headers`, `Max-Age: 600`, `Vary: Access-Control-Request-Headers`
      (`decentraland/catalyst:src/components.ts:492-505`, `test/integration/controller/cors.spec.ts:20-114`;
      confirmed by `captures/content-reads/CR-11-preflight.json` and `CR-11-get-with-origin.json`).
    - Place: `services/catalyst/shared/src/cors.rs:15-79` (or a content-only variant if other services need the current rule).
    - Severity: P2.

12. **CR-12** Method handling.
    - Gap: `GET /entities/active` is 405 (upstream 400 `Unrecognized type: active`); unknown methods are 405 (upstream 501).
    - Expected: `GET /entities/active` handled by the `/entities/:type` handler; 405 with `Allow` only for
      methods the router knows (`decentraland/catalyst:src/controllers/routes.ts:86-95`, `src/service.ts:64-65`).
    - Capture: `GET /content/entities/active` answers 400 `{"error":"Unrecognized type: active"}`; `PUT`/`PATCH`
      answer 405 with an empty body and `Allow: HEAD, GET` (comma and space, `HEAD` first); the public CDN
      answers an unknown method with its own 400, so behind that CDN the 501 is never seen
      (`captures/content-reads/CR-12-*.json`).
    - Place: `services/catalyst/content/src/lib.rs:630-638` (add `.get(active_entities::get_entities)` on
      `/entities/active` with `type = "active"`).
    - Severity: P2.

13. **CR-13** Unknown-route 404 body. Status: MATCH, no work.
    - Gap: dcl-one `text/plain "Not found"`; upstream body unknown from the pinned tree.
    - Expected: the `@dcl/http-server` default answer.
    - Capture: `404 text/plain; charset=utf-8` `Not found` (`captures/content-reads/CR-13-unknown-route.json`),
      the same status, type and body dcl-one answers. No code change is needed.
    - Place: `services/catalyst/content/src/lib.rs:623-625`.
    - Severity: none (closed).

14. **CR-14** Entity JSON key order.
    - Gap: dcl-one serialises entity objects, `metadata` and `authChain` links with keys in alphabetical
      order (serde_json without `preserve_order`, `services/catalyst/Cargo.toml:68`).
    - Expected: `version,id,type,pointers,timestamp,content,metadata`
      (`decentraland/catalyst:src/logic/deployments/component.ts:186-194`); masked
      `version,id,type,timestamp,pointers,content,metadata` (`get-entities-handler.ts:94`); `metadata` in
      PostgreSQL jsonb key order as stored.
    - Capture: the production peer answers `POST /content/entities/active` with
      `id,type,pointers,content,version,timestamp,metadata` (a wearable and the scene at 0,0), and
      `GET /content/entities/scene?pointer=0,0` with `version,id,type,timestamp,pointers,content,metadata`
      (`captures/content-reads/CR-14-active-entity-key-order.json`, `CR-16-trailing-slash-entities.json`).
      The `POST` order is neither of the orders read above; trace where it comes from before fixing.
    - Place: `services/catalyst/content/src/queries.rs:112-127` (build the document bytes in order).
    - Severity: P2.

15. **CR-15** /available-content duplicates and empty cid.
    - Gap: a repeated cid is answered twice; `cid=` is 200.
    - Expected: one item per distinct cid in first-seen order; `cid=` alone is 400 `Please set at least one cid.`
      (`decentraland/catalyst:src/controllers/handlers/get-available-content-handler.ts:22-47`).
    - Place: `services/catalyst/content/src/available_content.rs:19-43`.
    - Severity: P2.

16. **CR-16** Case-insensitive paths.
    - Gap: axum matches paths case-sensitively.
    - Expected: the upstream router's case-insensitive match. Confirmed by capture: `/content/Contents/<hash>`,
      `/content/Status` and `/content/Deployments` answer 200 (`captures/content-reads/CR-16-path-case-contents.json`, `CR-16-path-case-status.json`,
      `captures/content-sync/CS-23-path-case-deployments.json`).
    - Place: a path-lowercasing layer for the static segments in `services/catalyst/content/src/lib.rs:695-727`.
    - Severity: P2.

17. **CR-17** POST /entities/active body cap.
    - Gap: 256 KiB, axum text 413.
    - Expected: `MAX_ACTIVE_ENTITIES_BODY_SIZE`, default 10 MiB; above it `413 text/plain; charset=utf-8`
      `Payload Too Large` (captured: `captures/content-reads/CR-17-body-over-10mib.json`; only the over-cap side
      was sent, so the exact cap is the pinned tree's default, not a measurement)
      (`decentraland/catalyst:src/Environment.ts:79`, `src/controllers/routes.ts:92`).
    - Place: `services/catalyst/content/src/lib.rs:637`.
    - Severity: P2.

18. **CR-18** GET /contents/:hashId gzip passthrough.
    - Gap: never `Content-Encoding: gzip`; `Content-Length: 0` sent for empty files.
    - Expected: a file stored gzip-compressed is sent compressed with `Content-Encoding: gzip` and its compressed
      `Content-Length`; no `Content-Length` when the size is 0 (`decentraland/catalyst:src/controllers/utils.ts:128-133`).
      The storage comment (`services/catalyst/storage/src/content.rs:333-338`) rules out serving unverified
      `.gzip` siblings; an equivalent needs a verified compressed copy.
    - Place: `services/catalyst/content/src/get_content.rs:193-198`, `storage/src/content.rs`.
    - Severity: P2.

19. **CR-19** Entity `timestamp` and audit `localTimestamp` as float.
    - Gap: `as i64` truncates the float8 `date_part(...) * 1000`.
    - Expected: the float8 value as a JSON number (integral in the common case)
      (`decentraland/catalyst:src/logic/deployments/component.ts:357`).
    - Place: `services/catalyst/content/src/queries.rs:121,917` (round, or emit the f64 when it is not integral).
    - Severity: P2.

20. **CR-20** Differential tests for the hand-written validators.
    - Gap: `is_valid_collection_urn` and `is_canonical_cid` replace `@dcl/urn-resolver` and `@dcl/schemas`
      `IPFSv1/IPFSv2`; equality is shown only for the upstream test inputs.
    - Expected: the same accept/reject and message for a corpus of URNs (networks, `collections-v1` names
      on non-ethereum networks, upper case) and hashes (`bafy`/`bafk`/other `ba` prefixes, lengths).
    - Place: `services/catalyst/content/src/active_entities.rs:183-249`, `library/dcl-auth/src/hash.rs:176-192`; fixtures
      under `services/catalyst/tests/fixtures/content/` captured from a peer.
    - Severity: P2.

## dcl-one routes in this area that upstream does not serve

- The whole content router at the root as well as under `/content` (`services/catalyst/content/src/lib.rs:710-711`).
- `POST /entities/active?fields=...` masking (`services/catalyst/content/src/active_entities.rs:112-116`).
- Read-through fills on entity and content misses (`READ_THROUGH_ENABLED`, `services/catalyst/content/src/read_through.rs:466-521`).
- `localProvenance` in `/audit` for entities published to this node (`services/catalyst/content/src/queries.rs:919-923`).
- `Connection: keep-alive` / `Keep-Alive: timeout=70` (`content/src/connection.rs:16-30`;
  `X-Content-Type-Options: nosniff` from `services/catalyst/shared/src/cors.rs:81-86` is not here because the
  public peer sends it too, see X3), the NUL-byte 400
  (`services/catalyst/shared/src/nul_guard.rs:12-22`), and a 408 when `request_timeout` is configured
  (`services/catalyst/content/src/lib.rs:720-725`).
- `X-Accel-Redirect` delivery when `STORAGE_X_ACCEL_BASE` is set (`services/catalyst/content/src/get_content.rs:177-191`).
