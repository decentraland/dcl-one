-- Public owner-filtered world lists probe both catalogue legs through place_indexed.
CREATE INDEX IF NOT EXISTS place_world_owner_idx ON explore.place
    (lower(COALESCE(raw->>'owner', creator_address))) WHERE world IS TRUE;
CREATE INDEX IF NOT EXISTS place_world_local_owner_idx ON explore.place_world_local
    (lower(COALESCE(raw->>'owner', creator_address))) WHERE world IS TRUE;
