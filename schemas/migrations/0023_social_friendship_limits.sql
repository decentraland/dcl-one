-- Fixed-window mutation counters: at most two atomic keyed increments per
-- attempt (actor, then pair). Expiry is set only by the first increment. NULL
-- records Redis's persistent key after a failed first EXPIRE. The expiry/key
-- index serves bounded 1000-row cleanup; no profile or friendship FK applies.
-- UTC Unix milliseconds preserve the signed-64 Redis TTL range, which exceeds
-- PostgreSQL timestamp range. The database clock supplies production time.
CREATE TABLE IF NOT EXISTS social.friendship_mutation_limits (
    key text NOT NULL,
    attempts bigint NOT NULL,
    expires_at_ms bigint,
    CONSTRAINT friendship_mutation_limits_pkey PRIMARY KEY (key),
    CONSTRAINT friendship_mutation_limits_attempts_check CHECK (attempts > 0)
);
CREATE INDEX IF NOT EXISTS friendship_mutation_limits_expiry_idx
    ON social.friendship_mutation_limits (expires_at_ms, key) WHERE expires_at_ms IS NOT NULL;
