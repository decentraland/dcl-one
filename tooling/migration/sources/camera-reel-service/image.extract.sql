-- One row per photo. Upstream names the bytes by S3 object key, rendered into
-- `url`/`thumbnail_url` as <API_URL>/api/images/<key>; the key is what the
-- blob copy maps to a content hash. Photos taken before thumbnails existed
-- have an empty thumbnail_url (thumbnail_key NULL).
SELECT i.id,
       i.user_address,
       substring(i.url FROM '/api/images/([^/?#]+)$') AS image_key,
       NULLIF(substring(i.thumbnail_url FROM '/api/images/([^/?#]+)$'), '') AS thumbnail_key,
       i.is_public,
       i.metadata,
       i.created_at AT TIME ZONE 'UTC' AS created_at
FROM images i
