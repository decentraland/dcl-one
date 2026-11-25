# Legacy Marketplace orders from the nft index

- Status: Accepted
- Date: 2026-11-24

## Decision

The legacy on-chain orders of `/v1/orders`, `/v1/orders/open-by-items` and
the activity feed's orders leg come from the shop's own order book,
`shop.legacy_order`, on a node that walks the nft index. No order read asks
the collections subgraphs there.

The nft walk already keeps every `OrderCreated`, `OrderSuccessful` and
`OrderCancelled` log of the Marketplace contracts in `shop.nft_event`
(Ethereum's, and Polygon's two), whatever asset the order names, so LAND,
estate and NAME orders are in the log beside the collections'. The order
book is one row per order, folded from those logs in the nft fold's
transaction (`services/catalyst/shop/src/market/legacy_orders.rs`) with the
rules both subgraphs applied:

- an `OrderCreated` opens an order, keyed by its marketplace and id;
- the `OrderSuccessful` naming the id marks it `sold`, with the buyer and
  the price paid; the `OrderCancelled` naming it marks it `cancelled`;
- a later `OrderCreated` on the same token, on either Marketplace of the
  network, cancels it unless one of those follows.

The subgraphs also moved an open order to `transferred` when its token left
the seller, and back to `open` when it returned. The reader derives that at
read time from the token's current owner on its own snapshot (`shop.nft`
for collections and names, `land_authz.token_right` for LAND and estates),
so the fold reads one index. An open order reads `open` only while that
source proves its seller holds the token: another owner, no owner (a burned
token, a dissolved estate) or no row at all reads `transferred`, in the
page, the status filters and the totals alike, so `status=open` never serves
an order whose ownership is not proven. The reader serves only the orders of the contracts
the subgraphs knew: a mirrored collection, the LAND and Estate registries,
and the DCLRegistrar.

Readiness follows the ownership readers' fresh rule: the nft index complete
and published within 30 minutes on every network the read names, from the
chain the node serves there (`MARKETPLACE_INDEX_CHAIN_ID_ETHEREUM`,
`_MATIC`), read on the same `REPEATABLE READ, READ ONLY` snapshot as the
rows, with the order book caught up with that publication (below) and the
trades mirror and its verdicts ready on it; and, when an open LAND or
estate order could be answered, the land-authz index ready, whose snapshot
the read then runs on. Otherwise the route answers the unavailable envelope
naming the index that is not ready. The v3 listings leg reads the same
snapshot behind its owner guard, so the order book no longer has a partial
mode. One request reads both legs at one instant, taken once its snapshot
is established (so a fold published while the request waited is not read as
published in the future), and its total counts each network up to the
reader's cap and sums them, as the subgraph reader did.

A node reads the book exactly when it walks the nft index: the switch on
and both chains it serves supported by the walk, the mainnet pair or
Sepolia/Amoy, the rule the walk is scheduled by. A testnet walker therefore
serves its testnet book under the same readiness. A node with the switch off
keeps the collections subgraph reader for its legacy orders and serves the
v3 listings unguarded, as before.

The book keeps its own marker per network (`shop.legacy_order_cursor`): the
nft walk's last block when the order fold last ran, and that block's hash.
The fold refolds the orders touched past the lower of that block and the nft
fold's marker, so the book catches up with nft folds that ran without it:
during a rollout, a replica on an earlier binary folds the nft index and
never writes the book. The hash binds the book to a branch. Every binary's
rewind drops the walk's recovery points (`shop.chain_checkpoint`) past its
target, so a branch an earlier binary rewound and republished, even at the
same height, carries another hash at the book's block, and a book left
behind past the kept points loses its point. A book whose point no longer
carries its hash does not stand: no reader serves it, and the next fold
deletes the network's orders and refolds them whole, as every rebuild does,
so no order of a lost branch survives. A reader serves the book only while
it stands and its block reaches the published nft fold. A database that
predates the table fills it in the schema run that creates it: the fold's
rules over every token, bounded by what the nft fold had folded
(`fold_through`), which becomes the book's marker with its recovery point's
hash. So the seed holds no creation, supersession or end that the ownership
it is read beside does not hold yet, and the next fold takes the rest.

## Consequences

- The order book no longer depends on a hosted subgraph per request.
- LAND, estate and NAME orders, which the collections subgraphs never
  indexed, are listed.
- An order whose `expiresAt` the 2018 contract took in seconds is compared
  in milliseconds, so it is listed until it really expires.
- While the nft index is not fresh, or the order book trails it,
  `/v1/orders` and `/v1/orders/open-by-items` answer 503. So does credits
  pricing, which reads the latter.
- `/v1/activity` keeps its partial 200: a leg whose index is not ready
  contributes nothing, as any failing source did. Its
  `x-activity-unavailable-sources` header names the source of each such
  leg (`nft-index`, `land-authz-index`, or a reader's subgraph), so a client
  can tell a partial history from a complete one.
- A token sent away and back, away for good, relisted, sold or cancelled
  leaves the statuses the subgraph left (a test replays those traces
  against a model of its handlers). One trace differs: when an order is
  superseded on the other Polygon Marketplace, sold there, and the token
  comes back to the superseding order's seller, that order reads `open`,
  where the subgraph, having cleared the token's active order with the
  sale, kept it `transferred`. The order is still executable on chain in
  that case.

Known gaps, accepted: the legacy bids of `/v1/bids` still read the
collections subgraphs, and Ethereum's ERC721Bid bids on LAND, estates and
names are not indexed yet.
