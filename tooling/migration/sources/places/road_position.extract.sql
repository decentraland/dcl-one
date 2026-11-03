SELECT DISTINCT btrim(position::text) AS position
FROM road_positions
WHERE btrim(position::text) <> ''
