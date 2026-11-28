SELECT i.id AS item_id,
       f.key AS file,
       f.value AS hash,
       i.eth_address AS owner,
       (f.position - 1)::integer AS position,
       i.updated_at AS item_updated_at
FROM items i
CROSS JOIN LATERAL json_each_text(CASE WHEN json_typeof(i.contents) = 'object' THEN i.contents ELSE '{}'::json END)
     WITH ORDINALITY AS f(key, value, position)
