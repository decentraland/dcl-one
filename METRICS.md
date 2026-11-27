# Measured query workloads

## Marketplace sales indexing

A PostgreSQL 17.6 fixture held 200,000 Polygon event rows, with one accepted bid per 1,000 blocks and a mint in each other block. After `ANALYZE`, the fold's ordered lookup for 96 accepted bids between blocks 20,001 and 150,000 scanned 200,000 rows and read 2,273 local buffers in 9.763 ms. The 16 KiB partial index on accepted bids returned those 96 rows in 0.204 ms with 97 local buffer reads.

A local Polygon refold fixture replayed 288 legacy sales in 288 distinct blocks using PostgreSQL and a loopback RPC with successful receipts. Replacing the NFT cursor epoch forced a complete rebuild. Each rebuild settled all 288 payments and published in three consecutive passes with 36 RPC requests. Three warmed runs took 90, 88 and 85 ms, excluding scheduling delays. Production history size, provider latency, retries and pass scheduling determine deployment recovery time.

The index process increments `catalyst_index_sales_repair_failures_total` by indexer and network when a historical sales repair fails. The error log includes the starting block and requests spent. The cursor keeps its last verified boundary.

## Marketplace order book reads

PostgreSQL 17.6 on Apple Silicon held one million legacy orders (700,000
Polygon and 300,000 Ethereum), 700,000 tokens and 2,000 Polygon collections.
The book held 130,000 stored open orders. Around 2% still had the seller's
ownership proof. `EXPLAIN (ANALYZE, BUFFERS)` used the repository's order
predicates, including LAND rights and catalogue membership. Each value below
is the median of three executions after `ANALYZE`. Other workspace builds
shared the machine.

| Unscoped page of 100 | Before | Indexed | Indexed shared buffers (hit/read) |
| --- | ---: | ---: | ---: |
| Issued ID ascending | 11,270 ms | 1.65 ms | 976/0 |
| Issued ID descending | 8,839 ms | 1.95 ms | 873/0 |
| Cheapest across statuses | 7,771 ms | 2.10 ms | 975/0 |

Both issued-ID directions use the 39.8 MiB token ID index with incremental
sorts for ties. Cheapest uses the 90.4 MiB price and order ID index. The
indexes took 2.75 s and 5.78 s to build. In a single rolled-back 1,000-order
append probe, they added 1,437 shared-buffer reads and 329 WAL full-page
images. That indexed run took 301 ms and produced 3.59 MB of WAL, compared to
12 ms and 0.87 MB in the baseline run, whose page accesses were all cache hits.

For the deepest newest-first open window (25,000), the page took 2,119 ms and
the two capped counts took 965 ms (Polygon) and 328 ms (Ethereum) with the
sort indexes installed. The page accessed 33,293 shared buffers and the counts
40,078 and 29,649. The query still proves ownership across the stored open
rows to fill the window. A trial partial index on open orders by creation time
left the heavy scans in place.

A skewed fixture added 100,000 backed open orders to one of the 200 requested
collections. The legacy leg uses a lateral top-N lookup inside the batched
statement so each index scan can stop at its own cap. The original batched
window had to rank every eligible row before its cut. In PostgreSQL-only
runs with eight clients and 24 reads per case, mean transaction latency fell
from 1,469 to 105 ms for a cap of 25, and from 1,690 to 488 ms for a cap of
5,000. Both shapes returned the same row counts. At the 5,000 cap, the median
`EXPLAIN (ANALYZE, BUFFERS)` fell from 432 ms (22,926 hits, 32,014 reads) to
89 ms (113,323 hits, zero reads).

The concurrent test used the same numbers of orders and tokens, plus 1,200
backed v3 listings in the requested collections. Eight clients used an
eight-connection pool and issued three reads each. The harness called the
Shop request functions directly, including their readiness checks and
response assembly. It sampled checked-out connections every 2 ms to
estimate mean time held per request. The mixed workload was 18 item
batches, three deepest open pages and three issued-ID pages. All 24 reads
in each run succeeded.

| Workload | Reads/s before → batched | Mean connection hold before → batched | Item batch p95 before → batched |
| --- | ---: | ---: | ---: |
| 20 contracts | 7.03 → 47.59 | 1,063 → 142 ms | 1,334 → 228 ms |
| 200 contracts | 2.30 → 17.49 | 3,370 → 432 ms | 3,634 → 626 ms |
| Mixed (200-contract batches) | 2.55 → 7.57 | 2,754 → 483 ms | 3,836 → 202 ms |

The 200-contract case took one local candidate statement per batch where
it had taken 400. Both implementations returned identical results at the
same cap of 5,000 per contract. The three deep pages in the batched mixed
run completed in up to 2.97 s. These are synthetic measurements without
HTTP transport time. The current request path uses the indexes and batching
without a book-result cache.

## Catalog legacy listing aggregation

A PostgreSQL 17.6 fixture held 1,000,000 legacy orders, including 18,867
open ones with seller ownership on Ethereum and Polygon, and 20,000 items
over 2,000 collections. The listing SQL joined ownership and item
identity, computed prices and counts, and capped the page at 20.
The legacy leg of a cheapest page took 234–375 ms in three single-session
runs and averaged 345 ms with eight clients and varying offsets. A
listing-dependent newest page (`isOnSale=true` on the fixture's resale
rows) took 322–448 ms and averaged 323 ms with eight clients. A newest
page without a listing predicate uses the page-first shape and took
10.5–12.7 ms. Its eight-client run averaged 21 ms with varying offsets.

V1's memo preflight reads publication and the next expiry on each
network. The index-only expiry lookup and cursor join took 0.068–0.209 ms
and averaged 0.149 ms with eight clients. A memo miss uses the applicable
listing query. These measurements exclude the v3 leg, the other readiness
checks and HTTP transport.

## NFT listing publication and page reads

A warm PostgreSQL 17.6 pool measured 20 Polygon folds at each latency.
Each fold committed a change to one ownership row, recorded the fold, waited
for two simulated successful head RPCs, and committed its publication.
The timing starts when the fold commit returns and ends when publication
returns.

| Delay per RPC | Median gap | p95 gap | p95 gap / 5 s |
| ---: | ---: | ---: | ---: |
| 0 ms | 0.60 ms | 0.79 ms | 0.02% |
| 25 ms | 55.75 ms | 57.16 ms | 1.14% |
| 100 ms | 205.87 ms | 207.25 ms | 4.15% |
| 250 ms | 506.89 ms | 507.89 ms | 10.16% |

The last column estimates the fraction of independent reads hitting the
window if a write-producing fold runs every five seconds. The deployed
rate depends on RPC latency, the pass cadence and which projections the
fold writes. The per-network publication-check and refusal counters use
`route="nfts"` for order-dependent NFT pages.
`catalyst_shop_nft_optional_land_read_failures_total` counts failed optional
LAND snapshot reads by network. Mixed pages can still return NAMEs and
collections when these reads fail.

On the same PostgreSQL version, a warm `EXPLAIN (ANALYZE, BUFFERS, TIMING OFF)`
of the bound page query used 32,000 tokens for one Polygon owner, 4,600
backed v3 listings, 4,450 open legacy listings, a wearable category and
`isOnSale=true&sortBy=cheapest&first=24`. Three executions after the first
pass took 210–220 ms for the epoch-to-milliseconds sort and 211–221 ms for
the stored `timestamp(3)` sort. Both plans drove each candidate's listing
lookup from `trade_assets_erc721_token_idx` and sorted at most one matching
trade per token in this fixture. The timestamp sort also leaves the
`(signer, created_at DESC)` index order available to the planner.

### Newest Ethereum NFT browse (2026-11-27)

On PostgreSQL 17.6 on Apple Silicon, a synthetic fixture held 3.5M NFTs,
102,000 items, 5,100 collections, 92,000 LAND and estate rights and 368,000
LAND authorization events. The page and capped count for `first=24`
were executed together from exact prepared SQL with forced generic plans
over 20 warmed repetitions.

| `/v1/nfts` | First before / after | p50 before / after | p95 before / after | Max before / after |
| --- | ---: | ---: | ---: | ---: |
| Ethereum | 698 / 346 ms | 685 / 261 ms | 1,157 / 302 ms | 1,198 / 304 ms |
| Polygon | 189 / 125 ms | 168 / 117 ms | 211 / 155 ms | 297 / 182 ms |

`EXPLAIN (ANALYZE, BUFFERS, SETTINGS)` showed the Ethereum LAND page doing
92,000 first-transfer lookups and joining token details before limiting.
Newest unfiltered pages still rank by those first transfers and attach
details only to the top LAND window. The unfiltered capped count reads
NAMEs first and can reach the cap before visiting the collection and
LAND legs. The paired pages and totals returned identical rows. Separate
v3 order enrichment of the 24 Ethereum tokens took 0.2–0.8 ms. The
eight Ethereum runs in a mixed four-client probe had a median of 362 ms
and a maximum of 399 ms. These are local SQL timings.

## Ethereum collections-v1 first walk

On 2026-11-26 the Ethereum `collections` walk (`shop::market::collections_v1`) ran its first history over `rpc.decentraland.org/mainnet`, which takes at most 10,000 blocks per `eth_getLogs`, with the default settings (10,000-block windows, a budget of 100 `eth_getLogs` per pass) and an empty scratch database. It walked from block 8,828,687 to the head, 17,671,095 blocks, in 18 passes run back to back, with 7,108 HTTP requests in all: `eth_getLogs` plus the headers of each window's bracket and of the blocks holding logs. That run recorded only the total. A two-pass rerun over the same start counted the requests by kind. Each pass spent its 100-request budget on `eth_getLogs`, about one per 10,000-block window: 989,704 and 1,000,000 blocks. Each pass also sent about 300 other requests, for the chain, the head, each window's bracket and the blocks holding logs: 299 and 302. So the full walk's `eth_getLogs` came to about 1,770, one per window, and the rest were headers. It took 2,198 s, plus the pause between passes a job adds (`MARKETPLACE_INDEX_INTERVAL_SECS`, 5 s, or 30 s under the mirror posture). It kept 438 `add_item`, 73,506 `issue` and 52 `ownership_transferred` rows. The 438 items and the 73,506 mints equal the collections subgraph's v1 items and their total supply at block 26,499,473. After the catch-up, each pass reads the same 42-address filter over the new blocks only. The command is in the `measure_the_first_walk` test, which prints each pass's `eth_getLogs` and other requests (`COLLECTIONS_V1_MEASURE_PASSES` stops it early).

## Marketplace catalogue mirror proof

`catalyst_catalogue_mirror_proof_outcomes_total` counts mainnet proof
attempts by `mirror`, `network` and `outcome`. `proof_rejected` records
a conflicting chain or block hash. `graph_unavailable` records a failed
graph query or a graph response without usable block metadata.
`proof_skipped_rpc_unavailable` records an unpinned graph pass started
after the RPC retry window elapsed. `rpc_auth_failed` counts HTTP 401/403
responses and accompanies the error log for the configured RPC.

For Ethereum v1 royalty attribution, a local PostgreSQL fixture saved 64
collections and 1,000 items. The tables and indexes occupied 32 KiB and
168 KiB. Publishing an unchanged full sweep took 0.56 ms for collections
and 1.89 ms for items on the measured run. The publication leaves unchanged
attribution rows in place and replaces the rows a completed sweep changes.

## Unfiltered Marketplace order browse (2026-11-27)

A local PostgreSQL 17.6 fixture on Apple Silicon held one million legacy
orders, including 130,435 stored open and 10,032 unexpired open rows, and
25,300 v3 trades with 5,060 active listings. Each repetition for
`/v1/orders?first=24` ran the prepared legacy page, both per-network capped
counts, and the v3 leg. With forced generic plans over 20 warmed
repetitions, the median went from 787 to 119 ms, p95 from 1,973 to
160 ms, and the maximum from 1,978 to 160 ms. The first execution before
and after took 813 and 144 ms. Eight order runs in a mixed four-client
probe had a median of 200 ms and a maximum of 228 ms.

`EXPLAIN (ANALYZE, BUFFERS, SETTINGS)` showed the old v3 leg checking
thousands of listings before sorting. The read now walks candidates in
creation order and checks eligibility before applying the same cap.
The legacy count probes the open-expiry index first and reads unexpired
terminal orders to fill the rest of the cap. All four prepared statements
returned identical rows and counts to the baseline. These are local SQL
timings, with no schema or writer changes.

## Published sales reads

A PostgreSQL 17 fixture held one million synthetic sales across Ethereum and Polygon, plus 32,000 sales for a single seller. The account plan used 40,004 published account totals and the creator plan included 9,900 mints. Timings are SQL execution times from local `EXPLAIN (ANALYZE, BUFFERS)`, measured with the publication present. The request statement budget is 10 seconds.

| Read | Execution time |
| --- | ---: |
| First sales page in timestamp order, with total | 12.0 ms |
| Seller summary, 100 matching sales | 615 ms |
| Buyer page, 34 matching sales | 2.8 ms |

A larger fixture included 5.3 million collection mint events, 6,200 factory creations and creator transfers, and 5.3 million NFT transfer events alongside one million sales. The accounts page took 8.00 s with 16 MiB of work memory, the lifetime creator ranking took 6.25 s, and an indexed token lookup took 0.282 ms. Creator lookups used the collection replay index in both directions. The separate partial creator wallet indexes occupied 64 KiB each on the fixture.

To measure the creator-position lookups separately, a PostgreSQL 17.6 fixture held 620,000 events across 6,200 collections, including 1,550 creatorship transfers. Two local `EXPLAIN (ANALYZE, BUFFERS)` runs of the preceding and following transfer lookups for 20,000 mint positions took 666 and 895 ms with the replay index. Both lookups used `collection_event_creatorship_position_idx` after it was added and took 52 and 47 ms. The partial index occupied 160 KiB. These runs measure the attribution probes; the rankings also read sales and aggregate results.

The complete default week creator ranking ran on PostgreSQL 17.6 with 5.3 million collection Issue events, 6,200 factory creations, 3,100 creatorship transfers, and one million published sales across both mainnets. The ledger contained 9,900 priced mints, 187 in the week. With 16 MiB of work memory and a 10-second statement timeout, `EXPLAIN (ANALYZE, BUFFERS)` took 7,077 ms when the query enriched all one million sales. Restricting the period read to mints used `sale_asset_read_kind` on both networks and took 690 and 215 ms in two runs. The first run hit 117,261 shared buffers and read 36,712, down from 8,377,416 hits and 153,491 reads. The query returned 100 creators with both completeness flags clear. Older mints supplied cumulative relationships without payment lookups.

The timestamp and price indexes occupied 54 MiB and 114 MiB on the sales fixture. Daily sale counts supply the total for unfiltered pages. Filtered sales count up to the page window cap. Indexed wallet and item lookups narrow rows before enriching them with payment evidence. Accounts, rankings and volume have a 30-second cache keyed by checked publications.

## Wallet activity transfers

A synthetic PostgreSQL 17.6 fixture on a laptop held 5M Polygon and 1M Ethereum transfers in `shop.nft_event` among 400,000 wallets. 30% of them settled a sale, giving 1.83M single-asset executions in the sales ledger. The older half of each network's transfers had no transaction hash. Timings are for the leg's statement on one network: both sides capped at 500, with the public-sale checks against the ledger. The leg runs the two networks concurrently.

| Index | Size | Concurrent build | Build in the migration transaction |
| --- | ---: | ---: | ---: |
| `nft_event_transfer_from_idx` | 601 MB | 35.6 s (first build, cold cache) | 15.7 s |
| `nft_event_transfer_to_idx` | 601 MB | 14.7 s | not measured |

| Wallet | Network | Rows returned | Cached | Cold |
| --- | --- | ---: | ---: | ---: |
| 400 transfers | Polygon | 232 | 8.4 ms | 128 ms |
| 400 transfers | Ethereum | 42 | 9.0 ms | 37 ms |
| 50,000 transfers, 90% of them sales | Polygon | 1,000 | 78 ms | 1.15 s |
| 50,000 transfers, 90% of them sales | Ethereum | 668 | 38 ms | 1.53 s |
| Three random wallets | Polygon | 19 to 25 | 1.1 to 2.2 ms | 18 to 24 ms |

Each side walks its index in the order the activity merges in (time, then position), with no sort step. On Polygon, each side of the 50,000-transfer wallet walked about 4,800 index rows to find 500 transfers that were not sales. Each walked row made one `sale_asset_read_token` probe and, when it found a sale, one probe of the execution's primary key. Other agents' builds were running during these measurements.

### LAND and Estate transfers

A synthetic PostgreSQL 17.6 fixture on a laptop held 1.05M `land_authz.authz_event` rows over blocks 4.9M to 20.65M. A third of them were transfers: 280,000 LAND and 70,000 Estate among 60,000 wallets, a tenth of them mints. 109,500 of the transfers settled a priced MANA sale in their transaction. The heavy wallet had 2,500 transfers on each side, and 90% of them were sales. The table took 293 MB.

| Index | Size | Concurrent build |
| --- | ---: | ---: |
| `authz_event_transfer_from_idx` | 32 MB | 0.48 s |
| `authz_event_transfer_to_idx` | 32 MB | 0.46 s |
| `authz_event_transfer_unrepaired_idx`, every transfer due (right after the step) | 7.5 MB | 0.22 s |
| `authz_event_transfer_unrepaired_idx`, none due | 8 kB | 0.14 s |

| LAND read, both sides capped at 500 | Rows | First run | Repeated |
| --- | ---: | ---: | ---: |
| 23 transfers | 15 | 6.5 ms | 0.9 ms |
| 5,000 transfers, 90% of them sales | 500 | 94 ms | 17 to 30 ms |

On the heavy wallet each side read its 2,500 rows through its index (a bitmap scan and a sort at that size) and made one `sale_asset_pkey` probe per row. The readiness probe (`EXISTS` over the due set) took 0.05 ms.

The repair, with every transfer due: the next due row took 4.1 ms; one 50,000-block window's `UPDATE` of 1,111 rows took 20 ms; the check for an unfilled row took 0.4 ms. The fixture's history is about 322 windows. Each costs two header reads and at least one `eth_getLogs`. At the defaults (`LAND_AUTHZ_INDEX_REQUESTS_PER_PASS` 50, which counts the repair's header reads, a pass every 120 s), an idle pass leaves room for about 15 windows. The repair then takes about 22 passes, some 45 minutes, unless the endpoint narrows the ranges. That estimate is not a measurement against a real endpoint. Other agents' builds were running during these measurements.

## Sepolia LAND authorization replay

On 2026-11-26, a single leased `catalyst index` worker at
`bb10e59289e1311ed88476c8692534334963de5c` replayed Sepolia LAND and Estate
into an empty PostgreSQL 17.6 database using Decentraland's public Sepolia
RPC. The first publication took 2.70 s from worker launch, excluding
compilation and initial schema application. The completed job pass took
1.99 s. It ran immediately on startup with 12 confirmations and a
200-request per-pass budget.

| Measurement | Result |
| --- | ---: |
| First block | 3,831,219 |
| Published block | 12,217,105 |
| Blocks covered, inclusive | 8,385,887 |
| Authorization events | 6,658 |
| Estate metadata events | 52 |
| Token rights | 4,116 |
| Account rights | 51 |
| Estate memberships | 851 |

The published cursor hash matched the Sepolia header at 12,217,105
(`0x3b4a18568ecdfb3989f148294617774906ae5db307118e560094b173b9f45e53`).
Separate measurements depend on RPC range limits and latency.

## Default Marketplace catalog reads (2026-11-27)

The baseline is `de3fb2cd3`. A disposable PostgreSQL 17.6 database on
Apple Silicon used 128 MiB shared buffers and 4 MiB work memory. The
synthetic fixture held 3.5M NFTs, 102,000 items, 5,100 collections, and 1M
legacy orders. Of 130,435 stored open orders, 10,032 were unexpired at the
query clock.

The table gives milliseconds for the prepared SQL, before and after,
over 20 warmed repetitions with one client and forced generic plans.
Every request has `first=24`. The first execution follows statement
preparation, with buffers carried over from setup and earlier reads.

| Read | First before / after | p50 before / after | p95 before / after | Max before / after |
| --- | ---: | ---: | ---: | ---: |
| Catalog, default | 4,594 / 1,442 | 1,994 / 718 | 2,393 / 1,211 | 2,433 / 1,457 |
| Catalog, Polygon | 1,766 / 370 | 2,344 / 313 | 3,511 / 443 | 3,633 / 456 |
| Catalog, Polygon wearables on sale | 2,038 / 216 | 1,533 / 201 | 1,956 / 245 | 2,346 / 254 |
| Catalog, newest | 1,489 / 170 | 2,361 / 142 | 4,022 / 190 | 4,375 / 190 |

`EXPLAIN (ANALYZE, BUFFERS, SETTINGS)` of custom and generic plans
showed that the combined legacy aggregation could scan the NFT table and
evaluate Ethereum wearable joins for Polygon orders. Each network now
resolves its own item key and probes the NFT primary key. Ethereum
resolves the wearable by URN once it knows the token. All four paired
prepared statements returned identical rows and counts. The queries use
existing indexes and add no storage or write work.

These measurements cover SQL on the local fixture. End-to-end latency
after deployment has yet to be measured.

## Latest sale per token

The fixtures below are synthetic, measured on a laptop with PostgreSQL 17.6 while other builds were running. Treat the ranges as an order of magnitude.

The backfill was measured on a ledger of 3M Polygon and 600,000 Ethereum single-asset sales spread over each network's history, 5% of them unpriced. Each 5,000,000-block window is one `INSERT ... SELECT DISTINCT ON` into `shop.nft_last_sale`, the statement a fold pass runs:

| Network | Windows | Rows per window | Time per window | Total |
| --- | ---: | ---: | ---: | ---: |
| Polygon (blocks 15.2M to 78.2M) | 13 | 226,000 (the last 136,000) | 3.3 to 10.3 s | 78 s |
| Ethereum (blocks 4.9M to 20.9M) | 4 | 50,000 (the last 32,000) | 0.7 to 1.8 s | 4.6 s |

The 525,000 resulting rows took 518 MB, with the primary key and the two secondary indexes the table had then. The index on `(network, sold_at DESC)` has since been dropped.

`/v1/nfts?sortBy=recently_sold` was measured on a seed shaped like the one in DATABASE.md's /v1/nfts measurements: 3M Polygon tokens over 100,000 items of 5,000 collections, 200,000 Ethereum v1 tokens, 300,000 names and 90,000 parcels. 0.2% of the tokens had a legacy sold time, and the latest sales covered 20% of Polygon tokens, 10% of v1 tokens, 5% of names and 2% of parcels. Each page is `first=24`, built by `build_nfts_query_for`, three runs:

| Page | Legacy orders only | With the latest sales | Capped total |
| --- | ---: | ---: | ---: |
| `category=wearable` | 3.6 to 5.4 s | 4.5 to 7.1 s | 173 to 319 ms |
| no category (collections, names and LAND) | 4.1 to 4.8 s | 6.0 to 7.0 s | 442 to 604 ms |

Both plans spend most of their time reading every held token: a parallel sequential scan of the 3M Polygon tokens, joined to their items and collections, then a top-N sort. With the latest sales, the Ethereum v1 source probes `nft_last_sale_pkey` once for each of its 200,000 tokens. Measured with `EXPLAIN ANALYZE` on this machine, one wearable page took 6.7 s with legacy orders only and 6.1 s with the latest sales, so the difference stayed within the noise of the shared machine. The recently sold sort now walks the sold tokens instead, through `nft_last_sale_recent_idx`; see [Recently sold](#recently-sold).

## Recently sold

`/v1/nfts?sortBy=recently_sold` walks each source's sold tokens newest first in two parts, then reads its unsold tokens by contract and token:
- the latest sales, through `nft_last_sale_recent_idx`;
- the legacy `OrderSuccessful` times, through `nft_sold_order_idx`, or for LAND from the registries' orders in `nft_event_successful_order`, aggregated per token.

It walks when the page's capped total counts at least 400 candidates per row of the window (`first + skip`), or more than the cap with a window of at most 12,500, and when each sold part of a collection or NAME source fills the window within a budget of half the capped total in index rows. Otherwise, and for an owner's page, it sorts the candidates whole. The fixture is synthetic, measured on a laptop with PostgreSQL 17.6 while other builds and test suites ran, so treat the ranges as an order of magnitude.

The seed is the one in [Latest sale per token](#latest-sale-per-token):
- 3M Polygon tokens over 100,000 items of 5,000 collections;
- 200,000 Ethereum v1 tokens, 300,000 names, 90,000 parcels and 2,000 estates;
- a legacy sold time on 0.2% of the tokens (6,400 held), the 6,000 on Polygon all emotes;
- 50,000 Ethereum `OrderSuccessful` events, 30,000 of them on LAND and estates;
- latest sales on 20% of Polygon tokens (599,574), and 10% of v1 tokens, 5% of names and 2% of LAND (36,933 on Ethereum);
- three items marked `unique`, with 90 tokens between them;
- one more collection of 20,000 tokens, five of them sold, all five older than every other Polygon sale.

Each page is `first=24`, the route's sources, both networks' latest sales admitted unless noted. Each read was timed in three runs after a warm-up, with the whole sort of the same page run between them, so the walk reads a cache the sort has churned. "Walk" is the walked page, one statement; when the walk misses its window, "Sorted after" is the whole sort that follows it. "Sorted whole" is the page read without walking. Both answered the same page in every case. The capped total, which both run first, is not in the times.

| Page | Capped total | Walks | Sorted whole | Walk | Sorted after |
| --- | ---: | --- | ---: | ---: | ---: |
| `category=wearable` | 100,001 | yes | 2.1 to 2.2 s | 23 to 27 ms | |
| no category (collections, names, LAND) | 100,001 | yes | 2.6 to 2.8 s | 137 to 143 ms | |
| `category=wearable`, `skip=200` | 100,001 | yes | 2.3 to 2.4 s | 38 to 41 ms | |
| `category=wearable`, `skip=1000` | 100,001 | yes | 2.7 s | 69 to 87 ms | |
| `category=wearable`, `skip=10000` | 100,001 | yes | 3.1 to 3.4 s | 0.77 to 0.78 s | |
| no category, `skip=10000` | 100,001 | yes | 3.2 to 3.7 s | 1.1 to 1.2 s | |
| `category=wearable`, `skip=12000` | 100,001 | yes | 2.8 to 3.6 s | 0.78 to 0.84 s | |
| `category=wearable`, no latest sales admitted | 100,001 | yes | 1.6 to 1.7 s | 17 to 18 ms | |
| no category, no latest sales admitted | 100,001 | yes | 2.0 to 2.4 s | 88 to 126 ms | |
| `category=wearable&isOnSale=false` | 100,001 | yes | 2.7 to 2.8 s | 60 to 71 ms | |
| `category=emote` | 100,001 | yes | 0.53 to 0.63 s | 0.48 to 0.49 s | |
| `category=emote`, `skip=1000` | 100,001 | yes | 0.51 to 0.58 s | 0.41 to 0.66 s | |
| `category=emote`, `skip=10000` | 100,001 | misses | 0.58 to 0.67 s | 0.65 to 0.71 s | 0.62 to 0.75 s |
| `category=emote`, `skip=20000` | 100,001 | no (window) | 0.61 to 0.69 s | | the same |
| `category=wearable`, `skip=50000` | 100,001 | no (window) | 2.9 to 3.2 s | | the same |
| `category=ens` | 100,001 | yes | 0.19 to 0.22 s | 2 ms | |
| `isLand=true` | 90,000 | yes | 0.34 to 0.37 s | 142 to 160 ms | |
| the collection of 20,000 tokens | 20,000 | misses | 55 to 62 ms | 5 ms | 53 to 56 ms |
| its creator | 20,000 | misses | 86 to 88 ms | 34 to 36 ms | 86 to 93 ms |
| `itemRarity=unique` | 90 | no | 79 to 82 ms | | the same |
| one creator | 1,600 | no | 71 to 103 ms | | the same |
| one collection | 600 | no | 6 to 9 ms | | the same |
| `search` | 330 | no | 0.30 to 0.31 s | | the same |
| an owner's 16 tokens | 16 | no (owner) | 0 to 3 ms | | the same |

Four choices come from earlier runs of this benchmark. The emote miss at `skip=10000` costs more than it did when each part was bounded by a sale time (0.41 s then): it now reads its whole budget of rows before it gives up, which is what keeps the cost bounded whatever the ties.

**The latest-sale part looks each sale's token up on its own** (a `LATERAL` lookup by primary key, `OFFSET 0`). Joined on `(network, contract_address, token_id)`, the planner multiplied the three columns' selectivities. It expected 419 Polygon tokens where 179,832 matched the wearable filter. At a window of 1,024 it read the window as most of the matches, so it hash-joined all 3M tokens to the 600,000 sales and sorted them (6.1 s of the part's 6.7 s). At a window of 24 it walked the index. Looked up one sale at a time, the estimate is one token per sale, and the walk holds at any depth: `skip=1000` took 3.3 to 5.1 s with the join and 123 to 169 ms with the lookup.

**The walk needs enough candidates.** A walk reads about `window × held / candidates` rows of each part before it fills the window; a whole sort reads each candidate once. Walking the 90 `unique` tokens read every latest sale and every unsold token in key order: 13.6 to 14.8 s, against 0.27 s sorted whole. The two meet near `sqrt(window × held)` candidates. That is about 9,000 for 24 rows on 3.5M held tokens, hence 400 per row of the window.

**The walk needs a budget.** The candidate count says nothing about how many of them sold. Walked without a bound, the collection of 20,000 tokens read every Polygon sale before giving up, as the `unique` tokens did. With the budget, a part that does not fill its window within half the capped total in index rows stops the walk, and the page is sorted whole. The deepest windows past the cap don't walk: the emotes are about a tenth of the sales, so at `skip=20000` they need more rows than the budget, and the walk would only add its cost to the sort's.

**The budget is a count of index rows.** Each part reads its index's first rows under a `LIMIT`, a constant the planner can estimate, before any filter. An earlier version bounded each part by the time of its network's budget-th index row instead. Written as a subquery inside the page, that bound's value was unknown to the planner. For the Polygon legacy part (6,000 rows, none of them a wearable), it then chose a parallel scan of every item's tokens, 2.5 s, over the partial index, and `skip=10000` took 3.3 s. Read first as a literal, the time held the plan, but every row sharing that time passed the bound: 20,000 sales at the same second would all be read under a budget of 100.

The indexes, built `CONCURRENTLY` on this seed:
- `nft_last_sale_recent_idx` on `(network, sold_at DESC, contract_address, token_id)`: 73 MB over 636,507 rows (the table is 79 MB, its primary key 89 MB), 1.4 s;
- `nft_sold_order_idx` on `shop.nft (network, sold_at DESC, contract_address, token_id)`, partial on held tokens with a legacy sold time: 768 kB over 6,400 rows, 1.2 s, against a 728 MB table.

## Credits metrics snapshot

The `credits-settlement` pass sets the settlement gauges (`credits_pledges_live`, `credits_pledged_usd_cents`, `credits_usages_unpaid`, `credits_revocations_pending`) from one statement, `shop::credits::metrics::SNAPSHOT_SQL`, every `CHECKOUT_WORKER_INTERVAL_SECS` (30 s by default). It was measured on a synthetic seed on a laptop with PostgreSQL 17.6, three runs of `EXPLAIN (ANALYZE, BUFFERS)`:

| Rows | Live pledges | Unpaid uses | Pending revocations | Time |
| --- | ---: | ---: | ---: | ---: |
| 100,000 authorizations, 90,000 indexed uses (all but 25 settled), 90,000 revocations | 2,000 | 25 | 9,000 | 42 to 45 ms |

The live pledges and their cents are read through the partial `authorizations_expiry_idx` (2,000 rows), and the pending revocations through `revocations_due_idx`. Each is bounded by what is live, not by the table. The unpaid count costs most of the time: a parallel hash anti-join of every indexed use at or below the fold against `usage_settlements`, then a primary key probe of the authorization for each unsettled use. It grows linearly with the uses the index has stored, about 0.5 ms per 1,000.

## Marketplace sales day rollups

On PostgreSQL 17.6, a temporary table held 200,000 executions on one network,
spaced five minutes apart. Selecting one UTC day returned 288 rows. With the
network and block primary key, PostgreSQL scanned 1,667 local buffers and
filtered out 199,712 rows. The index on network and the UTC day expression
reduced that read to six local buffers and three shared buffer hits. The index
occupied 1,458,176 bytes. This measures the day lookup that precedes rebuilding
its aggregates.

## Credit revocation sweep

A disposable PostgreSQL 18 table held 200,000 revocations across two chains,
including 4,000 pending or sent rows. After `ANALYZE`, the worker's hash-range
page of 20 used 2,015 local buffer accesses with the creation-time partial
index. It filtered and sorted 1,503 candidates. Adding the partial index on
`(chain_id, credit_hash)` reduced the page to 22 buffer accesses under both
custom and generic prepared plans. The upper-bound query went from 44 buffers
to three with a custom plan and from 2,015 to three with a generic plan.
The new index occupied 424 KiB. These synthetic plans measure read work and
index size. Production cardinalities and write costs remain unmeasured.

## Credit studio budget reads

A local PostgreSQL 17 database held the studio tables as `schemas/60_ops.sql`
defines them, with 200 studios, 800 operators and 200,000 budget entries over
365 UTC days (one deposit in ten, the rest grants naming a ledger row). Each
figure is one warm `EXPLAIN (ANALYZE, BUFFERS)` run after `ANALYZE`.

| Query | Plan | Work |
| --- | --- | --- |
| The day's new budget (the ceiling, under the deposits lock) | Bitmap scan of `studio_budget_entries_inflow_idx`, 55 rows | 39 buffers, 0.06 ms |
| A studio's balance or history total (1,000 entries) | Index-only scan of `studio_budget_entries_studio_idx` | 659 buffers before `VACUUM` (1,000 heap fetches), 16 after; 0.2 ms |
| A studio's history page of 50, newest first, with each grant's recipient | Backward scan of `studio_budget_entries_studio_idx`, incremental sort on the id tie-break, the ledger key per row | 250 buffers, 0.19 ms |
| An idempotency probe | `studio_budget_entries_idempotency_uidx` | 9 buffers, 0.03 ms |
| The studio list (every studio's figures) | Parallel sequential scan and aggregate of every entry | 3,840 buffers, 14 to 18 ms |

The studio and idempotency indexes occupied 13 MB each, the ledger index
4.9 MB and the inflow index 648 kB, beside a 30 MB table. The list reads every
entry, so its cost grows with the ledger; it is an operator route. Write
costs, production cardinalities and latency are unmeasured.

## Seasonal photo progress

A disposable PostgreSQL 18 fixture with 1,000,000 photo-goal receipts and
500,000 retained Camera events measured the wallet/season/week `photosTaken`
query with `EXPLAIN (ANALYZE, BUFFERS)` after `ANALYZE`. Returning two counted
photos used a parallel sequential scan in 26.199 ms. The partial index on
`(address, season_id, week_id, ordinal) WHERE outcome = 'counted'` selected an
index scan in 0.030 ms with five shared-buffer reads. Its measured size was
67 MB. Including `photo_id` increased it to 82 MB without improving this
sample's execution time (0.031 ms), so the index has no included columns.

The global oldest-unprocessed-event diagnostic took 182.756 ms with one
pending event and 173.302 ms with none. Replacing the aggregate with an
ordered limit still selected a hash anti-join and did not reliably reduce
the work. The worker samples this diagnostic separately from photo
processing, at most every five minutes, with a query deadline; diagnostic
failure does not roll back completed work or prevent that pass's processing.

These are single local samples with synthetic distributions and uncontrolled
cache state. They establish the selected access path and the cost of the
diagnostic in that fixture, not production latency, throughput, index write
cost or capacity.

## Seasonal room goal processing

A local PostgreSQL 17 temporary-table fixture held 200,100 room facts (101
pending) and 20,000 visits (101 with timers). After `ANALYZE`, `EXPLAIN (ANALYZE,
BUFFERS)` selected the partial pending index to take 100 facts (0.044 ms), the
session index to find a matching leave (0.025 ms), and the partial due index
to take 100 timers (0.032 ms). The pending and due indexes were 16 KiB each,
and the session index was 26 MiB. In a second fixture with 30,000 event and
30,000 deployment rows, the webhook venue query used the world index
(0.076 ms) or the deployment and coordinate indexes (0.173 ms). The partial
enrollment index returned 100 pending dates from 200,100 login rows in
0.035 ms. These synthetic queries measured local read paths. Production event
volume and write cost remain unmeasured.

## Social notification polling and oversized-event expiry

The explicit `rpc::tests::fanout::notification_and_spill_queries_use_bounded_index_paths`
PostgreSQL test uses its own temporary database and inserts 5,000 community rows,
with 1,250 tracked communities and 2,500 independently cased notification keys,
and 5,000 temporary payload rows with 10 expired entries inside a rolled-back
transaction. The statements run `EXPLAIN (ANALYZE, BUFFERS)` after `ANALYZE`.

The plan selects `community_voice_notifications_pkey` directly
for an ordered raw-key page: 0.027 ms execution, 64 returned rows and four
shared-buffer hits. The ordered expiry selection uses
`event_payloads_expiry_idx`: 0.014 ms execution, 10 rows and 13 shared-buffer hits
including row locks. Both statements cap returned work at 64 rows. The test checks
that these indexes remain usable. Polling cost follows tracked raw keys, which can
outnumber canonical communities; the fixture measures two keys per tracked room.

These small warm-cache plans establish access paths, not production throughput or
capacity for the 512 MiB payload admission bound. Production active-room counts,
large-event frequency, cold-cache behavior, index write cost and maximum-payload
memory/latency remain unmeasured.

## Social mutation burst fanout

The real-NATS tests run an owned broker and two links, with 1,000 updates for one
local subscription. A targeted block burst delivered all 1,000 updates in order,
without origin loopback duplication, through awaited publisher admission and the
synchronous target callback. The community test uses PostgreSQL membership reads,
a pool of eight connections, and 100 interleaved messages on a control subject.

With the 1,000-entry community queue and four concurrent deliveries, the fixture
delivers all 1,000 updates; the 100 control messages arrived in
24.757 ms. A 256-entry serial queue in the same fixture delivered 331 of 1,000
updates, with controls in 16.145 ms. This is one small warm-cache burst fixture,
not a sustained throughput, multi-community fairness or maximum-payload benchmark.
SQL concurrency is capped by the configured pool size; ingress beyond the queue
bound can drop transient events and increments the delivery-drop counter.

The targeted burst harness records readiness, submission, broker observer counts
and both subscription queue lengths. Within the full Social suite it reaches
readiness in 5.007 seconds, finishes submission at 5.021 seconds and observes
all 1,000 frames at both broker links and both queues by 5.027 seconds; community
updates and the 100 controls arrive in 28.482 milliseconds. These are single
samples without controlled system load. The harness has a 10-second delivery
deadline and exact count, ordering and duplicate
assertions; that deadline is not a protocol latency guarantee.

## Ordinary Social RPC writer

The source transport fixture executes the pinned TypeScript transport with a
controlled socket and virtual timers. Its 17 cases cover queue capacity,
backoff, drain callbacks, accepted-once handling, closure and explicit unsupported
configuration edges. A real socket test preserves an 8 MiB profile
URL in a unary response, then stops reading and observes transport retirement
after bounded retries of subsequent responses, in 1.06 seconds. These checks do not measure native uWS buffer behavior, kernel
capacity, sustained throughput or maximum-memory consumption. Accepted bytes are
charged through the existing writer's physical flush; pending-message storage
grows on demand up to the configured finite threshold.

## Private-call RPC UUID boundary

The private identifier replay samples 128 sequential SQLx executions of the bound
scalar `SELECT $1::text::uuid` on the existing test pool. On local PostgreSQL 18.6,
over loopback with a warm statement cache and concurrent builds, one sample
measured p50 133 microseconds, p95 848 microseconds and maximum 5,001
microseconds. These timings include pool acquisition and the query round trip.
The sample adds no application-table reads or row locks. Hardware details, p99,
sustained throughput and production-load latency are not recorded. The RPC
performs this extra query; the measurement does not claim source performance parity.

## Community ranking and compliance cache

A disposable PostgreSQL 18.6 database uses the schema and query text in
`services/catalyst/social/src/communities/ranking.rs` and `compliance.rs`,
including the cache expiry recheck. The fixture contains 100 communities, 100,000 memberships, 10,000 posts,
1,000 community places, 100,000 photos, 100,000 ended local event occurrences and
100,000 compliance results. These are single `EXPLAIN (ANALYZE, BUFFERS)` samples
from a developer machine during a concurrent build, with uncontrolled cache state.

| Statement | Fixture result | Execution time |
| --- | --- | ---: |
| Ranking facts for 100 communities | 100 rows | 100.389 ms |
| Empty pending-photo page, partial index | 0 of 100,000 | 0.045 ms |
| Same empty page, index scans disabled | 0 of 100,000 | 30.039 ms |
| Pending-photo page, partial index | 50 of 100,000 | 0.166 ms |
| Photo receipt and counter fanout | 100 communities | 11.098 ms |
| Unbounded ended local occurrence scan | capped at 100 of 100,000 | 538.459 ms |
| Fresh compliance result by primary key | 1 of 100,000 | 0.057 ms |
| Expired compliance deletion | capped at 1,000 of 100,000 | 85.488 ms |

The partial index avoids a full photo-history scan when no repair is due. Cache
reads use the primary key; bounded expiry selection uses the expiry index.
Ranking aggregates depend on membership data. The event collector
bounds source reads and recurrence expansion as measured below. Repeated latency
percentiles, production distributions,
concurrent writer throughput, cold-cache performance, hardware characterization,
receipt growth and replica lag remain unmeasured. These samples establish query
paths and bounded output sizes, without establishing production capacity.

## Bounded local event collection

A disposable PostgreSQL 18.6 comparison uses actual query text, 100,000 already
receipted ordinary events and then 100 additional schedules of 1,000 recurrence
positions each. Three warm `EXPLAIN (ANALYZE, BUFFERS, WAL)` executions measured:

| Work | Execution times |
| --- | --- |
| Whole-catalogue scan, ordinary history | 1,084.812 / 1,006.240 / 1,131.275 ms |
| 100-key page, ordinary history | 0.342 / 0.353 / 0.374 ms |
| Ordinary occurrence window | 0.881 / 0.479 / 1.055 ms |
| Whole-catalogue scan, history plus long schedules | 2,874.111 / 1,851.613 / 2,495.208 ms |
| 100-key page, long schedules | 3.243 / 3.316 / 3.241 ms |
| 10,000-position window, long schedules | 89.958 / 59.554 / 179.075 ms |

These compare bounded page work with whole-catalogue work, rather than claiming
that one page completes the catalogue. Materializing the bounded schedules and
parsed occurrence rows avoids repeated JSON extraction within the page. Receipt
checks use the existing primary key and attendance counts run only for new
candidates. It needs no schema or index. A 100-receipt/counter batch takes 17.623 ms
of SQL execution and 114.364 ms wall time, writing 79,611 WAL bytes. Cache state,
concurrent builds and per-query timings do not establish production throughput;
full-cycle latency also depends on the three-second pass budget, the 1,024-page
cap, the 100-new-receipt cap, recurrence lengths and the minute cadence.

The actual Rust worker, using an isolated full-schema database with
100,000 completed ordinary events and receipts, finished one cycle in two passes
and 5,550.080 ms of active work without receipt/counter writes, without sleeping
between passes. Adding the normal 60-second interval gives a derived
65.550-second cycle for this sample, rather than a measured production freshness
bound. The page timings above exclude these transaction and driver round trips.

## Pending photo repair scheduling

A disposable PostgreSQL 18.6 fixture with 100,000 pending
photos across 200 owners uses the actual storage adapter with an absent valid
content hash. This sample excludes the one-second row budget measured below.
The two age cohorts read 200,000 keys through 2,002 bounded pages and perform
100,000 storage probes. At 32 pages per pass with one shared three-second budget,
the complete dual sweep takes 63 passes and 30.155 seconds of active work: median
364 ms, p95 983 ms and maximum 1.651 seconds per pass. No pass times out; there
are no storage errors or false publications. Adding the
existing 60-second sleep between passes gives a derived first-sweep duration of
62 minutes 30.155 seconds; the minute sleeps are not part of the measured time.
Cold retries add an hour after completion of their sweep; the ten-minute age
split and retry policy are candidates rather than an accepted freshness target.

The pending index returns 100 keys in 0.136 ms at the start and 0.126 ms midway,
with eight and nine shared buffer hits. It uses no additional index. A separate 1,000
actual publications measure median 1.671 ms and p95 7.968 ms per transaction;
markers, receipts and counter increments each totaled 1,000. These warm local
samples exclude production storage/network latency, sustained incoming writes,
cold caches and deployment interruption. They show bounded work and its backlog
cost; they do not establish production throughput or a 60-second repair bound.

The one-second row-budget regression uses an actual held PostgreSQL row lock. A
healthy later photo publishes in 1,029.286 ms while the blocked row stays pending
and its lock stays held; PostgreSQL activity shows the blocked UPDATE ends before
unlock. The shared pass deadline also keeps its tail with a shorter 500 ms
budget. The repair-only SQL timeout's setup cost and physically stalled storage
are not measured separately.

## Shared social registry profile cache

A disposable PostgreSQL 18.6 database populates a temporary cache relation
with 100,000 synthetic rows and the canonical primary/expiry indexes. Single
`EXPLAIN (ANALYZE, BUFFERS)` samples during a concurrent build measured:

| Statement | Fixture result | Execution time |
| --- | --- | ---: |
| Fresh profile key set | Primary-key index scan | 5.919 ms |
| Bulk conflicting profile write | 1,000 keys | 9.200 ms |
| Expired selection and deletion | capped at 1,000 rows | 2.465 ms |

Expiry selection uses the expiry index and deletion used tuple identifiers. The
reader always excludes expired rows independently of this sweep. These samples
show the selected access paths and bounded cleanup output; they do not establish
production latency, a sustained 1,000-key-per-second reclamation rate or storage
capacity. Production key distributions, concurrent writers, repeated percentiles,
cold-cache costs and the existing source-cache cutover remain unmeasured.

## Referral recent-invitation read

A PostgreSQL 18.6 TEMP fixture holds 100,000 referral rows with the
`(referrer,status)` index. The post-insert query orders one referrer's history
by creation time and returns at most two rows. Single local
`EXPLAIN (ANALYZE, BUFFERS)` samples use a bitmap index/heap scan and top-N heapsort:
90 matching rows take 0.086 ms; a skewed referrer with 10,000 matching rows take
0.870 ms. These are query execution observations from a developer machine during
a build. They do not include HTTP/Slack time, prove sustained throughput or bound
scan work as one referrer's history grows. No additional index is inferred from
these samples.

The community SNS replay executes 18 source broadcaster cases,
two SNS adapter result cases and twelve source event creators. The local target
uses its real membership SQL and the official AWS SDK with a local HTTP stub.
For a rename of 1,101 members, both sides fetch twelve membership pages,
create twelve recipient messages and make two PublishBatch requests (ten and
two entries). A 205-member deletion excludes its owner and emits three
recipient messages. Partial result IDs and full failed chunks are compared;
failed broadcasts do not undo HTTP ban or voice START. This establishes the
sampled audience, batching and request shapes. SNS service latency, retry timing,
concurrent membership mutation across pages and sustained throughput are not measured.

## Social HTTP body boundary

The body replay uses social-service-ea at
`b6b2cefefdf727b2ed47ddb887e77d226a41eea6`, Node 26.12.0 and its installed
`@dcl/http-server` 2.3.0. It compares 18 startup inputs and 44 HTTP cases: 39 with
a 64-byte cap and five multipart cases with a 2 MiB cap. The target uses real
Hyper, the actual community and RPC HTTP routes, the shared unread-body drain
and the outer trailing-slash normalizer. The replay compares status, complete
body and every application header; date and transport framing/hop headers are excluded. Declared oversize and consumed-stream overflow explicitly checked the
close header. A declared oversized request received its response without an
upload body. Another domain accepted 65 bytes behind the same outer layers.

The larger-cap fixture is valid native multipart with 1,050,055 wire bytes,
including a 1,024-byte thumbnail and an unused padding field. Declared, chunked
and header-only variants all reached the independent multipart 400. Thumbnail
limits are checked separately while the total stays below 1 MiB. This replay
does not measure RSS, concurrent upload capacity, throughput, or unread chunked
connection-close timing. The latter remains a separate source/target audit.

## Private voice pending and retained media

An owned PostgreSQL fixture contains 10,512 media calls and 21,024
participants, including 512 historical calls for one wallet. Each media
participant is past its initial-connection TTL; no pending calls remain.
The target INSERT statement is extracted from the Rust write path. Fifty
local starts per wallet measured BEGIN, explicit READ COMMITTED, sorted
participant advisory locks, INSERT and COMMIT; cleanup is excluded.

| Wallet history | Median start | Local p95 | Final INSERT plan execution |
| --- | ---: | ---: | ---: |
| One retained call | 0.419 ms | 0.663 ms | 0.026 ms |
| 512 retained calls | 1.798 ms | 2.367 ms | 1.755 ms |

Both plans use the nonunique caller/callee indexes. The small history uses the
participant primary key; the skewed case uses a hash anti-join against the
participant set. Repeated insert/delete cleanup left dead index entries during
the sample. Rust compilation runs concurrently and cache state is uncontrolled.
These samples establish the access paths and local command cost; they do not
compare source performance or establish production capacity, contention latency,
cold-cache behavior or a service-level bound. Explicit isolation adds one SQL
command to pending creation so waiting writers observe the prior winner even
under a role whose default isolation is REPEATABLE READ.

## Community voice read separation

A PostgreSQL 18.6 TEMP fixture holds 100,000 synthetic voice-presence
rows across 10,000 communities with the `(community_id,address)` primary
key. Single local `EXPLAIN (ANALYZE, BUFFERS)` samples during a concurrent build
measured an all-community status aggregation at 21.030 ms (100,000 rows
scanned), the existing comms bulk query for 50 community IDs at 0.999 ms (500 rows
through bitmap primary-key/heap scans), and the all-active-room query at
17.344 ms (100,000 rows scanned). Ordinary community pages use the bulk
query after core SQL; active filters require the all-room query.

These are individual query observations on a temporary relation with matching
columns and key, not production latency or a sustained-throughput measurement.
They exclude HTTP, profile enrichment and queueing, and do not establish cold
cache behavior, repeated percentiles or capacity under concurrent presence
writes. It needs no index or schema change.

## Community child deletion indexes

An owned PostgreSQL 18.6 scratch database holds 10,000 communities and 100,000
rows in each of `community_voice_notifications` and `community_ranking_events`,
with ten children per selected community. Five samples per operation use
`EXPLAIN (ANALYZE, BUFFERS, WAL, FORMAT JSON)` and rolled back each mutation.

| Operation | Without community-id indexes, median | With indexes, median |
| --- | ---: | ---: |
| Delete ten notification rows | 4.384 ms | 0.128 ms |
| Delete ten ranking receipts | 4.499 ms | 0.127 ms |
| Delete parent with both cascades | 10.142 ms | 1.603 ms |
| Insert 10,000 notification rows | 39.582 ms | 48.747 ms |
| Insert 10,000 ranking receipts | 47.042 ms | 86.380 ms |

Without the indexes, child deletion uses sequential scans discarding 99,990
rows; with them, bitmap scans selecting ten rows. Each index occupies 2,072,576 bytes.
The insertion medians increase by 23.2% and 83.6%, respectively. These warm,
synthetic samples establish the tradeoff for this fixture; they do not establish
production latency, sustained throughput or a service-level bound.

## Community mutation coordination

A PostgreSQL 18.6 synthetic fixture used eight workers, 2,000 operations per sample
and three warm samples. Each operation runs two independently committed SQL
statements. With no guard on one community, median/p95 are 0.182/0.331ms;
with its session advisory guard, 1.029/1.189ms; guards over eight communities
measured 0.269/0.427ms. These isolate SQL coordination cost and exclude HTTP
signatures, Rust/SQLx prepare costs and external awaits. They establish no
production capacity or service-level bound.


## Referral lifecycle step on shipped main

A disposable PostgreSQL 18.6 database holds a shipped-main-shaped
`social.referral_progress` with 1,000,000 rows (205 MB with its primary key),
a quarter of them `tier_granted`. Step `0021_social_referral_lifecycle` runs in one
transaction under `psql` timing on a developer machine during a concurrent build:

| Statement | Execution time |
| --- | ---: |
| Add `id` with `gen_random_uuid()` default (table rewrite) | 3,214.949 ms |
| Add the five other columns (constant defaults or nullable) | 0.867 ms total |
| Mark 250,000 granted rows | 5,436.449 ms |
| Build the unique `id` index | 494.525 ms |
| Step `0023` nullable `reward_tier` | 0.181 ms |

The table holds 301 MB afterwards. The rewrite and index build hold ACCESS
EXCLUSIVE on the table; the cutover already stops every writer. One warm sample
scales with row count and storage; it does not establish production duration.

## Referral canonical IP quota comparison

PostgreSQL 18.6 TEMP data holds 100,000 four-column referral
probe rows with the existing `(referrer,status)` index shape. Seven warm
`EXPLAIN (ANALYZE, BUFFERS)` samples after warmup compare text equality with
guarded inet equality including mapped IPv4 aliases. Median execution times:

| Matching referrer prefix | Text equality | Guarded inet equality | Local buffer hits, both |
| --- | --- | --- | --- |
| 10 rows | 0.004 ms | 0.006 ms | 3 |
| 1,000 rows | 0.053 ms | 0.183 ms | 16 |
| 10,000 rows | 0.520 ms | 1.819 ms | 135 |

Both comparisons use the same index access path and buffer counts; canonical
historical comparison adds conversion work within the selected prefix. This synthetic
probe does not measure full attribution writes, transaction contention,
providers, cold data, production distributions or a service-level bound.
No index or historical-row rewrite is inferred from these observations.

HTTP method labels cover nine standard methods in each source transport casing
(uppercase for ordinary HTTP, lowercase for uWS), plus `other`; WebSocket close
labels use a finite standard/source allowlist plus `other`. Per-protocol open
counts update under the existing socket-map mutex without scanning that map.
These are bounded label/state invariants, not measured throughput improvements.
The community coordination benchmark above excludes the bounded SQLx
acquisition setup and does not measure it.

## Snapshot inactive-entities count

The snapshot generator's port of upstream's inactive-entities check counts, at
each pass (every `SNAPSHOT_GENERATION_INTERVAL_HOURS`, default 6), the active
deployments of every window whose row was generated over four weeks earlier
(`content/src/snapshot_gen.rs` `has_inactive_entities`). On a settled node that
is nearly every window, the year windows included, so a pass counts close to
every active row of `content.deployments`.

Dataset: a synthetic `content.deployments` of 4,042,824 rows (2,730 MB heap)
on PostgreSQL 18.6, Apple M3 Pro, 18 GiB, `shared_buffers` 64 MB, during a
concurrent build. Active rows follow the 14 windows and `numberOfEntities` a
public upstream node listed on 2026-11-05 (1,470,117 active; 707,202 in one
month window, 396,161 in the newest year window), with 1.75 overwritten rows
per active row in each window, uniform `entity_timestamp` inside the window
and random physical order. Rows carry a three-link auth chain and no
metadata (about 675 heap bytes per row), so production rows are wider. 809 `local_entities`
rows, 270 tombstoned. The indexes on the measured columns were those of
`schemas/00_content.sql` (`deployments_entity_timestamp_entity_id_idx`,
`deployments_full_snapshots_ix`). Query: the count with the tombstone
anti-join. The table is `VACUUM ANALYZE`d. Execution times from
`EXPLAIN (ANALYZE, BUFFERS)`: the first run, then five more as
min / median / max.

| Window (active rows) | Existing indexes: plan, buffers | first; min / median / max | With `deployments_active_entity_timestamp_idx`: buffers | first; min / median / max |
| --- | --- | --- | --- | --- |
| year 2025-07 .. 2026-06 (396,161) | parallel seq scan, 349,370 | 7,399; 4,162 / 4,567 / 5,335 ms | index-only, 4,684 | 116; 48 / 51 / 62 ms |
| month 2026-08 .. 09 (707,202) | parallel seq scan, 349,370 | 4,279; 3,280 / 4,078 / 6,386 ms | index-only, 8,259 | 369; 103 / 133 / 148 ms |
| year 2020-01 .. 12 (16,162) | bitmap AND of both indexes, 45,277 | 4,019; 3,709 / 3,816 / 7,371 ms | index-only, 215 | 7.6; 3.2 / 4.0 / 5.4 ms |
| month 2026-06 .. 07 (2,941) | bitmap heap scan, 8,097 | 608; 13 / 20 / 26 ms | index-only, 64 | 2.1; 0.6 / 0.9 / 1.5 ms |

The existing indexes do not serve the count: `deleter_deployment` lives only in
the heap or in `deployments_full_snapshots_ix`, which has no `entity_timestamp`,
so each large window reads the whole heap (2.7 GB here) and a pass several
times that. The partial index `deployments_active_entity_timestamp_idx`
(`entity_timestamp` INCLUDE `entity_id` WHERE `deleter_deployment IS NULL`)
makes each count an index-only scan with no heap fetches after a vacuum. It
holds 133 MB for 1,470,117 active rows and built in 7.8 s.

Write cost, `EXPLAIN (ANALYZE, WAL)` in a rolled-back transaction on the same
table: inserting 10,000 active rows wrote 24,408,070 WAL bytes in 50,261
records with the index and 23,035,141 bytes in 42,820 records without it
(+6 %, about 137 bytes per row). Setting `deleter_deployment` on those rows
wrote 23,941,555 bytes with it and 23,893,327 without: the new row version
leaves the partial index. `catalyst schema migrate` builds the index in its
transaction (not concurrently), holding writes on `content.deployments` for
the build; DATABASE.md gives the concurrent build to run beforehand.

These are single warm-cache samples on synthetic data during other load. They
do not measure production row width, cold reads, the build time or lock wait
on the production table, or a whole pass's total.

## Wallet presence and world quota lookups

On 100,000 synthetic memberships per table,
selective and missing-wallet probes used 3–4 buffers instead of 1,024–1,137
under custom and generic plans. A wallet holding 10% of community rows still
selected a sequential scan with a custom plan. Each index occupied about
2.9 MiB after write probes; inserting 1,000 memberships added about 78 kB of
WAL per table. These are warm synthetic measurements, not production latency
or capacity claims.

On 100,000 synthetic identities,
the quota read used 103 buffers instead of 668, with both custom and generic
prepared plans; the index occupied 5.65 MiB after write probes. Retiring 1,000
new identities increased measured WAL from 221 kB to 332 kB. Production world
sizes, request rates and latency are unmeasured.

Building the two wallet indexes and the retired identity index on the synthetic tables took 64, 65 and 108 ms respectively.

## Sales accounting day and order lookups

A PostgreSQL fixture contained 200,000 Polygon account-day rows across 365 UTC days and 250,000 NFT events with one successful order per 50 events. Each figure is one local `EXPLAIN (ANALYZE, BUFFERS)` run.

| Fold query | Primary key only | With the sales index |
| --- | --- | --- |
| Distinct wallets for two days | Sequential scan filtered 198,904 rows in 8.060 ms | The network and day index read 1,096 matching rows in 1.386 ms |
| First 96 successful orders in a 250,000-block window | Primary key scan filtered 4,704 events in 0.313 ms | The partial order index returned 96 rows in 0.108 ms |

These measurements cover the SQL lookups. Receipt RPC time and production replay throughput remain unmeasured.
