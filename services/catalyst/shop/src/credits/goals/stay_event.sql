WITH publication AS (
    SELECT pass_started_at, last_complete_pass_at
      FROM explore.mirror_state
     WHERE mirror = 'events' AND source_url = $5::text
       AND next_offset IS NULL AND last_pass_complete IS TRUE
       AND pass_started_at >= $4 AND last_complete_pass_at >= pass_started_at
),
published_events AS NOT MATERIALIZED (
    SELECT e.* FROM explore.event e
     WHERE e.approved IS TRUE AND e.rejected IS FALSE AND e.deleted IS FALSE
       AND (e.origin = 'local' OR EXISTS (
           SELECT 1 FROM publication p
            WHERE e.fetched_at BETWEEN p.pass_started_at AND p.last_complete_pass_at
       ))
),
scene AS (
    SELECT d.entity_pointers,
           d.entity_timestamp <= $4 AND (d.deleter_deployment IS NULL OR replaced.entity_timestamp > $4) AS active
      FROM content.deployments d
      LEFT JOIN content.deployments replaced ON replaced.id = d.deleter_deployment
     WHERE NOT $1 AND d.entity_id = $3 AND d.entity_type = 'scene'
       AND (d.deleter_deployment IS NULL OR replaced.id IS NOT NULL)
),
venue_events AS (
    SELECT e.id, e.start_at, e.finish_at, e.next_start_at, e.next_finish_at,
           e.recurrent, e.duration_ms, e.recurrence_dates
      FROM published_events e
     WHERE $1 AND e.world IS TRUE AND e.server_lower = lower($2)
    UNION ALL
    SELECT e.id, e.start_at, e.finish_at, e.next_start_at, e.next_finish_at,
           e.recurrent, e.duration_ms, e.recurrence_dates
      FROM scene s CROSS JOIN LATERAL unnest(s.entity_pointers) AS pointer
      CROSS JOIN LATERAL regexp_match(pointer, '^(-?[0-9]{1,9}),(-?[0-9]{1,9})$') AS parcel
      JOIN published_events e ON e.coordinates_x = parcel[1]::integer
                          AND e.coordinates_y = parcel[2]::integer
     WHERE s.active AND e.world IS FALSE
),
matched AS (
    SELECT e.id, o.starts_at, o.finishes_at
      FROM venue_events e
     CROSS JOIN LATERAL (
        SELECT COALESCE(e.next_start_at, e.start_at) AS starts_at,
               CASE WHEN e.recurrent THEN e.next_finish_at
                    ELSE COALESCE(e.next_finish_at, e.finish_at) END AS finishes_at
        UNION
        SELECT started, started + e.duration_ms * interval '1 millisecond'
          FROM unnest(array_prepend(e.start_at, e.recurrence_dates)) AS occurrence(started)
         WHERE e.recurrent AND e.duration_ms > 0
     ) o
     WHERE $4 >= o.starts_at AND $4 < o.finishes_at
     ORDER BY e.start_at, e.id, o.starts_at DESC LIMIT 1
)
SELECT $1 OR EXISTS(SELECT 1 FROM scene) AS scene_known,
       $5::text IS NULL OR EXISTS (SELECT 1 FROM publication) OR EXISTS (
           SELECT 1 FROM explore.mirror_state WHERE mirror='events' AND adopted_at IS NOT NULL
       ) AS mirror_covers_join,
       m.id, m.starts_at, m.finishes_at
  FROM (VALUES (1)) AS anchor(n)
  LEFT JOIN matched m ON true
