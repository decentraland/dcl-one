-- Upstream `blocked` is the storage-quota block of an owner over its allowance
-- (deploys refused, worlds hidden), not a visitor ban: it lands in
-- worlds.quota_blocks, never in the platform-wide worlds.blocked_wallets. That
-- table comes with the worlds storage-quota schema; on a schema without it a
-- non-empty batch stops instead of being dropped.
DO $quota$ DECLARE n bigint; BEGIN
  IF to_regclass('worlds.quota_blocks') IS NULL THEN
    IF EXISTS (SELECT 1 FROM stage) THEN
      RAISE EXCEPTION 'worlds-content-server/quota_block: worlds.quota_blocks is not in this schema';
    END IF;
  ELSE
    INSERT INTO worlds.quota_blocks (wallet, created_at, updated_at)
    SELECT s.wallet, s.created_at AT TIME ZONE 'UTC', s.updated_at AT TIME ZONE 'UTC'
    FROM stage s
    -- A block the node already holds keeps its start (the grace window runs from it).
    ON CONFLICT (wallet) DO UPDATE SET
        created_at = LEAST(quota_blocks.created_at, EXCLUDED.created_at),
        updated_at = GREATEST(quota_blocks.updated_at, EXCLUDED.updated_at);
    GET DIAGNOSTICS n = ROW_COUNT;
    RAISE NOTICE 'worlds-content-server/quota_block: upserted % quota blocks', n;
  END IF;
END $quota$;
