-- One provenance column on the mirrored catalogues (explore.poster already has
-- it): 'upstream' rows are a mirror's, which refuses edits to their own fields
-- and sweeps them; 'local' rows are this node's authority. Declared on the
-- tables in schema/10_explore.sql and schema/40_social.sql; a database stamped
-- before them takes them here, backfilled from the markers they replace (left
-- in place: a binary from before this step may still write them, and the
-- places and events mirrors reclassify by the same rules on every pass):
--   * a place derived from this node's own scenes carried raw.local;
--   * an event created here got its events_local overlay in the creating
--     transaction: a {"create": ...} payload, or a later edit or moderation
--     over it that kept that transaction's signed_at. Every other overlay
--     edited a mirrored event: that event stays 'upstream', so the next pass
--     writes the upstream's fields back over it (released, not frozen). An
--     operator's moderation or delete among them is kept: the object pass
--     that follows the steps (a step creates nothing) creates
--     explore.event_moderation and its seed carries them into it;
--   * a mirrored community holds no member row (a join on it was refused and
--     the pass writes none), while every local one keeps its owner's row;
--     social.community_mirror, its provenance row, may postdate the database
--     (the object pass creates it after the steps).
ALTER TABLE explore.place ADD COLUMN IF NOT EXISTS origin text NOT NULL DEFAULT 'upstream';
ALTER TABLE explore.place ADD CONSTRAINT place_origin_known CHECK (origin IN ('upstream', 'local'));
UPDATE explore.place SET origin = 'local' WHERE raw->>'local' = 'true' AND origin <> 'local';
ALTER TABLE explore.event ADD COLUMN IF NOT EXISTS origin text NOT NULL DEFAULT 'upstream';
ALTER TABLE explore.event ADD CONSTRAINT event_origin_known CHECK (origin IN ('upstream', 'local'));
UPDATE explore.event e SET origin = 'local' FROM explore.events_local l WHERE l.id = e.id AND e.origin <> 'local' AND (l.signed_payload ? 'create' OR l.signed_at = e.fetched_at);
ALTER TABLE social.communities ADD COLUMN IF NOT EXISTS origin text NOT NULL DEFAULT 'local';
ALTER TABLE social.communities ADD CONSTRAINT communities_origin_known CHECK (origin IN ('upstream', 'local'));
UPDATE social.communities c SET origin = 'upstream' WHERE c.origin <> 'upstream' AND NOT EXISTS (SELECT 1 FROM social.community_members m WHERE m.community_id = c.id);
