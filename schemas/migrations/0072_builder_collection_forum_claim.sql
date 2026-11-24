ALTER TABLE builder.collections ADD COLUMN IF NOT EXISTS forum_claimed_at timestamp without time zone;
ALTER TABLE builder.collections ADD CONSTRAINT collections_forum_claim_check CHECK (forum_claimed_at IS NULL OR forum_link IS NULL);
UPDATE builder.collections c
SET forum_link = to_jsonb(p)->>'forum_url', forum_claimed_at = NULL
FROM builder.linked_collection_publications p
WHERE p.collection_id = c.id AND c.forum_link IS NULL
  AND to_jsonb(p)->>'forum_url' IS NOT NULL;
