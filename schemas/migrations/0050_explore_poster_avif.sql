-- Admit the detected type of mirrored AVIF posters and retain legacy generic rows.
ALTER TABLE IF EXISTS explore.poster DROP CONSTRAINT IF EXISTS poster_type_known;
ALTER TABLE IF EXISTS explore.poster ADD CONSTRAINT poster_type_known CHECK (content_type IN ('image/png', 'image/jpeg', 'image/gif', 'image/webp', 'image/avif', 'application/octet-stream'));
