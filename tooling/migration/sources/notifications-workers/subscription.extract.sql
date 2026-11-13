-- Upstream times are epoch-ms bigint; addresses are lowercased on write but
-- older rows may not be, so the load dedupes on the lowercased address.
SELECT s.address AS source_address,
       lower(s.address) AS address,
       NULLIF(btrim(s.email), '') AS email,
       s.details,
       timestamptz 'epoch' + s.updated_at * interval '1 millisecond' AS updated_at
FROM subscriptions s
WHERE lower(s.address) ~ '^0x[0-9a-f]{40}$'
