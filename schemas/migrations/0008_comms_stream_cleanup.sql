-- Existing streaming authorities gain lifecycle state before the object pass.
-- New cleanup table and indexes are owned by schemas/40_social.sql. A database
-- stamped before social.scene_stream_access existed (the closure base) takes
-- the whole table, these columns included, from the object pass that follows
-- the steps, so the step applies only where the table already is.
ALTER TABLE IF EXISTS social.scene_stream_access ADD COLUMN IF NOT EXISTS streaming_state_version bigint NOT NULL DEFAULT 0;
ALTER TABLE IF EXISTS social.scene_stream_access ADD COLUMN IF NOT EXISTS streaming_checked_at timestamp without time zone;
ALTER TABLE IF EXISTS social.scene_stream_access ADD COLUMN IF NOT EXISTS cleanup_retry_at timestamp without time zone;
