-- A reader's mark on an address-less row, with the row's payload so readers
-- without a subscription still get their (read) copy.
SELECT br.notification_id,
       br.address AS source_address,
       lower(br.address) AS address,
       n.type,
       n.metadata,
       timestamptz 'epoch' + (CASE WHEN n."timestamp" < 100000000000 THEN n."timestamp" * 1000 ELSE n."timestamp" END) * interval '1 millisecond' AS occurred_at,
       timestamptz 'epoch' + n.created_at * interval '1 millisecond' AS created_at,
       timestamptz 'epoch' + br.read_at * interval '1 millisecond' AS read_at
FROM broadcast_read br
JOIN notifications n ON n.id = br.notification_id AND n.address IS NULL
WHERE lower(br.address) ~ '^0x[0-9a-f]{40}$'
