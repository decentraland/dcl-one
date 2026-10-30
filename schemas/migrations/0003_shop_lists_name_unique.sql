-- shop.lists gained upstream's name_user_address_unique (marketplace-server
-- favorites migration 1677778846950): two lists of one wallet never share a
-- name, which POST/PUT /v1/lists answer as 422 "There is already a list with
-- the same name" (src/shop/market/lists.rs, 2026-09-26). Declared on the
-- table in schema/30_shop.sql; a database stamped before it takes it here.
ALTER TABLE shop.lists ADD CONSTRAINT lists_name_user_unique UNIQUE (name, user_address);
