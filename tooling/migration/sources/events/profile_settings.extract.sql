-- events `profile_settings` -> explore.event_profile_settings. created_at and
-- updated_at are `timestamp without time zone` upstream (decentraland-gatsby
-- Type.TimeStampTZ) holding UTC. Email lifecycle timestamps are timestamptz.
-- Of rows equal under lower("user"), the latest save is kept.
SELECT lower(p."user")                        AS "user",
       p.email,
       p.email_verified,
       p.email_verified_at,
       p.email_updated_at,
       p.use_local_time,
       p.notify_by_email,
       p.notify_by_browser,
       to_jsonb(COALESCE(p.permissions, '{}')::text[]) AS permissions,
       p.created_at AT TIME ZONE 'UTC'        AS created_at,
       p.updated_at AT TIME ZONE 'UTC'        AS updated_at
FROM profile_settings p
WHERE NOT EXISTS (
    SELECT 1 FROM profile_settings q
    WHERE lower(q."user") = lower(p."user")
      AND (q.updated_at, q."user") > (p.updated_at, p."user"))
