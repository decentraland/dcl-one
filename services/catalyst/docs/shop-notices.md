# Shop chain notices: cutover seed and rewind

The chain notification producers (`shop/src/market/{transfer,sale,royalty}_notices.rs`,
`shop/src/rentals/notices.rs`) announce sales, royalties, rentals and transfers from the
node's chain indexes. Each keeps one row of `shop.notification_cursor`, named in
`shop::market::notice_cursor::PRODUCERS`: its cursor (the last chain position handled;
`rental_ended` orders by the period's end time first) and its seed, the floor below which
a reorg replay never moves it. A producer's first pass seeds the row at its index's
published head, so on its own it announces nothing mined before that pass.

## The command

```sh
catalyst shop notice-rewind --database-url "$DATABASE_URL" --producer <name> \
  (--since <time> | --from-block <block>) [--dry-run]
```

- `--producer` is one of `PRODUCERS` (`catalyst shop notice-rewind --help` lists them).
- `--from-block N` announces the logs of block N and later: the cursor becomes
  (N - 1, after its last log).
- `--since T` announces the logs mined at or after T: RFC 3339
  (`2026-11-01T00:00:00Z`), a day (`2026-11-01`, UTC midnight) or epoch milliseconds,
  the unit a legacy producer cursor stores. A bare number below 10^12 is refused.
  T resolves through the logs of the indexes the producer reads
  (`shop.nft_event`, `shop.trade_event`, `shop.sale_event`, `shop.rental_event`): the cursor
  goes after the last block mined before T on any of them. The command refuses while one of
  those indexes has not indexed past T (its `last_block_time`), since logs it has not reached
  would later be read as already handled. For `rental_ended`, T is the end-time cursor
  itself: every period ending at or after T is announced; it takes no block.
- The cursor and the seed only move back. A target at or after the cursor changes nothing.
  A producer no pass has seeded yet is seeded at the target, and its first pass keeps that
  seed.
- `--dry-run` resolves and prints without writing.

It runs in one transaction holding the producer's row, so a running pass finishes its page
first and the next page starts from the new cursor. It prints one JSON object: the
`producer`, its `network`, the resolved `target_*`, the row `before` (absent when no pass
had seeded it) and `after`, `changed`, `dry_run` and `indexes` (each index's `start_block`,
`full_history`, `last_block` and `last_block_time`). An index that is not `full_history`
holds no logs below its `start_block`; those cannot be announced. A change is recorded in
`ops.admin_audit` (domain `market`, action `notice-cursor.rewind`, actor `cli`). The
command needs only database access; whether anything is emitted is still up to the
producers' own jobs and switches.

Cost: for `--since`, one backward walk of each index's log by its primary key from the
head down to T, so a date far back reads every log since then once; the command allows each
statement 15 minutes. If a walk still times out, give the block with `--from-block`. The passes that
follow re-read the same range a page at a time.

## Duplicates

A notification's id is derived from its log (`notice_cursor::notification_id`: the
transaction and the log's place in it, or the token and its identical predecessors for
`shop.nft_event`), and `shared::notifications::insert_notifications` upserts by that id: a
notice already stored is refreshed in place, keeps its read state and gets no new email
delivery row. A rewind over notices this node already emitted therefore stores and emails
nothing twice.

That does not hold for notices another system delivered. Notifications imported from
notifications-workers keep that service's ids
([mapping](../../../tooling/migration/sources/notifications-workers/MAPPING.md), "Node
producers"), which never match the node's. Rewinding below what the legacy producer already
announced stores and emails those sales again; seeding above it leaves a gap no pass fills.

## Cutover from events-notifier

events-notifier (`decentraland/events-notifier@5ceb8ab`) keeps one cursor per event
subtype in its `cursors` table, keyed by the `@dcl/schemas` subtype (`item-sold`,
`bid-accepted`, `royalties-earned`, `land-rental-started`, `land-rental-ended`, `bid-received`): `last_successful_run_at`, epoch milliseconds of the start of
its last successful run. Its next run would have queried events at or after that second
(`timestamp_gte`, `startedAt_gte`, `endsAt_gte`), which is what `--since` with the same
value announces. Its `item-sold` row can instead hold a sale id of the collections subgraph (a value below
10^12); the command refuses it. Look that sale's `timestamp` (unix seconds) up in the subgraph:
the legacy producer had announced that sale and every sale with a lower id, so pass the next
second as an RFC 3339 `--since` (sales in that sale's own second are then skipped), or that
second itself and accept that they are announced twice.

The table maps by notice type. events-notifier polled sales, bids and royalties on Polygon
(the collections subgraph) and rentals on Ethereum, as these producers read them.

| events-notifier cursor | node producers to seed from it |
| --- | --- |
| `item-sold` | `sales:order_successful:matic`, `sales:mint:matic`, `sales:bid_accepted:matic` |
| `bid-accepted` | `sales:bid_accepted:matic` |
| `royalties-earned` | `royalties_earned`, `royalties:bid_accepted:matic` |
| `land-rental-started` | `rental_started` (Ethereum) |
| `land-rental-ended` | `rental_ended` (Ethereum) |

A producer fed by two legacy cursors (`sales:bid_accepted:matic` tells the seller "Item Sold"
and the bidder "Bid Accepted") takes the earlier one; the notices of the other type between
the two times are then announced a second time. `sales:traded:ethereum`,
`sales:traded:matic` (off-chain trades, announced before by marketplace-server's accept
events, not by events-notifier) and `transfer_received` have no events-notifier cursor;
leave them to their first pass, or give them the cutover time. A producer added later is
seeded the same way from its legacy cursor, if it has one (a `bid_received` producer from
`bid-received`).

Order, with the notifications import (`MAPPING.md`, "Node producers"):

1. Stop events-notifier's producers, then read its cursors:
   `SELECT id, last_successful_run_at FROM cursors`.
2. Finish the notifications import (its final pass).
3. Wait until the node's indexes have indexed past those times
   (`shop.chain_cursor.last_block_time`), then, per producer in the table above, run the
   command with `--dry-run`, check the target, and run it again without. This works
   before or after the producers' first pass: a row they already seeded at the head is
   moved back.
4. Enable the node's producers. Their next passes announce the sales since the legacy
   cursors.

## Rewinding later

To announce again from a time or block, for instance after a delivery outage, run the
same command. Notices the node had already stored are refreshed in place without an email;
the ones it had missed are stored and emailed.
