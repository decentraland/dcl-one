-- One row per address-less notification and audience shard (the three hex
-- digits after `0x`), so a batch fans out to about batch_size / 4096 of the
-- audience per row instead of all of it.
SELECT n.id,
       s.shard,
       n.type,
       n.metadata,
       timestamptz 'epoch' + (CASE WHEN n."timestamp" < 100000000000 THEN n."timestamp" * 1000 ELSE n."timestamp" END) * interval '1 millisecond' AS occurred_at,
       timestamptz 'epoch' + n.created_at * interval '1 millisecond' AS created_at,
       timestamptz 'epoch' + n.updated_at * interval '1 millisecond' AS updated_at
FROM notifications n
CROSS JOIN (SELECT lpad(to_hex(g), 3, '0') AS shard FROM generate_series(0, 4095) g) s
WHERE n.address IS NULL
