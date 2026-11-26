-- The schema seeds the same upstream list; this only adds parcels it lacks.
INSERT INTO explore.road_positions (position)
SELECT position FROM stage
ON CONFLICT (position) DO NOTHING;
