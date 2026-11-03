SELECT n.id,
       n.type,
       n.metadata,
       timestamptz 'epoch' + (CASE WHEN n."timestamp" < 100000000000 THEN n."timestamp" * 1000 ELSE n."timestamp" END) * interval '1 millisecond' AS occurred_at,
       timestamptz 'epoch' + n.created_at * interval '1 millisecond' AS created_at,
       timestamptz 'epoch' + n.updated_at * interval '1 millisecond' AS updated_at
FROM notifications n
WHERE n.address IS NULL
