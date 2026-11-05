# Lambdas: names, LAND and permissions

Conformance backlog for the lambdas names, LAND and parcel-permission endpoints.

- Upstream lambdas: `decentraland/lamb2` @ `02a37f5d88fb895339c20e84c2470be68cdf3f57` (2026-10-21), served in production under `/lambdas/*`.
- dcl-one: `dcl-one-conformance` @ `37e91aed` (origin/main, 2026-11-04), Rust workspace `services/catalyst`.
- Method: static reading only. Nothing was built or run, so nothing in this file is verified. Every verdict is "read", except where a row cites a capture under [`captures/lambdas-land/`](captures/lambdas-land/): those state what the production peer, its subgraphs and the marketplace API answered on 2026-11-04 ([`captures/README.md`](captures/README.md)).
- `@dcl/schemas` `Parcel.validate` comes from a local `@dcl/schemas` 23.1.0 copy (`dist/core/parcel-validation.js`, `dist/core/parcel-exceptions.js`). lamb2 pins `^26.5.0`. The exception blocks are assumed to still exist in 26.x; this was not read in 26.x. dcl-one's `services/catalyst/shared/src/parcel.rs:1-13` states the same four ranges for 26.5.0.

Upstream files read:
- `src/controllers/routes.ts`
- `src/controllers/handlers/{names,name-owner,lands,user-lands-permissions,parcel-permissions,parcel-operators}-handler.ts`, `errorHandler.ts`, `pagination-utils.ts`
- `src/logic/pagination.ts`, `pagination-constants.ts`, `api-with-fallback.ts`
- `src/logic/fetch-elements/{fetch-names,fetch-name-owner,fetch-lands,fetch-permissions,fetch-elements}.ts`
- `src/adapters/{parcel-rights-fetcher,elements-fetcher,marketplace-api-fetcher}.ts`, `src/ports/the-graph.ts`
- `src/components.ts`, `src/service.ts`, `src/types.ts`
- tests under `test/integration/` and `test/data/` for these routes

dcl-one files read:
- `land-authz/src/{routes,lib,resolve,events}.rs`, and `indexer.rs` (cursor and readiness parts)
- `shared/src/parcel.rs`
- `shop/src/market/{lambdas_land,user_items,readers,nft_reads,nft_index,chain_walk,mod}.rs`
- `chain-index/src/{graph,land_graph,source}.rs`
- `shared/src/{http_util,query_params,identity,cors}.rs`
- `config/src/{config,shop,node,land_authz,posture}.rs`, `src/serve.rs` (mounts)
- `schemas/33_land_authz.sql`
- fixture `shop/src/market/fixtures/users-names-200-pagesize-cap-1000.json`

## Where dcl-one gets the data

| Data | Upstream source | dcl-one source |
|---|---|---|
| Names owned (`/users/:a/names`) | Primary: marketplace-server API `/v1/users/:a/names`. Fallback: marketplace subgraph `nfts(category: ens)`. Defaults point at the production URLs (`decentraland/lamb2:src/ports/the-graph.ts:19`). | Only the marketplace subgraph reader (`MARKETPLACE_SUBGRAPH_URL`, set by `NODE_POSTURE=mirror` and unset otherwise; `services/catalyst/config/src/posture.rs:272-276`, `services/catalyst/chain-index/src/graph.rs:491`). The nft chain index (`shop.nft`) is never consulted for this route. |
| Name owner | Marketplace subgraph `searchText` lookup | Own nft chain index (`shop.nft` rows of the DCLRegistrar, from `eth_getLogs` over `ETH_RPC_URL`) once `INDEXER_NFT`/ethereum is published. Before that, the marketplace subgraph if configured, else 503 (`services/catalyst/shop/src/market/lambdas_land.rs:349`). |
| LAND and estates owned | Marketplace subgraph `nfts(category_in:[parcel,estate])` | Own land-authz index (`land_authz.token_right`) plus the nft index (`shop.land_data` for names and descriptions, `shop.nft_order` for prices) once both are published. Before that, the marketplace subgraph if configured, else 503 (`services/catalyst/shop/src/market/lambdas_land.rs:174`). |
| Parcel operators, update operators, update managers, approved-for-all | LAND (land-manager) subgraph `parcels`/`estates`/`authorizations` | Own land-authz index, folded from LAND and Estate registry logs (`UpdateOperator`, `UpdateManager`, `ApprovalForAll`, `Approval`, `Transfer`, `AddLand`, `RemoveLand`; `services/catalyst/land-authz/src/events.rs:1`), once published. Before that: the land-manager subgraph (`LAND_SUBGRAPH_URL`, set by `NODE_POSTURE=mirror` and unset otherwise; `services/catalyst/config/src/posture.rs:282-288`) for `/operators`, `/lands-permissions` and `/permissions`. |
| Rentals | Not read by any of these upstream routes. A rented LAND's owner is the Rentals contract and the tenant is its `updateOperator`/`operator`, both on chain. | Not read either. The same on-chain facts come through `Transfer` and `UpdateOperator` logs, so both trees treat a tenant as `updateOperator` or `operator`. Neither tree reads the rentals API or subgraph for these routes. |

**What a node answers, by posture** (`NODE_POSTURE`, `services/catalyst/config/src/posture.rs`). Content sync feeds none of these routes.

- `NODE_POSTURE=mirror` sets `MARKETPLACE_SUBGRAPH_URL` and `LAND_SUBGRAPH_URL` to the marketplace and land-manager subgraphs lamb2 defaults to, and turns on `LAND_AUTHZ_INDEX_ENABLED` and `MARKETPLACE_NFT_INDEX_ENABLED` (`services/catalyst/config/src/posture.rs:272-297`).
  - Until the indexes publish, names, name owner, lands, operators, lands-permissions and permissions come from the same subgraphs upstream reads, with the divergences listed below. After that they come from the indexes.
- With `NODE_POSTURE` unset (the binary's defaults), `island` or `journey`, no subgraph URL is set and `LAND_AUTHZ_INDEX_ENABLED` is false (`services/catalyst/config/src/land_authz.rs:10`, `services/catalyst/land-authz/src/lib.rs:173-174`).
  - Every route answers the 503 `unavailable` envelope, except `/permissions`, which answers lamb2's bare 500.
  - Setting `MARKETPLACE_SUBGRAPH_URL` and `LAND_SUBGRAPH_URL` by hand gives the mirror behaviour without the indexes.
- `/genesis-parcels` answers 503 under every posture until the land-authz index is published.

## Endpoint inventory

| Method | Upstream path | Prod prefix | dcl-one route | Status |
|---|---|---|---|---|
| GET | `/users/:address/names` | `/lambdas` | `/lambdas/users/{address}/names`: `services/catalyst/shop/src/market/mod.rs:900` → `services/catalyst/shop/src/market/user_items.rs:339` | DIVERGENT |
| GET | `/names/:name/owner` | `/lambdas` | `/lambdas/names/{name}/owner`: `services/catalyst/shop/src/market/mod.rs:940` → `services/catalyst/shop/src/market/lambdas_land.rs:328` | DIVERGENT |
| GET | `/users/:address/lands` | `/lambdas` | `/lambdas/users/{address}/lands`: `services/catalyst/shop/src/market/mod.rs:933` → `services/catalyst/shop/src/market/lambdas_land.rs:174` | DIVERGENT |
| GET | `/users/:address/lands-permissions` | `/lambdas` | `services/catalyst/land-authz/src/routes.rs:47` → `services/catalyst/land-authz/src/routes.rs:180` | DIVERGENT |
| GET | `/users/:address/parcels/:x/:y/permissions` | `/lambdas` | `services/catalyst/land-authz/src/routes.rs:51` → `services/catalyst/land-authz/src/routes.rs:280` | DIVERGENT |
| GET | `/parcels/:x/:y/operators` | `/lambdas` | `services/catalyst/land-authz/src/routes.rs:45` → `services/catalyst/land-authz/src/routes.rs:135` | DIVERGENT |

Counts: 6 endpoints. MATCH 0, GAP 0, DIVERGENT 6, MISSING 0.

### Behaviour shared by all six routes

| Aspect | Upstream | dcl-one | Verdict |
|---|---|---|---|
| Error envelope | `{error, message}`. Handled errors map as follows (`decentraland/lamb2:src/controllers/handlers/errorHandler.ts:6`): `InvalidRequestError` → 400 `"Bad request"`, `ServiceOverloadedError` → 503 `"Service overloaded, retry shortly"`, `FetcherError` → 502 `"The requested items cannot be fetched right now"`, `NotFoundError` → 404 `"Not Found"`, `ParcelOrStateNotFoundError` → 404 `"Bad Request"`. Anything else → 500 `{"error":"Internal Server Error"}` with **no** `message` (`errorHandler.ts:71`). | `bad_request` matches the 400 label (`services/catalyst/shared/src/http_util.rs:14`). 500 is `{"error":"Internal Server Error","message":"Internal Server Error"}` (`http_util.rs:30`), except on `/permissions`, which answers lamb2's bare 500 (`services/catalyst/land-authz/src/routes.rs:94-100`). A configured subgraph that fails answers lamb2's 502 `FetcherError` on the element routes (`fetcher_error`, `services/catalyst/shared/src/http_util.rs:41`) and the bare 500 on `/operators` and `/permissions`. An unconfigured or unpublished source answers 503 `{"error":"Service Unavailable","message":…,"source":…,"configure"?:…}` plus `Retry-After: 60` (`services/catalyst/chain-index/src/source.rs:88`), which has no upstream trigger. | DIVERGENT (the 500 `message`; the 503 has no upstream trigger) |
| Trailing slash | `path-to-regexp` non-strict: accepted | `NormalizePath::trim_trailing_slash` (`services/catalyst/src/serve.rs:1304`) | MATCH |
| Path case | The well-known-components router uses `path-to-regexp` defaults (case-insensitive), so `/USERS/...` matches | axum is case-sensitive, so 404 | DIVERGENT (P2; covered by the cross-cutting area) |
| 405 | `router.allowedMethods()` (`decentraland/lamb2:src/service.ts:15`) | axum 405 with `Allow` | MATCH (status) |
| CORS | `cors: {maxAge: 36000}` (`decentraland/lamb2:src/components.ts:65`) | `cors_middleware` on both routers (`services/catalyst/land-authz/src/routes.rs:60`, `services/catalyst/shop/src/market/mod.rs:985`) | See the CORS area |
| Rate limit | None in lamb2 for these routes. The marketplace bulkhead only sheds names (503). | None in the app. Behind this tree's gateway, `/lambdas/*` takes the server-wide `limit_req zone=read` (30 r/s per client IP, burst 60, `nodelay`), answering nginx's 429 page (`infra/gateway/gateway.py:257,260,292`). | DIVERGENT behind the gateway (P2) |
| Auth | None (no signed fetch) | None | MATCH |

---

## 1. GET `/users/:address/names`

**Request**

| Item | Upstream | dcl-one |
|---|---|---|
| `address` | Not validated. Lowercased for the fetch and the cache key (`decentraland/lamb2:src/adapters/elements-fetcher.ts:198`). | Must match `0x`/`0X` + 40 hex digits, else 400 (`services/catalyst/shop/src/market/user_items.rs:344`). Then lowercased. |
| `pageSize` | `parseInt` (lenient: `"10abc"`→10, `"abc"`→NaN). Default 100. Over 1000 → 400 `max allowed pageSize is 1000` (`decentraland/lamb2:src/logic/pagination.ts:12`, `names-handler.ts:11`). | `i64` parse; a parse failure means default 100. Over 1000 → the same 400 (`OversizePolicy::Reject`, `user_items.rs:355`). Below 1 is clamped to 1. |
| `pageNum` | `parseInt`, default 1. No lower bound. | Clamped to at least 1 |
| Other params | None | None |

**Responses**

| Trigger | Status | Body | Headers | Upstream | dcl-one | Verdict |
|---|---|---|---|---|---|---|
| OK, marketplace API answers (primary) | 200 | `{"elements":[{"name":"alice","contractAddress":"0x00000000000000000000000000000000000000aa","tokenId":"123","price":"1000000000000000000"}],"totalAmount":1,"pageNum":1,"pageSize":100}`. `price` is a string, or absent when there is no order. Page and total come from the API (`limit`/`offset`). | JSON | `names-handler.ts:13-31`, `marketplace-api-fetcher.ts:474-512` | `user_items.rs:364-379` (subgraph only) | DIVERGENT in source: dcl-one never calls the marketplace API. For one address holding 16 names, the API, lamb2 and the subgraph's `id asc` gave the same order; the API's `total` (16) equalled the count; the API's `price: null` reached the client as an absent key, as dcl-one's `OwnedName` does (`captures/lambdas-land/LL-7-*.json`). |
| OK, marketplace API failed, subgraph fallback | 200 | Same fields, but `elements` is **every** name: the handler does not slice, because `fetchNames` ignores `pagination` on the graph leg (`fetch-names.ts:58-72`). `totalAmount` = count. `pageNum` and `pageSize` are echoed. | JSON | `fetch-names.ts:58`, `names-handler.ts:13` | `user_items.rs:371-378` slices JS-style | DIVERGENT (upstream quirk; dcl-one slices) |
| `pageSize` > 1000 | 400 | `{"error":"Bad request","message":"max allowed pageSize is 1000"}` | | `pagination.ts:12` | The same 400 (`user_items.rs:355`; fixture `shop/src/market/fixtures/users-names-400-pagesize-over-1000.json`) | MATCH |
| `pageSize=abc` | 200 | `elements: []` (slice NaN), `"pageSize": null` | | `pagination.ts:5` | 200 with default 100 | DIVERGENT (P2) |
| `pageNum=0` or negative | 200 | API receives a negative offset (behaviour depends on the API). On the fallback, the whole list. | | `fetch-items.ts:36` | Clamped to page 1 | DIVERGENT (P2) |
| Invalid address (`0x12`) | 200 | The API call fails or returns empty; the subgraph returns `[]`, so the answer is an empty page | | `api-with-fallback.ts:28-45` | 400 `{"error":"Bad request","message":"Address must be a valid Ethereum address"}` | DIVERGENT |
| Address with `0X` prefix | 200 (lowercased) | | | `elements-fetcher.ts:198` | 200 | MATCH |
| Marketplace bulkhead full | 503 | `{"error":"Service overloaded, retry shortly","message":…}` | | `marketplace-api-fetcher.ts:136`, `errorHandler.ts:17` | n/a | n/a (dcl-one has no API leg) |
| Both sources fail | 502 | `{"error":"The requested items cannot be fetched right now","message":"Cannot fetch elements for <address>"}` (a stale entry is served for up to `maxAge+maxStaleAge` = 120 s) | | `elements-fetcher.ts:265-274` | The same 502, the address as requested (`user_items.rs:368`). No stale serving, 30 s TTL (`services/catalyst/chain-index/src/graph.rs:39`) | MATCH (status and body); stale serving is LL-24 |
| No subgraph configured | n/a (defaults always exist) | | | `the-graph.ts:38` | 503 envelope, `source:"marketplace-subgraph"`, message "the marketplace subgraph reader (LAND, estates, names) is not configured on this node" (`services/catalyst/shop/src/market/readers.rs:162`) | DIVERGENT |
| Subgraph name `null` | Element kept with `name: null` | | | `fetch-names.ts:61` | Element dropped (`services/catalyst/chain-index/src/graph.rs:473`) | DIVERGENT (P2) |
| More than 10,000 names | All returned | | | `fetch-elements.ts:23-44` | Every page walked (`FULL_WALK_PAGES`, a 1,000,000-row runaway guard; `services/catalyst/chain-index/src/graph.rs:34`) | MATCH |

Subgraph query: upstream filters on `owner_: {address: $owner}` (`fetch-names.ts:11`); dcl-one on `owner: $owner` (`services/catalyst/chain-index/src/graph.rs:458`). Account ids are addresses in the marketplace subgraph, so these are equivalent (read, not confirmed by capture).

**Data dependency:** marketplace subgraph (`MARKETPLACE_SUBGRAPH_URL`). Upstream also reads the marketplace-server API (`MARKETPLACE_API_URL`). dcl-one has its own DCLRegistrar index (`shop.nft`), but this route does not use it. The `services/catalyst/config/src/shop.rs:19` doc ("unset answers empty names") disagrees with the code, which answers 503.

---

## 2. GET `/names/:name/owner`

**Request:** `name` path param. Upstream applies `trim()`, `toLowerCase()`, then strips `/\.dcl\.eth$/` (`decentraland/lamb2:src/controllers/handlers/name-owner-handler.ts:11`). dcl-one applies `trim()`, `to_lowercase()` and strips the `.dcl.eth` suffix (`services/catalyst/shop/src/market/lambdas_land.rs:363`).

**Responses**

| Trigger | Status | Body | Upstream | dcl-one | Verdict |
|---|---|---|---|---|---|
| Name held | 200 | `{"owner":"0x0000000000000000000000000000000000000001"}` (lowercase, from the subgraph) | `name-owner-handler.ts:25`, `fetch-name-owner.ts:30` | `lambdas_land.rs:332`. `Address::parse` lowercases (`services/catalyst/shared/src/identity.rs:61`). Index path: `shop.nft` where `lower(ens_subdomain)=lower($1)`. | MATCH |
| `Alice.DCL.ETH` | 200, same as `alice` | | `name-owner-handler.ts:11` | `lambdas_land.rs:354` | MATCH |
| Name with surrounding whitespace (`%20alice`) | 200 (trimmed) | | `name-owner-handler.ts:11` | 200 (trimmed) | MATCH |
| Empty after strip (`.dcl.eth`) | 404, empty body | | `name-owner-handler.ts:12` | 404, empty body (no row matches `''`) | MATCH |
| Unknown name, or owner null | 404, empty body | | `name-owner-handler.ts:19` | 404, empty body (`lambdas_land.rs:333`) | MATCH |
| Subgraph fails | 502 `{"error":"The requested items cannot be fetched right now","message":"Cannot fetch elements for <name>"}` (no stale serving; `ownershipDecisions`) | | `components.ts:217`, `elements-fetcher.ts:273` | The same 502, `<name>` trimmed, lowercased and stripped (`lambdas_land.rs:341`) | MATCH |
| nft index not ready and no subgraph URL | n/a | | | 503 envelope `source:"nft-index"` | DIVERGENT (no upstream equivalent) |
| Index DB error | 500 | | | 500 with `message` | DIVERGENT (P2) |

Upstream query: `nfts(where:{category: ens, searchText:$name}, first:1)` with no `orderBy`. dcl-one: the same filter plus `orderBy: id asc` (`services/catalyst/chain-index/src/graph.rs:630`). A name is a single token, so the order never matters.

**Data dependency:** dcl-one's own nft index over `ETH_RPC_URL` (DCLRegistrar `Transfer`/`NameRegistered`), or the marketplace subgraph as fallback.

---

## 3. GET `/users/:address/lands`

**Request**

| Item | Upstream | dcl-one |
|---|---|---|
| `address` | Not validated. Lowercased (`decentraland/lamb2:src/logic/fetch-elements/fetch-elements.ts:20`). | Not validated. Lowercased (`services/catalyst/shop/src/market/lambdas_land.rs:180`). An invalid address yields `[]` on the reader path (`services/catalyst/chain-index/src/graph.rs:740`). |
| `pageSize` | `parseInt`, default 100, **no maximum** (`maxPageSize = Number.MAX_VALUE`, `lands-handler.ts:10`) | No maximum (`i64::MAX`, `lambdas_land.rs:184`). Below 1 is clamped to 1. Parse failure means 100. |
| `pageNum` | `parseInt`, default 1 | Clamped to at least 1 |

**Element shape** (`decentraland/lamb2:src/logic/fetch-elements/fetch-lands.ts:61-80`, `types.ts:209`):

```json
{"name":"Genesis Plaza","contractAddress":"0x00000000000000000000000000000000000000bb","tokenId":"340282366920938463463374607431768211457","category":"parcel","x":"1","y":"1","description":"…","price":"1000000000000000000","image":"https://api.decentraland.org/v1/parcels/1/1/map.png"}
```

Undefined keys are omitted. `x` and `y` are present only when `category == "parcel"`. An estate has no `x`/`y`, and its `description` comes from `estate.data`. A `data` record whose `description` is null serializes as `"description": null`.

**Responses**

| Trigger | Status | Upstream | dcl-one | Verdict |
|---|---|---|---|---|
| OK | 200 `{elements,totalAmount,pageNum,pageSize}`. In-memory slice of the full list (`decentraland/lamb2:src/logic/pagination.ts:25`). | `lands-handler.ts:12` | Index path (`lambdas_land.rs:194-241`) or reader path (`lambdas_land.rs:135`) | DIVERGENT (fields below) |
| `name` | Subgraph `nft.name`; null is omitted | `fetch-lands.ts:69` | Index: `shop.land_data.name`. Reader: `nft.name`. Omitted when null (`lambdas_land.rs:112`). | MATCH (shape) |
| `x`/`y` | Strings from `parcel.x/y` | `fetch-lands.ts:65` | Strings from `token_right.x/y` or `searchParcelX/Y`, parcels only (`lambdas_land.rs:115`) | MATCH |
| `description` | `data.description`; null when the data record exists with a null description; absent when there is no data | `fetch-lands.ts:67` | `""` when the data record exists with a null description (`lambdas_land.rs:231`, `services/catalyst/chain-index/src/graph.rs:691`) | DIVERGENT (P2) |
| `price` | Subgraph `activeOrder.price` (wei string). The marketplace subgraph keeps `activeOrder` on an expired open order and on an order whose status became `transferred` (placed by a previous owner); lamb2 emits `price` for both (`captures/lambdas-land/LL-13-*.json`). | `fetch-lands.ts:76` | Index: the token's latest legacy order (`shop.nft_order`) while its status is `open`, whatever its expiry and seller (`lambdas_land.rs:88`). Reader: `activeOrder.price`. | MATCH (index path: a sold or cancelled order has no price; the one captured cancelled `activeOrder` is not reproduced) |
| `image` | Subgraph `nft.image` (production: `https://api.decentraland.org/v1/{parcels/x/y,estates/id}/map.png`; dcl-one's test in `lambdas_land.rs` assumes the same) | `fetch-lands.ts:77` | Reader: the subgraph's `image` as is (`services/catalyst/chain-index/src/graph.rs:685`). Index: synthesized as `<LAND_IMAGE_BASE_URL>/v1/parcels/{x}/{y}/map.png` or `/v1/estates/{id}/map.png`; `NODE_POSTURE=mirror` sets the base to `https://api.decentraland.org` (`services/catalyst/config/src/posture.rs:469`), other postures to the node's own `/api` | MATCH under `mirror` (index path: read, not captured) |
| Order | `orderBy: transferredAt desc` per 1000-row page, paged by `id_gt` (so the order is only page-local beyond 1000) | `fetch-lands.ts:9` | Index: latest `Transfer` event (block, log) desc, then contract and token (`lambdas_land.rs:86`). Reader: full sort by `transferredAt` desc (`services/catalyst/chain-index/src/graph.rs:762`). | MATCH for ≤1000 lands. Tie-break differs (P2). |
| `pageSize` > 1000 | 200 with the requested size | `lands-handler.ts:10` | 200 with the requested size | MATCH |
| `pageSize=0` / `pageNum=0` / negative | `[]`, or JS `slice` with a negative start (e.g. `pageSize=-1` gives every element but the last); the value is echoed | `pagination.ts:16-36` | Clamped to 1 | DIVERGENT (P2) |
| `pageSize=abc` | `elements: []`, `"pageSize": null` | | 100 | DIVERGENT (P2) |
| More than 10,000 lands | All returned | `fetch-elements.ts:23` | Every page walked on the reader (`FULL_WALK_PAGES`). The index has no cap. | MATCH |
| Subgraph fails | Stale list served for up to 10 min. With no stale entry: 502 `{"error":"The requested items cannot be fetched right now","message":"Cannot fetch elements for <address>"}`. | `elements-fetcher.ts:284-315` | The same 502, the address as requested (`lambdas_land.rs:172`); no stale list | MATCH without a stale entry; stale serving is LL-24 |
| Indexes not ready, no subgraph | n/a | | 503 envelope `source:"land-authz-index"` or `"nft-index"` (`lambdas_land.rs:186-192`) | DIVERGENT |
| DB error | n/a | | 500 | n/a |

Estate vs parcel: in both trees a parcel inside an estate belongs on chain to the Estate registry, so the user sees the estate and not its parcels. In dcl-one, an estate with no parcels (`x`/`y` NULL) is still listed. The index path binds `LAND_REGISTRY_MAINNET` and `ESTATE_REGISTRY_MAINNET`, and `land_ready` checks `NETWORK_MAINNET` (`lambdas_land.rs:198`, `services/catalyst/shop/src/market/nft_reads.rs:107`), so a sepolia node never answers from its index.

**Data dependency:** land-authz index (RPC logs) plus the nft index (`land_data`, `nft_order`), or the marketplace subgraph.

---

## 4. GET `/users/:address/lands-permissions`

**Request:** `address` is not validated in either tree; both lowercase it (upstream via `decentraland/lamb2:src/adapters/elements-fetcher.ts:198`, dcl-one at `services/catalyst/land-authz/src/routes.rs:185`). Pagination follows the same rules as `/lands`: neither tree has a maximum (`user-lands-permissions-handler.ts:13`, `routes.rs:185`); dcl-one clamps non-positive values to 1.

**Element** (`decentraland/lamb2:src/logic/fetch-elements/fetch-permissions.ts:57`):

```json
{"id":"<subgraph Parcel id>","x":"10","y":"-5","owner":"0x0000000000000000000000000000000000000001","updateOperator":"0x0000000000000000000000000000000000000002"}
```

**Responses**

| Trigger | Status | Upstream | dcl-one | Verdict |
|---|---|---|---|---|
| OK, from the LAND subgraph | 200 `{elements,totalAmount,pageNum,pageSize}`. `parcels(where:{updateOperator})`, `orderBy: id asc`, every page fetched (`fetch-permissions.ts:37`). | `user-lands-permissions-handler.ts:15` | Index not ready + `LAND_SUBGRAPH_URL`: same query (`services/catalyst/chain-index/src/land_graph.rs:90`), every page (`land_graph.rs:24`, the runaway guard) | MATCH |
| OK, from the index | 200 | n/a | `token_right` where `update_operator = lower($addr)`, `LIMIT/OFFSET` (`services/catalyst/land-authz/src/resolve.rs:473`). `id` = the land-manager subgraph's `Parcel.id`, the token id as lower-case hex without leading zeros (`PARCEL_SUBGRAPH_ID`, `resolve.rs:255`), ordered as a string (`COLLATE "C"`); `owner` = the parcel's direct owner, lowercased; `updateOperator` = the queried address. | MATCH (`captures/lambdas-land/LL-17-*.json`; the odd-length ids `0x0` and `0x1` show `Parcel.id` is a string, so string order) |
| Estate-level update operator | Not listed (parcel query only) | | Not listed (LAND registry rows only) | MATCH |
| `pageSize` > 1000 | Honoured | | Honoured | MATCH |
| non-positive / NaN paging | JS `slice` semantics, as in `/lands` | | Clamped | DIVERGENT (P2) |
| Subgraph fails | 502 `{"error":"The requested items cannot be fetched right now","message":"Cannot fetch elements for <address>"}` (no stale; `ownershipDecisions`) | `components.ts:145`, `elements-fetcher.ts:273` | The same 502, the address as requested (`routes.rs:213`) | MATCH |
| Neither index nor `LAND_SUBGRAPH_URL` | n/a | | 503 envelope `source:"land-authz-index"`, `NOT_READY_MSG` (`routes.rs:40`, `routes.rs:202`) | DIVERGENT |
| Index DB error | n/a | | 500 | n/a |

**Data dependency:** land-authz index (`UpdateOperator` and `Transfer` logs over `ETH_RPC_URL`, `LAND_AUTHZ_INDEX_ENABLED=true`), or the land-manager subgraph.

---

## 5. GET `/users/:address/parcels/:x/:y/permissions`

**Request validation**

Both trees validate the coordinates first and the address second.

| Item | Upstream | dcl-one |
|---|---|---|
| `x`, `y` | `parseInt(x)`, then `Parcel.validate({x,y})`: a number within ±150, **or one of three exception blocks**: x 62..150 / y 151..158; x 151..162 / y 144..158; x 151..163 / y 59..143 (`@dcl/schemas` `parcel-exceptions.js`). `parseInt` is lenient: `"1.5"`→1, `"12abc"`→12, `"0x10"`→16. Error: 400 `{"error":"Bad request","message":"Coordinates X and Y must be valid numbers in a valid range"}` (`decentraland/lamb2:src/controllers/handlers/parcel-permissions-handler.ts:15`). | `str::parse::<i64>` (strict), then `is_inside_world_limits`: the ±150 square and the same three blocks (`services/catalyst/land-authz/src/routes.rs:121-127`, `services/catalyst/shared/src/parcel.rs:8-20`). Same message. |
| `address` | `EthAddress.validate`: `^0x[a-fA-F0-9]{40}$`. Error: 400 `"Address must be a valid Ethereum address"` (`parcel-permissions-handler.ts:20`). | Lowercased, then `0x`/`0X` + 40 hex digits (`routes.rs:288-291`), so `0X…` passes. Same message. |

**Responses**

| Trigger | Status | Body | Upstream | dcl-one | Verdict |
|---|---|---|---|---|---|
| Parcel known | 200 | `{"owner":true,"operator":false,"updateOperator":false,"updateManager":false,"approvedForAll":false}`. Each flag is the address compared with the `/operators` legs (`parcel-rights-fetcher.ts:212-222`). | `parcel-rights-fetcher.ts:212` | Index ready: the same five booleans from `rights(x, y)` and `flags_for` (`services/catalyst/land-authz/src/routes.rs:321-325`, `services/catalyst/land-authz/src/resolve.rs:111-116`), plus header `x-land-authz-source: land-authz-index` | MATCH |
| Parcel known, index not ready | 200 | As above (land-manager subgraph) | | The same five booleans from the land-manager subgraph's operators (`land_graph::parcel_operators`, then `operator_flags` and the owner compared case-insensitively; `routes.rs:292-313`, `services/catalyst/land-authz/src/resolve.rs:96-105`), plus header `x-land-authz-source: land-subgraph` | MATCH |
| Exception-block parcel (e.g. 160,100) | 200 / 404 | | `Parcel.validate` accepts | Accepted (`services/catalyst/shared/src/parcel.rs:8-20`) | MATCH |
| `x=1.5` / `x=0x10` | 200 for parcel (1,y) / (16,y) | | parseInt | 400 | DIVERGENT (P2) |
| Not found | 404 | `{"error":"Bad Request","message":"Parcel or estate rights not found for x: 3, y: 4"}` | `types.ts:260`, `errorHandler.ts:47` | `routes.rs:314`, `routes.rs:326` (body from `routes.rs:64-70`), plus the source header | MATCH |
| Subgraph fails | 500 | `{"error":"Internal Server Error"}` (not a `FetcherError`) | `errorHandler.ts:71` | 500 `{"error":"Internal Server Error"}`, no `message`, no source header (`routes.rs:315-318`, `routes.rs:94-100`). An index DB error answers the same (`routes.rs:327-330`). | MATCH |
| Index not ready and no `LAND_SUBGRAPH_URL` | n/a (upstream always has its subgraph default) | | | 500 `{"error":"Internal Server Error"}`, lamb2's answer to an unmapped error (`routes.rs:293-298`) | MATCH (no upstream trigger; the body is lamb2's 500) |

Estate handling is the same in both trees: the owner, operator and approved-for-all legs come from the estate; `updateOperator` is the parcel's own, falling back to the estate's (`decentraland/lamb2:src/adapters/parcel-rights-fetcher.ts:179-187`, `services/catalyst/land-authz/src/resolve.rs:172-177`). Rental tenants appear as `updateOperator` (or `operator`) because the Rentals contract sets them on chain. Neither tree reads rentals data.

**Data dependency:** land-authz index (all five legs) or the land-manager subgraph (all five legs). The marketplace subgraph's owner-only fallback serves only the dcl-one batch route and the write paths (`services/catalyst/land-authz/src/lib.rs:366-439`).

---

## 6. GET `/parcels/:x/:y/operators`

**Request:** `x`/`y` are validated as in §5. Upstream logs at info level and throws the same 400 (`decentraland/lamb2:src/controllers/handlers/parcel-operators-handler.ts:15`). dcl-one: `services/catalyst/land-authz/src/routes.rs:139-142`.

**Responses**

| Trigger | Status | Body | Upstream | dcl-one | Verdict |
|---|---|---|---|---|---|
| Parcel known | 200 | `{"owner":"0x…01","operator":null,"updateOperator":"0x…02","updateManagers":["0x…03"],"approvedForAll":[]}` | `parcel-rights-fetcher.ts:166-210` | Index (`routes.rs:160-168`) or land-manager subgraph (`routes.rs:147-155`, the same query as `services/catalyst/chain-index/src/land_graph.rs:64`) | MATCH (shape) |
| `updateManagers` / `approvedForAll` order | | Insertion order of the authorization log (a JS `Map`, timestamps ascending) | `parcel-rights-fetcher.ts:120-163` | Subgraph path: log order (`land_graph.rs:145`). Index path: the position of the approval that opened the current grant, its first approval after its latest revocation (`GRANTED_SINCE`, `resolve.rs:185`) | MATCH (block and log order stand in for the subgraph's `timestamp` order) |
| Cleared legs | | The land-manager subgraph stores `null` for a leg never set and the zero address for a leg cleared on chain; lamb2 passes the zero address through: `"updateOperator":"0x0000000000000000000000000000000000000000"` (`captures/lambdas-land/LL-18-*.json`) | | Index: an `Approval` or `UpdateOperator` naming the zero address stores it, a `Transfer` stores NULL (`services/catalyst/land-authz/src/indexer.rs:959`); the deploy check reads the zero address as no leg (`deploy_flags_for`, `resolve.rs:122`) | MATCH |
| More than 1000 authorizations | All pages fetched | `parcel-rights-fetcher.ts:123` | Subgraph path: every page (the runaway guard). Index: no cap. | MATCH |
| Exception-block parcel | 200 / 404 | | | Accepted (`routes.rs:121-127`) | MATCH |
| Not found (no estate or parcel row) | 404 | `{"error":"Bad Request","message":"Parcel or estate rights not found for x: X, y: Y"}` | `parcel-rights-fetcher.ts:189` | `routes.rs:156` / `routes.rs:169` | MATCH |
| Subgraph fails | 500 | `{"error":"Internal Server Error"}` | `errorHandler.ts:71` | The same bare 500 (`routes.rs:150`) | MATCH |
| No index and no `LAND_SUBGRAPH_URL` | n/a | | | 503 envelope `source:"land-authz-index"` (`routes.rs:144-146`) | DIVERGENT |
| Index DB error | 500 | | | The same bare 500 (`routes.rs:166`) | MATCH |

**Data dependency:** land-authz index (all `UpdateOperator`, `Approval`, `ApprovalForAll`, `UpdateManager`, `Transfer`, `AddLand` and `RemoveLand` logs), or the land-manager subgraph.

---

## Backlog

Each item gives the gap, the expected behaviour, the place to change it in dcl-one, and a severity.

1. **LL-1: Coordinate validation covers the exception blocks.** Endpoints: `/parcels/:x/:y/operators`, `/users/:a/parcels/:x/:y/permissions`, and the batch route. Status: MATCH, no work.
   - Upstream: `|x|,|y| ≤ 150` and the blocks `x∈[62,150], y∈[151,158]`, `x∈[151,162], y∈[144,158]`, `x∈[151,163], y∈[59,143]` (`@dcl/schemas` `Parcel.validate`, called at `decentraland/lamb2:src/controllers/handlers/parcel-operators-handler.ts:15`).
   - dcl-one: `validate_coords` checks `shared::parcel::is_inside_world_limits`, the same set (`services/catalyst/land-authz/src/routes.rs:121-127`, `services/catalyst/shared/src/parcel.rs:8-20`). A unit test decodes an exception-block token id (`services/catalyst/land-authz/src/lib.rs:489-490`).
   - Severity: none (closed).
2. **LL-2: `/permissions` answers five booleans before the index is ready.** Endpoint: `/users/:a/parcels/:x/:y/permissions`. Status: MATCH, no work.
   - Upstream: five booleans from the land-manager subgraph operators (`decentraland/lamb2:src/adapters/parcel-rights-fetcher.ts:212`); a subgraph failure is a 500 `{"error":"Internal Server Error"}`.
   - dcl-one: before readiness, `land_graph::parcel_operators` and `operator_flags` (`services/catalyst/land-authz/src/routes.rs:292-313`); with no `LAND_SUBGRAPH_URL`, or on a source error, lamb2's bare 500 (`routes.rs:293-298`, `routes.rs:315-318`).
   - Severity: none (closed).
3. **LL-3: Outside `NODE_POSTURE=mirror`, these routes have no source.** All six endpoints.
   - Gap: with `NODE_POSTURE` unset, `island` or `journey`, `MARKETPLACE_SUBGRAPH_URL` and `LAND_SUBGRAPH_URL` are unset and `LAND_AUTHZ_INDEX_ENABLED` is false, so the routes answer the 503 envelope (`/permissions`: a 500). Under `mirror`, the posture sets both URLs to the subgraphs lamb2 reads and turns on the land-authz and nft indexes (`services/catalyst/config/src/posture.rs:272-297`), so every route answers.
   - Expected: upstream always answers, from its production subgraph defaults (`decentraland/lamb2:src/ports/the-graph.ts:19-48`).
   - Change: state `NODE_POSTURE=mirror` as the replacement posture where the node's deployment is described, and correct the `MARKETPLACE_SUBGRAPH_URL` doc comment ("unset answers empty names"; the routes answer 503) in `services/catalyst/config/src/shop.rs:19`.
   - Severity: P2 (configuration; the replacement posture answers).
4. **LL-4: Names `pageSize` over 1000 is clamped.** Endpoint: `/users/:a/names`. Status: MATCH, no work (`OversizePolicy::Reject`; the fixture is now `users-names-400-pagesize-over-1000.json`).
   - Expected: 400 `{"error":"Bad request","message":"max allowed pageSize is 1000"}` (`decentraland/lamb2:src/logic/pagination.ts:12`).
   - Change: use `OversizePolicy::Reject` in `services/catalyst/shop/src/market/user_items.rs:349`, and fix the fixture `shop/src/market/fixtures/users-names-200-pagesize-cap-1000.json`, which encodes the divergent clamp.
   - Severity: P1.
5. **LL-5: `/lands` and `/lands-permissions` clamp `pageSize` to 1000.** Status: MATCH, no work (both pass `i64::MAX`).
   - Expected: no maximum (`decentraland/lamb2:src/controllers/handlers/lands-handler.ts:10`, `user-lands-permissions-handler.ts:13`).
   - Change: pass `i64::MAX` as the maximum in `services/catalyst/shop/src/market/lambdas_land.rs:35` and `services/catalyst/land-authz/src/routes.rs:187-192`.
   - Severity: P1.
6. **LL-6: Paging edge cases.** Endpoints: names, lands, lands-permissions.
   - Gap: `pageNum ≤ 0`, `pageSize ≤ 0` and non-numeric values are clamped, or replaced with the default.
   - Expected: JS `parseInt` plus `slice(offset, offset+limit)` semantics (empty page for 0, negative-index slicing, `"pageSize": null` for NaN) (`decentraland/lamb2:src/logic/pagination.ts:4-41`).
   - Change: `NonPositivePolicy::PassThrough` with JS slicing, as `js_page` (`services/catalyst/shop/src/market/user_items.rs:190`) already does, in the three handlers.
   - Severity: P2.
7. **LL-7: Names come from the subgraph only.** Endpoint: `/users/:a/names`.
   - Gap: no marketplace API leg, and the order is subgraph id order.
   - Expected: page, order and total as the marketplace-server `/v1/users/:a/names` returns them (`decentraland/lamb2:src/adapters/marketplace-api-fetcher.ts:474`).
   - Capture: for one address with 16 names, the API order equals the subgraph's `id asc`, `total` equals the count and a null `price` is dropped by lamb2 (`captures/lambdas-land/LL-7-marketplace-api-names.json`, `LL-7-lambdas-names.json`, `LL-7-subgraph-names.json`). dcl-one's subgraph answer matches that sample. One sample does not establish the API's ordering rule.
   - Change: serve from dcl-one's own `/v1/users/{address}/names` logic (`services/catalyst/shop/src/market/mod.rs:1048`) or from the `shop.nft` index, with the marketplace API's ordering, in `user_items.rs:364`.
   - Severity: P1.
8. **LL-8: Names rejects invalid addresses.** Endpoint: `/users/:a/names`.
   - Expected: 200 with an empty page (`decentraland/lamb2:src/logic/api-with-fallback.ts:28-45`).
   - Change: remove the 400 at `services/catalyst/shop/src/market/user_items.rs:344`.
   - Severity: P2.
9. **LL-9: Status codes when the source fails.** Endpoints: names, name owner, lands, lands-permissions. Status: MATCH, no work (`fetcher_error`, `services/catalyst/shared/src/http_util.rs:41`; an unconfigured or unpublished source keeps the 503 envelope, which has no upstream trigger).
   - Gap: dcl-one answers 503 envelopes.
   - Expected: 502 `{"error":"The requested items cannot be fetched right now","message":"Cannot fetch elements for <key>"}` (`decentraland/lamb2:src/adapters/elements-fetcher.ts:273`).
   - Change: `services/catalyst/shop/src/market/user_items.rs:368`, `lambdas_land.rs:164`, `lambdas_land.rs:370`, `services/catalyst/land-authz/src/routes.rs:84`.
   - Severity: P1.
10. **LL-10: Status codes when the subgraph or the index fails.** Endpoint: `/operators`. Status: MATCH, no work.
    - Gap: a subgraph failure answers the 503 envelope, and an index DB error a 500 with `message`.
    - Expected: 500 `{"error":"Internal Server Error"}` with no `message` (`decentraland/lamb2:src/controllers/handlers/errorHandler.ts:71`).
    - Change: `services/catalyst/land-authz/src/routes.rs:157` and `routes.rs:172`. Answer `upstream_internal_error` (`routes.rs:94-100`), which `/permissions` already uses.
    - Severity: P1.
11. **LL-11: `updateManagers` and `approvedForAll` order.** Endpoint: `/operators` (index path). Status: MATCH, no work (`GRANTED_SINCE`, `services/catalyst/land-authz/src/resolve.rs:185`).
    - Gap: dcl-one sorts alphabetically.
    - Expected: first-approval order, as a JS `Map` keeps it (`decentraland/lamb2:src/adapters/parcel-rights-fetcher.ts:120-163`).
    - Change: order by the approval event position (block, log) in `services/catalyst/land-authz/src/resolve.rs:255-264`.
    - Severity: P1.
12. **LL-12: Default `image` URL.** Endpoint: `/users/:a/lands`. Status: MATCH, no work under `NODE_POSTURE=mirror` (the reader answers the subgraph's `image`; the posture sets `LAND_IMAGE_BASE_URL=https://api.decentraland.org` for the index path). `island` and `journey` keep the node's own `/api`.
    - Gap: the default base is the node's own `/api`.
    - Expected: the subgraph's `image` value (`decentraland/lamb2:src/logic/fetch-elements/fetch-lands.ts:77`; production `https://api.decentraland.org/v1/...`).
    - Change: default `LAND_IMAGE_BASE_URL` in `services/catalyst/config/src/config.rs:530`, or read `image` in `QUERY_OWNED_LANDS` (`services/catalyst/chain-index/src/graph.rs:663`).
    - Severity: P1.
13. **LL-13: `price` filter on the index path.** Endpoint: `/users/:a/lands`.
    - Gap: dcl-one drops orders that are not `open`, expired orders, and orders by a seller other than the owner.
    - Expected: the subgraph's `activeOrder.price`, which does not test expiry (`decentraland/lamb2:src/logic/fetch-elements/fetch-lands.ts:76`).
    - Capture: the marketplace subgraph clears neither. Open orders that expired in 2026 stay the parcel's `activeOrder`, and lamb2 emits their `price` (`captures/lambdas-land/LL-13-subgraph-expired-open-orders.json`, `LL-13-lands-expired-order.json`). Orders that a previous owner placed have status `transferred` and stay the `activeOrder` after the sale; lamb2 emits their `price` too (`LL-13-subgraph-active-order-status.json`, `LL-13-lands-transferred-order.json`).
    - Change: `services/catalyst/shop/src/market/lambdas_land.rs:80`: follow the subgraph's `activeOrder`, which survives expiry and transfer (and, in one sampled row, cancellation). Read the marketplace subgraph's mappings for the exact rule before replacing the filter.
    - Status: MATCH, no work. `USER_LANDS_SQL` keeps the token's latest legacy order while it is `open`, with no expiry or seller test (`lambdas_land.rs:88`). The marketplace subgraph's mappings are not vendored; the one sampled cancelled `activeOrder` is not reproduced (a cancelled order has no price).
    - Severity: P1.
14. **LL-14: `description` when the data record has a null description.** Endpoint: `/users/:a/lands`.
    - Gap: dcl-one answers `""`.
    - Expected: `null` (`decentraland/lamb2:src/logic/fetch-elements/fetch-lands.ts:67`).
    - Change: `services/catalyst/shop/src/market/lambdas_land.rs:231` and `services/catalyst/chain-index/src/graph.rs:691`.
    - Severity: P2.
15. **LL-15: The `/lands` index path is mainnet-only.**
    - Gap: `LAND_REGISTRY_MAINNET`, `ESTATE_REGISTRY_MAINNET` and `NETWORK_MAINNET` are hard-coded, so a sepolia node never answers from its index.
    - Expected: the registries of `ETH_NETWORK` (`decentraland/lamb2:src/ports/the-graph.ts:38`).
    - Change: `services/catalyst/shop/src/market/lambdas_land.rs:198`, `lambdas_land.rs:283`, `services/catalyst/shop/src/market/nft_reads.rs:107`.
    - Severity: P1.
16. **LL-16: Reader page caps.** Endpoints: names and lands (reader path), operators and lands-permissions (subgraph path). Status: MATCH, no work (`FULL_WALK_PAGES`, a 1,000-page runaway guard; one sequential GraphQL request per 1000 rows per uncached key).
    - Gap: 10 or 20 pages at most, which truncates large holders.
    - Expected: walk every page (`decentraland/lamb2:src/logic/fetch-elements/fetch-elements.ts:23-44`, `fetch-permissions.ts:45`).
    - Change: `services/catalyst/chain-index/src/graph.rs:29`, `services/catalyst/chain-index/src/land_graph.rs:23`.
    - Severity: P1.
17. **LL-17: `lands-permissions` `id` and order on the index path.**
    - Gap: `id` is the decimal token id, ordered numerically.
    - Expected: the land-manager subgraph `Parcel.id` in subgraph `id asc` order (`decentraland/lamb2:src/logic/fetch-elements/fetch-permissions.ts:4-21`).
    - Capture: `id` is the token id as lower-case `0x` hex without leading zeros (`0x1000000000000000000000000000000009` is 16,9; `0x16ffffffffffffffffffffffffffffffed` is 22,-19), and elements come in ascending order of that id (`captures/lambdas-land/LL-17-lands-permissions.json`, `LL-17-subgraph-parcels-id-order.json`). Every id in the `lands-permissions` capture has the same length, so it does not tell string order from numeric order; the subgraph sample, where x=16 sorts first, points to string order but does not prove it.
    - Change: emit that hex id and sort by it as a string, in `services/catalyst/land-authz/src/routes.rs:224-233` and `services/catalyst/land-authz/src/resolve.rs:397`.
    - Status: MATCH, no work. The index page answers `PARCEL_SUBGRAPH_ID` (`services/catalyst/land-authz/src/resolve.rs:255`) in `COLLATE "C"` order (`resolve.rs:473`). The capture's `0x0` and `0x1` ids are odd-length hex, so `Parcel.id` is a string (a `Bytes` id would be even-length) and string order follows.
    - Severity: P1.
18. **LL-18: Cleared operator legs.** Endpoint: `/operators`.
    - Gap: dcl-one returns `null` for a cleared leg.
    - Expected: the value the land-manager subgraph stores (`decentraland/lamb2:src/adapters/parcel-rights-fetcher.ts:55-56`).
    - Capture: a leg never set is `null`; a leg cleared on chain is the zero address, and `/parcels/16/65/operators` answers `"updateOperator":"0x0000000000000000000000000000000000000000"` (`captures/lambdas-land/LL-18-*.json`).
    - Change: keep the zero address for a leg that was set and then cleared (`services/catalyst/land-authz/src/indexer.rs:959-960`).
    - Status: MATCH, no work. The fold keeps the zero address of an `Approval` or `UpdateOperator` (`indexer.rs:959`); step `schemas/migrations/0025_land_authz_cleared_legs.sql` backfills rows folded before. `/operators` and `/permissions` read it as lamb2 does (`parcel.updateOperator ?? estate.updateOperator`). The deploy check reads it as no leg (`deploy_flags_for`, `services/catalyst/land-authz/src/resolve.rs:122`), so deploy rights are unchanged. A `Transfer` still clears to NULL; what the subgraph stores after a transfer is not captured.
    - Severity: P1.
19. **LL-19: Lenient coordinate parsing.** Endpoints: `/operators`, `/permissions`.
    - Gap: `"1.5"`, `"12abc"` and `"0x10"` answer 400.
    - Expected: JS `parseInt` truncation and hex prefix (`decentraland/lamb2:src/controllers/handlers/parcel-operators-handler.ts:12`).
    - Change: `services/catalyst/land-authz/src/routes.rs:122-123` (keep the strict parse for the dcl-only batch).
    - Severity: P2.
20. **LL-20: `0X` address prefix.** Endpoint: `/permissions`.
    - Gap: `0X…` is accepted.
    - Expected: 400 `"Address must be a valid Ethereum address"` (`EthAddress.validate`, `decentraland/lamb2:src/controllers/handlers/parcel-permissions-handler.ts:20`).
    - Change: `services/catalyst/land-authz/src/routes.rs:288-291`.
    - Severity: P2.
21. **LL-21: Name owner does not trim the name.** Status: MATCH, no work.
    - Expected: `trim()` before lowercasing (`decentraland/lamb2:src/controllers/handlers/name-owner-handler.ts:11`).
    - Change: `services/catalyst/shop/src/market/lambdas_land.rs:354`.
    - Severity: P2.
22. **LL-22: Names with a null `name` are dropped.** Endpoint: `/users/:a/names` (reader).
    - Expected: keep the element with `name: null` (`decentraland/lamb2:src/logic/fetch-elements/fetch-names.ts:61`).
    - Change: `services/catalyst/chain-index/src/graph.rs:473`.
    - Severity: P2.
23. **LL-23: Route matching is case-sensitive.** All six endpoints.
    - Expected: case-insensitive (`path-to-regexp` defaults in the lamb2 router).
    - Change: cross-cutting, `services/catalyst/src/serve.rs:1304`.
    - Severity: P2.
24. **LL-24: Stale serving on outage.** Endpoints: names, lands.
    - Gap: dcl-one has a 30 s TTL with no stale serving.
    - Expected: names serve stale entries up to 60 s + 60 s; lands serve the last good list for 10 min on fetch failure (`decentraland/lamb2:src/adapters/elements-fetcher.ts:38-43`, `elements-fetcher.ts:284-302`).
    - Change: `services/catalyst/chain-index/src/graph.rs:34`.
    - Severity: P2.

## dcl-one-only endpoints in this area

- `GET /lambdas/users/{address}/genesis-parcels` (`services/catalyst/shop/src/market/lambdas_land.rs:268`).
  - Answers parcels owned directly or through an owned estate: `{elements:[{tokenId,x,y,viaEstate}],totalAmount,pageNum,pageSize}`, ordered by `x, y`.
  - Invalid address → 400. Land index not ready → 503. DB error → 500 with an **empty body** (`lambdas_land.rs:293`).
  - Reads the land-authz index only, with mainnet registries hard-coded.
- `POST /lambdas/users/{address}/parcels/permissions` (`services/catalyst/land-authz/src/routes.rs:370`).
  - Batch permissions for up to 100 `"x,y"` entries, 64 KiB body.
  - Answers `{elements:[{x,y,permissions|null}]}` with the `x-land-authz-source` header. Before the index is ready it keeps the marketplace subgraph's owner-only fallback: the four operator legs are `null` (`routes.rs:252-265`).
- `GET /lambdas/land-authz/status` (`services/catalyst/land-authz/src/routes.rs:414`).
  - Answers `{network, landRegistry, estateRegistry, indexerEnabled, ready, readyWindowSecs, fallback, cursor:{lastBlock, foldedBlock, foldedAt, foldAgeSecs, fullReplayStarted, updatedAt}|null}`. It answers 200 even when not ready; only a cursor-read error gives 500.
  - `fallback` reports `"marketplace-subgraph:owner-only"` whenever `MARKETPLACE_SUBGRAPH_URL` is set. It does not reflect `LAND_SUBGRAPH_URL`, which `/operators`, `/lands-permissions` and `/permissions` use (`routes.rs:439`).
- Response header `x-land-authz-source` on `/permissions`: `land-authz-index` or `land-subgraph`, absent on the 500 (upstream does not send it).
