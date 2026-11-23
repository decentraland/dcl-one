CREATE TABLE IF NOT EXISTS explore_api.magic_deletion_attempts (
    token_key bytea PRIMARY KEY,
    requested_at timestamptz NOT NULL DEFAULT now(),
    status text NOT NULL DEFAULT 'requested'
        CHECK (status IN ('requested', 'processed', 'unprocessed', 'rate_limited', 'upstream_error'))
);
