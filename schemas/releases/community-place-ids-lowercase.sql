BEGIN;
-- Community place ids are stored lowercased (explore keys catalogue ids and
-- world names that way), so case variants of one id in a community collapse
-- to one row: the lowercase spelling, else the earliest added, then the
-- smallest spelling. The others are removed and the survivor is lowercased.
-- Then community_places_place_id_lowercase (schemas/40_social.sql) is added,
-- when missing, and validated, so no later writer can store another case.
-- Run after every old replica is replaced: an old replica's mixed-case write
-- fails the check afterwards. Safe to run again. Adding the check holds an
-- exclusive lock on the table for the run; a rerun only blocks writers.
-- Rollback needs the matching backup.
SET LOCAL lock_timeout = '10s';
DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_catalog.pg_constraint
                    WHERE conname = 'community_places_place_id_lowercase'
                      AND conrelid = 'social.community_places'::regclass) THEN
        ALTER TABLE social.community_places
            ADD CONSTRAINT community_places_place_id_lowercase CHECK (place_id = lower(place_id)) NOT VALID;
    END IF;
END
$$;
LOCK TABLE social.community_places IN SHARE ROW EXCLUSIVE MODE;

DELETE FROM social.community_places t
 USING (SELECT place_id, community_id,
               row_number() OVER (PARTITION BY community_id, lower(place_id)
                                  ORDER BY place_id <> lower(place_id), added_at, place_id) AS rank
          FROM social.community_places) r
 WHERE r.rank > 1 AND t.place_id = r.place_id AND t.community_id = r.community_id;

UPDATE social.community_places SET place_id = lower(place_id) WHERE place_id <> lower(place_id);

ALTER TABLE social.community_places VALIDATE CONSTRAINT community_places_place_id_lowercase;

COMMIT;
