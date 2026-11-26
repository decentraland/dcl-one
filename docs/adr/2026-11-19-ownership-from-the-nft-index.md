# Wearable and emote ownership from the nft index

- Status: Accepted
- Date: 2026-11-19

## Decision

The shop answers who holds which wearable or emote from its own nft index
(`shop.nft`), the fold of the collections' Transfer and v1 Issue logs it walks
on chains 1 and 137. No ownership read asks the collections subgraphs. The
reader lives in the shop (`services/catalyst/shop/src/market/owned_items.rs`)
and serves profiles and the Registry's profile batches, the lambdas wearable
and emote lists, the explorer backpack, `/v1/users`, outfits, the `/v1/orders`
wearable order guard and the wearable and emote pages of `/v1/nfts`.
LAND and Estate listing ownership comes from the land-authz fold. Content deployment
validation keeps its own check, by RPC at the entity's block plus
`shop.usage_grants`.

A v2 item is matched by contract and the item id its token id encodes, so the
profile and orders reads do not wait for the items mirror; a v1 item by its
collection's URN (`shop.collections`) and the wearable id of its Issue log.
When a profile names no token, the first held token is the newest transfer.
The orders guard checks the listing's exact token: a listing whose signer no
longer holds that token drops out even when the signer holds another copy.
An order needs a published open trade-events verdict from a watched venue.
A transfer ahead of the owner fold withholds that token's listings until
publication.

Every read checks the index's cursor in the same `REPEATABLE READ, READ ONLY`
transaction as its rows. Two rules:

- Profiles need the index complete on the networks their items name: the
  whole history walked and a fold published, with no reorg rewind since and
  no later fold found unpublishable (`full_history` and `folded_block` set).
  A fold that still owes a late collection's backfill clears the marker in
  the transaction that commits its rows, so it is never read as complete. A
  fold that owes nothing commits before its pass publishes, so until that
  statement, or until the next pass when the chain head cannot be read in
  between, its rows, later transfers of the same complete walk, are read
  under the previous marker. An index
  that is complete but was published more than 30 minutes ago, as during an
  RPC outage, keeps serving them. An incomplete index fails the read: a
  direct read answers lamb2's 404 for a failed lookup (the singular alias
  an empty profile), and a Registry batch leaves the profile out and
  counts the failure.
- The other readers need the index complete and published within 30 minutes
  on the networks they read (wearables both, emotes Polygon), and the lists
  also the items mirror's first complete pass; otherwise they answer the
  `nft-index` unavailable envelope. The open listings legs of `/v1/orders`
  and `/v1/orders/open-by-items` require ownership for every off-chain NFT
  listing. They count the listings backed by the NFT or land-authz
  publication and report `x-listings-owner-check: checked`. The legacy
  orders continue to use the configured graph readers.

Neither rule compares `folded_block` with `last_block`: each pass advances
`last_block` before it folds and publishes, so that equality is false for part
of every pass while the table still holds a published fold. A reorg rewind
clears `folded_block` and deletes the fold's rows in one transaction, and only
a pass that reaches the head sets it again, so a refill is never read as
complete.

A node without its own walker can serve off-chain listings from the same
published ownership tables when another job updates them. Profile and
outfit inventory readiness follows its own read rules. A mainnet mirror refuses to start
with the index off, with a `TRADE_RPC_URLS` (inline, or read from
`TRADE_RPC_URLS_FILE` once the secrets are resolved) that names no chain 137
endpoint, or, when it fills its own catalogue (`MARKETPLACE_MIRRORS_ENABLED`),
with a blank collections subgraph URL: the walk takes its contracts from the
collections mirror and starts on a network once that mirror completed a
pass there, so without it that network's ownership would never publish.

## Consequences

Ownership no longer depends on a hosted subgraph per request, and an owner of
any size is one query. The answers trail the chain head by the confirmation
depth (12 blocks on Ethereum, Polygon's finality) plus a pass, and the
inventory lists leave out an item the items mirror does not hold yet. Testnet
nodes serve profiles unchecked until the walk reads their chains. LAND,
estates and names on `/v1/nfts` keep reading the marketplace subgraph until
their reads move to the indexes.

A `/v1/nfts` page without an owner reads each source's first `first + skip`
rows in the page's order and pages their union, the collections and names
walking `nft_mint_order_idx` and the on-sale reads starting from the listed
tokens. On a 3.5M-token index the newest wearables take 30 ms, the page
naming no category 291 ms (most of it the LAND source, which reads every
parcel's mint before sorting) and the wearables on sale, among 4,450 legacy
orders and 4,600 v3 listings, 767 ms. A window of 10,000 rows takes 0.9 to
1.6 s and the largest, 100,000 rows, 2.6 to 3.9 s; a sort by last sale reads
every held token (1.6 to 3.3 s). A sort by price or listing time reads
each source's listed rows and then its rows without an order in key order:
0.7 to 0.8 s for the first page, and 2.5 to 8 s for the 100,000-row window
(DATABASE.md, "Wearable and emote ownership workload"). An owner's page
under that sort still looks up the listing of each of its tokens, 7 to 8 s
for a 32,000-token owner.
Production data is unmeasured.

Inventory memos:

- The memos are keyed by each network's published block and time, and a
  pass that reaches the head publishes a new time about every 5 seconds, so
  a memo rarely hits beyond one pass and every request, hits included,
  reads the cursors. The time stays in the key because a fold that
  backfills a late collection can publish new rows at the same block.
  Answers stay correct; only load is affected.

Revisit when the walk covers the testnets, or when a reader needs ownership
fresher than the confirmation depth.
