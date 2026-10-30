-- Catalyst v2: land_authz domain: the one fresh chain index. Who may deploy
-- to a parcel, folded from the LAND and Estate registries' logs.
--
-- Derived from catalyrst/crates/catalyrst-land-authz/migrations/0001..0002 as
-- the indexer (events.rs, indexer.rs) and the resolver (resolve.rs) use them,
-- with the squid joins removed: the old resolver read squid_marketplace.parcel
-- and .estate for coordinates, ownership and estate membership. Here the
-- coordinates are the token id (token_x/token_y), the owner is the latest
-- Transfer (its `account` is `to`) and estate membership is the Estate
-- registry's AddLand/RemoveLand logs, two topics the indexer adds. The
-- schema is created here (the old bootstrap script created it out of band);
-- every timestamp is `timestamp without time zone` UTC, block times are the
-- chain's unix seconds.
--
-- Readers use this index only after index_cursor.folded_block is set (a
-- completed full replay); until then the marketplace subgraph reader answers.

CREATE SCHEMA land_authz;

-- The log, append-only, keyed by the chain's own position.
CREATE TABLE land_authz.authz_event (
    block_number bigint NOT NULL,
    log_index integer NOT NULL,
    block_time bigint NOT NULL,
    token_address varchar(42) NOT NULL,
    kind text NOT NULL,
    token_id numeric(78,0),
    estate_id numeric(78,0),
    account varchar(42),
    operator varchar(42),
    approved boolean,
    PRIMARY KEY (block_number, log_index),
    CONSTRAINT authz_event_kind_known CHECK (kind IN (
        'update_operator', 'approval', 'transfer', 'update_manager', 'approved_for_all', 'add_land', 'remove_land')),
    CONSTRAINT authz_event_token_address_lowercase CHECK (token_address = lower(token_address)),
    CONSTRAINT authz_event_estate_kinds_carry_estate CHECK ((kind IN ('add_land', 'remove_land')) = (estate_id IS NOT NULL))
);

-- The per-token legs of the fold: DISTINCT ON (token_address, token_id) ORDER BY block_number DESC, log_index DESC.
CREATE INDEX authz_event_token_idx ON land_authz.authz_event (token_address, token_id, block_number, log_index)
    WHERE token_id IS NOT NULL;
-- Scene-collaborator latest ownership/membership probes must skip approval
-- churn. The 1.041M-event profile reduces a hot parcel probe from 5.456 to
-- 0.211 ms p50; see METRICS.md, LAND collaborator history (2026-10-26).
CREATE INDEX authz_event_ownership_idx ON land_authz.authz_event
    (token_address, token_id, block_number, log_index)
    WHERE token_id IS NOT NULL AND kind IN ('transfer', 'add_land', 'remove_land');
-- One-time legacy timestamp repair selects at most 100 distinct blocks. This
-- due-set index avoids rescanning positive history on every repair pass
-- (92.880 to 0.075 ms p50 on the same fixture); it empties after repair.
CREATE INDEX authz_event_missing_time_idx ON land_authz.authz_event (block_number)
    WHERE block_time <= 0;
-- The account legs: DISTINCT ON (token_address, account, operator, kind).
CREATE INDEX authz_event_account_idx ON land_authz.authz_event (token_address, account, operator, kind, block_number, log_index)
    WHERE account IS NOT NULL AND operator IS NOT NULL;

-- LAND packs its signed coordinates into the two 128-bit halves of the token
-- id; the fold and the reverse lookups read coordinates the way the contract
-- wrote them.
CREATE FUNCTION land_authz.decode_coord(half numeric) RETURNS integer
LANGUAGE sql IMMUTABLE AS $$
  SELECT (CASE WHEN half >= 170141183460469231731687303715884105728::numeric
               THEN half - 340282366920938463463374607431768211456::numeric
               ELSE half END)::integer
$$;

CREATE FUNCTION land_authz.token_x(token_id numeric) RETURNS integer
LANGUAGE sql IMMUTABLE AS $$
  SELECT land_authz.decode_coord(div(token_id, 340282366920938463463374607431768211456::numeric))
$$;

CREATE FUNCTION land_authz.token_y(token_id numeric) RETURNS integer
LANGUAGE sql IMMUTABLE AS $$
  SELECT land_authz.decode_coord(mod(token_id, 340282366920938463463374607431768211456::numeric))
$$;

-- Per-token rights, derived: `operator` from the latest Approval,
-- `update_operator` from the latest UpdateOperator, `owner` from the latest
-- Transfer (which also clears both operators); the zero address is NULL.
-- x/y are NULL for estate tokens.
CREATE TABLE land_authz.token_right (
    token_address varchar(42) NOT NULL,
    token_id numeric(78,0) NOT NULL,
    x integer,
    y integer,
    owner varchar(42),
    operator varchar(42),
    update_operator varchar(42),
    updated_block bigint NOT NULL,
    updated_log integer NOT NULL,
    PRIMARY KEY (token_address, token_id)
);

-- The parcel lookup by coordinates (parcel_subject, rights_batch over unnest).
CREATE INDEX token_right_xy_idx ON land_authz.token_right (x, y) WHERE x IS NOT NULL;
-- The reverse lookups of lands-permissions.
CREATE INDEX token_right_update_operator_idx ON land_authz.token_right (update_operator) WHERE update_operator IS NOT NULL;
CREATE INDEX token_right_operator_idx ON land_authz.token_right (operator) WHERE operator IS NOT NULL;
-- The parcels/estates an address owns (the worlds policy LAND leg).
CREATE INDEX token_right_owner_idx ON land_authz.token_right (token_address, owner) WHERE owner IS NOT NULL;

-- Estate membership, derived: the latest AddLand puts a LAND in an estate, a
-- later RemoveLand deletes the row.
CREATE TABLE land_authz.estate_land (
    land_token_id numeric(78,0) PRIMARY KEY,
    estate_token_id numeric(78,0) NOT NULL,
    updated_block bigint NOT NULL,
    updated_log integer NOT NULL
);

-- The parcels of an estate (parcels_updatable_by's estate branch, estate size).
CREATE INDEX estate_land_estate_idx ON land_authz.estate_land (estate_token_id);

-- Account-wide rights per registry: UpdateManager and ApprovalForAll
-- authorise an operator over everything an account holds in one contract.
CREATE TABLE land_authz.account_right (
    token_address varchar(42) NOT NULL,
    account varchar(42) NOT NULL,
    operator varchar(42) NOT NULL,
    kind text NOT NULL,
    is_approved boolean NOT NULL,
    updated_block bigint NOT NULL,
    updated_log integer NOT NULL,
    PRIMARY KEY (token_address, account, operator, kind),
    CONSTRAINT account_right_kind_known CHECK (kind IN ('update_manager', 'approved_for_all'))
);

-- The grant legs (account_grants) and their reverse.
CREATE INDEX account_right_account_idx ON land_authz.account_right (account, token_address, kind) WHERE is_approved;
CREATE INDEX account_right_operator_idx ON land_authz.account_right (operator, kind) WHERE is_approved;

-- Durable progress per network: `last_block` advances per inserted chunk,
-- `folded_block` is the publish marker (set only after the three folds
-- committed on a row whose full replay ran); a repair scan rewinds it.
CREATE TABLE land_authz.index_cursor (
    network text PRIMARY KEY,
    land_registry varchar(42) NOT NULL,
    estate_registry varchar(42) NOT NULL,
    last_block bigint NOT NULL,
    folded_block bigint,
    folded_at timestamp without time zone,
    full_replay_started boolean NOT NULL DEFAULT false,
    -- Set only after the indexer repairs legacy missing block timestamps.
    history_timestamps_ready boolean NOT NULL DEFAULT false,
    updated_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    CONSTRAINT index_cursor_network_known CHECK (network IN ('ethereum-mainnet', 'ethereum-sepolia')),
    CONSTRAINT index_cursor_folded_within_indexed CHECK (folded_block IS NULL OR folded_block <= last_block)
);

-- Bounded scene-footprint probes use token_right_xy_idx and authz_event_ownership_idx.
-- The source deployment must have passed signed-entity validation (including its
-- 15-minute future bound). Subtracting that allowance prevents a pre-transfer
-- deployment from outliving the transfer, including an away-and-back transfer.
-- Fresh published ownership remains usable while unrelated events ingest.
-- Any relevant event newer than that fold denies the affected footprint until
-- folded, including membership changes that could hide a new estate subject.
-- This uses the immutable signed timestamp, never import/resync/arrival time.
-- A valid deployment too close to a transfer needs a later deployment; waiting
-- cannot make its fixed timestamp eligible.
CREATE FUNCTION land_authz.scene_collaborators_current(
    configured_network text, configured_land text, configured_estate text,
    pointers text[], entity_timestamp timestamp without time zone
) RETURNS boolean LANGUAGE sql STABLE AS $$
  WITH ready AS (
    SELECT folded_block FROM land_authz.index_cursor
    WHERE network = configured_network
      AND land_registry = configured_land AND estate_registry = configured_estate
      AND full_replay_started AND history_timestamps_ready
      AND folded_block IS NOT NULL
      AND folded_at BETWEEN timezone('UTC', now()) - interval '30 minutes'
                        AND timezone('UTC', now())
  ), parcels AS (
    SELECT p.pointer, t.token_id, t.owner, el.estate_token_id, et.owner AS estate_owner
    FROM unnest(pointers) AS p(pointer)
    LEFT JOIN land_authz.token_right t
      ON t.token_address = configured_land
     AND t.x = CASE WHEN p.pointer ~ '^-?[0-9]{1,9},-?[0-9]{1,9}$'
                        THEN split_part(p.pointer, ',', 1)::integer END
     AND t.y = CASE WHEN p.pointer ~ '^-?[0-9]{1,9},-?[0-9]{1,9}$'
                        THEN split_part(p.pointer, ',', 2)::integer END
     AND p.pointer = t.x::text || ',' || t.y::text
    LEFT JOIN land_authz.estate_land el ON el.land_token_id = t.token_id
    LEFT JOIN land_authz.token_right et
      ON et.token_address = configured_estate AND et.token_id = el.estate_token_id
  )
  SELECT COALESCE(
    entity_timestamp > timestamp '1970-01-01'
    AND entity_timestamp <= timezone('UTC', now())
    AND cardinality(pointers) > 0 AND EXISTS (SELECT 1 FROM ready)
    AND (SELECT bool_and(COALESCE(
      p.token_id IS NOT NULL AND p.owner IS NOT NULL
      AND p.owner <> '0x0000000000000000000000000000000000000000'
      AND ((p.owner = configured_estate) = (p.estate_token_id IS NOT NULL))
      AND lt.block_number <= (SELECT folded_block FROM ready)
      AND lt.block_time > 0
      AND lt.block_time < extract(epoch FROM entity_timestamp - interval '15 minutes')
      AND (membership.block_number IS NULL OR (
        membership.block_number <= (SELECT folded_block FROM ready)
        AND membership.block_time > 0
        AND membership.block_time < extract(epoch FROM entity_timestamp - interval '15 minutes')))
      AND (p.estate_token_id IS NULL OR (
        p.estate_owner IS NOT NULL
        AND p.estate_owner <> '0x0000000000000000000000000000000000000000'
        AND et.block_number <= (SELECT folded_block FROM ready)
        AND et.block_time > 0
        AND et.block_time < extract(epoch FROM entity_timestamp - interval '15 minutes')))
    , false)) FROM parcels p
    LEFT JOIN LATERAL (
      SELECT block_number, block_time FROM land_authz.authz_event
      WHERE token_address = configured_land AND token_id = p.token_id AND kind = 'transfer'
      ORDER BY block_number DESC, log_index DESC LIMIT 1
    ) lt ON true
    LEFT JOIN LATERAL (
      SELECT block_number, block_time FROM land_authz.authz_event
      WHERE token_address = configured_estate AND token_id = p.token_id
        AND kind IN ('add_land', 'remove_land')
      ORDER BY block_number DESC, log_index DESC LIMIT 1
    ) membership ON true
    LEFT JOIN LATERAL (
      SELECT block_number, block_time FROM land_authz.authz_event
      WHERE token_address = configured_estate AND token_id = p.estate_token_id AND kind = 'transfer'
      ORDER BY block_number DESC, log_index DESC LIMIT 1
    ) et ON true), false)
$$;
