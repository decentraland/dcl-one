-- Keep the attribution fields from the last completed Ethereum v1 mirror
-- passes. The next full sweep updates the live catalogue before it publishes.
CREATE TABLE IF NOT EXISTS shop.v1_royalty_collection (
    contract_address text PRIMARY KEY,
    urn text NOT NULL,
    creator text NOT NULL
);

CREATE TABLE IF NOT EXISTS shop.v1_royalty_item (
    urn text PRIMARY KEY,
    contract_address text NOT NULL,
    creator text NOT NULL
);

CREATE INDEX IF NOT EXISTS v1_royalty_item_creator_idx ON shop.v1_royalty_item (creator);

CREATE OR REPLACE FUNCTION shop.publish_v1_royalty_catalogue(
    p_mirror text, p_started timestamp without time zone, p_finished timestamp without time zone
) RETURNS void LANGUAGE plpgsql AS $$
BEGIN
    IF p_mirror = 'collections' THEN
        DELETE FROM shop.v1_royalty_collection saved
        WHERE NOT EXISTS (
            SELECT FROM shop.collections c
            WHERE c.network = 'ethereum' AND c.chain_id = 1
              AND c.contract_address = saved.contract_address
              AND c.mirrored_at BETWEEN p_started AND p_finished
        );

        INSERT INTO shop.v1_royalty_collection AS saved (contract_address, urn, creator)
        SELECT c.contract_address, c.urn, c.creator
        FROM shop.collections c
        WHERE c.network = 'ethereum' AND c.chain_id = 1
          AND c.mirrored_at BETWEEN p_started AND p_finished
        ON CONFLICT (contract_address) DO UPDATE SET urn = EXCLUDED.urn, creator = EXCLUDED.creator
        WHERE (saved.urn, saved.creator) IS DISTINCT FROM (EXCLUDED.urn, EXCLUDED.creator);
    ELSIF p_mirror = 'items' THEN
        DELETE FROM shop.v1_royalty_item saved
        WHERE NOT EXISTS (
            SELECT FROM shop.items i
            WHERE i.network = 'ethereum' AND i.chain_id = 1 AND i.item_type = 'wearable_v1'
              AND i.urn = saved.urn AND i.mirrored_at BETWEEN p_started AND p_finished
        );

        INSERT INTO shop.v1_royalty_item AS saved (urn, contract_address, creator)
        SELECT i.urn, i.contract_address, i.creator
        FROM shop.items i
        WHERE i.network = 'ethereum' AND i.chain_id = 1 AND i.item_type = 'wearable_v1'
          AND i.mirrored_at BETWEEN p_started AND p_finished
        ON CONFLICT (urn) DO UPDATE SET contract_address = EXCLUDED.contract_address, creator = EXCLUDED.creator
        WHERE (saved.contract_address, saved.creator) IS DISTINCT FROM (EXCLUDED.contract_address, EXCLUDED.creator);
    END IF;
END;
$$;

-- This trigger uses columns written by both old and new mirror workers.
CREATE OR REPLACE FUNCTION shop.publish_v1_royalties_on_completion() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    IF NEW.network = 'ethereum'
       AND (TG_OP = 'INSERT' OR NEW.last_complete_pass_at IS DISTINCT FROM OLD.last_complete_pass_at) THEN
        PERFORM shop.publish_v1_royalty_catalogue(
            NEW.mirror,
            CASE WHEN NEW.chain_id = 1 OR (NEW.chain_id IS NULL AND NEW.pass_block_hash IS NULL)
                THEN NEW.pass_started_at END,
            NEW.last_complete_pass_at
        );
    END IF;
    RETURN NEW;
END;
$$;

CREATE OR REPLACE TRIGGER publish_v1_royalties_on_completion
AFTER INSERT OR UPDATE OF last_complete_pass_at ON shop.mirror_state
FOR EACH ROW EXECUTE FUNCTION shop.publish_v1_royalties_on_completion();

-- Populate any copy whose completed pass still has both original bounds.
SELECT shop.publish_v1_royalty_catalogue(mirror, pass_started_at, last_complete_pass_at)
FROM shop.mirror_state
WHERE network = 'ethereum' AND pass_started_at <= last_complete_pass_at
  AND (chain_id = 1 OR (chain_id IS NULL AND pass_block_hash IS NULL));
