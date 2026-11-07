-- One row per lowercase world name: mixed-case duplicates collapse onto the
-- already-lowercase row, else the newest. The access JSONB becomes the three
-- typed columns plus the allow-list members; a malformed restricted setting
-- (no secret, no nft, unknown type) becomes an empty allow-list, fail closed.
SELECT
    lower(w.name) AS name,
    CASE WHEN lower(w.owner) ~ '^0x[0-9a-f]{40}$' THEN lower(w.owner) END AS owner,
    CASE
        WHEN w.access IS NULL OR w.access->>'type' = 'unrestricted' THEN 'unrestricted'
        WHEN w.access->>'type' = 'shared-secret' AND coalesce(w.access->>'secret', '') <> '' THEN 'shared-secret'
        WHEN w.access->>'type' = 'nft-ownership' AND coalesce(w.access->>'nft', '') <> '' THEN 'nft-ownership'
        ELSE 'allow-list'
    END AS access_type,
    CASE WHEN w.access->>'type' = 'shared-secret' AND coalesce(w.access->>'secret', '') <> ''
         THEN w.access->>'secret' END AS access_secret_hash,
    CASE WHEN w.access->>'type' = 'nft-ownership' AND coalesce(w.access->>'nft', '') <> ''
         THEN w.access->>'nft' END AS access_nft,
    ARRAY(
        SELECT DISTINCT lower(a)
        FROM jsonb_array_elements_text(CASE WHEN jsonb_typeof(w.access->'wallets') = 'array'
                                            THEN w.access->'wallets' ELSE '[]'::jsonb END) a
        WHERE lower(a) ~ '^0x[0-9a-f]{40}$'
    ) AS access_wallets,
    ARRAY(
        SELECT DISTINCT lower(c)
        FROM jsonb_array_elements_text(CASE WHEN jsonb_typeof(w.access->'communities') = 'array'
                                            THEN w.access->'communities' ELSE '[]'::jsonb END) c
        WHERE c ~* '^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$'
    ) AS access_communities,
    w.spawn_coordinates::text AS spawn_coordinates,
    w.title::text AS title,
    w.description,
    w.content_rating::text AS content_rating,
    w.skybox_time,
    w.categories,
    w.single_player,
    w.show_in_places,
    w.thumbnail_hash::text AS thumbnail_hash,
    w.settings_version,
    w.created_at AT TIME ZONE 'UTC' AS created_at,
    w.updated_at AT TIME ZONE 'UTC' AS updated_at
FROM (
    SELECT worlds.*,
           row_number() OVER (PARTITION BY lower(name)
                              ORDER BY (name = lower(name)) DESC, updated_at DESC, name) AS rn
    FROM worlds
    WHERE btrim(name) <> ''
) w
WHERE w.rn = 1
