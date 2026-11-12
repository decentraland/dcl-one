-- Each new domain requires its own explicit baseline; retained adapters keep
-- their identity and progress. No row is relabelled by this additive step.
ALTER TABLE IF EXISTS migration_sync.capture_sources DROP CONSTRAINT IF EXISTS capture_sources_adapter_check;
ALTER TABLE IF EXISTS migration_sync.capture_sources ADD CONSTRAINT capture_sources_adapter_check CHECK (adapter IN ('places_outbox_v1', 'places_outbox_v2', 'places_outbox_v3', 'events_schedules_outbox_v1', 'social_settings_outbox_v1'));
