-- Preserve raw URLs for exact configured-origin preflight; do not URL-decode keys.
SELECT i.id, i.user_address, i.url, i.thumbnail_url,
       i.is_public, i.metadata,
       i.created_at AT TIME ZONE 'UTC' AS created_at
FROM images i
