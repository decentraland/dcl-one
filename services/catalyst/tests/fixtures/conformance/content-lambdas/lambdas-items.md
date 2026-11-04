# Lambdas: wearables, emotes, third-party wearables and collections

Area: the lambdas item endpoints (owned wearables and emotes, third-party / linked wearables,
the explorer backpack, the collections list, and the legacy-lambdas ports under `/collections/*`).

Pinned revisions:

- Upstream lambdas: `decentraland/lamb2` @ `02a37f5d88fb895339c20e84c2470be68cdf3f57` (2026-10-21), served in production at `/lambdas/*`.
- Upstream content server, for the collection-prefix query only: `decentraland/catalyst` @ `a416565db1e705cff9523540a2578af642598a29`.
- dcl-one: `dcl-one-conformance` @ `37e91aed` (origin/main, 2026-11-04), workspace `services/catalyst`.

Method: everything below was read statically. Nothing was built, run or captured, so no row here is verified.

Files read (upstream lamb2):
`src/controllers/routes.ts`, `src/controllers/handlers/{wearables,emotes,third-party-wearables,explorer,explorer-emotes,all-collections,wearables-by-owner,emotes-by-owner,wearables-catalog,emotes-catalog}-handler.ts`, `src/controllers/handlers/{items-commons,pagination-utils,utils,errorHandler}.ts`, `src/logic/{pagination,pagination-constants,sorting,utils,api-with-fallback,items-by-owner,items-catalog,linked-wearables-mapper,third-party-providers-storage}.ts`, `src/logic/fetch-elements/{fetch-items,fetch-elements,graph-pagination,fetch-base-items,fetch-third-party-wearables,fetch-items-by-filters}.ts`, `src/adapters/{elements-fetcher,marketplace-api-fetcher,third-party-providers-graph-fetcher,alchemy-nft-fetcher,entities-fetcher,definitions,definitions-fetcher,content-server-url}.ts`, `src/components.ts`, `src/service.ts`, `src/types.ts`, `docs/openapi.yaml`.
Upstream catalyst: `src/adapters/pointers-repository/component.ts`, `src/controllers/routes.ts`.

Files read (dcl-one, under `services/catalyst/`):
`shop/src/market/{mod,user_items,lambdas_explorer,lambdas_catalog,third_party,base_wearables,definitions,readers}.rs`, `config/src/{shop,posture}.rs`, `shop/src/market/mirrors/mod.rs`, `chain-index/src/{graph,source}.rs`, `shared/src/{query_params,http_util,cors}.rs`, `content/src/queries.rs` (projection and prefix query), `src/serve.rs` (mounts and trailing-slash layer), the fixtures `shop/src/market/fixtures/{users-wearables-*,users-emotes-200-unknown-address,collections-wearables-by-owner-200-unknown}.json`.
`shop/src/market/nft_reads.rs` and `shop/src/market/collections.rs` serve the marketplace-server `/v1/*` routes, not the lambdas routes in this area. They are listed only under "dcl-one extras" and are not compared.

## Conventions that apply to every endpoint here

These are cross-cutting differences. Endpoint sections refer back to them as C1–C9 and do not repeat them.

| # | Topic | Upstream lamb2 | dcl-one | Verdict |
|---|---|---|---|---|
| C1 | 400 body | `{"error":"Bad request","message":<msg>}` (decentraland/lamb2:src/controllers/handlers/errorHandler.ts:7-15) | `{"error":"Bad request","message":<msg>}` (services/catalyst/shared/src/http_util.rs:10-16) | MATCH |
| C2 | 404 body | `{"error":"Not Found","message"}` (errorHandler.ts:37-45) | `{"error":"Not Found","message"}` (http_util.rs:18-20) | MATCH (no route in this area raises 404 upstream) |
| C3 | Fetch failure | A `FetcherError` returns **502** `{"error":"The requested items cannot be fetched right now","message":"Cannot fetch elements for <address>"}` (errorHandler.ts:27-35; decentraland/lamb2:src/adapters/elements-fetcher.ts:265-274) | The explorer, the third-party routes and the by-owner lists with `collectionId` answer the same 502 (services/catalyst/shop/src/market/third_party.rs:1645-1656, 980-989). `/users/:address/{wearables,emotes}` and the on-chain by-owner lists answer **503** `{"error":"Service Unavailable","message":"ownership readers unavailable"}` (http_util.rs:22-28). An unconfigured reader answers the unavailable envelope `{"error":"Service Unavailable","message","source","configure"?}` with `Retry-After: 60` (services/catalyst/chain-index/src/source.rs:88-103) | DIVERGENT on the `/users/*` and on-chain by-owner routes (LI-15) |
| C4 | Overload | The marketplace bulkhead returns **503** `{"error":"Service overloaded, retry shortly","message"}` (errorHandler.ts:17-25; decentraland/lamb2:src/adapters/marketplace-api-fetcher.ts:307-318) | No bulkhead, so there is no 503 of this kind | DIVERGENT (P2) |
| C5 | Unhandled error | **500** `{"error":"Internal Server Error"}`, with no `message` (errorHandler.ts:66-76) | **500** `{"error":"Internal Server Error","message":"Internal Server Error"}` (http_util.rs:30-36) | DIVERGENT (P2) |
| C6 | JSON key order | Insertion order (JS object) | `serde_json` without `preserve_order` (services/catalyst/Cargo.toml:21-23), so `json!` objects serialize with keys sorted alphabetically | DIVERGENT (P2, semantic JSON equal) |
| C7 | Query parsing | WHATWG `URLSearchParams`: `get` returns the first value, `getAll` all values, `has` tests presence. Integers come from `parseInt(x,10)`, so `"10abc"`→10, `"1.5"`→1 and `"abc"`→`NaN` | `parse_query_string` (services/catalyst/shared/src/query_params.rs:20-38) also folds `key[0]=` into `key` (line 227-237). Integers come from `str::parse::<i64>`, so `"10abc"`, `"1.5"` and `"abc"` all fall back to the default 100/1 (line 48-50, 136-137) | DIVERGENT (P2) |
| C8 | Trailing slash | `@dcl/http-server` router (path-to-regexp). Not determined from the pinned tree | `NormalizePath::trim_trailing_slash` (services/catalyst/src/serve.rs:1301-1304) | Not determined |
| C9 | CORS preflight max-age | `cors: { maxAge: 36000 }` (decentraland/lamb2:src/components.ts:65-67) | `Access-Control-Max-Age: 86400` (services/catalyst/shared/src/cors.rs:18) | DIVERGENT (P2; shared layer, also covered by the CORS area) |

Notes:

- Upstream `pagination-utils.ts` (`ensurePagination`, `validatePagination`) has no caller anywhere in `src/`. Its messages (`pageNum must be greater than 0`, …) are therefore never served. The live pagination is `paginationObject` (decentraland/lamb2:src/logic/pagination.ts:4-19). It has defaults `pageSize=100` and `pageNum=1` (decentraland/lamb2:src/logic/pagination-constants.ts:6-11), and a single check, `pageSize > max` → 400 `max allowed pageSize is <max>`. A non-positive or NaN `pageNum`/`pageSize` passes through.
- No route in this area has auth (signed fetch), rate limiting, or a 503 for bootstrapping or read-only mode, in either tree.
- dcl-one wraps the whole lambdas router in a request-timeout layer that answers **408** (services/catalyst/shop/src/market/mod.rs:986-991). Upstream has no such response.

## Ownership data: where each tree reads it

| Data | Upstream lamb2 | dcl-one | Equal for the same chain state? |
|---|---|---|---|
| On-chain wearables/emotes an address owns | **Primary:** marketplace-server `GET {MARKETPLACE_API_URL}/v1/users/<addr>/{wearables,emotes}/grouped` (decentraland/lamb2:src/adapters/marketplace-api-fetcher.ts:395-472). This is the marketplace's own indexed DB, and its order and total come from there. **Fallback**, on any `MarketplaceApiError`: the TheGraph collections subgraphs, L1 + L2 for wearables and L2 only for emotes. The fallback walks `nfts` by `id_gt`, with no page cap (decentraland/lamb2:src/logic/fetch-elements/fetch-items.ts:237-336; decentraland/lamb2:src/logic/fetch-elements/fetch-elements.ts:13-46). The marketplace is abandoned above 100 pages of 1,000 groups (marketplace-api-fetcher.ts:362-385), and the fallback then runs. | TheGraph collections subgraphs only, L1 + L2 for both categories, through `chain-index` (services/catalyst/chain-index/src/graph.rs:236-378). It has no chain index of its own for collection NFTs. It is capped at **10 pages × 1,000 tokens per network** (graph.rs:28-29, 323-347) and memoized for 30 s (graph.rs:34). The explorer adds rows from `shop.usage_grants` (escrow grants) (services/catalyst/shop/src/market/lambdas_explorer.rs:209-229). | **Same set of tokens** when the marketplace DB agrees with the subgraph and the owner holds ≤ 10,000 tokens per network. **Different** in four cases: above that cap (dcl-one truncates; upstream walks everything); grouping order and `individualData` order (upstream follows the marketplace order, or subgraph `id` ascending in the fallback; dcl-one goes newest transfer first); explorer items granted by escrow (dcl-one only); and subgraph/marketplace indexing lag. |
| Third-party registry (providers) | TheGraph third-party registry subgraph, `thirdParties(where:{isApproved:true})`, default `first` (100) (decentraland/lamb2:src/adapters/third-party-providers-graph-fetcher.ts:12-29). Cached 6 h; an error with no stale value raises `FetcherError` (decentraland/lamb2:src/logic/third-party-providers-storage.ts:33-52). | Same subgraph and the same query for providers (services/catalyst/shop/src/market/third_party.rs:146-162, 205-223), cached 6 h. An error gives an **empty list**. The integrations query asks `first: 1000` (third_party.rs:1351). | Same providers (≤ 100). Different on registry failure and when there are more than 100 approved providers (integrations list). |
| Linked-wearable ownership (v2 mappings) | NFT worker `POST {NFT_WORKER_BASE_URL}/wallets/<owner>/networks/<network>/nfts` with the provider contract list. It returns every `network:contract:tokenId` the wallet holds (decentraland/lamb2:src/adapters/alchemy-nft-fetcher.ts:16-54) for every `ContractNetwork` (`mainnet`, `sepolia`, `matic`, `amoy`). Mappings are matched with `createMappingsHelper.includesNft` (decentraland/lamb2:src/logic/linked-wearables-mapper.ts:6-21). There is no Merkle-proof or resolver path in this revision. | With `NFT_WORKER_BASE_URL` (set to the same worker under `NODE_POSTURE=mirror`, services/catalyst/config/src/posture.rs:277-281): the same `POST <base>/wallets/<owner>/networks/<network>/nfts` per `ContractNetwork`, only the providers whose contracts returned a token are read, and mappings are matched with `mappings_includes_nft` (services/catalyst/shop/src/market/third_party.rs:877-935, 1029-1047, 225-262). A non-2xx answer reports nothing for that network; a transport failure or an unreadable body fails the read (502). Without it: JSON-RPC over each network's endpoint (`TRADE_RPC_URLS`, else `ETH_RPC_URL` for the L1 and `RPC_ENDPOINT_POLYGON` for the L2 of `ETH_NETWORK`; third_party.rs:358-377): per contract `balanceOf` plus ERC-165 probes, the whole `tokenOfOwnerByIndex` walk of an enumerable ERC-721 up to 50,000 tokens (past that the read fails), else `ownerOf`/`balanceOfBatch` on the mapped ids, with a `range` expanded only up to 2,048 ids (third_party.rs:275-339, 468-553, 622-695). An `any` or wider `range` mapping on a plain ERC-721 or an ERC-1155 is logged as unresolved and left out (third_party.rs:513-535). A network with no endpoint reports nothing (third_party.rs:407-411). A failed contract read (transport, non-2xx, a JSON-RPC error other than a revert) fails the read (third_party.rs:757-870). No Merkle-proof path. | With `NFT_WORKER_BASE_URL`: **equal**, apart from upstream's stale serving. Without it: equal for `single`/`multiple`/small-`range` mappings and enumerable ERC-721; **different** for `any` or wide `range` mappings (more than 2,048 ids) on a plain ERC-721 or an ERC-1155, for enumerable holdings above 50,000 (502), and for networks with no RPC endpoint (nothing owned). |
| Third-party entities of a provider | Content server `GET /entities/active/collections/<providerUrn>?pageSize=1000&pageNum=n` (decentraland/lamb2:src/adapters/entities-fetcher.ts:62-80). That is `pointer LIKE '<prefix>%'` (decentraland/catalyst:src/adapters/pointers-repository/component.ts:14-27). | Local content DB, with the same `pointer LIKE '<prefix>%'` (services/catalyst/content/src/queries.rs:277-311), 1,000 per page, keeping only entities with `metadata.mappings` (third_party.rs:1158-1195). | Equal for the same synced content. |
| Definitions and entities (`includeDefinitions`, `includeEntities`, explorer `entity`) | Content server `POST /entities/active` by pointer (decentraland/lamb2:src/adapters/definitions-fetcher.ts), mapped by `extractWearableDefinitionFromEntity` / `extractEmoteDefinitionFromEntity` with `CONTENT_URL` (lowercased, trailing `/` stripped) + `/contents/<hash>` (decentraland/lamb2:src/adapters/definitions.ts:4-102). | Local content DB `active_entity_docs_by_pointers` (services/catalyst/content/src/queries.rs:233-238), mapped by `extract_*_definition` with `content_public_url` + `contents/<hash>` (services/catalyst/shop/src/market/definitions.rs:53-160). | Equal when the content DB is synced, apart from the edge mappings in LI-14. |
| Collections list | Live L1 + L2 subgraph `collections(first:1000, orderBy:urn)` (decentraland/lamb2:src/controllers/handlers/all-collections-handler.ts:7-36). | `shop.collections` mirror filled from the same subgraphs by a periodic job (services/catalyst/shop/src/market/mirrors/mod.rs:1-31). | Equal after a complete mirror pass; lags by up to the mirror interval (default 300 s). |
| Global catalog (`/collections/wearables`, `/collections/emotes`) | Live subgraph `items(where:{searchItemType_in, urn_gt, searchText_contains, urn_in}, orderBy:urn)`, L1 then L2 (decentraland/lamb2:src/logic/fetch-elements/fetch-items-by-filters.ts:41-141). | `shop.items` mirror once it has a complete pass, filtered with SQL (`name ILIKE` stands in for `searchText_contains`) (services/catalyst/shop/src/market/lambdas_catalog.rs:422-519). Before that, the live subgraphs with a different `where` document (lambdas_catalog.rs:592-758). | `collectionId` and `wearableId` filters give equal sets. `textSearch` can differ: `searchText` is not the item name. |

## Endpoint inventory

| Method | Upstream path | Production prefix | dcl-one route(s) | Status |
|---|---|---|---|---|
| GET | `/users/:address/wearables` | `/lambdas` | `/lambdas/users/{address}/wearables`, services/catalyst/shop/src/market/mod.rs:891-894 → `user_items::user_wearables` (services/catalyst/shop/src/market/user_items.rs:319-326) | DIVERGENT |
| GET | `/users/:address/emotes` | `/lambdas` | `/lambdas/users/{address}/emotes`, mod.rs:895-898 → `user_items::user_emotes` (user_items.rs:328-335) | DIVERGENT |
| GET | `/users/:address/third-party-wearables` | `/lambdas` | mod.rs:924-927 → `third_party::user_third_party_wearables` (services/catalyst/shop/src/market/third_party.rs:1224-1269) | DIVERGENT |
| GET | `/users/:address/third-party-wearables/:collectionId` | `/lambdas` | mod.rs:928-931 → `third_party::user_third_party_collection_wearables` (third_party.rs:1272-1304) | DIVERGENT |
| GET | `/third-party-integrations` | `/lambdas` | mod.rs:920-923 → `third_party::third_party_integrations` (third_party.rs:1378-1401) | DIVERGENT |
| GET | `/explorer/:address/wearables` | `/lambdas` | `/lambdas/explorer/{address}/wearables` and `/explorer/{address}/wearables`, mod.rs:959-970 → `lambdas_explorer::explorer_wearables` (services/catalyst/shop/src/market/lambdas_explorer.rs:790-804) | DIVERGENT |
| GET | `/explorer/:address/emotes` | `/lambdas` | `/lambdas/explorer/{address}/emotes` and `/explorer/{address}/emotes`, mod.rs:963-974 → `lambdas_explorer::explorer_emotes` (lambdas_explorer.rs:807-814) | DIVERGENT |
| GET | `/nfts/collections` | `/lambdas` | `/lambdas/nfts/collections` (+ `/lambdas/collections`), mod.rs:941-948 → `lambdas_catalog::nfts_collections` (services/catalyst/shop/src/market/lambdas_catalog.rs:1072-1097) | DIVERGENT |
| GET | `/collections/wearables-by-owner/:owner` | `/lambdas` | mod.rs:903-906 → `user_items::wearables_by_owner` (user_items.rs:527-543) | DIVERGENT |
| GET | `/collections/wearables` | `/lambdas` | mod.rs:949-952 → `lambdas_catalog::collections_wearables_catalog` (lambdas_catalog.rs:1017-1025) | DIVERGENT |
| GET | `/collections/emotes-by-owner/:owner` | `/lambdas` | mod.rs:907-910 → `user_items::emotes_by_owner` (user_items.rs:545-561) | DIVERGENT |
| GET | `/collections/emotes` | `/lambdas` | mod.rs:953-956 → `lambdas_catalog::collections_emotes_catalog` (lambdas_catalog.rs:1028-1047) | DIVERGENT |

Counts: 12 endpoints. MATCH 0, GAP 0, DIVERGENT 12, MISSING 0.

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

- `urn`. Upstream: as the marketplace or subgraph spells it. dcl-one: subgraph URN with `:mainnet:` rewritten to `:ethereum:` (services/catalyst/chain-index/src/graph.rs:158-164). The collections subgraphs already spell L1 as `ethereum`. MATCH.
- `amount`. Both: number, `individualData.length` (wearables-handler.ts:9; user_items.rs:180). MATCH.
- `individualData[].id`. Both: `<urn>:<tokenId>`. MATCH.
- `individualData[].transferredAt`. Both: string. Upstream passes the source value through, either the marketplace string or the subgraph BigInt string. dcl-one parses to `i64` and back (services/catalyst/chain-index/src/graph.rs:252-259, 279), so a non-numeric or null value becomes `"0"`. MATCH for normal data.
- `individualData[].price`. Both: string. dcl-one uses `"0"` when the subgraph `item` is null (graph.rs:280-283). The upstream fallback throws on a null `item` (fetch-items.ts:170), which gives 502. Edge.
- `individualData` order. Upstream: marketplace order, or subgraph `id` ascending in the fallback (fetch-items.ts:157-195). dcl-one: newest transfer first, ties by `tokenId` then `price` (graph.rs:369-374). DIVERGENT (P2).
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
| Source networks | Marketplace emotes; fallback **L2 only** (fetch-items.ts:262-275). | L1 + L2 subgraphs, `category: "emote"` (services/catalyst/chain-index/src/graph.rs:236-247, 363-368). L1 has no emote NFTs. | MATCH in result |
| Category filter | `searchEmoteCategory` enum, lowercased (graph-pagination.ts:23-25). | Case-sensitive equality with `metadata.emote.category` (user_items.rs:102-104). | DIVERGENT (case) |
| `definition` | `extractEmoteDefinitionFromEntity` (definitions.ts:61-102). | `extract_emote_definition` (services/catalyst/shop/src/market/definitions.rs:125-160). | See LI-14 |
| Reader requirement | — | Both collections subgraphs required (user_items.rs:251-253), even though emotes live on L2 only. | DIVERGENT (dcl-one returns 503 if only L1 is missing) |

Example success: as in §1, with `"category":"dance"` and `"rarity":"rare"`.

Data dependency: as in §1. Emote entities are needed for `includeDefinitions`/`includeEntities`.

---

## 3. GET /users/:address/third-party-wearables

### Request

| Param | Upstream (decentraland/lamb2:src/controllers/handlers/third-party-wearables-handler.ts:33-74) | dcl-one (services/catalyst/shop/src/market/third_party.rs:1224-1269) | Verdict |
|---|---|---|---|
| `:address` | Not validated. Lowercased by the elements fetcher (elements-fetcher.ts:198). | 400 `Address must be a valid Ethereum address` unless valid (third_party.rs:1229-1231). | DIVERGENT |
| `pageSize` | Default 100, **max `Number.MAX_VALUE`**, so never rejected (line 42). | Max 1000, and > 1000 gives 400 `max allowed pageSize is 1000` (third_party.rs:1209-1221). | DIVERGENT (P1) |
| `pageNum` | Default 1, pass-through. `fetchAndPaginate` uses a JS `slice` (decentraland/lamb2:src/logic/pagination.ts:25-41). | Default 1, pass-through, JS-style slice (third_party.rs:1312-1326). | MATCH, except C7 |
| `category` | `getAll`, **case-sensitive**, matches `entity.metadata.data.category` (lines 13-26). | `qs_get_array`, case-sensitive (third_party.rs:1197-1207, 1236). | MATCH |
| `name` | First value, case-insensitive substring (line 21). | Same (third_party.rs:1201-1205). | MATCH |
| `orderBy`/`direction` | `createBaseSorting`: default `name`. Only `name ASC|DESC` is allowed; anything else gives 400 `Invalid sorting requested: <sort> <DIR>` (sorting.ts:131-146). nameAZ = `name.localeCompare || urn.localeCompare`; nameZA is the full reverse. | Same default, the same allowed set, the same message, and the same comparator (UCA collator) (third_party.rs:1238-1266). | MATCH |
| `includeDefinitions` | Presence flag (line 41). | Presence flag (third_party.rs:1267). | MATCH |

Validation order. Upstream: pageSize → sort → fetch. dcl-one: address → pageSize → sort.

### Responses

| Trigger | Status | Body shape | Upstream | dcl-one | Verdict |
|---|---|---|---|---|---|
| Success | 200 | `{"elements":[TPW],"totalAmount":number,"pageNum":number,"pageSize":number}`. `TPW = {"urn":string,"individualData":[{"id":"<urn>:<network>:<contract>:<tokenId>","tokenId":"<network>:<contract>:<tokenId>"}],"amount":number,"name":string,"category":string,"entity":Entity,"definition"?:WearableDefinition}`. | third-party-wearables-handler.ts:46-73; decentraland/lamb2:src/logic/fetch-elements/fetch-third-party-wearables.ts:33-58 | third_party.rs:947-957, 1306-1349 | Shape MATCH. Key order C6. Data source: MATCH with `NFT_WORKER_BASE_URL`; without it, the chain-path differences of LI-9. |
| `includeDefinitions`, definition missing | 200 | Upstream `definitions[i]!` is `undefined`, so the key is omitted (lines 60-65). dcl-one: `"definition": null` (third_party.rs:1332-1338). | | | DIVERGENT (P1) |
| `pageSize` > 1000 | upstream 200 / dcl-one 400 | — | line 42 | third_party.rs:1213-1219 | DIVERGENT |
| Bad sort | 400 | C1, `Invalid sorting requested: date DESC` etc. | sorting.ts:144 | third_party.rs:1250-1252 | MATCH |
| Invalid address | upstream 200 (empty) / dcl-one 400 | — | — | third_party.rs:1229-1231 | DIVERGENT |
| Registry subgraph down, no cached value | upstream 502 `{"error":"The requested items cannot be fetched right now","message":"Cannot fetch elements for <address>"}` / dcl-one 200 with an empty page (the provider list reads as empty) | — | third-party-providers-storage.ts:46-52; elements-fetcher.ts:272-273 | third_party.rs:205-223 | DIVERGENT (P1, LI-10) |
| NFT worker answers non-2xx | 200, nothing owned for that network (both) | — | alchemy-nft-fetcher.ts:35-39 | third_party.rs:906-909 | MATCH |
| NFT worker request throws (network error, timeout, unreadable body), no stale entry | 502 `{"error":"The requested items cannot be fetched right now","message":"Cannot fetch elements for <address>"}` (both) | — | alchemy-nft-fetcher.ts:26-41; elements-fetcher.ts:265-274; errorHandler.ts:27-35 | third_party.rs:897-914, 1060-1063, 1645-1656 | MATCH (upstream may serve a stale entry first) |
| Without `NFT_WORKER_BASE_URL`: a provider contract's RPC read fails | dcl-one only: the same 502; the failure is remembered 5 s | — | — | third_party.rs:63, 425-436, 1060-1063 | dcl-one only (no upstream RPC path) |

Example success:

```json
{"elements":[{"urn":"urn:decentraland:matic:collections-thirdparty:example:collection1:hat","individualData":[{"id":"urn:decentraland:matic:collections-thirdparty:example:collection1:hat:matic:0x00000000000000000000000000000000000000c2:7","tokenId":"matic:0x00000000000000000000000000000000000000c2:7"}],"amount":1,"name":"Linked Hat","category":"hat","entity":{"version":"v3","id":"bafkreiexample","type":"wearable","pointers":["urn:decentraland:matic:collections-thirdparty:example:collection1:hat"],"timestamp":1700000000000,"content":[],"metadata":{}}}],"totalAmount":1,"pageNum":1,"pageSize":100}
```

Upstream merges duplicates by URN, last one winning (fetch-third-party-wearables.ts:144-153). dcl-one keeps the first entity per URN and adds every matching NFT to it (third_party.rs:1076-1126). For the same entity set, `individualData` is the same.

### Data dependency

- Upstream: the third-party registry subgraph, the NFT worker, and the content server's collection prefix query.
- dcl-one: the registry subgraph (`THIRD_PARTY_REGISTRY_L2_SUBGRAPH_URL`), synced third-party entities in its content DB, and either the NFT worker (`NFT_WORKER_BASE_URL`, set under `NODE_POSTURE=mirror`) or the JSON-RPC endpoints of each `ContractNetwork` (`TRADE_RPC_URLS`, `ETH_RPC_URL`, `RPC_ENDPOINT_POLYGON`). On the RPC path, a network with no endpoint is logged and reports nothing owned (third_party.rs:407-411).

---

## 4. GET /users/:address/third-party-wearables/:collectionId

### Request

| Param | Upstream (third-party-wearables-handler.ts:76-128) | dcl-one (third_party.rs:1272-1304) | Verdict |
|---|---|---|---|
| `:address` | Not validated. | 400 `Address must be a valid Ethereum address`. | DIVERGENT |
| `:collectionId` | Cut to the first 5 `:` parts, then parsed with `@dcl/urn-resolver` (which validates the network segment). Unparseable gives 400 `Invalid collection id: <collectionId> not a valid URN`. Parseable but not `blockchain-collection-third-party-name` gives 400 `Invalid collection id: not a blockchain-collection-third-party-name nor blockchain-collection-third-party-collection URN` (lines 85-97). | Cut to 5 parts. The structural check is `urn:decentraland:<any>:collections-thirdparty:<non-empty>` (services/catalyst/shop/src/market/user_items.rs:403-410). Any failure gives 400 `Invalid collection id: <collectionId> not a valid URN` (third_party.rs:1280-1289). | DIVERGENT: the second message is never produced, and the network segment is not validated (P2) |
| `pageSize` | Default 100, max `Number.MAX_VALUE`. | Max 1000, which gives a 400. | DIVERGENT (P1) |
| `pageNum` | Pass-through. | Pass-through. | MATCH |
| `includeDefinitions` | Presence. | Presence. | MATCH |
| `category`, `name`, `orderBy` | Not read. No filter and **no sort**: elements come in fetcher order (entity insertion order, lines 102-105). | Not read for filtering, but **always sorted by name then urn** (third_party.rs:1301). | DIVERGENT in order (P2) |

Upstream filtering: it takes the owner's full third-party list and keeps the wearables whose URN parses to `blockchain-collection-third-party` and whose `thirdPartyName` equals the requested one (fetch-third-party-wearables.ts:156-176). dcl-one fetches only the providers whose id has that name at part 4 (third_party.rs:1010-1011). The resulting set is the same.

### Responses

| Trigger | Status | Body | Upstream | dcl-one | Verdict |
|---|---|---|---|---|---|
| Success | 200 | `{"elements":[TPW],"totalAmount","pageNum","pageSize"}` as in §3 | lines 102-127 | third_party.rs:952-955 | DIVERGENT (order; data source as in §3) |
| Provider not in the registry | 502 `{"error":"The requested items cannot be fetched right now","message":"Third Party not found: <name>"}` (both, when dcl-one has a registry URL; without one, 200 with an empty page) | — | fetch-third-party-wearables.ts:178-183; errorHandler.ts:27-35 | third_party.rs:1010-1014, 980-989 | MATCH |
| Bad URN | 400 | C1, messages as in Request | lines 88-97 | third_party.rs:1285-1289 | DIVERGENT (second message) |
| `includeDefinitions`, missing definition | 200 | Upstream `definition: undefined` (omitted). dcl-one `null`. | lines 107-120 | third_party.rs:1332-1338 | DIVERGENT (P1) |
| Registry down | upstream 502 `Cannot fetch elements for <address>` / dcl-one 502 `Third Party not found: <name>` (the provider list reads as empty) | — | third-party-providers-storage.ts:46-52; elements-fetcher.ts:265-274 | third_party.rs:219-222, 1010-1014 | DIVERGENT (P2: `message`; LI-10) |

Data dependency: as in §3.

---

## 5. GET /third-party-integrations

### Request

No parameters, no auth.

### Responses

| Trigger | Status | Body | Upstream | dcl-one | Verdict |
|---|---|---|---|---|---|
| Success | 200 | `{"data":[{"name":string,"description":string,"urn":string}]}` in subgraph order. Upstream reads the provider cache (`thirdParties(where:{isApproved:true})`, default `first`, so at most 100). dcl-one runs its own query with `first: 1000`. | third-party-wearables-handler.ts:143-156; third-party-providers-graph-fetcher.ts:12-29 | third_party.rs:1351-1401 | DIVERGENT when there are more than 100 approved providers (P2) |
| `name`/`description` null in the subgraph | 200 | Upstream: `null`, or the key omitted if undefined. dcl-one: `""` (third_party.rs:1363-1370). | | | DIVERGENT (P2) |
| Registry fails, no cached value | upstream 502 `{"error":"The requested items cannot be fetched right now","message":"Cannot fetch third party providers"}` / dcl-one 200 `{"data":[]}` | — | third-party-providers-storage.ts:51 | third_party.rs:1396-1399 | DIVERGENT (P1) |
| Registry URL not configured | dcl-one only: 200 `{"data":[]}` | — | — | third_party.rs:1379-1384 | dcl-one only |
| Contracts | Upstream lowercases `contracts[].network/address` (decentraland/lamb2:src/logic/utils.ts:73-82). They are not part of this response. | n/a | | | n/a |

Example: `{"data":[{"name":"Example Provider","description":"Linked wearables","urn":"urn:decentraland:matic:collections-thirdparty:example"}]}`

Data dependency: the third-party registry subgraph, in both trees. The cache is 6 h in both.

---

## 6. GET /explorer/:address/wearables

### Request

| Param | Upstream (decentraland/lamb2:src/controllers/handlers/explorer-handler.ts:162-245) | dcl-one (services/catalyst/shop/src/market/lambdas_explorer.rs:74-141, 653-787) | Verdict |
|---|---|---|---|
| `:address` | Not validated. Base wearables are still returned for any string. | 400 `Address must be a valid Ethereum address` (lambdas_explorer.rs:660-662). | DIVERGENT (P2) |
| `pageNum`/`pageSize` | `paginationObject(url)` with max 1000. `fetchAndPaginate` uses a JS `slice`, so a negative offset counts from the end (pagination.ts:25-41). | Same max and message, but the slice clamps a negative offset to 0, with `skip/take` (lambdas_explorer.rs:707-713). `pageNum=-1` gives page 1 instead of the tail. | DIVERGENT (P2) |
| `category` | `getAll`, lowercased, any-of (decentraland/lamb2:src/controllers/handlers/items-commons.ts:12-14, 29-31). | `qs_get_array`, lowercased (lambdas_explorer.rs:104-107). | MATCH |
| `name` | First value, lowercased, substring; an item without a name is excluded (items-commons.ts:15, 26-28). | Same (lambdas_explorer.rs:103, 431-435). | MATCH |
| `rarity` | First value, lowercased, validated against `SORTED_RARITIES`: 400 `Invalid rarity requested: '<rarity>'.` Items without a rarity (base, third-party) are excluded (items-commons.ts:16-24). | Same message and rule (lambdas_explorer.rs:108-111, 426-430; services/catalyst/shop/src/market/definitions.rs:19-25). | MATCH |
| `orderBy`/`direction` | `createCombinedSorting`: default `rarity` (DESC). An invalid value gives 400 `Invalid sorting requested: '<sort> <DIR>'. Valid options are '[rarity, name, date] [ASC, DESC]'.` (sorting.ts:148-167). | Same default and message (definitions.rs:31-43; lambdas_explorer.rs:112-119). | MATCH |
| `collectionType` | `getAll`, default all three. Any value outside the set gives 400 `Invalid collection type. Valid types are: base-wearable, on-chain, third-party.` (lines 23, 177-195). | Same (lambdas_explorer.rs:80-89). | MATCH |
| `thirdPartyCollectionId` | `getAll`, deduplicated as strings. When present, the third-party leg holds only those providers' wearables (5-part cut; invalid ids dropped; a name no approved provider has is a 502 `Third Party not found: <name>`). Two distinct values naming one provider list its wearables twice. No count limit (lines 106-143, 180-182). | `qs_get_array`, each value cut to 5 parts and kept when it is a `collections-thirdparty` name URN (structural check, the network segment is not validated); an unknown name is the same 502. Values are deduplicated **by provider name**, so a provider is listed once. More than 100 values give 400 `Too many thirdPartyCollectionId values: at most 100.` (lambdas_explorer.rs:43, 97-102, 623-651; third_party.rs:1010-1014). | DIVERGENT (P2: duplicate listing, the 100-value cap) |
| `trimmed` | `true` or `1` (lines 183-184). | Same (lambdas_explorer.rs:120, 135). | MATCH |
| `isSmartWearable` | `true` or `1`: the on-chain leg asks `itemType=smartWearable` (only `smart_wearable_v1`), and **base and third-party are skipped** (lines 75-79, 145-157, 185-186). | Same: the owned copies are kept by their item type (the subgraph `itemType`, else the `shop.items` mirror), and the base and third-party legs are skipped (lambdas_explorer.rs:137, 148-158, 734-756). | MATCH |
| `network` | `ETHEREUM` or `MATIC` (exact case; anything else is ignored). The on-chain leg is limited to that network's item types (`wearable_v1` for ETHEREUM; `wearable_v2`+`smart_wearable_v1` for MATIC), and base and third-party are skipped (lines 187-189; marketplace-api-fetcher.ts:282-297; graph-pagination.ts:35-56). | Same values, item types and skipped legs (lambdas_explorer.rs:121-125, 148-158, 734-756). An owned copy whose item type neither source knows is dropped by either filter. | MATCH |
| `includeAmount` | `true` or `1`: trimmed elements carry `amount`. Without it, `amount` is absent (lines 190-191, 209-216). | Same, `amount = individualData.length` (lambdas_explorer.rs:136, 569-571). | MATCH |

Validation order. Upstream: pageSize → rarity → sort → collectionType. dcl-one: address → collectionType → pageSize → `thirdPartyCollectionId` count → rarity → sort (lambdas_explorer.rs:660-662, 80-119). A request with several errors gets a different message (P2).

### Responses

| Trigger | Status | Body | Upstream | dcl-one | Verdict |
|---|---|---|---|---|---|
| Success, not trimmed | 200 | `{"elements":[Mixed],"totalAmount","pageNum","pageSize"}`. Base: `{"type":"base-wearable","urn","name","category","individualData":[{"id":urn}],"amount":1,"entity"}`. On-chain: `{"type":"on-chain","urn","amount","individualData":[{"id","tokenId","transferredAt","price"}],"name","rarity","itemType","category","entity"}`, with `min/maxTransferredAt` removed. Third-party: `{"type":"third-party","urn","individualData":[{"id","tokenId"}],"amount","name","category","entity"}`. | lines 37-160, 226-243; decentraland/lamb2:src/logic/fetch-elements/fetch-base-items.ts:292-313 | lambdas_explorer.rs:444-490, 575-651 | DIVERGENT, see list |
| Success, trimmed | 200 | Upstream elements: `{"entity":{"id":entity.id,"thumbnail":<hash>|absent,"individualData":[...],"metadata":{"id","name","rarity"?,"isSmart":bool,"data":{"category","representations":[{"bodyShapes":[...]}]}}},"amount"?:number}` (decentraland/lamb2:src/logic/utils.ts:84-114) | lines 208-225 | `item_to_trimmed_value` (lambdas_explorer.rs:497-573): the same keys, each left out when the entity has none, `isSmart` from the item type | MATCH (field 1) |
| Invalid rarity / sort / collectionType / pageSize | 400 | C1 with the messages above | items-commons.ts:18-20; sorting.ts:164-166; explorer-handler.ts:193-195; pagination.ts:12-14 | lambdas_explorer.rs:85-119 | MATCH (messages) |
| Invalid address | upstream 200 / dcl-one 400 | — | — | lambdas_explorer.rs:660-662 | DIVERGENT |
| Owned fetch fails | 502 `{"error":"The requested items cannot be fetched right now","message":"Cannot fetch elements for <address>"}` (both) | — | elements-fetcher.ts:272-273 | lambdas_explorer.rs:716-719, 770 | MATCH |
| Collections subgraph not configured and on-chain requested | dcl-one 503 envelope | — | — | lambdas_explorer.rs:667-671 | dcl-one only |
| Third-party registry fails | upstream 502 / dcl-one: the third-party leg is empty and the rest is served; with `thirdPartyCollectionId`, 502 `Third Party not found: <name>` | — | third-party-providers-storage.ts:51 | third_party.rs:219-222, 1010-1014 | DIVERGENT (LI-10) |
| A provider's holdings cannot be read (NFT worker transport failure, or an RPC failure on the chain path) | upstream 502 (C3) / dcl-one: that contract's or network's items are left out (logged) and the rest is served | — | elements-fetcher.ts:265-274 | lambdas_explorer.rs:647; third_party.rs:961-966, 1060-1069 | DIVERGENT (P2, LI-9) |

Field-level differences:

1. **Trimmed entity shape.**
   - Both trees build a minimal object: `id`, `thumbnail` (the content hash of the `metadata.thumbnail` file, absent if not found), `individualData`, and `metadata` with `id`, `name`, `rarity`, `isSmart` (wearables only, `itemType == smart_wearable_v1`) and `data.category` plus `data.representations[].bodyShapes`. Keys the entity lacks are left out in both.
   - Wearable `metadata.name` is the item's grouped name in both; dcl-one's grouped name comes from the entity (field 5). Emote `name` is `metadata.name` and `data` comes from `emoteDataADR74` in both.
   - Sources: utils.ts:84-145; lambdas_explorer.rs:497-573.
   - Verdict: MATCH.
2. **`amount` in trimmed.** Present only with `includeAmount`, as `individualData.length` (`|| 0` upstream), in both. MATCH.
3. **On-chain `itemType`.** Upstream: the marketplace `itemType`, or the subgraph `itemType` in the fallback, absent if missing. dcl-one: the subgraph `itemType` of the owned copies, else the `shop.items` mirror, and `""` when neither knows it (services/catalyst/chain-index/src/graph.rs:242, 289; lambdas_explorer.rs:197-204, 313-315, 321-333, 481-486). P2.
4. **On-chain `amount`.** Upstream: the marketplace `amount` (`toFiniteNumber(amount, individualData.length)`, marketplace-api-fetcher.ts:160). dcl-one: the count of copies. MATCH for consistent data.
5. **On-chain `name`/`category`/`rarity`.** Upstream: marketplace or subgraph item metadata. dcl-one: `name`/`category` come from the content entity metadata (with an `i18n[0].text` fallback), and `rarity` from the subgraph item (lambdas_explorer.rs:335-367). Equal when the entity and the subgraph agree.
6. **Extra on-chain rows.** dcl-one adds active escrow grants from `shop.usage_grants` as owned copies, with `price: ""` and `transferredAt` = grant time (lambdas_explorer.rs:209-229). Upstream has no such source. DIVERGENT (P1, a dcl-one extension).
7. **Item without an entity.** Dropped in both (explorer-handler.ts:61-71, 93-103; lambdas_explorer.rs:305-307). MATCH.
8. **Rarity sort.**
   - Upstream `rarestOptional` treats any non-empty `rarity` string as "has rarity" (rank -1 when unknown) (sorting.ts:38-56).
   - dcl-one requires the rank to be ≥ 0 (lambdas_explorer.rs:376-377), so an unknown non-empty rarity sorts with the unrated items.
   - Edge, P2.
9. **Date sort.**
   - Upstream `hasDate` needs both `maxTransferredAt` and `minTransferredAt` to be non-zero (sorting.ts:58-60).
   - dcl-one marks every on-chain item as dated (lambdas_explorer.rs:277).
   - Edge, P2.
10. **Base wearables.**
    - The pointer list is identical: 282 URNs, compared with `diff` (decentraland/lamb2:src/logic/fetch-elements/fetch-base-items.ts:6-290; services/catalyst/shop/src/market/base_wearables.rs:13-296).
    - `urn = metadata.id`, `name = metadata.name || urn part 4`, `individualData:[{id}]` and `amount:1` match in both (base_wearables.rs:351-382; lambdas_explorer.rs:575-595).
    - Base wearables carry no `rarity`, so they are excluded by `rarity` filters in both.
    - MATCH.
11. **Caching.** Upstream elements are cached 60 s, plus 60 s stale-while-revalidate. dcl-one caches the computed list for 30 s (lambdas_explorer.rs:39). Not a wire difference.

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

Upstream: decentraland/lamb2:src/controllers/handlers/explorer-emotes-handler.ts:64-123. dcl-one: services/catalyst/shop/src/market/lambdas_explorer.rs:807-814, sharing the code of §6 with `valid_collection_types = ["on-chain"]`.

| Param | Upstream | dcl-one | Verdict |
|---|---|---|---|
| `:address` | Not validated | 400 if invalid | DIVERGENT |
| `pageNum`/`pageSize` | As in §6 | As in §6 | DIVERGENT for a negative `pageNum` (P2) |
| `category`/`name`/`rarity`/`orderBy`/`direction` | `createFilters`/`createCombinedSorting`, as in §6 | As in §6 | MATCH |
| `collectionType` | Only `on-chain`; otherwise 400 `Invalid collection type. Valid types are: on-chain.` (lines 16, 79-81) | Same (lambdas_explorer.rs:813) | MATCH |
| `trimmed` | `true`/`1` | Same | MATCH |
| `includeAmount` | `true`/`1` (lines 76-77, 95-97) | Same (lambdas_explorer.rs:136, 569-571) | MATCH |

### Responses

| Trigger | Status | Body | Upstream | dcl-one | Verdict |
|---|---|---|---|---|---|
| Success, not trimmed | 200 | `{"elements":[{"type":"on-chain","urn","amount","individualData","name","rarity","category","entity"}],...}`. Upstream also includes `itemType` when the source provides it (the subgraph fallback sets `emote_v1`; the marketplace mapping has none) | lines 108-121 | lambdas_explorer.rs:472-489 (`itemType` omitted for emotes, line 775) | DIVERGENT (P2) only in the fallback case |
| Success, trimmed | 200 | Upstream: `{"entity":{"id","thumbnail"?,"individualData","metadata":{"id","name","rarity","data":{"category":emoteDataADR74.category,"representations":[{"bodyShapes"}]}}},"amount"?}` (decentraland/lamb2:src/logic/utils.ts:116-145). dcl-one: the same object (lambdas_explorer.rs:497-573, `isSmart` left out for emotes) | lines 90-107 | lambdas_explorer.rs:497-573 | MATCH |
| Errors | as in §6 | | | | as in §6 |

Field detail: an emote's `category` comes from the subgraph or the marketplace upstream. dcl-one takes it from `metadata.emoteDataADR74.category`, falling back to `emoteDataV0.category`, which v0 emotes do not carry, so the result is `""` (lambdas_explorer.rs:352-358). A v0 emote then fails every `category` filter in dcl-one. P2.

Data dependency: emote entities in the content DB, both collections subgraphs, and `shop.usage_grants` (`category = 'emote'`).

---

## 8. GET /nfts/collections

### Request

No parameters.

### Responses

| Trigger | Status | Body | Upstream | dcl-one | Verdict |
|---|---|---|---|---|---|
| Success | 200 | `{"collections":[{"id":"urn:decentraland:off-chain:base-avatars","name":"Base Wearables"},{"id":"urn:decentraland:off-chain:base-emotes","name":"Base Emotes"},...L1 (≤1000, urn asc), ...L2 (≤1000, urn asc)]}` | decentraland/lamb2:src/controllers/handlers/all-collections-handler.ts:15-54 | services/catalyst/shop/src/market/lambdas_catalog.rs:1049-1097 | DIVERGENT: dcl-one adds `"as_of"` and `"stale_after_secs"` as the first keys (services/catalyst/shop/src/market/mirrors/mod.rs:621-645), P2 |
| Ordering | | Subgraph `orderBy: urn` versus mirror `ORDER BY urn COLLATE "C"` per network, L1 then L2 | all-collections-handler.ts:9, 34-36 | lambdas_catalog.rs:1058-1067 | MATCH, assuming the subgraph orders strings in byte order (not determinable from the trees) |
| Subgraph error | upstream 500 `{"error":"Internal Server Error"}` (C5) | | errorHandler.ts:66-76 | | DIVERGENT, see next row |
| Mirror without a complete pass | dcl-one 503 envelope (`Retry-After: 60`) | | — | lambdas_catalog.rs:1073-1075 | dcl-one only |
| Mirror query error | dcl-one 500 `{"error":"Internal Server Error","message":"Internal Server Error"}` | | | lambdas_catalog.rs:1092-1095 | C5 |
| Collection `name` null | upstream `"name": null`; dcl-one decodes `(String, String)`, so a NULL name fails the query and gives 500 | | | lambdas_catalog.rs:1058-1067 | DIVERGENT (P2, edge) |

Data dependency: dcl-one needs the `shop.collections` mirror (job `collections-mirror`) with a complete pass on both networks. Upstream reads live.

---

## 9. GET /collections/wearables-by-owner/:owner (legacy-lambdas port; verbatim wire contract)

### Request

| Param | Upstream (decentraland/lamb2:src/controllers/handlers/wearables-by-owner-handler.ts:12-34; decentraland/lamb2:src/logic/items-by-owner.ts:24-66) | dcl-one (services/catalyst/shop/src/market/user_items.rs:424-543) | Verdict |
|---|---|---|---|
| `:owner` | Not validated | Not validated; lowercased for the subgraph | MATCH |
| `includeDefinitions` | Presence only; `includeDefinitions=false` still enables it (items-by-owner.ts:29-31) | Raw query scan for `includeDefinitions` or `includeDefinitions=` (user_items.rs:390-396). A percent-encoded key (`include%44efinitions`) is not recognized | MATCH (P2 edge) |
| `collectionId` | `get` (first value). An empty string means on-chain. Otherwise the 5-part cut, which must parse as `blockchain-collection-third-party-name`, else 400 `'collectionId' must be a valid third-party collection URN` (items-by-owner.ts:32-36, 54-63) | First value; empty gives `None`. Structural check, same message (user_items.rs:398-422, 432-436) | MATCH (the network segment is not validated in dcl-one, P2) |
| Pagination | None. A flat array of every item | None | MATCH |

### Responses

| Trigger | Status | Body | Upstream | dcl-one | Verdict |
|---|---|---|---|---|---|
| Success, on-chain | 200 | `[{"urn":string,"amount":number,"definition"?:WearableDefinition}]`, with `amount = individualData.length` | items-by-owner.ts:38-52 | user_items.rs:477-525 | DIVERGENT in order: upstream follows the fetcher (marketplace default, or subgraph `id` asc grouping in the fallback; not sorted); dcl-one uses rarity DESC then `localeCompare(urn)` (user_items.rs:382-388, 498). P2 |
| `definition` missing | 200 | Upstream `definition: undefined`, omitted (line 43). dcl-one omits the key (user_items.rs:515-520) | | | MATCH |
| Success, third-party `collectionId` | 200 | `[{"urn","amount","definition"?}]` in fetcher order | items-by-owner.ts:64-65 | user_items.rs:432-453 (the `elements_from` insertion order) | MATCH (order equal for the same entity order) |
| Third-party provider unknown | 502 `{"error":"The requested items cannot be fetched right now","message":"Third Party not found: <name>"}` (both, when dcl-one has a registry URL) | | fetch-third-party-wearables.ts:178-183 | user_items.rs:438-441; third_party.rs:1404-1416, 1010-1014 | MATCH |
| Third-party holdings cannot be read | 502 `Cannot fetch elements for <owner>` (both) | | elements-fetcher.ts:265-274 | user_items.rs:438-441; third_party.rs:1060-1063 | MATCH |
| Bad `collectionId` | 400 | C1, `'collectionId' must be a valid third-party collection URN` | items-by-owner.ts:61-63 | user_items.rs:434-436 | MATCH |
| Owned fetch fails | upstream 502 (C3) / dcl-one 503 `ownership readers unavailable` | | elements-fetcher.ts:272-273 | user_items.rs:468-474 | DIVERGENT |
| Subgraph not configured | dcl-one 503 envelope | | | user_items.rs:454-456 | dcl-one only |

Example: `[{"urn":"urn:decentraland:matic:collections-v2:0x00000000000000000000000000000000000000c1:0","amount":2}]`. The existing fixture `services/catalyst/shop/src/market/fixtures/collections-wearables-by-owner-200-unknown.json` (`[]` for an unknown owner) agrees with upstream. It is hand-authored, not captured.

Data dependency: as in §1. For `collectionId`, as in §3.

---

## 10. GET /collections/wearables (legacy-lambdas port; verbatim wire contract)

### Request

| Param | Upstream (decentraland/lamb2:src/controllers/handlers/wearables-catalog-handler.ts:26-76; decentraland/lamb2:src/logic/items-catalog.ts:14-43) | dcl-one (services/catalyst/shop/src/market/lambdas_catalog.rs:227-383, 875-955) | Verdict |
|---|---|---|---|
| `collectionId` | `getAll`, lowercased | `qs_get_array`, lowercased | MATCH |
| `wearableId` | `getAll`, lowercased | Same | MATCH |
| `textSearch` | `get`, lowercased; an empty string means absent; a value shorter than 3 (UTF-16 length) gives 400 `The text search must be at least 3 characters long` | Same message, but counts **chars** (lambdas_catalog.rs:353-357) | MATCH (P2 for astral characters) |
| No filter | 400 `You must use one of the filters: 'textSearch', 'collectionId' or 'wearableId'` | Same message, but any **extended** filter (`collectionType`, `category`, `rarity`, `name`) also satisfies the requirement (lambdas_catalog.rs:344-352) | DIVERGENT: `?category=hat` is 400 upstream and 200 in dcl-one |
| More than 500 ids | 400 `You can't ask for more than 500 wearables` / `You can't filter for more than 500 collection ids` | Same | MATCH |
| `limit` | `parseInt`; NaN, ≤0 or >500 give 500 (items-catalog.ts:88-97) | `i64` parse, same clamp (lambdas_catalog.rs:324-329); `"10abc"` gives 500 instead of 10 | DIVERGENT (C7, P2) |
| `lastId` | `get`, lowercased; empty means absent | Same (lambdas_catalog.rs:381) | MATCH |
| `collectionType`, `category`, `rarity`, `name`, `orderBy`, `direction` | **Ignored** | Parsed and validated. `collectionType=xyz` gives 400 `Invalid collectionType 'xyz'. ...`; an invalid rarity or sort gives 400; `orderBy` without exactly one `collectionType` gives 400 (lambdas_catalog.rs:138-149, 227-264) | DIVERGENT (dcl-one extension; a request upstream accepts can be a 400) |

### Responses

| Trigger | Status | Body | Upstream | dcl-one | Verdict |
|---|---|---|---|---|---|
| Success | 200 | `{"wearables":[WearableDefinition],"filters":{"collectionIds"?:[...],"itemIds"?:[...],"textSearch"?:string},"pagination":{"limit":number,"lastId"?:string,"next"?:"?<query>"}}`. Undefined keys are omitted by `JSON.stringify` | wearables-catalog-handler.ts:64-75 | lambdas_catalog.rs:853-873 | DIVERGENT: dcl-one **never emits `pagination.lastId`** (lambdas_catalog.rs:861-866). P1 |
| Ordering | | Base wearables first: in-range base items sorted by lowercased URN with `localeCompare`. Then on-chain definitions, re-sorted by lowercased `id` with `localeCompare` (items-catalog.ts:45-61; wearables-catalog-handler.ts:88-109) | | dcl-one: base sorted by lowercased URN **byte order** (lambdas_catalog.rs:828); on-chain sorted by lowercased id byte order (lambdas_catalog.rs:766-795) | DIVERGENT where UCA and byte order disagree (`_` versus digits, P2) |
| On-chain source | | Upstream L1 `items` first, then L2, each `orderBy: urn asc`, with `urn_gt: lastId`. The cursor's network picks the starting layer (fetch-items-by-filters.ts:41-70) | | dcl-one mirror: one SQL query over both networks with `lower(urn) COLLATE "C" > lastId` (lambdas_catalog.rs:422-519). Before the first pass: the subgraphs, with a different query (lambdas_catalog.rs:592-758) | MATCH in order for L1 < L2 (`ethereum` < `matic`) |
| `textSearch` semantics | | Upstream: subgraph `searchText_contains` on items | | Mirror: `i.name ILIKE '%t%'` (lambdas_catalog.rs:468-471); subgraph fallback: `searchText_contains` (lambdas_catalog.rs:626-628) | DIVERGENT on the mirror path (P1) |
| `collectionId` semantics | | Upstream: `collections(where:{urn_in})` exact, then nested `items` | | dcl-one: `lower(i.urn) LIKE '<collectionId>:%'` (lambdas_catalog.rs:441-456) | MATCH for collection URNs. A partial URN (for example a v2 collection id lacking the contract) matches nothing upstream but may match a prefix in dcl-one (P2) |
| `next` encoding | | `URLSearchParams.toString()`: space becomes `+`, `*` is kept, `~` becomes `%7E` (items-catalog.ts:63-86) | | `urlencoding::encode`: space becomes `%20`, `*` becomes `%2A`, `~` is kept (lambdas_catalog.rs:151-153, 385-411) | DIVERGENT (P2; `:` is `%3A` in both) |
| Mirror read | | — | | Adds `as_of` and `stale_after_secs` keys when the mirror was read (mirrors/mod.rs:621-645) | DIVERGENT (P2) |
| Base-only cursor | | Base items are included when there is no `lastId` or `lastId` starts with the base collection id (wearables-catalog-handler.ts:41-48) | | Same (lambdas_catalog.rs:884-893) | MATCH |
| Definition mapping | | `extractWearableDefinitionFromEntity` | | `extract_wearable_definition` | See LI-14 |
| Validation errors | 400 | C1 with the messages above | items-catalog.ts:20-32 | lambdas_catalog.rs:344-372 | MATCH (messages) |
| Subgraph error | upstream 500 (C5) / dcl-one 503 envelope `the collections subgraph reader did not answer` (subgraph path) or 500 (mirror SQL error) | | | lambdas_catalog.rs:547-566 | DIVERGENT (P1) |
| No mirror and no subgraph | dcl-one 503 envelope | | | lambdas_catalog.rs:530-536 | dcl-one only |

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

Identical to §9 with `emotesFetcher`/`emoteDefinitionsFetcher` (decentraland/lamb2:src/controllers/handlers/emotes-by-owner-handler.ts:10-32) and `user_items::emotes_by_owner` (services/catalyst/shop/src/market/user_items.rs:545-561).

Every row of §9's tables applies. In addition:

- With `collectionId`, both trees resolve **third-party wearables**, not emotes (items-by-owner.ts:64; user_items.rs:438). With `includeDefinitions`, they map them with the emote extractor (upstream through `emoteDefinitionsFetcher`; dcl-one `extract_emote_definition` on the entity, user_items.rs:446). MATCH.
- Upstream's on-chain emotes source is L2 only. dcl-one queries L1 + L2 and requires both readers configured (user_items.rs:454-456).

Data dependency: as in §2.

---

## 12. GET /collections/emotes (legacy-lambdas port)

### Request

As in §10, with `emoteId` in place of `wearableId` and the label `emotes`. There are no base emotes and no off-chain merge (decentraland/lamb2:src/controllers/handlers/emotes-catalog-handler.ts:22-45; items-catalog.ts:26-29). dcl-one passes `ExtendedFilters::default()`, so `collectionType`/`category`/`rarity`/`name`/`orderBy` are **not** parsed here (services/catalyst/shop/src/market/lambdas_catalog.rs:1028-1047). That matches upstream.

### Responses

| Trigger | Status | Body | Upstream | dcl-one | Verdict |
|---|---|---|---|---|---|
| Success | 200 | `{"emotes":[EmoteDefinition],"filters":{...},"pagination":{"limit","lastId"?,"next"?}}` | emotes-catalog-handler.ts:31-44 | lambdas_catalog.rs:957-1008 | DIVERGENT: `pagination.lastId` is never emitted (P1); `as_of` on the mirror path (P2) |
| Source | | Upstream: L2 subgraph only (`queryL1: false`), `urn_gt` cursor, `searchText_contains` (fetch-items-by-filters.ts:33-39) | | Mirror `item_type = 'emote_v1'` over both networks, `name ILIKE` | DIVERGENT on `textSearch` (P1) |
| Ordering | | `localeCompare` of lowercased id (items-catalog.ts:52-54) | | Byte order of lowercased id (lambdas_catalog.rs:766-795) | DIVERGENT (P2 edge) |
| Validation | 400 | `You must use one of the filters: 'textSearch', 'collectionId' or 'emoteId'`; `You can't ask for more than 500 emotes`; `You can't filter for more than 500 collection ids`; `The text search must be at least 3 characters long` | items-catalog.ts:20-32 | lambdas_catalog.rs:344-372 | MATCH |
| Subgraph or mirror unavailable | upstream 500 / dcl-one 503 envelope or 500 | | | lambdas_catalog.rs:530-568 | DIVERGENT |

Data dependency: the `shop.items` mirror (or the collections subgraphs) and emote entities in the content DB.

---

## Backlog

**LI-1 (no work). `/explorer/:address/wearables` applies `isSmartWearable` and `network`.**

- Status: MATCH, no work.
- Upstream (decentraland/lamb2:src/controllers/handlers/explorer-handler.ts:145-157, 185-189; decentraland/lamb2:src/adapters/marketplace-api-fetcher.ts:282-297; decentraland/lamb2:src/logic/fetch-elements/graph-pagination.ts:35-56): `isSmartWearable=true|1` keeps `smart_wearable_v1`; `network=ETHEREUM` keeps `wearable_v1`, `network=MATIC` keeps `wearable_v2`+`smart_wearable_v1`; either drops the base and third-party legs; any other `network` value is ignored.
- dcl-one: the same rule on the owned copies' item type, read from the subgraph `itemType` (services/catalyst/chain-index/src/graph.rs:242, 289) or the `shop.items` mirror, with both params in the cache key (services/catalyst/shop/src/market/lambdas_explorer.rs:121-125, 148-158, 677-686, 734-756).
- Severity: none (closed).

**LI-2 (P2). `/explorer/:address/wearables`: `thirdPartyCollectionId` lists each provider once and caps the values at 100.**

- Upstream (explorer-handler.ts:106-143, 180-182): the values are deduplicated as strings, cut to 5 parts and kept when they parse as a `blockchain-collection-third-party-name` URN; only those providers' wearables form the third-party leg, and an unknown name is a 502 `Third Party not found: <name>`. Two distinct values naming one provider list its wearables twice. There is no count limit.
- dcl-one: the same filter and the same 502 (services/catalyst/shop/src/market/lambdas_explorer.rs:623-651; services/catalyst/shop/src/market/third_party.rs:1010-1014). It deduplicates by provider name, refuses more than 100 values with a 400 (lambdas_explorer.rs:43, 97-102), and checks the URN structurally, without validating the network segment.
- Change: drop the cap or document it as a hardening; keep one entry per distinct string if byte parity is wanted.
- Severity: P2.

**LI-3 (no work). Explorer `trimmed=true` answers the trimmed object.**

- Status: MATCH, no work.
- Upstream (decentraland/lamb2:src/logic/utils.ts:84-145): wearables `{"entity":{"id","thumbnail"?,"individualData","metadata":{"id","name","rarity"?,"isSmart":itemType==="smart_wearable_v1","data":{"category","representations":[{"bodyShapes"}]}}}}`; emotes the same without `isSmart`, with `name = metadata.name` and `data` from `emoteDataADR74`.
- dcl-one: `item_to_trimmed_value` builds the same keys and leaves out the ones the entity lacks (services/catalyst/shop/src/market/lambdas_explorer.rs:497-573).
- Severity: none (closed).

**LI-4 (no work). Explorer `includeAmount` controls the trimmed `amount`.**

- Status: MATCH, no work.
- Upstream: `amount` (= `individualData.length || 0`) only when `includeAmount=true|1` (explorer-handler.ts:190-191, 209-216; decentraland/lamb2:src/controllers/handlers/explorer-emotes-handler.ts:76-77, 95-97).
- dcl-one: the same (services/catalyst/shop/src/market/lambdas_explorer.rs:136, 569-571).
- Severity: none (closed).

**LI-5 (P1). Explorer on-chain items include escrow grants from `shop.usage_grants`.**

- Upstream reports only chain-owned tokens (explorer-handler.ts:74-104).
- Decision needed: keep the extension behind an opt-in query param, or document it as an intended divergence.
- Change: services/catalyst/shop/src/market/lambdas_explorer.rs:209-229.

**LI-6 (P2). Explorer on-chain `itemType` is `""` when neither the subgraph nor the `shop.items` mirror knows it.**

- Expected: the source `itemType` (`wearable_v1|wearable_v2|smart_wearable_v1`), absent if unknown (decentraland/lamb2:src/adapters/marketplace-api-fetcher.ts:168; decentraland/lamb2:src/logic/fetch-elements/fetch-items.ts:184).
- dcl-one reads the subgraph `itemType` first, then the mirror (services/catalyst/shop/src/market/lambdas_explorer.rs:313-315), and writes `""` when both are empty (lambdas_explorer.rs:481-486).
- Change: omit the key when the type is unknown, in `item_to_value` (lambdas_explorer.rs:481-486).

**LI-7 (P1). `/users/:address/{wearables,emotes}`: `category` and `rarity` filters are case-sensitive.**

- Expected: both values are lowercased before matching (decentraland/lamb2:src/controllers/handlers/utils.ts:12-19).
- Change: `filter_groups` call site at services/catalyst/shop/src/market/user_items.rs:263-268.

**LI-8 (P1). `/users/:address/{wearables,emotes}`: sorting is validated and applied when `orderBy` is absent.**

- Expected (utils.ts:45-48; fetch-items.ts:82-85):
  - Validation runs only when `orderBy` is present, so `direction=foo` alone is a 200.
  - Without `orderBy`, no in-memory sort is applied: the order is the source's (subgraph `id` asc grouping in the fallback).
- Change: user_items.rs:114-163, 247-249, 270-272.
- Note: if dcl-one keeps a default order, document that the marketplace-server's default order is not in the pinned trees.

**LI-9 (P2). Third-party ownership without the NFT worker misses open mappings.**

- Expected: every token the wallet holds on each provider contract, on every `ContractNetwork` (`mainnet`, `sepolia`, `matic`, `amoy`), as the NFT worker returns it, matched against mappings with `includesNft`; a non-2xx worker answer is nothing for that network, a thrown request a 502 `FetcherError` (decentraland/lamb2:src/adapters/alchemy-nft-fetcher.ts:16-54; decentraland/lamb2:src/logic/fetch-elements/fetch-third-party-wearables.ts:80-139; decentraland/lamb2:src/adapters/elements-fetcher.ts:265-274).
- dcl-one with `NFT_WORKER_BASE_URL` (set to the same worker under `NODE_POSTURE=mirror`, services/catalyst/config/src/posture.rs:277-281) asks the worker the same way and answers the same (services/catalyst/shop/src/market/third_party.rs:877-935, 1029-1047, 1060-1063). MATCH there.
- dcl-one without it reads the chain (third_party.rs:391-695) and still differs:
  - `any` or wide-`range` mappings (more than 2,048 ids: a range is expanded only when `to - from < 2048`) on a plain ERC-721 the owner holds some of, or on any ERC-1155: only the explicitly mapped ids are confirmed; the rest is logged as unresolved and left out (third_party.rs:91, 309-331, 513-535);
  - an enumerable ERC-721 balance above 50,000 fails the read with a 502 (third_party.rs:93, 629-633);
  - a network with no RPC endpoint (`TRADE_RPC_URLS` entry, or `ETH_RPC_URL`/`RPC_ENDPOINT_POLYGON` for the networks of `ETH_NETWORK`) reports nothing owned (third_party.rs:358-377, 407-411).
- In the explorer, a contract or worker network that cannot be read is left out and the rest is served (`OnFailure::Skip`, third_party.rs:961-966; services/catalyst/shop/src/market/lambdas_explorer.rs:647), where upstream answers 502.
- Change: require `NFT_WORKER_BASE_URL` where the third-party routes are public, or index the provider contracts' transfers for the chain path; use `OnFailure::Fail` in the explorer for parity.
- Severity: P2 (the replacement posture uses the worker).

**LI-10 (P1). Third-party routes: a registry failure answers 200 with an empty list.**

- Expected:
  - `/users/:address/third-party-wearables`, `/users/:address/third-party-wearables/:collectionId`, the explorer and `/collections/*-by-owner/:owner?collectionId=` with the registry unreadable and no cached value: 502 `{"error":"The requested items cannot be fetched right now","message":"Cannot fetch elements for <address>"}`.
  - `/third-party-integrations`: 502 with `"message":"Cannot fetch third party providers"`.
  - Refs: decentraland/lamb2:src/logic/third-party-providers-storage.ts:46-52; decentraland/lamb2:src/adapters/elements-fetcher.ts:265-274; decentraland/lamb2:src/controllers/handlers/errorHandler.ts:27-35.
- dcl-one: `third_party_providers` reads a failure as an empty provider list (services/catalyst/shop/src/market/third_party.rs:205-223). The listings then answer 200 with nothing third-party; a route naming a provider answers 502 `Third Party not found: <name>` instead; `/third-party-integrations` answers 200 `{"data":[]}` (third_party.rs:1396-1399). An unknown provider already answers lamb2's 502 `Third Party not found: <name>` (third_party.rs:1010-1014).
- Change: make `third_party_providers` return `Result` and map its error to `FetchFailure::Fetch` (third_party.rs:205-223, 1002-1014), and answer 502 from `third_party_integrations` (third_party.rs:1378-1401).

**LI-11 (P1). Third-party routes cap `pageSize` at 1000.**

- Expected: no maximum (`paginationObject(url, Number.MAX_VALUE)`, decentraland/lamb2:src/controllers/handlers/third-party-wearables-handler.ts:42, 100).
- Change: `pagination_object` in services/catalyst/shop/src/market/third_party.rs:1209-1221. Use `OversizePolicy::Clamp` with `i64::MAX`, or skip the check.

**LI-12 (P2). `/users/:address/third-party-wearables/:collectionId` sorts by name.**

- Expected: fetcher order, with no sort (third-party-wearables-handler.ts:102-105).
- Change: services/catalyst/shop/src/market/third_party.rs:1301.

**LI-13 (P1). `includeDefinitions`/`includeEntities` emit `null` for a missing definition or entity.**

- Expected: the key is omitted (decentraland/lamb2:src/controllers/handlers/utils.ts:56-67; third-party-wearables-handler.ts:60-65).
- Change: services/catalyst/shop/src/market/user_items.rs:294-305; services/catalyst/shop/src/market/third_party.rs:1332-1338.

**LI-14 (P2). Definition mapping edge cases differ.**

- Expected (decentraland/lamb2:src/adapters/definitions.ts:4-102):
  - `thumbnail` is the content URL, or **absent** when the file has no hash; dcl-one keeps the file name.
  - Each representation's `contents[]` entry without a hash is `{"key":<file>}` with no `url`; dcl-one emits `"url":""`.
  - For a legacy emote, `tags` is `metadata.data.tags`, absent if undefined; dcl-one emits `[]`.
  - `loop` is `emoteDataV0.loop` when `emoteDataV0` exists, else `false`.
  - The base URL is `CONTENT_URL` **lowercased**, without a trailing `/`.
  - A wearable whose `metadata.data.representations` is missing yields no definition; dcl-one returns the metadata unchanged.
- Change: services/catalyst/shop/src/market/definitions.rs:53-160.

**LI-15 (P1). `/users/:address/{wearables,emotes}` and the on-chain by-owner lists answer 503 when the reader fails.**

- Expected: a failed owned-items fetch is 502 `{"error":"The requested items cannot be fetched right now","message":"Cannot fetch elements for <address>"}` (decentraland/lamb2:src/adapters/elements-fetcher.ts:265-274).
- The explorer and the third-party routes already answer it (services/catalyst/shop/src/market/lambdas_explorer.rs:716-719; services/catalyst/shop/src/market/third_party.rs:1645-1656).
- Change: services/catalyst/shop/src/market/user_items.rs:255-261, 468-474. Answer `third_party::fetch_error`.
- The unconfigured-reader envelope (`readers::require_both_collections`) is a deployment condition and may stay.

**LI-16 (P2). Address validation on routes where upstream has none.**

- Upstream accepts any `:address` and answers 200: empty for `/users/*`, and base wearables only for the explorer.
- dcl-one answers 400 `Address must be a valid Ethereum address`:
  - services/catalyst/shop/src/market/user_items.rs:231-233;
  - services/catalyst/shop/src/market/third_party.rs:1229-1231, 1277-1279;
  - services/catalyst/shop/src/market/lambdas_explorer.rs:660-662.
- Decide whether this is an intended hardening. If it is not, answer 200 with an empty page, and with the base wearables for the explorer.

**LI-17 (P1). Owned-copies cap of 10 pages × 1,000 per network.**

- Expected: the full inventory. The upstream fallback walks every page (decentraland/lamb2:src/logic/fetch-elements/fetch-elements.ts:13-46).
- Change: `MAX_PAGES` in services/catalyst/chain-index/src/graph.rs:28-29, 323-347. Remove the cap, or raise it to match marketplace-api's 100 × 1,000 groups.
- Also return an error rather than a silently truncated `totalAmount`.

**LI-18 (P2). `individualData` order is newest transfer first.**

- Expected: source order; subgraph `id` ascending in the fallback (fetch-items.ts:157-195).
- Change: services/catalyst/chain-index/src/graph.rs:369-374; services/catalyst/shop/src/market/user_items.rs:61-94.

**LI-19 (P2). `/collections/*-by-owner/:owner` is sorted by rarity DESC then urn.**

- Expected: fetcher order, with no sort (decentraland/lamb2:src/logic/items-by-owner.ts:46-52).
- Change: services/catalyst/shop/src/market/user_items.rs:382-388, 498.

**LI-20 (P1). `/collections/{wearables,emotes}`: `pagination.lastId` is never emitted.**

- Expected: `pagination: {limit, lastId: <request lastId, lowercased> (omitted if none), next?}` (decentraland/lamb2:src/controllers/handlers/wearables-catalog-handler.ts:68-72; decentraland/lamb2:src/controllers/handlers/emotes-catalog-handler.ts:37-41).
- Change: `catalog_response` in services/catalyst/shop/src/market/lambdas_catalog.rs:853-873. Pass `query.last_id`.

**LI-21 (P1). `/collections/{wearables,emotes}`: `textSearch` uses `name ILIKE` on the mirror.**

- Expected: subgraph `items.searchText_contains: <textSearch>` (decentraland/lamb2:src/logic/fetch-elements/fetch-items-by-filters.ts:116-119).
- Change: services/catalyst/shop/src/market/lambdas_catalog.rs:468-471. Mirror the subgraph `searchText` column into `shop.items` (services/catalyst/shop/src/market/mirrors/items.rs) and match on it.

**LI-22 (P2). `/collections/wearables`: the extended filters change the upstream contract.**

- Expected: `collectionType`, `category`, `rarity`, `name`, `orderBy` and `direction` are ignored, and alone they do not satisfy "one of the filters" (decentraland/lamb2:src/logic/items-catalog.ts:14-43).
- dcl-one either rejects (400) or accepts requests that upstream treats the other way.
- Change: services/catalyst/shop/src/market/lambdas_catalog.rs:227-264, 344-352. Gate the extensions behind a separate path or opt-in.

**LI-23 (P2). Catalog ordering uses byte order instead of `localeCompare`.**

- Expected: `a.id.toLowerCase().localeCompare(b.id.toLowerCase())` for on-chain items, and the same on lowercased URNs for base items (decentraland/lamb2:src/logic/items-catalog.ts:52-54; wearables-catalog-handler.ts:107).
- Change: services/catalyst/shop/src/market/lambdas_catalog.rs:766-795, 828. Use `locale_cmp`.

**LI-24 (P2). Catalog `next` query encoding.**

- Expected: `URLSearchParams.toString()` (space as `+`, `*` kept unencoded, `~` as `%7E`) (items-catalog.ts:63-86).
- Change: `qp`/`build_next_query` in services/catalyst/shop/src/market/lambdas_catalog.rs:151-153, 385-411.

**LI-25 (P2). Mirror-backed answers carry `as_of` and `stale_after_secs` body keys.**

- Affects `/nfts/collections` and `/collections/{wearables,emotes}`.
- Expected: no such keys (decentraland/lamb2:src/controllers/handlers/all-collections-handler.ts:50-53).
- Change: services/catalyst/shop/src/market/mirrors/mod.rs:621-645. Use a response header instead of body keys on lambdas routes.

**LI-26 (P2). Pagination parsing.**

- Expected: `parseInt` semantics for `pageNum`, `pageSize` and `limit`: `"10abc"`→10 and `"abc"`→NaN, so an empty page with `null` echoes (decentraland/lamb2:src/logic/pagination.ts:4-19; items-catalog.ts:88-97).
- Expected: the explorer's negative offset slices from the end (pagination.ts:31-40).
- Change: services/catalyst/shared/src/query_params.rs:48-50, 130-176; services/catalyst/shop/src/market/lambdas_explorer.rs:707-713; services/catalyst/shop/src/market/lambdas_catalog.rs:324-329.

**LI-27 (P2). `/third-party-integrations` query uses `first: 1000`.**

- Expected: the provider cache list, default `first` of 100 (decentraland/lamb2:src/adapters/third-party-providers-graph-fetcher.ts:12-29).
- Expected: `name`/`description` passed through as the subgraph returns them (`null` stays `null`).
- Change: services/catalyst/shop/src/market/third_party.rs:1351-1374.

**LI-28 (P2). Explorer sort edges.**

- Expected (decentraland/lamb2:src/logic/sorting.ts:38-84):
  - `rarestOptional` counts any non-empty `rarity` as rated.
  - `newestOptional` and `oldestOptional` count an item as dated only when both min and max transfer are non-zero.
- Change: services/catalyst/shop/src/market/lambdas_explorer.rs:369-423, 277.

**LI-29 (P2). Explorer validation order.**

- Expected order: `pageSize` → `rarity` → sort → `collectionType` (decentraland/lamb2:src/controllers/handlers/explorer-handler.ts:174-195).
- Change: services/catalyst/shop/src/market/lambdas_explorer.rs:74-119.

**LI-30 (P2). `/users/:address/third-party-wearables/:collectionId` URN errors.**

- Expected: a parseable non-third-party-name URN gets `Invalid collection id: not a blockchain-collection-third-party-name nor blockchain-collection-third-party-collection URN`. The network segment is validated as `@dcl/urn-resolver` does (decentraland/lamb2:src/controllers/handlers/third-party-wearables-handler.ts:88-97).
- Change: services/catalyst/shop/src/market/third_party.rs:1280-1289; `is_third_party_name_urn` at services/catalyst/shop/src/market/user_items.rs:403-410.

**LI-31 (P2). Explorer emote `category` for v0 emotes is `""`.**

- Expected: the category of the source item (subgraph or marketplace), so a v0 emote still matches category filters.
- Change: services/catalyst/shop/src/market/lambdas_explorer.rs:352-358.

**LI-32 (P2). Global error shape (C5).**

- Expected: 500 `{"error":"Internal Server Error"}` without `message` (errorHandler.ts:66-76).
- Change: services/catalyst/shared/src/http_util.rs:30-36. Shared across lambdas; coordinate with the other lambdas areas.

## dcl-one extras in this area (no upstream counterpart)

- `GET /lambdas/collections`: alias of `/lambdas/nfts/collections` (services/catalyst/shop/src/market/mod.rs:941-944).
- `GET /explorer/{address}/wearables` and `GET /explorer/{address}/emotes` at the root, besides the `/lambdas` paths (mod.rs:967-974).
- `/collections/wearables` extended filters: `collectionType`, `category`, `rarity`, `name`, `orderBy`, `direction`, with their 400 messages and `filters` echo keys `collectionTypes`, `categories`, `rarity`, `name`, `orderBy`, `direction` (services/catalyst/shop/src/market/lambdas_catalog.rs:161-264).
- Explorer escrow grants (`shop.usage_grants`) merged into the on-chain leg (services/catalyst/shop/src/market/lambdas_explorer.rs:209-229).
- `as_of` and `stale_after_secs` body keys on mirror-backed answers (services/catalyst/shop/src/market/mirrors/mod.rs:621-645).
- The unavailable envelope (`source`, `configure`, `Retry-After: 60`) when a reader or mirror is not configured or not ready (services/catalyst/chain-index/src/source.rs:88-103).
- The marketplace-server routes in `shop/src/market/collections.rs` and `nft_reads.rs` (`/v1/collections`, `/v1/nfts`, …, under `/shop-api` and `/marketplace-api`) are outside the lambdas surface. They are not compared here.
