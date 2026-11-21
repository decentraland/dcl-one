ALTER TABLE builder.item_contents ADD COLUMN IF NOT EXISTS position integer NOT NULL DEFAULT 0;
ALTER TABLE builder.item_contents DROP CONSTRAINT IF EXISTS item_contents_hash_fkey;
