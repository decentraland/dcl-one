# marketplace-server → shop

Source: the `dapps` database, schema `marketplace`, at
marketplace-server `cc5baede` (`src/migrations/dapps`). The favorites
database is the `marketplace-favorites-server` source; `src/migrations/substreams`
only creates a builder FDW table and a view over it (no owned rows).

| Step | Upstream | Target |
| --- | --- | --- |
| `trades` | `trades` + `trade_assets` + `trade_assets_{erc20,erc721,item}`, one row per trade with its assets as jsonb | `shop.trades`, `shop.trade_assets`, `shop.trade_assets_{erc20,erc721,item}` |
| `coupons` | `coupons` | `shop.coupons` |
| `coupon_state` | `coupon_state` | `shop.coupon_state` |

## Rules

- **Signed data.** The signature, `hashed_signature`, `state_key`, signer and
  addresses are lowercased, as both node insert paths (`market::listings`, the
  trades mirror, `coupons::component`) store them. The decoded signature bytes
  do not change, so `hashed_signature` (keccak of the bytes) still matches.
  `checks`, `root` and `collections` are copied as stored, never rebuilt.
  `contract` keeps its case (the node compares `lower(contract)`).
- **Ids.** Upstream trade, asset and coupon ids are kept. The trades mirror
  already keeps trade ids, so a mirrored copy is the same row: the load promotes
  it to local authority (`mirrored_from`/`mirrored_at` NULL) and replaces its
  assets (random ids, same content) with upstream's. That is the only delete.
- **Conflicts.** If the node already holds a signature (or a coupon state key)
  under another id, the node's row is kept and the upstream row is skipped. If
  an id is already held with another signature, that row is left alone.
- **Asset normalization** follows the mirror: `extra` NULL/''/'0x' becomes '',
  an empty beneficiary becomes NULL. A trade with no assets is skipped (the
  mirror refuses those too).
- **Timestamps.** `timestamptz(3)` is converted to UTC `timestamp(3)`.
- **coupon_state** is chain-read state. It is imported as a seed because the
  node's `coupon-state-refresh` job re-reads live coupons only. A newer reading
  already in the target wins.

## Chain state: cancellations, pause, index bumps

Upstream keeps no off-chain cancellation table. V1/V2/V3 cancellations, signer
and contract signature-index bumps (the V1/V2 invalidation of
marketplace-server#432/#435) and `Paused` are all on chain, read through the
squid. In the node they come from the `trade_events` index (`shop.trade_event`)
and its fold into `shop.trade_chain_state`. That fold compares
`checks.contractSignatureIndex`/`signerSignatureIndex` with the indexed bumps.
It picks up every imported trade that has no folded row, so an invalidated
trade folds to `cancelled`. Until the index is published for a network, the
routes ignore chain verdicts there, which is the node's normal pre-ready
behaviour. `trade_digest` is not copied because the fold recomputes the V3
digest from the row. Nothing is written to `shop.trade_cancellations`, which
holds only cancellations signed into the node.

## Not migrated

- `mv_trades` and the item/collection/creator search tables and
  `creator_profiles`: rebuilt caches.
- `item_neighbors*`: the node rebuilds these with its `item-neighbours` job.
- `mana_usd_daily`: oracle cache.
- `favorite_discount_notifications` and `coupons.favorites_*`: notification
  dedupe, with no target table.
- `mv_builder_server_items` and the FDW tables: foreign data.

## Left to the node (chain-derived or mirrored)

`shop.trade_event`, `trade_chain_state`, `nft*`, `nft_event`, `nft_order`,
`rental_event`, `chain_cursor`, `land_data`, `land_proximity`, `mana_transfer`,
`collections`, `items`, `mirror_state`, `trades_mirror`, `item_neighbors*`,
`executions`.
