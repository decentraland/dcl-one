# Polygon legacy bids from the sales index

- Status: Accepted
- Date: 2026-11-24

## Decision

On a node that walks the nft and sales indexes, `/v1/bids` and the
activity feed's bids legs read Polygon's legacy bids (ERC721BidV2) from
`shop.nft_bid`, the sales fold's table. They no longer query the
collections subgraph for them. Every legacy bid on a node that does not walk
the indexes keeps the collections subgraph reader. Ethereum's legacy bids
follow the same reader
([Ethereum legacy bids](2026-11-25-ethereum-bids-from-the-sales-index.md)).

The wire shape is the subgraph's: the id is `<contract>-<token>-<bidder>`,
`blockchainId` the bid id, `blockNumber` the bid's last event,
`bidAddress` the bid contract, and `fingerprint` empty. An acceptance reads
`sold`. A bid the contract dropped when another bid on its token was
accepted reads `cancelled`: the contract emits nothing for it, and the
subgraph kept it `open`, but it can no longer be accepted.

Each read runs on one `REPEATABLE READ, READ ONLY` snapshot, at one instant
taken once it is established, that the trades mirror's bids are read on too.
It checks:

- the trades mirror and its trade-events verdicts are ready;

- the Polygon sales cursor is published: its whole history, a fold within
  30 minutes, bound to the chain the node serves on Polygon
  (`MARKETPLACE_INDEX_CHAIN_ID_MATIC`), out of quarantine. As for the nft
  index's readers, `folded_block` is not compared with `last_block`, which
  every pass advances before it folds;
- the bid book holds that published fold, by a marker the fold writes with
  its rows: the walk publishes a fold only after it commits and reads the
  head, and a read in between answers the envelope rather than rows past the
  published block. A change to the book invalidates the changed network's
  marker in the write transaction. A current fold writes the marker after
  its rows. An older writer leaves the changed network's book unavailable
  until a current writer folds and publishes again;
- the nft index is fresh on Polygon, on that same chain;
- the catalogue supplying Polygon membership has published. The reader
  uses the native collections fold while it owns Polygon and waits for a
  complete collections mirror pass after hand-back;
- every bid in the request's scope (its filters but the seller's) has a
  seller.

The scope, the page and the total hold only bids on the catalogue the nft
index walks on the served chain: a mirrored collection of that chain,
verified there on a testnet. The bid contract takes any ERC721, but the
collections subgraph kept no bid on an asset it had not catalogued, and the
nft walk never reads such a token's transfers.

A bid's seller is the owner of its token when the bid was placed. The fold
infers it from the nft walk's transfers, and the sales walk can publish a
bid before the nft walk reaches its block. So the reader takes the stored
seller only while no nft rewind the fold has not rechecked yet lies before
the bid, or when an acceptance named it. Otherwise it reads the token's
latest transfer before the bid from the nft log, when the log reaches the
bid's block and holds the contract's whole history: a contract added to the
walk late is read only once its backfill below the walk is done, as the fold
does, since a transfer the backfill has yet to add may be the latest. A bid
with neither fails the read closed with the `nft-index`
envelope, rather than being served without its seller or left out of a
`seller` filter.

## Consequences

- Polygon's bid book no longer depends on a hosted subgraph per request.
- While the sales walk is ahead of the nft walk, a read whose scope holds a
  bid placed past the nft walk answers 503 until the nft walk catches up.
  So does every `seller=` read, whose scope is every bid.
- A bid dropped by another acceptance reads `cancelled` where the subgraph
  kept it `open`.
