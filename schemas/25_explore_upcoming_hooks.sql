-- Workload: opt-in minute scans enqueue at most 100 approved local occurrences
-- starting within one hour; delivery claims at most ten rows per tick, with a
-- six-attempt ceiling. Expected volume depends on native events with local RSVPs;
-- hourly series produce at most 24 intents/day. These are bounds, not measurements.
-- Authority: explore.event and event_attendance_local. Unique occurrence keys
-- deduplicate rolling scans, including late approval/RSVP and concurrent workers.
-- Lifecycle: send-time source reads suppress changed/deleted/expired occurrences;
-- completed_at NULL with attempts=6 is an observable dead letter. A bounded
-- cleanup removes occurrences older than thirty days, including after opt-out.
-- No FK to explore.event: intent/dead-letter history survives event deletion;
-- the writer and send-time source check enforce eligibility. Retention deletes it.
-- The unused name column is retained for additive compatibility with earlier
-- development layouts; new writes use an empty value and delivery reads event.name.
-- The old cursor table, if present on an existing database, is unused and retained;
-- dropping either legacy object requires a separately reviewed data-safe release.
CREATE TABLE explore.event_upcoming_hooks (
    event_id text NOT NULL,
    occurrence_start timestamp without time zone NOT NULL,
    name text NOT NULL,
    next_attempt_at timestamp without time zone NOT NULL DEFAULT (now() AT TIME ZONE 'UTC'),
    attempts integer NOT NULL DEFAULT 0 CHECK (attempts BETWEEN 0 AND 6),
    claim uuid,
    completed_at timestamp without time zone,
    PRIMARY KEY (event_id, occurrence_start)
);
CREATE INDEX event_upcoming_hooks_due_idx
    ON explore.event_upcoming_hooks (next_attempt_at,event_id,occurrence_start)
    WHERE completed_at IS NULL AND attempts < 6;

CREATE INDEX event_upcoming_hooks_retention_idx
    ON explore.event_upcoming_hooks (occurrence_start,event_id);
