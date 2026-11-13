-- Presence badges are catalog configuration, copied verbatim (upstream's
-- column names and types) for the node's presence consumer; the catalog
-- step imported the badges they name. As upstream's catalog reader, a
-- configuration is refused (skipped, reported) on a badge with an observer,
-- a tiered handler or an import of its own, and on one that is not a single
-- badge asking for its steps (the seconds of a stay, 1 for a visit). The
-- table comes with the badge catalog schema; on a schema without it a
-- non-empty batch stops instead of being dropped.
DO $presence$ DECLARE n bigint; r record; BEGIN
  IF to_regclass('social.presence_badges') IS NULL THEN
    IF EXISTS (SELECT 1 FROM stage) THEN
      RAISE EXCEPTION 'badges/presence_badge: social.presence_badges is not in this schema; apply the badge catalog schema first';
    END IF;
  ELSE
    CREATE TEMP TABLE presence_refused ON COMMIT DROP AS
    SELECT badge_id,
           CASE WHEN handled THEN 'it has an observer, a tiered handler or an import of its own'
                ELSE format('it is not a single badge asking for %s steps', steps) END AS reason
    FROM (SELECT s.badge_id, CASE WHEN s.mode = 'stay' THEN s.required_seconds ELSE 1 END AS steps,
                 -- Upstream's BADGE_EVALUATORS with an observer, tiers or an import of its own.
                 s.badge_id = ANY (ARRAY['dclcitizen', 'epic_ensemble', 'regally_rare', 'legendary_look',
                                         'mythic_model', 'unique_unicorn', 'exotic_elegance', 'open_for_business',
                                         'traveler', 'profile_pro', 'emotionista', 'fashionista',
                                         'event_enthusiast', 'moves_master', 'social_butterfly',
                                         'vertical_voyager', 'walkabout_wanderer', 'land_architect',
                                         'emote_creator', 'wearable_designer', 'referral']) AS handled,
                 d.is_tier, d.criteria_steps, d.id IS NOT NULL AS cataloged
          FROM stage s
          LEFT JOIN social.badge_definitions d ON d.id = s.badge_id) c
    WHERE handled OR (cataloged AND (is_tier OR criteria_steps IS DISTINCT FROM steps));
    FOR r IN SELECT badge_id, reason FROM presence_refused ORDER BY badge_id LOOP
      RAISE NOTICE 'badges/presence_badge: skipped the presence configuration of %: %', r.badge_id, r.reason;
    END LOOP;

    INSERT INTO social.presence_badges AS b
        (badge_id, location_kind, parcel, world_name, window_start, window_end, mode, required_seconds, created_at)
    SELECT s.badge_id, s.location_kind, s.parcel, s.world_name, s.window_start, s.window_end, s.mode,
           s.required_seconds, s.created_at
    FROM stage s
    WHERE NOT EXISTS (SELECT 1 FROM presence_refused x WHERE x.badge_id = s.badge_id)
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
    DROP TABLE presence_refused;
  END IF;
END $presence$;
