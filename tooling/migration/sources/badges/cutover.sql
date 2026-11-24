-- Run once at the frozen import boundary, before admitting new activity.
-- Re-running preserves every committed checkpoint, including a running job's.
BEGIN;
SET LOCAL lock_timeout = '5s';
-- psql quotes the operator's last fully covered Polygon block as a literal.
CREATE TEMP TABLE badge_cutover_block (height bigint NOT NULL CHECK (height >= 0)) ON COMMIT DROP;
INSERT INTO badge_cutover_block VALUES (:'last_covered_block');

DO $ready$
BEGIN
    IF (SELECT count(*) FROM shop.mirror_state
        WHERE network = 'matic' AND mirror IN ('items', 'collections')
          AND last_complete_pass_at IS NOT NULL AND last_error IS NULL) <> 2 THEN
        RAISE EXCEPTION 'badge cutover requires complete Polygon items and collections mirrors';
    END IF;
END
$ready$;

-- The same publication conditions the sales job reads. Save the block in
-- the same statement that checks readiness, so an index cannot change
-- between the check and the checkpoint.
DO $sales$
DECLARE
    sources integer;
BEGIN
    WITH published AS (
        SELECT log.source, boundary.height
        FROM (VALUES ('sales', 'sales:sale_event:matic'),
                     ('nft', 'sales:nft_event:matic')) AS log(indexer, source)
        JOIN shop.chain_cursor c USING (indexer)
        CROSS JOIN badge_cutover_block boundary
        WHERE c.network = 'matic' AND c.full_history AND c.chain_id = 137
          AND c.quarantined_at IS NULL AND c.folded_block = c.last_block
          AND c.folded_block >= boundary.height
          AND c.folded_at BETWEEN timezone('UTC', now()) - interval '30 minutes' AND timezone('UTC', now())
    ), seeded AS (
        INSERT INTO social.badge_cursors (source, position, log_index)
        SELECT source, height, 2147483647 FROM published
        ON CONFLICT (source) DO NOTHING
    )
    SELECT count(*) INTO sources FROM published;
    IF sources <> 2 THEN
        RAISE EXCEPTION 'badge cutover requires current Polygon sales and nft indexes';
    END IF;
END
$sales$;

-- Sequence values precede commit. Wait out earlier inserts and hold later
-- inserts until both max-id reads commit, so an in-flight low id cannot
-- disappear below a higher committed id.
LOCK TABLE content.deployments, social.client_events IN SHARE MODE;
INSERT INTO social.badge_cursors (source, position)
VALUES ('deployments', (SELECT coalesce(max(id), 0) FROM content.deployments)),
       ('client_events', (SELECT coalesce(max(seq), 0) FROM social.client_events))
ON CONFLICT (source) DO NOTHING;
COMMIT;
