ALTER TABLE shop.mirror_state
    ADD COLUMN IF NOT EXISTS last_complete_pass_started_at timestamp without time zone;

UPDATE shop.mirror_state
SET last_complete_pass_started_at = pass_started_at
WHERE pass_started_at <= last_complete_pass_at
  AND last_complete_pass_started_at IS NULL;
