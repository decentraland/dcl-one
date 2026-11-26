ALTER TABLE IF EXISTS shop.third_party_registry_event ADD COLUMN IF NOT EXISTS is_review boolean;
ALTER TABLE land_authz.index_cursor ADD COLUMN IF NOT EXISTS folded_block_hash text;

-- Retain the published header identity while later windows advance the walk.
CREATE OR REPLACE FUNCTION land_authz.remember_folded_block_hash() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    NEW.folded_block_hash := CASE WHEN NEW.folded_block IS NOT NULL THEN NEW.last_block_hash END;
    RETURN NEW;
END;
$$;

CREATE OR REPLACE TRIGGER remember_folded_block_hash
BEFORE UPDATE OF folded_block ON land_authz.index_cursor
FOR EACH ROW EXECUTE FUNCTION land_authz.remember_folded_block_hash();
