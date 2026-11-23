-- Upstream's lifetime ranking counters and score onto the imported (local)
-- communities. dcl_migration.social_community_ranking_loaded holds upstream's
-- counters as last loaded: while they are unchanged the node's are left alone
-- (upstream rescores daily as communities age, so its score alone is no change).
-- On a change, events and attendees take upstream's values and
-- ranking_tracking_since moves to the snapshot, so the node's ended-event scan
-- never counts an imported event upstream already counted; photos and streams
-- keep the node's own count: what it holds above upstream's last loaded value.
-- ranking_score is upstream's, written with the counters, until the node's next pass.
CREATE TEMP TABLE ranking_changed ON COMMIT DROP AS
SELECT s.community_id, s.events_count, s.photos_count, s.streams_count,
       s.events_total_attendees, s.streams_total_participants, s.ranking_score,
       s.snapshot_at AT TIME ZONE 'UTC' AS snapshot_at,
       l.photos_count AS loaded_photos_count, l.streams_count AS loaded_streams_count,
       l.streams_total_participants AS loaded_streams_total_participants
FROM stage s
JOIN social.communities c ON c.id = s.community_id AND c.origin = 'local'
LEFT JOIN dcl_migration.social_community_ranking_loaded l ON l.community_id = s.community_id
WHERE l.community_id IS NULL
   OR (l.events_count, l.photos_count, l.streams_count, l.events_total_attendees,
       l.streams_total_participants)
      IS DISTINCT FROM (s.events_count, s.photos_count, s.streams_count, s.events_total_attendees,
                        s.streams_total_participants);

WITH written AS (
    UPDATE social.communities c SET
        ranking_events_count = r.events_count,
        ranking_events_total_attendees = r.events_total_attendees,
        ranking_photos_count = LEAST(r.photos_count::bigint
            + GREATEST(c.ranking_photos_count - COALESCE(r.loaded_photos_count, 0), 0), 2147483647)::int4,
        ranking_streams_count = LEAST(r.streams_count::bigint
            + GREATEST(c.ranking_streams_count - COALESCE(r.loaded_streams_count, 0), 0), 2147483647)::int4,
        ranking_streams_total_participants = LEAST(r.streams_total_participants::bigint
            + GREATEST(c.ranking_streams_total_participants - COALESCE(r.loaded_streams_total_participants, 0), 0),
            2147483647)::int4,
        ranking_score = r.ranking_score,
        ranking_tracking_since = GREATEST(c.ranking_tracking_since, r.snapshot_at)
    FROM ranking_changed r
    WHERE c.id = r.community_id
    RETURNING c.id
)
INSERT INTO dcl_migration.social_community_ranking_loaded AS l
    (community_id, events_count, photos_count, streams_count, events_total_attendees,
     streams_total_participants, ranking_score)
SELECT r.community_id, r.events_count, r.photos_count, r.streams_count, r.events_total_attendees,
       r.streams_total_participants, r.ranking_score
FROM ranking_changed r
JOIN written w ON w.id = r.community_id
ON CONFLICT (community_id) DO UPDATE SET
    events_count = EXCLUDED.events_count,
    photos_count = EXCLUDED.photos_count,
    streams_count = EXCLUDED.streams_count,
    events_total_attendees = EXCLUDED.events_total_attendees,
    streams_total_participants = EXCLUDED.streams_total_participants,
    ranking_score = EXCLUDED.ranking_score;
