-- Social owns the notification audience of a community voice generation.
-- The executor creates absent tables/indexes from the canonical object pass.
ALTER TABLE social.communities ADD COLUMN IF NOT EXISTS voice_notification_scope text;
ALTER TABLE social.communities ADD COLUMN IF NOT EXISTS voice_notification_expires_at timestamp without time zone;
ALTER TABLE social.communities ADD COLUMN IF NOT EXISTS voice_notification_generation uuid;
ALTER TABLE social.communities ADD CONSTRAINT communities_voice_notification_scope_check CHECK (
    (voice_notification_scope IS NULL AND voice_notification_expires_at IS NULL AND voice_notification_generation IS NULL)
    OR (voice_notification_scope IS NOT NULL AND voice_notification_scope IN ('all', 'members') AND voice_notification_expires_at IS NOT NULL AND voice_notification_generation IS NOT NULL)
);
