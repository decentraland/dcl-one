# notifications-workers → social

Upstream: `processor/src/migrations` at the pinned revision. Every upstream
time is an epoch-ms `bigint`; extracts convert it exactly
(`'epoch' + ms * 1 ms`). Addresses are lowercased; rows whose address is not
`0x` + 40 hex are dropped (the node's `Address` cannot decode them).

| Step | Upstream | Target | Notes |
| --- | --- | --- | --- |
| `subscription` | `subscriptions` | `social.subscriptions` | `details` copied as is (same `SubscriptionDetails` document; the node normalizes on read). Blank email becomes NULL. Newest `updated_at` wins, also against rows the node wrote. `is_credits_workflow` is not stored upstream: false on insert, kept on update. |
| `unconfirmed_email` | `unconfirmed_emails` | `social.unconfirmed_emails` | Code kept: neither system expires codes and the node's confirm route checks length (32) and equality only, so links already mailed keep working. `created_at` = upstream `updated_at` (last issue). `source` is not stored upstream: `'account'`. A newer pending request on the node wins. Each row written is recorded as `(address, code)` in `dcl_migration.unconfirmed_email_loaded`. |
| `unconfirmed_email_reconcile` | `unconfirmed_emails` (addresses, whole set) | `social.unconfirmed_emails` | Deletes, see Deletions. |
| `opt_out` | `notification_opt_outs` | `social.subscription_opt_outs` | Community `scope_id` canonicalized to the hyphenated lowercase uuid, as the node writes it; opaque ids kept. Earliest `created_at` wins. Each row this load inserts is recorded with its `created_at` in `dcl_migration.notification_opt_out_loaded`; an opt-out the node already held is not. |
| `opt_out_reconcile` | `notification_opt_outs` (keys, whole set) | `social.subscription_opt_outs` | Deletes, see Deletions. |
| `notification` | `notifications` with an address | `social.notifications` | Id preserved, except ingest-keyed rows (Cutover and duplicate events). `occurred_at` = `timestamp` (values below 1e11 are epoch seconds and are scaled); `read` = `read_at IS NOT NULL`; `in_app` = true (upstream showed every row). Read state only moves forward, so reads on the node survive a re-import. |
| `broadcast_read` | `broadcast_read` ⨝ `notifications` | `social.broadcast_notifications`, `social.broadcast_reads` | Preserves the source broadcast ID (ingest-keyed rows take the ingest ID) and each normalized wallet's read receipt. Can seed missing parent broadcasts when run independently. |
| `broadcast` | `notifications` without an address | `social.broadcast_notifications`, `social.broadcast_reads` | One global row per source ID, visible to new wallets as well. Converts copies from the former loader and preserves their read state. |

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

## Global broadcasts

A broadcast is stored once and listed for every wallet, including wallets absent from both
databases at import time. The target's broadcast retention horizon still applies. Read
receipts are keyed by broadcast ID and wallet; repeated imports retain reads made on the node.
The load also converts former per-wallet copies, preserving their reads before removing
only copies whose broadcast key and deterministic ID both match that loader's output.
No notification deliveries are created.

The broadcast cursor is now `(updated_at, id)`, and governance and reward rows now take the
ingest id (below). After upgrading an earlier import, run these once, in this order, before
governance or rewards post to the node:

```sh
python3 tooling/migration/migrate.py run notifications-workers --step notification --restart
python3 tooling/migration/migrate.py run notifications-workers --step broadcast --restart
python3 tooling/migration/migrate.py run notifications-workers --step broadcast_read --restart
```

The `broadcast` restart converts every historical per-wallet copy, and each restart moves the
rows it re-reads from the upstream id to the ingest id.

## Deletions

Rows carry no provenance, so the loads record what they wrote in engine
tables: `dcl_migration.notification_opt_out_loaded` (key and `created_at`)
and `dcl_migration.unconfirmed_email_loaded` (address and code). The
`*_reconcile` steps are `whole` steps: one batch holding the full upstream
set, every run (no watermark, `--since` does not narrow them). Each deletes
a recorded row that upstream no longer holds, and forgets it, when the
target row still holds the recorded value:

- an opt-out removed upstream goes unless the node created the row (never
  recorded) or recreated it (another `created_at`);
- a pending email confirmed or withdrawn upstream goes unless the node
  issued its own code since (another code).

A row written natively after cutover is never recorded, so it is never
deleted. A reconcile whose upstream set is empty while recorded rows exist
fails instead of deleting them all (a wrong source database looks like
that), unless `--allow-empty` names the step. Subscriptions and notifications are never deleted upstream and are not
reconciled.

## Cutover and duplicate events

Upstream deduplicated on `(event_key, type, address)`; the node has no
event key column, and its producers dedupe on their own deterministic ids.

### External producers (ingest endpoint)

Governance and rewards keep posting to the node's `POST
/notifications/notifications`, which keys a row by uuid v5 over the compact
JSON `[type, address, eventKey]` (address lowercase, `null` for a broadcast)
and on a repeat updates the row without emailing. The import gives every
`governance_*` and `reward_*` row with a non-empty `event_key` that same id
(a PL/pgSQL SHA-1, since `schemas/` installs no pgcrypto), in
`social.notifications`, `social.broadcast_notifications` and
`social.broadcast_reads`, so a post-cutover re-post of an imported event
updates it instead of adding a second row and email. Other types keep the
upstream id. Upstream rows that collapse onto one id (addresses differing
only in case, or a repeated broadcast) keep the newest payload and the
earliest read. A row an earlier import stored under the upstream id moves to
the ingest id, with its read state, when a step reads it again; an incremental
run reads only rows near its watermark, so an import made before ingest ids
needs the upgrade restarts above. Until then an old-id row and a re-post of
its event are two rows. Whichever
system receives a post first, later passes merge onto one row; an event posted to both before
the final pass is still emailed by each.

### Node producers

An event that upstream's processor and a node producer both turn into a
notification is stored twice if both run. The import cannot give the
upstream row the node producer's id, because the keys do not line up:
community events key on the producer's own `Date.now()` and upstream
batches suffix `-batch-N`; event notices key on the node's intent ids,
moderation notices on the node's ban ids, world permission notices on the
node's change time, quota notices on the block's creation time in
microseconds; LAND transfers are a type upstream did not emit.

So the rule is a cutover order:

1. Import while upstream runs (`run`, then `run --since last` as needed),
   with the node's producers of upstream types disabled or not yet serving
   these wallets.
2. Stop upstream's processor (and its producers' SNS feed) so it writes
   nothing more.
3. `run notifications-workers --since last` (the final pass, reconciles
   included), then `run notifications-workers --step broadcast --restart`.
4. Enable the node's producers.

Types that collide if 4 happens before 2 (step 4 does not cover the
external producers above): the community, moderation
(`banned`, `ban_lifted`, `ban_warning`, scene bans), event, worlds and
streaming notices the node emits. Rows produced in both systems during an
overlap stay duplicated; no pass removes them.

## Watermarks

Every upsert step uses upstream `updated_at` (`read_at` for `broadcast_read`);
a read or a metadata upsert moves it, so `--since last` picks both up. The
`*_reconcile` steps have none: they read the whole set every run.

Import everything, settings and the whole notification history, with
`run notifications-workers`, then `run notifications-workers --since last`
until upstream writes stop, following the cutover order above.

## Caveats

- Events notified by upstream's processor and a node producer during an
  overlap are stored twice (Cutover and duplicate events); external
  producers' re-posts are not.
- An earlier broadcast checkpoint with a shard key requires `--restart` (Global broadcasts).
