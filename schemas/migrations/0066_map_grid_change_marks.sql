-- What the explorer map's build watches to rebuild when its inputs change
-- (DATABASE.md "Map grid build workload"): a count of the land-authz log's
-- rewrites (a rewind or the block-time repair), a count of a market chain
-- index's rebuilds, and when the build last found the stored grid current.
ALTER TABLE land_authz.index_cursor ADD COLUMN IF NOT EXISTS rewrites bigint NOT NULL DEFAULT 0;
ALTER TABLE shop.chain_cursor ADD COLUMN IF NOT EXISTS rebuilds integer NOT NULL DEFAULT 0;
ALTER TABLE explore_api.map_grid ADD COLUMN IF NOT EXISTS checked_at timestamp without time zone;
