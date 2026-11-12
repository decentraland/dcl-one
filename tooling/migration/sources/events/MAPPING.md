# events → catalyst

Source: `github.com/decentraland/events` at the `revision` in `source.toml`;
table shapes are the end state of `src/migrations/*` there.

## Authority

The import makes this node the authority for the catalogue: every imported
event is `explore.event.origin = 'local'`, shaped as `events/moderation.rs`
`create_as` shapes a local event (`raw` is the API document, RFC 3339 UTC
timestamps). The events mirror (`events/mirror.rs`) never overwrites or sweeps
a local row. A row the mirror already wrote is taken over, with any operator
moderation of it (`explore.event_moderation`) laid over the upstream flags the
way the mirror's upsert does; the `event_moderation` row stays and is inert
for a local event. A local event this node has edited since (an
`explore.events_local` row) is never overwritten by a re-run.

With `EVENTS_MIRROR_UPSTREAM` still set after the import, the mirror keeps
adding events created upstream later as `'upstream'` rows; the next
`run events --since last` converts them. Unset it at cutover.

## Steps

| Step | Source table | Target | Key | Watermark |
| --- | --- | --- | --- | --- |
| `schedule` | `schedule` | `explore.schedules_local` | `id` | none (small; update path not verified to bump `updated_at`) |
| `event` | `events` | `explore.event` | `id` | `updated_at` (`EventModel.update` sets it on every write, attendee counts and the recurrence cron included) |
| `attendance` | `event_attendees` | `explore.event_attendance_local` (`going`) | `event_id, signer` | none (`created_at` nullable; a cancel deletes the row) |
| `profile_subscription` | `profile_subscriptions` | `explore.event_profile_subscriptions` | `endpoint` | none (source has no timestamps) |
| `profile_settings` | `profile_settings` | `explore.event_profile_settings` | `user` | none (updates keep the old `updated_at`) |

Notes:

- `raw.total_attendees` is 0 and `raw.latest_attendees` is empty: the RSVPs
  become local rows, which the read path counts on top of the preview.
  Attendees of events missing upstream are skipped (the mirror sweep deletes
  orphan RSVPs). An RSVP or cancel this node recorded later wins.
- Posters on the upstream bucket (`events-assets-<hex>.decentraland.*/poster[-vertical]/<file>`)
  become `/poster[-vertical]/<file>`, the path local writes store. The poster
  bytes are not in the database: they come through the poster read-through
  (`EVENTS_POSTER_UPSTREAM_URL`) or a separate bucket copy.
- `deleted_at` is `timestamp` (UTC) upstream; the other event and schedule
  timestamps are `timestamptz`. `profile_settings.created_at/updated_at` are
  `timestamp` (gatsby `Type.TimeStampTZ`) and are read as UTC.
- Profile `permissions` are trusted: upstream's admins are their only
  writers, so a non-empty list is imported with `permissions_granted = true`
  (`schemas/migrations/0014`). A row saved on this node after upstream's last
  save keeps its own permissions and grant.
  `email_verified_at` and `email_updated_at` are also preserved as UTC, including nulls.
- Browser subscription endpoints, users and push keys are retained for the existing
  settings/subscription APIs; the target does not send browser pushes. The source
  has no creation timestamp, so the target stamps first import and preserves it
  on reruns. Keep target subscription writes disabled during import: these rows
  cannot be ordered against target edits.
- Deletions upstream are not propagated (engine contract), except event
  soft-deletes, which are flags in `raw`.

## Not migrated

- `event_categories`: the list is fixed in `events/categories.rs`.
- `jobs`, `notification_cursors`: upstream scheduler state.
- `events.textsearch`, `coordinates`: computed at read time / from `x`, `y`.
- `explore.events_local`, `explore.signed_actions`: upstream stores no signed
  payloads, and none are fabricated.
- `explore.moderators`: upstream admins are the `ADMIN_ADDRESSES` setting,
  not a table; seed it by hand.

## Left to the node

`social.event_notification_intents` (written by the node's own transitions and
occurrence scanner), `explore.mirror_state`, `explore.poster`,
`explore.seen_nonces`.
