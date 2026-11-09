-- The moderation outbox: the Slack notices the events and places writes
-- capture in their own transaction (`explore::moderation_hooks`), one row per
-- notice, delivered by the leased `moderation-hook` job at least once with a
-- six-attempt ceiling. `channel` names the webhook a row goes to. Payloads
-- carry public labels and rendered messages only; the endpoints are
-- deployment configuration, never stored.
CREATE TABLE explore.moderation_hooks (
    id uuid PRIMARY KEY,
    channel text NOT NULL CHECK (channel IN ('events','places')),
    -- The event id, the wallet of a 'profile_permissions' notice, the place id.
    subject text NOT NULL,
    kind text NOT NULL,
    -- events: the public labels the message renders at delivery; places: the message.
    payload jsonb NOT NULL,
    creator_address text,
    edit_bucket timestamp without time zone,
    created_at timestamp without time zone NOT NULL DEFAULT (now() AT TIME ZONE 'UTC'),
    next_attempt_at timestamp without time zone NOT NULL DEFAULT (now() AT TIME ZONE 'UTC'),
    attempts integer NOT NULL DEFAULT 0 CHECK (attempts BETWEEN 0 AND 6),
    claim uuid,
    completed_at timestamp without time zone,
    CONSTRAINT moderation_hooks_kind_check CHECK (
        CASE channel
            WHEN 'events' THEN kind IN ('submitted','edited','approved','rejected','deleted','profile_permissions','upcoming')
            WHEN 'places' THEN kind IN ('disabled','rating_upgraded')
        END),
    UNIQUE (channel, subject, kind, edit_bucket)
);
-- Operator actions precede submissions, which precede edited notices.
CREATE INDEX moderation_hooks_due_idx
    ON explore.moderation_hooks ((CASE WHEN kind IN ('approved','rejected','deleted','disabled','rating_upgraded') THEN 0 WHEN kind='submitted' THEN 1 ELSE 2 END),next_attempt_at,id)
    WHERE completed_at IS NULL AND attempts < 6;

-- Bounded rolling creator throttle applies only to edited notices.
CREATE INDEX moderation_hooks_creator_idx
    ON explore.moderation_hooks (creator_address,created_at DESC)
    WHERE kind='edited';

CREATE INDEX moderation_hooks_retention_idx
    ON explore.moderation_hooks (created_at,id);

-- Enabling the optional hook starts at the activation instant, never replays
-- an unconfigured node's historical moderation queue.
CREATE TABLE explore.moderation_hook_activation (
    singleton boolean PRIMARY KEY DEFAULT true CHECK (singleton),
    activated_at timestamp without time zone NOT NULL
);
