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
  else the `sales-index` envelope. It also requires the land-authz index
  ready for the served registries on the read snapshot, else the
  `land-authz-index` envelope.
- The scope is the catalogue the nft index walks on the served chain plus
  that chain's LAND, Estate and names registrar, the assets the marketplace
  indexer kept bids on.
- A LAND or estate bid's seller, when the fold stored none, is the token's
  latest transfer before the bid in the land-authz log, read when the land
  index's published block reaches the bid's block. The nft walk does not
  read those transfers. One past it fails the read closed with the
  `land-authz-index` envelope.
- The land-authz walk and the sales walk are independent and each trails
  the head by its confirmations only, so after a fork each can publish the
  same height on another branch. The land log is read for a bid only up to
  the highest block both walks hashed alike: the land cursor's block or its
  checkpoint, against the sales cursor's block or one of its checkpoints
  (`shop.chain_checkpoint`). A common hash proves the two logs equal up to
  it. A bid past it fails the read closed with the `land-authz-index`
  envelope, as one past the land index's publication does.
- A mirrored (v3) bid on a LAND or Estate token reads its seller from the
  land-authz fold (`land_authz.token_right`). On both walking and
  graph-backed nodes, a scoped LAND or Estate bid requires that fold to be
  ready for the trade's served chain on the read snapshot. On an NFT walker,
  a scoped mirrored bid that reads `shop.nft` requires that ownership
  publication. The native book can reuse its proof for the same chain.
- The wire is the marketplace subgraph's: `fingerprint` 0x-prefixed hex
  (`0x` when empty or not kept), `bidAddress` the chain's ERC721Bid.
- Each network's total is capped as the subgraph reader capped it, and the
  networks are summed.

## Consequences

- `/v1/bids` and the activity bids legs no longer depend on a hosted
  subgraph on a walking node.
- An upgraded node answers Ethereum bid reads with the `sales-index`
  envelope until the bid history repair completes.
- Reads of the native Ethereum book need the land-authz index ready.
- The sales walk records a checkpoint at each window end and keeps the
  newest 64. On Ethereum it passes every few seconds, about one block per
  window, so the land cursor's block is normally one of them. While the land
  walk is ahead of every block the sales walk has hashed, and its own
  checkpoint is older than the sales walk's oldest, an open LAND or estate
  bid in a read's scope answers `land-authz-index` until the sales walk
  catches up, typically within one pass.
