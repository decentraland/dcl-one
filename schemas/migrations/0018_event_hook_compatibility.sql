-- Earlier event-hook development layouts predate creator throttling and buckets.
-- Keep every intent; the object pass rebuilds the two changed supporting indexes.
ALTER TABLE IF EXISTS explore.event_moderation_hooks ADD COLUMN IF NOT EXISTS creator_address text;
ALTER TABLE IF EXISTS explore.event_moderation_hooks ADD COLUMN IF NOT EXISTS edit_bucket timestamp without time zone;
ALTER TABLE IF EXISTS explore.event_moderation_hooks ADD CONSTRAINT event_moderation_hooks_event_id_kind_edit_bucket_key UNIQUE (event_id, kind, edit_bucket);
DROP INDEX IF EXISTS explore.event_moderation_hooks_due_idx;
DROP INDEX IF EXISTS explore.event_moderation_hooks_creator_idx;
