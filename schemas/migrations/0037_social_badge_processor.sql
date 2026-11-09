-- The badge processor reads social.client_events forward by an insertion
-- sequence (created_at is the inserting transaction's start, so a cursor on
-- it skips rows). Adding the identity column numbers the rows already kept
-- in physical order. A database without the table yet gains it, with the
-- column, from the object pass, which also creates client_events_seq_idx and
-- social.badge_cursors.
ALTER TABLE IF EXISTS social.client_events ADD COLUMN IF NOT EXISTS seq bigint GENERATED ALWAYS AS IDENTITY;
