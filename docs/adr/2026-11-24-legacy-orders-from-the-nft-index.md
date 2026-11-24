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
so the fold reads one index and `status=open` never serves an order whose
seller lost the token. The reader serves only the orders of the contracts
the subgraphs knew: a mirrored collection, the LAND and Estate registries,
and the DCLRegistrar.

Readiness follows the ownership readers' fresh rule: the nft index complete
and published within 30 minutes on every network the read names, read on
the same `REPEATABLE READ, READ ONLY` snapshot as the rows; and, when an
open LAND or estate order could be answered, the land-authz index ready,
whose snapshot the read then runs on. Otherwise the route answers the
unavailable envelope naming the index that is not ready. The v3 listings
leg reads the same snapshot behind its owner guard, so the order book no
longer has a partial mode.

A node that does not walk the index (the switch off, or a network other
than mainnet) keeps the collections subgraph reader for its legacy orders
and serves the v3 listings unguarded, as before.

A database that predates the table fills it from the log in the schema run
that creates it (the schema's seed statement, the fold's rules over every
token), so no reader finds it empty under a published cursor.

## Consequences

- The order book no longer depends on a hosted subgraph per request.
- LAND, estate and NAME orders, which the collections subgraphs never
  indexed, are listed.
- An order whose `expiresAt` the 2018 contract took in seconds is compared
  in milliseconds, so it is listed until it really expires.
- While the nft index is not fresh, `/v1/orders` and
  `/v1/orders/open-by-items` answer 503. So does credits pricing, which reads
  the latter, and the activity feed leaves its orders out.
- An order superseded on the other Polygon Marketplace and then sold there
  reads `sold`, as in the subgraph. The superseding order reads `open`, or
  `transferred` once the sale moved the token; it reads `open` again if the
  token comes back to its seller, where the subgraph kept it `transferred`.
  It is still executable on chain in that case.

Known gaps, accepted: the legacy bids of `/v1/bids` still read the
collections subgraphs, and Ethereum's ERC721Bid bids on LAND, estates and
names are not indexed yet.
