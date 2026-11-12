-- Likes and favorites share source ordering but retain independent key history.
-- A like tombstone must never overwrite a favorite version for the same pair.
CREATE TABLE migration_sync.like_versions (
    stream text NOT NULL,
    generation uuid NOT NULL,
    "user" text NOT NULL CHECK ("user" ~ '^0x[0-9a-f]{40}$'),
    entity_id text NOT NULL,
    commit_lsn pg_lsn NOT NULL,
    deleted boolean NOT NULL,
    PRIMARY KEY (stream, generation, "user", entity_id),
    FOREIGN KEY (stream, generation, commit_lsn)
        REFERENCES migration_sync.inbox (stream, generation, commit_lsn)
);
