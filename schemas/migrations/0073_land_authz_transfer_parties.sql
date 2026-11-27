-- A LAND or Estate transfer keeps its sender and transaction for the activity's
-- transfer leg. Rows indexed before stay NULL until the indexer's transfer
-- repair reads their logs again; the object pass creates the indexes.
ALTER TABLE land_authz.authz_event ADD COLUMN IF NOT EXISTS from_address varchar(42);
ALTER TABLE land_authz.authz_event ADD COLUMN IF NOT EXISTS tx_hash text;
