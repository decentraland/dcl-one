-- Social START notification facts are keyed by the source's exact Redis key.
-- UUID spelling aliases share media/membership but retain independent audiences,
-- generations and 24-hour expirations. This table definition is also included
-- verbatim by the reviewed, transactional schema-version-3 release.
CREATE TABLE social.community_voice_notifications (
    raw_community_id text COLLATE "C" NOT NULL,
    community_id uuid NOT NULL REFERENCES social.communities(id) ON DELETE CASCADE,
    notification_scope text NOT NULL,
    generation uuid NOT NULL,
    expires_at timestamp without time zone NOT NULL,
    CONSTRAINT community_voice_notifications_pkey PRIMARY KEY (raw_community_id),
    CONSTRAINT community_voice_notifications_identity_check
        CHECK (lower(raw_community_id) = community_id::text),
    CONSTRAINT community_voice_notifications_scope_check
        CHECK (notification_scope IN ('all', 'members'))
);
