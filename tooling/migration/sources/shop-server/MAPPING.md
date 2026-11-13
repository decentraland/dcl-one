# shop-server → shop

`run all` excludes this entire source for security reasons. At cutover, run `run shop-server --step treasury_ledger
--step treasury_pending_refill`; `treasury_guard` runs first automatically.
Explicit `run shop-server` includes the treasury too.

Source: the shop-server database at `609a1f69` (`src/migrations`). Its
migrations are the same as at `dc1a8be96`, the revision `schemas/30_shop.sql`
was derived from. Upstream times are epoch-ms `bigint`, except on the
waitlist (`timestamptz`). The target keeps the milliseconds where its wire
exposes them (outfits, thumbnails, the treasury). Each watermark is the same
instant as a `timestamptz`.

| Step | Upstream | Target |
| --- | --- | --- |
| `outfit_thumbnail` | `outfit_thumbnails` without `bytes` | `shop.outfit_thumbnails` |
| `outfit` | `outfits` | `shop.outfits` |
| `notify_request` | `item_notify_requests` | `shop.item_notify_requests` |
| `treasury_guard` | unresolved `treasury_pending_refill` rows | nothing (refuses) |
| `treasury_ledger` | `treasury_ledger` | `shop.treasury_ledger` |
| `treasury_pending_refill` | `treasury_pending_refill`, `settled`/`aborted` | `shop.treasury_pending_refill` |

## Rules

- **Thumbnails are blobs.** The node serves a thumbnail from the ONE blob
  store under `blob_cid` (`hash_bytes_v1` of the bytes). Upstream keeps the
  bytes in the row. The blob copy writes each one to the store and records
  it in `dcl_migration.blob_inventory` (source `shop-server`, `object_key` =
  the sha256 hex, `hash` = the cid, `size`). A thumbnail that is missing
  there, or recorded with a different size, stops the batch. Conflicts are
  no-ops because the rows are content-addressed.
- **Outfits.** Upstream ids are kept. Upstream validated outfits with the
  same rules the target declares. A row that breaks one is skipped and
  counted, never repaired. Gradient stops and the author are lowercased. A
  newer `updated_at` already in the target wins. The load counts outfits
  whose thumbnail the target does not hold.
- **Waitlist.** The identity is `(address, contract_address, item_id)`,
  with both addresses lowercased. The upstream `bigserial` is a surrogate
  that no wire or row references, so the node assigns its own id. The newest
  `updated_at` wins, also against a row the node wrote. Status and
  `notified_at` are copied, so a `notified` row is never mailed again: the
  node enqueues only `pending` requests, and its sender re-checks that the
  request is still pending. Emails are copied as stored. They are personal
  data, so never print a stage or a failing row (`--verbose-errors`).
- **Treasury ledger.** Upstream ids are kept. Deltas are `numeric(78,0)`
  base units (USDC 6, MANA 18) and travel as text, never as floats. Rows are
  immutable, so a replay inserts nothing. The batch stops on any row that
  would not land exactly: an unknown type, a fractional amount, an id the
  target holds with other content, or a `(type, reference)` the target holds
  under another id. Either the whole ledger lands or the run stops, so the
  per-type sums and counts match upstream.
- **Refill intents.** `treasury_guard` refuses the run while any intent is
  `pending`, `swapped` or `transferred`. Such an intent may be on chain
  without its ledger row. Upstream does not persist the signed swap before
  `swapped`, so the node cannot recover an intent safely. Stop upstream's
  refill job and let it finish first. Resolved intents are copied 1:1. The
  node-only columns (nonces, raw transactions, `abort_reason`) stay NULL. The
  load counts settled intents whose `ledger_entry_id` is missing from the
  target.

## Not migrated

- `treasury_proceeds_reconciliation`: audit snapshots of the
  proceeds-to-treasury flow, which the node does not run. There is no
  target table.
- `outfit_thumbnails.bytes`: blobs (see above).
- `pgmigrations`.

## Left to the node

- `item_notify_outbox`: upstream's delivery hop was SQS, so there is no
  durable state to copy. An item that went on sale during cutover and was
  not mailed upstream yet is not mailed. Requests stay `pending` for the
  next transition.
- `item_availability_watch`: the watcher's first observation is its
  baseline, and only a later false → true transition notifies.
- `treasury_drift_checks`: no upstream equivalent.

## Caveats

- Deletions are not detected: an outfit deleted upstream after its copy
  stays in the target. Do the final pass after upstream writes stop.
