-- Presence badges are catalog configuration, copied verbatim (upstream's
-- column names and types) for the node's presence consumer; the catalog
-- step imported the badges they name. The table comes with the badge
-- catalog schema; on a schema without it a non-empty batch stops instead of
-- being dropped.
DO $presence$ DECLARE n bigint; BEGIN
  IF to_regclass('social.presence_badges') IS NULL THEN
    IF EXISTS (SELECT 1 FROM stage) THEN
      RAISE EXCEPTION 'badges/presence_badge: social.presence_badges is not in this schema; apply the badge catalog schema first';
    END IF;
  ELSE
    INSERT INTO social.presence_badges AS b
        (badge_id, location_kind, parcel, world_name, window_start, window_end, mode, required_seconds, created_at)
    SELECT s.badge_id, s.location_kind, s.parcel, s.world_name, s.window_start, s.window_end, s.mode,
           s.required_seconds, s.created_at
    FROM stage s
    ON CONFLICT (badge_id) DO UPDATE SET
        location_kind = EXCLUDED.location_kind,
        parcel = EXCLUDED.parcel,
        world_name = EXCLUDED.world_name,
        window_start = EXCLUDED.window_start,
        window_end = EXCLUDED.window_end,
        mode = EXCLUDED.mode,
        required_seconds = EXCLUDED.required_seconds,
        created_at = EXCLUDED.created_at
    WHERE (b.location_kind, b.parcel, b.world_name, b.window_start, b.window_end, b.mode, b.required_seconds,
           b.created_at)
          IS DISTINCT FROM (EXCLUDED.location_kind, EXCLUDED.parcel, EXCLUDED.world_name, EXCLUDED.window_start,
                            EXCLUDED.window_end, EXCLUDED.mode, EXCLUDED.required_seconds, EXCLUDED.created_at);
    GET DIAGNOSTICS n = ROW_COUNT;
    RAISE NOTICE 'badges/presence_badge: upserted % presence badges', n;
  END IF;
END $presence$;
