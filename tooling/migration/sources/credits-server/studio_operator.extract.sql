-- Every wallet acting for a studio. Upstream stores addresses lower-case (a CHECK); lower() keeps
-- the node's address CHECK whatever an older row holds.
SELECT o.studio_id, lower(o.address) AS address, o.added_by,
       to_timestamp(o.created_at / 1000.0) AS created_at
FROM studio_operators o
