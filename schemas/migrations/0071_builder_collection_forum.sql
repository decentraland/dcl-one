ALTER TABLE builder.collections ADD COLUMN IF NOT EXISTS forum_link text, ADD COLUMN IF NOT EXISTS forum_id integer;
ALTER TABLE builder.collections ADD CONSTRAINT collections_forum_link_check CHECK (forum_link IS NULL OR (char_length(forum_link) <= 2048 AND forum_link ~ '^https?://[^[:space:]]+$'));
ALTER TABLE builder.collections ADD CONSTRAINT collections_forum_id_check CHECK (forum_id IS NULL OR (forum_id > 0 AND forum_link IS NOT NULL));
