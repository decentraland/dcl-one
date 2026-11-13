-- Admit explicit native favorite/vote command capture without relabelling old history.
ALTER TABLE IF EXISTS migration_sync.capture_sources DROP CONSTRAINT IF EXISTS capture_sources_adapter_check;
ALTER TABLE IF EXISTS migration_sync.capture_sources ADD CONSTRAINT capture_sources_adapter_check CHECK(adapter IN ('places_outbox_v1','places_hybrid_commands_v1'));
