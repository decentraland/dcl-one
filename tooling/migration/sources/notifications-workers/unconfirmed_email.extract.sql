-- updated_at is when the current code was issued (a re-request rewrites it).
SELECT u.address AS source_address,
       lower(u.address) AS address,
       btrim(u.email) AS email,
       u.code,
       timestamptz 'epoch' + u.updated_at * interval '1 millisecond' AS updated_at
FROM unconfirmed_emails u
WHERE lower(u.address) ~ '^0x[0-9a-f]{40}$'
  AND btrim(u.email) <> ''
  AND u.code <> ''
