-- Scoped catalogue history survives physical removals and cannot collide with
-- either favorite or like keys. Pending external cleanup remains Explore-owned.
CREATE TABLE migration_sync.catalogue_versions (
    stream text NOT NULL,
    generation uuid NOT NULL,
    entity_id text NOT NULL,
    commit_lsn pg_lsn NOT NULL,
    deleted boolean NOT NULL,
    PRIMARY KEY(stream,generation,entity_id),
    FOREIGN KEY(stream,generation,commit_lsn) REFERENCES migration_sync.inbox(stream,generation,commit_lsn)
);
