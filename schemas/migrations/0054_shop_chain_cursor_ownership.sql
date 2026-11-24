-- Ownership, chain binding and quarantine of the market chain index cursors
-- (services/catalyst/shop/src/market/chain_walk.rs).
--
-- owner_generation: the job that takes a projection raises it on the session
-- holding its lease. Every transaction that writes the projection checks it
-- before commit, so a writer that lost the projection commits nothing.
--
-- chain_id: the chain the cursor's history was read from, as the source
-- proved it. Existing rows keep NULL until a pass proves their last block's
-- hash on the expected chain. Until then they are not ready.
--
-- quarantined_at, quarantine_reason: a cursor whose history could not be
-- proven on the expected chain. It is not walked, rewound or published until
-- an operator revalidates or rebuilds it.
--
-- rebuild_requested_at: set by an operator to have the projection's owner
-- rebuild it from its start block. The owner runs the projection's own
-- cleanup in a fenced transaction.
ALTER TABLE shop.chain_cursor ADD COLUMN IF NOT EXISTS chain_id bigint;
ALTER TABLE shop.chain_cursor ADD COLUMN IF NOT EXISTS owner_generation bigint NOT NULL DEFAULT 0;
ALTER TABLE shop.chain_cursor ADD COLUMN IF NOT EXISTS quarantined_at timestamp without time zone;
ALTER TABLE shop.chain_cursor ADD COLUMN IF NOT EXISTS quarantine_reason text;
ALTER TABLE shop.chain_cursor ADD COLUMN IF NOT EXISTS rebuild_requested_at timestamp without time zone;
ALTER TABLE shop.chain_cursor DROP CONSTRAINT IF EXISTS chain_cursor_quarantine_reason;
ALTER TABLE shop.chain_cursor ADD CONSTRAINT chain_cursor_quarantine_reason CHECK ((quarantined_at IS NULL) = (quarantine_reason IS NULL));
