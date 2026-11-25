-- The chain that supplied the mirror pass and its saved page cursor.
-- Existing passes acquire their binding after a fresh pass from the first page.
ALTER TABLE shop.mirror_state ADD COLUMN IF NOT EXISTS chain_id bigint;
