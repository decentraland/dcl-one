-- Permit explicitly bootstrapped interaction capture while retaining historical
-- favorites-only adapter rows. This does not upgrade a source or change any
-- stream's scope, generation, cursor or adapter identity.
ALTER TABLE IF EXISTS migration_sync.capture_sources DROP CONSTRAINT IF EXISTS capture_sources_adapter_check;
ALTER TABLE IF EXISTS migration_sync.capture_sources ADD CONSTRAINT capture_sources_adapter_check CHECK (adapter IN ('places_outbox_v1', 'places_outbox_v2'));
