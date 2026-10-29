-- The new history function and partial indexes are created by the schema object pass.
ALTER TABLE land_authz.index_cursor ADD COLUMN IF NOT EXISTS history_timestamps_ready boolean NOT NULL DEFAULT false;
