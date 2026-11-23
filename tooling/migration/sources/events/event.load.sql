-- Imported events are this node's own ('local'): the events mirror never
-- overwrites or sweeps them. A row the mirror wrote first ('upstream') is
-- taken over, and an operator's moderation of it (explore.event_moderation)
-- is laid over the upstream flags exactly as mirror.rs UPSERT does, so a
-- decision made here survives the takeover. A local event this node already
-- edited (it has an explore.events_local audit) is left alone.
INSERT INTO explore.event AS t
    (id, name, start_at, finish_at, next_start_at, next_finish_at, duration_ms,
     recurrent, highlighted, trending, approved, attending, community_id,
     user_creator, coordinates_x, coordinates_y, description, raw, fetched_at, origin)
SELECT s.id, s.name,
       s.start_at AT TIME ZONE 'UTC', s.finish_at AT TIME ZONE 'UTC',
       s.next_start_at AT TIME ZONE 'UTC', s.next_finish_at AT TIME ZONE 'UTC',
       s.duration_ms, s.recurrent,
       COALESCE(m.highlighted, s.highlighted), COALESCE(m.trending, s.trending),
       COALESCE(m.approved, s.approved), false, s.community_id, s.user_creator,
       s.coordinates_x, s.coordinates_y, s.description,
       s.raw || jsonb_strip_nulls(jsonb_build_object(
           'approved', m.approved, 'rejected', m.rejected,
           'highlighted', m.highlighted, 'trending', m.trending,
           'deleted_by_admin', CASE WHEN m.hidden THEN true END,
           'deleted_at', CASE WHEN m.hidden
               THEN to_char(m.set_at, 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') END))
         || CASE WHEN m.approved IS NOT NULL
                THEN jsonb_build_object('approved_by', m.approved_by) ELSE '{}' END
         || CASE WHEN m.rejected IS NOT NULL
                THEN jsonb_build_object('rejected_by', m.rejected_by,
                                        'rejection_reason', m.rejection_reason) ELSE '{}' END
         || CASE WHEN m.hidden
                THEN jsonb_build_object('deleted_by', m.deleted_by,
                                        'deleted_reason', m.deleted_reason) ELSE '{}' END,
       s.updated_at AT TIME ZONE 'UTC', 'local'
FROM stage s
LEFT JOIN explore.event_moderation m ON m.event_id = s.id
ON CONFLICT (id) DO UPDATE SET
    name           = EXCLUDED.name,
    start_at       = EXCLUDED.start_at,
    finish_at      = EXCLUDED.finish_at,
    next_start_at  = EXCLUDED.next_start_at,
    next_finish_at = EXCLUDED.next_finish_at,
    duration_ms    = EXCLUDED.duration_ms,
    recurrent      = EXCLUDED.recurrent,
    highlighted    = EXCLUDED.highlighted,
    trending       = EXCLUDED.trending,
    approved       = EXCLUDED.approved,
    attending      = EXCLUDED.attending,
    community_id   = EXCLUDED.community_id,
    user_creator   = EXCLUDED.user_creator,
    coordinates_x  = EXCLUDED.coordinates_x,
    coordinates_y  = EXCLUDED.coordinates_y,
    description    = EXCLUDED.description,
    raw            = EXCLUDED.raw,
    fetched_at     = EXCLUDED.fetched_at,
    origin         = 'local'
WHERE t.origin = 'upstream'
   OR NOT EXISTS (SELECT 1 FROM explore.events_local l WHERE l.id = t.id);
