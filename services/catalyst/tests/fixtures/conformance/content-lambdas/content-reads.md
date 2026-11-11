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
| 4 | GET | `/contents/:hashId` | `/content` | `services/catalyst/content/src/lib.rs:639-642` -> `get_content.rs:90` | DIVERGENT |
| 5 | HEAD | `/contents/:hashId` | `/content` | `services/catalyst/content/src/lib.rs:641` -> `get_content.rs:90` | DIVERGENT |
| 6 | GET | `/contents/:hashId/active-entities` | `/content` | `services/catalyst/content/src/lib.rs:643-646` -> `active_entities.rs:169` | MATCH |
| 7 | GET | `/available-content` | `/content` | `services/catalyst/content/src/lib.rs:647-650` -> `available_content.rs:14` | MATCH |
| 8 | GET | `/audit/:type/:entityId` | `/content` | `services/catalyst/content/src/lib.rs:651` -> `audit.rs:13` | MATCH |
| X1 | any | unknown path (router miss) | `/content` | `services/catalyst/content/src/lib.rs:623-625,712` | MATCH |
| X2 | wrong method on a known path | any | `/content` | axum `MethodRouter` default | DIVERGENT |
| X3 | OPTIONS (preflight) and CORS on every response | any | `/content` | `services/catalyst/shared/src/cors.rs:20-70` | DIVERGENT |

Counts: 8 endpoints: MATCH 3, GAP 0, DIVERGENT 5, MISSING 0. Cross-cutting: MATCH 1, DIVERGENT 2.

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
| type not recognised | 400 | `{"error":"Unrecognized type: <raw :type>"}` | `Content-Type: application/json` | `get-entities-handler.ts:37-42` | `active_entities.rs:130-134`, `shared/src/query_params.rs` `parse_entity_type` | MATCH (fixtures `entities-200-outfits-accepted.json`, `entities-400-outfitss-rejected.json`, `entities-400-outfit-rejected.json`) |
| both or neither of id/pointer | 400 | `{"error":"ids or pointers must be present, but not both"}` | json | `get-entities-handler.ts:45-50` | `active_entities.rs` `entities_selector`, `validation.rs:14` | MATCH: a multi-value list containing an empty element (`?pointer=a&pointer=`) selects, and the empty element matches nothing (CR-9) |
| > 1000 ids or pointers | 400 | `{"error":"Too many ids or pointers; the maximum allowed is 1000"}` | json | `get-entities-handler.ts:52-57` | `validation.rs:26-28,49-51` | MATCH |
| success, no `fields` | 200 | `Entity[]` (example below); unknown ids/pointers are omitted; denylisted ids dropped | `Cache-Control: public, max-age=<ENTITIES_CACHE_CONTROL_MAX_AGE>` (default 10; 0 omits the header) | `get-entities-handler.ts:66-77`, `Environment.ts:713-717` | `active_entities.rs:151-164`, `config/src/node.rs:71-72` | MATCH on fields, values and top-level key order (`maskEntity`'s, CR-14); `metadata`'s inner key order differs (CR-14) |
| success, `fields=<list>` | 200 | `[{version,id,type,timestamp,pointers,content,metadata?}]`: unselected `pointers`/`content` are `[]`, unselected `metadata` key absent | same | `get-entities-handler.ts:80-95` | `formatters.rs:122-170` | MATCH |
| `fields=` (empty) | 200 | upstream: unmasked entities | | `get-entities-handler.ts:61` (`if (fields)`) | `active_entities.rs` `requested_fields` (empty is no mask) | MATCH (CR-9) |
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
- Key order: `GET /entities/:type` masks every entity, so its keys are `maskEntity`'s
  `version,id,type,timestamp,pointers,content,metadata`; `POST /entities/active` and the collection
  listing answer the stored entity, captured as `id,type,pointers,content,version,timestamp,metadata`.
  dcl-one writes both orders (`formatters.rs` `MaskedEntity`, `queries.rs` `ActiveEntityJson`).
  `metadata`'s own keys are sorted in dcl-one (CR-14).
- `timestamp`: upstream sends `date_part('epoch', entity_timestamp) * 1000` as a JS number (float8);
  dcl-one sends the same double, as an integer when it is integral (`queries.rs` `js_millis`).
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
| `pageSize`/`pageNum` not a positive integer | 400 | `{"error":"pageSize must be a positive integer"}` / `{"error":"pageNum must be a positive integer"}` | `utils.ts:11-17` | `shared/src/query_params.rs` `parse_content_pagination` | MATCH: a present value is read with `parseInt` (`10abc` is 10, `1.5` is 1); an unparseable or empty one is a 400 (CR-10) |
| `pageSize` > 1000 | 400 | `{"error":"max allowed pageSize is 1000"}` | `utils.ts:19-21` | `query_params.rs` `parse_content_pagination` | MATCH |
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
| body larger than the cap | 413 | `text/plain; charset=utf-8` `Payload Too Large` (capture `captures/content-reads/CR-17-body-over-10mib.json`) | | `routes.ts:92` | `lib.rs` (`routing::MAX_ACTIVE_ENTITIES_BODY_BYTES`, 10 MiB, axum `DefaultBodyLimit`), `routing.rs` `payload_too_large_text`: `413 text/plain; charset=utf-8` `Payload Too Large` | MATCH (CR-17) |
| body not JSON | 400 | `{"ok":false,"message":<V8 JSON.parse message>}`, no `data`. Captured: empty body `Unexpected end of JSON input`; `{` `Expected property name or '}' in JSON at position 1 (line 1 column 2)`; `hello` `Unexpected token 'h', "hello" is not valid JSON` (`captures/content-reads/CR-2-empty-body.json`, `CR-2-not-json.json`, `CR-2-invalid-token.json`) | json | `routes.ts:93` | 400 `{"ok":false,"message":<text>}`, no `data`: V8's message for the three captured shapes (an empty body; `{` not followed by a name or `}`, with its position, line and column; a first character no JSON value starts with, in a source under 21 characters), otherwise serde's parse message (`validation.rs` `v8_parse_message`) | MATCH on the captured shapes; any other parse failure answers serde's wording, as V8's other messages are not captured (CR-2) |
| `Content-Type` not `application/json` (e.g. `text/plain` from a bare `fetch`) | validated normally (`text/plain` with `{}` answers the same 400 as JSON `{}`: `captures/content-reads/CR-2-text-plain.json`) | | | `routes.ts:38-41` | the body is read as `Bytes` and parsed whatever the header (`active_entities.rs:98-110`) | MATCH |
| `{}`; both keys; empty array; empty string element; extra property; non-string element; `null` value; non-object body; > 1000 items | 400 | `{"ok":false,"message":"Invalid JSON body","data":<ajv errors>}`: the first error of each `oneOf` branch, then the `oneOf` error, each `{instancePath, schemaPath, keyword, params, message}` in that key order (`captures/content-reads/CR-2-*.json`, eleven cases) | json | `active-entities-handler.ts:8-23` | the same `ok`/`message`, with `data` built as AJV's first error per `oneOf` branch plus the `oneOf` error (`validation.rs`; fixtures `tests/fixtures/content/active-entities-400-*.json`), written in the captured key order (`validation.rs` `ordered_refusal_json`) | MATCH (CR-2) |
| element containing NUL | 500 `{"error":"Internal Server Error"}` (`captures/content-reads/CR-2-nul-element.json`) | | | | 500 `{"error":"Internal Server Error"}` (`validation.rs` `active_entities_body`) | MATCH (CR-2) |
| success | 200 | `Entity[]` as section 1; unknown ids/pointers omitted; denylisted dropped; duplicates collapsed | no `Cache-Control` (`captures/content-reads/CR-14-active-entity-key-order.json`) | `active-entities-handler.ts:34-47` | `active_entities.rs` `get_active_entities`, no `Cache-Control` | MATCH (CR-8) |
| success with `?fields=` | 200 | upstream ignores the query: full entities | | `active-entities-handler.ts:34-42` | `active_entities.rs` `get_active_entities` masks (a non-empty `fields`) | DIVERGENT: kept as this node's superset (CR-8) |
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
- Rate limit (dcl-one): the same three buckets, defaults, environment variables and exemption in the app
  (`services/catalyst/content/src/content_get_limit.rs`, a route layer on GET and HEAD). The client is the
  socket peer; only a peer in `HTTP_TRUSTED_PROXY_CIDRS` (the gateway) may name it, through the rightmost
  entry of `TRUSTED_CLIENT_IP_HEADER` when set, else its one `X-Real-IP`. Upstream reads its headers from
  any peer; dcl-one's routes are reachable without the gateway too, so a direct client's headers never
  count. `TRUSTED_SYNC_PEER_IPS` also takes networks (CIDR), as the gateway's list does. Counters are per
  process, as upstream's in-memory store. The gateway's own limits (`zone=content`, `zone=read`,
  `infra/gateway/gateway.py`) stay in front; it leaves `GATEWAY_TRUSTED_SYNC_PEER_IPS` uncounted on
  `GET`/`HEAD /contents/{hash}` only, as upstream.
- Rollout: a node behind the gateway must list the gateway's network in `HTTP_TRUSTED_PROXY_CIDRS`;
  otherwise every client shares the gateway's budget (1000 requests a minute by default).

**Responses**

| trigger | status | body | headers | upstream | dcl-one | verdict |
|---|---|---|---|---|---|---|
| over burst/hourly/daily budget | 429 | `{"error":"Too many requests"}` | `Retry-After`, `RateLimit-*` (added by the limiter); a request that passes carries no `RateLimit-*` header (`captures/content-reads/CR-16-path-case-contents.json`) | `components.ts:554`, `content-get-rate-limit.ts:99-118` | `content_get_limit.rs`: the same buckets, body, `Retry-After` and `RateLimit-Limit`/`-Remaining`/`-Reset` on a 429, nothing on a request that passes. The gateway's per-second limits still answer nginx's HTML 429 above 100 r/s (`/content/`) or 30 r/s (root) | MATCH on the app's budgets and body; the `RateLimit-*` values follow a reading of `@dcl/rate-limiter-component`, which is not vendored (needs a capture); the gateway can answer first above its per-second rates |
| client IP in `TRUSTED_SYNC_PEER_IPS` | never limited | | | `sync-peer-exemption.ts:17-24` | `content_get_limit.rs` skips listed peers (addresses or networks); the gateway leaves `GATEWAY_TRUSTED_SYNC_PEER_IPS` uncounted in `zone=content` on `GET`/`HEAD /contents/{hash}` only | MATCH |
| hash fails validation | 404 | `{"error":"No content found with hash <hash>"}` | json | `get-content-handler.ts:15-17` | `get_content.rs:97-99` | MATCH (fixture `tests/fixtures/content/contents-404-garbage-hash.json`) |
| hash denylisted (upstream: its denylist file and `DENYLIST_URLS`, by default Decentraland's ABR list, `Environment.ts:13,513-514`; dcl-one: `content.denylist`, the camera reel's withheld blobs and `DENYLIST_URLS`, Decentraland's ABR list under `NODE_POSTURE=mirror`, `config/src/posture.rs`) | 404 | same | | `get-content-handler.ts:22-24`, `adapters/denylist/component.ts:59-86` | `get_content.rs:97-99`, `content/src/denylist.rs` (`reload_remote`), `storage/src/denylist.rs` | MATCH |
| `If-None-Match` matches `"<hash>"` or is `*` (checked before storage) | 304 | none | `ETag: "<hash>"`, `Cache-Control: public,max-age=31536000,s-maxage=31536000,immutable`, `Access-Control-Expose-Headers: ETag` | `get-content-handler.ts:34-35`, `utils.ts:88-113` | `get_content.rs:101-105`, `formatters.rs:9-33` | MATCH |
| not in storage | 404 | `{"error":"No content found with hash <hash>"}` | json | `get-content-handler.ts:40-43` | `get_content.rs:107-117` | MATCH (dcl-one first tries read-through when attached) |
| Range unsatisfiable (`start >= size`, `start > end`, `bytes=-0`, empty file) | 416 | none | `Content-Range: bytes */<size>`, `Access-Control-Expose-Headers: Content-Range` | `utils.ts:184-195` | `get_content.rs:125-135`, `formatters.rs:41-83` | MATCH |
| Range satisfiable | 206 | the slice (GET), none (HEAD) | base headers below + `Content-Range: bytes a-b/<size>`, `Content-Length: <b-a+1>` | `utils.ts:197-211`, `get-content-handler.ts:52-63` | `get_content.rs:136-159` | MATCH. Upstream `<size>` is the decompressed size of a gzip-stored file (`utils.ts:184`); dcl-one stores raw bytes, so the same number |
| plain GET | 200 | the bytes | `Content-Type: application/octet-stream` (or sniffed), `ETag: "<hash>"`, `Access-Control-Expose-Headers: ETag, Content-Range, Accept-Ranges, Content-Length`, `Accept-Ranges: bytes`, `Cache-Control: public,max-age=31536000,s-maxage=31536000,immutable`, `Content-Length` | `utils.ts:115-135` | `formatters.rs:85-102`, `get_content.rs:193-198` | DIVERGENT: upstream serves a gzip-stored file as stored, with `Content-Encoding: gzip` and the compressed `Content-Length`, regardless of `Accept-Encoding` (`utils.ts:128-133`); dcl-one never sets `Content-Encoding` (`storage/src/content.rs:333-338`). Upstream omits `Content-Length` for a 0-byte file (`if (content.size)`); dcl-one sends `0` |
| plain GET with `STORAGE_X_ACCEL_BASE` set | 200 | empty; nginx serves the file | `X-Accel-Redirect` | n/a | `get_content.rs:367-380`, `infra/gateway/gateway.py:265-272` | behind the gateway, the client gets the plain GET headers (one `Cache-Control`, the sniffed `Content-Type`, the expose list) and a range the app does not support gets the whole file (worlds WR-25). A passing request carries no `RateLimit-*`, and a 429 is answered by the app with them |
| plain HEAD | 200 | none | as plain GET | `get-content-handler.ts:62` | `get_content.rs:163-167` | MATCH, except sniffing (next row) |
| `?includeMimeType` | 200/206 | | `Content-Type` from `file-type` on the stream head, else `application/octet-stream` | `utils.ts:137-153` | `get_content.rs` `detect_content_type`, sniffed on GET, HEAD and the head of a range | DIVERGENT on rarer formats only: dcl-one detects png, glb, mp4 (any `ftyp`), jpeg, gif, mp3 (ID3 or a Layer III frame), wav, webp, zip, ogg, webm, and answers `application/octet-stream` for anything else (JSON and text included, as `file-type`); `file-type`'s other formats (ktx2, `ftyp` brands such as `qt`/`M4A`/`heic`, Ogg codec subtypes, zip-based documents) need its table, which is not vendored (CR-7) |
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
| no `cid`, or a single empty `cid=` | 400 | `{"error":"Please set at least one cid."}` | `get-available-content-handler.ts:24-26`, `logic/query-params/component.ts:25` | `available_content.rs:21-23`, `qs_get_filter_array` | MATCH (fixtures `available-content-400-missing-cid.json`, `available-content-400-empty-cid.json`) |
| > 1000 cids | 400 | `{"error":"Too many cids requested; the maximum allowed is 1000."}` | `get-available-content-handler.ts:27-29` | `available_content.rs:24-28` | MATCH |
| success | 200 | `[{"cid": string, "available": boolean}]` in input order | `get-available-content-handler.ts:33-47` | `available_content.rs:30-45` | MATCH: a repeated cid appears once, as upstream's `Map` |
| storage fault | 500 | `{"error":"Internal Server Error"}` | | `available_content.rs:35`, `storage/src/content.rs:139-165` | MATCH (invalid ids are `false` in both, fixture `available-content-200-bogus-cid.json`) |

**Data dependency**: the blob store. No read-through: on a node that syncs a subset of types (or
relies on read-through), cids an upstream full node holds answer `false`.

---

## 8. GET /audit/:type/:entityId

**Request**: `:type` parsed as section 1 (`outfits` accepted). `:entityId` free text.

| trigger | status | body | upstream | dcl-one | verdict |
|---|---|---|---|---|---|
| type not recognised | 400 | `{"error":"Unrecognized type: <raw :type>"}` | `decentraland/catalyst:src/controllers/handlers/get-audit-handler.ts:17-20` | `audit.rs:17-21` | MATCH (fixtures `audit-404-outfits-accepted.json`, `audit-400-outfitss-rejected.json`, `audit-400-outfit-rejected.json`) |
| no deployment with that id **and that type** | 404 | `{"error":"No deployment found"}` | `get-audit-handler.ts:22-30` (filters `entityIds` and `entityTypes`, includes overwritten and denylisted) | `audit.rs`, `queries.rs` `audit_info` (`WHERE d.entity_id = $1 AND d.entity_type = $2`) | MATCH |
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
- dcl-one: axum answers 405 with `Allow` and an empty body for any method; `routing.rs`
  `upstream_allow` rewrites `Allow` under `/content/` as upstream writes it (`HEAD, GET, POST`, comma
  and space, `HEAD` first); the root mounts keep axum's `GET,HEAD`. A method outside koa-router's list (`FOO`) is also a 405 in dcl-one.
- Path shadowing: upstream `GET /entities/active` matches `/entities/:type` and answers 400
  `{"error":"Unrecognized type: active"}` (captured: `captures/content-reads/CR-12-get-entities-active.json`);
  dcl-one serves `GET` on its static `/entities/active` route with the `/entities/:type` handler and the
  type `active` (`active_entities.rs` `get_entities_active`), the same answer.
- Verdict: MATCH for every captured answer (test `wrong_methods_answer_like_the_captured_router`);
  DIVERGENT for a method outside the router's list, which the capture cannot show (CR-12).

## X3. CORS

Under `/content/` this tree applies upstream's rule (`services/catalyst/content/src/routing.rs`
`content_cors`); the root mounts, the lambdas routes of the content router and this node's own write
routes under `/content/` (`scenes`, `forwarding`, which need `DELETE`) keep the shared rule
(`services/catalyst/shared/src/cors.rs`: the `Origin` echoed with `Allow-Credentials: true`,
`GET,HEAD,POST,PUT,DELETE,PATCH,OPTIONS`, max-age 86400, `Vary: Origin`).

| aspect | upstream (`decentraland/catalyst:src/components.ts:489-506`, `test/integration/controller/cors.spec.ts`) | dcl-one under `/content/` (`routing.rs` `content_cors`) |
|---|---|---|
| `Access-Control-Allow-Origin` | `*` when the request has an `Origin`; absent otherwise (captures) | the same |
| `Access-Control-Allow-Credentials` | not sent | not sent |
| `Access-Control-Expose-Headers` | `*`; the content handler's own list wins on `/contents` (`content.spec.ts:62-70`) | `*` unless the handler set one |
| preflight status | 204 | 204 |
| `Access-Control-Allow-Methods` | `GET,HEAD,POST,OPTIONS` | the same |
| `Access-Control-Allow-Headers` | reflects `Access-Control-Request-Headers` | the same; absent when not requested |
| `Access-Control-Max-Age` | `600` | `600` |
| `Vary` | `Access-Control-Request-Headers` on preflight (`cors.spec.ts:52-54`), none on other responses | the same |
| extra | none from the application code; every response captured from the public peer carried `X-Content-Type-Options: nosniff`, so a layer in front of it adds the header ([`captures/README.md`](captures/README.md), Sanitization) | `X-Content-Type-Options: nosniff` on every response |

The upstream column is confirmed by capture: the preflight for `POST /content/entities/active` answered
`204` with `access-control-allow-headers: content-type`, `access-control-allow-methods: GET,HEAD,POST,OPTIONS`,
`access-control-allow-origin: *`, `access-control-expose-headers: *`, `access-control-max-age: 600`,
`vary: Access-Control-Request-Headers`; a `GET /content/status` with `Origin` answered
`access-control-allow-origin: *` and `access-control-expose-headers: *`
(`captures/content-reads/CR-11-preflight.json`, `CR-11-get-with-origin.json`). Fixtures
`tests/fixtures/cors/preflight-*.json` pin the preflight; tests `content_paths_answer_upstreams_cors`,
`other_paths_keep_the_shared_cors`.

Verdict: MATCH under `/content/` (CR-11).

## Trailing slash, case and prefix

- Trailing slash: upstream's router (path-to-regexp, `strict: false` in the koa-router port it uses)
  matches `/entities/scene/`: captured, `GET /content/entities/scene/?pointer=0,0` answers the same bytes
  as the path without the slash (`captures/content-reads/CR-16-trailing-slash-entities.json`). dcl-one trims a trailing slash before
  routing (`services/catalyst/src/serve.rs:1355`). MATCH.
- Case: the same router port defaults to `sensitive: false`. Captured: `HEAD /content/Contents/<hash>`
  answers `200` with the file's headers, and `/content/Status` and `/content/Deployments` answer `200`
  (`captures/content-reads/CR-16-path-case-contents.json`, `CR-16-path-case-status.json`,
  `captures/content-sync/CS-23-path-case-deployments.json`). dcl-one spells the static segments of a
  content route under `/content/` in the route's case before routing, so these answer the same
  (`services/catalyst/content/src/routing.rs` `upstream_routing`, wrapped around the whole app in
  `services/catalyst/src/serve.rs`); parameters keep their case. The root mounts stay case-sensitive
  (upstream production has none), as does every non-content path. MATCH (CR-16).
  - The write and admin routes (`POST /entities`, `/scenes/{coord}`, `/forwarding/*`) stay
    case-sensitive (`CASE_SENSITIVE_ROUTE_PATHS`): the captures cover reads only, and the gateway's
    deploy limits key on those spellings. `POST /content/Entities` answers 404.
  - `GET /content/entities/Active` keeps the type for `/entities/{type}` and answers
    `Unrecognized type: Active`, as upstream's `/entities/:type` would. Other methods take
    `/entities/active`.
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

2. **CR-2** POST /entities/active: the schema-validator `data` array and parse messages. Status: MATCH, no work.
   - Upstream: the validator component's output (`decentraland/catalyst:src/controllers/handlers/active-entities-handler.ts:8-23`,
     `test/integration/controller/active-entities.spec.ts:28-54`), captured in `captures/content-reads/CR-2-*.json`.
   - dcl-one: the eleven schema cases answer the captured `data` arrays in the captured key order (`ok,message,data`;
     `instancePath,schemaPath,keyword,params,message`; `validation.rs` `ordered_refusal_json`); the three captured
     parse failures answer V8's messages (`v8_parse_message`); a NUL element answers `500 {"error":"Internal Server Error"}`.
     Fixtures `tests/fixtures/content/active-entities-400-*.json` cite the captures; tests
     `parse_failures_answer_v8s_captured_messages`, `refusals_keep_the_captured_key_order`,
     `active_entities_body_with_a_nul_element_is_upstreams_500`.
   - Left: V8's messages for other malformed bodies (a long source, a bad token inside an object) are not
     captured, so those answer serde's wording.
   - Severity: none (closed).

3. **CR-3** GET/HEAD /contents/:hashId: the content-get rate limit.
   - Upstream: per client IP (IPv6 per /64), burst 1000/60 s, then hourly 3000, then daily 20000, each a
     separate bucket, burst first; 429 `{"error":"Too many requests"}` with `Retry-After` and `RateLimit-*`;
     every knob an env var (`decentraland/catalyst:src/controllers/content-get-rate-limit.ts:86-121`,
     `src/Environment.ts:106-125`). A request that passes carries no `RateLimit-*` header
     (`captures/content-reads/CR-16-path-case-contents.json`).
   - dcl-one: `services/catalyst/content/src/content_get_limit.rs` with the same names and defaults
     (`config/src/content.rs`; zero limits and prefixes outside 1-128 fail at startup, as upstream); no
     `RateLimit-*` header on a request that passes (test `the_router_answers_upstreams_429`).
   - Gap: the `RateLimit-*` values on a 429 follow a reading of `@dcl/rate-limiter-component` (not
     vendored); needs a capture of a 429. Counters are per process. Forwarding headers count only from
     `HTTP_TRUSTED_PROXY_CIDRS` peers.
   - Severity: P2.

4. **CR-4** GET/HEAD /contents/:hashId: trusted sync peer bypass. Status: MATCH, no work.
   - Upstream: canonical IPs listed in `TRUSTED_SYNC_PEER_IPS` (comma list; invalid entries dropped with a
     warning) skip all three buckets (`decentraland/catalyst:src/controllers/sync-peer-exemption.ts:12-25`,
     `src/components.ts:580-595`).
   - dcl-one: the same in `content_get_limit.rs`, which also takes networks (CIDR); the gateway gives the
     peers of `GATEWAY_TRUSTED_SYNC_PEER_IPS` an empty `limit_req` key in `zone=content` on `GET`/`HEAD
     /contents/{hash}` only, every other route counted (`infra/gateway/gateway.py`).
   - Severity: none (closed).

5. **CR-5** GET /entities/:type and GET /audit/:type/:entityId: accept `outfits`. Status: MATCH, no work.
   - Upstream: an exact lookup of the trimmed, lower-cased value in the type names plus `<type>s` for names
     not ending in `s`: `outfits` and `scenes` accepted, `outfit` and `outfitss` rejected
     (`decentraland/catalyst:src/types.ts:212-221`, `test/integration/controller/entity-type-parsing.spec.ts:19-56`).
   - dcl-one: `parse_entity_type` builds the same set (`services/catalyst/shared/src/query_params.rs`); `/entities`,
     `/audit`, `/deployments` and `/pointer-changes` use it. Fixtures `entities-200-outfits-accepted.json`,
     `entities-400-outfitss-rejected.json`, `audit-404-outfits-accepted.json`, `audit-400-outfitss-rejected.json`.
   - Severity: none (closed).

6. **CR-6** GET /audit/:type/:entityId: filter by type. Status: MATCH, no work.
   - Upstream: 404 `No deployment found` unless a deployment has that id and that type
     (`decentraland/catalyst:src/controllers/handlers/get-audit-handler.ts:22-30`).
   - dcl-one: `queries::audit_info` filters on the id and the canonical type (`services/catalyst/content/src/queries.rs`,
     `content/src/audit.rs`); pinned in `content/src/tests.rs` (a scene id under `/audit/wearable/` is 404).
   - Severity: none (closed).

7. **CR-7** GET /contents/:hashId?includeMimeType: sniff like `file-type`.
   - Gap: GET, HEAD and ranges sniff png, glb, jpeg, gif, mp3, wav, webp, zip, ogg, webm and any `ftyp` as
     `video/mp4`, and answer `application/octet-stream` otherwise (JSON and text included). `file-type`'s
     other formats and its `ftyp` brand and Ogg codec subtypes differ; its signature table is not in the
     pinned tree, so match it from the `file-type` 21.1.1 source or a capture.
   - Expected: the `file-type` result on the stream head (gif, mp3, wav, zip, png, jpeg, webp, glb, mp4,
     ogg, webm, ...), `application/octet-stream` when it finds none (JSON and text included), on GET and
     HEAD (`decentraland/catalyst:src/controllers/utils.ts:137-153`, `get-content-handler.ts:54`).
   - Place: `services/catalyst/content/src/get_content.rs` (`detect_content_type`).
   - Severity: P2.

8. **CR-8** POST /entities/active: no `Cache-Control`, no `fields`. Status: Kept by decision (the `fields` mask).
   - Decision: `?fields=` keeps masking, as a dcl-one superset. A client that sends it to upstream gets
     full entities, so dropping it changes nothing for upstream clients and removes a feature.
   - Upstream: no `Cache-Control` header; the query string ignored, full entities returned
     (`decentraland/catalyst:src/controllers/handlers/active-entities-handler.ts:36-47`;
     `captures/content-reads/CR-14-active-entity-key-order.json` has no `Cache-Control`).
   - Done: no `Cache-Control` (`active_entities.rs` `get_active_entities`; test
     `active_entities_by_pointers_dedupe_multi_pointer_entities`).
   - Left: `?fields=` still masks; it is listed below as this node's superset.
   - Severity: P2 (the `fields` superset only, by decision).

9. **CR-9** GET /entities/:type: `fields` and empty elements. Status: MATCH, no work.
   - Upstream: an empty `fields` means no mask (`get-entities-handler.ts:61`). `qsGetArray` treats only a
     missing key or a single empty value (`?pointer=`) as absent; empty strings inside a multi-value list are
     kept and simply match nothing. The 400 comes only when both `id` and `pointer` are absent in that sense,
     or both are present (`decentraland/catalyst:src/logic/query-params/component.ts:21-31`).
   - dcl-one: the same (`active_entities.rs` `requested_fields`, `entities_selector`; tests
     `an_empty_element_inside_a_list_is_kept`, `an_empty_fields_value_is_no_mask`,
     `post_without_fields_is_unchanged_and_with_fields_equals_the_full_documents_masked`).
   - Severity: none (closed).

10. **CR-10** GET /entities/active/collections/:collectionUrn: `parseInt` pagination. Status: MATCH, no work.
    - Upstream: present but NaN -> 400 `pageSize must be a positive integer` / `pageNum must be a positive integer`;
      a numeric prefix is used (`10abc` -> 10, `1.5` -> 1) (`decentraland/catalyst:src/controllers/utils.ts:7-26`).
    - dcl-one: `shared/src/query_params.rs` `parse_content_pagination` (`js_parse_int`); test
      `js_pagination_reads_like_pagination_object`.
    - Severity: none (closed).

11. **CR-11** CORS headers. Status: MATCH, no work.
    - Upstream: `Access-Control-Allow-Origin: *`, no `Allow-Credentials`, `Access-Control-Expose-Headers: *`,
      preflight 204 with `Allow-Methods: GET,HEAD,POST,OPTIONS`, reflected `Allow-Headers`, `Max-Age: 600`,
      `Vary: Access-Control-Request-Headers` (`decentraland/catalyst:src/components.ts:492-505`,
      `test/integration/controller/cors.spec.ts:20-114`; `captures/content-reads/CR-11-preflight.json`,
      `CR-11-get-with-origin.json`).
    - dcl-one: the same under `/content/` (`services/catalyst/content/src/routing.rs` `content_cors`), see X3;
      the root mounts and `/content/scenes`, `/content/forwarding` keep the shared rule.
    - Severity: none (closed).

12. **CR-12** Method handling.
    - Gap: a method outside the router's list (`FOO`) is a 405 in dcl-one; upstream's application answers
      501, and the public CDN in front of it a 400 (`captures/content-reads/CR-12-unknown-method.json`).
      Neither body is captured, so it is left.
    - Done: `GET /entities/active` answers 400 `{"error":"Unrecognized type: active"}` through the
      `/entities/:type` handler; a 405's `Allow` is written `HEAD, GET, POST` (comma and space, `HEAD`
      first), with an empty body (`active_entities.rs` `get_entities_active`, `routing.rs` `upstream_allow`;
      test `wrong_methods_answer_like_the_captured_router`), as captured in
      `captures/content-reads/CR-12-*.json`.
    - Expected: 501 for an unknown method only if a capture of the application (not the CDN) shows its
      bytes (`decentraland/catalyst:src/service.ts:64-65`).
    - Place: `services/catalyst/content/src/routing.rs`.
    - Severity: P2.

13. **CR-13** Unknown-route 404 body. Status: MATCH, no work.
    - Gap: dcl-one `text/plain "Not found"`; upstream body unknown from the pinned tree.
    - Expected: the `@dcl/http-server` default answer.
    - Capture: `404 text/plain; charset=utf-8` `Not found` (`captures/content-reads/CR-13-unknown-route.json`),
      the same status, type and body dcl-one answers. No code change is needed.
    - Place: `services/catalyst/content/src/lib.rs:623-625`.
    - Severity: none (closed).

14. **CR-14** Entity JSON key order.
    - Done: the top-level keys follow the capture: `POST /entities/active` and the collection listing
      `id,type,pointers,content,version,timestamp,metadata` (the order upstream's cache holds entities in,
      `decentraland/catalyst:src/logic/entities/component.ts:125-135`); `GET /entities/:type`
      `version,id,type,timestamp,pointers,content,metadata` (`maskEntity`,
      `get-entities-handler.ts:94`) (`captures/content-reads/CR-14-active-entity-key-order.json`,
      `CR-16-trailing-slash-entities.json`; `queries.rs` `ActiveEntityJson`, `formatters.rs` `MaskedEntity`;
      tests `active_entity_keys_follow_the_captured_order`, `masked_keys_follow_upstream_mask_entity_order`).
      An entity upstream reads from the database on a cache miss is `version,id,type,pointers,timestamp,content,metadata`
      (`decentraland/catalyst:src/logic/deployments/component.ts:186-194`); the capture shows the cached order.
    - Gap: `metadata` and `authChain` links keep sorted keys (serde_json without `preserve_order`,
      `services/catalyst/Cargo.toml:68`); upstream sends the deployed text order (cache) or PostgreSQL jsonb
      order (database).
    - Expected: needs capture: the captures elide `metadata`.
    - Place: `services/catalyst/content/src/queries.rs` `row_to_doc`.
    - Severity: P2.

15. **CR-15** /available-content duplicates and empty cid. Status: MATCH, no work.
    - Upstream: one item per distinct cid in first-seen order; `cid=` alone is 400 `Please set at least one cid.`
      (`decentraland/catalyst:src/controllers/handlers/get-available-content-handler.ts:22-47`).
    - dcl-one: the same (`services/catalyst/content/src/available_content.rs`; fixture
      `available-content-400-empty-cid.json`, unit test `a_repeated_cid_is_listed_once`).
    - Severity: none (closed).

16. **CR-16** Case-insensitive paths. Status: MATCH, no work.
    - Upstream: the router matches case-insensitively. Captured: `/content/Contents/<hash>`, `/content/Status`
      and `/content/Deployments` answer 200 (`captures/content-reads/CR-16-path-case-contents.json`,
      `CR-16-path-case-status.json`, `captures/content-sync/CS-23-path-case-deployments.json`).
    - dcl-one: `services/catalyst/content/src/routing.rs` `upstream_routing`, wrapped around the whole
      app in `services/catalyst/src/serve.rs`, spells the static segments of a content route under
      `/content/` in the route's case before routing; parameters keep their case. Root mounts and other
      services' paths are not rewritten. The write and admin routes stay case-sensitive
      (`CASE_SENSITIVE_ROUTE_PATHS`). Tests: `content_paths_match_case_insensitively`,
      `every_listed_content_route_is_routed`, and `every_content_route_is_listed_once`, which checks
      that each registered route is in exactly one list.
    - Severity: none (closed).

17. **CR-17** POST /entities/active body cap. Status: MATCH, no work.
    - Upstream: `MAX_ACTIVE_ENTITIES_BODY_SIZE`, default 10 MiB; above it `413 text/plain; charset=utf-8`
      `Payload Too Large` (captured: `captures/content-reads/CR-17-body-over-10mib.json`; only the over-cap
      side was sent, so the exact cap is the pinned tree's default)
      (`decentraland/catalyst:src/Environment.ts:79`, `src/controllers/routes.ts:92`).
    - dcl-one: a fixed 10 MiB cap and the same 413 (`routing.rs` `MAX_ACTIVE_ENTITIES_BODY_BYTES`,
      `payload_too_large_text`; test `active_entities_body_cap_is_ten_mib`). The cap is not configurable.
      Behind the gateway, `/entities/active` and `/content/entities/active` (any case) take a 10m body
      cap, so the server-wide 1m does not answer nginx's HTML 413 first (`infra/gateway/gateway.py`;
      test `test_active_entities_take_upstreams_ten_mib_body_cap`).
    - Memory: upstream caps neither pointer nor id length. The read-through negative cache stores a
      key longer than 128 bytes as its SHA-256, so its 100,000 entries stay small
      (`read_through.rs` `negative_key`).
    - Severity: none (closed).

18. **CR-18** GET /contents/:hashId gzip passthrough.
    - Gap: never `Content-Encoding: gzip`; `Content-Length: 0` sent for empty files.
    - Expected: a file stored gzip-compressed is sent compressed with `Content-Encoding: gzip` and its compressed
      `Content-Length`; no `Content-Length` when the size is 0 (`decentraland/catalyst:src/controllers/utils.ts:128-133`).
      The storage comment (`services/catalyst/storage/src/content.rs:333-338`) rules out serving unverified
      `.gzip` siblings; an equivalent needs a verified compressed copy.
    - Place: `services/catalyst/content/src/get_content.rs:193-198`, `storage/src/content.rs`.
    - Severity: P2.

19. **CR-19** Entity `timestamp` and audit `localTimestamp` as float. Status: MATCH, no work.
    - Upstream: the float8 `date_part(...) * 1000` as a JS number (`decentraland/catalyst:src/logic/deployments/component.ts:357`).
    - dcl-one: the same double, written as an integer when it is integral and with its fraction otherwise
      (`services/catalyst/content/src/queries.rs` `js_millis`; test `js_millis_keeps_a_fraction_and_drops_a_zero_one`).
      The history routes' timestamps are CS-19.
    - Severity: none (closed).

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
