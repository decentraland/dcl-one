# ADR: Credits usage notices share the selected season's identity per expiry

- Status: Accepted
- Date: 2026-11-22

## Context

A wallet can hold seasonal MANA grants from several seasons with overlapping
expiry windows. Separate notices by grant origin split the balance and let an
older notice key suppress a later season's notice. Changing the keys alone can
queue another email for someone who already received that season's reminder.

A key made only of the wallet, kind and selected season also merges different
expiry batches. When a later batch enters the window while the same season is
still selected, the upsert refreshes the earlier notice's metadata and creates
neither an unread notice nor an email for the later batch.

## Decision

Each usage reminder kind sums all eligible grants per wallet. Its identity is
the current or last season's key qualified by the aggregate's full expiry
(`max(expires_at)` as UTC microseconds since the epoch):
`<address>-<sub-type>-<season>-expiry-<micros>`. Seven-day and one-day
reminders are independent. A newly created notice retains the deterministic
UUID derived from that full key.

The shared notification writer takes a per-wallet transaction advisory lock.
An existing qualified key wins. Otherwise, it attaches the qualified key to one
same-kind notice under an exact unqualified key (origin-season or selected
season, all started seasons) that:

- was first created since both the selected season started and this expiry's
  reminder window opened (`expires_at` minus seven days or one day); and
- carries metadata containing the same `expirationDate`, a filter the caller
  supplies so the shared writer holds no Credits knowledge.

The notice retains its UUID, creation time, read state and delivery records;
its metadata and occurrence time refresh through the ordinary shared upsert.
Other historical notices remain. Reconciliation and the upsert commit or roll
back together.

## Consequences

Replays, concurrent passes, restarts and balance growth with the same expiry
create no second delivery for one batch. A later expiry batch in the same
selected season receives its own notice and delivery; an earlier notice keeps
its key, read state and delivery. An older season's notice does not suppress a
new season's notice, and a leftover old key cannot absorb a later batch.

Because the aggregate uses the latest expiry, a change in that latest expiry
(a new later grant, or consumption or expiry of the latest one while earlier
credits remain in the window) is a different identity and may notify again.

The same still-unused credits can still produce a seven-day reminder on each
side of a season change. With one to six grace days, the expiry window can open
before the granting season ends. A gap between seasons can also let the window
open while that season remains the last selected season, including with longer
grace periods. If the next season starts before expiry and the wallet has
progress in it, its key permits another reminder and email, subject to email
preferences. Zero grace does not leave credits unexpired after their granting
season ends.

Old keys hold only a month and day in metadata. When two batches expire on the
same calendar day, an old notice cannot be shown to announce one of them; the
first qualified run treats it as that day's batch. Manual, incomplete or deleted
history is not reconstructed and is not claimed as reconciled.

Existing partial notices and already queued emails are not deleted or recalled.
No new table, index, process or service dependency is needed. Retire writers
using origin-season keys and writers using unqualified selected-season keys
before the corrected leased job runs: those writers do not participate in its
notification lock.

## Verification

PostgreSQL regressions cover two expiry batches within one selected season for
both kinds (first notice read and sent, second unread and pending, replays
add nothing), combined balances, qualified keys, concurrent fresh inserts and
reconciliation, existing qualified notices, previous-season history,
sent/pending/failed/skipped and retained-away deliveries, expired origin grants,
the first corrected run after an old batch expired, leftover aliases, an alias
announcing another date inside the later window, balance growth with the same
expiry, a restarted job, email preferences, and rollback after a notification
failure. Unit tests cover microsecond-distinct keys and UUIDs. Existing tests
cover expiry boundaries and consumed credits.
