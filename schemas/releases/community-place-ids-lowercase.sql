BEGIN;
-- Community place ids are stored lowercased (explore keys catalogue ids and
-- world names that way), so case variants of one id in a community collapse
-- to one row: the lowercase spelling, else the earliest added, then the
-- smallest spelling. The others are removed and the survivor is lowercased.
-- Safe to run while updated binaries serve and to run again: run it after
-- every old replica and TS import run that may still write mixed case.
-- Rollback needs the matching backup.
LOCK TABLE social.community_places IN SHARE ROW EXCLUSIVE MODE;

DELETE FROM social.community_places t
 USING (SELECT place_id, community_id,
               row_number() OVER (PARTITION BY community_id, lower(place_id)
                                  ORDER BY place_id <> lower(place_id), added_at, place_id) AS rank
          FROM social.community_places) r
 WHERE r.rank > 1 AND t.place_id = r.place_id AND t.community_id = r.community_id;

UPDATE social.community_places SET place_id = lower(place_id) WHERE place_id <> lower(place_id);

COMMIT;
