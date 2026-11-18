-- Imported communities are local authority (origin 'local'): this database
-- is the upstream the communities mirror copies from, so a community the
-- mirror already holds is taken over (origin flips to 'local') and loses its
-- mirror provenance row, which would otherwise add upstream's member count
-- on top of the members imported next. The node's own axes (suspension,
-- ranking_score) are kept; a local edit newer than upstream is kept.
INSERT INTO social.communities AS t
    (id, name, description, owner_address, private, active, unlisted, editors_choice,
     created_at, updated_at, origin)
SELECT s.id, s.name, s.description, s.owner_address, s.private, s.active, s.unlisted, s.editors_choice,
       s.created_at AT TIME ZONE 'UTC', s.updated_at AT TIME ZONE 'UTC', 'local'
FROM stage s
ON CONFLICT (id) DO UPDATE SET
    name = EXCLUDED.name,
    description = EXCLUDED.description,
    owner_address = EXCLUDED.owner_address,
    private = EXCLUDED.private,
    active = EXCLUDED.active,
    unlisted = EXCLUDED.unlisted,
    editors_choice = EXCLUDED.editors_choice,
    created_at = EXCLUDED.created_at,
    updated_at = EXCLUDED.updated_at,
    origin = 'local'
WHERE t.origin = 'upstream' OR t.updated_at <= EXCLUDED.updated_at;

DELETE FROM social.community_mirror m
USING stage s
WHERE m.community_id = s.id;
