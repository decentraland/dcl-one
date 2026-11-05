# Lambdas conformance: profiles, outfits, about/status, contracts, crypto

- Area: lamb2 `/profiles`, `/profile`, `/outfits`, `/status`, `/about`, `/contracts/*`, `/crypto/validate-signature`.
- Upstream lambdas: `decentraland/lamb2` @ `02a37f5d88fb895339c20e84c2470be68cdf3f57` (2026-10-21), served at `/lambdas/*` in production.
- Upstream content server (it backs `content.fetchEntitiesByPointers`): `decentraland/catalyst` @ `a416565db1e705cff9523540a2578af642598a29` (2026-10-21).
- dcl-one: `dcl-one-conformance` @ `37e91aed` (origin/main, 2026-11-04), workspace `services/catalyst`.
- Method: static reading only, except where a row cites a capture under [`captures/lambdas-profiles/`](captures/lambdas-profiles/): those state what the production peer answered on 2026-11-04 ([`captures/README.md`](captures/README.md)). Nothing in this file was run. "read" means the source was read; "not vendored" means the behaviour lives in an npm package missing from the upstream clone (`@dcl/crypto`, `@dcl/schemas`, `@dcl/catalyst-contracts`, `@dcl/schema-validator-component`, `@well-known-components/http-server`). Those rows need a live capture before anyone acts on them.

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
| 503 body | `{"error":"Service overloaded, retry shortly","message":<msg>}` for `ServiceOverloadedError` (marketplace API bulkhead full, `decentraland/lamb2:src/adapters/marketplace-api-fetcher.ts:136-141`) `decentraland/lamb2:src/controllers/handlers/errorHandler.ts:17-25` | the only 503 in this area (profiles, a reader failure, LP-10) answers that label `services/catalyst/shop/src/market/profiles.rs` (`Failure::Reader`); `service_unavailable` (`Service Unavailable`) is not used by these routes | MATCH (label) |
| 502 body | `{"error":"The requested items cannot be fetched right now","message":"Cannot fetch elements for <address>"}` (`FetcherError`) `decentraland/lamb2:src/controllers/handlers/errorHandler.ts:27-35`, `decentraland/lamb2:src/adapters/elements-fetcher.ts:265-274` | the same body, from `third_party::fetch_error` `services/catalyst/shop/src/market/third_party.rs:1645-1656`; produced only by outfits in this area | MATCH (body); the triggers differ per route (see outfits and the profiles failure rows) |
| 500 body | `{"error":"Internal Server Error"}`, no `message` `decentraland/lamb2:src/controllers/handlers/errorHandler.ts:66-77` | the same on this area's routes (profiles, outfits, contracts) through `lambdas_internal_error` `services/catalyst/shared/src/http_util.rs`; `internal_server_error` (with `message`) stays for the other services that share the helper | MATCH (LP-30) |
| 405 | `router.allowedMethods()` `decentraland/lamb2:src/service.ts:15`. Captured: `PUT` and `GET /lambdas/profiles` answer `405`, empty body, `Content-Length: 0`, `Allow: POST` (`captures/lambdas-profiles/LP-31-put-profiles.json`, `LP-31-get-profiles.json`) | axum method router: 405, empty body, `Allow` with the route's methods; pinned by `services/catalyst/shop/src/market/lambdas_http.rs` (`a_wrong_method_is_an_empty_405`) and `fixtures/profiles-put-405.json` | MATCH for a single-method route (axum joins several methods with `,` and no space; upstream content writes `HEAD, GET`) |
| CORS | http-server `cors: { maxAge: 36000 }` `decentraland/lamb2:src/components.ts:62-69`. Captured preflight (`OPTIONS /lambdas/profiles`): `204`, `access-control-allow-headers` reflected, `access-control-allow-origin: *`, `access-control-max-age: 36000`, `vary: Access-Control-Request-Headers`, no `Allow-Methods`, no `Expose-Headers`. A `GET` with `Origin` gets only `access-control-allow-origin: *` (`captures/lambdas-profiles/LP-31-preflight.json`, `LP-31-get-with-origin.json`) | the `/lambdas/` paths of the shop router: the same headers, plus `X-Content-Type-Options: nosniff` (the peer sends it too); without `Origin` no CORS header `services/catalyst/shop/src/market/lambdas_http.rs:24-66`. The router's other paths (`/outfits`, `/explorer`, `/v1/trades`) and the `/lambdas/` routes of other routers (content's `/lambdas/about`, `/lambdas/explore/realms`, `/lambdas/collections/contents/…`; land-authz's `/lambdas/parcels/…`, `/lambdas/users/…/parcels/…`, `/lambdas/users/…/lands-permissions`) keep the shared CORS (origin echoed with credentials, max-age 86400) `services/catalyst/shared/src/cors.rs` | MATCH for this area's routes; the other routers' `/lambdas/` routes are their areas' |
| Rate limits | none in lamb2 for this area | none; body limit 256 KiB on the two POST routes (413 above it) `services/catalyst/shop/src/market/mod.rs:887,977` | DIVERGENT (413 only on huge bodies) |
| Request timeout | none | `TimeoutLayer` → 408 when `request_timeout` is set `services/catalyst/shop/src/market/mod.rs:986-991` | DIVERGENT (config-dependent) |
| Path case / trailing slash | Captured on the production peer: an unknown path (`/lambdas/nope`), a different case (`/lambdas/Status`, `/lambdas/Profiles/0x…01`) and `/lambdas/status/` answer the gateway's nginx `404` HTML page (`text/html`), not lamb2; `POST /lambdas/profiles/` reaches lamb2 and answers as `/lambdas/profiles` (`captures/lambdas-profiles/LP-31-*.json`) | axum is case-sensitive; a `/lambdas/` path no merged router routes answers the same nginx page, `text/html` (`/lambdas/{*rest}` `services/catalyst/shop/src/market/mod.rs:982`, `lambdas_http.rs:68-76`). A trailing slash is trimmed for the whole app (`services/catalyst/src/serve.rs:1355`), so `POST /lambdas/profiles/` answers as captured and `/lambdas/status/` answers 200 | MATCH except `/lambdas/status/` (200 here, the gateway's 404 on the peer; LP-31) |
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
| `links` | each url kept if `LinkUrl.validate` passes, else `decodeURIComponent` and revalidate, else dropped; `undefined`/`[]` returned as is `profiles.ts:84-106` | the regex `(?i)^(?:https?)://[^\s/$.?#].[^\s]*$`, then the decoded retry; no host check `services/catalyst/shop/src/market/profiles.rs` (`sanitize_links`) | MATCH as far as read: the regex is `LinkUrl`'s in a local `@dcl/schemas` 23.1.0 (`dist/platform/profile/avatar.js`); lamb2 pins `^26.5.0`, not read (LP-18) |
| `hasClaimedName` | always set: `ownedNames.has(avatar.name)`; `false` for default profiles `profiles.ts:197,261` | always set from the owned names (none without `MARKETPLACE_SUBGRAPH_URL`); `false` for default profiles `profiles.rs:215-229,460-467` | MATCH |
| `bodyShape` | legacy `dcl://` ids translated to URN; `?? ''` `profiles.ts:55-62,265` | untouched | DIVERGENT (legacy ids only) |
| snapshots | `face256` and `body` overwritten with `<PROFILE_CDN_BASE_URL>/entities/<entityId>/{face,body}.png`; default base `https://profile-images.decentraland.org`; other snapshot keys kept; a missing object becomes `{face256:'',body:''}` and is then overwritten `profiles.ts:73-82,142,266-270` | the same, with the base `PROFILE_CDN_BASE_URL` (the mirror posture sets `https://profile-images.decentraland.org`, `services/catalyst/config/src/posture.rs:401-407`) or, unset (island and journey), this node's `profile-images` `profiles.rs:230-277` | MATCH (mirror) |
| wearables, base | `includes('base-avatars')` → kept as is `profiles.ts:47-49,207-210` | same `profiles.rs:45-47` | MATCH |
| wearables, on-chain | `splitUrnAndTokenId` (7 segments and not `collections-thirdparty` → urn + tokenId) `decentraland/lamb2:src/logic/utils.ts:51-61`. The URN must equal an owned inventory URN **exactly**. If a tokenId is given it must be one of the owned tokens. Output (`ENSURE_ERC_721` default on, `profiles.ts:141`) is `<ownedUrn>:<tokenId or first owned tokenId>` `profiles.ts:212-229` | same split. Ownership = the token ids held of `(owner, urn)` in the collections subgraph (`urn_in`, subgraph id order); the URN must equal the inventory's exactly, so a `:mainnet:` spelling is not owned (LP-16). A named tokenId must be held; the output is `<ownedUrn>:<tokenId or first held tokenId>` `services/catalyst/shop/src/market/profiles.rs` (`owned_item`), `services/catalyst/chain-index/src/graph.rs:405-462` | DIVERGENT: the first held token follows subgraph order, upstream the marketplace API's `transferredAt` descending order (`captures/lambdas-items/LI-18-marketplace-api-wearables-grouped.json`) |
| wearables, third-party | Not in the owned inventory, so dropped from the validated list. Then appended at the **end**: v1 `collections-thirdparty` URNs of an L2 collection unchecked; linked (v2) items checked through the content-server mapping + on-chain `ownerOf`/`balanceOf`; order v1, then the NFTs on L1, then those on L2 `profiles.ts:178,199-201,271`, `decentraland/lamb2:src/ports/ownership-checker/tpw-ownership-checker.ts:65-95`, `decentraland/lamb2:src/ports/ownership-checker/contract-helpers.ts:57-91`, `decentraland/lamb2:src/ports/ownership-checker/third-party-item-checker.ts:33-100` | the same split, checks and order: `third_party::sanitize_third_party_urns` over every avatar's non-base wearables, appended after the validated wearables of each avatar `profiles.rs:146-180,379-447`, `services/catalyst/shop/src/market/third_party.rs:1446-1643`. Runs only when a collections subgraph is configured. A linked item on a network with no RPC endpoint reads as not owned | MATCH |
| wearables, other off-chain | dropped (not owned) | dropped (`of_urn` → `None`) | MATCH |
| wearables, dedup | `Array.from(new Set(...))` `profiles.ts:271` | the merged list is deduplicated `profiles.rs:162-180` | MATCH |
| third-party check fails (RPC or entity read) | the error propagates; `getProfiles` catches it and the whole batch answers `[]` `profiles.ts:187,381-389` | that profile keeps its v1 third-party items only, logged; the rest of the batch is served `profiles.rs:404-417` | DIVERGENT (P2) |
| emotes, base | kept as the deployed object if `!urn.includes(':')` or `urn.includes('urn:decentraland:off-chain:base-emotes')` `profiles.ts:51-53,234-237` | the same test `services/catalyst/shop/src/market/profiles.rs` (`is_base_emote`); a `base-scene-emotes` URN goes through ownership and is dropped | MATCH (LP-16) |
| emotes, on-chain | same token logic as wearables; output object is exactly `{ urn: "<ownedUrn>:<tokenId>", slot }` `profiles.ts:239-253` | the same `profiles.rs:162-171` | MATCH |
| emotes, absent | `emotes: []` always present `profiles.ts:233,264` | `emotes: []` `profiles.rs:146-176` | MATCH |
| emote without `urn` | `emote.urn.includes` throws → whole batch `[]` | kept | DIVERGENT (edge) |
| default profiles (`pointer` starts with `default`) | nothing owned: every non-base wearable and emote dropped; no TP; `hasClaimedName:false`; no `userId` override `profiles.ts:172,285-294` | the same, with or without a reader `profiles.rs:439-467` | MATCH |
| no ownership reader configured | n/a (the marketplace API or subgraph is always configured) | without a collections subgraph the wearables and emotes are served as deployed (`emotes` still present) `profiles.rs:128-176,439-446`. A mirror refuses to start with either collections subgraph or the marketplace subgraph set empty (`services/catalyst/config/src/posture.rs:711-744`); island and journey set none by design | MATCH (mirror) |
| result order | content-server order: pointer-cache hits first (in request order), then DB misses in DB order `decentraland/catalyst:src/logic/active-entities/component.ts:323-356`, `profiles.ts:375-380` | ≤50 ids, no IMS: request order `profiles.rs:527-564`; otherwise byte order of each entity's smallest matching pointer `services/catalyst/content/src/queries.rs:179-211` | MATCH: upstream has no stable order; the replay compares this answer as a set (`"order": "unspecified"` in `fixtures/profiles-post-200-unknown-id.json`, LP-20) |

**Responses**

| trigger | status | body | headers | upstream | dcl-one | verdict |
|---|---|---|---|---|---|---|
| `ids` missing, not an array, or `[]` | 400 | `{"error":"Bad request","message":"No profile ids were specified. Expected ids:string[] in body"}` | | `profiles-handler.ts:12-14` | the same for any body whose `ids` is not a non-empty array, including a JSON body that is not an object `services/catalyst/shop/src/market/profiles.rs` (`profiles`); `fixtures/profiles-post-400-ids-not-array.json` | MATCH |
| `ids.length > 1000` | 400 | `{"error":"Bad request","message":"Too many profile ids were specified. Maximum allowed is 1000"}` | | `profiles-handler.ts:20-24` | `profiles.rs:649-660` | MATCH |
| body is not JSON, or is `null` | 500 | `{"error":"Internal Server Error"}` (`request.json()` / `body.ids` throws, not mapped) | | `profiles-handler.ts:10`, `errorHandler.ts:66-77` | the same `services/catalyst/shop/src/market/profiles.rs` (`profiles`); `fixtures/profiles-post-500-not-json.json` | MATCH |
| `ids` contains a non-string or `""` | 200 `[]` (content server 400s the batch, lamb2 catches it) | `[]` | | `active-entities-handler.ts:8-23`, `profiles.ts:381-389` | `200 []` for the whole batch `services/catalyst/shop/src/market/profiles.rs` (`profiles`); `fixtures/profiles-post-200-non-string-id.json` | MATCH |
| `If-Modified-Since` parses and every found entity has `floor(ts/1000)*1000 <= ims` (also true when **no** entity is found) | 304 | empty | | `profiles.ts:64-67,342-348` | RFC 2822/7231 and ISO 8601 dates (a date-time without offset is read as UTC); rounded to the second; vacuously true for no entity `profiles.rs:648-681,720-724` | MATCH (the other `Date.parse` forms are not read) |
| `If-Modified-Since` = epoch 0 | IMS ignored (`0` is falsy) | | | `profiles.ts:344` | parsed as 0; 304 only if every ts ≤ 0 | MATCH in effect |
| success | 200 | `Profile[]` (example below) | `Content-Type: application/json` | `profiles-handler.ts:50-53` | `profiles.rs:711-731` | DIVERGENT (assembly table above) |
| unknown ids | 200 | `[]` | | | fixture `services/catalyst/shop/src/market/fixtures/profiles-post-200-unknown-id.json` | MATCH |
| content server unreachable / errors | 200 | `[]` | | `profiles.ts:381-389` | 500 `{"error":"Internal Server Error"}` `profiles.rs` (`Failure::Db`) | DIVERGENT, deliberate (LP-10) |
| ownership/names fetch fails (non-overload) | 200 | `[]` (`FetcherError` is caught inside `getProfiles`) | | `profiles.ts:381-389` | 503 `{"error":"Service overloaded, retry shortly","message":"ownership readers unavailable"}` (lamb2's retryable envelope) `profiles.rs:528-547` | DIVERGENT, deliberate (LP-10: dcl-one fails closed) |
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

dcl-one returns the same object under the mirror posture; without `PROFILE_CDN_BASE_URL` (island, journey) the snapshot URLs are on this node's `profile-images` host.

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
| content server fails | 500 `{"error":"Internal Server Error"}` (content client throws, not mapped) | | `outfits.ts:31` | the same, not cached `services/catalyst/shop/src/market/lambdas_catalog.rs` (`outfits`) | MATCH |
| wearables/names fetch fails | 502 `{"error":"The requested items cannot be fetched right now","message":"Cannot fetch elements for <id>"}` | | `outfits.ts:45,101`, `elements-fetcher.ts:265-274` | the same 502 for both `lambdas_catalog.rs:1135,1191` | MATCH |
| third-party check fails (RPC or entity read) | 500 `{"error":"Internal Server Error"}` (the checker's error is not a `FetcherError`) | | `outfits.ts:85-88`, `errorHandler.ts:66-77` | the same `services/catalyst/shop/src/market/lambdas_catalog.rs` (`sanitize_outfits`) | MATCH (LP-02) |
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

Captured on the production peer: `GET /about` answers lamb2's body below; `GET /lambdas/about` answers the gateway's nginx `404` HTML page, so the production gateway does not expose lamb2's `/about` under `/lambdas`; `GET /content/about` answers the content server's `404` `Not found` (`captures/lambdas-profiles/LP-14-about.json`, `LP-12-lambdas-about.json`, `LP-14-content-about.json`). The captured `/about` has no `comms` key, `content.publicUrl` and `lambdas.publicUrl` end with `/`, and `realmName` is set.

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
| `configurations.map.satelliteView` | always `{version:"v1",baseUrl:"https://genesis.city/map/latest",suffixUrl:".jpg",topLeftOffset:{x:-2,y:-6}}` `:103-108` | present with `MAP_SATELLITE_BASE_URL`, using that value `about.rs:159-167`; the mirror posture sets it to `https://genesis.city/map/latest` (`services/catalyst/config/src/posture.rs:456-460`); island and journey omit the view | MATCH (mirror) |
| `configurations.map.parcelView` | always `{version:"v1",imageUrl:"https://api.decentraland.org/v1/minimap.png"}` `:109-112` | `MAP_PARCEL_VIEW_URL` `about.rs:168-171`; the mirror posture sets `https://api.decentraland.org/v1/minimap.png` (`services/catalyst/config/src/posture.rs:462-468`); island and journey advertise their own minimap | MATCH (mirror) |
| `comms` | present only with `ARCHIPELAGO_URL`: `{healthy, protocol:"v3", version, commitHash, usersCount, adapter:"archipelago:archipelago:wss://<host>/archipelago/ws"}`. `version`/`commitHash` come from archipelago `/status` `:77-93`. The production peer sends **no** `comms` key (`captures/lambdas-profiles/LP-14-about.json`) | present only with `COMMS_FIXED_ADAPTER`: `{healthy, protocol (COMMS_PROTOCOL, default "v3"), version?, commitHash?, usersCount, adapter:"fixed-adapter:<x>", fixedAdapter:"<x>"}`. `"offline:offline"` when the SFU probe fails `services/catalyst/content/src/about_comms.rs:22-37,92-105,202-227` | MATCH with production when `COMMS_FIXED_ADAPTER` is unset (no key); DIVERGENT when it is set: adapter scheme and extra `fixedAdapter` key |
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

Upstream: the L1 `Catalyst` contract via `@dcl/catalyst-contracts` `getCatalystServersFromDAO`, mapped to `{baseUrl: address, owner, id}`. The list is cached in an LRU (max 1, TTL 6 h, no stale serving) `decentraland/lamb2:src/adapters/catalysts-fetcher.ts:45-59`; the handler is at `decentraland/lamb2:src/controllers/handlers/catalyst-servers-handler.ts:4-11`. The library's domain normalization is not vendored. dcl-one drops `http://` domains, prefixes `https://` and trims whitespace; the owner is EIP-55 checksummed and the id is `0x` + 64 lowercase hex `services/catalyst/shop/src/market/contracts.rs:218-257`.

| trigger | status | body | upstream | dcl-one | verdict |
|---|---|---|---|---|---|
| success | 200 | `[{"baseUrl":"https://peer.example.org","owner":"0x0000000000000000000000000000000000000001","id":"0x00…01"}]` in contract index order | `catalysts-fetcher.ts:56-59` | the same keys in the same order (`CatalystServer`) `contracts.rs:218-257,368-392` | MATCH: the production list has only `https://` hosts without a trailing slash, checksummed owners, `0x` + 64-hex ids and keys `baseUrl,owner,id` (`captures/lambdas-profiles/LP-26-contracts-servers.json`) |
| RPC fails, nothing cached / TTL expired | 500 `{"error":"Internal Server Error"}` (the LRU `fetch` rejects) | | `catalysts-fetcher.ts:45-58` | nothing cached: the same 500; a value fetched before: served stale; a failure is not retried for 60 s `services/catalyst/shop/src/market/contracts.rs` (`Slot::get`, `contracts_servers`) | MATCH when cold; DIVERGENT, deliberate, after a first success (LP-25) |
| no RPC configured | n/a (the provider is hard-wired to `rpc.decentraland.org`, `decentraland/lamb2:src/components.ts:167`) | `200 []` when `ETH_RPC_URL` is unset `contracts.rs:355-358` | DIVERGENT (config) |

Data dependency: L1 RPC (`ETH_RPC_URL`); the contract address per network is in `contracts.rs:35-48`.

## 9. GET /lambdas/contracts/pois

Upstream: the L2 POI list contract via `getPoisFromContract`, cached the same way (LRU max 1, 6 h) `decentraland/lamb2:src/adapters/pois-fetcher.ts:21-31`.

| trigger | status | body | upstream | dcl-one | verdict |
|---|---|---|---|---|---|
| success | 200 | `["-10,20","5,5"]` (strings, contract order) | `get-pois-handler.ts:4-11` | `size()`, then `get(i)` `contracts.rs:275-290,378-402` | MATCH as read |
| RPC fails | 500 `{"error":"Internal Server Error"}` | | `pois-fetcher.ts:29-31` | stale value, else the same 500 | MATCH when cold; stale serving is LP-25 |
| no RPC | n/a | | | `200 []` without `RPC_ENDPOINT_POLYGON`/`TRADE_RPC_URLS` 137 | DIVERGENT (config) |

Data dependency: L2 RPC.

## 10. GET /lambdas/contracts/denylisted-names

Upstream: the L1 name-denylist contract via `getNameDenylistFromContract`, LRU max 1, 6 h `decentraland/lamb2:src/adapters/name-denylist-fetcher.ts:27-37`. The production list is served as stored: 7 of its 11 names have upper-case letters and none has surrounding whitespace (`captures/lambdas-profiles/LP-26-denylisted-names.json`); dcl-one returns the raw strings `contracts.rs:421-447`.

| trigger | status | body | upstream | dcl-one | verdict |
|---|---|---|---|---|---|
| success | 200 | `["name1","name2"]` | `get-name-denylist-handler.ts:4-11` | `contracts.rs:421-447` | MATCH (captured) |
| RPC fails | 500 `{"error":"Internal Server Error"}` | | | stale value, else the same 500 | MATCH when cold; stale serving is LP-25 |
| no RPC | n/a | | | `200 []` without `ETH_RPC_URL` | DIVERGENT (config) |

Data dependency: L1 RPC.

## 11. POST /lambdas/crypto/validate-signature

**Request** (`decentraland/lamb2:src/controllers/handlers/validate-signature-handler.ts:20-29`): body `{ timestamp?: string, signedMessage?: string, authChain: AuthChain }`, `additionalProperties: false`, `authChain` required and validated by `AuthChain.schema` (`@dcl/schemas`, not vendored). Validation runs in the `@dcl/schema-validator-component` middleware `decentraland/lamb2:src/controllers/routes.ts:69-73`. Then the handler checks the chain length and the final authority (`signedMessage ?? timestamp`). No auth, no rate limit. EIP-1654 links are checked over `l1Provider` (`rpc.decentraland.org`, `decentraland/lamb2:src/components.ts:167`).

dcl-one (`services/catalyst/shop/src/market/lambdas_crypto.rs:35-331`): the middleware's checks first (Content-Type, JSON parse, the body schema with ajv-shaped `data`), then the handler's. `dcl-auth` verifies the chain, except a lone SIGNER, which is compared with the final authority as `@dcl/crypto` does; the EIP-1654 validator runs only when `ETH_RPC_URL` is set.

| trigger | status | body | upstream | dcl-one | verdict |
|---|---|---|---|---|---|
| `Content-Type` not `application/json`, or absent | **415** | `{"ok":false,"message":"Content-Type must be application/json"}`; `application/json; charset=utf-8` is accepted (`captures/lambdas-profiles/LP-27-text-plain.json`, `LP-27-no-content-type.json`) | middleware | the same; the media type is compared case-insensitively without its parameters `lambdas_crypto.rs:159-167,298-304` | MATCH |
| body not JSON | 400 | `{"ok":false,"message":<V8 JSON.parse message>}`: `Unexpected end of JSON input` for an empty body, `Expected property name or '}' in JSON at position 1 (line 1 column 2)` for `{` (`captures/lambdas-profiles/LP-27-empty-body.json`, `LP-27-not-json.json`) | middleware | the same envelope; V8's text for an empty (or whitespace) body and for an object whose first token is neither a key nor `}`, serde's text for other syntax errors `lambdas_crypto.rs:169-192` | MATCH for the captured shapes; other parse messages differ (needs capture) |
| schema violation (missing `authChain`, unknown key, `timestamp` not a string, bad link `type`) | 400 | `{"ok":false,"message":"Invalid JSON body","data":[<one ajv error>]}`. Captured `data`: missing `authChain` `[{"instancePath":"","schemaPath":"#/required","keyword":"required","params":{"missingProperty":"authChain"},"message":"must have required property 'authChain'"}]`; unknown key `#/additionalProperties`, `params.additionalProperty`; `timestamp: 1` `/timestamp`, `#/properties/timestamp/type`, `must be string`; bad link type `/authChain/0/type`, `#/properties/authChain/items/properties/type/enum`, `params.allowedValues` = the five link types (`captures/lambdas-profiles/LP-27-*.json`) | middleware | the same `data` for each captured case `lambdas_crypto.rs:57-148`; another link error (a missing `type` or `payload`, a non-string field, an `authChain` that is not an array) answers `data: []` | MATCH for the captured cases; other link errors' `data` needs capture |
| `authChain` empty | 400 | the schema rejects it first: `data` `[{"instancePath":"/authChain","schemaPath":"#/properties/authChain/minItems","keyword":"minItems","params":{"limit":1},"message":"must NOT have fewer than 1 items"}]` (`captures/lambdas-profiles/LP-27-empty-chain.json`) | `AuthChain.schema` | the same `lambdas_crypto.rs:119-127` | MATCH |
| `authChain` length > 10 | 400 | `{"error":"Bad request","message":"'authChain' length must be between 1 and 10"}` (not captured) | `validate-signature-handler.ts:51-53` | same message `lambdas_crypto.rs:206-210` | MATCH as read |
| neither `signedMessage` nor `timestamp` (or the chosen one is `""`) | 400 | `{"error":"Bad request","message":"Expected 'signedMessage' property to be set"}` (captured: `captures/lambdas-profiles/LP-27-no-signed-message.json`) | `validate-signature-handler.ts:55-58` | same `lambdas_crypto.rs:211-214` | MATCH (`""` `signedMessage` with a `timestamp`: both reject) |
| valid chain | 200 | `{"valid":true,"ownerAddress":"0x0000000000000000000000000000000000000001"}` (`error` undefined, omitted) | `validate-signature-handler.ts:60-69` | `{"valid":true,"ownerAddress":<authChain[0].payload>}` `lambdas_crypto.rs:326-329` | MATCH |
| invalid chain | 200 | `{"valid":false,"error":<@dcl/crypto message>}`. Captured: a single SIGNER with another `signedMessage` gives `ERROR: Invalid final authority. Expected: hello. Current 0x…01.`; a SIGNER, an `ECDSA_EPHEMERAL` with a bad signature and a signed entity give `ERROR. Link type: ECDSA_EPHEMERAL. Invalid ethereum signature.` (`captures/lambdas-profiles/LP-28-single-signer-other.json`, `LP-28-bad-signature.json`) | same | `{"valid":false,"error":…}`: the two captured texts for a final-authority mismatch and an `ECDSA_EPHEMERAL` signature that does not recover `lambdas_crypto.rs:224-266`; any other failure keeps the `dcl-auth` text (`library/dcl-auth/src/error.rs`) | MATCH for the captured messages; the others differ (needs capture) |
| single SIGNER link whose payload equals `signedMessage` | 200 `{"valid":true,"ownerAddress":"0x…01"}` (captured: `captures/lambdas-profiles/LP-28-single-signer-match.json`) | | | the same: a lone SIGNER is compared with the final authority before `dcl-auth` `lambdas_crypto.rs:268-280` | MATCH |
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
- Status: MATCH, no work.
- The ownership filter matches upstream: an outfit with a non-base, non-off-chain wearable not owned (tokenId-aware) or a TP item not owned is dropped; owned wearables become `<ownedUrn>:<tokenId>`; every kept wearable is lowercased; the `break` quirk is kept. `decentraland/lamb2:src/logic/outfits.ts:45-116`; `services/catalyst/shop/src/market/lambdas_catalog.rs` (`sanitize_outfits`).
- Upstream: the third-party checker's error is not one `errorHandler` maps, so 500 `{"error":"Internal Server Error"}`. `decentraland/lamb2:src/logic/outfits.ts:85-88`, `errorHandler.ts:66-77`.
- dcl-one: a failed third-party check answers that bare 500 (`lambdas_internal_error`); a failed wearables or names read keeps lamb2's 502 `FetcherError`. Pinned by `a_failed_third_party_check_is_lamb2s_bare_500` and `lambdas_tests.rs` (`the_g1_lambdas_answer_from_the_mirrors_indexes_and_readers`). Without a collections subgraph nothing is checked (as LP-07).
- Severity: none (closed).

LP-03. **Profiles: ERC-721 token handling.**
- Status: MATCH, no work.
- Upstream: a named tokenId must be among the owned tokens, else the item is dropped; output `<ownedUrn>:<tokenId or first owned tokenId>` (`ENSURE_ERC_721` default true); owned emotes re-emitted as exactly `{urn, slot}`. `decentraland/lamb2:src/adapters/profiles.ts:141,212-229,239-253`.
- dcl-one: the same over `graph::owned_tokens` `services/catalyst/shop/src/market/profiles.rs:108-176`, `services/catalyst/chain-index/src/graph.rs:405-462`. The first held token follows subgraph id order (lamb2's subgraph fallback order). The marketplace API, lamb2's primary source, orders `individualData` by `transferredAt` descending, so upstream's first owned tokenId is the most recently received copy (`captures/lambdas-items/LI-18-marketplace-api-wearables-grouped.json`). The `mainnet` spelling is LP-16.
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
- Status: MATCH, no work.
- Upstream: always filters.
- dcl-one: a mirror refuses to start when `ETH_COLLECTIONS_SUBGRAPH_URL`, `MATIC_COLLECTIONS_SUBGRAPH_URL` or `MARKETPLACE_SUBGRAPH_URL` is set empty (the posture names all three) `services/catalyst/config/src/posture.rs:654-656,711-744`. Island and journey have no ownership source by design and serve the items as deployed `services/catalyst/shop/src/market/profiles.rs:439-446`.
- Severity: none (closed).

LP-08. **Profiles: snapshot URLs.**
- Status: MATCH, no work.
- Upstream: `face256 = <PROFILE_CDN_BASE_URL or https://profile-images.decentraland.org>/entities/<entityId>/face.png`, `body = …/body.png`, every other snapshot key untouched, a missing object created. `decentraland/lamb2:src/adapters/profiles.ts:73-82,142,266-270`.
- dcl-one: the same `services/catalyst/shop/src/market/profiles.rs:230-277`; the mirror posture sets `PROFILE_CDN_BASE_URL=https://profile-images.decentraland.org` `services/catalyst/config/src/posture.rs:401-407`. Island and journey leave it unset and point at this node's `/profile-images` (sync never copies snapshot files).
- Severity: none (closed).

LP-09. **POST /profiles: `If-Modified-Since`.**
- Status: MATCH, no work.
- Upstream: `Date.parse` of the header; 304 when every found entity has `floor(timestamp/1000)*1000 <= ims`, including when none is found. `decentraland/lamb2:src/controllers/handlers/profiles-handler.ts:26-48`, `decentraland/lamb2:src/adapters/profiles.ts:64-67,342-348`.
- dcl-one: RFC 2822/7231 and ISO 8601 dates (a date-time without an offset read as UTC; `Date.parse` reads it in the server's zone), the same rounding and the same empty-match 304 `services/catalyst/shop/src/market/profiles.rs:648-681,720-724`. Other `Date.parse` forms are not read.
- Severity: none (closed).

LP-10. **Profiles: failure semantics.**
- Deliberate difference: on a DB failure dcl-one answers 500; on a reader failure, 503 `{"error":"Service overloaded, retry shortly","message":"ownership readers unavailable"}` (lamb2's retryable envelope). dcl-one fails closed: a `[]`, 404 or stub reads to a client as "no profile", and an unproven inventory is not served.
- Upstream: `200 []` (POST), 404 (`/profiles/:id`) or the stub (`/profile/:id`) for any failure except overload; overload is 503 with `"error":"Service overloaded, retry shortly"`. `decentraland/lamb2:src/adapters/profiles.ts:381-389`, `decentraland/lamb2:src/controllers/handlers/errorHandler.ts:17-25`.
- Place: `services/catalyst/shop/src/market/profiles.rs:528-547`.
- Severity: none (kept by decision).

LP-11. **GET /profiles/:id and /profile/:id: synthetic default profiles.**
- Status: MATCH, no work.
- Upstream: the deployed entity for that pointer, assembled with "nothing owned", else 404 (or the stub on the alias). `decentraland/lamb2:src/adapters/profiles.ts:172,285-294`.
- dcl-one: the same `services/catalyst/shop/src/market/profiles.rs:590-616`.
- Severity: none (closed).

LP-12. **/lambdas/about not routed.**
- Status: MATCH, no work.
- Upstream: `GET /lambdas/about`. `decentraland/lamb2:src/controllers/routes.ts:42`.
- dcl-one: `/lambdas/about` and the root `/about` are one handler `services/catalyst/content/src/lib.rs:698-700`.
- Capture: the production peer's gateway does not expose it: `GET /lambdas/about` answers the nginx `404` HTML page (`captures/lambdas-profiles/LP-12-lambdas-about.json`). A client of the public peer cannot depend on it.
- Severity: none (closed).

LP-13. **/about: map views.**
- Status: MATCH, no work.
- Upstream: always `satelliteView {version:"v1",baseUrl:"https://genesis.city/map/latest",suffixUrl:".jpg",topLeftOffset:{x:-2,y:-6}}` and `parcelView {version:"v1",imageUrl:"https://api.decentraland.org/v1/minimap.png"}`. `decentraland/lamb2:src/controllers/handlers/about-handler.ts:96-113`.
- dcl-one: the mirror posture sets `MAP_SATELLITE_BASE_URL=https://genesis.city/map/latest` and `MAP_PARCEL_VIEW_URL=https://api.decentraland.org/v1/minimap.png` `services/catalyst/config/src/posture.rs:456-468`. Island and journey omit the satellite view and advertise their own minimap, by design (no third-party host).
- Severity: none (closed).

LP-14. **/about: comms block.**
- Gap: `adapter` is `fixed-adapter:<x>` with an extra `fixedAdapter` key (or `offline:offline`), driven by `COMMS_FIXED_ADAPTER`.
- Expected (pinned lamb2): `{healthy, protocol:"v3", version, commitHash, usersCount, adapter:"archipelago:archipelago:wss://<host>/archipelago/ws"}`, present when an archipelago URL is configured. `decentraland/lamb2:src/controllers/handlers/about-handler.ts:77-93`.
- Capture: the production peer's `/about` has no `comms` key at all (`captures/lambdas-profiles/LP-14-about.json`, 2026-11-04). dcl-one sends none when `COMMS_FIXED_ADAPTER` is unset.
- Place: `services/catalyst/content/src/about_comms.rs:202-227`. Choose the contract explicitly.
- Severity: P1.

LP-15. **Profiles: wearable dedup.**
- Status: MATCH, no work.
- Upstream: `Array.from(new Set(validated.concat(thirdParty)))`. `decentraland/lamb2:src/adapters/profiles.ts:271`.
- dcl-one: `append_third_party` deduplicates each avatar's merged list `services/catalyst/shop/src/market/profiles.rs:162-180`.
- Severity: none (closed).

LP-16. **Profiles: `base-scene-emotes` and `mainnet` URNs.**
- Status: MATCH, no work.
- Upstream: base emote = `includes('urn:decentraland:off-chain:base-emotes')`. Ownership is exact URN equality with the inventory (`ethereum` spelling). `decentraland/lamb2:src/adapters/profiles.ts:51-53,214`.
- dcl-one: `is_base_emote` is that substring test, so a `base-scene-emotes` emote goes through ownership and is dropped; `owned_item` looks the URN up as deployed, so a `:mainnet:` spelling is not owned `services/catalyst/shop/src/market/profiles.rs`. Pinned by `base_emotes_are_the_off_chain_base_emotes_only` and `ownership_filter_reemits_owned_emotes_as_urn_and_slot`. `graph::normalize_urn` is unchanged for its other callers.
- Severity: none (closed).

LP-17. **Profiles: default-profile processing in POST.**
- Status: MATCH, no work.
- Upstream: nothing owned, so non-base wearables and emotes are dropped and `hasClaimedName:false`. `decentraland/lamb2:src/adapters/profiles.ts:285-294`.
- dcl-one: `services/catalyst/shop/src/market/profiles.rs:439-467`.
- Severity: none (closed).

LP-18. **Profiles: link sanitizing.**
- Status: MATCH, no work, as far as the package was read.
- Upstream: `LinkUrl.validate` only, then a decoded retry. `decentraland/lamb2:src/adapters/profiles.ts:84-106`.
- dcl-one: the same regex and retry, without the `is_safe_http_url` host check (this node never fetches these URLs) `services/catalyst/shop/src/market/profiles.rs` (`sanitize_links`); pinned by `links_keep_what_link_url_accepts`.
- Left open: the regex was compared with a local `@dcl/schemas` 23.1.0 (`dist/platform/profile/avatar.js`: pattern `^(?:https?):\/\/[^\s/$.?#].[^\s]*$`, flag `i`); lamb2 pins `^26.5.0`, which was not read.
- Severity: none (closed, subject to the 26.x check).

LP-19. **Profiles: legacy `dcl://` bodyShape.**
- Gap: not translated.
- Expected: `parseUrn(...).uri`. `decentraland/lamb2:src/adapters/profiles.ts:55-62,265`.
- Place: `services/catalyst/shop/src/market/profiles.rs:344-375`.
- Left open (not vendored): the translation is `@dcl/urn-resolver`'s, which is not in the upstream clone nor in any local `node_modules`; no capture holds a profile with a `dcl://` bodyShape. Read `resolveLegacyDclUrl` at the version lamb2 pins, or capture such a profile, before changing it.
- Severity: P2.

LP-20. **POST /profiles: result order.**
- Status: MATCH, no work.
- Upstream: no stable order (content-server cache hits first, then DB order). `decentraland/catalyst:src/logic/active-entities/component.ts:323-356`.
- dcl-one: request order (≤50 ids) or smallest-pointer byte order; the replay compares the answer as a set: `"order": "unspecified"` in `services/catalyst/shop/src/market/fixtures/profiles-post-200-unknown-id.json`, read by `tests.rs` (`replay_one`).
- Severity: none (closed).

LP-21. **POST /profiles: malformed bodies.**
- Status: MATCH, no work.
- Upstream: a body that is not JSON, or `null`, throws (500 `{"error":"Internal Server Error"}`); any body whose `ids` is not a non-empty array is the "No profile ids…" 400; an id that is not a non-empty string fails the content server's body schema, which `getProfiles` turns into `200 []`. `decentraland/lamb2:src/controllers/handlers/profiles-handler.ts:10-14`, `decentraland/lamb2:src/adapters/profiles.ts:381-389`, `decentraland/catalyst:src/controllers/handlers/active-entities-handler.ts:8-23`.
- dcl-one: the same `services/catalyst/shop/src/market/profiles.rs` (`profiles`); pinned by `malformed_profile_bodies_answer_as_lamb2` and the fixtures `profiles-post-500-not-json.json`, `profiles-post-400-ids-not-array.json`, `profiles-post-200-non-string-id.json`.
- Severity: none (closed).

LP-22. **Outfits: read-through and failure paths.**
- Status: MATCH, no work.
- Upstream: the content lookup reaches the node's full view; a content failure is 500; a wearables/names failure is 502 `{"error":"The requested items cannot be fetched right now","message":"Cannot fetch elements for <id>"}`. `decentraland/lamb2:src/logic/outfits.ts:31,45,101`, `decentraland/lamb2:src/adapters/elements-fetcher.ts:265-274`.
- dcl-one: `profiles::active_entities` (the read-through when configured); a content failure is the bare 500 (not cached); both reader failures the 502 `services/catalyst/shop/src/market/lambdas_catalog.rs` (`outfits`).
- Severity: none (closed).

LP-23. **Outfits: shape details.**
- Status: MATCH, no work.
- Upstream: metadata `{outfits, namesForExtraSlots}` only; the raw entity when `metadata` is missing or `outfits` is empty; `<=4` outfits first, then `>4`; an outfit without `slot` dropped. `decentraland/lamb2:src/logic/outfits.ts:37-40,104-124`.
- dcl-one: `services/catalyst/shop/src/market/lambdas_catalog.rs:1120-1126,1276-1298`.
- Severity: none (closed).

LP-24. **/about: health and capacity inputs.**
- Gap: no CPU/memory overload check; an extra console toggle; an unknown `ETH_NETWORK` maps to 1.
- Expected: `acceptingUsers = healthy && !(cpu>90% || mem>90%) && userCount < MAX_USERS`; `networkId` from `l1Contracts`, and 500 for an unknown network. `decentraland/lamb2:src/controllers/handlers/about-handler.ts:31-37,70-82`, `decentraland/lamb2:src/adapters/resource-status.ts:8-43`.
- Not a gap: `content.version` is always present and the sync status is never `Unknown` because the content server is in-process, so it is always reachable; upstream answers the same values when its content server answers `/status`.
- Left open (design decision): the CPU/memory check reads host-wide figures (`os.loadavg()` per CPU, `totalmem - freemem`). A dcl-one node runs content, lambdas, comms and often Postgres on one host, so sync, a build or the database's load would take it out of the realm pickers, and the integration tests that expect `acceptingUsers: true` would fail on a loaded host. The console toggle is a dcl-one feature. The `l1Contracts` key set is not vendored (`@dcl/catalyst-contracts`).
- Place: `services/catalyst/content/src/about.rs:108-118,175-223`.
- Severity: P2.

LP-25. **Contracts: failure behaviour.**
- Status: MATCH, no work, when nothing is cached; stale serving kept by decision.
- Upstream: with no fresh value, the LRU fetch rejects, so 500 `{"error":"Internal Server Error"}`; no stale serving after 6 h. `decentraland/lamb2:src/adapters/catalysts-fetcher.ts:45-59`, `pois-fetcher.ts:21-31`, `name-denylist-fetcher.ts:27-37`.
- dcl-one: a failure with nothing cached answers that 500 on all three routes `services/catalyst/shop/src/market/contracts.rs`; pinned by `a_cold_failure_is_lamb2s_bare_500`. After one success the last good list is served stale while the RPC fails (a one-minute back-off), and without an RPC endpoint (`island`, `journey`) the lists answer `[]`.
- Deliberate difference: stale serving after 6 h. The lists change rarely (a DAO vote), and an old list is closer to the truth than a 500 for a client that reads the catalyst list or the name denylist.
- Severity: none (closed).

LP-26. **Contracts: library normalization.**
- Status: MATCH, no work.
- Upstream: every production `baseUrl` is `https://` with no trailing slash or whitespace, owners are checksummed, ids are `0x` + 64 hex, keys in order `baseUrl,owner,id` (`captures/lambdas-profiles/LP-26-contracts-servers.json`). The 11 denylisted names are served as stored: 7 have upper-case letters, none has surrounding whitespace (`LP-26-denylisted-names.json`).
- dcl-one: the same values; the servers answer through a struct in that key order `services/catalyst/shop/src/market/contracts.rs:218-257,368-392`; the denylist is returned raw `contracts.rs:421-447`.
- Severity: none (closed).

LP-27. **validate-signature: schema-validator errors.**
- Status: MATCH, no work, for every captured case.
- Upstream: `415 {"ok":false,"message":"Content-Type must be application/json"}` for another or a missing content type; `400 {"ok":false,"message":<V8 parse message>}` for a body that is not JSON; `400 {"ok":false,"message":"Invalid JSON body","data":[<one ajv error>]}` for a schema violation, including an empty `authChain` and an unknown link `type` (`captures/lambdas-profiles/LP-27-*.json`). `decentraland/lamb2:src/controllers/handlers/validate-signature-handler.ts:20-29`, `decentraland/lamb2:src/controllers/routes.ts:69-73`.
- dcl-one: the same statuses and bodies `services/catalyst/shop/src/market/lambdas_crypto.rs:57-192,293-323`; pinned by the module's tests and `services/catalyst/shop/src/market/fixtures/validate-signature-400-empty-chain.json`.
- Left open (needs capture): V8's text for other syntax errors (serde's text is answered), and ajv's `data` for link errors other than the `type` enum (`data: []` is answered).
- Severity: none for the captured cases; P2 for the rest.

LP-28. **validate-signature: verdict and messages.**
- Status: MATCH, no work, for every captured case.
- Upstream: a single SIGNER whose payload equals `signedMessage` is `{"valid":true,"ownerAddress":…}`; a final-authority mismatch is `ERROR: Invalid final authority. Expected: <message>. Current <authority>.`; a bad ephemeral signature is `ERROR. Link type: ECDSA_EPHEMERAL. Invalid ethereum signature.` (`captures/lambdas-profiles/LP-28-*.json`).
- dcl-one: a lone SIGNER is compared with the final authority before `dcl-auth`; the final-authority mismatch and an `ECDSA_EPHEMERAL` link whose signature does not recover map to those texts `services/catalyst/shop/src/market/lambdas_crypto.rs:224-291`; `library/dcl-auth` is unchanged; pinned by `fixtures/validate-signature-200-single-signer.json`.
- Left open (needs capture): the other `@dcl/crypto` texts (signer mismatch, missing signature, expired ephemeral, signed-entity and EIP-1654 failures) keep `dcl-auth`'s; `dcl-auth` still rejects a signed entity mid-chain. An expired ephemeral needs a chain signed by a real key and was not captured.
- Severity: P2 (rest).

LP-29. **validate-signature: EIP-1654 without RPC.**
- Status: MATCH, no work.
- Upstream: validated over L1 RPC. `decentraland/lamb2:src/controllers/handlers/validate-signature-handler.ts:60`.
- dcl-one: a mirror node (and a node without `NODE_POSTURE`) always has an `ETH_RPC_URL`: an unset or empty value becomes `https://rpc.decentraland.org/<mainnet|sepolia>` (`services/catalyst/config/src/config.rs:132-156`), which feeds the EIP-1654 validator (`services/catalyst/src/serve.rs:312-315`). Island and journey send no third-party request, so a contract-wallet chain answers `valid:false` there.
- Severity: none (closed).

LP-30. **Shared error bodies.**
- Status: MATCH, no work, for this area's routes.
- Upstream: 500 `{"error":"Internal Server Error"}`; 503 `{"error":"Service overloaded, retry shortly","message":…}`. `decentraland/lamb2:src/controllers/handlers/errorHandler.ts:17-25,66-77`.
- dcl-one: `lambdas_internal_error` `services/catalyst/shared/src/http_util.rs` answers the bare 500 on profiles, outfits and contracts; the only 503 here (profiles, LP-10) already carries lamb2's label. `internal_server_error` and `service_unavailable` keep their bodies for the other services that share them; each lambdas area switches its own routes.
- Severity: none (closed).

LP-31. **CORS max-age and routing edge cases.**
- Status: MATCH, no work, except a trailing slash on a GET route.
- Upstream (`captures/lambdas-profiles/LP-31-*.json`): the preflight answers `204` with `access-control-allow-origin: *`, the requested headers, `access-control-max-age: 36000` and `vary: Access-Control-Request-Headers` only; a `GET` with `Origin` gets `access-control-allow-origin: *` only. `PUT`/`GET /lambdas/profiles` answer `405`, empty body, `Allow: POST`. `POST /lambdas/profiles/` answers as `/lambdas/profiles`. `/lambdas/Profiles/…`, `/lambdas/Status`, `/lambdas/status/` and `/lambdas/nope` answer the gateway's nginx `404` HTML page.
- dcl-one: lamb2's CORS on the `/lambdas/` paths of the shop router and the nginx page for any `/lambdas/` path no router matches `services/catalyst/shop/src/market/lambdas_http.rs`, `services/catalyst/shop/src/market/mod.rs:982-988`; the 405 is axum's. Other routers keep their CORS (see the cross-cutting CORS row).
- Gap: `/lambdas/status/` answers 200: the app trims a trailing slash before routing (`services/catalyst/src/serve.rs:1355`), which the explorer's `/api/events/` needs. Only `/lambdas/status/` (404) and `POST /lambdas/profiles/` (served) are captured, so the gateway's rule per route is unknown; a client of the peer cannot depend on the 404.
- Severity: P2 (edge).

LP-32. **Replay fixtures.**
- Status: MATCH, no work.
- dcl-one: `services/catalyst/shop/src/market/fixtures/` now pins this area's fixed rows: `profiles-post-500-not-json.json`, `profiles-post-400-ids-not-array.json`, `profiles-post-200-non-string-id.json` (LP-21), and `profiles-post-200-unknown-id.json` as a set (LP-20). Each of this area's fixtures names its source: the capture it mirrors, or the lamb2 and dcl-one functions it was written from (`lambdas-status-200-shape.json` no longer names a missing `lambdas.rs`). The replay reads a raw `body_raw` request body and `"order": "unspecified"` (`tests.rs`). Rows fixed by unit tests only (LP-02, LP-16, LP-18, LP-25) need a reader or an RPC the replay does not start.
- Severity: none (closed).

## 13. Endpoints dcl-one serves in this area that upstream does not

- `GET /outfits/{id}` at the root (`services/catalyst/shop/src/market/mod.rs:958`), an alias of `/lambdas/outfits/{id}`.
- `GET /lambdas/explore/realms` (`services/catalyst/content/src/lib.rs:694`, `services/catalyst/content/src/about.rs:255-282`): not a route in lamb2 at the pinned revision.
- The root `GET /about` (`services/catalyst/content/src/lib.rs:693`) is not an upstream lamb2 path. It corresponds to the production node's root `/about`, which is lamb2's `/about` behind the gateway.
