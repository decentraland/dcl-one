-- Every upstream pending email, filtered as unconfirmed_email.extract.sql.
SELECT u.address AS source_address,
       lower(u.address) AS address
FROM unconfirmed_emails u
WHERE lower(u.address) ~ '^0x[0-9a-f]{40}$'
  AND btrim(u.email) <> ''
  AND u.code <> ''
