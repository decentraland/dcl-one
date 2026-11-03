-- events `schedule` -> explore.schedules_local. Every column is timestamptz upstream.
SELECT s.id::text                                AS id,
       s.name::text                              AS name,
       s.description::text                       AS description,
       s.image::text                             AS image,
       s.theme::text                             AS theme,
       to_jsonb(COALESCE(s.background, '{}')::text[]) AS background,
       s.active_since,
       s.active_until,
       s.active,
       s.created_at,
       s.updated_at
FROM schedule s
