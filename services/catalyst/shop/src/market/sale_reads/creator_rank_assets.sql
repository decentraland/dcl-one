-- Period rankings need the published mints for attribution and cumulative
-- relationships. Only lifetime seller totals need all sale types.
, scoped_assets AS NOT MATERIALIZED (
    SELECT * FROM shop.sale_asset WHERE $7=0 OR kind='mint'
)
