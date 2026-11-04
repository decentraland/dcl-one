# Lambdas: names, LAND and permissions

Conformance backlog for the lambdas names, LAND and parcel-permission endpoints.

- Upstream lambdas: `decentraland/lamb2` @ `02a37f5d88fb895339c20e84c2470be68cdf3f57` (2026-10-21), served in production under `/lambdas/*`.
- dcl-one: `dcl-one-conformance` @ `0229ef9e`, Rust workspace `services/catalyst`.
- Method: static reading only. Nothing was built or run, so nothing in this file is verified. Every verdict is "read".
- `@dcl/schemas` `Parcel.validate` comes from a local `@dcl/schemas` 23.1.0 copy (`dist/core/parcel-validation.js`, `dist/core/parcel-exceptions.js`). lamb2 pins `^26.5.0`. The exception blocks are assumed to still exist in 26.x; this was not read in 26.x.

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
- `shop/src/market/{lambdas_land,user_items,readers,nft_reads,nft_index,chain_walk,mod}.rs`
- `chain-index/src/{graph,land_graph,source}.rs`
- `shared/src/{http_util,query_params,identity,cors}.rs`
- `config/src/{config,shop,node}.rs`, `src/serve.rs` (mounts)
- `schemas/33_land_authz.sql`
- fixture `shop/src/market/fixtures/users-names-200-pagesize-cap-1000.json`

## Where dcl-one gets the data

| Data | Upstream source | dcl-one source |
|---|---|---|
| Names owned (`/users/:a/names`) | Primary: marketplace-server API `/v1/users/:a/names`. Fallback: marketplace subgraph `nfts(category: ens)`. Defaults point at the production URLs (`decentraland/lamb2:src/ports/the-graph.ts:19`). | Only the marketplace subgraph reader (`MARKETPLACE_SUBGRAPH_URL`, no default; `services/catalyst/chain-index/src/graph.rs:486`). The nft chain index (`shop.nft`) is never consulted for this route. |
| Name owner | Marketplace subgraph `searchText` lookup | Own nft chain index (`shop.nft` rows of the DCLRegistrar, from `eth_getLogs` over `ETH_RPC_URL`) once `INDEXER_NFT`/ethereum is published. Before that, the marketplace subgraph if configured, else 503 (`services/catalyst/shop/src/market/lambdas_land.rs:349`). |
| LAND and estates owned | Marketplace subgraph `nfts(category_in:[parcel,estate])` | Own land-authz index (`land_authz.token_right`) plus the nft index (`shop.land_data` for names and descriptions, `shop.nft_order` for prices) once both are published. Before that, the marketplace subgraph if configured, else 503 (`services/catalyst/shop/src/market/lambdas_land.rs:174`). |
| Parcel operators, update operators, update managers, approved-for-all | LAND (land-manager) subgraph `parcels`/`estates`/`authorizations` | Own land-authz index, folded from LAND and Estate registry logs (`UpdateOperator`, `UpdateManager`, `ApprovalForAll`, `Approval`, `Transfer`, `AddLand`, `RemoveLand`; `services/catalyst/land-authz/src/events.rs:1`), once published. Before that: the land-manager subgraph (`LAND_SUBGRAPH_URL`, no default) for `/operators` and `/lands-permissions`. For `/permissions`, only the owner leg from the marketplace subgraph. |
| Rentals | Not read by any of these upstream routes. A rented LAND's owner is the Rentals contract and the tenant is its `updateOperator`/`operator`, both on chain. | Not read either. The same on-chain facts come through `Transfer` and `UpdateOperator` logs, so both trees treat a tenant as `updateOperator` or `operator`. Neither tree reads the rentals API or subgraph for these routes. |

**Fresh node with catalyst content synced but no chain data** (no published land-authz or nft index; `LAND_AUTHZ_INDEX_ENABLED` defaults to false, `services/catalyst/land-authz/src/lib.rs:171`):

- Content sync feeds none of these routes.
- The node answers the way upstream does only when `MARKETPLACE_SUBGRAPH_URL` and `LAND_SUBGRAPH_URL` are set by hand. Upstream defaults both to the production subgraphs.
- With both set:
  - names, name owner, lands, operators and lands-permissions come from the same subgraphs upstream reads, with the divergences listed below;
  - `/permissions` still answers `null` for the four operator legs, because it never uses the land-manager subgraph.
- With neither set, every route answers the 503 `unavailable` envelope, and `/genesis-parcels` always does until the land-authz index is published.

## Endpoint inventory

| Method | Upstream path | Prod prefix | dcl-one route | Status |
|---|---|---|---|---|
| GET | `/users/:address/names` | `/lambdas` | `/lambdas/users/{address}/names`: `services/catalyst/shop/src/market/mod.rs:893` → `services/catalyst/shop/src/market/user_items.rs:339` | DIVERGENT |
| GET | `/names/:name/owner` | `/lambdas` | `/lambdas/names/{name}/owner`: `services/catalyst/shop/src/market/mod.rs:933` → `services/catalyst/shop/src/market/lambdas_land.rs:328` | DIVERGENT |
| GET | `/users/:address/lands` | `/lambdas` | `/lambdas/users/{address}/lands`: `services/catalyst/shop/src/market/mod.rs:926` → `services/catalyst/shop/src/market/lambdas_land.rs:174` | DIVERGENT |
| GET | `/users/:address/lands-permissions` | `/lambdas` | `services/catalyst/land-authz/src/routes.rs:47` → `services/catalyst/land-authz/src/routes.rs:168` | DIVERGENT |
| GET | `/users/:address/parcels/:x/:y/permissions` | `/lambdas` | `services/catalyst/land-authz/src/routes.rs:51` → `services/catalyst/land-authz/src/routes.rs:267` | DIVERGENT |
| GET | `/parcels/:x/:y/operators` | `/lambdas` | `services/catalyst/land-authz/src/routes.rs:45` → `services/catalyst/land-authz/src/routes.rs:123` | DIVERGENT |

Counts: 6 endpoints. MATCH 0, GAP 0, DIVERGENT 6, MISSING 0.

### Behaviour shared by all six routes

| Aspect | Upstream | dcl-one | Verdict |
|---|---|---|---|
| Error envelope | `{error, message}`. Handled errors map as follows (`decentraland/lamb2:src/controllers/handlers/errorHandler.ts:6`): `InvalidRequestError` → 400 `"Bad request"`, `ServiceOverloadedError` → 503 `"Service overloaded, retry shortly"`, `FetcherError` → 502 `"The requested items cannot be fetched right now"`, `NotFoundError` → 404 `"Not Found"`, `ParcelOrStateNotFoundError` → 404 `"Bad Request"`. Anything else → 500 `{"error":"Internal Server Error"}` with **no** `message` (`errorHandler.ts:71`). | `bad_request` matches the 400 label (`services/catalyst/shared/src/http_util.rs:14`). 500 is `{"error":"Internal Server Error","message":"Internal Server Error"}` (`http_util.rs:30`). Source outages answer 503 `{"error":"Service Unavailable","message":…,"source":…,"configure"?:…}` plus `Retry-After: 60` (`services/catalyst/chain-index/src/source.rs:88`). | DIVERGENT |
| Trailing slash | `path-to-regexp` non-strict: accepted | `NormalizePath::trim_trailing_slash` (`services/catalyst/src/serve.rs:1294`) | MATCH |
| Path case | The well-known-components router uses `path-to-regexp` defaults (case-insensitive), so `/USERS/...` matches | axum is case-sensitive, so 404 | DIVERGENT (P2; covered by the cross-cutting area) |
| 405 | `router.allowedMethods()` (`decentraland/lamb2:src/service.ts:15`) | axum 405 with `Allow` | MATCH (status) |
| CORS | `cors: {maxAge: 36000}` (`decentraland/lamb2:src/components.ts:65`) | `cors_middleware` on both routers (`services/catalyst/land-authz/src/routes.rs:60`, `services/catalyst/shop/src/market/mod.rs:978`) | See the CORS area |
| Rate limit | None in lamb2 for these routes. The marketplace bulkhead only sheds names (503). | None | MATCH |
| Auth | None (no signed fetch) | None | MATCH |

---

## 1. GET `/users/:address/names`

**Request**

| Item | Upstream | dcl-one |
|---|---|---|
| `address` | Not validated. Lowercased for the fetch and the cache key (`decentraland/lamb2:src/adapters/elements-fetcher.ts:198`). | Must match `0x`/`0X` + 40 hex digits, else 400 (`services/catalyst/shop/src/market/user_items.rs:344`). Then lowercased. |
| `pageSize` | `parseInt` (lenient: `"10abc"`→10, `"abc"`→NaN). Default 100. Over 1000 → 400 `max allowed pageSize is 1000` (`decentraland/lamb2:src/logic/pagination.ts:12`, `names-handler.ts:11`). | `i64` parse; a parse failure means default 100. Over 1000 is **clamped** to 1000. Below 1 is clamped to 1 (`user_items.rs:349`). |
| `pageNum` | `parseInt`, default 1. No lower bound. | Clamped to at least 1 |
| Other params | None | None |

**Responses**

| Trigger | Status | Body | Headers | Upstream | dcl-one | Verdict |
|---|---|---|---|---|---|---|
| OK, marketplace API answers (primary) | 200 | `{"elements":[{"name":"alice","contractAddress":"0x00000000000000000000000000000000000000aa","tokenId":"123","price":"1000000000000000000"}],"totalAmount":1,"pageNum":1,"pageSize":100}`. `price` is a string, or absent when there is no order. Page and total come from the API (`limit`/`offset`). | JSON | `names-handler.ts:13-31`, `marketplace-api-fetcher.ts:474-512` | `user_items.rs:364-379` (subgraph only) | DIVERGENT: dcl-one never calls the marketplace API. Element order is subgraph `id asc`, not the API's order. The API's `total` is replaced by the subgraph count. |
| OK, marketplace API failed, subgraph fallback | 200 | Same fields, but `elements` is **every** name: the handler does not slice, because `fetchNames` ignores `pagination` on the graph leg (`fetch-names.ts:58-72`). `totalAmount` = count. `pageNum` and `pageSize` are echoed. | JSON | `fetch-names.ts:58`, `names-handler.ts:13` | `user_items.rs:371-378` slices JS-style | DIVERGENT (upstream quirk; dcl-one slices) |
| `pageSize` > 1000 | 400 | `{"error":"Bad request","message":"max allowed pageSize is 1000"}` | | `pagination.ts:12` | Clamped: 200 with `pageSize:1000` | DIVERGENT |
| `pageSize=abc` | 200 | `elements: []` (slice NaN), `"pageSize": null` | | `pagination.ts:5` | 200 with default 100 | DIVERGENT (P2) |
| `pageNum=0` or negative | 200 | API receives a negative offset (behaviour depends on the API). On the fallback, the whole list. | | `fetch-items.ts:36` | Clamped to page 1 | DIVERGENT (P2) |
| Invalid address (`0x12`) | 200 | The API call fails or returns empty; the subgraph returns `[]`, so the answer is an empty page | | `api-with-fallback.ts:28-45` | 400 `{"error":"Bad request","message":"Address must be a valid Ethereum address"}` | DIVERGENT |
| Address with `0X` prefix | 200 (lowercased) | | | `elements-fetcher.ts:198` | 200 | MATCH |
| Marketplace bulkhead full | 503 | `{"error":"Service overloaded, retry shortly","message":…}` | | `marketplace-api-fetcher.ts:136`, `errorHandler.ts:17` | n/a | n/a (dcl-one has no API leg) |
| Both sources fail | 502 | `{"error":"The requested items cannot be fetched right now","message":"Cannot fetch elements for <address>"}` (a stale entry is served for up to `maxAge+maxStaleAge` = 120 s) | | `elements-fetcher.ts:265-274` | 503 `{"error":"Service Unavailable","message":"ownership readers unavailable"}` (`user_items.rs:368`), no stale serving, 30 s TTL (`services/catalyst/chain-index/src/graph.rs:34`) | DIVERGENT |
| No subgraph configured | n/a (defaults always exist) | | | `the-graph.ts:38` | 503 envelope, `source:"marketplace-subgraph"`, message "the marketplace subgraph reader (LAND, estates, names) is not configured on this node" (`services/catalyst/shop/src/market/readers.rs:162`) | DIVERGENT |
| Subgraph name `null` | Element kept with `name: null` | | | `fetch-names.ts:61` | Element dropped (`services/catalyst/chain-index/src/graph.rs:468`) | DIVERGENT (P2) |
| More than 10,000 names | All returned | | | `fetch-elements.ts:23-44` | Cut at `MAX_PAGES=10` × 1000 (`services/catalyst/chain-index/src/graph.rs:29`) | DIVERGENT |

Subgraph query: upstream filters on `owner_: {address: $owner}` (`fetch-names.ts:11`); dcl-one on `owner: $owner` (`services/catalyst/chain-index/src/graph.rs:453`). Account ids are addresses in the marketplace subgraph, so these are equivalent (read, not confirmed by capture).

**Data dependency:** marketplace subgraph (`MARKETPLACE_SUBGRAPH_URL`). Upstream also reads the marketplace-server API (`MARKETPLACE_API_URL`). dcl-one has its own DCLRegistrar index (`shop.nft`), but this route does not use it. The `config/src/shop.rs:20` doc ("unset answers empty names") disagrees with the code, which answers 503.

---

## 2. GET `/names/:name/owner`

**Request:** `name` path param. Upstream applies `trim()`, `toLowerCase()`, then strips `/\.dcl\.eth$/` (`decentraland/lamb2:src/controllers/handlers/name-owner-handler.ts:11`). dcl-one applies `to_lowercase()` and strips the `.dcl.eth` suffix, with **no trim** (`services/catalyst/shop/src/market/lambdas_land.rs:354`).

**Responses**

| Trigger | Status | Body | Upstream | dcl-one | Verdict |
|---|---|---|---|---|---|
| Name held | 200 | `{"owner":"0x0000000000000000000000000000000000000001"}` (lowercase, from the subgraph) | `name-owner-handler.ts:25`, `fetch-name-owner.ts:30` | `lambdas_land.rs:332`. `Address::parse` lowercases (`services/catalyst/shared/src/identity.rs:61`). Index path: `shop.nft` where `lower(ens_subdomain)=lower($1)`. | MATCH |
| `Alice.DCL.ETH` | 200, same as `alice` | | `name-owner-handler.ts:11` | `lambdas_land.rs:354` | MATCH |
| Name with surrounding whitespace (`%20alice`) | 200 (trimmed) | | `name-owner-handler.ts:11` | 404 | DIVERGENT (P2) |
| Empty after strip (`.dcl.eth`) | 404, empty body | | `name-owner-handler.ts:12` | 404, empty body (no row matches `''`) | MATCH |
| Unknown name, or owner null | 404, empty body | | `name-owner-handler.ts:19` | 404, empty body (`lambdas_land.rs:333`) | MATCH |
| Subgraph fails | 502 `{"error":"The requested items cannot be fetched right now","message":"Cannot fetch elements for <name>"}` (no stale serving; `ownershipDecisions`) | | `components.ts:217`, `elements-fetcher.ts:273` | 503 envelope `source:"marketplace-subgraph"`, "the marketplace names reader did not answer" | DIVERGENT |
| nft index not ready and no subgraph URL | n/a | | | 503 envelope `source:"nft-index"` | DIVERGENT (no upstream equivalent) |
| Index DB error | 500 | | | 500 with `message` | DIVERGENT (P2) |

Upstream query: `nfts(where:{category: ens, searchText:$name}, first:1)` with no `orderBy`. dcl-one: the same filter plus `orderBy: id asc` (`services/catalyst/chain-index/src/graph.rs:625`). A name is a single token, so the order never matters.

**Data dependency:** dcl-one's own nft index over `ETH_RPC_URL` (DCLRegistrar `Transfer`/`NameRegistered`), or the marketplace subgraph as fallback.

---

## 3. GET `/users/:address/lands`

**Request**

| Item | Upstream | dcl-one |
|---|---|---|
| `address` | Not validated. Lowercased (`decentraland/lamb2:src/logic/fetch-elements/fetch-elements.ts:20`). | Not validated. Lowercased (`services/catalyst/shop/src/market/lambdas_land.rs:180`). An invalid address yields `[]` on the reader path (`services/catalyst/chain-index/src/graph.rs:735`). |
| `pageSize` | `parseInt`, default 100, **no maximum** (`maxPageSize = Number.MAX_VALUE`, `lands-handler.ts:10`) | Clamped to 1000. Below 1 is clamped to 1. Parse failure means 100 (`lambdas_land.rs:35`). |
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
| `description` | `data.description`; null when the data record exists with a null description; absent when there is no data | `fetch-lands.ts:67` | `""` when the data record exists with a null description (`lambdas_land.rs:231`, `services/catalyst/chain-index/src/graph.rs:686`) | DIVERGENT (P2) |
| `price` | Subgraph `activeOrder.price` (wei string). The subgraph's active order does not test expiry. | `fetch-lands.ts:76` | Index: `shop.nft_order` with `status='open' AND expires_at > now AND seller = owner` (`lambdas_land.rs:80`). Reader: `activeOrder.price`. | DIVERGENT on the index path: expired or foreign-seller orders are dropped |
| `image` | Subgraph `nft.image` (production: `https://api.decentraland.org/v1/{parcels/x/y,estates/id}/map.png`; dcl-one's test at `lambdas_land.rs:449` assumes the same) | `fetch-lands.ts:77` | Synthesized as `<LAND_IMAGE_BASE_URL>/v1/parcels/{x}/{y}/map.png` or `/v1/estates/{id}/map.png`. The default base is the node's own `/api` (`services/catalyst/config/src/config.rs:530`). The reader never asks for `image`. | DIVERGENT by default |
| Order | `orderBy: transferredAt desc` per 1000-row page, paged by `id_gt` (so the order is only page-local beyond 1000) | `fetch-lands.ts:9` | Index: latest `Transfer` event (block, log) desc, then contract and token (`lambdas_land.rs:86`). Reader: full sort by `transferredAt` desc (`services/catalyst/chain-index/src/graph.rs:757`). | MATCH for ≤1000 lands. Tie-break differs (P2). |
| `pageSize` > 1000 | 200 with the requested size | `lands-handler.ts:10` | Clamped to 1000; `pageSize:1000` echoed | DIVERGENT |
| `pageSize=0` / `pageNum=0` / negative | `[]`, or JS `slice` with a negative start (e.g. `pageSize=-1` gives every element but the last); the value is echoed | `pagination.ts:16-36` | Clamped to 1 | DIVERGENT (P2) |
| `pageSize=abc` | `elements: []`, `"pageSize": null` | | 100 | DIVERGENT (P2) |
| More than 10,000 lands | All returned | `fetch-elements.ts:23` | The reader stops at 10 pages (`services/catalyst/chain-index/src/graph.rs:29`). The index has no cap. | DIVERGENT (reader path) |
| Subgraph fails | Stale list served for up to 10 min. With no stale entry: 502 `{"error":"The requested items cannot be fetched right now","message":"Cannot fetch elements for <address>"}`. | `elements-fetcher.ts:284-315` | 503 envelope `source:"marketplace-subgraph"` | DIVERGENT |
| Indexes not ready, no subgraph | n/a | | 503 envelope `source:"land-authz-index"` or `"nft-index"` (`lambdas_land.rs:186-192`) | DIVERGENT |
| DB error | n/a | | 500 | n/a |

Estate vs parcel: in both trees a parcel inside an estate belongs on chain to the Estate registry, so the user sees the estate and not its parcels. In dcl-one, an estate with no parcels (`x`/`y` NULL) is still listed. The index path binds `LAND_REGISTRY_MAINNET` and `ESTATE_REGISTRY_MAINNET`, and `land_ready` checks `NETWORK_MAINNET` (`lambdas_land.rs:198`, `services/catalyst/shop/src/market/nft_reads.rs:107`), so a sepolia node never answers from its index.

**Data dependency:** land-authz index (RPC logs) plus the nft index (`land_data`, `nft_order`), or the marketplace subgraph.

---

## 4. GET `/users/:address/lands-permissions`

**Request:** `address` is not validated in either tree; both lowercase it (upstream via `decentraland/lamb2:src/adapters/elements-fetcher.ts:198`, dcl-one at `services/catalyst/land-authz/src/routes.rs:173`). Pagination follows the same rules as `/lands`: upstream has no maximum (`user-lands-permissions-handler.ts:13`); dcl-one clamps to 1000 and clamps non-positive values to 1 (`routes.rs:175`).

**Element** (`decentraland/lamb2:src/logic/fetch-elements/fetch-permissions.ts:57`):

```json
{"id":"<subgraph Parcel id>","x":"10","y":"-5","owner":"0x0000000000000000000000000000000000000001","updateOperator":"0x0000000000000000000000000000000000000002"}
```

**Responses**

| Trigger | Status | Upstream | dcl-one | Verdict |
|---|---|---|---|---|
| OK, from the LAND subgraph | 200 `{elements,totalAmount,pageNum,pageSize}`. `parcels(where:{updateOperator})`, `orderBy: id asc`, every page fetched (`fetch-permissions.ts:37`). | `user-lands-permissions-handler.ts:15` | Index not ready + `LAND_SUBGRAPH_URL`: same query (`services/catalyst/chain-index/src/land_graph.rs:90`), capped at 20 pages (`land_graph.rs:23`) | MATCH up to 20,000 rows |
| OK, from the index | 200 | n/a | `token_right` where `update_operator = lower($addr)`, `ORDER BY token_id` (numeric), `LIMIT/OFFSET` (`services/catalyst/land-authz/src/resolve.rs:387`). `id` = decimal LAND token id; `owner` = the parcel's direct owner, lowercased; `updateOperator` = the queried address. | DIVERGENT unless the land-manager subgraph `Parcel.id` is the decimal token id and sorts the same way (the id format is not in the sources read; needs a capture). A string `id` order differs from a numeric order. |
| Estate-level update operator | Not listed (parcel query only) | | Not listed (LAND registry rows only) | MATCH |
| `pageSize` > 1000 | Honoured | | Clamped to 1000 | DIVERGENT |
| non-positive / NaN paging | JS `slice` semantics, as in `/lands` | | Clamped | DIVERGENT (P2) |
| Subgraph fails | 502 `{"error":"The requested items cannot be fetched right now","message":"Cannot fetch elements for <address>"}` (no stale; `ownershipDecisions`) | `components.ts:145`, `elements-fetcher.ts:273` | 503 envelope `source:"land-subgraph"`, "the LAND subgraph reader did not answer" (`routes.rs:84`) | DIVERGENT |
| Neither index nor `LAND_SUBGRAPH_URL` | n/a | | 503 envelope `source:"land-authz-index"`, `NOT_READY_MSG` (`routes.rs:40`) | DIVERGENT |
| Index DB error | n/a | | 500 | n/a |

**Data dependency:** land-authz index (`UpdateOperator` and `Transfer` logs over `ETH_RPC_URL`, `LAND_AUTHZ_INDEX_ENABLED=true`), or the land-manager subgraph.

---

## 5. GET `/users/:address/parcels/:x/:y/permissions`

**Request validation**

Both trees validate the coordinates first and the address second.

| Item | Upstream | dcl-one |
|---|---|---|
| `x`, `y` | `parseInt(x)`, then `Parcel.validate({x,y})`: a number within ±150, **or one of three exception blocks**: x 62..150 / y 151..158; x 151..162 / y 144..158; x 151..163 / y 59..143 (`@dcl/schemas` `parcel-exceptions.js`). `parseInt` is lenient: `"1.5"`→1, `"12abc"`→12, `"0x10"`→16. Error: 400 `{"error":"Bad request","message":"Coordinates X and Y must be valid numbers in a valid range"}` (`decentraland/lamb2:src/controllers/handlers/parcel-permissions-handler.ts:15`). | `str::parse::<i64>` (strict) and `-150..=150` only. Same message (`services/catalyst/land-authz/src/routes.rs:109`). |
| `address` | `EthAddress.validate`: `^0x[a-fA-F0-9]{40}$`. Error: 400 `"Address must be a valid Ethereum address"` (`parcel-permissions-handler.ts:20`). | Lowercased, then `0x`/`0X` + 40 hex digits (`routes.rs:275`), so `0X…` passes. Same message. |

**Responses**

| Trigger | Status | Body | Upstream | dcl-one | Verdict |
|---|---|---|---|---|---|
| Parcel known | 200 | `{"owner":true,"operator":false,"updateOperator":false,"updateManager":false,"approvedForAll":false}`. Each flag is the address compared with the `/operators` legs (`parcel-rights-fetcher.ts:212-222`). | `parcel-rights-fetcher.ts:212` | Index ready: the same five booleans from `rights_batch` (`services/catalyst/land-authz/src/resolve.rs:229`), plus header `x-land-authz-source: land-authz-index` | MATCH (index) |
| Parcel known, index not ready | 200 | As above (land-manager subgraph) | | Marketplace subgraph owner leg only: `{"owner":bool,"operator":null,"updateOperator":null,"updateManager":null,"approvedForAll":null}` with `x-land-authz-source: marketplace-subgraph:owner-only` (`routes.rs:240`, `services/catalyst/land-authz/src/lib.rs:395`). It does not use `LAND_SUBGRAPH_URL` even when that is set. | DIVERGENT |
| Exception-block parcel (e.g. 160,100) | 200 / 404 | | `Parcel.validate` accepts | 400 | DIVERGENT |
| `x=1.5` / `x=0x10` | 200 for parcel (1,y) / (16,y) | | parseInt | 400 | DIVERGENT (P2) |
| Not found | 404 | `{"error":"Bad Request","message":"Parcel or estate rights not found for x: 3, y: 4"}` | `types.ts:260`, `errorHandler.ts:47` | `routes.rs:64`, plus the source header | MATCH |
| Subgraph fails | 500 | `{"error":"Internal Server Error"}` (not a `FetcherError`) | `errorHandler.ts:71` | Reader fails: 503 envelope `source:"marketplace-subgraph"`. Index DB error: 500 with `message`. | DIVERGENT |
| No source | n/a | | | 503 envelope `source:"land-authz-index"`, `NO_SOURCE_MSG` | DIVERGENT |

Estate handling is the same in both trees: the owner, operator and approved-for-all legs come from the estate; `updateOperator` is the parcel's own, falling back to the estate's (`decentraland/lamb2:src/adapters/parcel-rights-fetcher.ts:179-187`, `services/catalyst/land-authz/src/resolve.rs:172-177`). Rental tenants appear as `updateOperator` (or `operator`) because the Rentals contract sets them on chain. Neither tree reads rentals data.

**Data dependency:** land-authz index (all five legs), the marketplace subgraph (owner leg only), or neither.

---

## 6. GET `/parcels/:x/:y/operators`

**Request:** `x`/`y` are validated as in §5. Upstream logs at info level and throws the same 400 (`decentraland/lamb2:src/controllers/handlers/parcel-operators-handler.ts:15`). dcl-one: `services/catalyst/land-authz/src/routes.rs:127`.

**Responses**

| Trigger | Status | Body | Upstream | dcl-one | Verdict |
|---|---|---|---|---|---|
| Parcel known | 200 | `{"owner":"0x…01","operator":null,"updateOperator":"0x…02","updateManagers":["0x…03"],"approvedForAll":[]}` | `parcel-rights-fetcher.ts:166-210` | Index (`routes.rs:148`) or land-manager subgraph (`routes.rs:135`, the same query as `services/catalyst/chain-index/src/land_graph.rs:64`) | MATCH (shape) |
| `updateManagers` / `approvedForAll` order | | Insertion order of the authorization log (a JS `Map`, timestamps ascending) | `parcel-rights-fetcher.ts:120-163` | Subgraph path: log order (`land_graph.rs:145`). Index path: `ORDER BY operator` (alphabetical; `resolve.rs:255-264`). | DIVERGENT on the index path |
| Cleared legs | | Whatever the land-manager subgraph stores for a cleared leg (null or the zero address; not in the sources read) | | Index: `NULLIF(…, zero)`, so `null` (`services/catalyst/land-authz/src/indexer.rs:958`) | Unconfirmed: needs a capture |
| More than 1000 authorizations | All pages fetched | `parcel-rights-fetcher.ts:123` | Subgraph path: 20 pages maximum. Index: no cap. | MATCH up to 20,000 |
| Exception-block parcel | 200 / 404 | | | 400 | DIVERGENT |
| Not found (no estate or parcel row) | 404 | `{"error":"Bad Request","message":"Parcel or estate rights not found for x: X, y: Y"}` | `parcel-rights-fetcher.ts:189` | `routes.rs:144` / `routes.rs:157` | MATCH |
| Subgraph fails | 500 | `{"error":"Internal Server Error"}` | `errorHandler.ts:71` | 503 envelope `source:"land-subgraph"` | DIVERGENT |
| No index and no `LAND_SUBGRAPH_URL` | n/a | | | 503 envelope `source:"land-authz-index"` | DIVERGENT |
| Index DB error | 500 | | | 500 with `message` | DIVERGENT (P2) |

**Data dependency:** land-authz index (all `UpdateOperator`, `Approval`, `ApprovalForAll`, `UpdateManager`, `Transfer`, `AddLand` and `RemoveLand` logs), or the land-manager subgraph.

---

## Backlog

Each item gives the gap, the expected behaviour, the place to change it in dcl-one, and a severity.

1. **LL-1: Coordinate validation rejects exception-block parcels.** Endpoints: `/parcels/:x/:y/operators`, `/users/:a/parcels/:x/:y/permissions`, and the batch route.
   - Gap: dcl-one accepts only ±150, so real parcels in the three exception blocks answer 400.
   - Expected: accept `|x|,|y| ≤ 150` and the blocks `x∈[62,150], y∈[151,158]`, `x∈[151,162], y∈[144,158]`, `x∈[151,163], y∈[59,143]` (`@dcl/schemas` `Parcel.validate`, called at `decentraland/lamb2:src/controllers/handlers/parcel-operators-handler.ts:15`).
   - Change: `services/catalyst/land-authz/src/routes.rs:109`. Also check that the index's token decode covers these coordinates.
   - Severity: **P0**.
2. **LL-2: `/permissions` has no operator legs before the index is ready.** Endpoint: `/users/:a/parcels/:x/:y/permissions`.
   - Gap: the four operator legs are `null` and only the owner is decided, even with `LAND_SUBGRAPH_URL` set.
   - Expected: five booleans computed from the land-manager subgraph operators (`decentraland/lamb2:src/adapters/parcel-rights-fetcher.ts:212`).
   - Change: in `services/catalyst/land-authz/src/routes.rs:267` and `services/catalyst/land-authz/src/lib.rs:381`, prefer `land_graph::parcel_operators` (as `/operators` does at `routes.rs:131`) before the marketplace owner-only fallback.
   - Severity: **P0**.
3. **LL-3: A fresh node does not answer these routes by default.** All six endpoints.
   - Gap: `MARKETPLACE_SUBGRAPH_URL` and `LAND_SUBGRAPH_URL` have no default, and `LAND_AUTHZ_INDEX_ENABLED` defaults to false, so a fresh node answers 503 everywhere.
   - Expected: always answer (upstream defaults to the production subgraphs, `decentraland/lamb2:src/ports/the-graph.ts:19-48`).
   - Change: default these URLs per `ETH_NETWORK` in `services/catalyst/config/src/shop.rs:20-26`, or document them as required in the replacement posture (`config/src/posture.rs`).
   - Severity: **P0**.
4. **LL-4: Names `pageSize` over 1000 is clamped.** Endpoint: `/users/:a/names`.
   - Expected: 400 `{"error":"Bad request","message":"max allowed pageSize is 1000"}` (`decentraland/lamb2:src/logic/pagination.ts:12`).
   - Change: use `OversizePolicy::Reject` in `services/catalyst/shop/src/market/user_items.rs:349`, and fix the fixture `shop/src/market/fixtures/users-names-200-pagesize-cap-1000.json`, which encodes the divergent clamp.
   - Severity: P1.
5. **LL-5: `/lands` and `/lands-permissions` clamp `pageSize` to 1000.**
   - Expected: no maximum (`decentraland/lamb2:src/controllers/handlers/lands-handler.ts:10`, `user-lands-permissions-handler.ts:13`).
   - Change: pass `i64::MAX` as the maximum in `services/catalyst/shop/src/market/lambdas_land.rs:35` and `services/catalyst/land-authz/src/routes.rs:175`.
   - Severity: P1.
6. **LL-6: Paging edge cases.** Endpoints: names, lands, lands-permissions.
   - Gap: `pageNum ≤ 0`, `pageSize ≤ 0` and non-numeric values are clamped, or replaced with the default.
   - Expected: JS `parseInt` plus `slice(offset, offset+limit)` semantics (empty page for 0, negative-index slicing, `"pageSize": null` for NaN) (`decentraland/lamb2:src/logic/pagination.ts:4-41`).
   - Change: `NonPositivePolicy::PassThrough` with JS slicing, as `js_page` (`services/catalyst/shop/src/market/user_items.rs:190`) already does, in the three handlers.
   - Severity: P2.
7. **LL-7: Names come from the subgraph only.** Endpoint: `/users/:a/names`.
   - Gap: no marketplace API leg, and the order is subgraph id order.
   - Expected: page, order and total as the marketplace-server `/v1/users/:a/names` returns them (`decentraland/lamb2:src/adapters/marketplace-api-fetcher.ts:474`).
   - Change: serve from dcl-one's own `/v1/users/{address}/names` logic (`services/catalyst/shop/src/market/mod.rs:1041`) or from the `shop.nft` index, with the marketplace API's ordering, in `user_items.rs:364`.
   - Severity: P1.
8. **LL-8: Names rejects invalid addresses.** Endpoint: `/users/:a/names`.
   - Expected: 200 with an empty page (`decentraland/lamb2:src/logic/api-with-fallback.ts:28-45`).
   - Change: remove the 400 at `services/catalyst/shop/src/market/user_items.rs:344`.
   - Severity: P2.
9. **LL-9: Status codes when the source fails.** Endpoints: names, name owner, lands, lands-permissions.
   - Gap: dcl-one answers 503 envelopes.
   - Expected: 502 `{"error":"The requested items cannot be fetched right now","message":"Cannot fetch elements for <key>"}` (`decentraland/lamb2:src/adapters/elements-fetcher.ts:273`).
   - Change: `services/catalyst/shop/src/market/user_items.rs:368`, `lambdas_land.rs:164`, `lambdas_land.rs:370`, `services/catalyst/land-authz/src/routes.rs:84`.
   - Severity: P1.
10. **LL-10: Status codes when the subgraph fails.** Endpoints: `/operators`, `/permissions`.
    - Expected: 500 `{"error":"Internal Server Error"}` with no `message` (`decentraland/lamb2:src/controllers/handlers/errorHandler.ts:71`).
    - Change: `services/catalyst/land-authz/src/routes.rs:84`, `routes.rs:99`, and `services/catalyst/shared/src/http_util.rs:30` (drop `message` for lambdas 500s).
    - Severity: P1.
11. **LL-11: `updateManagers` and `approvedForAll` order.** Endpoint: `/operators` (index path).
    - Gap: dcl-one sorts alphabetically.
    - Expected: first-approval order, as a JS `Map` keeps it (`decentraland/lamb2:src/adapters/parcel-rights-fetcher.ts:120-163`).
    - Change: order by the approval event position (block, log) in `services/catalyst/land-authz/src/resolve.rs:255-264`.
    - Severity: P1.
12. **LL-12: Default `image` URL.** Endpoint: `/users/:a/lands`.
    - Gap: the default base is the node's own `/api`.
    - Expected: the subgraph's `image` value (`decentraland/lamb2:src/logic/fetch-elements/fetch-lands.ts:77`; production `https://api.decentraland.org/v1/...`).
    - Change: default `LAND_IMAGE_BASE_URL` in `services/catalyst/config/src/config.rs:530`, or read `image` in `QUERY_OWNED_LANDS` (`services/catalyst/chain-index/src/graph.rs:658`).
    - Severity: P1.
13. **LL-13: `price` filter on the index path.** Endpoint: `/users/:a/lands`.
    - Gap: dcl-one drops expired orders and orders by a seller other than the owner.
    - Expected: the subgraph's `activeOrder.price`, which does not test expiry (`decentraland/lamb2:src/logic/fetch-elements/fetch-lands.ts:76`).
    - Change: `services/catalyst/shop/src/market/lambdas_land.rs:80`, after a capture confirms whether the subgraph clears `activeOrder` on expiry or transfer.
    - Severity: P1.
14. **LL-14: `description` when the data record has a null description.** Endpoint: `/users/:a/lands`.
    - Gap: dcl-one answers `""`.
    - Expected: `null` (`decentraland/lamb2:src/logic/fetch-elements/fetch-lands.ts:67`).
    - Change: `services/catalyst/shop/src/market/lambdas_land.rs:231` and `services/catalyst/chain-index/src/graph.rs:686`.
    - Severity: P2.
15. **LL-15: The `/lands` index path is mainnet-only.**
    - Gap: `LAND_REGISTRY_MAINNET`, `ESTATE_REGISTRY_MAINNET` and `NETWORK_MAINNET` are hard-coded, so a sepolia node never answers from its index.
    - Expected: the registries of `ETH_NETWORK` (`decentraland/lamb2:src/ports/the-graph.ts:38`).
    - Change: `services/catalyst/shop/src/market/lambdas_land.rs:198`, `lambdas_land.rs:283`, `services/catalyst/shop/src/market/nft_reads.rs:107`.
    - Severity: P1.
16. **LL-16: Reader page caps.** Endpoints: names and lands (reader path), operators and lands-permissions (subgraph path).
    - Gap: 10 or 20 pages at most, which truncates large holders.
    - Expected: walk every page (`decentraland/lamb2:src/logic/fetch-elements/fetch-elements.ts:23-44`, `fetch-permissions.ts:45`).
    - Change: `services/catalyst/chain-index/src/graph.rs:29`, `services/catalyst/chain-index/src/land_graph.rs:23`.
    - Severity: P1.
17. **LL-17: `lands-permissions` `id` and order on the index path.**
    - Gap: `id` is the decimal token id, ordered numerically.
    - Expected: the land-manager subgraph `Parcel.id` in subgraph `id asc` order (`decentraland/lamb2:src/logic/fetch-elements/fetch-permissions.ts:4-21`).
    - Change: capture one real response first, then `services/catalyst/land-authz/src/routes.rs:216` and `services/catalyst/land-authz/src/resolve.rs:397`.
    - Severity: P1.
18. **LL-18: Cleared operator legs.** Endpoint: `/operators`.
    - Gap: unconfirmed whether the subgraph returns `null` or the zero address for a cleared leg; dcl-one returns `null`.
    - Expected: the value the land-manager subgraph stores (`decentraland/lamb2:src/adapters/parcel-rights-fetcher.ts:55-56`).
    - Change: capture, then `services/catalyst/land-authz/src/indexer.rs:958` if needed.
    - Severity: P1.
19. **LL-19: Lenient coordinate parsing.** Endpoints: `/operators`, `/permissions`.
    - Gap: `"1.5"`, `"12abc"` and `"0x10"` answer 400.
    - Expected: JS `parseInt` truncation and hex prefix (`decentraland/lamb2:src/controllers/handlers/parcel-operators-handler.ts:12`).
    - Change: `services/catalyst/land-authz/src/routes.rs:109` (keep the strict parse for the dcl-only batch).
    - Severity: P2.
20. **LL-20: `0X` address prefix.** Endpoint: `/permissions`.
    - Gap: `0X…` is accepted.
    - Expected: 400 `"Address must be a valid Ethereum address"` (`EthAddress.validate`, `decentraland/lamb2:src/controllers/handlers/parcel-permissions-handler.ts:20`).
    - Change: `services/catalyst/land-authz/src/routes.rs:275`.
    - Severity: P2.
21. **LL-21: Name owner does not trim the name.**
    - Expected: `trim()` before lowercasing (`decentraland/lamb2:src/controllers/handlers/name-owner-handler.ts:11`).
    - Change: `services/catalyst/shop/src/market/lambdas_land.rs:354`.
    - Severity: P2.
22. **LL-22: Names with a null `name` are dropped.** Endpoint: `/users/:a/names` (reader).
    - Expected: keep the element with `name: null` (`decentraland/lamb2:src/logic/fetch-elements/fetch-names.ts:61`).
    - Change: `services/catalyst/chain-index/src/graph.rs:468`.
    - Severity: P2.
23. **LL-23: Route matching is case-sensitive.** All six endpoints.
    - Expected: case-insensitive (`path-to-regexp` defaults in the lamb2 router).
    - Change: cross-cutting, `services/catalyst/src/serve.rs:1294`.
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
- `POST /lambdas/users/{address}/parcels/permissions` (`services/catalyst/land-authz/src/routes.rs:331`).
  - Batch permissions for up to 100 `"x,y"` entries, 64 KiB body.
  - Answers `{elements:[{x,y,permissions|null}]}` with the `x-land-authz-source` header.
- `GET /lambdas/land-authz/status` (`services/catalyst/land-authz/src/routes.rs:375`).
  - Answers `{network, landRegistry, estateRegistry, indexerEnabled, ready, readyWindowSecs, fallback, cursor:{lastBlock, foldedBlock, foldedAt, foldAgeSecs, fullReplayStarted, updatedAt}|null}`. It answers 200 even when not ready; only a cursor-read error gives 500.
  - `fallback` reports `"marketplace-subgraph:owner-only"` whenever `MARKETPLACE_SUBGRAPH_URL` is set. It does not reflect `LAND_SUBGRAPH_URL`, which `/operators` and `/lands-permissions` use.
- Response header `x-land-authz-source` on `/permissions` (added; upstream does not send it).
