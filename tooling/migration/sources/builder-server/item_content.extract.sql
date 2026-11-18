-- The item's `contents` map (file name -> content hash), one row per file.
-- `owner` is the item owner, recorded as the uploader of a hash the node has
-- not accepted before.
SELECT i.id AS item_id,
       f.key AS file,
       f.value AS hash,
       i.eth_address AS owner
FROM items i
CROSS JOIN LATERAL json_each_text(CASE WHEN json_typeof(i.contents) = 'object' THEN i.contents ELSE '{}'::json END) AS f
