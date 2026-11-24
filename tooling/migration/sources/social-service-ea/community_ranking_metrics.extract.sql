-- Upstream's lifetime ranking counters (fed by its event-ended, photo-taken and
-- streaming-ended handlers) and its last score. A community with no metrics row
-- has zeros, as upstream's ranking job reads it. snapshot_at is when they were read.
SELECT c.id AS community_id,
       GREATEST(COALESCE(m.events_count, 0), 0) AS events_count,
       GREATEST(COALESCE(m.photos_count, 0), 0) AS photos_count,
       GREATEST(COALESCE(m.streams_count, 0), 0) AS streams_count,
       GREATEST(COALESCE(m.events_total_attendees, 0), 0) AS events_total_attendees,
       GREATEST(COALESCE(m.streams_total_participants, 0), 0) AS streams_total_participants,
       c.ranking_score,
       now() AS snapshot_at
FROM communities c
LEFT JOIN community_ranking_metrics m ON m.community_id = c.id
