# ADR: Shop prices at the marketplace's MANA/USD feed

- Status: Accepted
- Date: 2026-11-06

## Context

A Shop item is priced in MANA, in USD, or in credits. Showing any of them in another unit is a
conversion at a MANA/USD rate. The Shop converted at the node's market spot: the newest
`economy.price_snapshots` row, polled from CoinGecko.

The off-chain marketplace converts a USD-pegged trade with its own feed instead: the
`manaUsdAggregator()` it is configured with, read in the block that settles the trade, and refused
once the round is older than its `manaUsdAggregatorTolerance()`. The spot and that feed are
different numbers. On Polygon they agree within a feed round. On Amoy the feed is a test answer:
on 2026-11-06 it read $0.2696 against a $0.0724 spot, so a $1.40 item showed 19.33 MANA in the
cart and the purchase charged 5.19 MANA.

The quote also sizes the buyer's MANA approval. An approval sized exactly at a rate the contract
does not use can fall short when the trade settles.

The pinned upstream Shop already reads the marketplace's aggregator for this
(`app/src/lib/mana-rate.ts`) and approves a USD-pegged trade's MANA plus 2%
(`app/src/lib/buy.ts`).

## Decision

- **Source.** The Shop's MANA/USD rate is the settlement chain's newest marketplace's own feed,
  read the way a credits authorization reads it (`shop::credits::feed::ChainFeed`): the
  marketplace's `manaUsdAggregator()` and `manaUsdAggregatorTolerance()`, the aggregator's
  `decimals()` and `latestRoundData()`, at one block hash, on the endpoint's checked chain. There
  is no configured aggregator in this path. A node with no endpoint or no marketplace for the
  settlement chain keeps the spot.
- **No request waits on the network.** One refresher per process keeps a slot current
  (`shop::mana::feed_rate`). Readers answer from the slot and never block. The refresher reads
  every 15 s, just before a round leaves the marketplace's tolerance, and every 2 s once it has. It
  backs off to 30 s while the endpoint fails. The slot is reported on `/health`
  (`shop_mana_usd_feed`).
- **One read, three uses, each with its own bound.** Every age is the round's (when the feed set
  the answer), not the read's. It is counted on a monotonic clock from the read's start, and is
  the older of the node's and the chain's clock:
  - *Showing a price* (catalog, item, import): the last good round within
    `CREDITS_USD_PEGGED_FEED_MAX_AGE_SECS` (an hour by default). An RPC outage keeps prices shown
    until then.
  - *Charging credits* (authorize, cart, top-up, reprice): the round within
    `MANA_PRICE_MAX_STALENESS_SECS` (900 s by default), the bound the spot had.
  - *Quoting MANA for a USD-pegged trade*: a read of the last minute, and a round at least 3 s
    inside the marketplace's tolerance on the chain's clock. Outside that, the line's MANA price is
    unavailable. A fixed-MANA line or a store mint is not affected, because it settles at no feed.
- **No fallback.** Once the feed is wired, no path answers the spot or an older rate. Without a
  rate, a price is unavailable.
- **Exact rate.** The rate stays an exact decimal down to every `NUMERIC` bind. A float bind made
  0.1 into 0.100000000000000006, and priced a 1 MANA store mint at $0.10 at two credits.
- **Approval margin.** A USD-pegged trade's approval is its quoted MANA plus 2%, as upstream does.
  It is a margin for the rate moving before the settling block, not a slippage bound. The quoted
  amount and the balance check are unchanged.
- **Scope.** The economy routes keep the market spot: they report the token's market price, not a
  checkout amount.

## Alternatives considered

- **Keep the spot for the Shop.** Simple and independent of an RPC, but it shows and quotes a
  price the contract does not charge. It is the defect this decision removes.
- **A periodic job writing the feed's answer into `economy.price_snapshots`.** It reuses the
  table, but every reader then sees a rate up to one poll late, the table mixes two kinds of rows,
  and the round's own age and tolerance are lost.
- **Read the feed on each request.** It is exact, but every price page then waits on the RPC, and
  a slow or failing endpoint takes the catalog down. A refresher with non-blocking readers keeps
  the RPC off the request path.
- **Fall back to the spot when the feed fails.** It keeps prices on screen, but on Amoy it shows a
  price 3.7 times off the charge. Serving the last good round within a bound, and nothing beyond
  it, was chosen instead.

## Consequences

- Shop prices on a node follow its settlement chain's feed. Testnet prices are the test feed's,
  which is what a testnet purchase charges.
- A node's Shop depends on its settlement chain's RPC endpoint. An outage degrades purchases
  priced in USD first, after a minute, then credit charges, after
  `MANA_PRICE_MAX_STALENESS_SECS`, then shown prices, after
  `CREDITS_USD_PEGGED_FEED_MAX_AGE_SECS`.
- `CREDITS_USD_PEGGED_FEED_MAX_AGE_SECS` bounds shown Shop prices as well as credit
  authorizations.
- Only the newest marketplace's feed is read. A USD-pegged trade signed for an older marketplace on
  the same chain is quoted at that rate too. On Polygon and Amoy today every marketplace points at
  the same aggregator.
- Follow-up: the broker (`checkout/broker.rs`) still checks a broker purchase's MANA against the
  configured aggregator (`MANA_USD_AGGREGATOR_ADDRESS`), not the marketplace's own.
