-- Optional operational summaries: one public aggregate per native occurrence,
-- sharing the moderation webhook configuration but no recipient identities.
CREATE TABLE explore.event_upcoming_hook_cursor (
    singleton boolean PRIMARY KEY DEFAULT true CHECK (singleton),
    due_at timestamp without time zone NOT NULL,
    event_id text NOT NULL DEFAULT '',
    occurrence_start timestamp without time zone NOT NULL DEFAULT '0001-01-01'
);

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
