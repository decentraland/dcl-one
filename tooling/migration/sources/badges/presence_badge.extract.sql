-- The presence rule of a catalog badge: where (a scene's base parcel or a
-- World), inside which window, and visit or a minimum stay.
SELECT b.badge_id,
       b.location_kind::text AS location_kind,
       b.parcel::text AS parcel,
       b.world_name::text AS world_name,
       b.window_start,
       b.window_end,
       b.mode::text AS mode,
       b.required_seconds,
       b.created_at
FROM presence_badges b
