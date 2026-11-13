-- The thumbnail records without their bytes (the blob copy moves those).
SELECT t.hash,
       t.content_type,
       t.size,
       t.author_address,
       t.created_at AS created_at_ms,
       timestamptz 'epoch' + t.created_at * interval '1 millisecond' AS created_at
FROM outfit_thumbnails t
