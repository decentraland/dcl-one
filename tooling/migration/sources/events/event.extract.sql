-- events `events` -> explore.event (origin 'local') as moderation.rs create_as
-- shapes a local event: `raw` is the API document (EventModel.build/toPublic
-- field names), timestamps as RFC 3339 UTC strings. Upstream timestamps are
-- timestamptz except deleted_at (timestamp holding UTC). The attendee
-- preview is reset: the RSVPs move to explore.event_attendance_local
-- (the attendance step), which the read path counts on top of the preview.
WITH e AS (
    SELECT ev.*,
           COALESCE(ev.next_start_at, ev.start_at) AS eff_next_start,
           COALESCE(ev.next_finish_at,
                    COALESCE(ev.next_start_at, ev.start_at) + ev.duration * interval '1 millisecond') AS eff_next_finish,
           COALESCE(ev.x, 0) AS px,
           COALESCE(ev.y, 0) AS py
    FROM events ev
), f AS (
    SELECT e.*,
           to_char(e.start_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS start_s,
           to_char(e.finish_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS finish_s,
           to_char(e.eff_next_start AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS next_start_s,
           to_char(e.eff_next_finish AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS next_finish_s,
           to_char(e.recurrent_until AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS until_s,
           to_char(e.created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS created_s,
           to_char(e.updated_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS updated_s,
           to_char(e.deleted_at, 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') AS deleted_s,
           (SELECT COALESCE(jsonb_agg(to_char(d AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') ORDER BY o), '[]'::jsonb)
              FROM unnest(e.recurrent_dates) WITH ORDINALITY AS r(d, o)) AS dates_j,
           -- Uploads on the upstream poster bucket become this node's
           -- /poster[-vertical]/<file> path (posters.rs normalize_image_input),
           -- served by the poster read-through; other URLs are kept.
           CASE WHEN e.image ~ '^https?://events-assets-[0-9a-f]+\.decentraland\.(org|zone|today)/(poster|poster-vertical)/[A-Za-z0-9][A-Za-z0-9.-]*$'
                THEN regexp_replace(e.image, '^https?://[^/]+', '') ELSE e.image END AS image_n,
           CASE WHEN e.image_vertical ~ '^https?://events-assets-[0-9a-f]+\.decentraland\.(org|zone|today)/(poster|poster-vertical)/[A-Za-z0-9][A-Za-z0-9.-]*$'
                THEN regexp_replace(e.image_vertical, '^https?://[^/]+', '') ELSE e.image_vertical END AS image_vertical_n
    FROM e
)
SELECT f.id::text                         AS id,
       f.name                             AS name,
       f.start_at,
       f.finish_at,
       f.eff_next_start                   AS next_start_at,
       f.eff_next_finish                  AS next_finish_at,
       f.duration                         AS duration_ms,
       f.recurrent,
       COALESCE(f.highlighted, false)     AS highlighted,
       f.trending,
       f.approved,
       f.community_id,
       f."user"                           AS user_creator,
       f.px                               AS coordinates_x,
       f.py                               AS coordinates_y,
       COALESCE(f.description, '')        AS description,
       jsonb_build_object(
           'id', f.id::text, 'name', f.name, 'image', f.image_n, 'image_vertical', f.image_vertical_n,
           'description', f.description, 'start_at', f.start_s, 'finish_at', f.finish_s,
           'next_start_at', f.next_start_s, 'next_finish_at', f.next_finish_s, 'duration', f.duration,
           'all_day', COALESCE(f.all_day, false), 'x', f.px, 'y', f.py, 'server', f.server, 'url', f.url,
           'user', f."user", 'user_name', f.user_name, 'estate_id', f.estate_id,
           'estate_name', COALESCE(f.estate_name, f.scene_name), 'scene_name', COALESCE(f.estate_name, f.scene_name),
           'approved', f.approved, 'rejected', f.rejected, 'highlighted', COALESCE(f.highlighted, false),
           'trending', f.trending, 'world', f.world, 'recurrent', f.recurrent,
           'recurrent_frequency', f.recurrent_frequency, 'recurrent_setpos', f.recurrent_setpos,
           'recurrent_monthday', f.recurrent_monthday, 'recurrent_weekday_mask', f.recurrent_weekday_mask,
           'recurrent_month_mask', f.recurrent_month_mask, 'recurrent_interval', f.recurrent_interval,
           'recurrent_count', f.recurrent_count, 'recurrent_until', f.until_s
       ) || jsonb_build_object(
           'recurrent_dates', f.dates_j,
           'categories', to_jsonb(COALESCE(f.categories, '{}')::text[]),
           'schedules', to_jsonb(COALESCE(f.schedules, '{}')::text[]),
           'contact', f.contact, 'details', f.details,
           'total_attendees', 0, 'latest_attendees', '[]'::jsonb,
           'coordinates', jsonb_build_array(f.px, f.py), 'position', jsonb_build_array(f.px, f.py),
           'live', false, 'attending', false,
           'place_id', f.place_id, 'community_id', f.community_id, 'featured_item', NULLIF(f.featured_item, ''),
           'created_at', f.created_s, 'updated_at', f.updated_s,
           'approved_by', f.approved_by::text, 'rejected_by', f.rejected_by::text,
           'rejection_reason', f.rejection_reason,
           'deleted_by_user', f.deleted_by_user, 'deleted_by_admin', f.deleted_by_admin,
           'deleted_by', f.deleted_by::text, 'deleted_at', f.deleted_s, 'deleted_reason', f.deleted_reason,
           'previous_place_id', f.previous_place_id
       )                                  AS raw,
       f.updated_at
FROM f
