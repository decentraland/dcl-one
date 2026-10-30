-- Recoverable native ended-occurrence facts. No FK to the mutable event
-- catalogue: event removal must not erase an already applied contribution.
CREATE TABLE social.event_ended_cursor (
    singleton boolean PRIMARY KEY DEFAULT true CHECK (singleton),
    due_at timestamp without time zone NOT NULL,
    event_id text NOT NULL DEFAULT '',
    occurrence_start timestamp without time zone NOT NULL DEFAULT '0001-01-01'
);

CREATE TABLE social.event_ended_occurrences (
    event_id text NOT NULL,
    occurrence_start timestamp without time zone NOT NULL,
    ended_at timestamp without time zone NOT NULL,
    community_id uuid,
    attendees bigint NOT NULL CHECK (attendees >= 0),
    recorded_at timestamp without time zone NOT NULL DEFAULT (now() AT TIME ZONE 'UTC'),
    PRIMARY KEY (event_id, occurrence_start)
);

-- Projection of the immutable occurrence facts, updated in the same commit.
-- Community deletion removes its projection; facts retain their original ID.
CREATE TABLE social.community_event_metrics (
    community_id uuid PRIMARY KEY REFERENCES social.communities(id) ON DELETE CASCADE,
    events_count bigint NOT NULL DEFAULT 0 CHECK (events_count >= 0),
    events_total_attendees bigint NOT NULL DEFAULT 0 CHECK (events_total_attendees >= 0)
);
