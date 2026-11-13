-- Addressed rows; ids are preserved. `timestamp` is epoch ms; values below
-- 1e11 can only be epoch seconds and are scaled.
SELECT n.id,
       lower(n.address) AS address,
       n.type,
       n.metadata,
       timestamptz 'epoch' + (CASE WHEN n."timestamp" < 100000000000 THEN n."timestamp" * 1000 ELSE n."timestamp" END) * interval '1 millisecond' AS occurred_at,
       timestamptz 'epoch' + n.read_at * interval '1 millisecond' AS read_at,
       timestamptz 'epoch' + n.created_at * interval '1 millisecond' AS created_at,
       timestamptz 'epoch' + n.updated_at * interval '1 millisecond' AS updated_at
FROM notifications n
WHERE n.address IS NOT NULL
  AND lower(n.address) ~ '^0x[0-9a-f]{40}$'
