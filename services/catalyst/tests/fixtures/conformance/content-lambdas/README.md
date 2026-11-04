# Content server and lambdas: replacement backlog

These files compare every endpoint that a Decentraland Catalyst serves with what a Catalyst node built from
this tree (`serve`) serves at the same path.

- **Content server:** upstream serves it at `/content/*`. dcl-one serves it at the root and again under
  `/content`.
- **Lambdas:** upstream serves them at `/lambdas/*`.

The target is a transparent replacement. Given the same synced data, each method, path, status, body field,
header and error branch should be the same. Every row was reached by reading both trees: nothing here was
captured from a live peer or run, unless a row says otherwise.

| upstream | revision |
| --- | --- |
| `github.com/decentraland/catalyst` (content server) | `a416565db1e705cff9523540a2578af642598a29` (2026-10-21) |
| `github.com/decentraland/lamb2` (lambdas) | `02a37f5d88fb895339c20e84c2470be68cdf3f57` (2026-10-21) |
| this tree | `0229ef9eaf13ff72c314c85d167956989fe00395` |

## Files

| file | area | endpoints | MATCH | GAP | DIVERGENT | MISSING | backlog |
| --- | --- | --- | --- | --- | --- | --- | --- |
| [content-writes.md](content-writes.md) | `POST /entities` (regular and `partial=true`), preflight, 405 | 4 | 1 | 0 | 2 | 1 | CW-1..18 |
| [content-reads.md](content-reads.md) | `/entities/*`, `/contents/:hash` (GET/HEAD), `/contents/:hash/active-entities`, `/available-content`, `/audit`, 404/405/CORS | 8 | 1 | 2 | 5 | 0 | CR-1..20 |
| [content-sync.md](content-sync.md) | `/deployments`, `/pointer-changes`, `/failed-deployments`, `/snapshots`, `/status`, `/challenge`, `/queries/items/*`, `/queries/erc721/*` | 11 | 0 | 1 | 10 | 0 | CS-01..23 |
| [lambdas-profiles.md](lambdas-profiles.md) | `/profiles`, `/profile/:id`, `/outfits/:id`, `/status`, `/about`, `/contracts/*`, `/crypto/validate-signature` | 10 | 1 | 1 | 7 | 1 | LP-01..32 |
| [lambdas-items.md](lambdas-items.md) | `/users/:a/{wearables,emotes,third-party-wearables}`, `/third-party-integrations`, `/explorer/:a/*`, `/nfts/collections`, `/collections/*` | 12 | 0 | 2 | 10 | 0 | LI-1..32 |
| [lambdas-land.md](lambdas-land.md) | `/users/:a/{names,lands,lands-permissions}`, `/names/:n/owner`, `/users/:a/parcels/:x/:y/permissions`, `/parcels/:x/:y/operators` | 6 | 0 | 0 | 6 | 0 | LL-1..24 |
| **total** | | **51** | **3** | **6** | **40** | **2** | **149** |

Every upstream route has a route at the same path in this tree, except two:
- `GET /lambdas/about`: only the root `/about` exists here.
- `POST /entities?partial=true`: partial uploads.

Most rows are DIVERGENT on one field, header or error branch rather than on the whole response. Each file
lists its own dcl-one-only routes at the end.

## P0: a client breaks or the data is wrong

| id | endpoint | gap |
| --- | --- | --- |
| CW-1 | `POST /entities?partial=true` | Partial uploads (ADR-325, `202 {"missing":[...]}`) are not implemented. Each batch is treated as a full deployment and refused with 400, so large scene deploys from the SDK and Creator Hub fail. |
| CR-1 | `POST /entities/active` | A body without `Content-Type: application/json` answers 415. Upstream parses it regardless of the header (`ensureJsonContentType: false`). |
| CS-01 | `GET /deployments` | Deployments of denylisted entities are returned; upstream drops them. |
| LP-01 | `POST /profiles`, `GET /profiles/:id` | Every third-party wearable is dropped, because ownership is checked against the collections subgraph. Upstream keeps v1 third-party URNs, checks linked ones through the mapping plus on-chain reads, and appends the owned ones. |
| LP-02 | `GET /outfits/:id` | Outfits are served as deployed. Upstream drops outfits that hold an unowned wearable, appends `:tokenId`, lowercases URNs and checks third-party items. |
| LI-1 | `GET /explorer/:a/wearables` | `isSmartWearable` and `network` are ignored. |
| LI-2 | `GET /explorer/:a/wearables` | `thirdPartyCollectionId` is ignored, so every provider's linked wearables are returned. |
| LI-3 | `GET /explorer/:a/{wearables,emotes}` | `trimmed=true` returns the whole entity instead of the trimmed object, and `metadata.isSmart` is missing. |
| LI-9 | third-party wearables (all routes) | Ownership is read over JSON-RPC, not from the NFT worker. Missed cases: `any` and wide `range` mappings on non-enumerable contracts, holdings above 1,000, and `sepolia`/`amoy`. An RPC error reads as nothing owned. |
| LL-1 | `/parcels/:x/:y/operators`, `/users/:a/parcels/:x/:y/permissions` | Coordinates are limited to ±150, so the real parcels outside that box answer 400. |
| LL-2 | `/users/:a/parcels/:x/:y/permissions` | Until the land-authz index is ready, the operator flags are `null` instead of booleans. The LAND subgraph is never consulted on this route. |
| LL-3 | all LAND and names routes | A fresh node answers 503: `MARKETPLACE_SUBGRAPH_URL` and `LAND_SUBGRAPH_URL` have no default, and `LAND_AUTHZ_INDEX_ENABLED` is off. Upstream defaults to the production subgraphs. |

## Themes that cut across files

These problems show up in several files, so one fix can close several rows.

- **Validation error bodies.** Upstream uses the `@dcl/schema-validator-component` body:
  `400 {"ok":false,"message":"Invalid JSON body","data":[...]}`. It is used on `POST /entities/active` and
  `POST /crypto/validate-signature`. Here, each handler has its own message, or axum's plain-text 400/415/422
  (CR-2, LP validate-signature, CW-8).
- **Failure statuses.**
  - When a source fails, upstream lambdas answer 502 or 500. This tree answers 503 (LL, LI).
  - Some 400/503 bodies here carry internal error text, where upstream sends a fixed message (CW-9).
- **Page-size limits.** This tree clamps or rejects at 1000 where upstream has no cap, and the reverse
  (LL, LI). Paged subgraph reads also stop at 10,000–20,000 rows, where upstream reads every page.
- **Rate limits.** These are not in this tree:
  - the `/contents` GET quotas and the trusted-sync-peer bypass (CR-3, CR-4);
  - the `POST /entities` burst limit, which answers a JSON 429 with `Retry-After` (CW-6);
  - per-source upload admission.
- **CORS.** Upstream answers `Access-Control-Allow-Origin: *` with max-age 600. This tree echoes the origin
  with credentials and uses max-age 86400 (CR X3, CW preflight).
- **Data sources and defaults.** Syncing the content server feeds only the content routes. Every
  lambdas route needs chain data, and a node that only synced catalyst content answers them with 503 or
  unfiltered data until the sources are configured:
  - **Ownership** of collections, names and LAND comes from TheGraph subgraphs or this tree's chain
    indexes, depending on the route.
  - **Third-party wearables** come from the registry subgraph plus JSON-RPC.
  - **EIP-1654** (contract-wallet signatures) needs `ETH_RPC_URL`.

  Upstream ships production defaults for each of these sources. The sources each route reads are listed in
  each file's "Data dependency" and "Where dcl-one gets the data" sections.

## Rows that need a live capture

Some upstream behaviour comes from npm packages that are not in the pinned trees. Those rows are marked
"not vendored" or "needs capture", and should be confirmed against a production peer before the fix is
written. The packages are:
- `@dcl/http-server`: 404, 405 and 413 bodies, trailing-slash and case matching;
- `@dcl/schema-validator-component`: the `data` array;
- `@dcl/content-validator`: most rejection strings;
- `@dcl/crypto`;
- `@dcl/schemas`: parcel bounds, ERC-721 traits;
- `@dcl/catalyst-contracts`.

The existing replay fixtures are relevant here too. Those in `tests/fixtures/content/` and
`shop/src/market/fixtures/` are hand-authored. Several of them pin behaviour that differs from the
upstream revisions above; each file names them in its backlog (CR-2, CR-5, CW-8, LI, LL).
