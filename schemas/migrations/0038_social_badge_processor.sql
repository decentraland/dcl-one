-- The badge processor reads social.client_events forward by an insertion
-- sequence (created_at is the inserting transaction's start, so a cursor on
-- it skips rows). Both statements change the catalog only: a nullable column
-- without a default and a default for later inserts rewrite no row. Their
-- ACCESS EXCLUSIVE lock lasts until the migrate transaction commits, through
-- the object pass and its index builds (DATABASE.md). Kept rows stay NULL: they
-- precede the processor's first cursor, which starts at the head. The
-- default names the sequence as text (resolved per insert) because the
-- object pass creates social.client_events_seq after the steps, in the same
-- transaction, with client_events_seq_idx, client_events_session_moves_idx
-- and social.badge_cursors.
ALTER TABLE IF EXISTS social.client_events ADD COLUMN IF NOT EXISTS seq bigint;
ALTER TABLE IF EXISTS social.client_events ALTER COLUMN seq SET DEFAULT nextval('social.client_events_seq'::text);
