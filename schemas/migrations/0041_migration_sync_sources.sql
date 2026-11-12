-- Extend the reviewed source allowlist without changing existing ownership.
ALTER TABLE IF EXISTS migration_sync.streams DROP CONSTRAINT IF EXISTS streams_source_check;
ALTER TABLE IF EXISTS migration_sync.streams ADD CONSTRAINT streams_source_check CHECK (source IN ('places', 'events', 'social-settings'));
