-- Existing profile permissions were self-writable and must not become authority.
-- Moderators explicitly re-grant them; preference updates do not grant authority.
ALTER TABLE IF EXISTS explore.event_profile_settings ADD COLUMN IF NOT EXISTS permissions_granted boolean NOT NULL DEFAULT false;
