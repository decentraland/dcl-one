-- Preserve the accepted upload MIME for both UUID filename aliases. A download
-- reads it with the existing photo primary-key lookup; no index or backfill.
-- Old rows remain NULL because their original object metadata is not known.
ALTER TABLE IF EXISTS camera_reel.images
    ADD COLUMN IF NOT EXISTS image_content_type text
        CONSTRAINT images_image_content_type_check
        CHECK (image_content_type IN ('image/png', 'image/jpeg')) NOT VALID;
