-- credits.authorizations keeps what an operation asked for (request_digest): a request
-- that names its purchase (operationId) is keyed by the signer and that id, so a retry
-- signed again replays the same credit, and the same id with other lines is refused
-- (services/catalyst/shop/src/credits/handlers/authorize.rs). schemas/60_ops.sql declares
-- the column; a database that created the table before it gains it here (IF EXISTS: a
-- database stamped before the table gains it from the missing-objects pass).
ALTER TABLE IF EXISTS credits.authorizations ADD COLUMN IF NOT EXISTS request_digest text;
