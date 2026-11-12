# Events: next native command coverage

Status: source and target paths reviewed; Events replay is **not implemented or enabled**. The next endpoint should be `PATCH /api/schedules/{uuid}`, followed by schedule creation, then RSVP/cancel. These are command candidates, not reasons to copy their tables continuously.

Source evidence is refreshed Events [2436e598](https://github.com/decentraland/events/tree/2436e598b0e706df24a47e92d438374875f912da), the current fetched `origin/master`. This is not confirmation of the deployed revision. See the [decision summary](README.md).

## Schedule edits first

The [source handlers](https://github.com/decentraland/events/blob/2436e598b0e706df24a47e92d438374875f912da/src/entities/Schedule/routes.ts) authenticate the caller, validate the body, require admin or schedule-editor permission, load an existing row and merge the supplied fields. No provider call, notification or scheduled schedule writer was found in these paths. A missing schedule is an error, not an upsert.

The [native target implementation](../services/catalyst/explore/src/events/profiles.rs) already validates, locks and updates schedules. Its ordinary HTTP path also verifies a client signature and records a signed action. Calling that route with an invented signature is not a replay solution. Extract the native mutation into a transaction-level helper used by both HTTP and private replay; private replay consumes trusted source acceptance and must not manufacture client signature history.

The required end-to-end increment is:

1. **Source acceptance:** default-off authenticated handler path. Commit the update, resolved input, source timestamp, original response and receipt atomically. Read/merge under the same source serialization boundary; retain the difference between absent fields and explicit null. Hash the request identity before generated values are added. Detect conflicting ID reuse.
2. **Events stream:** add a separate typed source identity, immutable schedule scope, baseline and ordered capture path. The existing worker is Places-specific; changing only its source string or sharing a Places cursor is incorrect. Observe unreceipted schedule writes so they block continuity claims.
3. **Native target execution:** apply the same accepted field setters to a migrated schedule, then compare expected state and commit its receipt atomically. Preserve accepted timestamps; no placeholder creation, generic JSON/table mapper, permission re-evaluation at a later replay time, or external effects. Baseline ownership must exclude concurrent native edits.
4. **Gateway:** allow the exact PATCH path only after both receipt implementations exist. Preserve signed path and body, keep production's response and strip migration headers elsewhere. Required stable operation identity first; assess per-attempt policy separately.
5. **Verification:** omitted/null fields, empty patch, stale concurrent edits, source rejection, lost responses after either commit, changed-payload retries, outside-scope writes and blocked-head ordering. Compare the read-back schedule as well as the PATCH response.

The source update uses inherited model behavior; the installed dependency adds `updated_at` on updates. The target does too. This does **not** justify a timestamp watermark: explicit source timestamps, deletes and bypass writers still need ordered evidence. Confirm the installed/deployed dependency before finalizing its receipt contract.

## Creation and RSVP require additional contracts

| Operation | Why two ordinary requests are insufficient today | Required native replay change |
| --- | --- | --- |
| `POST /api/schedules` | Both implementations generate their own UUID and clock values. Source creation also forces `theme` to null. | Source-generated ID/timestamps become accepted command inputs; duplicate creation resolves the same durable receipt. Do not generate a new target ID on retry. |
| `POST /api/events/{uuid}/attendees` | [Source](https://github.com/decentraland/events/blob/2436e598b0e706df24a47e92d438374875f912da/src/entities/EventAttendee/routes.ts) creates attendance and later updates counts in separate commits. The source also resolves profile name. | Establish atomic acceptance across the coupled source changes, or explicitly model the intermediate outcomes. Carry source identity/name/time; use native target execution and a complete attendance baseline. |
| `DELETE /api/events/{uuid}/attendees` | Source physically deletes; [target](../services/catalyst/explore/src/events/attendees.rs) retains a cancel marker to override mirrored previews. | Native cancellation, not row deletion. Verify merged attendee list/count semantics and prevent target mirrors from resurrecting attendance. |

Schedule coverage alone does not establish full Events continuity. Events, permissions, attendees, recurrence and poster assets remain separate dependencies. The private follower still cannot serve these public APIs.

## Recurrence job prerequisite fixed locally

The [source recurrence job](https://github.com/decentraland/events/blob/2436e598b0e706df24a47e92d438374875f912da/src/entities/Event/cron.ts) starts `EventModel.update` without awaiting it. The local Events change awaits each update: completion now follows persistence, failures reach the scheduler, and a later event does not start while an earlier write remains pending. Six focused tests, TypeScript checking and scoped lint pass.

This change is needed to make job completion usable as evidence. It does not install DB sync or prove the recurrence result needs copying. Recurrence still needs a comparison of deterministic input/time replay versus a narrowly identified committed outcome. Production remains the only recurrence and notification-job owner.
