# notifications-workers → social

Upstream: `processor/src/migrations` at the pinned revision. Every upstream
time is an epoch-ms `bigint`; extracts convert it exactly
(`'epoch' + ms * 1 ms`). Addresses are lowercased; rows whose address is not
`0x` + 40 hex are dropped (the node's `Address` cannot decode them).

| Step | Upstream | Target | Notes |
| --- | --- | --- | --- |
| `subscription` | `subscriptions` | `social.subscriptions` | `details` copied as is (same `SubscriptionDetails` document; the node normalizes on read). Blank email becomes NULL. Newest `updated_at` wins, also against rows the node wrote. `is_credits_workflow` is not stored upstream: false on insert, kept on update. |
| `unconfirmed_email` | `unconfirmed_emails` | `social.unconfirmed_emails` | Code kept: neither system expires codes and the node's confirm route checks length (32) and equality only, so links already mailed keep working. `created_at` = upstream `updated_at` (last issue). `source` is not stored upstream: `'account'`. A newer pending request on the node wins. |
| `opt_out` | `notification_opt_outs` | `social.subscription_opt_outs` | Community `scope_id` canonicalized to the hyphenated lowercase uuid, as the node writes it; opaque ids kept. Earliest `created_at` wins. |
| `notification` | `notifications` with an address | `social.notifications` | Id preserved. `occurred_at` = `timestamp` (values below 1e11 are epoch seconds and are scaled); `read` = `read_at IS NOT NULL`; `in_app` = true (upstream showed every row). Read state only moves forward, so reads on the node survive a re-import. |
| `broadcast` | `notifications` without an address | `social.notifications` | The node stores a broadcast per recipient: one row per `social.subscriptions` address (the node's own broadcast audience), `broadcast_key = 'notifications-workers:<upstream id>'`, id `md5(upstream id ':' address)::uuid`. Runs after `subscription`. |
| `broadcast_read` | `broadcast_read` ⨝ `notifications` | `social.notifications` | Marks that reader's copy read, creating it when the reader has no subscription. |

All notification types are imported, including ones no node producer emits
(governance, rentals, credits...): the bell lists rows regardless of type,
upstream did the same, and the client decides what it renders.

## Not migrated

- `cursors`: upstream producer checkpoints; the node's producers keep their own.
- `pgmigrations`.

## Left to the node

- `notification_deliveries`: no rows for imported history, so the outbox never
  e-mails it again. Upstream sent its mail at insert time and kept no state.
- `notification_reader_seen`, `event_notification_intents`: no upstream
  equivalent.

## Watermarks

Every step uses upstream `updated_at` (`read_at` for `broadcast_read`); a read
or a metadata upsert moves it, so `--since last` picks both up.

Import everything, settings and the whole notification history, with
`run notifications-workers`, then `run notifications-workers --since last`
until upstream writes stop.

## Caveats

- Deletions are not seen: an opt-out removed upstream, or a code consumed
  upstream, after its row was copied stays. Do the final pass after upstream
  writes stop.
- A broadcast reaches only the subscriptions present when its batch loads;
  re-run `broadcast` with `--restart` after the final `subscription` pass.
- Rows re-produced by node producers for the same upstream event get the
  node's own id and are not deduplicated against imported ids.
