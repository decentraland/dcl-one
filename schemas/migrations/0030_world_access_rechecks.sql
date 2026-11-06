-- Keep the latest edit cutoff even after successful enforcement: refreshed tokens can reconnect.
CREATE TABLE IF NOT EXISTS social.world_access_rechecks (
    world_name text PRIMARY KEY,
    revision uuid NOT NULL DEFAULT gen_random_uuid(),
    changed_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', clock_timestamp()),
    next_attempt_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now())
);
CREATE INDEX IF NOT EXISTS world_access_rechecks_due_idx ON social.world_access_rechecks (next_attempt_at, world_name);
