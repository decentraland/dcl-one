ALTER TABLE builder.items ADD COLUMN IF NOT EXISTS video_named_by text;
UPDATE builder.items i
SET video_named_by = (
        SELECT f.named_by
        FROM builder.item_contents f
        WHERE f.item_id = i.id AND f.hash = i.video
        ORDER BY EXISTS (SELECT 1 FROM builder.uploaded_contents u WHERE u.hash = f.hash AND u.uploaded_by = f.named_by) DESC,
                 f.named_by
        LIMIT 1)
WHERE i.video IS NOT NULL AND i.video_named_by IS NULL;
ALTER TABLE builder.items ADD CONSTRAINT items_video_named_by_check
    CHECK (video_named_by IS NULL OR (video_named_by <> '' AND video_named_by = lower(video_named_by)));
