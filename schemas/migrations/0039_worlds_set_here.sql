-- Whether the owner set a world's access or settings on this node, and which
-- owner did, so the worlds mirror never replaces them with its source's while
-- that owner holds the name (core::store::mirror_keeps_local). Constant
-- defaults add the columns without rewriting the table.
--
-- Before this step the mirror copied no access setting, and of the settings
-- only `skybox_time` and `single_player`, so a mirrored row's other values
-- were written by the owner here and are marked as such. Their setter is not
-- known; the mirror records the name's holder at its next pass as it. An
-- owner who explicitly set `unrestricted` here cannot be told from the
-- default and is not marked: the mirror then copies the source's access,
-- which can only restrict that world. The old mirror wrote `skybox_time` and
-- `single_player` itself, so an owner's edit of only those is not marked
-- either, and the source's values apply.
ALTER TABLE IF EXISTS worlds.worlds
    ADD COLUMN IF NOT EXISTS access_set_here boolean NOT NULL DEFAULT false,
    ADD COLUMN IF NOT EXISTS access_set_by text,
    ADD COLUMN IF NOT EXISTS settings_set_here boolean NOT NULL DEFAULT false,
    ADD COLUMN IF NOT EXISTS settings_set_by text;
UPDATE worlds.worlds SET access_set_here = true
 WHERE access_type <> 'unrestricted' AND NOT access_set_here;
UPDATE worlds.worlds SET settings_set_here = true
 WHERE mirror_source IS NOT NULL AND NOT settings_set_here
   AND (title IS NOT NULL OR description IS NOT NULL OR content_rating IS NOT NULL
        OR categories IS NOT NULL OR thumbnail_hash IS NOT NULL OR NOT show_in_places);
