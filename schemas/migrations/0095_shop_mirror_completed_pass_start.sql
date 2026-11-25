ALTER TABLE shop.mirror_state
    ADD COLUMN IF NOT EXISTS last_complete_pass_started_at timestamp without time zone;

UPDATE shop.mirror_state
SET last_complete_pass_started_at = pass_started_at
WHERE pass_started_at <= last_complete_pass_at
  AND last_complete_pass_started_at IS NULL;

-- Keep both completed-pass bounds in sync even with an older mirror writer.
CREATE OR REPLACE FUNCTION shop.remember_mirror_completed_pass_start() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP = 'INSERT' OR NEW.last_complete_pass_at IS DISTINCT FROM OLD.last_complete_pass_at THEN
        NEW.last_complete_pass_started_at := CASE
            WHEN NEW.pass_started_at <= NEW.last_complete_pass_at THEN NEW.pass_started_at
        END;
    END IF;
    RETURN NEW;
END;
$$;

CREATE OR REPLACE TRIGGER remember_mirror_completed_pass_start
BEFORE INSERT OR UPDATE OF last_complete_pass_at ON shop.mirror_state
FOR EACH ROW EXECUTE FUNCTION shop.remember_mirror_completed_pass_start();
