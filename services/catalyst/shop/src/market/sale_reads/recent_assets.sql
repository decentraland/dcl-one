, scoped_assets AS MATERIALIZED (
    SELECT a.* FROM shop.sale_execution e
    CROSS JOIN LATERAL (
        SELECT a.* FROM shop.sale_asset a
        WHERE (a.network,a.block_number,a.log_index)=(e.network,e.block_number,e.log_index)
          AND ($7::text IS NULL OR a.kind=$7) AND a.price>0 AND a.currency=CASE WHEN e.network='ethereum' THEN $2 ELSE $3 END
          AND (e.kind<>'trade' OR NOT EXISTS (SELECT FROM shop.sale_asset other WHERE
            (other.network,other.block_number,other.log_index)=(a.network,a.block_number,a.log_index)
            AND other.ordinal<>a.ordinal))
        OFFSET 0
    ) a
    WHERE e.network=ANY($1) AND e.block_number<=(SELECT block_number FROM shop.sale_publication WHERE network=e.network)
      AND ($13::bigint IS NULL OR e.block_time >= $13)
      AND ($14::bigint IS NULL OR e.block_time <= $14)
    ORDER BY e.block_time DESC,a.public_id
    LIMIT CASE WHEN $21::boolean THEN $19::bigint+$20::bigint ELSE $22::bigint END
)
