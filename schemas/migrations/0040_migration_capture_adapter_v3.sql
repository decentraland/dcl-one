-- Permit reviewed catalogue-inclusive baselines without relabelling history.
ALTER TABLE IF EXISTS migration_sync.capture_sources DROP CONSTRAINT IF EXISTS capture_sources_adapter_check;
ALTER TABLE IF EXISTS migration_sync.capture_sources ADD CONSTRAINT capture_sources_adapter_check CHECK (adapter IN ('places_outbox_v1','places_outbox_v2','places_outbox_v3'));
