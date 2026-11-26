-- The caller scopes execution to the network and UTC days being folded.
, priced AS MATERIALIZED (
    SELECT a.*,e.day,e.kind AS venue_kind FROM execution e JOIN shop.sale_asset a USING(network,block_number,log_index)
    WHERE a.price>0 AND (e.kind<>'trade' OR NOT EXISTS (
        SELECT FROM shop.sale_asset other WHERE (other.network,other.block_number,other.log_index)=
            (a.network,a.block_number,a.log_index) AND other.ordinal<>a.ordinal))
), consideration AS (
    SELECT p.*,e.day,e.kind,e.venue,
           sum(p.gross) OVER (PARTITION BY p.network,p.block_number,p.log_index,p.currency,
               CASE WHEN e.kind='trade' THEN NULL ELSE p.ordinal END) AS line_gross
    FROM execution e JOIN shop.sale_payment p USING(network,block_number,log_index)
    WHERE p.consideration
), classified AS (
    SELECT p.*,a.ordinal AS asset_ordinal,p.kind='name_fee' OR a.price IS NOT NULL AS allocated
    FROM consideration p LEFT JOIN priced a
      ON (a.network,a.block_number,a.log_index,a.currency,a.price)=
         (p.network,p.block_number,p.log_index,p.currency,p.line_gross)
     AND (p.kind='trade' OR a.ordinal=p.ordinal)
)
