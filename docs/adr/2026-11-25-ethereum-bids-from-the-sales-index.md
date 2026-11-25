# Ethereum legacy bids from the sales index

- Status: Accepted
- Date: 2026-11-25

## Decision

On a node that walks the nft and sales indexes, `/v1/bids` and the activity
feed's bids legs read Ethereum's legacy bids (ERC721Bid: v1 wearables, LAND,
estates and names) from `shop.nft_bid`, as Polygon's are read
([Polygon legacy bids](2026-11-24-polygon-bids-from-the-sales-index.md)).
No legacy bid read asks the subgraph on such a node.

The Ethereum leg follows Polygon's rules (one snapshot and one instant, the
catalogue scope, the seller rule, the statuses), with these differences:

- Readiness adds, on Ethereum mainnet, the bid history repair complete and
  explaining the sales cursor
  ([the bid lifecycle in the sales walk](2026-11-25-ethereum-bid-lifecycle-in-the-sales-walk.md)),
  else the `sales-index` envelope. It also adds the land-authz index ready,
  whose snapshot the read runs on, else the `land-authz-index` envelope.
- The scope is the catalogue the nft index walks on the served chain plus
  that chain's LAND, Estate and names registrar, the assets the marketplace
  indexer kept bids on.
- A LAND or estate bid's seller, when the fold stored none, is the token's
  latest transfer before the bid in the land-authz log, read when the land
  index's published block reaches the bid's block. The nft walk does not
  read those transfers. One past it fails the read closed with the
  `land-authz-index` envelope.
- The wire is the marketplace subgraph's: `fingerprint` 0x-prefixed hex
  (`0x` when empty or not kept), `bidAddress` the chain's ERC721Bid.
- Each network's total is capped as the subgraph reader capped it, and the
  networks are summed.

## Consequences

- `/v1/bids` and the activity bids legs no longer depend on a hosted
  subgraph on a walking node.
- An upgraded node answers Ethereum bid reads with the `sales-index`
  envelope until the bid history repair completes.
- A read that could include an Ethereum bid needs the land-authz index
  ready.
