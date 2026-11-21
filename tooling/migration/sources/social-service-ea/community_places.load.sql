INSERT INTO social.community_places AS t (place_id, community_id, added_by, added_at)
SELECT s.place_id, s.community_id, s.added_by, s.added_at AT TIME ZONE 'UTC'
FROM stage s
JOIN social.communities c ON c.id = s.community_id AND c.origin = 'local'
ON CONFLICT (place_id, community_id) DO UPDATE SET
    added_by = EXCLUDED.added_by,
    added_at = EXCLUDED.added_at;

-- A Genesis id resolves through explore.place, keyed by upstream's places UUID
-- only once `run places` imported it (the derived catalogue keys by UUIDv5).
DO $catalogue$ DECLARE n bigint; BEGIN
  SELECT count(*) INTO n FROM stage s
   WHERE s.place_id ~* '^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$'
     AND EXISTS (SELECT 1 FROM social.communities c WHERE c.id = s.community_id AND c.origin = 'local')
     AND NOT EXISTS (SELECT 1 FROM explore.place p WHERE p.id = lower(s.place_id));
  IF n > 0 THEN RAISE NOTICE
    'social-service-ea/community_places: % place ids of this batch are not in explore.place and answer bare rows; run `migrate.py run places` (rows resolve once it has)', n;
  ELSE NULL; END IF; END $catalogue$;
