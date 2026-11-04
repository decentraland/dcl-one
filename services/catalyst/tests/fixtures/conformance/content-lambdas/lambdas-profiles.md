# Lambdas conformance: profiles, outfits, about/status, contracts, crypto

- Area: lamb2 `/profiles`, `/profile`, `/outfits`, `/status`, `/about`, `/contracts/*`, `/crypto/validate-signature`.
- Upstream lambdas: `decentraland/lamb2` @ `02a37f5d88fb895339c20e84c2470be68cdf3f57` (2026-10-21), served at `/lambdas/*` in production.
- Upstream content server (it backs `content.fetchEntitiesByPointers`): `decentraland/catalyst` @ `a416565db1e705cff9523540a2578af642598a29` (2026-10-21).
- dcl-one: `dcl-one-conformance` @ `37e91aed` (origin/main, 2026-11-04), workspace `services/catalyst`.
- Method: static reading only. Nothing in this file was run. "read" means the source was read; "not vendored" means the behaviour lives in an npm package missing from the upstream clone (`@dcl/crypto`, `@dcl/schemas`, `@dcl/catalyst-contracts`, `@dcl/schema-validator-component`, `@well-known-components/http-server`). Those rows need a live capture before anyone acts on them.

Files read (upstream lamb2): `src/controllers/routes.ts`, `src/controllers/handlers/{profiles-handler,errorHandler,status-handler,about-handler,outfits-handler,catalyst-servers-handler,get-pois-handler,get-name-denylist-handler,validate-signature-handler}.ts`, `src/adapters/{profiles,elements-fetcher,status,resource-status,catalysts-fetcher,pois-fetcher,name-denylist-fetcher,marketplace-api-fetcher}.ts`, `src/logic/{outfits,utils,api-with-fallback,pagination-constants}.ts`, `src/logic/fetch-elements/{fetch-elements,fetch-items,fetch-names}.ts`, `src/ports/ownership-checker/{tpw-ownership-checker,contract-helpers,third-party-item-checker}.ts`, `src/components.ts`, `src/service.ts`, `src/types.ts`, `test/unit/{profile-handlers,validate-signature-handler}.spec.ts`, `test/integration/profile-adapter.spec.ts`.
Upstream catalyst: `src/controllers/handlers/active-entities-handler.ts`, `src/logic/active-entities/component.ts`, `test/integration/controller/active-entities.spec.ts`.

Files read (dcl-one): `shop/src/market/{mod,profiles,contracts,lambdas_crypto,lambdas_catalog,third_party}.rs`, `shop/src/market/fixtures/*.json`, `chain-index/src/graph.rs`, `content/src/{about,about_comms,lib,queries}.rs`, `shared/src/{http_util,cors}.rs`, `library/dcl-auth/src/{verify,error}.rs`, `src/serve.rs` (mounts, around line 1196).

## 1. Endpoint inventory

| method | upstream path | production prefix | dcl-one route(s) | status |
|---|---|---|---|---|
| POST | `/profiles` | `/lambdas` | `/lambdas/profiles` `services/catalyst/shop/src/market/mod.rs:885` | DIVERGENT |
| GET | `/profiles/:id` | `/lambdas` | `/lambdas/profiles/{id}` `services/catalyst/shop/src/market/mod.rs:889` | DIVERGENT |
| GET | `/profile/:id` | `/lambdas` | `/lambdas/profile/{id}` `services/catalyst/shop/src/market/mod.rs:890` | DIVERGENT |
| GET | `/outfits/:id` | `/lambdas` | `/lambdas/outfits/{id}` and root `/outfits/{id}` `services/catalyst/shop/src/market/mod.rs:957-958` | DIVERGENT |
| GET | `/status` | `/lambdas` | `/lambdas/status` `services/catalyst/shop/src/market/mod.rs:979` | MATCH |
| GET | `/about` | `/lambdas` (the root `/about` of a production node is this same handler) | `/lambdas/about` and root `/about`, one handler `services/catalyst/content/src/lib.rs:698-700` | DIVERGENT |
| GET | `/contracts/servers` | `/lambdas` | `/lambdas/contracts/servers` `services/catalyst/shop/src/market/mod.rs:911` | DIVERGENT |
| GET | `/contracts/pois` | `/lambdas` | `/lambdas/contracts/pois` `services/catalyst/shop/src/market/mod.rs:915` | DIVERGENT |
| GET | `/contracts/denylisted-names` | `/lambdas` | `/lambdas/contracts/denylisted-names` `services/catalyst/shop/src/market/mod.rs:916` | DIVERGENT |
| POST | `/crypto/validate-signature` | `/lambdas` | `/lambdas/crypto/validate-signature` `services/catalyst/shop/src/market/mod.rs:975` | DIVERGENT |

Counts: 10 endpoints. MATCH 1, GAP 0, DIVERGENT 9, MISSING 0.

Upstream routes: `decentraland/lamb2:src/controllers/routes.ts:41-73`.

### Cross-cutting behaviour (applies to every endpoint below)

| aspect | upstream | dcl-one | verdict |
|---|---|---|---|
| 400 body | `{"error":"Bad request","message":<msg>}` `decentraland/lamb2:src/controllers/handlers/errorHandler.ts:7-14` | same `services/catalyst/shared/src/http_util.rs:14-16` | MATCH |
| 404 body | `{"error":"Not Found","message":<msg>}` `decentraland/lamb2:src/controllers/handlers/errorHandler.ts:37-45` | same `services/catalyst/shared/src/http_util.rs:18-20` | MATCH |
| 503 body | `{"error":"Service overloaded, retry shortly","message":<msg>}` for `ServiceOverloadedError` (marketplace API bulkhead full, `decentraland/lamb2:src/adapters/marketplace-api-fetcher.ts:136-141`) `decentraland/lamb2:src/controllers/handlers/errorHandler.ts:17-25` | `{"error":"Service Unavailable","message":<msg>}` `services/catalyst/shared/src/http_util.rs:22-28` | DIVERGENT: `error` label |
| 502 body | `{"error":"The requested items cannot be fetched right now","message":"Cannot fetch elements for <address>"}` (`FetcherError`) `decentraland/lamb2:src/controllers/handlers/errorHandler.ts:27-35`, `decentraland/lamb2:src/adapters/elements-fetcher.ts:265-274` | the same body, from `third_party::fetch_error` `services/catalyst/shop/src/market/third_party.rs:1645-1656`; produced only by outfits in this area | MATCH (body); the triggers differ per route (see outfits and the profiles failure rows) |
| 500 body | `{"error":"Internal Server Error"}`, no `message` `decentraland/lamb2:src/controllers/handlers/errorHandler.ts:66-77` | `{"error":"Internal Server Error","message":"Internal Server Error"}` `services/catalyst/shared/src/http_util.rs:30-36` | DIVERGENT: extra `message` |
| 405 | `router.allowedMethods()` `decentraland/lamb2:src/service.ts:15` (library not vendored) | axum method router 405 | not compared (needs capture) |
| CORS | http-server `cors: { maxAge: 36000 }` `decentraland/lamb2:src/components.ts:62-69` (header set not vendored) | hand-rolled: reflects `Origin`, `Access-Control-Allow-Credentials: true`, `Access-Control-Max-Age: 86400`, `Access-Control-Expose-Headers: *`, `X-Content-Type-Options: nosniff` `services/catalyst/shared/src/cors.rs:15-86` | DIVERGENT: max-age 36000 vs 86400; other headers need capture |
| Rate limits | none in lamb2 for this area | none; body limit 256 KiB on the two POST routes (413 above it) `services/catalyst/shop/src/market/mod.rs:887,977` | DIVERGENT (413 only on huge bodies) |
| Request timeout | none | `TimeoutLayer` → 408 when `request_timeout` is set `services/catalyst/shop/src/market/mod.rs:986-991` | DIVERGENT (config-dependent) |
| Path case / trailing slash | `@well-known-components/http-server` router (not vendored) | axum: case-sensitive, no trailing-slash match; misses fall to the content 404 | not compared (needs capture) |
| Content-Type of JSON answers | `application/json` | `application/json` | MATCH |

---

## 2. POST /lambdas/profiles

**Request**
- Body: JSON `{ "ids": string[] }`. No auth, no rate limit upstream.
- `ids` must be a non-empty array, else 400. Above 1000 entries (`PAGINATION_DEFAULTS.MAX_PAGE_SIZE`), 400 `decentraland/lamb2:src/controllers/handlers/profiles-handler.ts:12-24`, `decentraland/lamb2:src/logic/pagination-constants.ts:9`.
- Header `If-Modified-Since`: parsed with JS `Date.parse` (RFC 7231 dates and ISO 8601 both accepted). If the value is unparseable it is ignored. `decentraland/lamb2:src/controllers/handlers/profiles-handler.ts:26-37`.
- Ids are passed to the content server as given (no lowercasing in lamb2). The content server matches pointers case-insensitively: its pointer cache key is lowercased, `decentraland/catalyst:src/logic/active-entities/component.ts:67`. Duplicates are removed with a `Set` `decentraland/catalyst:src/logic/active-entities/component.ts:323-324`.
- Content-server limits that apply through `POST /entities/active`: every id must be a non-empty string; at most 1000 ids `decentraland/catalyst:src/controllers/handlers/active-entities-handler.ts:8-23`. If that call fails, lamb2 answers `200 []` (see responses).

dcl-one: `ids` must deserialize as `Option<Vec<String>>`. Every id is lowercased before the lookup. Lookups of ≤50 ids without `If-Modified-Since` go through a per-id 10 s cache and come back in request order, deduplicated. Other lookups query the DB (or the read-through) directly. `services/catalyst/shop/src/market/profiles.rs:684-731`.

**Profile assembly (upstream `decentraland/lamb2:src/adapters/profiles.ts:165-282`, dcl-one `services/catalyst/shop/src/market/profiles.rs:337-482`)**

| step | upstream | dcl-one | verdict |
|---|---|---|---|
| entities without metadata | filtered out `profiles.ts:350` | `None`, skipped `profiles.rs:340` | MATCH |
| `timestamp` | always overwritten with `entity.timestamp` `profiles.ts:175` | the entity's `timestamp` `profiles.rs:470-479` | MATCH |
| top-level shape | only `{ timestamp, avatars }`; every other metadata key is dropped `profiles.ts:276-279` | `{ timestamp, avatars }` `profiles.rs:470-479` | MATCH |
| missing `avatars` | `metadata.avatars.flatMap` throws, caught; the whole batch answers `[]` `profiles.ts:157-158,381-389` | `avatars: []` `profiles.rs:473-477` | DIVERGENT (edge) |
| `userId`/`ethAddress` | set to `entity.pointers[0]` unless the pointer starts with `default` `profiles.ts:171-172,259` | same, lowercased pointer `profiles.rs:319-335,350-352` | MATCH |
| `links` | each url kept if `LinkUrl.validate` passes, else `decodeURIComponent` and revalidate, else dropped; `undefined`/`[]` returned as is `profiles.ts:84-106` | regex `(?i)^(?:https?)://[^\s/$.?#].[^\s]*$` **and** `is_safe_http_url` (rejects loopback, private and single-label hosts) `profiles.rs:279-317` | DIVERGENT: dcl-one also drops links to private/single-label hosts. Whether the regex equals `LinkUrl` is not verified (package not vendored) |
| `hasClaimedName` | always set: `ownedNames.has(avatar.name)`; `false` for default profiles `profiles.ts:197,261` | always set from the owned names (none without `MARKETPLACE_SUBGRAPH_URL`); `false` for default profiles `profiles.rs:215-229,460-467` | MATCH |
| `bodyShape` | legacy `dcl://` ids translated to URN; `?? ''` `profiles.ts:55-62,265` | untouched | DIVERGENT (legacy ids only) |
| snapshots | `face256` and `body` overwritten with `<PROFILE_CDN_BASE_URL>/entities/<entityId>/{face,body}.png`; default base `https://profile-images.decentraland.org`; other snapshot keys kept; a missing object becomes `{face256:'',body:''}` and is then overwritten `profiles.ts:73-82,142,266-270` | the same, with the base `PROFILE_CDN_BASE_URL` or, unset (every posture's default), this node's `profile-images` `profiles.rs:230-277` | DIVERGENT: default host only; `PROFILE_CDN_BASE_URL=https://profile-images.decentraland.org` matches |
| wearables, base | `includes('base-avatars')` → kept as is `profiles.ts:47-49,207-210` | same `profiles.rs:45-47` | MATCH |
| wearables, on-chain | `splitUrnAndTokenId` (7 segments and not `collections-thirdparty` → urn + tokenId) `decentraland/lamb2:src/logic/utils.ts:51-61`. The URN must equal an owned inventory URN **exactly**. If a tokenId is given it must be one of the owned tokens. Output (`ENSURE_ERC_721` default on, `profiles.ts:141`) is `<ownedUrn>:<tokenId or first owned tokenId>` `profiles.ts:212-229` | same split. Ownership = the token ids held of `(owner, urn)` in the collections subgraph (`urn_in`, subgraph id order), after `:mainnet:`→`:ethereum:` normalization. A named tokenId must be held; the output is `<ownedUrn>:<tokenId or first held tokenId>` `profiles.rs:108-169`, `services/catalyst/chain-index/src/graph.rs:405-462` | DIVERGENT (LP-16 only): `mainnet` spelling accepted; the first held token follows subgraph order, upstream the marketplace API's order (needs capture) |
| wearables, third-party | Not in the owned inventory, so dropped from the validated list. Then appended at the **end**: v1 `collections-thirdparty` URNs of an L2 collection unchecked; linked (v2) items checked through the content-server mapping + on-chain `ownerOf`/`balanceOf`; order v1, then the NFTs on L1, then those on L2 `profiles.ts:178,199-201,271`, `decentraland/lamb2:src/ports/ownership-checker/tpw-ownership-checker.ts:65-95`, `decentraland/lamb2:src/ports/ownership-checker/contract-helpers.ts:57-91`, `decentraland/lamb2:src/ports/ownership-checker/third-party-item-checker.ts:33-100` | the same split, checks and order: `third_party::sanitize_third_party_urns` over every avatar's non-base wearables, appended after the validated wearables of each avatar `profiles.rs:146-180,379-447`, `services/catalyst/shop/src/market/third_party.rs:1446-1643`. Runs only when a collections subgraph is configured. A linked item on a network with no RPC endpoint reads as not owned | MATCH |
| wearables, other off-chain | dropped (not owned) | dropped (`of_urn` → `None`) | MATCH |
| wearables, dedup | `Array.from(new Set(...))` `profiles.ts:271` | the merged list is deduplicated `profiles.rs:162-180` | MATCH |
| third-party check fails (RPC or entity read) | the error propagates; `getProfiles` catches it and the whole batch answers `[]` `profiles.ts:187,381-389` | that profile keeps its v1 third-party items only, logged; the rest of the batch is served `profiles.rs:404-417` | DIVERGENT (P2) |
| emotes, base | kept as the deployed object if `!urn.includes(':')` or `urn.includes('urn:decentraland:off-chain:base-emotes')` `profiles.ts:51-53,234-237` | kept if no `:` or a 5-segment `…:base-emotes:x` / `…:base-scene-emotes:x` `profiles.rs:50-55,159-161` | DIVERGENT: `base-scene-emotes` is base only in dcl-one (upstream requires ownership → dropped) |
| emotes, on-chain | same token logic as wearables; output object is exactly `{ urn: "<ownedUrn>:<tokenId>", slot }` `profiles.ts:239-253` | the same `profiles.rs:162-171` | MATCH |
| emotes, absent | `emotes: []` always present `profiles.ts:233,264` | `emotes: []` `profiles.rs:146-176` | MATCH |
| emote without `urn` | `emote.urn.includes` throws → whole batch `[]` | kept | DIVERGENT (edge) |
| default profiles (`pointer` starts with `default`) | nothing owned: every non-base wearable and emote dropped; no TP; `hasClaimedName:false`; no `userId` override `profiles.ts:172,285-294` | the same, with or without a reader `profiles.rs:439-467` | MATCH |
| no ownership reader configured | n/a (the marketplace API or subgraph is always configured) | without a collections subgraph the wearables and emotes are served as deployed (`emotes` still present) `profiles.rs:128-176,439-446`. The mirror posture sets both subgraphs; island and journey set none | DIVERGENT (config) |
| result order | content-server order: pointer-cache hits first (in request order), then DB misses in DB order `decentraland/catalyst:src/logic/active-entities/component.ts:323-356`, `profiles.ts:375-380` | ≤50 ids, no IMS: request order `profiles.rs:527-564`; otherwise byte order of each entity's smallest matching pointer `services/catalyst/content/src/queries.rs:179-211` | DIVERGENT (neither is request order upstream) |

**Responses**

| trigger | status | body | headers | upstream | dcl-one | verdict |
|---|---|---|---|---|---|---|
| `ids` missing, not an array, or `[]` | 400 | `{"error":"Bad request","message":"No profile ids were specified. Expected ids:string[] in body"}` | | `profiles-handler.ts:12-14` | `profiles.rs:40-41,647-663` (missing or `[]`) | MATCH for missing/`[]`. DIVERGENT for a non-array `ids` (e.g. `"ids":"0x…"`): dcl-one answers 400 `Failed to parse the request body as JSON: …` `profiles.rs:672` |
| `ids.length > 1000` | 400 | `{"error":"Bad request","message":"Too many profile ids were specified. Maximum allowed is 1000"}` | | `profiles-handler.ts:20-24` | `profiles.rs:649-660` | MATCH |
| body is not JSON, or is `null` | 500 | `{"error":"Internal Server Error"}` (`request.json()` / `body.ids` throws, not mapped) | | `profiles-handler.ts:10`, `errorHandler.ts:66-77` | 400 `Failed to parse the request body as JSON: <serde error>` `profiles.rs:670-673` | DIVERGENT |
| `ids` contains a non-string or `""` | 200 `[]` (content server 400s the batch, lamb2 catches it) | `[]` | | `active-entities-handler.ts:8-23`, `profiles.ts:381-389` | non-string: 400 parse error; `""`: looked up, other ids answered | DIVERGENT (edge) |
| `If-Modified-Since` parses and every found entity has `floor(ts/1000)*1000 <= ims` (also true when **no** entity is found) | 304 | empty | | `profiles.ts:64-67,342-348` | RFC 2822/7231 and ISO 8601 dates (a date-time without offset is read as UTC); rounded to the second; vacuously true for no entity `profiles.rs:648-681,720-724` | MATCH (the other `Date.parse` forms are not read) |
| `If-Modified-Since` = epoch 0 | IMS ignored (`0` is falsy) | | | `profiles.ts:344` | parsed as 0; 304 only if every ts ≤ 0 | MATCH in effect |
| success | 200 | `Profile[]` (example below) | `Content-Type: application/json` | `profiles-handler.ts:50-53` | `profiles.rs:711-731` | DIVERGENT (assembly table above) |
| unknown ids | 200 | `[]` | | | fixture `services/catalyst/shop/src/market/fixtures/profiles-post-200-unknown-id.json` | MATCH |
| content server unreachable / errors | 200 | `[]` | | `profiles.ts:381-389` | 500 `{"error":"Internal Server Error","message":"Internal Server Error"}` `profiles.rs:485-499,716-719` | DIVERGENT |
| ownership/names fetch fails (non-overload) | 200 | `[]` (`FetcherError` is caught inside `getProfiles`) | | `profiles.ts:381-389` | 503 `{"error":"Service overloaded, retry shortly","message":"ownership readers unavailable"}` (lamb2's retryable envelope) `profiles.rs:528-547` | DIVERGENT (dcl-one fails closed) |
| marketplace API bulkhead full | 503 | `{"error":"Service overloaded, retry shortly","message":<msg>}` | | `profiles.ts:383-385`, `errorHandler.ts:17-25` | no equivalent | MISSING (not applicable: no bulkhead) |
| body > 256 KiB | n/a | | | | 413 | DIVERGENT (edge) |

Success example (upstream shape, `ENSURE_ERC_721` on, default CDN):

```json
[
  {
    "timestamp": 1730000000123,
    "avatars": [
      {
        "userId": "0x0000000000000000000000000000000000000001",
        "ethAddress": "0x0000000000000000000000000000000000000001",
        "name": "example",
        "hasClaimedName": true,
        "description": "",
        "version": 7,
        "tutorialStep": 256,
        "links": [{ "title": "Site", "url": "https://example.org" }],
        "avatar": {
          "bodyShape": "urn:decentraland:off-chain:base-avatars:BaseMale",
          "eyes": { "color": { "r": 0.37, "g": 0.22, "b": 0.19 } },
          "hair": { "color": { "r": 0.6, "g": 0.46, "b": 0.27 } },
          "skin": { "color": { "r": 0.94, "g": 0.76, "b": 0.6 } },
          "wearables": [
            "urn:decentraland:off-chain:base-avatars:eyebrows_00",
            "urn:decentraland:matic:collections-v2:0x00000000000000000000000000000000000000a1:7:105312291668557186697918027683670432318895095400549111254310977543",
            "urn:decentraland:matic:collections-thirdparty:example-tp:example-coll:item-1:matic:0x00000000000000000000000000000000000000b2:9"
          ],
          "emotes": [
            { "slot": 0, "urn": "wave" },
            { "urn": "urn:decentraland:matic:collections-v2:0x00000000000000000000000000000000000000a3:0:1", "slot": 1 }
          ],
          "snapshots": {
            "face256": "https://profile-images.decentraland.org/entities/bafkreiexampleentityid/face.png",
            "body": "https://profile-images.decentraland.org/entities/bafkreiexampleentityid/body.png"
          }
        }
      }
    ]
  }
]
```

dcl-one returns the same object; without `PROFILE_CDN_BASE_URL` the snapshot URLs are on this node's `profile-images` host.

**Data dependency**
- Profile entities: upstream reads its content server. dcl-one reads `content.deployments`/`active_pointers` (peer sync), or the read-through when one is configured `profiles.rs:471-480`. Present.
- Owned wearables/emotes with token ids: upstream uses the marketplace API `/v1/users/<addr>/{wearables,emotes}/grouped`, falling back to the collections subgraphs `decentraland/lamb2:src/logic/api-with-fallback.ts:8-47`. dcl-one asks the collections subgraphs (`ETH_/MATIC_COLLECTIONS_SUBGRAPH_URL`) for the token ids held of each worn URN, `graph::owned_tokens` (`services/catalyst/chain-index/src/graph.rs:424`).
- Names: upstream uses the marketplace API `/v1/users/<addr>/names`, falling back to the subgraph. dcl-one uses `MARKETPLACE_SUBGRAPH_URL` `services/catalyst/chain-index/src/graph.rs:491-525`. Present.
- Third-party ownership: upstream uses content-server item entities (`metadata.mappings`) plus L1/L2 RPC. dcl-one uses its content DB's item entities and each network's RPC endpoint (`TRADE_RPC_URLS`, `ETH_RPC_URL`, `RPC_ENDPOINT_POLYGON`) through `sanitize_third_party_urns` `services/catalyst/shop/src/market/third_party.rs:1501-1643`. Present when those endpoints are configured.

## 3. GET /lambdas/profiles/:id

**Request**: path `id` (any string; not validated; case-insensitive through the content server). No query params, no auth. `If-Modified-Since` is not read.

| trigger | status | body | upstream | dcl-one | verdict |
|---|---|---|---|---|---|
| profile found | 200 | one `Profile` object (shape as in §2) | `decentraland/lamb2:src/controllers/handlers/profiles-handler.ts:56-68`, `profiles.ts:392-395` | `services/catalyst/shop/src/market/profiles.rs:590-616,734-740` | DIVERGENT (assembly differences of §2) |
| not found / no metadata | 404 | `{"error":"Not Found","message":"Profile not found"}` | `profiles-handler.ts:60-62` | `profiles.rs:737`; fixture `fixtures/profile-by-id-404-unknown-address.json` | MATCH |
| content server or ownership fetch fails | 404 (the `[]` from `getProfiles` → `undefined`) | as above | `profiles.ts:381-394` | DB: 500; reader: 503 `Service overloaded, retry shortly` / `ownership readers unavailable` | DIVERGENT |
| overload | 503 `Service overloaded, retry shortly` | | `profiles.ts:383-385` | n/a | MISSING (no bulkhead) |
| `id` = `default` or `default<digits>` | the deployed default-profile entity if the content server holds one, else 404 | | | the same, assembled owning nothing `profiles.rs:590-616` | MATCH |

Data dependency: as §2.

## 4. GET /lambdas/profile/:id (legacy alias)

Documented divergence (upstream comment `decentraland/lamb2:src/controllers/handlers/profiles-handler.ts:70-75`): a missing profile answers `200 {"avatars":[],"timestamp":0}` instead of 404.

| trigger | status | body | upstream | dcl-one | verdict |
|---|---|---|---|---|---|
| found | 200 | `Profile` | `profiles-handler.ts:79-83` | `services/catalyst/shop/src/market/profiles.rs:743-749` | DIVERGENT (assembly, §2) |
| not found | 200 | `{"avatars":[],"timestamp":0}` | `profiles-handler.ts:80-81` | `profiles.rs:746`; fixture `fixtures/profile-alias-200-unknown-address.json` | MATCH |
| content server or ownership fetch fails | 200 stub | `{"avatars":[],"timestamp":0}` | `profiles.ts:381-394` | 500 / 503 (`Service overloaded, retry shortly`) | DIVERGENT |
| overload | 503 | | `profiles.ts:383-385` | n/a | MISSING (no bulkhead) |
| `default*` id | deployed entity or stub | | | the same | MATCH |

## 5. GET /lambdas/outfits/:id

**Request**: path `id`, the address. Not validated or lowercased by lamb2; the content server matches the pointer `<id>:outfits` case-insensitively. No auth.

**Logic** (`decentraland/lamb2:src/logic/outfits.ts:26-125`; dcl-one `services/catalyst/shop/src/market/lambdas_catalog.rs:1099-1300`)

| step | upstream | dcl-one | verdict |
|---|---|---|---|
| entity lookup | `content.fetchEntitiesByPointers(['<id>:outfits'])` `outfits.ts:31` | `profiles::active_entities(['<lower id>:outfits'])`: the read-through when one is configured, else the local DB `lambdas_catalog.rs:1112-1116` | MATCH |
| entity without metadata, or `outfits: []` | the raw entity is returned unchanged (no `namesForExtraSlots`) `outfits.ts:37-40` | the same `lambdas_catalog.rs:1120-1126` | MATCH |
| wearable ownership per outfit | Kept as is if it contains `off-chain` or `base-avatars`. `collections-thirdparty` items are checked through the TP ownership checker. Every other item must be owned (tokenId-aware); otherwise the **whole outfit is dropped** `outfits.ts:50-98` | the same rule: base and off-chain pass, third-party items go through `sanitize_third_party_urns`, any other must be an owned copy (and, given a token id, that token) from `graph::owned_copies` `lambdas_catalog.rs:1155-1268`. With no collections subgraph configured nothing is checked and the wearables are only lowercased `lambdas_catalog.rs:1170-1184` | MATCH (configured); the owned copies stop at 10,000 per network (see the items area, LI-17) |
| owned wearable string | `<ownedUrn>:<tokenId or first owned tokenId>` (`ENSURE_ERC_721`) `outfits.ts:73-78` | the same `lambdas_catalog.rs:1219-1225` | MATCH |
| wearables case | lowercased `outfits.ts:108-116` | lowercased `lambdas_catalog.rs:1260-1263` | MATCH |
| third-party quirk | A missing regular wearable `break`s the loop. The TP check that follows then **overwrites** `allWearablesOwned` with the result for the TP items seen before the break, so an outfit `[tp(owned), unowned]` is kept with a truncated wearable list `outfits.ts:79-93` | kept: the walk breaks and the TP result decides alone `lambdas_catalog.rs:1226-1229, 1252-1256` | MATCH |
| extra slots | slots `<= 4` always kept; slots `> 4` only if the address owns ≥1 name. Output order: all `<=4` outfits first, then `>4` `outfits.ts:104-108` | the same; an outfit without a numeric `slot` is dropped `lambdas_catalog.rs:1276-1298` | MATCH |
| metadata shape | `{ outfits, namesForExtraSlots }` only (other keys dropped) `outfits.ts:118-124` | the same `lambdas_catalog.rs:1297` | MATCH |
| `namesForExtraSlots` | owned names, in fetcher order `outfits.ts:101-102,122` | `graph::owned_names` (subgraph id order); `[]` without `MARKETPLACE_SUBGRAPH_URL` or when `id` is not a hex address `lambdas_catalog.rs:1127-1139` | MATCH when configured |

**Responses**

| trigger | status | body | upstream | dcl-one | verdict |
|---|---|---|---|---|---|
| found | 200 | entity `{version,id,type,pointers,timestamp,content,metadata:{outfits:[{slot,outfit:{bodyShape,eyes,hair,skin,wearables}}],namesForExtraSlots:[string]}}` | `outfits-handler.ts:25-33` | `lambdas_catalog.rs:1136-1140` | DIVERGENT (shape details, LP-23) |
| no entity | 404 | `{"error":"Not Found","message":"Outfits not found"}` | `outfits-handler.ts:26-28` | `lambdas_catalog.rs:1140` | MATCH |
| content server fails | 500 `{"error":"Internal Server Error"}` (content client throws, not mapped) | | `outfits.ts:31` | 500 `{"error":"Internal Server Error","message":"Internal Server Error"}`, not cached `lambdas_catalog.rs:1114-1116` | MATCH (status; the body's extra `message` is LP-30) |
| wearables/names fetch fails | 502 `{"error":"The requested items cannot be fetched right now","message":"Cannot fetch elements for <id>"}` | | `outfits.ts:45,101`, `elements-fetcher.ts:265-274` | the same 502 for both `lambdas_catalog.rs:1135,1191` | MATCH |
| third-party check fails (RPC or entity read) | 500 `{"error":"Internal Server Error"}` (the checker's error is not a `FetcherError`) | | `outfits.ts:85-88`, `errorHandler.ts:66-77` | 502 `Cannot fetch elements for <id>` `lambdas_catalog.rs:1242-1244` | DIVERGENT (P2) |
| overload | 503 `Service overloaded, retry shortly` | | | n/a | MISSING (no bulkhead) |

Example (upstream, one owned outfit, one name):

```json
{
  "version": "v3",
  "id": "bafkreiexampleoutfitsentity",
  "type": "outfits",
  "pointers": ["0x0000000000000000000000000000000000000001:outfits"],
  "timestamp": 1730000000000,
  "content": [],
  "metadata": {
    "outfits": [
      { "slot": 0, "outfit": { "bodyShape": "urn:decentraland:off-chain:base-avatars:BaseFemale",
        "eyes": { "color": { "r": 0.2, "g": 0.3, "b": 0.4 } }, "hair": { "color": { "r": 0.1, "g": 0.1, "b": 0.1 } },
        "skin": { "color": { "r": 0.9, "g": 0.7, "b": 0.6 } },
        "wearables": ["urn:decentraland:off-chain:base-avatars:f_eyebrows_00",
                      "urn:decentraland:matic:collections-v2:0x00000000000000000000000000000000000000a1:7:42"] } }
    ],
    "namesForExtraSlots": ["example"]
  }
}
```

**Data dependency**: the outfits entity (content DB, or the read-through when configured); owned wearables with token ids (collections subgraphs; dcl-one has `graph::owned_copies`); TP ownership (`third_party.rs`); names (marketplace subgraph, present).

## 6. GET /lambdas/status

| trigger | status | body | upstream | dcl-one | verdict |
|---|---|---|---|---|---|
| always | 200 | `{"version":string,"currentTime":number(ms),"commitHash":string}`; `""` when `CURRENT_VERSION`/`COMMIT_HASH` are unset | `decentraland/lamb2:src/controllers/handlers/status-handler.ts:3-19` | `services/catalyst/shop/src/market/mod.rs:869-877`; fixture `fixtures/lambdas-status-200-shape.json` | MATCH (the JSON key order differs, which has no meaning) |

Data dependency: none. The fixture's `captured_from` names `lambdas.rs::lambdas_status`; the handler is in `mod.rs`.

## 7. GET /lambdas/about (and the root /about)

dcl-one serves the same handler at `/lambdas/about` and at the root `/about` (`services/catalyst/content/src/lib.rs:698-700`). `/content/about` is not routed.

**Upstream computation** (`decentraland/lamb2:src/controllers/handlers/about-handler.ts:27-151`): it fetches `<CONTENT_URL or INTERNAL_CONTENT_URL>/status` and `<LAMBDAS_URL or INTERNAL_LAMBDAS_URL>/status`, plus `<ARCHIPELAGO_URL or INTERNAL_ARCHIPELAGO_URL>/status` when `ARCHIPELAGO_URL` is set. Each answer is cached 60 s; a non-2xx answer or a network error counts as unhealthy `decentraland/lamb2:src/adapters/status.ts:21-51`.

| field | upstream | dcl-one (`services/catalyst/content/src/about.rs:175-232`) | verdict |
|---|---|---|---|
| HTTP status | 200 if `healthy`, else 503 `about-handler.ts:147-150` | same `about.rs:225-231` | MATCH |
| `healthy` | content reachable **and** `synchronizationState == "Syncing"` **and** lambdas `/status` reachable **and** (archipelago reachable if configured) `:70-81` | in-process sync state `Syncing` (`PartiallySynced` also reports `Syncing`, `services/catalyst/content/src/lib.rs:368-374`) **and** comms block healthy | DIVERGENT in derivation; the field is the same |
| `acceptingUsers` | `healthy && !(cpu load% > 90 || mem used% > 90) && (!MAX_USERS || userCount < MAX_USERS)` `:74,82`, `decentraland/lamb2:src/adapters/resource-status.ts:8-43` | `healthy && under_capacity (COMMS max_users) && console accepting-users toggle` `about.rs:186,222`, `services/catalyst/content/src/about_comms.rs:164-169` | DIVERGENT: no CPU/memory check; extra toggle |
| `content.healthy` | as computed above | sync state only | MATCH in effect |
| `content.version` | content `/status` `version`; omitted if unreachable | `settings.version` (always present) | DIVERGENT (edge) |
| `content.synchronizationStatus` | content `/status` `synchronizationStatus.synchronizationState`, `"Unknown"` if unreachable | `Bootstrapping`/`Syncing`/`Failed` | MATCH except `Unknown` |
| `content.commitHash` | content `/status` `commitHash` | `settings.commit_hash` (omitted if `None`) | MATCH in effect |
| `content.publicUrl` | `CONTENT_URL` as configured | `content_public_url` | MATCH |
| `lambdas.healthy` | `true` | `true` | MATCH |
| `lambdas.version` / `commitHash` | `CURRENT_VERSION` / `COMMIT_HASH` (omitted when unset) | `settings.version` / `settings.commit_hash` | MATCH |
| `lambdas.publicUrl` | `LAMBDAS_URL` | `lambdas_public_url` | MATCH |
| `configurations.networkId` | `l1Contracts[ETH_NETWORK].chainId`: mainnet 1, sepolia 11155111. Unknown network → throw → 500 `:31-36` | mainnet 1, sepolia 11155111, goerli 5, ropsten 3, rinkeby 4, kovan 42, anything else 1 `about.rs:108-118` | DIVERGENT (edge) |
| `configurations.globalScenesUrn` / `scenesUrn` | `[]` / `[]` | `[]` / `[]` | MATCH |
| `configurations.realmName` | `REALM_NAME`, omitted when unset | same | MATCH |
| `configurations.map.minimapEnabled`, `sizes` | `true`; `[{-150,150,150,-150},{62,158,162,151},{151,150,163,59}]` | same `about.rs:125-146` | MATCH |
| `configurations.map.satelliteView` | always `{version:"v1",baseUrl:"https://genesis.city/map/latest",suffixUrl:".jpg",topLeftOffset:{x:-2,y:-6}}` `:103-108` | present only with `MAP_SATELLITE_BASE_URL`, using that value `about.rs:159-167` | DIVERGENT |
| `configurations.map.parcelView` | always `{version:"v1",imageUrl:"https://api.decentraland.org/v1/minimap.png"}` `:109-112` | this node's parcel view URL, absent when the node has none `about.rs:168-171` | DIVERGENT |
| `comms` | present only with `ARCHIPELAGO_URL`: `{healthy, protocol:"v3", version, commitHash, usersCount, adapter:"archipelago:archipelago:wss://<host>/archipelago/ws"}`. `version`/`commitHash` come from archipelago `/status` `:77-93` | present only with `COMMS_FIXED_ADAPTER`: `{healthy, protocol (COMMS_PROTOCOL, default "v3"), version?, commitHash?, usersCount, adapter:"fixed-adapter:<x>", fixedAdapter:"<x>"}`. `"offline:offline"` when the SFU probe fails `services/catalyst/content/src/about_comms.rs:22-37,92-105,202-227` | DIVERGENT: adapter scheme and extra `fixedAdapter` key |
| `bff` | `{healthy:true,protocolVersion:"1.0_0",userCount:0,publicUrl:"/bff"}` | same `about.rs:213-218` | MATCH |

Example (upstream, no archipelago):

```json
{
  "healthy": true,
  "content": { "healthy": true, "version": "7.0.0", "synchronizationStatus": "Syncing", "commitHash": "abc123", "publicUrl": "https://peer.example.org/content" },
  "lambdas": { "healthy": true, "version": "7.0.0", "commitHash": "def456", "publicUrl": "https://peer.example.org/lambdas" },
  "configurations": { "networkId": 1, "globalScenesUrn": [], "scenesUrn": [], "realmName": "example",
    "map": { "minimapEnabled": true, "sizes": [{"left":-150,"top":150,"right":150,"bottom":-150},{"left":62,"top":158,"right":162,"bottom":151},{"left":151,"top":150,"right":163,"bottom":59}],
      "satelliteView": { "version": "v1", "baseUrl": "https://genesis.city/map/latest", "suffixUrl": ".jpg", "topLeftOffset": { "x": -2, "y": -6 } },
      "parcelView": { "version": "v1", "imageUrl": "https://api.decentraland.org/v1/minimap.png" } } },
  "bff": { "healthy": true, "protocolVersion": "1.0_0", "userCount": 0, "publicUrl": "/bff" },
  "acceptingUsers": true
}
```

Data dependency: sync state (in-process, present), comms core (in-process, present), map URLs (config).

## 8. GET /lambdas/contracts/servers

Upstream: the L1 `Catalyst` contract via `@dcl/catalyst-contracts` `getCatalystServersFromDAO`, mapped to `{baseUrl: address, owner, id}`. The list is cached in an LRU (max 1, TTL 6 h, no stale serving) `decentraland/lamb2:src/adapters/catalysts-fetcher.ts:45-59`; the handler is at `decentraland/lamb2:src/controllers/handlers/catalyst-servers-handler.ts:4-11`. The library's domain normalization is not vendored. dcl-one drops `http://` domains, prefixes `https://` and trims whitespace; the owner is EIP-55 checksummed and the id is `0x` + 64 lowercase hex `services/catalyst/shop/src/market/contracts.rs:218-243`.

| trigger | status | body | upstream | dcl-one | verdict |
|---|---|---|---|---|---|
| success | 200 | `[{"baseUrl":"https://peer.example.org","owner":"0x0000000000000000000000000000000000000001","id":"0x00…01"}]` in contract index order | `catalysts-fetcher.ts:56-59` | `contracts.rs:245-272,354-375` | MATCH as far as read (`baseUrl` normalization needs a capture) |
| RPC fails, nothing cached / TTL expired | 500 `{"error":"Internal Server Error"}` (the LRU `fetch` rejects) | | `catalysts-fetcher.ts:45-58` | stale value if any, else `200 []`; a failure is not retried for 60 s `contracts.rs:292-345,369-373` | DIVERGENT |
| no RPC configured | n/a (the provider is hard-wired to `rpc.decentraland.org`, `decentraland/lamb2:src/components.ts:167`) | `200 []` when `ETH_RPC_URL` is unset `contracts.rs:355-358` | DIVERGENT (config) |

Data dependency: L1 RPC (`ETH_RPC_URL`); the contract address per network is in `contracts.rs:35-48`.

## 9. GET /lambdas/contracts/pois

Upstream: the L2 POI list contract via `getPoisFromContract`, cached the same way (LRU max 1, 6 h) `decentraland/lamb2:src/adapters/pois-fetcher.ts:21-31`.

| trigger | status | body | upstream | dcl-one | verdict |
|---|---|---|---|---|---|
| success | 200 | `["-10,20","5,5"]` (strings, contract order) | `get-pois-handler.ts:4-11` | `size()`, then `get(i)` `contracts.rs:275-290,378-402` | MATCH as read |
| RPC fails | 500 `{"error":"Internal Server Error"}` | | `pois-fetcher.ts:29-31` | stale value or `200 []` | DIVERGENT |
| no RPC | n/a | | | `200 []` without `RPC_ENDPOINT_POLYGON`/`TRADE_RPC_URLS` 137 | DIVERGENT (config) |

Data dependency: L2 RPC.

## 10. GET /lambdas/contracts/denylisted-names

Upstream: the L1 name-denylist contract via `getNameDenylistFromContract`, LRU max 1, 6 h `decentraland/lamb2:src/adapters/name-denylist-fetcher.ts:27-37`. Whether the library trims or filters entries is not vendored; dcl-one returns the raw strings `contracts.rs:405-431`.

| trigger | status | body | upstream | dcl-one | verdict |
|---|---|---|---|---|---|
| success | 200 | `["name1","name2"]` | `get-name-denylist-handler.ts:4-11` | `contracts.rs:405-431` | MATCH as read (library normalization unknown) |
| RPC fails | 500 `{"error":"Internal Server Error"}` | | | stale value or `200 []` | DIVERGENT |
| no RPC | n/a | | | `200 []` without `ETH_RPC_URL` | DIVERGENT (config) |

Data dependency: L1 RPC.

## 11. POST /lambdas/crypto/validate-signature

**Request** (`decentraland/lamb2:src/controllers/handlers/validate-signature-handler.ts:20-29`): body `{ timestamp?: string, signedMessage?: string, authChain: AuthChain }`, `additionalProperties: false`, `authChain` required and validated by `AuthChain.schema` (`@dcl/schemas`, not vendored). Validation runs in the `@dcl/schema-validator-component` middleware `decentraland/lamb2:src/controllers/routes.ts:69-73`. Then the handler checks the chain length and the final authority (`signedMessage ?? timestamp`). No auth, no rate limit. EIP-1654 links are checked over `l1Provider` (`rpc.decentraland.org`, `decentraland/lamb2:src/components.ts:167`).

dcl-one (`services/catalyst/shop/src/market/lambdas_crypto.rs:20-83`): serde `{timestamp?: String, signedMessage?: String, authChain?: Vec<AuthLink>}`. Unknown keys are ignored and there is no Content-Type check. `dcl-auth` verifies the chain, and the EIP-1654 validator runs only when `ETH_RPC_URL` is set.

| trigger | status | body | upstream | dcl-one | verdict |
|---|---|---|---|---|---|
| `Content-Type` not `application/json` | 400 | `{"ok":false,"message":"Content-Type must be application/json"}` (library, not vendored) | middleware | ignored; the body is parsed anyway | DIVERGENT (needs capture) |
| body not JSON | 400 | `{"ok":false,"message":<parse error>}` (library) | middleware | 400 `{"error":"Bad request","message":"Invalid JSON body: <serde error>"}` `lambdas_crypto.rs:61-64` | DIVERGENT |
| schema violation (missing `authChain`, unknown key, `timestamp` not a string, bad link `type`) | 400 | `{"ok":false,"message":"Invalid JSON body","data":[<ajv errors>]}` (`ok` and `message` confirmed for the same middleware by `decentraland/catalyst:test/integration/controller/active-entities.spec.ts:28-42`; `data` not vendored) | middleware | 400 `{"ok":false,"message":"Invalid JSON body","data":[…]}`; `data` holds one ajv-shaped error for the top-level keywords (`type`, `required`, `additionalProperties`, property `type`), `[]` for an `authChain` link error `lambdas_crypto.rs:50-99` | MATCH for status, `ok`, `message`; `data` needs capture |
| `authChain` length 0 or > 10 | 400 | `{"error":"Bad request","message":"'authChain' length must be between 1 and 10"}` (an empty array may already fail `AuthChain.schema`; not vendored) | `validate-signature-handler.ts:51-53` | same message `lambdas_crypto.rs:43-47` | MATCH |
| neither `signedMessage` nor `timestamp` (or the chosen one is `""`) | 400 | `{"error":"Bad request","message":"Expected 'signedMessage' property to be set"}` | `validate-signature-handler.ts:55-58` | same `lambdas_crypto.rs:48-51` | MATCH (`""` `signedMessage` with a `timestamp`: both reject) |
| valid chain | 200 | `{"valid":true,"ownerAddress":"0x0000000000000000000000000000000000000001"}` (`error` undefined, omitted) | `validate-signature-handler.ts:60-69` | `{"valid":true,"ownerAddress":<authChain[0].payload>}` `lambdas_crypto.rs:79` | MATCH |
| invalid chain | 200 | `{"valid":false,"error":<@dcl/crypto message>}` | same | `{"valid":false,"error":<dcl-auth message>}` `lambdas_crypto.rs:80`, messages in `library/dcl-auth/src/error.rs` | DIVERGENT: message text (e.g. dcl-auth `Invalid final authority. Expected: X. Got: Y`) |
| single SIGNER link whose payload equals `signedMessage` | 200 (per `@dcl/crypto` the final authority equals the signer; not vendored) | | | `valid:false`, `Malformed auth chain: auth chain has no signed link after the SIGNER` `library/dcl-auth/src/verify.rs:224-229` | DIVERGENT (edge, needs capture) |
| EIP-1654 link | 200, validated over L1 RPC | | | without `ETH_RPC_URL`: `valid:false`, `EIP-1654 smart-contract wallet verification is not implemented (requires RPC)` | DIVERGENT (config) |
| body > 256 KiB | n/a | | | 413 | DIVERGENT (edge) |

Data dependency: L1 RPC for EIP-1654 (`ETH_RPC_URL`).

---

## 12. Backlog

LP-01. **POST/GET profiles: owned third-party wearables are kept and appended.**
- Status: MATCH, no work.
- Upstream: v1 TP URNs of an L2 collection kept unchecked; linked TP items kept when the content-server mapping includes the NFT and `ownerOf`/`balanceOf` confirms it; the kept items appended **after** the validated wearables (v1, then L1 NFTs, then L2 NFTs), deduplicated. `decentraland/lamb2:src/adapters/profiles.ts:178,199-201,271`, `decentraland/lamb2:src/ports/ownership-checker/tpw-ownership-checker.ts:65-95`, `decentraland/lamb2:src/ports/ownership-checker/contract-helpers.ts:57-91`.
- dcl-one: the same, through `third_party::sanitize_third_party_urns` `services/catalyst/shop/src/market/profiles.rs:146-180,379-447`, `services/catalyst/shop/src/market/third_party.rs:1501-1643`. The failure path differs and is a P2 row of §2 (a failed check keeps that profile's v1 items; upstream answers `[]` for the batch).
- Severity: none (closed).

LP-02. **Outfits: failure codes after the ownership filter.**
- The ownership filter matches upstream: an outfit with a non-base, non-off-chain wearable not owned (tokenId-aware) or a TP item not owned is dropped; owned wearables become `<ownedUrn>:<tokenId>`; every kept wearable is lowercased; the `break` quirk is kept. `decentraland/lamb2:src/logic/outfits.ts:45-116`; `services/catalyst/shop/src/market/lambdas_catalog.rs:1155-1268`.
- Gap: a failed third-party check answers 502 `Cannot fetch elements for <id>`; upstream's checker error is unmapped, so 500 `{"error":"Internal Server Error"}`. Without a collections subgraph nothing is checked (as LP-07).
- Place: `services/catalyst/shop/src/market/lambdas_catalog.rs:1242-1244`.
- Severity: P2.

LP-03. **Profiles: ERC-721 token handling.**
- Status: MATCH, no work.
- Upstream: a named tokenId must be among the owned tokens, else the item is dropped; output `<ownedUrn>:<tokenId or first owned tokenId>` (`ENSURE_ERC_721` default true); owned emotes re-emitted as exactly `{urn, slot}`. `decentraland/lamb2:src/adapters/profiles.ts:141,212-229,239-253`.
- dcl-one: the same over `graph::owned_tokens` `services/catalyst/shop/src/market/profiles.rs:108-176`, `services/catalyst/chain-index/src/graph.rs:405-462`. The first held token follows subgraph id order (lamb2's subgraph fallback order; the marketplace API's order needs a capture). The `mainnet` spelling is LP-16.
- Severity: none (closed).

LP-04. **Profiles: `emotes` is always present.**
- Status: MATCH, no work.
- Upstream: `avatar.emotes = []` when absent. `decentraland/lamb2:src/adapters/profiles.ts:233,264`.
- dcl-one: `services/catalyst/shop/src/market/profiles.rs:146-176`.
- Severity: none (closed).

LP-05. **Profiles: response is `{timestamp, avatars}` with the entity timestamp.**
- Status: MATCH, no work.
- Upstream: `timestamp = entity.timestamp`, only `timestamp` and `avatars` at the top level. `decentraland/lamb2:src/adapters/profiles.ts:175,276-279`.
- dcl-one: `services/catalyst/shop/src/market/profiles.rs:469-479`.
- Severity: none (closed).

LP-06. **Profiles: `hasClaimedName` is always computed.**
- Status: MATCH, no work.
- Upstream: `hasClaimedName = ownedNames.has(avatar.name)`, `false` for default profiles. `decentraland/lamb2:src/adapters/profiles.ts:197,261`.
- dcl-one: always set from the owned names, `false` for default profiles and when no `MARKETPLACE_SUBGRAPH_URL` is configured (no name is proven) `services/catalyst/shop/src/market/profiles.rs:215-229,460-467`. Requiring a names reader at startup is LP-07.
- Severity: none (closed).

LP-07. **Profiles: serving without ownership readers.**
- Gap: with no collections subgraph configured, wearables and emotes are served as deployed. The mirror posture sets `ETH_/MATIC_COLLECTIONS_SUBGRAPH_URL` and `MARKETPLACE_SUBGRAPH_URL` (`services/catalyst/config/src/posture.rs:273-287`), but an operator can set them empty; island and journey set none by design (no third-party traffic).
- Expected: upstream always filters.
- Place: the posture checks in `services/catalyst/config/src/posture.rs`: refuse a mirror whose collections or marketplace subgraph URL is empty. `services/catalyst/shop/src/market/profiles.rs:439-446` keeps serving as deployed on island and journey.
- Severity: P1.

LP-08. **Profiles: snapshot URLs.**
- Gap: without `PROFILE_CDN_BASE_URL` (unset under every posture) the base is this node's `profile-images`, not `https://profile-images.decentraland.org`. Face, body, the other keys and a missing object follow upstream.
- Expected: `face256 = <PROFILE_CDN_BASE_URL or https://profile-images.decentraland.org>/entities/<entityId>/face.png`, `body = …/body.png`, every other snapshot key untouched, a missing object created. `decentraland/lamb2:src/adapters/profiles.ts:73-82,142,266-270`.
- Place: `services/catalyst/shop/src/market/profiles.rs:230-277` serves the node's own images on purpose (sync never copies snapshot files). Parity needs `PROFILE_CDN_BASE_URL=https://profile-images.decentraland.org`, for example as the mirror posture's default.
- Severity: P1.

LP-09. **POST /profiles: `If-Modified-Since`.**
- Status: MATCH, no work.
- Upstream: `Date.parse` of the header; 304 when every found entity has `floor(timestamp/1000)*1000 <= ims`, including when none is found. `decentraland/lamb2:src/controllers/handlers/profiles-handler.ts:26-48`, `decentraland/lamb2:src/adapters/profiles.ts:64-67,342-348`.
- dcl-one: RFC 2822/7231 and ISO 8601 dates (a date-time without an offset read as UTC; `Date.parse` reads it in the server's zone), the same rounding and the same empty-match 304 `services/catalyst/shop/src/market/profiles.rs:648-681,720-724`. Other `Date.parse` forms are not read.
- Severity: none (closed).

LP-10. **Profiles: failure semantics.**
- Gap: on a DB failure dcl-one answers 500; on a reader failure, 503 `{"error":"Service overloaded, retry shortly","message":"ownership readers unavailable"}` (lamb2's retryable envelope).
- Expected: upstream answers `200 []` (POST), 404 (`/profiles/:id`) or the stub (`/profile/:id`) for any failure except overload. Overload is 503 with `"error":"Service overloaded, retry shortly"`. `decentraland/lamb2:src/adapters/profiles.ts:381-389`, `decentraland/lamb2:src/controllers/handlers/errorHandler.ts:17-25`.
- Place: `services/catalyst/shop/src/market/profiles.rs:528-547`. Decide whether to keep failing closed.
- Severity: P1.

LP-11. **GET /profiles/:id and /profile/:id: synthetic default profiles.**
- Status: MATCH, no work.
- Upstream: the deployed entity for that pointer, assembled with "nothing owned", else 404 (or the stub on the alias). `decentraland/lamb2:src/adapters/profiles.ts:172,285-294`.
- dcl-one: the same `services/catalyst/shop/src/market/profiles.rs:590-616`.
- Severity: none (closed).

LP-12. **/lambdas/about not routed.**
- Status: MATCH, no work.
- Upstream: `GET /lambdas/about`. `decentraland/lamb2:src/controllers/routes.ts:42`.
- dcl-one: `/lambdas/about` and the root `/about` are one handler `services/catalyst/content/src/lib.rs:698-700`.
- Severity: none (closed).

LP-13. **/about: map views.**
- Gap: under the mirror posture `satelliteView` is upstream's (`MAP_SATELLITE_BASE_URL=https://genesis.city/map/latest`, `services/catalyst/config/src/posture.rs:445-450`) and `parcelView` is this node's minimap (`MAP_PARCEL_VIEW_URL` unset). Island and journey omit `satelliteView` and point `parcelView` at the node, by design.
- Expected: always `satelliteView {version:"v1",baseUrl:"https://genesis.city/map/latest",suffixUrl:".jpg",topLeftOffset:{x:-2,y:-6}}` and `parcelView {version:"v1",imageUrl:"https://api.decentraland.org/v1/minimap.png"}`. `decentraland/lamb2:src/controllers/handlers/about-handler.ts:96-113`.
- Place: the mirror row of `MAP_PARCEL_VIEW_URL` in `services/catalyst/config/src/posture.rs`.
- Severity: P1.

LP-14. **/about: comms block.**
- Gap: `adapter` is `fixed-adapter:<x>` with an extra `fixedAdapter` key (or `offline:offline`), driven by `COMMS_FIXED_ADAPTER`.
- Expected (pinned lamb2): `{healthy, protocol:"v3", version, commitHash, usersCount, adapter:"archipelago:archipelago:wss://<host>/archipelago/ws"}`, present when an archipelago URL is configured. `decentraland/lamb2:src/controllers/handlers/about-handler.ts:77-93`.
- Place: `services/catalyst/content/src/about_comms.rs:202-227`. Choose the contract explicitly and record it with a dated capture.
- Severity: P1.

LP-15. **Profiles: wearable dedup.**
- Status: MATCH, no work.
- Upstream: `Array.from(new Set(validated.concat(thirdParty)))`. `decentraland/lamb2:src/adapters/profiles.ts:271`.
- dcl-one: `append_third_party` deduplicates each avatar's merged list `services/catalyst/shop/src/market/profiles.rs:162-180`.
- Severity: none (closed).

LP-16. **Profiles: `base-scene-emotes` and `mainnet` URNs.**
- Gap: dcl-one treats `…:base-scene-emotes:x` as base, and accepts a `:mainnet:` URN as owned.
- Expected: base emote = `includes('urn:decentraland:off-chain:base-emotes')`. Ownership is exact URN equality with the inventory (`ethereum` spelling). `decentraland/lamb2:src/adapters/profiles.ts:51-53,214`.
- Place: `services/catalyst/shop/src/market/profiles.rs:48-53`, `services/catalyst/chain-index/src/graph.rs:162-164`.
- Severity: P2.

LP-17. **Profiles: default-profile processing in POST.**
- Status: MATCH, no work.
- Upstream: nothing owned, so non-base wearables and emotes are dropped and `hasClaimedName:false`. `decentraland/lamb2:src/adapters/profiles.ts:285-294`.
- dcl-one: `services/catalyst/shop/src/market/profiles.rs:439-467`.
- Severity: none (closed).

LP-18. **Profiles: link sanitizing.**
- Gap: `is_safe_http_url` also drops links to loopback, private and single-label hosts.
- Expected: `LinkUrl.validate` only, then a decoded retry. `decentraland/lamb2:src/adapters/profiles.ts:84-106`.
- Place: `services/catalyst/shop/src/market/profiles.rs:279-309`. Confirm the regex against `@dcl/schemas` by capture.
- Severity: P2.

LP-19. **Profiles: legacy `dcl://` bodyShape.**
- Gap: not translated.
- Expected: `parseUrn(...).uri`. `decentraland/lamb2:src/adapters/profiles.ts:55-62,265`.
- Place: `services/catalyst/shop/src/market/profiles.rs:344-375`.
- Severity: P2.

LP-20. **POST /profiles: result order.**
- Gap: request order (≤50 ids) or smallest-pointer byte order.
- Expected: no stable order upstream (content-server cache hits first, then DB order). `decentraland/catalyst:src/logic/active-entities/component.ts:323-356`.
- Place: none required. Pin "order unspecified" in the fixtures so the replay compares as a set.
- Severity: P2.

LP-21. **POST /profiles: malformed bodies.**
- Gap: non-JSON and `null` bodies give 400 (upstream 500). A non-array `ids` gives the parse message (upstream: the "No profile ids…" message). Non-string ids give 400 (upstream `200 []`).
- Expected: `decentraland/lamb2:src/controllers/handlers/profiles-handler.ts:10-14`.
- Place: `services/catalyst/shop/src/market/profiles.rs:670-673`. At least map a non-array `ids` to `MISSING_PROFILE_IDS_MSG`.
- Severity: P2.

LP-22. **Outfits: read-through and failure paths.**
- Status: MATCH, no work.
- Upstream: the content lookup reaches the node's full view; a content failure is 500; a wearables/names failure is 502 `{"error":"The requested items cannot be fetched right now","message":"Cannot fetch elements for <id>"}`. `decentraland/lamb2:src/logic/outfits.ts:31,45,101`, `decentraland/lamb2:src/adapters/elements-fetcher.ts:265-274`.
- dcl-one: `profiles::active_entities` (the read-through when configured); a content failure is a 500 that is not cached; both reader failures the 502 `services/catalyst/shop/src/market/lambdas_catalog.rs:1106-1148`. The 500 body's extra `message` is LP-30.
- Severity: none (closed).

LP-23. **Outfits: shape details.**
- Status: MATCH, no work.
- Upstream: metadata `{outfits, namesForExtraSlots}` only; the raw entity when `metadata` is missing or `outfits` is empty; `<=4` outfits first, then `>4`; an outfit without `slot` dropped. `decentraland/lamb2:src/logic/outfits.ts:37-40,104-124`.
- dcl-one: `services/catalyst/shop/src/market/lambdas_catalog.rs:1120-1126,1276-1298`.
- Severity: none (closed).

LP-24. **/about: health and capacity inputs.**
- Gap: no CPU/memory overload check; an extra console toggle; `content.version` is always present; there is never an `Unknown` sync status; an unknown `ETH_NETWORK` maps to 1.
- Expected: `acceptingUsers = healthy && !(cpu>90% || mem>90%) && userCount < MAX_USERS`; `networkId` from `l1Contracts`, and 500 for an unknown network. `decentraland/lamb2:src/controllers/handlers/about-handler.ts:31-37,70-82`, `decentraland/lamb2:src/adapters/resource-status.ts:8-43`.
- Place: `services/catalyst/content/src/about.rs:108-118,175-223`.
- Severity: P2.

LP-25. **Contracts: failure behaviour.**
- Gap: on an RPC failure dcl-one serves the stale value or `200 []`, and `[]` when no RPC is configured.
- Expected: with no fresh value, the LRU fetch rejects, so 500 `{"error":"Internal Server Error"}`; no stale serving after 6 h. `decentraland/lamb2:src/adapters/catalysts-fetcher.ts:45-59`, `pois-fetcher.ts:21-31`, `name-denylist-fetcher.ts:27-37`.
- Place: `services/catalyst/shop/src/market/contracts.rs:292-431`. Decide whether to keep the more lenient behaviour, and record the decision in the replay fixtures.
- Severity: P2.

LP-26. **Contracts: library normalization.**
- Gap: `@dcl/catalyst-contracts` domain and denylist normalization (trailing `/`, trimming, filtering) is not vendored. dcl-one drops `http://`, adds `https://` and trims whitespace only.
- Expected: capture `GET /lambdas/contracts/servers` and `/denylisted-names` from a pinned lamb2 and diff them.
- Place: `services/catalyst/shop/src/market/contracts.rs:218-243,275-290`.
- Severity: P2.

LP-27. **validate-signature: schema-validator errors.**
- Gap: a schema violation answers the middleware's 400 `{"ok":false,"message":"Invalid JSON body","data":[…]}` (`services/catalyst/shop/src/market/lambdas_crypto.rs:50-99`), but `data` is dcl-one's rendering of ajv's error for the top-level keywords and `[]` for an `authChain` link error. A non-JSON body and a non-JSON Content-Type keep the lambdas-style 400 / no check.
- Expected: the library's `data` (ajv errors against `AuthChain.schema`), its parse-error body, and `400 {"ok":false,"message":"Content-Type must be application/json"}` for another content type; all need a capture of `@dcl/schema-validator-component`. `decentraland/lamb2:src/controllers/handlers/validate-signature-handler.ts:20-29`, `decentraland/lamb2:src/controllers/routes.ts:69-73`.
- Place: `services/catalyst/shop/src/market/lambdas_crypto.rs:50-99,131-139`.
- Severity: P1 (needs capture).

LP-28. **validate-signature: verdict and messages.**
- Gap: `dcl-auth` rejects one-link chains and signed entities mid-chain, and its error texts differ from `@dcl/crypto`.
- Expected: `Authenticator.validateSignature` results and `message` strings. Capture them for a single-SIGNER chain, an expired ephemeral, a bad signature and a final-authority mismatch.
- Place: `services/catalyst/shop/src/market/lambdas_crypto.rs:70-81` (map the errors here rather than changing `library/dcl-auth`).
- Severity: P2.

LP-29. **validate-signature: EIP-1654 without RPC.**
- Status: MATCH, no work.
- Upstream: validated over L1 RPC. `decentraland/lamb2:src/controllers/handlers/validate-signature-handler.ts:60`.
- dcl-one: a mirror node (and a node without `NODE_POSTURE`) always has an `ETH_RPC_URL`: an unset or empty value becomes `https://rpc.decentraland.org/<mainnet|sepolia>` (`services/catalyst/config/src/config.rs:132-156`), which feeds the EIP-1654 validator (`services/catalyst/src/serve.rs:312-315`). Island and journey send no third-party request, so a contract-wallet chain answers `valid:false` there.
- Severity: none (closed).

LP-30. **Shared error bodies.**
- Gap: the 500 body carries a `message`; the 503 label is `Service Unavailable`.
- Expected: 500 `{"error":"Internal Server Error"}`; 503 `{"error":"Service overloaded, retry shortly","message":…}`. `decentraland/lamb2:src/controllers/handlers/errorHandler.ts:17-25,66-77`.
- Place: `services/catalyst/shared/src/http_util.rs:22-36`, or lambdas-specific helpers so other services keep theirs.
- Severity: P2.

LP-31. **CORS max-age and routing edge cases.**
- Gap: `Access-Control-Max-Age` is 86400 (upstream config 36000). Trailing-slash, path-case and 405 behaviour of the upstream router are not read.
- Expected: capture `OPTIONS`, `/lambdas/profiles/`, `/lambdas/Profiles/0x…01` and `PUT /lambdas/profiles` against a pinned lamb2.
- Place: `services/catalyst/shared/src/cors.rs:18`.
- Severity: P2.

LP-32. **Replay fixtures.**
- Gap: `services/catalyst/shop/src/market/fixtures/` covers only the 400/404/stub/status cases, and `lambdas-status-200-shape.json` names a handler file that does not exist (`lambdas.rs`).
- Expected: one fixture per row marked DIVERGENT/GAP above, once fixed, with the `captured_from` revision.
- Place: `services/catalyst/shop/src/market/fixtures/`.
- Severity: P2.

## 13. Endpoints dcl-one serves in this area that upstream does not

- `GET /outfits/{id}` at the root (`services/catalyst/shop/src/market/mod.rs:958`), an alias of `/lambdas/outfits/{id}`.
- `GET /lambdas/explore/realms` (`services/catalyst/content/src/lib.rs:694`, `services/catalyst/content/src/about.rs:255-282`): not a route in lamb2 at the pinned revision.
- The root `GET /about` (`services/catalyst/content/src/lib.rs:693`) is not an upstream lamb2 path. It corresponds to the production node's root `/about`, which is lamb2's `/about` behind the gateway.
