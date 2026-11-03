-- Millisecond times are kept as stored (the node's wire exposes them);
-- `updated_at` is the same instant as a timestamptz for the watermark.
SELECT o.id,
       o.name,
       o.thumbnail_hash,
       o.items,
       o.body_shape,
       o.gradient_from,
       o.gradient_to,
       o.author_address,
       o.published,
       o.created_at AS created_at_ms,
       o.updated_at AS updated_at_ms,
       timestamptz 'epoch' + o.updated_at * interval '1 millisecond' AS updated_at
FROM outfits o
