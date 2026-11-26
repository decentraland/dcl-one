-- shop.nft_last_sale keeps each token's latest public sale for /v1/nfts.
-- The publication records through which block that table holds every
-- published sale, and the publication generation it was kept for. Every
-- writer rotates the generation when it replaces history or corrects a
-- published sale; a writer from before this step leaves these columns as
-- they were, so its changes no longer match them. An existing publication
-- reads NULL until the sales fold has filled the table from its ledger, one
-- bounded window per pass.
ALTER TABLE IF EXISTS shop.sale_publication
    ADD COLUMN IF NOT EXISTS last_sale_through bigint,
    ADD COLUMN IF NOT EXISTS last_sale_generation uuid;
