# credits-server -> credits

Pinned at `6b7407ef227af07885fe863123c6dbfb9056d1f7` (migrations through
`1787000000000_usd-purchase-alerts`). Run at cutover with upstream writes
stopped.

| Step | Upstream | Target |
| --- | --- | --- |
| `cutover_guard` | `usd_purchase_intents`, `stripe_orders` in flight | nothing (refuses the run) |
| `wallet` | `users`, `flagged_wallets` | `credits.wallets` flags |
| `purchase` | `stripe_orders` + their `user_credits` | `credits.purchases` |
| `ledger` | USD `user_credits`, SETTLED `usd_purchase_intents` | `credits.ledger`, `credits.wallets` balances |

## Known limitations

- **MANA-denominated credits stay outside the node.** Upstream signed each
  one and the CreditsManager contract accepts it until its `expires_at`, so
  importing it would let it be spent twice. After cutover these credits are
  not part of any node balance and the node has no bucket for them: holders
  keep spending them on-chain until they expire, and the node's balance
  shows only USD credits.
- **In-app (Apple) credits cannot be clawed back.** USD credits granted by an
  Apple purchase import as ordinary grants; the node has no App Store
  integration, so an Apple refund issued after cutover does not reverse
  them.

## Money

Both sides use 10 USD cents per Credit, so `amount = usd_cents / 10` (one
decimal, exact). Upstream's spendable USD balance is

    SUM(user_credits.usd_cents WHERE USD AND available)
      - SUM(usd_purchase_intents.usd_cents WHERE SETTLED or held)

and the node's `wallets.available` is the signed ledger sum. The ledger step
writes one row per upstream row, all in the `paid` bucket:

| Upstream row | Ledger kind |
| --- | --- |
| USD credit, `credit_source = 'purchase'` | `purchase` |
| other positive USD credit (`grant`, `studio_grant`, `iap`, `sale_proceeds`, `sale_royalty`, `dispute_won`) | `grant` |
| negative USD credit (`refund` claw-back) | `consume` |
| SETTLED intent | `spend` |

`tx_ref` is `credits-server:credit:<id>` / `credits-server:intent:<id>`; a
replay finds it and writes nothing. After inserting, the load recomputes
`available`/`earned_available` for each wallet in the batch from its whole
ledger. Credits sort before debits within a wallet, so a batch boundary never
leaves a partial wallet negative; a wallet whose final balance is negative
upstream fails the CHECK and stops the run.

`cutover_guard` proves "held = 0": it refuses PENDING intents, EXPIRED intents
inside upstream's one-hour recovery horizon, and Stripe orders processing,
crediting or initiated in the last 24 hours. With it, the node's balance
equals upstream's spendable USD for every wallet.

`purchases`: keyed by the upstream order id (`order_id`). `credits` is what
the order's purchase credit granted, so the reconciler's paid-vs-granted check
holds by construction. A credited order is `paid` until its net reversal
(`reversal_usd_cents - restored_usd_cents`) covers the charge, then
`refunded`; `revoked_credits` is the net of its refund/dispute_won rows, so a
later Stripe reversal processed by the node claws back only the remainder.
Uncredited orders become `failed` or `abandoned`.

`wallet`: `has_started_program = opted_out_at IS NULL`;
`is_blocked_for_claiming` = a live (unexpired) captcha or multi-account flag.

## Not migrated

- MANA-denominated `user_credits` (goal, on_demand, legacy iap) and
  `rollbacked_credits`, `on_demand_credits`: each MANA credit is signed by
  upstream and spendable on the CreditsManager until its `expires_at`;
  counting it here too would let it be spent twice.
- USD credits whose `available_at` is in the future (held sale proceeds): the
  node has no hold. A later run imports each once it is available.
- `usd_purchase_intents` as `credits.authorizations`: upstream signed their
  salts; the node's usage index would settle a matching CreditUsed log again.
- `stripe_events`, `stripe_payment_refunds`, `stripe_payment_disputes`:
  upstream endpoint's de-duplication and reconciliation state; their outcome
  is in `purchases` and the refund ledger rows.
- `iap_credits`, `iap_account_tokens` (no IAP on the node), `seasons`,
  `goals`, `season_goals`, `user_progress`, `sale_proceeds`,
  `proceeds_cursor`, `proceeds_abandoned_fills`, `studios`,
  `studio_operators`, `studio_budget_entries`, `shop_daily_summaries`,
  `usd_purchase_alerts`: no target; money effects are `user_credits` rows.
- Left to the node: `packs`, `idempotency`, `captcha_challenges`, `carts`,
  `cart_items`, `checkouts`, `fulfillment_outbox`, `authorizations`,
  `authorization_lines`, `usage_settlements`, `revocations`, `usages`,
  `usage_index_cursor`.

No watermarks; every run re-scans and is idempotent.
