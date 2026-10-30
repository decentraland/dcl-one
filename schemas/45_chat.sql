-- Catalyst v2: chat domain: the "Decentraland standalone chat" (<domain>/social,
-- chat.<domain>, dcl.social): community channels and their settings, messages,
-- threads, reactions, reports, pins, the activity feed, the open voice lounges
-- and the frontend's prepare/sign/complete staging.
--
-- Designed from the workload, not copied: the
-- old service's SQLite file (catalyrst/crates/dcl-social-api/src/{lib,
-- conversation,activity}.rs, rusqlite `CREATE TABLE IF NOT EXISTS`) split
-- general and named channels into two tables of one shape and read them
-- through UNION ALL; here they are ONE relation keyed by (community, channel,
-- seq), channel_config folds into channels (private typed, the rest one jsonb
-- unit), INTEGER seq -> bigint identity, INTEGER epoch -> timestamp without
-- time zone UTC, TEXT JSON -> jsonb where read as a unit.
--
-- Authority: this node owns durable community chat. Membership, roles, bans,
-- posts, places, voice lounges and profiles are src/social's rows in schema
-- `social`, read in-process; community_id names social.communities.id with no
-- FK across the schema boundary (DATABASE.md 4): deleting a community's row
-- in chat.channels cascades through every chat table, and that delete is the
-- reconciliation the port runs when social reports the community gone.
--
-- Conventions: wallets are lowercase 0x + 40 hex (src/social/identity.rs
-- Address), checked row-locally; every timestamp holds UTC (the pool pins
-- TimeZone=UTC) and the wire's millisecond `createdAt` is computed on read;
-- the current time is always a bind parameter, never in an index predicate;
-- every index names its query.
--
-- The port agent (src/social/chat) appends ONLY inside the `-- ==== section:
-- chat port ====` block at the end, after re-reading this file.

CREATE SCHEMA chat;

-- ── channels ───────────────────────────────────────────────────────────────

-- Every channel of a community, `general` included (the FK target of
-- messages; the first write into a community inserts its general row ON
-- CONFLICT DO NOTHING in the same transaction). `private` is typed because the
-- channels listing filters on it and general may never be private; `config`
-- is the unit the settings dialog writes and authorize/listing read whole:
-- {"members": [wallet, ...] (at most 100), "read_only": bool, "slow_mode":
-- seconds 0..3600}, validated by chat::ChannelConfig before the bind. An
-- absent key means its default, so '{}' is a fresh channel.
CREATE TABLE chat.channels (
    community_id uuid NOT NULL,
    name text NOT NULL,
    private boolean NOT NULL DEFAULT false,
    config jsonb NOT NULL DEFAULT '{}'::jsonb,
    created_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    updated_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    CONSTRAINT channels_pkey PRIMARY KEY (community_id, name),
    CONSTRAINT channels_name_check CHECK (name ~ '^[a-z0-9-]{1,32}$'),
    CONSTRAINT channels_general_open_check CHECK (name <> 'general' OR NOT private),
    CONSTRAINT channels_config_object_check CHECK (jsonb_typeof(config) = 'object')
);

-- ── messages ───────────────────────────────────────────────────────────────

-- One relation for every channel; `seq` is the server order and the wire
-- cursor (after/before), `id` the client-visible uuid the API mints. `scene`
-- is the optional {x, y, world?} the composer attaches, read as a unit.
-- A moderator's remove sets removed_at (the read renders `[Message removed]`
-- without the scene) and deletes the replies, pins and reports in the same
-- transaction; the row and its time stay.
CREATE TABLE chat.messages (
    seq bigint GENERATED ALWAYS AS IDENTITY,
    id uuid NOT NULL,
    community_id uuid NOT NULL,
    channel text NOT NULL,
    wallet text NOT NULL,
    text text NOT NULL,
    scene jsonb,
    created_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    removed_at timestamp without time zone,
    CONSTRAINT messages_pkey PRIMARY KEY (seq),
    CONSTRAINT messages_channel_fkey FOREIGN KEY (community_id, channel)
        REFERENCES chat.channels (community_id, name) ON DELETE CASCADE,
    CONSTRAINT messages_wallet_check CHECK (wallet ~ '^0x[0-9a-f]{40}$'),
    CONSTRAINT messages_text_length_check CHECK (char_length(text) <= 4000),
    CONSTRAINT messages_scene_object_check CHECK (scene IS NULL OR jsonb_typeof(scene) = 'object')
);

-- The reply/action "message belongs to this community" probe (id = $1 AND
-- community_id = $2), the `message=` history filter and the FK target of
-- replies, reactions, reports and pins.
CREATE UNIQUE INDEX messages_id_uidx ON chat.messages (id);
-- The history page and its 4-s poll: WHERE community_id = $1 AND channel = $2
-- AND seq > $3 AND ($4 IS NULL OR seq < $4) ORDER BY seq DESC LIMIT 101,
-- walked backwards; the search/pinned/id filters are filters under it.
CREATE INDEX messages_channel_seq_idx ON chat.messages (community_id, channel, seq);
-- The slow-mode probe of a send into a channel with slow_mode > 0:
-- SELECT max(created_at) WHERE community_id = $1 AND channel = $2 AND
-- wallet = $3. One extra btree entry per message for a read only slow-mode
-- channels make.
CREATE INDEX messages_slow_mode_idx ON chat.messages (community_id, channel, wallet, created_at DESC);

-- ── threads, reactions, reports, pins ──────────────────────────────────────

-- Replies under a message; `seq` is the thread cursor (reply_before).
CREATE TABLE chat.replies (
    seq bigint GENERATED ALWAYS AS IDENTITY,
    id uuid NOT NULL,
    parent_id uuid NOT NULL REFERENCES chat.messages (id) ON DELETE CASCADE,
    wallet text NOT NULL,
    text text NOT NULL,
    created_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    CONSTRAINT replies_pkey PRIMARY KEY (seq),
    CONSTRAINT replies_wallet_check CHECK (wallet ~ '^0x[0-9a-f]{40}$'),
    CONSTRAINT replies_text_length_check CHECK (char_length(text) <= 4000)
);

CREATE UNIQUE INDEX replies_id_uidx ON chat.replies (id);
-- The page enrichment (the newest 100 and the count per parent_id = ANY($1)),
-- the thread page (WHERE parent_id = $1 AND ($2 IS NULL OR seq < $2) ORDER BY
-- seq DESC LIMIT 101) and the remove's DELETE WHERE parent_id = $1.
CREATE INDEX replies_parent_seq_idx ON chat.replies (parent_id, seq DESC);

-- A wallet's heart/celebrate on a message; the toggle deletes the key and
-- inserts it when nothing was there. The PK serves the enrichment's
-- `message_id = ANY($1) ORDER BY message_id, emoji, wallet`.
CREATE TABLE chat.reactions (
    message_id uuid NOT NULL REFERENCES chat.messages (id) ON DELETE CASCADE,
    wallet text NOT NULL,
    emoji text NOT NULL,
    created_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    CONSTRAINT reactions_pkey PRIMARY KEY (message_id, wallet, emoji),
    CONSTRAINT reactions_wallet_check CHECK (wallet ~ '^0x[0-9a-f]{40}$'),
    CONSTRAINT reactions_emoji_check CHECK (emoji IN ('heart', 'celebrate'))
);

-- Open reports only: a dismiss or remove deletes the message's rows, so the
-- moderators' review (JOIN messages ON id, WHERE m.community_id = $1, GROUP
-- BY message ORDER BY min(created_at) LIMIT 100) scans a table bounded by
-- what is still open.
CREATE TABLE chat.reports (
    message_id uuid NOT NULL REFERENCES chat.messages (id) ON DELETE CASCADE,
    wallet text NOT NULL,
    created_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    CONSTRAINT reports_pkey PRIMARY KEY (message_id, wallet),
    CONSTRAINT reports_wallet_check CHECK (wallet ~ '^0x[0-9a-f]{40}$')
);

-- A moderator's pin; the PK serves the `pinned=1` history filter's EXISTS and
-- the enrichment's `message_id = ANY($1)`.
CREATE TABLE chat.pins (
    message_id uuid NOT NULL REFERENCES chat.messages (id) ON DELETE CASCADE,
    pinned_by text NOT NULL,
    pinned_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    CONSTRAINT pins_pkey PRIMARY KEY (message_id),
    CONSTRAINT pins_pinned_by_check CHECK (pinned_by ~ '^0x[0-9a-f]{40}$')
);

-- ── the activity feed and the open lounges ─────────────────────────────────

-- Append-only news this node witnessed per community: an announcement (a
-- post id), a hangout (a place id) or a lounge (the session start as text),
-- with the actor (lowercase wallet, '' when unknown) and a summary of at most
-- 140 characters. `silent` rows are the baseline sentinels (object '') and
-- the history a first listing held: never fed, only remembered. `seq` is the
-- feed cursor: WHERE NOT silent AND seq > $1 AND actor <> $2 ORDER BY seq,
-- limited to the wallet's communities, the first 100.
CREATE TABLE chat.community_activity (
    seq bigint GENERATED ALWAYS AS IDENTITY,
    community_id uuid NOT NULL,
    kind text NOT NULL,
    object text NOT NULL,
    actor text NOT NULL,
    summary text NOT NULL,
    created_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    silent boolean NOT NULL DEFAULT false,
    CONSTRAINT community_activity_pkey PRIMARY KEY (seq),
    CONSTRAINT community_activity_kind_check CHECK (kind IN ('announcement', 'hangout', 'lounge')),
    CONSTRAINT community_activity_actor_check CHECK (actor = '' OR actor ~ '^0x[0-9a-f]{40}$'),
    CONSTRAINT community_activity_summary_length_check CHECK (char_length(summary) <= 140)
);

-- Every witness is `INSERT ... ON CONFLICT DO NOTHING` on this key (the same
-- post, place or lounge session is news once); the baseline sentinel lookup
-- (community_id, kind, '') and the hangout removal DELETE are probes on it.
CREATE UNIQUE INDEX community_activity_object_uidx ON chat.community_activity (community_id, kind, object);

-- The voice lounges currently seen open: one row per live lounge, gone when
-- the observer's next listing no longer shows it. A lounge unseen for
-- chat::LIVE_GAP starts a new session when seen again. Bounded by the
-- communities with a live voice chat; the `WHERE observer = $1` scan needs no
-- index.
CREATE TABLE chat.community_live (
    community_id uuid NOT NULL,
    session_started_at timestamp without time zone NOT NULL,
    seen_at timestamp without time zone NOT NULL,
    observer text NOT NULL,
    CONSTRAINT community_live_pkey PRIMARY KEY (community_id),
    CONSTRAINT community_live_observer_check CHECK (observer ~ '^0x[0-9a-f]{40}$'),
    CONSTRAINT community_live_session_order_check CHECK (session_started_at <= seen_at)
);

-- ── the prepare/sign/complete staging ──────────────────────────────────────

-- The frontend's round trip: `prepare` stores the operation and the payload
-- to sign (90 s), `complete` claims it (pending -> submitted, 1 row or 409)
-- and an open-community answer stores a `read` ticket (at most 50 s) that the
-- history polls present. At most 10,000 live rows (429 past it); the sweep
-- deletes rows 5 minutes past expiry at most once a minute. `prepared` is
-- written once and read back whole. Goes away with the round trip if the
-- port makes it stateless.
CREATE TABLE chat.actions (
    id uuid NOT NULL,
    wallet text NOT NULL,
    operation text NOT NULL,
    prepared jsonb NOT NULL,
    state text NOT NULL DEFAULT 'pending',
    expires_at timestamp without time zone NOT NULL,
    created_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    CONSTRAINT actions_pkey PRIMARY KEY (id),
    CONSTRAINT actions_wallet_check CHECK (wallet ~ '^0x[0-9a-f]{40}$'),
    CONSTRAINT actions_state_check CHECK (state IN ('pending', 'submitted', 'read'))
);

-- The sweep: DELETE FROM chat.actions WHERE expires_at < $1.
CREATE INDEX actions_expires_at_idx ON chat.actions (expires_at);

-- ==== section: chat port ====
-- The port agent (src/social/chat) appends here: anything the route port
-- finds it needs beyond the catalogue above, each index with the query it
-- serves.
-- ==== end section: chat port ====
