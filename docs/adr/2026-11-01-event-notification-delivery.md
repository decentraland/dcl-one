# ADR: Event notification delivery and signed profile mutations

- Status: Proposed
- Date: 2026-11-01

## Context

Event moderation and authoring belong to Explore. Social owns notification
delivery records and subscription preferences. An event change and its notice
must commit together, while delivery must survive worker restarts and retries.
The components share one PostgreSQL database and one process; adding an internal
HTTP call or a separate broker would not improve this transaction boundary.

Profile settings support both signed federation envelopes and signed-fetch JSON.
Administrative permission grants and private profile reads require a moderator.
A signed-fetch signature over a method and pathname alone does not bind mutation
contents or prevent a captured request from being applied again.

## Decision

Social owns `social.event_notification_intents`, including the event snapshot,
recipient progress and completion state. Explore inserts transition intents into
this relation in the transaction that updates the event. This is an explicit
SQL-level dependency on the shared database, without a reverse Rust dependency.
Social's worker reads Explore's catalogue and local RSVP records to discover
occurrences and rechecks recipient eligibility before inserting delivery records.

Lifecycle discovery uses a rolling one-hour catch-up window and an indexable live
event bound and an upper start-time bound. The same catch-up policy applies on
first deployment: still-running occurrences with a local audience that started
within the last hour may generate a start notice. Only occurrences with eligible
local audiences enter the queue.
Stable occurrence identifiers and a unique occurrence constraint retain durable
scan progress, including due-time ties and late arrivals; there is no separate
timestamp cursor that could permanently skip a newly approved event.

The worker drains recipient pages within a five-second dispatch budget. Cleanup runs
first with its own bounded opportunity, independent of dispatch backlog. Reminder
and start scans receive separate three-second budgets so one cannot starve the other;
scan statements have a two-second server timeout. Scan timeouts and budget
exhaustion emit error-level diagnostics and defer remaining discovery without
activating job backoff, preserving the normal delivery cadence. Other scan
database failures reach job error reporting after queued delivery gets its turn.
Cleanup failures and cleanup budget exhaustion emit error-level diagnostics
without preventing scanning or delivery.
Transaction-local PostgreSQL statement and lock timeouts bound server work as
well as client waits. Each claim commits a queue-order rotation before processing;
a timed-out page therefore retains its recipient cursor without keeping the
oldest queue position. Recipient candidates are limited within each ordered
audience branch before combining them. Cursor advancement,
notification insertion and email-outbox insertion commit atomically. Completed
queue entries are retained for 30 days and removed in bounded batches only after
their occurrence and processing times are older than retention. This exceeds the
catch-up window, preventing cleanup from making old lifecycle events eligible
again. Unfinished entries are retained. Reminder and start delivery checks reject
stale occurrences and recheck the current event schedule and payload under a
shared event-row lock before each page, so an old queued start is suppressed after
rescheduling. Committed suppression emits a structured warning identifying the
intent, event, kind and reason, including notices that expired while deferred by
lock contention. An already processed or
suppressed occurrence remains deduplicated if the same occurrence is later
reapproved. Outage recovery does not promise delivery of every missed historical
notice. New notification rows use the database delivery clock for `occurred_at`,
avoiding source-time backdating of catch-up notices in incremental bell polling.
This preserves the feed's existing timestamp cursor semantics; it does not
establish a commit-ordered watermark across concurrent transactions. Source event timing
remains in the notification metadata. Titles, descriptions and links are the
old events service's (`Notifications/index.ts`): the reminder is "Event starts in
an hour" and links the jump-in site (`JUMP_IN_SITE_URL`), as that service sent it,
even for an occurrence approved with less than an hour to go (its cursor skipped
those; this node reminds them once).

Event notification channel preferences come from Social subscriptions, including
per-type settings and community opt-outs. Community UUID scopes are normalized
on write and lookup; event and community delivery also recognize existing
non-canonical UUID scopes. Lookup parameters are parsed in Rust and bound as
optional UUIDs, preserving opaque legacy scope IDs without unsafe parameter casts.
UUID spellings accepted by Rust, including `urn:uuid:` prefixes, refer to the
same canonical community scope. Checks and deletion remain restricted to the
authenticated caller's address; opaque non-UUID IDs retain exact matching.
Legacy Explore profile notification
settings remain a separate contract. Event-only in-app suppression does not
change unrelated producers. Suppressed event rows are stored as read with a
read timestamp, keeping them out of the unread index while preserving independent
email delivery.

Plain profile PATCH requests bind the exact body bytes through `body_sha256` and
a canonical UUID `nonce` in signed metadata. Nonce consumption and the profile
write commit together; a failed write does not consume the nonce. Signed
federation envelopes retain their typed-payload signature and nonce checks.
Only moderators may grant permissions or administer/read other profiles. A
delegated event permission does not grant profile-administration authority.
Both schedule-write encodings accept moderators or an explicitly trusted
`edit_any_schedule` grant and observe revocation through the same authority check.

## Verification and limits

### Monitoring scan deferrals

A successful periodic pass does not prove occurrence discovery completed: scan
timeouts deliberately preserve the normal delivery cadence. Configure the
deployment's log alerting to match error-level events from
`catalyst_social::notifications::events` with
`operation="event_occurrence_scan"` and `reason="database_timeout"` or
`reason="budget_exhausted"`. The corresponding messages are
`event occurrence scan deferred; retaining normal delivery cadence` or
`event occurrence scan exceeded budget; retaining normal delivery cadence`.
Group by occurrence `kind` and alert when deferrals persist across three normal
one-minute passes. Monitor `event intent cleanup deferred` and
`event intent cleanup exceeded budget` independently; delivery can succeed while
retention is failing. Alert routing is deployment configuration and is not
installed by this PR.

On an alert, check database lock waits and statement duration, catalogue size and
recurrence-list lengths, and the age of unfinished
`social.event_notification_intents`. Check committed suppression warnings for
`event_ended` or `reminder_expired` to identify notices that aged out. Restore
discovery and retention capacity without enabling job backoff for expected scan
deferrals or delivering expired notices. The rolling catch-up window remains one
hour; recovery does not replay notices that have already aged out.

### Regression coverage

Regression coverage must include moderator authority on self and other-address
paths, altered bodies and replay, channel preferences, local audience filtering,
late approvals, stale notices, due-time ties beyond one page, concurrent workers,
retention and schema upgrades. Query plans and measured test workloads accompany
the implementation report; they are not a production capacity guarantee.

This change adds no service, process, transport or dependency. The schema changes
are additive for released databases. The unpublished queue layout belongs to this
feature's migration and object-creation path. Production delivery, restore and
rollback readiness require separate evidence.
