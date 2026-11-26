WITH sizes AS (
    SELECT e.estate_id,sum(CASE WHEN e.kind='add_land' THEN 1 ELSE -1 END) AS size
    FROM land_authz.authz_event e
    WHERE e.token_address=$7 AND e.estate_id::text=ANY($2::text[])
      AND e.kind IN ('add_land','remove_land') AND e.block_number<=$1
    GROUP BY e.estate_id
), page AS MATERIALIZED (
    SELECT estate_id,size FROM sizes WHERE $3::bigint IS NULL OR size>$3
    ORDER BY estate_id::text COLLATE "C" LIMIT $5 OFFSET $6
), candidates AS (
    SELECT DISTINCT e.token_id
    FROM land_authz.authz_event e JOIN page p ON p.estate_id=e.estate_id
    WHERE $4::boolean AND e.token_address=$7 AND e.kind='add_land'
      AND e.token_id IS NOT NULL AND e.block_number<=$1
), parcels AS (
    SELECT e.estate_id,jsonb_agg(
        jsonb_build_object('tokenId',c.token_id::text,
                           'x',land_authz.token_x(c.token_id),'y',land_authz.token_y(c.token_id))
        ORDER BY c.token_id::text COLLATE "C") AS data
    FROM candidates c CROSS JOIN LATERAL (
        SELECT e.estate_id,e.kind FROM land_authz.authz_event e
        WHERE e.token_address=$7 AND e.token_id=c.token_id AND e.token_id IS NOT NULL
          AND e.kind IN ('add_land','remove_land') AND e.block_number<=$1
        ORDER BY e.block_number DESC,e.log_index DESC LIMIT 1
    ) e JOIN page p USING(estate_id)
    WHERE e.kind='add_land' GROUP BY e.estate_id
)
SELECT jsonb_build_object('tokenId',p.estate_id::text,'size',p.size)
    || CASE WHEN $4 THEN jsonb_build_object('parcels',coalesce(parcels.data,'[]'::jsonb)) ELSE '{}'::jsonb END
FROM page p LEFT JOIN parcels USING(estate_id)
ORDER BY p.estate_id::text COLLATE "C"
