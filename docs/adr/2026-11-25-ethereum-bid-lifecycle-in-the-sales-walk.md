# The Ethereum bid lifecycle in the sales walk

- Status: Accepted
- Date: 2026-11-25

## Context

Ethereum's legacy bids (ERC721Bid: v1 wearables, LAND, estates and names)
are read from the marketplace subgraph. To serve them from the node's own
indexes, as Polygon's are, the node needs each bid's creation and
cancellation, not only its acceptance.

The Ethereum `sales` walk already reads the bid contract, for `BidAccepted`
only, which sales accounting needs. On a deployed node its cursor is far past
the contract's deployment. Reading the other two topics from now on covers
only the bids placed after the upgrade.

The sales history repair (`sale_history`) re-walks a cursor's inputs when the
sales input version changes. Sales accounting requires that version current
on every input cursor, so changing it to add the two topics would hold
accounting until a full-history repair finished.

A second walk with its own cursor over the bid contract would have full
history from its start, but two walks would then write and rewind the same
`shop.sale_event` rows and fold the same `shop.nft_bid` rows, each from its
own snapshot.

## Decision

- The Ethereum sales walk reads `BidCreated`, `BidAccepted` and
  `BidCancelled` and folds them into `shop.nft_bid` with the Polygon fold's
  rules. It stays the only writer of Ethereum bid events.
- Its history below the first window that read the three topics is repaired
  by its own `shop.worker_progress` row (`ethereum_bid_history`), as the
  estate creations repair works:
  - windows run after each publication with spare log budget, from the
    contract's deployment block (7,270,906);
  - each window is bracketed by the cursor's checkpoint and checked against
    its predecessor and each event's canonical block hash;
  - the window that completes the repair refolds every Ethereum bid in its
    transaction.
- Sales accounting never waits on the repair. The Ethereum bid book is read
  only while the repair is complete and its marker explains the cursor.
- A creation keeps its fingerprint as the subgraph spelled it: 0x-prefixed
  hex, `0x` when empty.
- The nft walk records a seller recheck on every network's rewind, so the
  Ethereum fold infers sellers again as Polygon's does.

## Consequences

- An upgraded node serves Ethereum bids from its index only after the repair
  has read the contract's history, about 15 million blocks in windows of at
  most 10,000.
- A writer that reads only `BidAccepted` (an earlier binary during a rollout)
  leaves a cursor the marker no longer explains, and the repair restarts from
  the deployment.
- Creations walked before the fingerprint was kept hold none. That affects
  only Sepolia history, whose sales walk read the lifecycle already.
