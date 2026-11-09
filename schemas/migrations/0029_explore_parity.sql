-- The disable decision's instant, projected into raw.disabled_at and re-applied
-- by the mirrors; an existing disable keeps its last decision time.
ALTER TABLE explore.place_moderation ADD COLUMN IF NOT EXISTS disabled_at timestamp without time zone;
UPDATE explore.place_moderation SET disabled_at = updated_at WHERE disabled IS TRUE AND disabled_at IS NULL;

-- The combined like rates on this node's interaction counters
-- (src/explore/places/interactions.rs): the old places service's qualified voters plus the
-- local votes with user_activity >= 100 (the Snapshot voting power the
-- favorite and like writes now fetch and store), the same Wilson lower
-- bound. Declared on the table in schemas/10_explore.sql; a database stamped
-- before it takes the two nullable columns here (one older than the table itself
-- takes them with the table from the object pass). NULL keeps the reads on the
-- mirrored value, so no backfill: the next local qualified vote on an entity
-- fills its row.
ALTER TABLE IF EXISTS explore.place_interaction_counts ADD COLUMN IF NOT EXISTS like_rate double precision;
ALTER TABLE IF EXISTS explore.place_interaction_counts ADD COLUMN IF NOT EXISTS like_score double precision;

-- The events profile settings gain the old service's email lifecycle marks and
-- the moderation outbox accepts the profile-permission notice kind. Existing
-- rows keep null marks (the old service never set them without verification).
ALTER TABLE IF EXISTS explore.event_profile_settings ADD COLUMN IF NOT EXISTS email_verified_at timestamp without time zone;
ALTER TABLE IF EXISTS explore.event_profile_settings ADD COLUMN IF NOT EXISTS email_updated_at timestamp without time zone;
ALTER TABLE IF EXISTS explore.event_moderation_hooks DROP CONSTRAINT IF EXISTS event_moderation_hooks_kind_check;
ALTER TABLE IF EXISTS explore.event_moderation_hooks ADD CONSTRAINT event_moderation_hooks_kind_check CHECK (kind IN ('submitted','edited','approved','rejected','deleted','profile_permissions'));

-- The moderation outbox accepts the reminder job's upcoming-event notice (the
-- old service's Slack notifyUpcomingEvent). Queued rows are retained; the
-- constraint is re-created with the wider set.
ALTER TABLE IF EXISTS explore.event_moderation_hooks DROP CONSTRAINT IF EXISTS event_moderation_hooks_kind_check;
ALTER TABLE IF EXISTS explore.event_moderation_hooks ADD CONSTRAINT event_moderation_hooks_kind_check CHECK (kind IN ('submitted','edited','approved','rejected','deleted','profile_permissions','upcoming'));

-- The moderation outbox names the wallet of a 'profile_permissions' notice in
-- its own column: subject holds it and event_id stays an event id (null for
-- that kind). The target check keeps the two columns exclusive for every row
-- written from here on; it is NOT VALID because the step cannot move the
-- wallets of rows queued before it (the table may not exist yet when the
-- steps run, and a conditional backfill is not an additive step). The worker
-- still reads those rows, and the 30-day retention drops them.
ALTER TABLE IF EXISTS explore.event_moderation_hooks ADD COLUMN IF NOT EXISTS subject text;
ALTER TABLE IF EXISTS explore.event_moderation_hooks ALTER COLUMN event_id DROP NOT NULL;
ALTER TABLE IF EXISTS explore.event_moderation_hooks ADD CONSTRAINT event_moderation_hooks_target_check CHECK (CASE WHEN kind = 'profile_permissions' THEN event_id IS NULL AND subject IS NOT NULL ELSE event_id IS NOT NULL AND subject IS NULL END) NOT VALID;

-- A like fact records where it came from: this node's own vote ('local') or
-- a row imported from the old places service's user_likes export ('upstream', the ground
-- truth the combined rates prefer to the reconstruction from mirrored
-- aggregates). Existing rows are this node's.
ALTER TABLE IF EXISTS explore.user_likes ADD COLUMN IF NOT EXISTS source text NOT NULL DEFAULT 'local';
ALTER TABLE IF EXISTS explore.user_likes ADD CONSTRAINT user_likes_source_check CHECK (source IN ('local', 'upstream'));

-- The counter row tells the reads how many of its like facts came from the
-- import: above zero, the facts replace the mirrored totals and rates (the
-- old places service already counted those voters). Existing rows hold no
-- imported fact, so zero is right and the next recount keeps it.
ALTER TABLE IF EXISTS explore.place_interaction_counts ADD COLUMN IF NOT EXISTS imported integer NOT NULL DEFAULT 0;
