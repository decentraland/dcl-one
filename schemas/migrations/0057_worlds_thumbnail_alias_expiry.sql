-- Mirror-observed aliases expire unless refreshed. Existing aliases have no
-- recorded provenance, so keep them pinned, as with verified imports.
ALTER TABLE IF EXISTS worlds.thumbnail_aliases
    ADD COLUMN IF NOT EXISTS mirror_expires_at timestamp with time zone;
