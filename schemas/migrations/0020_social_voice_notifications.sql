-- Social owns the transient notification audience of a community voice generation.
-- Existing rooms without a recorded scope continue to use member-only cleanup.
ALTER TABLE social.communities ADD COLUMN IF NOT EXISTS voice_notification_scope text;
ALTER TABLE social.communities ADD COLUMN IF NOT EXISTS voice_notification_expires_at timestamp without time zone;
ALTER TABLE social.communities ADD COLUMN IF NOT EXISTS voice_notification_generation uuid;
DO $$ BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conrelid='social.communities'::regclass AND conname='communities_voice_notification_scope_check') THEN
        ALTER TABLE social.communities ADD CONSTRAINT communities_voice_notification_scope_check CHECK (
            (voice_notification_scope IS NULL AND voice_notification_expires_at IS NULL AND voice_notification_generation IS NULL)
            OR (voice_notification_scope IS NOT NULL AND voice_notification_scope IN ('all', 'members') AND voice_notification_expires_at IS NOT NULL AND voice_notification_generation IS NOT NULL)
        );
    END IF;
END $$;
CREATE TABLE IF NOT EXISTS social.event_payloads (
    id uuid PRIMARY KEY,
    payload bytea NOT NULL,
    expires_at timestamp without time zone NOT NULL,
    CONSTRAINT event_payloads_size_check CHECK (octet_length(payload) > 0 AND octet_length(payload) <= 536870912)
);
CREATE INDEX IF NOT EXISTS event_payloads_expiry_idx ON social.event_payloads (expires_at, id);

-- The 45 s notification poll walks only currently tracked generations in PK pages.
CREATE INDEX IF NOT EXISTS communities_voice_notifications_idx ON social.communities(id)
  WHERE voice_notification_generation IS NOT NULL;
