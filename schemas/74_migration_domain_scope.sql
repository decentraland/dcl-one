-- Domain keys are explicit and immutable for each source generation. Places
-- retains its UUID catalogue scope; other adapters declare their typed key here.
CREATE TABLE migration_sync.domain_scope (
    stream text NOT NULL,
    generation uuid NOT NULL,
    key text NOT NULL CHECK (length(key) BETWEEN 1 AND 256),
    PRIMARY KEY (stream, generation, key),
    FOREIGN KEY (stream, generation) REFERENCES migration_sync.streams(name, generation)
);
