-- Each stream has one source owner; deleted facts retain their ordered history.
CREATE TABLE migration_sync.domain_versions (
    stream text NOT NULL,
    generation uuid NOT NULL,
    key text NOT NULL,
    commit_lsn pg_lsn NOT NULL,
    deleted boolean NOT NULL,
    PRIMARY KEY (stream, generation, key),
    FOREIGN KEY (stream, generation, commit_lsn) REFERENCES migration_sync.inbox(stream, generation, commit_lsn)
);
