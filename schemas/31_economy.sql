-- Catalyst v2: economy domain: the meta-transaction relay log and quota and
-- the MANA spot archive. The Landiler broker/escrow/NAME custody ledgers and
-- the escrow lease overlay are the Shop's (`shop.broker_purchases`,
-- `shop.escrow_actions`, `shop.name_transfers`, `shop.usage_grants`,
-- schema/30_shop.sql) since schema version 2.
--
-- Derived from catalyrst/crates/catalyrst-economy/migrations/0001..0008 and
-- catalyrst/crates/catalyrst-price/migrations/0001..0003, folded into one
-- create each (the ALTERs inline), re-keyed with the DATABASE.md constraints
-- and every timestamp `timestamp without time zone` holding UTC (the old
-- tables mixed timestamp and timestamptz). No squid copy: the relay's
-- collection allowlist reads shop.collections (the mirror).
--
-- Every row here is local authority written behind DATA_WRITES_ENABLED; the
-- relayer key, the OZ credentials and the admin bearer live in *_FILE paths
-- or the unit's EnvironmentFile and never in this schema. Money is
-- numeric(78,0) wei, never a float.

CREATE SCHEMA economy;

-- The relay log and the quota reservation in one relation: a reservation is
-- `tx_hash NULL, session_id set` (inserted under a per-user advisory lock
-- before the broadcast), a relayed tx is `tx_hash set, session_id NULL`.
-- The quota count binds the request's clock, never now().
CREATE TABLE economy.transactions (
    id bigint PRIMARY KEY GENERATED ALWAYS AS IDENTITY,
    tx_hash text UNIQUE,
    session_id text UNIQUE,
    user_address varchar(42) NOT NULL,
    chain_id integer NOT NULL,
    contract_address varchar(42) NOT NULL,
    created_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    CONSTRAINT transactions_user_lowercase CHECK (user_address = lower(user_address)),
    CONSTRAINT transactions_contract_lowercase CHECK (contract_address = lower(contract_address)),
    CONSTRAINT transactions_reserved_or_relayed CHECK (tx_hash IS NOT NULL OR session_id IS NOT NULL)
);

-- The per-day quota count (WHERE user_address = $1 AND created_at >= $now - interval '1 day')
-- and GET /v1/transactions/{address}.
CREATE INDEX transactions_user_created_idx ON economy.transactions (user_address, created_at);

-- The MANA spot archive: one row per poll (the mana-price-poll job): CoinGecko,
-- or the first fallback exchange that answered (source names it).
CREATE TABLE economy.price_snapshots (
    id bigint PRIMARY KEY GENERATED ALWAYS AS IDENTITY,
    taken_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    source text NOT NULL DEFAULT 'coingecko',
    source_updated_at timestamp without time zone,
    mana_usd numeric(20,8) NOT NULL,
    mana_eth numeric(28,18),
    mana_btc numeric(28,18),
    mana_matic numeric(28,18),
    matic_usd numeric(20,8),
    mana_market_cap_usd numeric(20,2),
    mana_volume_24h_usd numeric(20,2),
    mana_price_change_24h_pct numeric(10,4),
    CONSTRAINT price_snapshots_mana_usd_positive CHECK (mana_usd > 0)
);

-- The CoinGecko-only read before the exchange fallback (2026-09-29); kept, a
-- migration step may not drop it.
CREATE INDEX price_snapshots_taken_at_idx ON economy.price_snapshots (taken_at DESC) WHERE source = 'coingecko';
-- latest_snapshot: ORDER BY taken_at DESC LIMIT 1 over every source (coingecko,
-- or the coinbase/binance/kraken fallback when CoinGecko fails).
CREATE INDEX price_snapshots_newest_idx ON economy.price_snapshots (taken_at DESC);

-- The operator's manual spot (admin bearer), exact numeric.
CREATE TABLE economy.price_overrides (
    token_id text NOT NULL,
    vs_currency text NOT NULL,
    value numeric NOT NULL,
    note text,
    updated_by text,
    updated_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    PRIMARY KEY (token_id, vs_currency),
    CONSTRAINT price_overrides_key_lowercase CHECK (token_id = lower(token_id) AND vs_currency = lower(vs_currency))
);

CREATE TABLE economy.price_override_audit (
    id bigint PRIMARY KEY GENERATED ALWAYS AS IDENTITY,
    action text NOT NULL,
    token_id text NOT NULL,
    vs_currency text NOT NULL,
    value numeric,
    note text,
    admin text NOT NULL,
    detail jsonb NOT NULL DEFAULT '{}'::jsonb,
    created_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    CONSTRAINT price_override_audit_action_known CHECK (action IN ('override.set', 'override.clear'))
);

-- The console's per-key history, newest first.
CREATE INDEX price_override_audit_key_idx ON economy.price_override_audit (token_id, vs_currency, id DESC);
