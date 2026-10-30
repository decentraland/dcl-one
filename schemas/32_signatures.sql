-- Catalyst v2: signatures domain: the rentals listings (LAND/Estate rental
-- offers signed off-chain) of the old catalyrst-signatures service.
--
-- Derived from catalyrst/crates/catalyrst-signatures/migrations/0001_rentals.sql
-- (itself the consolidated decentraland/signatures-server migrations):
-- `metadata` becomes `rental_assets` keyed by (contract, token) with the
-- wire's `nft_id` as a UNIQUE, uuids come from gen_random_uuid (no uuid-ossp),
-- every timestamp is `timestamp without time zone` UTC. The asset's owner and
-- LAND columns come from the marketplace subgraph reader at create/refresh
-- time (the old squid.nft probe); `fetched_at` is their freshness. The
-- rentals-subgraph status sync had no job in the old crate and is deferred,
-- so no sync row exists. Writes are local authority behind DATA_WRITES_ENABLED.

CREATE SCHEMA signatures;

CREATE TYPE signatures.rental_status AS ENUM ('open', 'executed', 'cancelled', 'claimed');

CREATE TABLE signatures.rental_assets (
    contract_address varchar(42) NOT NULL,
    token_id numeric(78,0) NOT NULL,
    nft_id text NOT NULL UNIQUE,
    network text NOT NULL,
    chain_id integer NOT NULL,
    category text NOT NULL,
    search_text text NOT NULL,
    distance_to_plaza smallint,
    adjacent_to_road boolean,
    estate_size smallint,
    nft_created_at timestamp without time zone NOT NULL,
    nft_updated_at timestamp without time zone NOT NULL,
    fetched_at timestamp without time zone NOT NULL,
    PRIMARY KEY (contract_address, token_id),
    CONSTRAINT rental_assets_contract_lowercase CHECK (contract_address = lower(contract_address)),
    CONSTRAINT rental_assets_category_known CHECK (category IN ('parcel', 'estate'))
);

CREATE TABLE signatures.rentals (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    contract_address varchar(42) NOT NULL,
    token_id numeric(78,0) NOT NULL,
    network text NOT NULL,
    chain_id integer NOT NULL,
    expiration timestamp without time zone NOT NULL,
    nonces text[] NOT NULL,
    signature text NOT NULL UNIQUE,
    rental_contract_address varchar(42) NOT NULL,
    status signatures.rental_status NOT NULL DEFAULT 'open',
    target varchar(42) NOT NULL DEFAULT '0x0000000000000000000000000000000000000000',
    rented_days integer,
    period_chosen uuid,
    created_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    updated_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    started_at timestamp without time zone,
    FOREIGN KEY (contract_address, token_id) REFERENCES signatures.rental_assets(contract_address, token_id) ON DELETE CASCADE,
    CONSTRAINT rentals_nonces_three CHECK (array_length(nonces, 1) = 3)
);

-- One open listing per asset: the 409 of the create route.
CREATE UNIQUE INDEX rentals_open_asset_key ON signatures.rentals (contract_address, token_id) WHERE status = 'open';
-- The per-asset history and the DISTINCT ON (asset) of the listing.
CREATE INDEX rentals_asset_idx ON signatures.rentals (contract_address, token_id, created_at DESC);
-- The market reader's newest-N leg (`shop::market::rentals::newest_listings`:
-- `WHERE status = ANY($1) ORDER BY created_at DESC, id LIMIT 1000`, an
-- incremental sort over this scan). The listing's own default sort is over
-- its DISTINCT ON derived page, which no index supplies.
CREATE INDEX rentals_created_idx ON signatures.rentals (created_at DESC);

CREATE TABLE signatures.rental_listings (
    id uuid PRIMARY KEY REFERENCES signatures.rentals(id) ON DELETE CASCADE,
    lessor varchar(42) NOT NULL,
    tenant varchar(42),
    CONSTRAINT rental_listings_lessor_lowercase CHECK (lessor = lower(lessor)),
    CONSTRAINT rental_listings_tenant_lowercase CHECK (tenant IS NULL OR tenant = lower(tenant))
);

-- ?lessor= and ?tenant=.
CREATE INDEX rental_listings_lessor_idx ON signatures.rental_listings (lessor);
CREATE INDEX rental_listings_tenant_idx ON signatures.rental_listings (tenant) WHERE tenant IS NOT NULL;

CREATE TABLE signatures.rental_periods (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    rental_id uuid NOT NULL REFERENCES signatures.rental_listings(id) ON DELETE CASCADE,
    min_days integer NOT NULL,
    max_days integer NOT NULL,
    price_per_day numeric(78,0) NOT NULL,
    CONSTRAINT rental_periods_min_nonneg CHECK (min_days >= 0),
    CONSTRAINT rental_periods_max_ge_min CHECK (max_days >= min_days),
    CONSTRAINT rental_periods_price_nonneg CHECK (price_per_day >= 0)
);

-- The periods aggregate of every listing row and the DISTINCT rental_id sub-select of the day filters.
CREATE INDEX rental_periods_rental_idx ON signatures.rental_periods (rental_id);
