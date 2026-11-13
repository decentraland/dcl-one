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
| this tree | `37e91aed` (origin/main, 2026-11-04) |

## Verdicts

- **MATCH:** every response read answers the same status, body fields, headers and error branches.
- **GAP:** the route answers the common case as upstream does, but a whole upstream behaviour is absent: a
  rate limit, a filter, an ownership check or a sanitization step. Requests that depend on it get a
  different answer.
- **DIVERGENT:** the route has the upstream behaviours, but one or more fields, headers, statuses or error
  bodies differ.
- **MISSING:** no route, or no handling, at that method and path.

A backlog item whose behaviour matches in this tree keeps its id and reads "Status: MATCH, no work".

## Files

| file | area | endpoints | MATCH | GAP | DIVERGENT | MISSING | backlog |
| --- | --- | --- | --- | --- | --- | --- | --- |
| [content-writes.md](content-writes.md) | `POST /entities` (regular and `partial=true`), preflight, 405 | 4 | 1 | 0 | 3 | 0 | CW-1..18 |
| [content-reads.md](content-reads.md) | `/entities/*`, `/contents/:hash` (GET/HEAD), `/contents/:hash/active-entities`, `/available-content`, `/audit`, 404/405/CORS | 8 | 1 | 2 | 5 | 0 | CR-1..20 |
| [content-sync.md](content-sync.md) | `/deployments`, `/pointer-changes`, `/failed-deployments`, `/snapshots`, `/status`, `/challenge`, `/queries/items/*`, `/queries/erc721/*` | 11 | 0 | 0 | 11 | 0 | CS-01..24 |
| [lambdas-profiles.md](lambdas-profiles.md) | `/profiles`, `/profile/:id`, `/outfits/:id`, `/status`, `/about`, `/contracts/*`, `/crypto/validate-signature` | 10 | 1 | 0 | 8 | 1 | LP-01..32 |
| [lambdas-items.md](lambdas-items.md) | `/users/:a/{wearables,emotes,third-party-wearables}`, `/third-party-integrations`, `/explorer/:a/*`, `/nfts/collections`, `/collections/*` | 12 | 0 | 0 | 12 | 0 | LI-1..32 |
| [lambdas-land.md](lambdas-land.md) | `/users/:a/{names,lands,lands-permissions}`, `/names/:n/owner`, `/users/:a/parcels/:x/:y/permissions`, `/parcels/:x/:y/operators` | 6 | 0 | 0 | 6 | 0 | LL-1..24 |
| **total** | | **51** | **3** | **2** | **45** | **1** | **150** (11 with no work, 139 open) |
| [asset-bundle-registry.md](asset-bundle-registry.md) (not in the total) | ABR content routes: `/profiles`, `/profiles/metadata`, `/entities/{active,versions,status}`, `/worlds/:n/manifest`, `/denylist`; admin routes listed | 14 | 2 | 5 | 2 | 4 | AB-1..16 |

The asset-bundle-registry area counts on its own: 14 endpoints (MATCH 2, GAP 5, DIVERGENT 2, MISSING 4, the four ABR admin routes outside catalyst parity; `/queues/status` is forwarded, outside parity scope), 16 backlog items (7 with no work, 9 open), no P0.

Every upstream route has a route at the same path in this tree, except `GET /lambdas/about`: only the root
`/about` exists here.

Most rows are DIVERGENT on one field, header or error branch rather than on the whole response. Each file
lists its own dcl-one-only routes at the end.

## P0: a client breaks or the data is wrong

No backlog item is P0 in this tree, the asset-bundle-registry area included. These items keep their
ids with "Status: MATCH, no work": CR-1, CR-13, CS-01, LP-01, LP-15, LI-1, LI-3, LI-4, LL-1 and LL-2.

The largest open items:

| id | endpoint | severity | what remains |
| --- | --- | --- | --- |
| CW-1 | `POST /entities?partial=true` | P2 | Batches are buffered in memory and take the in-memory upload budget (upstream spools to disk), so under load dcl-one answers 503 where upstream accepts; the busy-lock and expired-upload messages differ. |
| LI-9 | third-party wearables (all routes) | P2 | With `NFT_WORKER_BASE_URL` (set by `NODE_POSTURE=mirror`) ownership matches. Without it, the JSON-RPC path leaves `any` and wide `range` mappings on plain ERC-721 and on ERC-1155 unresolved, fails above 50,000 enumerable tokens, and reads a network with no RPC endpoint as nothing owned. |
| LI-10 | third-party routes | P1 | A registry failure reads as an empty provider list (200, or 502 `Third Party not found`), where upstream answers 502 `Cannot fetch elements for <address>`. |
| LP-02 | `GET /outfits/:id` | P2 | A failed third-party check answers 502 where upstream answers 500; without a collections subgraph nothing is checked. |
| LI-2 | `GET /explorer/:a/wearables` | P2 | `thirdPartyCollectionId` lists each provider once, refuses more than 100 values, and checks the URN without its network segment. |

## Themes that cut across files

These problems show up in several files, so one fix can close several rows.

- **Validation error bodies.** Upstream uses the `@dcl/schema-validator-component` body:
  `400 {"ok":false,"message":"Invalid JSON body","data":[...]}`. It is used on `POST /entities/active` and
  `POST /crypto/validate-signature`. `POST /entities/active` answers that shape here, with a `data` array
  rebuilt from AJV's rules; the eleven captured schema cases match it value for value, and key order differs (CR-2). The other handlers have their own
  message, or axum's plain-text 400/415/422 (LP validate-signature, CW-8).
- **Failure statuses.**
  - When a source fails, upstream lambdas answer 502 or 500. This tree answers:
    - lamb2's 502 `FetcherError` on the explorer, the third-party routes and the by-owner lists with
      `collectionId` (LI-15); outfits answer the same 502 when the third-party check fails, where upstream
      answers 500 (LP-02);
    - lamb2's bare 500 on `/users/:a/parcels/:x/:y/permissions`;
    - 503 on the other subgraph and index readers: names, lands, operators, lands-permissions,
      `/users/:a/{wearables,emotes}` and the on-chain by-owner lists (LL-9, LL-10, LI-15);
    - an empty provider list on a third-party registry failure, where upstream answers 502 (LI-10).
  - Some 400/503 bodies here carry internal error text, where upstream sends a fixed message (CW-9).
- **Page-size limits.** This tree clamps or rejects at 1000 where upstream has no cap, and the reverse
  (LL, LI). Paged subgraph reads also stop at 10,000–20,000 rows, where upstream reads every page.
- **Rate limits.** This tree's gateway limits every path by client IP: `/content/*` at 100 r/s with burst
  1000, `POST /entities` at 2 r/s with burst 4, and everything else, `/lambdas/*` included, at 30 r/s with
  burst 60. Over the limit it answers nginx's HTML 429 without `Retry-After` (`infra/gateway/gateway.py`).
  Upstream limits only `/contents` reads and `POST /entities`. These upstream limits are not in this tree:
  - the `/contents` GET quotas and the trusted-sync-peer bypass, which the gateway limit lacks too (CR-3, CR-4);
  - the `POST /entities` burst limit, which answers a JSON 429 with `Retry-After` (CW-6);
  - per-source upload admission.
- **CORS.** Upstream answers `Access-Control-Allow-Origin: *` with max-age 600. This tree echoes the origin
  with credentials and uses max-age 86400 (CR X3, CW preflight).
- **Data sources and defaults.** Syncing the content server feeds only the content routes. Every
  lambdas route needs chain data, and a node that only synced catalyst content answers them with 503 or
  unfiltered data until the sources are configured:
  - **Ownership** of collections, names and LAND comes from TheGraph subgraphs or this tree's chain
    indexes, depending on the route.
  - **Third-party wearables** come from the registry subgraph plus the NFT worker when
    `NFT_WORKER_BASE_URL` is set, else JSON-RPC (LI-9).
  - **EIP-1654** (contract-wallet signatures) needs `ETH_RPC_URL`.

  Upstream ships production defaults for each of these sources. In this tree `NODE_POSTURE=mirror` sets the
  collections, marketplace, third-party registry and LAND subgraph URLs and `NFT_WORKER_BASE_URL`, turns on
  the chain indexes, and leaves `ETH_RPC_URL` at the binary's default (`services/catalyst/config/src/posture.rs`).
  A mirror refuses to start with the third-party registry URL or the NFT worker set empty. With `NODE_POSTURE`
  unset, `island` or `journey`, the subgraph URLs stay unset (LL-3). The sources each route reads are listed in each file's "Data dependency" and "Where dcl-one gets
  the data" sections.

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

Captures taken from the production peer on 2026-11-04 are in [`captures/`](captures/README.md); the rows
they settle cite them. `@dcl/content-validator` rejection strings are not among them: reaching them needs
`POST /entities`, a write.

The existing replay fixtures are relevant here too. Those in `tests/fixtures/content/` and
`shop/src/market/fixtures/` are hand-authored. Several of them pin behaviour that differs from the
upstream revisions above; each file names them in its backlog (CR-2, CR-5, CW-8, LI, LL).
