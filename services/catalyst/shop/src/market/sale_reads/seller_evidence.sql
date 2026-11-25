WITH unresolved AS NOT MATERIALIZED (
    SELECT a.network,a.block_number,a.log_index,a.ordinal,a.contract_address,a.item_id,a.seller,e.kind AS venue_kind
    FROM shop.sale_asset a JOIN shop.sale_execution e USING(network,block_number,log_index)
    JOIN shop.sale_publication pub ON pub.network=a.network AND a.block_number<=pub.block_number
    WHERE a.network=ANY($1) AND a.price>0 AND
       (a.seller IS NULL OR (e.kind='trade' AND a.kind='bid' AND
           (SELECT p.beneficiary FROM shop.sale_payment p WHERE p.consideration AND p.currency=a.currency
             AND (p.network,p.block_number,p.log_index)=(a.network,a.block_number,a.log_index)
             ORDER BY p.ordinal LIMIT 1) IS NULL))
), seller_evidence AS NOT MATERIALIZED (
    SELECT a.network,a.block_number,a.log_index,a.ordinal,candidate.address FROM unresolved a
    CROSS JOIN LATERAL (
        SELECT a.seller AS address
        UNION ALL SELECT p.beneficiary FROM shop.sale_payment p WHERE p.consideration
            AND (p.network,p.block_number,p.log_index)=(a.network,a.block_number,a.log_index)
            AND (a.venue_kind<>'store' OR a.ordinal=p.ordinal)
        UNION ALL SELECT i.creator FROM shop.items i WHERE (i.network,i.contract_address,i.item_id)=(a.network,a.contract_address,a.item_id)
        UNION ALL SELECT c.creator FROM shop.collections c WHERE (c.network,c.contract_address)=(a.network,a.contract_address)
        UNION ALL SELECT s.creator FROM shop.collection_seed s WHERE (s.network,s.contract_address)=(a.network,a.contract_address)
        UNION ALL SELECT v.address FROM shop.collection_event t CROSS JOIN LATERAL (VALUES(t.account),(t.previous_account)) v(address)
            WHERE t.kind='creatorship_transferred' AND (t.network,t.contract_address)=(a.network,a.contract_address)
            AND t.block_number<=(SELECT block_number FROM shop.sale_publication WHERE network=t.network)
    ) candidate WHERE candidate.address IS NOT NULL
)
