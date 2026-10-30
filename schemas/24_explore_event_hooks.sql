-- Optional moderation integration. Payloads contain only public labels/reasons;
-- the endpoint is deployment configuration, never stored with event payloads.
CREATE TABLE explore.event_moderation_hooks (
    id uuid PRIMARY KEY,
    event_id text NOT NULL,
    kind text NOT NULL CHECK (kind IN ('submitted','edited','approved','rejected','deleted')),
    event jsonb NOT NULL,
    edit_bucket timestamp without time zone,
    created_at timestamp without time zone NOT NULL DEFAULT (now() AT TIME ZONE 'UTC'),
    next_attempt_at timestamp without time zone NOT NULL DEFAULT (now() AT TIME ZONE 'UTC'),
    attempts integer NOT NULL DEFAULT 0 CHECK (attempts BETWEEN 0 AND 6),
    claim uuid,
    completed_at timestamp without time zone,
    UNIQUE (event_id, kind, edit_bucket)
);
CREATE INDEX event_moderation_hooks_due_idx
    ON explore.event_moderation_hooks (next_attempt_at,id) WHERE completed_at IS NULL AND attempts < 6;

-- Enabling the optional hook starts at the activation instant, never replays
-- an unconfigured node's historical moderation queue.
CREATE TABLE explore.event_moderation_hook_activation (
    singleton boolean PRIMARY KEY DEFAULT true CHECK (singleton),
    activated_at timestamp without time zone NOT NULL
);
