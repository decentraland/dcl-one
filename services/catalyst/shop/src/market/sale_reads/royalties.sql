-- An Ethereum v1 NFT identifies its item via the collection and Issue log.
, resales AS MATERIALIZED (
    SELECT s.*, coalesce(n.item_id, s.item_id) AS resolved_item_id
    FROM sold s
    LEFT JOIN ethereum_tokens n ON s.network = 'ethereum' AND
        (n.contract_address, n.token_id) = (s.contract_address, s.token_id)
    WHERE s.kind IN ('order', 'bid') AND s.token_id IS NOT NULL
        AND ($8::bigint IS NULL OR s.block_time * 1000 >= $8)
        AND ($9::bigint IS NULL OR s.block_time * 1000 <= $9)
), page AS (
    SELECT * FROM resales
    ORDER BY block_time DESC, public_id
    LIMIT $10 OFFSET $11
)
SELECT coalesce(jsonb_agg(jsonb_build_object(
           'id', s.public_id, 'timestamp', s.block_time * 1000,
           'contractAddress', s.contract_address, 'itemId', s.resolved_item_id::text,
           'tokenId', s.token_id::text, 'priceWei', s.price::text,
           'royaltyWei', s.royalties::text, 'collector', collectors.recipient,
           'buyer', s.buyer, 'seller', s.seller, 'network', s.network
       ) ORDER BY s.block_time DESC, s.public_id), '[]'::jsonb),
       (SELECT count(*) FROM resales),
       (SELECT coalesce(sum(royalties), 0)::text FROM resales),
       EXISTS (SELECT FROM resales WHERE NOT settled OR seller IS NULL)
FROM page s
CROSS JOIN LATERAL (
    SELECT CASE WHEN count(DISTINCT r.recipient) = 1 THEN min(r.recipient) END AS recipient
    FROM shop.sale_payment p JOIN shop.sale_payout r
      ON (r.network, r.block_number, r.log_index, r.payment_ordinal) =
         (p.network, p.block_number, p.log_index, p.ordinal)
    WHERE (p.network, p.block_number, p.log_index, p.currency) =
          (s.network, s.block_number, s.log_index, s.currency)
      AND p.consideration AND p.settled AND r.role = 'royalty' AND r.amount > 0
) collectors
