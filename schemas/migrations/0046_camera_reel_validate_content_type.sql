-- Validate the upload MIME constraint separately from the column addition so
-- adding it does not hold an exclusive table lock while scanning existing rows.
ALTER TABLE IF EXISTS camera_reel.images
    VALIDATE CONSTRAINT images_image_content_type_check;
