-- The badge catalog takes upstream's shape (schemas/40_social.sql): a
-- single badge's target, publication, archive and creation order on the
-- definition, upstream's per-badge progress document on the progress row,
-- and upstream's constraints. A database stamped before this step keeps its
-- placeholder catalog rows (a step never inserts or deletes): the two ids
-- upstream shares with it (open_for_business, emotionista) take upstream's
-- content here, the two it does not (decentraland_citizen, walkabout) stay
-- unpublished, so no read lists them and nothing awards them. Upstream's
-- other badges and tiers (emotionista's starter, platinum and diamond among
-- them) arrive with the TS database import, which also retires the two
-- placeholders; deleting them otherwise is its own reviewed release.
ALTER TABLE social.badge_definitions ADD COLUMN IF NOT EXISTS criteria_steps integer, ADD COLUMN IF NOT EXISTS published_at timestamp without time zone, ADD COLUMN IF NOT EXISTS archived_at timestamp without time zone, ADD COLUMN IF NOT EXISTS created_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now());
ALTER TABLE social.user_badge_progress ADD COLUMN IF NOT EXISTS progress jsonb NOT NULL DEFAULT '{}'::jsonb;

UPDATE social.badge_definitions
SET name = 'Open for Business',
    description = 'Complete Store Information and submit at least 1 collection',
    category = 'Creator',
    is_tier = false,
    criteria_steps = 2,
    assets = jsonb_build_object(
        '2d', jsonb_build_object('normal', 'open_for_business/2d/normal.png'),
        '3d', jsonb_build_object('normal', 'open_for_business/3d/normal.png', 'hrm', 'open_for_business/3d/hrm.png',
                                 'basecolor', 'open_for_business/3d/basecolor.png'))
WHERE id = 'open_for_business';
UPDATE social.badge_definitions
SET name = 'Emotionista',
    description = 'Starter: 1 Emote purchased;Bronze: 10 Emotes purchased;Silver: 25 Emotes purchased;Gold: 50 Emotes purchased;Platinum: 150 Emotes purchased;Diamond: 300  Emotes purchased',
    category = 'Collector',
    is_tier = true,
    criteria_steps = NULL,
    assets = '{}'::jsonb
WHERE id = 'emotionista';
-- the placeholder's three tier ids are upstream's; its ordinals move to upstream's positions
UPDATE social.badge_tiers t
SET tier_name = u.tier_name,
    description = u.description,
    criteria_steps = u.steps,
    ordinal = u.ordinal,
    assets = jsonb_build_object(
        '2d', jsonb_build_object('normal', 'emotionista/' || u.slot || '/2d/normal.png'),
        '3d', jsonb_build_object('normal', 'emotionista/' || u.slot || '/3d/normal.png',
                                 'hrm', 'emotionista/' || u.slot || '/3d/hrm.png',
                                 'basecolor', 'emotionista/' || u.slot || '/3d/basecolor.png'))
FROM (VALUES
  ('emotionista-bronze', 'Bronze', '10 Emotes purchased', 'bronze', 10, 1),
  ('emotionista-silver', 'Silver', '25 Emotes purchased', 'silver', 25, 2),
  ('emotionista-gold', 'Gold', '50 Emotes purchased', 'gold', 50, 3)
) AS u (tier_id, tier_name, description, slot, steps, ordinal)
WHERE t.badge_id = 'emotionista' AND t.tier_id = u.tier_id;
-- every other single badge was a one-step goal
UPDATE social.badge_definitions SET criteria_steps = 1 WHERE NOT is_tier AND criteria_steps IS NULL;
-- upstream's creation order for its ids, wherever they already are
UPDATE social.badge_definitions d
SET created_at = timestamp '2026-10-07 00:00:00' + s.n * interval '1 millisecond'
FROM (VALUES
  ('dclcitizen', 0),
  ('epic_ensemble', 1),
  ('regally_rare', 2),
  ('legendary_look', 3),
  ('mythic_model', 4),
  ('unique_unicorn', 5),
  ('exotic_elegance', 6),
  ('open_for_business', 7),
  ('traveler', 8),
  ('profile_pro', 9),
  ('emotionista', 10),
  ('fashionista', 11),
  ('event_enthusiast', 12),
  ('moves_master', 13),
  ('social_butterfly', 14),
  ('vertical_voyager', 15),
  ('walkabout_wanderer', 16),
  ('land_architect', 17),
  ('emote_creator', 18),
  ('wearable_designer', 19),
  ('referral', 20),
  ('music_festival_2024', 21),
  ('fashion_week_2025', 22),
  ('builder_talent_trail_2025', 23),
  ('music_talent_trail_2025', 24),
  ('game_night_2025', 25),
  ('career_quest_2025', 26),
  ('art_week_2025', 27),
  ('fright_night_2025', 28),
  ('music_festival_2025', 29),
  ('festive_trail_2025', 30),
  ('welcome_guide', 31),
  ('buildathon_friendzone', 32),
  ('halloweeks_2026', 33)
) AS s (id, n)
WHERE d.id = s.id;
-- every row stays public but the two placeholders upstream names differently
UPDATE social.badge_definitions
SET published_at = timestamp '2026-10-07 00:00:00'
WHERE published_at IS NULL AND id NOT IN ('decentraland_citizen', 'walkabout');

ALTER TABLE social.badge_definitions ADD CONSTRAINT badge_definitions_category_check CHECK (category IN ('Explorer', 'Collector', 'Creator', 'Socializer', 'Builder'));
ALTER TABLE social.badge_definitions ADD CONSTRAINT badge_definitions_criteria_steps_check CHECK (criteria_steps > 0);
ALTER TABLE social.badge_definitions ADD CONSTRAINT badge_definitions_tier_steps_check CHECK (NOT is_tier OR criteria_steps IS NULL);
ALTER TABLE social.badge_tiers ALTER COLUMN criteria_steps DROP DEFAULT;
ALTER TABLE social.badge_tiers ADD CONSTRAINT badge_tiers_criteria_steps_check CHECK (criteria_steps > 0);
ALTER TABLE social.badge_tiers ADD CONSTRAINT badge_tiers_ordinal_check CHECK (ordinal >= 0);
ALTER TABLE social.badge_tiers ADD CONSTRAINT badge_tiers_badge_ordinal_key UNIQUE (badge_id, ordinal) DEFERRABLE INITIALLY DEFERRED;
ALTER TABLE social.user_badge_progress ADD CONSTRAINT user_badge_progress_progress_check CHECK (jsonb_typeof(progress) = 'object');
