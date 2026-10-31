-- Per-report upload authority is separate from the URL returned to moderators.
-- Existing capabilities are revoked; reporters retain signed upload access.
ALTER TABLE explore.place_reports ADD COLUMN IF NOT EXISTS upload_token_hash bytea;
ALTER TABLE explore.place_reports ADD CONSTRAINT place_reports_upload_token_hash_length CHECK (upload_token_hash IS NULL OR octet_length(upload_token_hash) = 32);
UPDATE explore.place_reports SET signed_url = split_part(signed_url, '?', 1) WHERE signed_url LIKE '%?%';
