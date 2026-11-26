# credits-server -> credits

`run all` excludes this source. Run `run credits-server` explicitly at
cutover. Selecting the `wallet`, `wallet_flag`, `purchase` or `ledger`
step also runs `cutover_guard` first.

Pinned at `6b7407ef227af07885fe863123c6dbfb9056d1f7` (migrations through
`1787000000000_usd-purchase-alerts`). Run at cutover with upstream writes
stopped.

| Step | Upstream | Target |
| --- | --- | --- |
| `cutover_guard` | `usd_purchase_intents`, `stripe_orders`, Stripe dispute/refund facts | nothing (refuses the run) |
| `wallet` | `users` | `credits.wallets.has_started_program` |
| `wallet_flag` | `flagged_wallets` (live `multi-account`) | `credits.wallet_flags` |
| `purchase` | `stripe_orders` + their `user_credits` | `credits.purchases` |
| `ledger` | USD `user_credits`, SETTLED `usd_purchase_intents` | `credits.ledger`, `credits.wallets` balances |
| `studio` | `studios` | `credits.studios` |
| `studio_operator` | `studio_operators` | `credits.studio_operators` |
| `studio_budget_entry` | `studio_budget_entries` | `credits.studio_budget_entries` (each studio's balance) |
| `grant_audit` | `on_demand_credits` for imported USD grants | `ops.admin_audit` and `credits.ledger.admin_audit_id` |

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

`cutover_guard` refuses all PENDING and EXPIRED intents, Stripe orders
processing, crediting or initiated in the last 24 hours, unresolved disputes,
and dispute/refund facts not yet reconciled upstream. Intent age is not a
release condition: upstream also requires the active squid deployment's head
past expiry and no indexed consumption. The migration cannot establish that
proof, so even old EXPIRED rows block cutover. This is deliberately conservative:
accounts with historical EXPIRED rows need a separately reviewed release-proof
mechanism before migrating; do not delete rows or relabel them to bypass it.

Disputes must have a known closed outcome and all recorded payment facts must
already be reflected in balances. This guard does not add dispute restoration
to the node. A subsequent reversal of a closed lost dispute, new refund, or new
dispute still needs a supported payment reconciliation owner after cutover;
a successful import alone is not approval to retire the upstream payment service.

`purchases`: keyed by the upstream order id (`order_id`). `credits` is what
the order's purchase credit granted, so the reconciler's paid-vs-granted check
holds by construction. A credited order is `paid` until its net reversal
(`reversal_usd_cents - restored_usd_cents`) covers the charge, then
`refunded`; `revoked_credits` is the net of its refund/dispute_won rows, so a
later Stripe reversal processed by the node claws back only the remainder.
Uncredited orders become `failed` or `abandoned`.

`wallet`: `has_started_program = opted_out_at IS NULL`. The ledger step
creates any other wallet with `has_started_program = false`, as the node does.
`dcl_migration.credit_wallet_loaded` records the flag each wallet was given; a
wallet whose flag no longer matches started the program on the node and keeps
it. A wallet with no record only goes from `false` to `true` (the node never
sets `false`), and an unrecorded `true` is never recorded: a start the node
made, even one upstream also shows, stays the node's when upstream later opts
out. The record moves only for wallets the import still owns, so a node start
that upstream later matches is kept the same way. A wallet imported as `true`
before this record existed is unrecorded too and does not follow a later
upstream opt-out.
`is_blocked_for_claiming` is the node operator's block (it refuses spends too)
and is never imported.

`wallet_flag`: each live `multi-account` flag becomes a `credits.wallet_flags`
row (claims blocked, spending allowed, as upstream) with source
`credits-server:multi-account`, IP and linked addresses from `reason`. The
node's flags never expire, so an expiring multi-account flag stays until an
operator unflags it. `captcha` flags are short-lived throttles and are not
imported. Each run drops imported flags that upstream no longer holds and
never re-adds one an operator lifted (`user.unflag`/`user.unblock` audited
after its `flagged_at`); flags raised on the node are left alone. An empty
upstream set while imported flags exist aborts the batch (wrong source database?);
when every upstream flag really has expired or been lifted, rerun with
`--allow-empty credits-server/wallet_flag`.

`purchases` are not overwritten when the node row's `updated_at` is newer
(the node refunded or abandoned it).

## Studios

Studios, their operators and their budgets are carried whole, after the
ledger. Both sides keep a studio's balance as the sum of its budget entries
in USD cents, so it is not a stored value: once the entries are loaded, each
studio's balance is upstream's.

- `studio`: one row per studio, keyed by upstream's `id` (operators and
  entries name it). A studio the node renamed or paused after upstream's last
  change (`updated_at`) keeps the node's values. A name the node already gives
  another studio (compared case-insensitively, as on both sides) stops the
  run; which studio keeps the name is an operator's decision.
- `studio_operator`: each wallet acting for a studio.
  `dcl_migration.credit_studio_operator_loaded` records the operators the
  import added, and only those follow upstream. One upstream removed is
  removed from the node too, so it stops granting from the budget. One the
  node removed after the import is not added back, and one added on the node
  is never touched. An empty upstream set while imported operators exist
  aborts the batch; when upstream really has none, rerun with
  `--allow-empty credits-server/studio_operator`.
- `studio_budget_entry`: every entry, keyed by upstream's `id`, with its
  amount, kind, note, idempotency key and request hash. Entries are
  append-only on both sides, so a replay writes nothing.

| Upstream entry | Node entry |
| --- | --- |
| `deposit` (`usd_cents > 0`) | `deposit` |
| `adjustment` (`usd_cents <> 0`) | `adjustment` |
| `grant` (`usd_cents < 0`), its `credit_id` imported by the ledger step | `grant`, `ledger_id` = the ledger row with `tx_ref` `credits-server:credit:<credit_id>` |
| `grant` whose credit the ledger has not imported (a USD credit not yet available) | `adjustment` of the same amount, its note naming the credit; the run that imports the credit turns it into the `grant` |

The money is never dropped. A grant whose credit is not imported yet still
subtracts from the budget as an adjustment, so the balance stays exact, and it
links to the credit on a later run. The studio grant's credit itself is a USD
`user_credits` row (`studio_grant`), imported by the ledger step as a `grant`
to the player like every other credit.

Each entry keeps upstream's idempotency key and request hash. The node hashes
a request differently from upstream, so a key first used before cutover and
retried after it is refused as reused, never applied twice.

## USD grant audit metadata

After the ledger and studio budget steps, `grant_audit` matches available
positive USD `grant` and `studio_grant` credits with `on_demand_credits`
metadata to the imported paid ledger row by wallet and `tx_ref`. It writes
one `ops.admin_audit` row and links it through `credits.ledger.admin_audit_id`.

| Source | Audit field |
| --- | --- |
| `on_demand_credits.granter_address` | `actor`, the lowercased wallet |
| `user_credits.user_address` and `timestamp` | `target`, the lowercased recipient, and `at`, the grant timestamp in UTC |
| `on_demand_credits.reason`, `annotation` and `credit_id` | `detail.reason`, `detail.annotation` and `detail.upstreamCreditId` |
| `studio_grant` and its imported budget link | `detail.studioName`, null when the name is unavailable |
| Grant audit | `domain = 'credits'`, `action = 'grant'` |

The credits report classifies grants with `studioName` as `studio_grant` and
audited Decentraland grants as `grant`. IAP, sale proceeds and other USD
sources folded into ledger grants keep their `migrated` report source.

The audit insert and the ledger link commit together. A rerun fills missing
links and preserves existing node or imported audit links. The money and
wallet balances stay the same. Each batch reports the number imported,
skipped because the audit was already linked, and skipped because there was
no matching ledger row. Each unmatched available grant produces a warning
with its upstream credit ID. A dry run reports the same categories for its
rolled-back changes.

## Not migrated

- MANA-denominated `user_credits` (goal, on_demand, legacy iap) and
  `rollbacked_credits` and `on_demand_credits` metadata for MANA: each
  MANA credit is signed by upstream and spendable on the CreditsManager until
  its `expires_at`. Counting it in the node's balance would let it be spent twice.
- USD credits whose `available_at` is in the future (held sale proceeds): the
  node has no hold. A later run imports each once it is available.
- `usd_purchase_intents` as `credits.authorizations`: upstream signed their
  salts; the node's usage index would settle a matching CreditUsed log again.
- `stripe_events`, `stripe_payment_refunds`, `stripe_payment_disputes`:
  upstream endpoint's de-duplication and reconciliation state; their outcome
  is in `purchases` and the refund ledger rows.
- `iap_credits`, `iap_account_tokens` (no IAP on the node), `seasons`,
  `goals`, `season_goals`, `user_progress`, `sale_proceeds`,
  `proceeds_cursor`, `proceeds_abandoned_fills`, `shop_daily_summaries`,
  `usd_purchase_alerts`: no target; money effects are `user_credits` rows.
- Left to the node: `packs`, `idempotency`, `captcha_challenges`, `carts`,
  `cart_items`, `checkouts`, `fulfillment_outbox`, `authorizations`,
  `authorization_lines`, `usage_settlements`, `revocations`, `usages`,
  `usage_index_cursor`.

No watermarks; every run re-scans and is idempotent.
