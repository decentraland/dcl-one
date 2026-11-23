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

Event notices retain the exact source event identity in `metadata.eventId` and
use `broadcast_key = event-notice:<type>:<eventId>` per address. Their source time
is retained in `metadata.occurredAt`.

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

### Event notice identity and upgrade

The six event kinds (`events_starts_soon`, `events_started`, `event_created`,
`event_approved`, `event_rejected`, `event_deleted`) use the source `event_key`,
not the source timestamp, as event identity. The loader and runtime use the same
numerically ordered wallet-bucket transaction locks and keyed carrier. Both
writers use the two-integer advisory key
`(1163284052, (hashtextextended(address, 0) & 255)::integer)`, so each transaction
retains at most 256 event locks even in a whole-transaction dry run. Hash collisions
only serialize additional wallets. Existing keyed rows win;
otherwise an exact source ID or native event match retains its ID, visibility,
read state and delivery evidence. The import creates no email delivery and never
overwrites newer source metadata on the node; equal source times keep the first
payload. Source duplicates for a normalized key merge within a batch. Existing
historical duplicate rows are not consolidated.

Before event jobs are enabled, freeze writers and perform a full
`run notifications-workers --step notification --restart` import to repair keys
on previously imported retained source rows. An incremental pass cannot repair
rows outside its watermark. If a prior imported notice no longer exists in the
source and its event identity was lost, use an earlier authoritative export;
titles and optional links do not prove identity. Without that evidence, historical
deduplication coverage remains unverified and activation is not ready.

### Node producers

Other producer types can overlap when both systems run. Their import keeps
upstream IDs rather than assigning the node producer's identity. Community
events key on the producer's own `Date.now()` and upstream batches suffix
`-batch-N`; event notices share the event/type/recipient key, moderation notices on
the node's ban ids, world permission notices on the node's change time, and
quota notices on wallet, kind and the UTC date of block creation. LAND
transfers are a type upstream did not emit.

The quota writer can link exact existing native IDs when a block's full
creation timestamp is available; imported IDs remain unlinked. Unlinked
history may coexist with the first daily notice, without historical cleanup
or a second delivery for later daily repeats. Retire quota writers that do
not set daily keys before relying on daily uniqueness. The
[quota identity decision](../../../../docs/adr/2026-11-21-world-quota-notice-identity.md)
states the coverage and cutover boundary.

So the rule is a cutover order:

1. Import while upstream runs (`run`, then `run --since last` as needed),
   with the node's producers of upstream types disabled or not yet serving
   these wallets.
2. Stop upstream's processor (and its producers' SNS feed) so it writes
   nothing more.
3. `run notifications-workers --since last` (the final pass, reconciles
   included), then `run notifications-workers --step notification --restart`
   to repair retained event identities and
   `run notifications-workers --step broadcast --restart`.
4. Enable the node's producers. Seed the Shop chain notice producers from
   events-notifier's cursors first ([runbook](../../../../services/catalyst/docs/shop-notices.md#cutover-from-events-notifier)).

Types that collide if 4 happens before 2 (step 4 does not cover the
external producers above): the community, moderation
(`banned`, `ban_lifted`, `ban_warning`, scene bans), worlds and streaming
notices the node emits. Those overlapping rows stay duplicated; no pass removes
them. The six keyed event kinds converge through the shared carrier, but email
already sent independently by each system cannot be recalled. Keep event jobs
disabled until the authoritative import and historical identity checks finish.

## Watermarks

Every upsert step uses upstream `updated_at` (`read_at` for `broadcast_read`);
a read or a metadata upsert moves it, so `--since last` picks both up. The
`*_reconcile` steps have none: they read the whole set every run.

Import everything, settings and the whole notification history, with
`run notifications-workers`, then `run notifications-workers --since last`
until upstream writes stop, following the cutover order above.

## Caveats

- Other producer types can retain duplicate rows from an overlap (Cutover
  and duplicate events). Keyed event notices converge, but pre-existing
  historical duplicates are not consolidated.
- An earlier broadcast checkpoint with a shard key requires `--restart` (Global broadcasts).
