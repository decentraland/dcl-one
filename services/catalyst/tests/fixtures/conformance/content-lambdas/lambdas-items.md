# Lambdas: wearables, emotes, third-party wearables and collections

Area: the lambdas item endpoints (owned wearables and emotes, third-party / linked wearables,
the explorer backpack, the collections list, and the legacy-lambdas ports under `/collections/*`).

Pinned revisions:

- Upstream lambdas: `decentraland/lamb2` @ `02a37f5d88fb895339c20e84c2470be68cdf3f57` (2026-10-21), served in production at `/lambdas/*`.
- Upstream content server, for the collection-prefix query only: `decentraland/catalyst` @ `a416565db1e705cff9523540a2578af642598a29`.
- dcl-one: `dcl-one-conformance` @ `0229ef9e`, workspace `services/catalyst`.

Method: everything below was read statically. Nothing was built, run or captured, so no row here is verified.

Files read (upstream lamb2):
`src/controllers/routes.ts`, `src/controllers/handlers/{wearables,emotes,third-party-wearables,explorer,explorer-emotes,all-collections,wearables-by-owner,emotes-by-owner,wearables-catalog,emotes-catalog}-handler.ts`, `src/controllers/handlers/{items-commons,pagination-utils,utils,errorHandler}.ts`, `src/logic/{pagination,pagination-constants,sorting,utils,api-with-fallback,items-by-owner,items-catalog,linked-wearables-mapper,third-party-providers-storage}.ts`, `src/logic/fetch-elements/{fetch-items,fetch-elements,graph-pagination,fetch-base-items,fetch-third-party-wearables,fetch-items-by-filters}.ts`, `src/adapters/{elements-fetcher,marketplace-api-fetcher,third-party-providers-graph-fetcher,alchemy-nft-fetcher,entities-fetcher,definitions,definitions-fetcher,content-server-url}.ts`, `src/components.ts`, `src/service.ts`, `src/types.ts`, `docs/openapi.yaml`.
Upstream catalyst: `src/adapters/pointers-repository/component.ts`, `src/controllers/routes.ts`.

Files read (dcl-one, under `services/catalyst/`):
`shop/src/market/{mod,user_items,lambdas_explorer,lambdas_catalog,third_party,base_wearables,definitions,readers}.rs`, `shop/src/market/mirrors/mod.rs`, `chain-index/src/{graph,source}.rs`, `shared/src/{query_params,http_util,cors}.rs`, `content/src/queries.rs` (projection and prefix query), `src/serve.rs` (mounts and trailing-slash layer), the fixtures `shop/src/market/fixtures/{users-wearables-*,users-emotes-200-unknown-address,collections-wearables-by-owner-200-unknown}.json`.
`shop/src/market/nft_reads.rs` and `shop/src/market/collections.rs` serve the marketplace-server `/v1/*` routes, not the lambdas routes in this area. They are listed only under "dcl-one extras" and are not compared.

## Conventions that apply to every endpoint here

These are cross-cutting differences. Endpoint sections refer back to them as C1–C9 and do not repeat them.

| # | Topic | Upstream lamb2 | dcl-one | Verdict |
|---|---|---|---|---|
| C1 | 400 body | `{"error":"Bad request","message":<msg>}` (decentraland/lamb2:src/controllers/handlers/errorHandler.ts:7-15) | `{"error":"Bad request","message":<msg>}` (services/catalyst/shared/src/http_util.rs:10-16) | MATCH |
| C2 | 404 body | `{"error":"Not Found","message"}` (errorHandler.ts:37-45) | `{"error":"Not Found","message"}` (http_util.rs:18-20) | MATCH (no route in this area raises 404 upstream) |
| C3 | Fetch failure | A `FetcherError` returns **502** `{"error":"The requested items cannot be fetched right now","message":"Cannot fetch elements for <address>"}` (errorHandler.ts:27-35; decentraland/lamb2:src/adapters/elements-fetcher.ts:265-274) | Returns **503** `{"error":"Service Unavailable","message":"ownership readers unavailable"}` (http_util.rs:22-28), or the unavailable envelope `{"error":"Service Unavailable","message","source","configure"?}` with `Retry-After: 60` (services/catalyst/chain-index/src/source.rs:88-103) | DIVERGENT |
| C4 | Overload | The marketplace bulkhead returns **503** `{"error":"Service overloaded, retry shortly","message"}` (errorHandler.ts:17-25; decentraland/lamb2:src/adapters/marketplace-api-fetcher.ts:307-318) | No bulkhead, so there is no 503 of this kind | DIVERGENT (P2) |
| C5 | Unhandled error | **500** `{"error":"Internal Server Error"}`, with no `message` (errorHandler.ts:66-76) | **500** `{"error":"Internal Server Error","message":"Internal Server Error"}` (http_util.rs:30-36) | DIVERGENT (P2) |
| C6 | JSON key order | Insertion order (JS object) | `serde_json` without `preserve_order` (services/catalyst/Cargo.toml:21-23), so `json!` objects serialize with keys sorted alphabetically | DIVERGENT (P2, semantic JSON equal) |
| C7 | Query parsing | WHATWG `URLSearchParams`: `get` returns the first value, `getAll` all values, `has` tests presence. Integers come from `parseInt(x,10)`, so `"10abc"`→10, `"1.5"`→1 and `"abc"`→`NaN` | `parse_query_string` (services/catalyst/shared/src/query_params.rs:20-38) also folds `key[0]=` into `key` (line 227-237). Integers come from `str::parse::<i64>`, so `"10abc"`, `"1.5"` and `"abc"` all fall back to the default 100/1 (line 48-50, 136-137) | DIVERGENT (P2) |
| C8 | Trailing slash | `@dcl/http-server` router (path-to-regexp). Not determined from the pinned tree | `NormalizePath::trim_trailing_slash` (services/catalyst/src/serve.rs:1291-1294) | Not determined |
| C9 | CORS preflight max-age | `cors: { maxAge: 36000 }` (decentraland/lamb2:src/components.ts:65-67) | `Access-Control-Max-Age: 86400` (services/catalyst/shared/src/cors.rs:18) | DIVERGENT (P2; shared layer, also covered by the CORS area) |

Notes:

- Upstream `pagination-utils.ts` (`ensurePagination`, `validatePagination`) has no caller anywhere in `src/`. Its messages (`pageNum must be greater than 0`, …) are therefore never served. The live pagination is `paginationObject` (decentraland/lamb2:src/logic/pagination.ts:4-19). It has defaults `pageSize=100` and `pageNum=1` (decentraland/lamb2:src/logic/pagination-constants.ts:6-11), and a single check, `pageSize > max` → 400 `max allowed pageSize is <max>`. A non-positive or NaN `pageNum`/`pageSize` passes through.
- No route in this area has auth (signed fetch), rate limiting, or a 503 for bootstrapping or read-only mode, in either tree.
- dcl-one wraps the whole lambdas router in a request-timeout layer that answers **408** (services/catalyst/shop/src/market/mod.rs:979-984). Upstream has no such response.

## Ownership data: where each tree reads it

| Data | Upstream lamb2 | dcl-one | Equal for the same chain state? |
|---|---|---|---|
| On-chain wearables/emotes an address owns | **Primary:** marketplace-server `GET {MARKETPLACE_API_URL}/v1/users/<addr>/{wearables,emotes}/grouped` (decentraland/lamb2:src/adapters/marketplace-api-fetcher.ts:395-472). This is the marketplace's own indexed DB, and its order and total come from there. **Fallback**, on any `MarketplaceApiError`: the TheGraph collections subgraphs, L1 + L2 for wearables and L2 only for emotes. The fallback walks `nfts` by `id_gt`, with no page cap (decentraland/lamb2:src/logic/fetch-elements/fetch-items.ts:237-336; decentraland/lamb2:src/logic/fetch-elements/fetch-elements.ts:13-46). The marketplace is abandoned above 100 pages of 1,000 groups (marketplace-api-fetcher.ts:362-385), and the fallback then runs. | TheGraph collections subgraphs only, L1 + L2 for both categories, through `chain-index` (services/catalyst/chain-index/src/graph.rs:233-373). It has no chain index of its own for collection NFTs. It is capped at **10 pages × 1,000 tokens per network** (graph.rs:28-29, 318-342) and memoized for 30 s (graph.rs:34). The explorer adds rows from `shop.usage_grants` (escrow grants) (services/catalyst/shop/src/market/lambdas_explorer.rs:156-175). | **Same set of tokens** when the marketplace DB agrees with the subgraph and the owner holds ≤ 10,000 tokens per network. **Different** in four cases: above that cap (dcl-one truncates; upstream walks everything); grouping order and `individualData` order (upstream follows the marketplace order, or subgraph `id` ascending in the fallback; dcl-one goes newest transfer first); explorer items granted by escrow (dcl-one only); and subgraph/marketplace indexing lag. |
| Third-party registry (providers) | TheGraph third-party registry subgraph, `thirdParties(where:{isApproved:true})`, default `first` (100) (decentraland/lamb2:src/adapters/third-party-providers-graph-fetcher.ts:12-29). Cached 6 h; an error with no stale value raises `FetcherError` (decentraland/lamb2:src/logic/third-party-providers-storage.ts:33-52). | Same subgraph and the same query for providers (services/catalyst/shop/src/market/third_party.rs:107-123, 166-184), cached 6 h. An error gives an **empty list**. The integrations query asks `first: 1000` (third_party.rs:1003). | Same providers (≤ 100). Different on registry failure and when there are more than 100 approved providers (integrations list). |
| Linked-wearable ownership (v2 mappings) | NFT worker `POST {NFT_WORKER_BASE_URL}/wallets/<owner>/networks/<network>/nfts` with the provider contract list. It returns every `network:contract:tokenId` the wallet holds (decentraland/lamb2:src/adapters/alchemy-nft-fetcher.ts:16-54) for every `ContractNetwork` (`mainnet`, `sepolia`, `matic`, `amoy`). Mappings are matched with `createMappingsHelper.includesNft` (decentraland/lamb2:src/logic/linked-wearables-mapper.ts:6-21). There is no Merkle-proof or resolver path in this revision. | JSON-RPC `eth_call` over the node's RPC endpoints: `balanceOf`, plus ERC-165 probes for 1155 and 721-Enumerable. ERC-1155 uses `balanceOfBatch` on the mapped ids. Enumerable ERC-721 uses `tokenOfOwnerByIndex`, capped at 1,000. Non-enumerable ERC-721 uses `ownerOf` on the **explicitly mapped ids only**, with `range` expanded only below 2,048 ids (services/catalyst/shop/src/market/third_party.rs:236-300, 323-509). Only `mainnet` and `matic` (third_party.rs:302-308). Any RPC failure gives **nothing owned** (third_party.rs:601-675). No Merkle-proof path. | Equal for `single`/`multiple`/small-`range` mappings on mainnet/matic. **Different** for `any` or wide `range` mappings on non-enumerable ERC-721, for enumerable holdings above 1,000, on `sepolia`/`amoy`, and on RPC failure (upstream NFT worker failure also gives `[]`). |
| Third-party entities of a provider | Content server `GET /entities/active/collections/<providerUrn>?pageSize=1000&pageNum=n` (decentraland/lamb2:src/adapters/entities-fetcher.ts:62-80). That is `pointer LIKE '<prefix>%'` (decentraland/catalyst:src/adapters/pointers-repository/component.ts:14-27). | Local content DB, with the same `pointer LIKE '<prefix>%'` (services/catalyst/content/src/queries.rs:277-311), 1,000 per page, keeping only entities with `metadata.mappings` (third_party.rs:819-856). | Equal for the same synced content. |
| Definitions and entities (`includeDefinitions`, `includeEntities`, explorer `entity`) | Content server `POST /entities/active` by pointer (decentraland/lamb2:src/adapters/definitions-fetcher.ts), mapped by `extractWearableDefinitionFromEntity` / `extractEmoteDefinitionFromEntity` with `CONTENT_URL` (lowercased, trailing `/` stripped) + `/contents/<hash>` (decentraland/lamb2:src/adapters/definitions.ts:4-102). | Local content DB `active_entity_docs_by_pointers` (services/catalyst/content/src/queries.rs:233-238), mapped by `extract_*_definition` with `content_public_url` + `contents/<hash>` (services/catalyst/shop/src/market/definitions.rs:53-160). | Equal when the content DB is synced, apart from the edge mappings in LI-14. |
| Collections list | Live L1 + L2 subgraph `collections(first:1000, orderBy:urn)` (decentraland/lamb2:src/controllers/handlers/all-collections-handler.ts:7-36). | `shop.collections` mirror filled from the same subgraphs by a periodic job (services/catalyst/shop/src/market/mirrors/mod.rs:1-31). | Equal after a complete mirror pass; lags by up to the mirror interval (default 300 s). |
| Global catalog (`/collections/wearables`, `/collections/emotes`) | Live subgraph `items(where:{searchItemType_in, urn_gt, searchText_contains, urn_in}, orderBy:urn)`, L1 then L2 (decentraland/lamb2:src/logic/fetch-elements/fetch-items-by-filters.ts:41-141). | `shop.items` mirror once it has a complete pass, filtered with SQL (`name ILIKE` stands in for `searchText_contains`) (services/catalyst/shop/src/market/lambdas_catalog.rs:421-518). Before that, the live subgraphs with a different `where` document (lambdas_catalog.rs:591-757). | `collectionId` and `wearableId` filters give equal sets. `textSearch` can differ: `searchText` is not the item name. |

## Endpoint inventory

| Method | Upstream path | Production prefix | dcl-one route(s) | Status |
|---|---|---|---|---|
| GET | `/users/:address/wearables` | `/lambdas` | `/lambdas/users/{address}/wearables`, services/catalyst/shop/src/market/mod.rs:884-887 → `user_items::user_wearables` (services/catalyst/shop/src/market/user_items.rs:319-326) | DIVERGENT |
| GET | `/users/:address/emotes` | `/lambdas` | `/lambdas/users/{address}/emotes`, mod.rs:888-891 → `user_items::user_emotes` (user_items.rs:328-335) | DIVERGENT |
| GET | `/users/:address/third-party-wearables` | `/lambdas` | mod.rs:917-920 → `third_party::user_third_party_wearables` (services/catalyst/shop/src/market/third_party.rs:885-926) | DIVERGENT |
| GET | `/users/:address/third-party-wearables/:collectionId` | `/lambdas` | mod.rs:921-924 → `third_party::user_third_party_collection_wearables` (third_party.rs:929-956) | DIVERGENT |
| GET | `/third-party-integrations` | `/lambdas` | mod.rs:913-916 → `third_party::third_party_integrations` (third_party.rs:1030-1053) | DIVERGENT |
| GET | `/explorer/:address/wearables` | `/lambdas` | `/lambdas/explorer/{address}/wearables` and `/explorer/{address}/wearables`, mod.rs:952-963 → `lambdas_explorer::explorer_wearables` (services/catalyst/shop/src/market/lambdas_explorer.rs:645-659) | GAP |
| GET | `/explorer/:address/emotes` | `/lambdas` | `/lambdas/explorer/{address}/emotes` and `/explorer/{address}/emotes`, mod.rs:956-967 → `lambdas_explorer::explorer_emotes` (lambdas_explorer.rs:662-669) | GAP |
| GET | `/nfts/collections` | `/lambdas` | `/lambdas/nfts/collections` (+ `/lambdas/collections`), mod.rs:934-941 → `lambdas_catalog::nfts_collections` (services/catalyst/shop/src/market/lambdas_catalog.rs:1071-1096) | DIVERGENT |
| GET | `/collections/wearables-by-owner/:owner` | `/lambdas` | mod.rs:896-899 → `user_items::wearables_by_owner` (user_items.rs:524-540) | DIVERGENT |
| GET | `/collections/wearables` | `/lambdas` | mod.rs:942-945 → `lambdas_catalog::collections_wearables_catalog` (lambdas_catalog.rs:1016-1024) | DIVERGENT |
| GET | `/collections/emotes-by-owner/:owner` | `/lambdas` | mod.rs:900-903 → `user_items::emotes_by_owner` (user_items.rs:542-558) | DIVERGENT |
| GET | `/collections/emotes` | `/lambdas` | mod.rs:946-949 → `lambdas_catalog::collections_emotes_catalog` (lambdas_catalog.rs:1027-1046) | DIVERGENT |

Counts: 12 endpoints. MATCH 0, GAP 2, DIVERGENT 10, MISSING 0.

Upstream routes: decentraland/lamb2:src/controllers/routes.ts:42-46, 56, 61-62, 73-76. In both trees every route answers HEAD as its GET (axum `get`; koa-style router).

---

## 1. GET /users/:address/wearables

### Request

| Param | Upstream | dcl-one | Verdict |
|---|---|---|---|
| `:address` | Not validated. Lowercased before the marketplace call and the subgraph query (marketplace-api-fetcher.ts:400; fetch-elements.ts:20). An invalid string answers 200 with an empty page (the marketplace rejects it, then the subgraph returns no rows). | Must match `is_valid_eth_address`, else 400 `Address must be a valid Ethereum address` (user_items.rs:231-233), then lowercased. | DIVERGENT |
| `pageNum` | `parseInt`, default 1, no lower bound (pagination.ts:8-10). | `i64` parse, default 1, no lower bound (`NonPositivePolicy::PassThrough`, user_items.rs:32-43). | MATCH, except C7 |
| `pageSize` | `parseInt`, default 100. A value > 1000 gives 400 `max allowed pageSize is 1000` (wearables-handler.ts:37; pagination.ts:12-14). | Same default, max and message (services/catalyst/shared/src/query_params.rs:159-166). | MATCH, except C7 |
| `pageNum` ≤ 0 | The offset is negative. The marketplace gets `offset<0`; in the fallback, `paginateItems` returns `elements: []` with the real total (fetch-items.ts:137-141). | `js_page` slices JS-style **from the end** (user_items.rs:190-199). `pageNum=0&pageSize=100` gives `slice(-100,0)` = `[]`. `pageNum=-1` gives `slice(-200,-100)`, a non-empty tail. | DIVERGENT for pageNum < 0 (P2) |
| `pageSize=abc` | `NaN` passes the max check. The fallback slice is empty and `pageSize`/`pageNum` echo as `null`. | Default 100, a normal page. | DIVERGENT (P2) |
| `category` | First value, **lowercased** (decentraland/lamb2:src/controllers/handlers/utils.ts:12-15). The marketplace filters on it. In the fallback, the subgraph `searchWearableCategory` enum; an invalid value or `body_shape` gives an empty answer (decentraland/lamb2:src/logic/fetch-elements/graph-pagination.ts:17-33). | First value, compared **case-sensitively** with `metadata.wearable.category` (user_items.rs:96-104, 265). | DIVERGENT: `category=Hat` gives an empty answer in dcl-one |
| `rarity` | First value, **lowercased**, **not validated** on this route (an unknown value matches nothing) (utils.ts:16-19; fetch-items.ts:106-117). | First value, case-sensitive, not validated (user_items.rs:105-107). | DIVERGENT in case (`rarity=Epic`) |
| `name` | Substring, case-insensitive. The marketplace uses `ILIKE`, so `%`/`_` act as wildcards there. The fallback uses `includes` (fetch-items.ts:107-116). | `to_lowercase().contains` (user_items.rs:108-111). | MATCH with the fallback; DIVERGENT for `%`/`_` versus the marketplace (P2) |
| `orderBy`/`direction` | Validated **only if `orderBy` is present**: `createSorting` gives 400 `Invalid sorting requested: <sort> <DIR>` (utils.ts:45-48; decentraland/lamb2:src/logic/sorting.ts:86-129). Sorting is then done by the marketplace. In the fallback, the rarest/leastRare/nameAZ/nameZA/newest/oldest comparators (fetch-items.ts:82-99). **Without `orderBy` nothing sorts**: the order is the marketplace default, or grouping order (subgraph `id` asc) in the fallback. `direction` without `orderBy` is ignored, though it is still forwarded to the marketplace. | Always validated and always sorted, with default `rarity DESC` (user_items.rs:114-163, 247-249, 270-272). `direction=sideways` without `orderBy` gives 400 `Invalid sorting requested: rarity SIDEWAYS`. | DIVERGENT (order without `orderBy`; `direction`-only requests) |
| `includeDefinitions` / `includeEntities` | Presence flags. Both present gives 400 `Cannot use includeEntities and includeDefinitions together` (wearables-handler.ts:29-34). | Same check and message (user_items.rs:237-241). | MATCH |
| `itemType`, `network` | Not read from the URL (utils.ts:9-31). | Not read. | MATCH |

Validation order. Upstream: flags → pageSize → sort (only when `orderBy` is present) → fetch. dcl-one: address → flags → pageSize → sort → reader check → fetch.

Auth: none. Rate limit: none (C4).

### Responses

| Trigger | Status | Body shape | Headers | Upstream | dcl-one | Verdict |
|---|---|---|---|---|---|---|
| Success | 200 | `{"elements":[Item],"totalAmount":number,"pageNum":number,"pageSize":number}`. `Item = {"urn":string,"amount":number,"individualData":[{"id":string,"tokenId":string,"transferredAt":string,"price":string}],"name":string,"category":string,"rarity":string,"definition"?:WearableDefinition,"entity"?:Entity}`. `amount = individualData.length`; `transferredAt` and `price` are stringified. | `Content-Type: application/json` | wearables-handler.ts:6-19, 52-69; utils.ts:56-67 | user_items.rs:165-186, 274-316 | DIVERGENT, see below |
| Both include flags | 400 | C1, `Cannot use includeEntities and includeDefinitions together` | json | wearables-handler.ts:32-34 | user_items.rs:239-241 | MATCH |
| `pageSize` > 1000 | 400 | C1, `max allowed pageSize is 1000` | json | pagination.ts:12-14 | services/catalyst/shared/src/query_params.rs:161-163 | MATCH |
| Bad sort (when `orderBy` is present) | 400 | C1, `Invalid sorting requested: <sort> <DIR>` (sort lowercased, DIR uppercased, no quotes) | json | sorting.ts:120-129 | user_items.rs:160 | MATCH |
| Bad `direction` without `orderBy` | upstream 200 / dcl-one 400 | — | — | utils.ts:45-48 | user_items.rs:247-249 | DIVERGENT |
| Invalid address | upstream 200 empty / dcl-one 400 `Address must be a valid Ethereum address` | — | — | (no check) | user_items.rs:231-233 | DIVERGENT |
| Marketplace and subgraph both fail | upstream 502 (C3) / dcl-one 503 `ownership readers unavailable` | — | — | elements-fetcher.ts:272-273 | user_items.rs:255-261 | DIVERGENT |
| A collections subgraph is not configured | dcl-one only: 503 envelope `{"error":"Service Unavailable","message":"the <net> collections subgraph reader is not configured on this node, and a route without a network filter answers for both networks","source","configure"}`, `Retry-After: 60` | — | — | n/a | services/catalyst/shop/src/market/readers.rs:131-148 | dcl-one only |
| Marketplace bulkhead saturated | upstream 503 (C4) | — | — | marketplace-api-fetcher.ts:307-318 | — | MISSING in dcl-one (P2) |
| Unhandled | 500 (C5) | — | — | errorHandler.ts:66-76 | — | C5 |

Field-level comparison of the success body:

- `urn`. Upstream: as the marketplace or subgraph spells it. dcl-one: subgraph URN with `:mainnet:` rewritten to `:ethereum:` (services/catalyst/chain-index/src/graph.rs:155-161). The collections subgraphs already spell L1 as `ethereum`. MATCH.
- `amount`. Both: number, `individualData.length` (wearables-handler.ts:9; user_items.rs:180). MATCH.
- `individualData[].id`. Both: `<urn>:<tokenId>`. MATCH.
- `individualData[].transferredAt`. Both: string. Upstream passes the source value through, either the marketplace string or the subgraph BigInt string. dcl-one parses to `i64` and back (services/catalyst/chain-index/src/graph.rs:248-255, 275), so a non-numeric or null value becomes `"0"`. MATCH for normal data.
- `individualData[].price`. Both: string. dcl-one uses `"0"` when the subgraph `item` is null (graph.rs:276-279). The upstream fallback throws on a null `item` (fetch-items.ts:170), which gives 502. Edge.
- `individualData` order. Upstream: marketplace order, or subgraph `id` ascending in the fallback (fetch-items.ts:157-195). dcl-one: newest transfer first, ties by `tokenId` then `price` (graph.rs:364-369). DIVERGENT (P2).
- `name`/`category`/`rarity`. Upstream: from the first row of the group (fallback) or the marketplace group. dcl-one: from the newest copy (user_items.rs:61-94). Equal for a single item.
- Element order. Without `orderBy`, DIVERGENT (see Request). With `orderBy`, dcl-one equals the upstream fallback comparators: rarity uses `SORTED_RARITIES` index with unknown = -1, then `localeCompare(urn)`; ASC is the full reverse; `date DESC` uses max transfer, then urn asc; `date ASC` uses min transfer, then urn **desc** (sorting.ts:8-36; user_items.rs:128-159). Edge: upstream `earliestDate` treats 0 as undated (fetch-items.ts:197-200); dcl-one takes the plain `min` including 0 (user_items.rs:85). Whether the marketplace order equals the fallback order is not determinable from the pinned trees.
- `totalAmount`. Upstream: the marketplace `total` (or `elements.length` when 0), or the fallback's filtered group count. dcl-one: the filtered group count. MATCH for the same data.
- `definition` when requested and absent. Upstream: `undefined`, so the key is omitted (utils.ts:62-66). dcl-one: `"definition": null` (user_items.rs:294-302). DIVERGENT (P1).
- `entity` when requested and absent. Upstream: omitted. dcl-one: `"entity": null` (user_items.rs:303-305). DIVERGENT (P1).
- Definition content: see LI-14.

Example success body (both trees, dcl-one key order alphabetical per C6):

```json
{"elements":[{"urn":"urn:decentraland:matic:collections-v2:0x00000000000000000000000000000000000000c1:0","amount":2,"individualData":[{"id":"urn:decentraland:matic:collections-v2:0x00000000000000000000000000000000000000c1:0:105312291668557186697918027683670432318895095400549111254310977537","tokenId":"105312291668557186697918027683670432318895095400549111254310977537","transferredAt":"1700000000","price":"1000000000000000000"}],"name":"Example Hat","category":"hat","rarity":"epic"}],"totalAmount":1,"pageNum":1,"pageSize":100}
```

### Data dependency

dcl-one needs both collections subgraph URLs (L1, L2) configured (readers.rs:131-148) and reachable. For `includeDefinitions`/`includeEntities` it also needs the wearable entities in its content DB. It has both sources. It does not use a marketplace-server API, so the upstream primary source has no equivalent; the subgraph path is the upstream fallback.

---

## 2. GET /users/:address/emotes

Same handler logic as §1 in both trees (decentraland/lamb2:src/controllers/handlers/emotes-handler.ts:21-70; services/catalyst/shop/src/market/user_items.rs:328-335). The Request table and the Responses table of §1 apply line for line, with these differences:

| Topic | Upstream | dcl-one | Verdict |
|---|---|---|---|
| Source networks | Marketplace emotes; fallback **L2 only** (fetch-items.ts:262-275). | L1 + L2 subgraphs, `category: "emote"` (services/catalyst/chain-index/src/graph.rs:233-243, 358-363). L1 has no emote NFTs. | MATCH in result |
| Category filter | `searchEmoteCategory` enum, lowercased (graph-pagination.ts:23-25). | Case-sensitive equality with `metadata.emote.category` (user_items.rs:102-104). | DIVERGENT (case) |
| `definition` | `extractEmoteDefinitionFromEntity` (definitions.ts:61-102). | `extract_emote_definition` (services/catalyst/shop/src/market/definitions.rs:125-160). | See LI-14 |
| Reader requirement | — | Both collections subgraphs required (user_items.rs:251-253), even though emotes live on L2 only. | DIVERGENT (dcl-one returns 503 if only L1 is missing) |

Example success: as in §1, with `"category":"dance"` and `"rarity":"rare"`.

Data dependency: as in §1. Emote entities are needed for `includeDefinitions`/`includeEntities`.

---

## 3. GET /users/:address/third-party-wearables

### Request

| Param | Upstream (decentraland/lamb2:src/controllers/handlers/third-party-wearables-handler.ts:33-74) | dcl-one (services/catalyst/shop/src/market/third_party.rs:885-926) | Verdict |
|---|---|---|---|
| `:address` | Not validated. Lowercased by the elements fetcher (elements-fetcher.ts:198). | 400 `Address must be a valid Ethereum address` unless valid (third_party.rs:890-892). | DIVERGENT |
| `pageSize` | Default 100, **max `Number.MAX_VALUE`**, so never rejected (line 42). | Max 1000, and > 1000 gives 400 `max allowed pageSize is 1000` (third_party.rs:870-882). | DIVERGENT (P1) |
| `pageNum` | Default 1, pass-through. `fetchAndPaginate` uses a JS `slice` (decentraland/lamb2:src/logic/pagination.ts:25-41). | Default 1, pass-through, JS-style slice (third_party.rs:964-978). | MATCH, except C7 |
| `category` | `getAll`, **case-sensitive**, matches `entity.metadata.data.category` (lines 13-26). | `qs_get_array`, case-sensitive (third_party.rs:858-868, 897). | MATCH |
| `name` | First value, case-insensitive substring (line 21). | Same (third_party.rs:862-866). | MATCH |
| `orderBy`/`direction` | `createBaseSorting`: default `name`. Only `name ASC|DESC` is allowed; anything else gives 400 `Invalid sorting requested: <sort> <DIR>` (sorting.ts:131-146). nameAZ = `name.localeCompare || urn.localeCompare`; nameZA is the full reverse. | Same default, the same allowed set, the same message, and the same comparator (UCA collator) (third_party.rs:899-923). | MATCH |
| `includeDefinitions` | Presence flag (line 41). | Presence flag (third_party.rs:924). | MATCH |

Validation order. Upstream: pageSize → sort → fetch. dcl-one: address → pageSize → sort.

### Responses

| Trigger | Status | Body shape | Upstream | dcl-one | Verdict |
|---|---|---|---|---|---|
| Success | 200 | `{"elements":[TPW],"totalAmount":number,"pageNum":number,"pageSize":number}`. `TPW = {"urn":string,"individualData":[{"id":"<urn>:<network>:<contract>:<tokenId>","tokenId":"<network>:<contract>:<tokenId>"}],"amount":number,"name":string,"category":string,"entity":Entity,"definition"?:WearableDefinition}`. | third-party-wearables-handler.ts:46-73; decentraland/lamb2:src/logic/fetch-elements/fetch-third-party-wearables.ts:33-58 | third_party.rs:686-696, 958-1001 | DIVERGENT in the data source (LI-9). Shape MATCH. Key order C6. |
| `includeDefinitions`, definition missing | 200 | Upstream `definitions[i]!` is `undefined`, so the key is omitted (lines 60-65). dcl-one: `"definition": null` (third_party.rs:984-990). | | | DIVERGENT (P1) |
| `pageSize` > 1000 | upstream 200 / dcl-one 400 | — | line 42 | third_party.rs:874-880 | DIVERGENT |
| Bad sort | 400 | C1, `Invalid sorting requested: date DESC` etc. | sorting.ts:144 | third_party.rs:911-913 | MATCH |
| Invalid address | upstream 200 (empty) / dcl-one 400 | — | — | third_party.rs:890-892 | DIVERGENT |
| Registry subgraph down, no cached value | upstream 502 `{"error":"The requested items cannot be fetched right now","message":"Cannot fetch elements for <address>"}` / dcl-one 200 with an empty page | — | third-party-providers-storage.ts:46-52; elements-fetcher.ts:272-273 | third_party.rs:166-184 | DIVERGENT (P1) |
| NFT worker / RPC failure | 200, nothing owned (both) | — | alchemy-nft-fetcher.ts:35-39 | third_party.rs:601-675 | MATCH |

Example success:

```json
{"elements":[{"urn":"urn:decentraland:matic:collections-thirdparty:example:collection1:hat","individualData":[{"id":"urn:decentraland:matic:collections-thirdparty:example:collection1:hat:matic:0x00000000000000000000000000000000000000c2:7","tokenId":"matic:0x00000000000000000000000000000000000000c2:7"}],"amount":1,"name":"Linked Hat","category":"hat","entity":{"version":"v3","id":"bafkreiexample","type":"wearable","pointers":["urn:decentraland:matic:collections-thirdparty:example:collection1:hat"],"timestamp":1700000000000,"content":[],"metadata":{}}}],"totalAmount":1,"pageNum":1,"pageSize":100}
```

Upstream merges duplicates by URN, last one winning (fetch-third-party-wearables.ts:144-153). dcl-one keeps the first entity per URN and adds every matching NFT to it (third_party.rs:737-787). For the same entity set, `individualData` is the same.

### Data dependency

- Upstream: the third-party registry subgraph, the NFT worker, and the content server's collection prefix query.
- dcl-one: the registry subgraph (`THIRD_PARTY_REGISTRY_L2_SUBGRAPH_URL`), the JSON-RPC endpoints for `mainnet`/`matic` (`ETH_RPC_URL`/`TRADE_RPC_URLS`/`RPC_ENDPOINT_POLYGON`), and synced third-party entities in its content DB. With no RPC endpoint for a network, it logs and reports nothing owned (third_party.rs:338-342).

---

## 4. GET /users/:address/third-party-wearables/:collectionId

### Request

| Param | Upstream (third-party-wearables-handler.ts:76-128) | dcl-one (third_party.rs:929-956) | Verdict |
|---|---|---|---|
| `:address` | Not validated. | 400 `Address must be a valid Ethereum address`. | DIVERGENT |
| `:collectionId` | Cut to the first 5 `:` parts, then parsed with `@dcl/urn-resolver` (which validates the network segment). Unparseable gives 400 `Invalid collection id: <collectionId> not a valid URN`. Parseable but not `blockchain-collection-third-party-name` gives 400 `Invalid collection id: not a blockchain-collection-third-party-name nor blockchain-collection-third-party-collection URN` (lines 85-97). | Cut to 5 parts. The structural check is `urn:decentraland:<any>:collections-thirdparty:<non-empty>` (services/catalyst/shop/src/market/user_items.rs:403-410). Any failure gives 400 `Invalid collection id: <collectionId> not a valid URN` (third_party.rs:937-946). | DIVERGENT: the second message is never produced, and the network segment is not validated (P2) |
| `pageSize` | Default 100, max `Number.MAX_VALUE`. | Max 1000, which gives a 400. | DIVERGENT (P1) |
| `pageNum` | Pass-through. | Pass-through. | MATCH |
| `includeDefinitions` | Presence. | Presence. | MATCH |
| `category`, `name`, `orderBy` | Not read. No filter and **no sort**: elements come in fetcher order (entity insertion order, lines 102-105). | Not read for filtering, but **always sorted by name then urn** (third_party.rs:953). | DIVERGENT in order (P2) |

Upstream filtering: it takes the owner's full third-party list and keeps the wearables whose URN parses to `blockchain-collection-third-party` and whose `thirdPartyName` equals the requested one (fetch-third-party-wearables.ts:156-176). dcl-one fetches only the providers whose id has that name at part 4 (third_party.rs:708-710). The resulting set is the same.

### Responses

| Trigger | Status | Body | Upstream | dcl-one | Verdict |
|---|---|---|---|---|---|
| Success | 200 | `{"elements":[TPW],"totalAmount","pageNum","pageSize"}` as in §3 | lines 102-127 | third_party.rs:952-955 | DIVERGENT (order; data source as in §3) |
| Provider not in the registry | upstream **502** `{"error":"The requested items cannot be fetched right now","message":"Third Party not found: <name>"}` / dcl-one **200** with an empty page | — | fetch-third-party-wearables.ts:178-183; errorHandler.ts:27-35 | third_party.rs:711-713 | DIVERGENT (P1) |
| Bad URN | 400 | C1, messages as in Request | lines 88-97 | third_party.rs:942-946 | DIVERGENT (second message) |
| `includeDefinitions`, missing definition | 200 | Upstream `definition: undefined` (omitted). dcl-one `null`. | lines 107-120 | third_party.rs:984-990 | DIVERGENT (P1) |
| Registry down | upstream 502 / dcl-one 200 with an empty page | — | third-party-providers-storage.ts:46-52 | third_party.rs:180-183 | DIVERGENT |

Data dependency: as in §3.

---

## 5. GET /third-party-integrations

### Request

No parameters, no auth.

### Responses

| Trigger | Status | Body | Upstream | dcl-one | Verdict |
|---|---|---|---|---|---|
| Success | 200 | `{"data":[{"name":string,"description":string,"urn":string}]}` in subgraph order. Upstream reads the provider cache (`thirdParties(where:{isApproved:true})`, default `first`, so at most 100). dcl-one runs its own query with `first: 1000`. | third-party-wearables-handler.ts:143-156; third-party-providers-graph-fetcher.ts:12-29 | third_party.rs:1003-1053 | DIVERGENT when there are more than 100 approved providers (P2) |
| `name`/`description` null in the subgraph | 200 | Upstream: `null`, or the key omitted if undefined. dcl-one: `""` (third_party.rs:1015-1022). | | | DIVERGENT (P2) |
| Registry fails, no cached value | upstream 502 `{"error":"The requested items cannot be fetched right now","message":"Cannot fetch third party providers"}` / dcl-one 200 `{"data":[]}` | — | third-party-providers-storage.ts:51 | third_party.rs:1048-1051 | DIVERGENT (P1) |
| Registry URL not configured | dcl-one only: 200 `{"data":[]}` | — | — | third_party.rs:1031-1036 | dcl-one only |
| Contracts | Upstream lowercases `contracts[].network/address` (decentraland/lamb2:src/logic/utils.ts:73-82). They are not part of this response. | n/a | | | n/a |

Example: `{"data":[{"name":"Example Provider","description":"Linked wearables","urn":"urn:decentraland:matic:collections-thirdparty:example"}]}`

Data dependency: the third-party registry subgraph, in both trees. The cache is 6 h in both.

---

## 6. GET /explorer/:address/wearables

### Request

| Param | Upstream (decentraland/lamb2:src/controllers/handlers/explorer-handler.ts:162-245) | dcl-one (services/catalyst/shop/src/market/lambdas_explorer.rs:63-115, 523-642) | Verdict |
|---|---|---|---|
| `:address` | Not validated. Base wearables are still returned for any string. | 400 `Address must be a valid Ethereum address` (lambdas_explorer.rs:530-532). | DIVERGENT (P2) |
| `pageNum`/`pageSize` | `paginationObject(url)` with max 1000. `fetchAndPaginate` uses a JS `slice`, so a negative offset counts from the end (pagination.ts:25-41). | Same max and message, but the slice clamps a negative offset to 0, with `skip/take` (lambdas_explorer.rs:573-579). `pageNum=-1` gives page 1 instead of the tail. | DIVERGENT (P2) |
| `category` | `getAll`, lowercased, any-of (decentraland/lamb2:src/controllers/handlers/items-commons.ts:12-14, 29-31). | `qs_get_array`, lowercased (lambdas_explorer.rs:87-90). | MATCH |
| `name` | First value, lowercased, substring; an item without a name is excluded (items-commons.ts:15, 26-28). | Same (lambdas_explorer.rs:86, 372-376). | MATCH |
| `rarity` | First value, lowercased, validated against `SORTED_RARITIES`: 400 `Invalid rarity requested: '<rarity>'.` Items without a rarity (base, third-party) are excluded (items-commons.ts:16-24). | Same message and rule (lambdas_explorer.rs:91-94, 367-371; services/catalyst/shop/src/market/definitions.rs:19-25). | MATCH |
| `orderBy`/`direction` | `createCombinedSorting`: default `rarity` (DESC). An invalid value gives 400 `Invalid sorting requested: '<sort> <DIR>'. Valid options are '[rarity, name, date] [ASC, DESC]'.` (sorting.ts:148-167). | Same default and message (definitions.rs:31-43; lambdas_explorer.rs:95-102). | MATCH |
| `collectionType` | `getAll`, default all three. Any value outside the set gives 400 `Invalid collection type. Valid types are: base-wearable, on-chain, third-party.` (lines 23, 177-195). | Same (lambdas_explorer.rs:69-78). | MATCH |
| `thirdPartyCollectionId` | `getAll`. When present, the third-party leg holds only those providers' wearables (5-part cut; invalid ids dropped) (lines 106-143, 180-182). | **Not read**: every provider is always returned. | MISSING (P0) |
| `trimmed` | `true` or `1` (lines 183-184). | Same (lambdas_explorer.rs:103). | MATCH |
| `isSmartWearable` | `true` or `1`: the on-chain leg asks `itemType=smartWearable` (only `smart_wearable_v1`), and **base and third-party are skipped** (lines 75-79, 145-157, 185-186). | **Not read.** | MISSING (P0) |
| `network` | `ETHEREUM` or `MATIC` (exact case; anything else is ignored). The on-chain leg is limited to that network's item types (`wearable_v1` for ETHEREUM; `wearable_v2`+`smart_wearable_v1` for MATIC), and base and third-party are skipped (lines 187-189; marketplace-api-fetcher.ts:282-297; graph-pagination.ts:35-56). | **Not read.** | MISSING (P0) |
| `includeAmount` | `true` or `1`: trimmed elements carry `amount`. Without it, `amount` is absent (lines 190-191, 209-216). | Not read. Trimmed elements **always** carry `amount` (lambdas_explorer.rs:475). | DIVERGENT (P1) |

Validation order. Upstream: pageSize → rarity → sort → collectionType. dcl-one: address → collectionType → pageSize → rarity → sort. A request with several errors gets a different message (P2).

### Responses

| Trigger | Status | Body | Upstream | dcl-one | Verdict |
|---|---|---|---|---|---|
| Success, not trimmed | 200 | `{"elements":[Mixed],"totalAmount","pageNum","pageSize"}`. Base: `{"type":"base-wearable","urn","name","category","individualData":[{"id":urn}],"amount":1,"entity"}`. On-chain: `{"type":"on-chain","urn","amount","individualData":[{"id","tokenId","transferredAt","price"}],"name","rarity","itemType","category","entity"}`, with `min/maxTransferredAt` removed. Third-party: `{"type":"third-party","urn","individualData":[{"id","tokenId"}],"amount","name","category","entity"}`. | lines 37-160, 226-243; decentraland/lamb2:src/logic/fetch-elements/fetch-base-items.ts:292-313 | lambdas_explorer.rs:385-431, 479-521 | DIVERGENT, see list |
| Success, trimmed | 200 | Upstream elements: `{"entity":{"id":entity.id,"thumbnail":<hash>|absent,"individualData":[...],"metadata":{"id","name","rarity"?,"isSmart":bool,"data":{"category","representations":[{"bodyShapes":[...]}]}}},"amount"?:number}` (decentraland/lamb2:src/logic/utils.ts:84-114) | lines 208-225 | lambdas_explorer.rs:433-477 | DIVERGENT (P0), see list |
| Invalid rarity / sort / collectionType / pageSize | 400 | C1 with the messages above | items-commons.ts:18-20; sorting.ts:164-166; explorer-handler.ts:193-195; pagination.ts:12-14 | lambdas_explorer.rs:74-102 | MATCH (messages) |
| Invalid address | upstream 200 / dcl-one 400 | — | — | lambdas_explorer.rs:530-532 | DIVERGENT |
| Owned fetch fails | upstream 502 (C3) / dcl-one 503 `ownership readers unavailable` | — | elements-fetcher.ts:272-273 | lambdas_explorer.rs:582-585 | DIVERGENT |
| Collections subgraph not configured and on-chain requested | dcl-one 503 envelope | — | — | lambdas_explorer.rs:537-541 | dcl-one only |
| Third-party registry fails | upstream 502 / dcl-one: the third-party leg is empty and the rest is served | — | third-party-providers-storage.ts:51 | third_party.rs:180-183 | DIVERGENT |

Field-level differences:

1. **Trimmed entity shape.**
   - Upstream builds a new, minimal object: `id`, `thumbnail` (the content hash of `metadata.thumbnail`, absent if not found), `individualData`, and `metadata` reduced to `id`, `name`, `rarity`, `isSmart`, and `data.category` plus `data.representations[].bodyShapes` only.
   - dcl-one returns the **whole entity**: `version`, `type`, `pointers`, `timestamp`, `content`, and the full `metadata`. It overwrites `thumbnail` with the hash, looking up `thumbnail.png` when `metadata.thumbnail` is missing. It fills `metadata.name` only when that is empty, and adds `individualData`.
   - It never sets `metadata.isSmart`. Upstream sets `isSmart = itemType === 'smart_wearable_v1'`.
   - Upstream `metadata.name` is the item's grouped `name` (marketplace or subgraph), not the entity's.
   - Sources: utils.ts:84-114; lambdas_explorer.rs:433-477.
   - Verdict: DIVERGENT (P0). `isSmart` is missing and the payload is a different object.
2. **`amount` in trimmed.** Upstream includes it only with `includeAmount`, as `individualData.length || 0`. dcl-one always includes it. P1.
3. **On-chain `itemType`.** Upstream: the marketplace `itemType`, or the subgraph `itemType` in the fallback, absent if missing. dcl-one: from the `shop.items` mirror, `""` when the mirror has no row (lambdas_explorer.rs:262-274, 422-427). P1.
4. **On-chain `amount`.** Upstream: the marketplace `amount` (`toFiniteNumber(amount, individualData.length)`, marketplace-api-fetcher.ts:160). dcl-one: the count of copies. MATCH for consistent data.
5. **On-chain `name`/`category`/`rarity`.** Upstream: marketplace or subgraph item metadata. dcl-one: `name`/`category` come from the content entity metadata (with an `i18n[0].text` fallback), and `rarity` from the subgraph item (lambdas_explorer.rs:276-308). Equal when the entity and the subgraph agree.
6. **Extra on-chain rows.** dcl-one adds active escrow grants from `shop.usage_grants` as owned copies, with `price: ""` and `transferredAt` = grant time (lambdas_explorer.rs:156-175). Upstream has no such source. DIVERGENT (P1, a dcl-one extension).
7. **Item without an entity.** Dropped in both (explorer-handler.ts:61-71, 93-103; lambdas_explorer.rs:248-250). MATCH.
8. **Rarity sort.**
   - Upstream `rarestOptional` treats any non-empty `rarity` string as "has rarity" (rank -1 when unknown) (sorting.ts:38-56).
   - dcl-one requires the rank to be ≥ 0 (lambdas_explorer.rs:317-318), so an unknown non-empty rarity sorts with the unrated items.
   - Edge, P2.
9. **Date sort.**
   - Upstream `hasDate` needs both `maxTransferredAt` and `minTransferredAt` to be non-zero (sorting.ts:58-60).
   - dcl-one marks every on-chain item as dated (lambdas_explorer.rs:220).
   - Edge, P2.
10. **Base wearables.**
    - The pointer list is identical: 282 URNs, compared with `diff` (decentraland/lamb2:src/logic/fetch-elements/fetch-base-items.ts:6-290; services/catalyst/shop/src/market/base_wearables.rs:13-296).
    - `urn = metadata.id`, `name = metadata.name || urn part 4`, `individualData:[{id}]` and `amount:1` match in both (base_wearables.rs:351-382; lambdas_explorer.rs:479-499).
    - Base wearables carry no `rarity`, so they are excluded by `rarity` filters in both.
    - MATCH.
11. **Caching.** Upstream elements are cached 60 s, plus 60 s stale-while-revalidate. dcl-one caches the computed list for 30 s (lambdas_explorer.rs:38). Not a wire difference.

Example (non-trimmed, one base element):

```json
{"elements":[{"type":"base-wearable","urn":"urn:decentraland:off-chain:base-avatars:BaseMale","name":"Male Body","category":"body_shape","individualData":[{"id":"urn:decentraland:off-chain:base-avatars:BaseMale"}],"amount":1,"entity":{"version":"v3","id":"bafkreiexample","type":"wearable","pointers":["urn:decentraland:off-chain:base-avatars:basemale"],"timestamp":1700000000000,"content":[],"metadata":{}}}],"totalAmount":1,"pageNum":1,"pageSize":100}
```

### Data dependency

- Base wearable entities in the content DB.
- Both collections subgraphs (on-chain).
- The `shop.items` mirror (`itemType`).
- `shop.usage_grants` (dcl-one only).
- The third-party registry and RPC (§3).

dcl-one has all of these. The `itemType` mirror needs a completed items mirror pass to be filled.

---

## 7. GET /explorer/:address/emotes

### Request

Upstream: decentraland/lamb2:src/controllers/handlers/explorer-emotes-handler.ts:64-123. dcl-one: services/catalyst/shop/src/market/lambdas_explorer.rs:662-669, sharing the code of §6 with `valid_collection_types = ["on-chain"]`.

| Param | Upstream | dcl-one | Verdict |
|---|---|---|---|
| `:address` | Not validated | 400 if invalid | DIVERGENT |
| `pageNum`/`pageSize` | As in §6 | As in §6 | DIVERGENT for a negative `pageNum` (P2) |
| `category`/`name`/`rarity`/`orderBy`/`direction` | `createFilters`/`createCombinedSorting`, as in §6 | As in §6 | MATCH |
| `collectionType` | Only `on-chain`; otherwise 400 `Invalid collection type. Valid types are: on-chain.` (lines 16, 79-81) | Same (lambdas_explorer.rs:668) | MATCH |
| `trimmed` | `true`/`1` | Same | MATCH |
| `includeAmount` | `true`/`1` (lines 76-77, 95-97) | Not read; `amount` is always present | DIVERGENT (P1) |

### Responses

| Trigger | Status | Body | Upstream | dcl-one | Verdict |
|---|---|---|---|---|---|
| Success, not trimmed | 200 | `{"elements":[{"type":"on-chain","urn","amount","individualData","name","rarity","category","entity"}],...}`. Upstream also includes `itemType` when the source provides it (the subgraph fallback sets `emote_v1`; the marketplace mapping has none) | lines 108-121 | lambdas_explorer.rs:413-430 (`itemType` omitted for emotes, line 630) | DIVERGENT (P2) only in the fallback case |
| Success, trimmed | 200 | Upstream: `{"entity":{"id","thumbnail"?,"individualData","metadata":{"id","name","rarity","data":{"category":emoteDataADR74.category,"representations":[{"bodyShapes"}]}}},"amount"?}` (decentraland/lamb2:src/logic/utils.ts:116-145). dcl-one: the full entity plus `thumbnail`, `individualData` and `amount` | lines 90-107 | lambdas_explorer.rs:433-477 | DIVERGENT (P1). Emote trimmed has no `isSmart` upstream, so the impact is payload shape and `amount` presence |
| Errors | as in §6 | | | | as in §6 |

Field detail: an emote's `category` comes from the subgraph or the marketplace upstream. dcl-one takes it from `metadata.emoteDataADR74.category`, falling back to `emoteDataV0.category`, which v0 emotes do not carry, so the result is `""` (lambdas_explorer.rs:293-299). A v0 emote then fails every `category` filter in dcl-one. P2.

Data dependency: emote entities in the content DB, both collections subgraphs, and `shop.usage_grants` (`category = 'emote'`).

---

## 8. GET /nfts/collections

### Request

No parameters.

### Responses

| Trigger | Status | Body | Upstream | dcl-one | Verdict |
|---|---|---|---|---|---|
| Success | 200 | `{"collections":[{"id":"urn:decentraland:off-chain:base-avatars","name":"Base Wearables"},{"id":"urn:decentraland:off-chain:base-emotes","name":"Base Emotes"},...L1 (≤1000, urn asc), ...L2 (≤1000, urn asc)]}` | decentraland/lamb2:src/controllers/handlers/all-collections-handler.ts:15-54 | services/catalyst/shop/src/market/lambdas_catalog.rs:1048-1096 | DIVERGENT: dcl-one adds `"as_of"` and `"stale_after_secs"` as the first keys (services/catalyst/shop/src/market/mirrors/mod.rs:621-645), P2 |
| Ordering | | Subgraph `orderBy: urn` versus mirror `ORDER BY urn COLLATE "C"` per network, L1 then L2 | all-collections-handler.ts:9, 34-36 | lambdas_catalog.rs:1057-1066 | MATCH, assuming the subgraph orders strings in byte order (not determinable from the trees) |
| Subgraph error | upstream 500 `{"error":"Internal Server Error"}` (C5) | | errorHandler.ts:66-76 | | DIVERGENT, see next row |
| Mirror without a complete pass | dcl-one 503 envelope (`Retry-After: 60`) | | — | lambdas_catalog.rs:1072-1074 | dcl-one only |
| Mirror query error | dcl-one 500 `{"error":"Internal Server Error","message":"Internal Server Error"}` | | | lambdas_catalog.rs:1091-1094 | C5 |
| Collection `name` null | upstream `"name": null`; dcl-one decodes `(String, String)`, so a NULL name fails the query and gives 500 | | | lambdas_catalog.rs:1057-1066 | DIVERGENT (P2, edge) |

Data dependency: dcl-one needs the `shop.collections` mirror (job `collections-mirror`) with a complete pass on both networks. Upstream reads live.

---

## 9. GET /collections/wearables-by-owner/:owner (legacy-lambdas port; verbatim wire contract)

### Request

| Param | Upstream (decentraland/lamb2:src/controllers/handlers/wearables-by-owner-handler.ts:12-34; decentraland/lamb2:src/logic/items-by-owner.ts:24-66) | dcl-one (services/catalyst/shop/src/market/user_items.rs:424-540) | Verdict |
|---|---|---|---|
| `:owner` | Not validated | Not validated; lowercased for the subgraph | MATCH |
| `includeDefinitions` | Presence only; `includeDefinitions=false` still enables it (items-by-owner.ts:29-31) | Raw query scan for `includeDefinitions` or `includeDefinitions=` (user_items.rs:390-396). A percent-encoded key (`include%44efinitions`) is not recognized | MATCH (P2 edge) |
| `collectionId` | `get` (first value). An empty string means on-chain. Otherwise the 5-part cut, which must parse as `blockchain-collection-third-party-name`, else 400 `'collectionId' must be a valid third-party collection URN` (items-by-owner.ts:32-36, 54-63) | First value; empty gives `None`. Structural check, same message (user_items.rs:398-422, 432-436) | MATCH (the network segment is not validated in dcl-one, P2) |
| Pagination | None. A flat array of every item | None | MATCH |

### Responses

| Trigger | Status | Body | Upstream | dcl-one | Verdict |
|---|---|---|---|---|---|
| Success, on-chain | 200 | `[{"urn":string,"amount":number,"definition"?:WearableDefinition}]`, with `amount = individualData.length` | items-by-owner.ts:38-52 | user_items.rs:474-522 | DIVERGENT in order: upstream follows the fetcher (marketplace default, or subgraph `id` asc grouping in the fallback; not sorted); dcl-one uses rarity DESC then `localeCompare(urn)` (user_items.rs:382-388, 495). P2 |
| `definition` missing | 200 | Upstream `definition: undefined`, omitted (line 43). dcl-one omits the key (user_items.rs:512-517) | | | MATCH |
| Success, third-party `collectionId` | 200 | `[{"urn","amount","definition"?}]` in fetcher order | items-by-owner.ts:64-65 | user_items.rs:432-450 (the `elements_from` insertion order) | MATCH (order equal for the same entity order) |
| Third-party provider unknown | upstream 502 `{"error":"The requested items cannot be fetched right now","message":"Third Party not found: <name>"}` / dcl-one 200 `[]` | | fetch-third-party-wearables.ts:178-183 | third_party.rs:1056-1066 | DIVERGENT (P1) |
| Bad `collectionId` | 400 | C1, `'collectionId' must be a valid third-party collection URN` | items-by-owner.ts:61-63 | user_items.rs:434-436 | MATCH |
| Owned fetch fails | upstream 502 (C3) / dcl-one 503 `ownership readers unavailable` | | elements-fetcher.ts:272-273 | user_items.rs:465-471 | DIVERGENT |
| Subgraph not configured | dcl-one 503 envelope | | | user_items.rs:451-453 | dcl-one only |

Example: `[{"urn":"urn:decentraland:matic:collections-v2:0x00000000000000000000000000000000000000c1:0","amount":2}]`. The existing fixture `services/catalyst/shop/src/market/fixtures/collections-wearables-by-owner-200-unknown.json` (`[]` for an unknown owner) agrees with upstream. It is hand-authored, not captured.

Data dependency: as in §1. For `collectionId`, as in §3.

---

## 10. GET /collections/wearables (legacy-lambdas port; verbatim wire contract)

### Request

| Param | Upstream (decentraland/lamb2:src/controllers/handlers/wearables-catalog-handler.ts:26-76; decentraland/lamb2:src/logic/items-catalog.ts:14-43) | dcl-one (services/catalyst/shop/src/market/lambdas_catalog.rs:226-382, 874-954) | Verdict |
|---|---|---|---|
| `collectionId` | `getAll`, lowercased | `qs_get_array`, lowercased | MATCH |
| `wearableId` | `getAll`, lowercased | Same | MATCH |
| `textSearch` | `get`, lowercased; an empty string means absent; a value shorter than 3 (UTF-16 length) gives 400 `The text search must be at least 3 characters long` | Same message, but counts **chars** (lambdas_catalog.rs:352-356) | MATCH (P2 for astral characters) |
| No filter | 400 `You must use one of the filters: 'textSearch', 'collectionId' or 'wearableId'` | Same message, but any **extended** filter (`collectionType`, `category`, `rarity`, `name`) also satisfies the requirement (lambdas_catalog.rs:343-351) | DIVERGENT: `?category=hat` is 400 upstream and 200 in dcl-one |
| More than 500 ids | 400 `You can't ask for more than 500 wearables` / `You can't filter for more than 500 collection ids` | Same | MATCH |
| `limit` | `parseInt`; NaN, ≤0 or >500 give 500 (items-catalog.ts:88-97) | `i64` parse, same clamp (lambdas_catalog.rs:323-328); `"10abc"` gives 500 instead of 10 | DIVERGENT (C7, P2) |
| `lastId` | `get`, lowercased; empty means absent | Same (lambdas_catalog.rs:380) | MATCH |
| `collectionType`, `category`, `rarity`, `name`, `orderBy`, `direction` | **Ignored** | Parsed and validated. `collectionType=xyz` gives 400 `Invalid collectionType 'xyz'. ...`; an invalid rarity or sort gives 400; `orderBy` without exactly one `collectionType` gives 400 (lambdas_catalog.rs:137-148, 226-263) | DIVERGENT (dcl-one extension; a request upstream accepts can be a 400) |

### Responses

| Trigger | Status | Body | Upstream | dcl-one | Verdict |
|---|---|---|---|---|---|
| Success | 200 | `{"wearables":[WearableDefinition],"filters":{"collectionIds"?:[...],"itemIds"?:[...],"textSearch"?:string},"pagination":{"limit":number,"lastId"?:string,"next"?:"?<query>"}}`. Undefined keys are omitted by `JSON.stringify` | wearables-catalog-handler.ts:64-75 | lambdas_catalog.rs:852-872 | DIVERGENT: dcl-one **never emits `pagination.lastId`** (lambdas_catalog.rs:860-865). P1 |
| Ordering | | Base wearables first: in-range base items sorted by lowercased URN with `localeCompare`. Then on-chain definitions, re-sorted by lowercased `id` with `localeCompare` (items-catalog.ts:45-61; wearables-catalog-handler.ts:88-109) | | dcl-one: base sorted by lowercased URN **byte order** (lambdas_catalog.rs:827); on-chain sorted by lowercased id byte order (lambdas_catalog.rs:765-794) | DIVERGENT where UCA and byte order disagree (`_` versus digits, P2) |
| On-chain source | | Upstream L1 `items` first, then L2, each `orderBy: urn asc`, with `urn_gt: lastId`. The cursor's network picks the starting layer (fetch-items-by-filters.ts:41-70) | | dcl-one mirror: one SQL query over both networks with `lower(urn) COLLATE "C" > lastId` (lambdas_catalog.rs:421-518). Before the first pass: the subgraphs, with a different query (lambdas_catalog.rs:591-757) | MATCH in order for L1 < L2 (`ethereum` < `matic`) |
| `textSearch` semantics | | Upstream: subgraph `searchText_contains` on items | | Mirror: `i.name ILIKE '%t%'` (lambdas_catalog.rs:467-470); subgraph fallback: `searchText_contains` (lambdas_catalog.rs:625-627) | DIVERGENT on the mirror path (P1) |
| `collectionId` semantics | | Upstream: `collections(where:{urn_in})` exact, then nested `items` | | dcl-one: `lower(i.urn) LIKE '<collectionId>:%'` (lambdas_catalog.rs:440-455) | MATCH for collection URNs. A partial URN (for example a v2 collection id lacking the contract) matches nothing upstream but may match a prefix in dcl-one (P2) |
| `next` encoding | | `URLSearchParams.toString()`: space becomes `+`, `*` is kept, `~` becomes `%7E` (items-catalog.ts:63-86) | | `urlencoding::encode`: space becomes `%20`, `*` becomes `%2A`, `~` is kept (lambdas_catalog.rs:150-152, 384-410) | DIVERGENT (P2; `:` is `%3A` in both) |
| Mirror read | | — | | Adds `as_of` and `stale_after_secs` keys when the mirror was read (mirrors/mod.rs:621-645) | DIVERGENT (P2) |
| Base-only cursor | | Base items are included when there is no `lastId` or `lastId` starts with the base collection id (wearables-catalog-handler.ts:41-48) | | Same (lambdas_catalog.rs:883-892) | MATCH |
| Definition mapping | | `extractWearableDefinitionFromEntity` | | `extract_wearable_definition` | See LI-14 |
| Validation errors | 400 | C1 with the messages above | items-catalog.ts:20-32 | lambdas_catalog.rs:343-371 | MATCH (messages) |
| Subgraph error | upstream 500 (C5) / dcl-one 503 envelope `the collections subgraph reader did not answer` (subgraph path) or 500 (mirror SQL error) | | | lambdas_catalog.rs:546-565 | DIVERGENT (P1) |
| No mirror and no subgraph | dcl-one 503 envelope | | | lambdas_catalog.rs:529-535 | dcl-one only |

Example (upstream; dcl-one lacks `lastId`):

```json
{"wearables":[{"id":"urn:decentraland:matic:collections-v2:0x00000000000000000000000000000000000000c1:0","name":"Example Hat","thumbnail":"https://peer.example/content/contents/bafkreithumb","image":"https://peer.example/content/contents/bafkreiimage","data":{"category":"hat","representations":[{"bodyShapes":["urn:decentraland:off-chain:base-avatars:BaseMale"],"mainFile":"hat.glb","contents":[{"key":"hat.glb","url":"https://peer.example/content/contents/bafkreihat"}]}]}}],"filters":{"collectionIds":["urn:decentraland:matic:collections-v2:0x00000000000000000000000000000000000000c1"]},"pagination":{"limit":1,"lastId":"urn:decentraland:matic:collections-v2:0x00000000000000000000000000000000000000c0:9","next":"?collectionId=urn%3Adecentraland%3Amatic%3Acollections-v2%3A0x00000000000000000000000000000000000000c1&limit=1&lastId=urn%3Adecentraland%3Amatic%3Acollections-v2%3A0x00000000000000000000000000000000000000c1%3A0"}}
```

Data dependency:

- The `shop.items` mirror with a complete pass (preferred), or live collections subgraphs.
- Base wearable and on-chain item entities in the content DB.
- `content_public_url` set to the public `/content` base.

---

## 11. GET /collections/emotes-by-owner/:owner (legacy-lambdas port)

Identical to §9 with `emotesFetcher`/`emoteDefinitionsFetcher` (decentraland/lamb2:src/controllers/handlers/emotes-by-owner-handler.ts:10-32) and `user_items::emotes_by_owner` (services/catalyst/shop/src/market/user_items.rs:542-558).

Every row of §9's tables applies. In addition:

- With `collectionId`, both trees resolve **third-party wearables**, not emotes (items-by-owner.ts:64; user_items.rs:438). With `includeDefinitions`, they map them with the emote extractor (upstream through `emoteDefinitionsFetcher`; dcl-one `extract_emote_definition` on the entity, user_items.rs:443). MATCH.
- Upstream's on-chain emotes source is L2 only. dcl-one queries L1 + L2 and requires both readers configured (user_items.rs:451-453).

Data dependency: as in §2.

---

## 12. GET /collections/emotes (legacy-lambdas port)

### Request

As in §10, with `emoteId` in place of `wearableId` and the label `emotes`. There are no base emotes and no off-chain merge (decentraland/lamb2:src/controllers/handlers/emotes-catalog-handler.ts:22-45; items-catalog.ts:26-29). dcl-one passes `ExtendedFilters::default()`, so `collectionType`/`category`/`rarity`/`name`/`orderBy` are **not** parsed here (services/catalyst/shop/src/market/lambdas_catalog.rs:1027-1046). That matches upstream.

### Responses

| Trigger | Status | Body | Upstream | dcl-one | Verdict |
|---|---|---|---|---|---|
| Success | 200 | `{"emotes":[EmoteDefinition],"filters":{...},"pagination":{"limit","lastId"?,"next"?}}` | emotes-catalog-handler.ts:31-44 | lambdas_catalog.rs:956-1007 | DIVERGENT: `pagination.lastId` is never emitted (P1); `as_of` on the mirror path (P2) |
| Source | | Upstream: L2 subgraph only (`queryL1: false`), `urn_gt` cursor, `searchText_contains` (fetch-items-by-filters.ts:33-39) | | Mirror `item_type = 'emote_v1'` over both networks, `name ILIKE` | DIVERGENT on `textSearch` (P1) |
| Ordering | | `localeCompare` of lowercased id (items-catalog.ts:52-54) | | Byte order of lowercased id (lambdas_catalog.rs:765-794) | DIVERGENT (P2 edge) |
| Validation | 400 | `You must use one of the filters: 'textSearch', 'collectionId' or 'emoteId'`; `You can't ask for more than 500 emotes`; `You can't filter for more than 500 collection ids`; `The text search must be at least 3 characters long` | items-catalog.ts:20-32 | lambdas_catalog.rs:343-371 | MATCH |
| Subgraph or mirror unavailable | upstream 500 / dcl-one 503 envelope or 500 | | | lambdas_catalog.rs:529-567 | DIVERGENT |

Data dependency: the `shop.items` mirror (or the collections subgraphs) and emote entities in the content DB.

---

## Backlog

**LI-1 (P0). `/explorer/:address/wearables`: `isSmartWearable` and `network` are ignored.**

- Expected (decentraland/lamb2:src/controllers/handlers/explorer-handler.ts:145-157, 185-189; decentraland/lamb2:src/adapters/marketplace-api-fetcher.ts:282-297; decentraland/lamb2:src/logic/fetch-elements/graph-pagination.ts:35-56):
  - `isSmartWearable=true|1` restricts the on-chain leg to `smart_wearable_v1` and drops the base and third-party legs.
  - `network=ETHEREUM` restricts the on-chain leg to `wearable_v1`; `network=MATIC` restricts it to `wearable_v2`+`smart_wearable_v1`. Both drop the base and third-party legs.
  - Any other `network` value is ignored.
- Change: `parse_query` and `compute_explorer_items` in services/catalyst/shop/src/market/lambdas_explorer.rs:63-115, 589-642. Filter the owned copies by subgraph `itemType`: add `itemType` to `QUERY_OWNED` in services/catalyst/chain-index/src/graph.rs:233-243, or use the `shop.items` type. Add both params to the cache key (lambdas_explorer.rs:547-561).

**LI-2 (P0). `/explorer/:address/wearables`: `thirdPartyCollectionId` is ignored.**

- Expected (explorer-handler.ts:106-143, 180-182): when the param is present (repeatable), each value is cut to 5 `:` parts and kept only if it is a `blockchain-collection-third-party-name` URN (invalid ones are silently dropped). Only those providers' wearables form the third-party leg.
- Change: lambdas_explorer.rs:501-521, 617-623. Call `fetch_all_third_party_wearables(state, owner, Some(name))` per unique name (services/catalyst/shop/src/market/third_party.rs:700-735).

**LI-3 (P0). Explorer `trimmed=true` returns the whole entity instead of the trimmed object, and `metadata.isSmart` is missing.**

- Expected wearables (decentraland/lamb2:src/logic/utils.ts:84-114): `{"entity":{"id":entity.id,"thumbnail":<hash of metadata.thumbnail, absent if none>,"individualData":[...],"metadata":{"id":metadata.id,"name":<item name>,"rarity":metadata.rarity,"isSmart":itemType==="smart_wearable_v1","data":{"category":metadata.data.category,"representations":[{"bodyShapes":[...]}]}}}}`.
- Expected emotes (utils.ts:116-145): the same, without `isSmart`, with `name = metadata.name` and `data` taken from `emoteDataADR74`.
- Change: `item_to_trimmed_value` in services/catalyst/shop/src/market/lambdas_explorer.rs:433-477. It also needs the item type for base and third-party items (both `isSmart:false`).

**LI-4 (P1). Explorer `includeAmount` is not read, and trimmed `amount` is always present.**

- Expected: `amount` (= `individualData.length || 0`) appears only when `includeAmount=true|1` (explorer-handler.ts:190-191, 209-216; decentraland/lamb2:src/controllers/handlers/explorer-emotes-handler.ts:76-77, 95-97).
- Change: lambdas_explorer.rs:63-115, 475.

**LI-5 (P1). Explorer on-chain items include escrow grants from `shop.usage_grants`.**

- Upstream reports only chain-owned tokens (explorer-handler.ts:74-104).
- Decision needed: keep the extension behind an opt-in query param, or document it as an intended divergence.
- Change: services/catalyst/shop/src/market/lambdas_explorer.rs:156-175.

**LI-6 (P1). Explorer on-chain `itemType` is `""` when the `shop.items` mirror has no row.**

- Expected: the source `itemType` (`wearable_v1|wearable_v2|smart_wearable_v1`), absent if unknown (decentraland/lamb2:src/adapters/marketplace-api-fetcher.ts:168; decentraland/lamb2:src/logic/fetch-elements/fetch-items.ts:184).
- Change: read `itemType` from the subgraph `nfts` row (services/catalyst/chain-index/src/graph.rs:233-290) instead of `shop.items` (lambdas_explorer.rs:262-274, 422-427), and omit the key when it is unknown.

**LI-7 (P1). `/users/:address/{wearables,emotes}`: `category` and `rarity` filters are case-sensitive.**

- Expected: both values are lowercased before matching (decentraland/lamb2:src/controllers/handlers/utils.ts:12-19).
- Change: `filter_groups` call site at services/catalyst/shop/src/market/user_items.rs:263-268.

**LI-8 (P1). `/users/:address/{wearables,emotes}`: sorting is validated and applied when `orderBy` is absent.**

- Expected (utils.ts:45-48; fetch-items.ts:82-85):
  - Validation runs only when `orderBy` is present, so `direction=foo` alone is a 200.
  - Without `orderBy`, no in-memory sort is applied: the order is the source's (subgraph `id` asc grouping in the fallback).
- Change: user_items.rs:114-163, 247-249, 270-272.
- Note: if dcl-one keeps a default order, document that the marketplace-server's default order is not in the pinned trees.

**LI-9 (P0). Third-party ownership resolution differs from the NFT worker.**

- Expected: every token the wallet holds on each provider contract, on every `ContractNetwork` (`mainnet`, `sepolia`, `matic`, `amoy`), matched against mappings with `includesNft` (decentraland/lamb2:src/adapters/alchemy-nft-fetcher.ts:16-54; decentraland/lamb2:src/logic/fetch-elements/fetch-third-party-wearables.ts:80-139).
- dcl-one misses tokens in these cases:
  - `any` or wide-`range` (≥ 2,048) mappings on non-enumerable ERC-721;
  - enumerable holdings above 1,000;
  - `sepolia`/`amoy` contracts.
- dcl-one also silently reports nothing on any RPC error.
- Change: `owned_via_rpc`/`owned_on_network`/`chain_of` in services/catalyst/shop/src/market/third_party.rs:302-509. Either add an NFT-worker-compatible indexer path, or index Transfer logs for registry contracts so the owner's full token set is known.

**LI-10 (P1). Third-party routes: registry failure and unknown provider answer 200 instead of 502.**

- Expected:
  - `/users/:address/third-party-wearables` with the registry unreadable and no cached value: 502 `{"error":"The requested items cannot be fetched right now","message":"Cannot fetch elements for <address>"}`.
  - `/third-party-integrations`: 502 with `message":"Cannot fetch third party providers"`.
  - `/users/:address/third-party-wearables/:collectionId` and `/collections/*-by-owner/:owner?collectionId=` for a provider not in the registry: 502 with `"message":"Third Party not found: <name>"`.
  - Refs: decentraland/lamb2:src/logic/third-party-providers-storage.ts:46-52; fetch-third-party-wearables.ts:178-183; decentraland/lamb2:src/controllers/handlers/errorHandler.ts:27-35.
- Change: services/catalyst/shop/src/market/third_party.rs:166-184, 700-713, 1030-1053, 1056-1066. Make `third_party_providers` return `Result` and add a provider-exists check.

**LI-11 (P1). Third-party routes cap `pageSize` at 1000.**

- Expected: no maximum (`paginationObject(url, Number.MAX_VALUE)`, decentraland/lamb2:src/controllers/handlers/third-party-wearables-handler.ts:42, 100).
- Change: `pagination_object` in services/catalyst/shop/src/market/third_party.rs:870-882. Use `OversizePolicy::Clamp` with `i64::MAX`, or skip the check.

**LI-12 (P2). `/users/:address/third-party-wearables/:collectionId` sorts by name.**

- Expected: fetcher order, with no sort (third-party-wearables-handler.ts:102-105).
- Change: services/catalyst/shop/src/market/third_party.rs:953.

**LI-13 (P1). `includeDefinitions`/`includeEntities` emit `null` for a missing definition or entity.**

- Expected: the key is omitted (decentraland/lamb2:src/controllers/handlers/utils.ts:56-67; third-party-wearables-handler.ts:60-65).
- Change: services/catalyst/shop/src/market/user_items.rs:294-305; services/catalyst/shop/src/market/third_party.rs:984-990.

**LI-14 (P2). Definition mapping edge cases differ.**

- Expected (decentraland/lamb2:src/adapters/definitions.ts:4-102):
  - `thumbnail` is the content URL, or **absent** when the file has no hash; dcl-one keeps the file name.
  - Each representation's `contents[]` entry without a hash is `{"key":<file>}` with no `url`; dcl-one emits `"url":""`.
  - For a legacy emote, `tags` is `metadata.data.tags`, absent if undefined; dcl-one emits `[]`.
  - `loop` is `emoteDataV0.loop` when `emoteDataV0` exists, else `false`.
  - The base URL is `CONTENT_URL` **lowercased**, without a trailing `/`.
  - A wearable whose `metadata.data.representations` is missing yields no definition; dcl-one returns the metadata unchanged.
- Change: services/catalyst/shop/src/market/definitions.rs:53-160.

**LI-15 (P1). Reader failures answer 503 instead of 502.**

- Expected: a failed owned-items fetch is 502 `{"error":"The requested items cannot be fetched right now","message":"Cannot fetch elements for <address>"}` (decentraland/lamb2:src/adapters/elements-fetcher.ts:265-274).
- Change:
  - services/catalyst/shop/src/market/user_items.rs:255-261, 465-471;
  - services/catalyst/shop/src/market/lambdas_explorer.rs:582-585.
- The unconfigured-reader envelope (`readers::require_both_collections`) is a deployment condition and may stay.

**LI-16 (P2). Address validation on routes where upstream has none.**

- Upstream accepts any `:address` and answers 200: empty for `/users/*`, and base wearables only for the explorer.
- dcl-one answers 400 `Address must be a valid Ethereum address`:
  - services/catalyst/shop/src/market/user_items.rs:231-233;
  - services/catalyst/shop/src/market/third_party.rs:890-892, 934-936;
  - services/catalyst/shop/src/market/lambdas_explorer.rs:530-532.
- Decide whether this is an intended hardening. If it is not, answer 200 with an empty page, and with the base wearables for the explorer.

**LI-17 (P1). Owned-copies cap of 10 pages × 1,000 per network.**

- Expected: the full inventory. The upstream fallback walks every page (decentraland/lamb2:src/logic/fetch-elements/fetch-elements.ts:13-46).
- Change: `MAX_PAGES` in services/catalyst/chain-index/src/graph.rs:28-29, 318-342. Remove the cap, or raise it to match marketplace-api's 100 × 1,000 groups.
- Also return an error rather than a silently truncated `totalAmount`.

**LI-18 (P2). `individualData` order is newest transfer first.**

- Expected: source order; subgraph `id` ascending in the fallback (fetch-items.ts:157-195).
- Change: services/catalyst/chain-index/src/graph.rs:364-369; services/catalyst/shop/src/market/user_items.rs:61-94.

**LI-19 (P2). `/collections/*-by-owner/:owner` is sorted by rarity DESC then urn.**

- Expected: fetcher order, with no sort (decentraland/lamb2:src/logic/items-by-owner.ts:46-52).
- Change: services/catalyst/shop/src/market/user_items.rs:382-388, 495.

**LI-20 (P1). `/collections/{wearables,emotes}`: `pagination.lastId` is never emitted.**

- Expected: `pagination: {limit, lastId: <request lastId, lowercased> (omitted if none), next?}` (decentraland/lamb2:src/controllers/handlers/wearables-catalog-handler.ts:68-72; decentraland/lamb2:src/controllers/handlers/emotes-catalog-handler.ts:37-41).
- Change: `catalog_response` in services/catalyst/shop/src/market/lambdas_catalog.rs:852-872. Pass `query.last_id`.

**LI-21 (P1). `/collections/{wearables,emotes}`: `textSearch` uses `name ILIKE` on the mirror.**

- Expected: subgraph `items.searchText_contains: <textSearch>` (decentraland/lamb2:src/logic/fetch-elements/fetch-items-by-filters.ts:116-119).
- Change: services/catalyst/shop/src/market/lambdas_catalog.rs:467-470. Mirror the subgraph `searchText` column into `shop.items` (services/catalyst/shop/src/market/mirrors/items.rs) and match on it.

**LI-22 (P2). `/collections/wearables`: the extended filters change the upstream contract.**

- Expected: `collectionType`, `category`, `rarity`, `name`, `orderBy` and `direction` are ignored, and alone they do not satisfy "one of the filters" (decentraland/lamb2:src/logic/items-catalog.ts:14-43).
- dcl-one either rejects (400) or accepts requests that upstream treats the other way.
- Change: services/catalyst/shop/src/market/lambdas_catalog.rs:226-263, 343-351. Gate the extensions behind a separate path or opt-in.

**LI-23 (P2). Catalog ordering uses byte order instead of `localeCompare`.**

- Expected: `a.id.toLowerCase().localeCompare(b.id.toLowerCase())` for on-chain items, and the same on lowercased URNs for base items (decentraland/lamb2:src/logic/items-catalog.ts:52-54; wearables-catalog-handler.ts:107).
- Change: services/catalyst/shop/src/market/lambdas_catalog.rs:765-794, 827. Use `locale_cmp`.

**LI-24 (P2). Catalog `next` query encoding.**

- Expected: `URLSearchParams.toString()` (space as `+`, `*` kept unencoded, `~` as `%7E`) (items-catalog.ts:63-86).
- Change: `qp`/`build_next_query` in services/catalyst/shop/src/market/lambdas_catalog.rs:150-152, 384-410.

**LI-25 (P2). Mirror-backed answers carry `as_of` and `stale_after_secs` body keys.**

- Affects `/nfts/collections` and `/collections/{wearables,emotes}`.
- Expected: no such keys (decentraland/lamb2:src/controllers/handlers/all-collections-handler.ts:50-53).
- Change: services/catalyst/shop/src/market/mirrors/mod.rs:621-645. Use a response header instead of body keys on lambdas routes.

**LI-26 (P2). Pagination parsing.**

- Expected: `parseInt` semantics for `pageNum`, `pageSize` and `limit`: `"10abc"`→10 and `"abc"`→NaN, so an empty page with `null` echoes (decentraland/lamb2:src/logic/pagination.ts:4-19; items-catalog.ts:88-97).
- Expected: the explorer's negative offset slices from the end (pagination.ts:31-40).
- Change: services/catalyst/shared/src/query_params.rs:48-50, 130-176; services/catalyst/shop/src/market/lambdas_explorer.rs:573-579; services/catalyst/shop/src/market/lambdas_catalog.rs:323-328.

**LI-27 (P2). `/third-party-integrations` query uses `first: 1000`.**

- Expected: the provider cache list, default `first` of 100 (decentraland/lamb2:src/adapters/third-party-providers-graph-fetcher.ts:12-29).
- Expected: `name`/`description` passed through as the subgraph returns them (`null` stays `null`).
- Change: services/catalyst/shop/src/market/third_party.rs:1003-1026.

**LI-28 (P2). Explorer sort edges.**

- Expected (decentraland/lamb2:src/logic/sorting.ts:38-84):
  - `rarestOptional` counts any non-empty `rarity` as rated.
  - `newestOptional` and `oldestOptional` count an item as dated only when both min and max transfer are non-zero.
- Change: services/catalyst/shop/src/market/lambdas_explorer.rs:310-364, 220.

**LI-29 (P2). Explorer validation order.**

- Expected order: `pageSize` → `rarity` → sort → `collectionType` (decentraland/lamb2:src/controllers/handlers/explorer-handler.ts:174-195).
- Change: services/catalyst/shop/src/market/lambdas_explorer.rs:63-102.

**LI-30 (P2). `/users/:address/third-party-wearables/:collectionId` URN errors.**

- Expected: a parseable non-third-party-name URN gets `Invalid collection id: not a blockchain-collection-third-party-name nor blockchain-collection-third-party-collection URN`. The network segment is validated as `@dcl/urn-resolver` does (decentraland/lamb2:src/controllers/handlers/third-party-wearables-handler.ts:88-97).
- Change: services/catalyst/shop/src/market/third_party.rs:937-946; `is_third_party_name_urn` at services/catalyst/shop/src/market/user_items.rs:403-410.

**LI-31 (P2). Explorer emote `category` for v0 emotes is `""`.**

- Expected: the category of the source item (subgraph or marketplace), so a v0 emote still matches category filters.
- Change: services/catalyst/shop/src/market/lambdas_explorer.rs:293-299.

**LI-32 (P2). Global error shape (C5).**

- Expected: 500 `{"error":"Internal Server Error"}` without `message` (errorHandler.ts:66-76).
- Change: services/catalyst/shared/src/http_util.rs:30-36. Shared across lambdas; coordinate with the other lambdas areas.

## dcl-one extras in this area (no upstream counterpart)

- `GET /lambdas/collections`: alias of `/lambdas/nfts/collections` (services/catalyst/shop/src/market/mod.rs:934-937).
- `GET /explorer/{address}/wearables` and `GET /explorer/{address}/emotes` at the root, besides the `/lambdas` paths (mod.rs:960-967).
- `/collections/wearables` extended filters: `collectionType`, `category`, `rarity`, `name`, `orderBy`, `direction`, with their 400 messages and `filters` echo keys `collectionTypes`, `categories`, `rarity`, `name`, `orderBy`, `direction` (services/catalyst/shop/src/market/lambdas_catalog.rs:160-263).
- Explorer escrow grants (`shop.usage_grants`) merged into the on-chain leg (services/catalyst/shop/src/market/lambdas_explorer.rs:156-175).
- `as_of` and `stale_after_secs` body keys on mirror-backed answers (services/catalyst/shop/src/market/mirrors/mod.rs:621-645).
- The unavailable envelope (`source`, `configure`, `Retry-After: 60`) when a reader or mirror is not configured or not ready (services/catalyst/chain-index/src/source.rs:88-103).
- The marketplace-server routes in `shop/src/market/collections.rs` and `nft_reads.rs` (`/v1/collections`, `/v1/nfts`, …, under `/shop-api` and `/marketplace-api`) are outside the lambdas surface. They are not compared here.
