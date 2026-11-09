-- The one-time adoption of the mirrored events as local rows (EVENTS_MIRROR_ADOPT,
-- events/mirror.rs adopt_upstream) is recorded on the mirror's row, so it never runs twice.
ALTER TABLE explore.mirror_state ADD COLUMN IF NOT EXISTS adopted_at timestamp without time zone;
