-- shop.nft_last_sale keeps each token's latest public sale for /v1/nfts.
-- The publication records through which block that table holds every
-- published sale, and the input history it was built from. A writer from
-- before this step keeps those columns as they were while it republishes,
-- so a history it replaces no longer matches them. An existing publication
-- reads NULL until the sales fold has filled the table from its ledger, one
-- bounded window per pass.
ALTER TABLE IF EXISTS shop.sale_publication
    ADD COLUMN IF NOT EXISTS last_sale_through bigint,
    ADD COLUMN IF NOT EXISTS last_sale_epochs uuid[],
    ADD COLUMN IF NOT EXISTS last_sale_revisions bigint[],
    ADD COLUMN IF NOT EXISTS last_sale_late_collections bigint;
