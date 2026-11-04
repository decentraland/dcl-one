-- Catalyst v2: meta (schema identity/version, database defaults) and the
-- content domain (entities, pointers, files, peer-sync state, local authority).
--
-- Derived from catalyrst/crates/catalyrst-server/migrations/0001..0007 as the
-- idempotent runner (src/schema_migrations.rs) applies them, collapsed into one
-- create: the content tables carry the 0005 (active_pointers.entity_type) and
-- 0006 (failed_deployments retry columns) additions inline, and the NOTIFY
-- trigger that bin/live/cache.rs installed at runtime lives here, because v2
-- startup never creates or resets objects.
--
-- Every timestamp column is `timestamp without time zone` holding UTC; the pool
-- pins TimeZone=UTC and search_path=content,meta at connect time, and the
-- database default below pins TimeZone=UTC for every other session too (psql
-- operators, backup/restore, the retained indexer roles), so no client can
-- misread them. The four columns the old migrations declared timestamptz
-- (local_entities.published_at/tombstoned_at, server_sync_cursors.updated_at,
-- failed_deployments.next_retry_at) follow that rule here; their defaults are
-- timezone('UTC', now()) instead of now().
--
-- Not carried over:
--   * 0007's deployments_auth_chain_consistent CHECK: it depends on the dcl_pg
--     extension (dcl_auth_chain_consistent(json, text, text)); add it with the
--     feature that needs it.
--   * the vestigial delta_pointer_result enum, translation_cache
--     (catalyrst-media), node-pg-migrate `migrations` and `_sqlx_migrations`.
--   * admin_audit: every console mutation records into the ONE
--     ops.admin_audit (schema/60_ops.sql).

DO $$ BEGIN
    EXECUTE format('ALTER DATABASE %I SET timezone = ' || quote_literal('UTC'), current_database());
END $$;

CREATE SCHEMA content;
CREATE SCHEMA meta;

CREATE TABLE meta.schema (
    id integer PRIMARY KEY CHECK (id = 1),
    identity text NOT NULL,
    version integer NOT NULL,
    applied_at timestamp without time zone NOT NULL
);
INSERT INTO meta.schema (id, identity, version, applied_at)
VALUES (1, 'catalyst', 2, timezone('UTC', now()));

-- The additive steps of schema/migrations/ this database has taken (or, for
-- a database created by `schema apply`, carries from birth: created_fresh),
-- by file name and the sha256 of the step text; `catalyst schema migrate`
-- applies the ones missing here, in name order, and refuses a recorded step
-- whose text changed.
CREATE TABLE meta.schema_steps (
    step text PRIMARY KEY,
    sha256 text NOT NULL CHECK (sha256 ~ '^[0-9a-f]{64}$'),
    created_fresh boolean NOT NULL,
    applied_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now())
);

-- ── content: entities, pointers, files ─────────────────────────────────────

CREATE SEQUENCE content.deployments_id_seq
    AS integer START WITH 1 INCREMENT BY 1 NO MINVALUE NO MAXVALUE CACHE 1;

CREATE TABLE content.deployments (
    id integer NOT NULL DEFAULT nextval('content.deployments_id_seq'::regclass),
    deployer_address text NOT NULL,
    version text NOT NULL,
    entity_type text NOT NULL,
    entity_id text NOT NULL,
    entity_metadata json,
    entity_timestamp timestamp without time zone NOT NULL,
    entity_pointers text[] NOT NULL,
    local_timestamp timestamp without time zone NOT NULL,
    auth_chain json NOT NULL,
    deleter_deployment integer,
    CONSTRAINT deployments_pkey PRIMARY KEY (id),
    CONSTRAINT deployments_entity_id_key UNIQUE (entity_id)
);
ALTER SEQUENCE content.deployments_id_seq OWNED BY content.deployments.id;

CREATE TABLE content.content_files (
    deployment integer NOT NULL,
    content_hash text NOT NULL,
    key text NOT NULL,
    CONSTRAINT content_files_uniq_deployment_key UNIQUE (deployment, key)
);

CREATE TABLE content.active_pointers (
    pointer character varying NOT NULL,
    entity_id text NOT NULL,
    entity_type text,
    CONSTRAINT active_pointers_pkey PRIMARY KEY (pointer)
);

-- ── content: sync state ────────────────────────────────────────────────────

-- retry_count holds the attempts already spent, next_retry_at the earliest
-- instant the entry may be tried again: the worker selects the due entries in
-- (next_retry_at, entity_id) order in bounded pages, and evicts the ones that
-- reached the retry cap once they are due.
CREATE TABLE content.failed_deployments (
    entity_id text NOT NULL,
    entity_type text NOT NULL,
    failure_time timestamp without time zone NOT NULL,
    reason text NOT NULL,
    auth_chain json NOT NULL,
    error_description text NOT NULL,
    snapshot_hash text NOT NULL,
    retry_count integer NOT NULL DEFAULT 0,
    next_retry_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    CONSTRAINT failed_deployments_pkey PRIMARY KEY (entity_id)
);

CREATE TABLE content.processed_snapshots (
    hash text NOT NULL,
    process_time timestamp without time zone NOT NULL,
    CONSTRAINT processed_snapshots_pkey PRIMARY KEY (hash)
);

CREATE TABLE content.snapshots (
    hash text,
    init_timestamp timestamp without time zone NOT NULL,
    end_timestamp timestamp without time zone NOT NULL,
    replaced_hashes text[] NOT NULL,
    number_of_entities integer NOT NULL,
    generation_time timestamp without time zone NOT NULL,
    CONSTRAINT snapshots_pkey PRIMARY KEY (init_timestamp, end_timestamp)
);

CREATE TABLE content.system_properties (
    key text NOT NULL,
    value text NOT NULL,
    CONSTRAINT system_properties_pkey PRIMARY KEY (key)
);

-- Per-peer confirmed resume point, advanced GREATEST-monotonically at the same
-- durable points as the global 'sync_frontier' system property. Rows outlive
-- server-set changes on purpose: re-adding a peer resumes from its own cursor.
CREATE TABLE content.server_sync_cursors (
    server_url text NOT NULL,
    cursor_ms bigint NOT NULL,
    updated_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    CONSTRAINT server_sync_cursors_pkey PRIMARY KEY (server_url)
);

-- ── content: local authority and audit ─────────────────────────────────────

-- Provenance for entities published directly against this node's write API,
-- as opposed to rows mirrored from peers by sync. Additive: the shared tables
-- above are untouched.
CREATE TABLE content.local_entities (
    entity_id text PRIMARY KEY,
    signer text NOT NULL,
    origin text NOT NULL DEFAULT 'land-publish',
    published_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    tombstoned_at timestamp without time zone
);

-- The operator's content denylist (src/content/denylist.rs): entity ids,
-- content hashes and pointers this node refuses to serve (the reads drop a
-- listed entity, /contents/{hash} answers 404) or to accept in a local
-- deploy. The in-process projection (content::Denylist) is loaded from here
-- at boot, after every console edit and every 60 s. Local authority, edited
-- only through the console (OPS_WRITES_ENABLED); id is compared exactly,
-- a pointer case-insensitively.
CREATE TABLE content.denylist (
    id text NOT NULL,
    added_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    added_by text NOT NULL,
    CONSTRAINT denylist_pkey PRIMARY KEY (id),
    CONSTRAINT denylist_id_check CHECK (id <> '' AND octet_length(id) <= 512)
);

-- The upstream forwarding outbox (src/content/forward.rs): one row per local
-- deploy (`POST /entities`) this node sends on, unchanged, to
-- CONTENT_FORWARD_UPSTREAM_URL. The deploy statement inserts it beside the
-- deployment (write_deployer.rs wraps the deploy function), so a committed
-- local deployment has its row and a refused one never does. The leased
-- `content-forward` worker claims one due row at a time (queued, or sending
-- past its send lease), sends after the claim committed and records the
-- outcome; `attempts` fences a late finish. `target` is the upstream base the
-- row was queued for and a worker sends only its own target's rows; another
-- target's open rows are closed `failed` once no upstream takes their entity
-- (created over 35 minutes ago: the upstream's 20-minute age window plus the
-- 15 minutes a timestamp may lead). Terminal rows (accepted, rejected,
-- failed, superseded) are deleted 30 days after their last update. No FK to
-- content.deployments: the row is the forwarding record and outlives a
-- deployment an operator removes; the worker reads a missing, overwritten or
-- unpublished (tombstoned) deployment as superseded and never sends it. The
-- operator's requeue (`POST /content/forwarding/{entityId}/retry`) puts a
-- queued, rejected or failed row back in the queue while its entity is inside
-- the upstream's window.
CREATE TABLE content.forward_outbox (
    id bigint GENERATED ALWAYS AS IDENTITY,
    entity_id text NOT NULL,
    target text NOT NULL,
    status text NOT NULL DEFAULT 'queued',
    attempts integer NOT NULL DEFAULT 0,
    next_attempt_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    last_error text,
    upstream_status smallint,
    created_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    updated_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    CONSTRAINT forward_outbox_pkey PRIMARY KEY (id),
    CONSTRAINT forward_outbox_entity_id_key UNIQUE (entity_id),
    CONSTRAINT forward_outbox_status_check
        CHECK (status IN ('queued', 'sending', 'accepted', 'rejected', 'failed', 'superseded')),
    CONSTRAINT forward_outbox_attempts_check CHECK (attempts >= 0),
    CONSTRAINT forward_outbox_upstream_status_check CHECK (upstream_status BETWEEN 100 AND 599)
);

-- ── content: indexes (the upstream content-server / node-pg-migrate set) ───

CREATE INDEX active_pointers_entity_id_idx ON content.active_pointers USING btree (entity_id);
CREATE INDEX active_pointers_pointer_ops_idx ON content.active_pointers USING btree (pointer varchar_pattern_ops);
CREATE INDEX active_pointers_entity_type_idx ON content.active_pointers USING btree (entity_type);
CREATE INDEX content_files_content_hash_index ON content.content_files USING btree (content_hash);
CREATE INDEX deployer_address_lower_case ON content.deployments USING btree (lower(deployer_address) text_pattern_ops);
CREATE INDEX deployments_deployer_addr_ts_idx ON content.deployments USING btree (deployer_address, entity_timestamp DESC);
CREATE INDEX deployments_entity_pointers_index ON content.deployments USING gin (entity_pointers);
CREATE INDEX deployments_entity_timestamp_entity_id_idx ON content.deployments USING btree (entity_timestamp DESC, entity_id DESC);
CREATE INDEX deployments_entity_type_index ON content.deployments USING btree (entity_type);
CREATE INDEX deployments_full_snapshots_ix ON content.deployments USING btree (deleter_deployment DESC, local_timestamp) INCLUDE (entity_type);
CREATE INDEX deployments_local_timestamp_lower_entity_id_idx ON content.deployments USING btree (local_timestamp DESC, lower(entity_id) DESC);
CREATE INDEX deployments_type_entity_ts_idx ON content.deployments USING btree (entity_type, entity_timestamp DESC, entity_id DESC);
CREATE INDEX deployments_type_local_ts_idx ON content.deployments USING btree (entity_type, local_timestamp DESC, lower(entity_id) DESC);
-- The retry worker's due page (next_retry_at <= $now AND retry_count < $max ORDER BY
-- next_retry_at, entity_id LIMIT $n, keyset-continued on the same pair), its give-up
-- DELETE over the same due prefix, and the waiting count (next_retry_at > $now).
-- $now is a query parameter, never an index predicate.
CREATE INDEX failed_deployments_due_idx ON content.failed_deployments USING btree (next_retry_at, entity_id);
CREATE INDEX local_entities_tombstoned_idx ON content.local_entities (tombstoned_at) WHERE tombstoned_at IS NOT NULL;
-- The forwarding worker's claim (status IN ('queued', 'sending') AND target =
-- $t AND next_attempt_at <= $now ORDER BY next_attempt_at, id LIMIT 1 FOR
-- UPDATE SKIP LOCKED): only the open rows are indexed, so the index stays the
-- size of the backlog while terminal rows accumulate, and `target` leads, so
-- a claim never walks the rows queued for another upstream.
CREATE INDEX forward_outbox_due_idx ON content.forward_outbox USING btree (target, next_attempt_at, id)
    WHERE status IN ('queued', 'sending');

-- ── content: cache invalidation ────────────────────────────────────────────

-- Every new deployment row wakes the in-process entity cache listener.
CREATE FUNCTION content.notify_new_deployment() RETURNS trigger AS $$
BEGIN
    PERFORM pg_notify('new_deployment', NEW.entity_type || ':' || NEW.entity_id);
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER deployment_notify_trigger
    AFTER INSERT ON content.deployments
    FOR EACH ROW EXECUTE FUNCTION content.notify_new_deployment();

-- ── content: the local write path's functions ──────────────────────────────

-- The deploy function (src/content/write_deployer.rs, one variant per
-- provenance: a local scene also writes content.local_entities) and the
-- unpublish function (src/content/land_publish.rs). Each is named by a hash
-- of its body: the binary computes the name from its own SQL and checks it
-- exists before its first call, so a binary never runs SQL it was not built
-- with and two binaries mid-rollout call their own functions. They live here
-- because the runtime role cannot create them. GENERATED TEXT: the lib test
-- `content::land_publish::tests::the_schema_carries_the_write_path_functions`
-- fails with the block to paste when the Rust SQL changes; an existing
-- database takes the new functions from `catalyst schema functions` (the
-- CREATE OR REPLACE form) run through psql as the administrative role.

CREATE FUNCTION content.deploy_entity_218dd4c4d67c(text, text, text, text, json, double precision, text[], json, text[], text[], text) RETURNS TABLE (dep_id integer, newer boolean, overwrote integer, cleared integer) LANGUAGE plpgsql AS $deploy$ #variable_conflict use_column
BEGIN
PERFORM pg_advisory_xact_lock(hashtext(k)) FROM (SELECT k FROM (SELECT DISTINCT unnest($7) AS k) d ORDER BY k COLLATE "C") locks;
RETURN QUERY
        WITH newer AS (
            SELECT 1 AS present FROM content.deployments
            WHERE entity_type = $3
              AND entity_pointers && $7
              AND (entity_timestamp > to_timestamp($6 / 1000.0)
                   OR (entity_timestamp = to_timestamp($6 / 1000.0) AND lower(entity_id) > lower($4)))
            LIMIT 1
        ),
        overwrote AS (
            SELECT dep1.id, dep1.entity_pointers
            FROM content.deployments AS dep1
            LEFT JOIN content.deployments AS dep2 ON dep1.deleter_deployment = dep2.id
            WHERE dep1.entity_type = $3
              AND dep1.entity_pointers && $7
              AND (dep1.entity_timestamp < to_timestamp($6 / 1000.0)
                   OR (dep1.entity_timestamp = to_timestamp($6 / 1000.0) AND lower(dep1.entity_id) < lower($4)))
              AND (dep2.id IS NULL
                   OR dep2.entity_timestamp > to_timestamp($6 / 1000.0)
                   OR (dep2.entity_timestamp = to_timestamp($6 / 1000.0) AND lower(dep2.entity_id) > lower($4)))
        ),
        ins AS (
            INSERT INTO content.deployments
                (deployer_address, version, entity_type, entity_id, entity_metadata,
                 entity_timestamp, entity_pointers, local_timestamp, auth_chain)
            SELECT $1, $2, $3, $4, $5, to_timestamp($6 / 1000.0), $7, now(), $8
            WHERE NOT EXISTS (SELECT 1 FROM newer)
            ON CONFLICT (entity_id) DO NOTHING
            RETURNING id
        ),
        files AS (
            INSERT INTO content.content_files (deployment, content_hash, key)
            SELECT ins.id, c.hash, c.key
            FROM ins, unnest($9::text[], $10::text[]) AS c(hash, key)
        ),
        pointed AS (
            INSERT INTO content.active_pointers (pointer, entity_id, entity_type)
            SELECT p.pointer, $4, $3 FROM ins, unnest($7::text[]) AS p(pointer)
            ON CONFLICT (pointer) DO UPDATE
                SET entity_id = EXCLUDED.entity_id, entity_type = EXCLUDED.entity_type
                WHERE NOT EXISTS (
                    SELECT 1 FROM content.deployments cur
                    WHERE cur.entity_id = active_pointers.entity_id
                      AND (cur.entity_timestamp > to_timestamp($6 / 1000.0)
                           OR (cur.entity_timestamp = to_timestamp($6 / 1000.0)
                               AND lower(cur.entity_id) > lower(EXCLUDED.entity_id)))
                )
        ),
        cleared AS (
            DELETE FROM content.active_pointers
            WHERE EXISTS (SELECT 1 FROM ins)
              AND pointer IN (SELECT unnest(entity_pointers) FROM overwrote)
              AND NOT (pointer = ANY($7))
            RETURNING pointer
        ),
        overwritten AS (
            UPDATE content.deployments SET deleter_deployment = ins.id
            FROM ins
            WHERE deployments.id IN (SELECT id FROM overwrote)
            RETURNING deployments.id
        )
        SELECT (SELECT id FROM ins) AS dep_id,
               EXISTS (SELECT 1 FROM newer) AS newer,
               (SELECT count(*) FROM overwritten)::int AS overwrote,
               (SELECT count(*) FROM cleared)::int AS cleared
        ;
END $deploy$;

CREATE FUNCTION content.deploy_entity_66918b614b85(text, text, text, text, json, double precision, text[], json, text[], text[], text) RETURNS TABLE (dep_id integer, newer boolean, overwrote integer, cleared integer) LANGUAGE plpgsql AS $deploy$ #variable_conflict use_column
BEGIN
PERFORM pg_advisory_xact_lock(hashtext(k)) FROM (SELECT k FROM (SELECT DISTINCT unnest($7) AS k) d ORDER BY k COLLATE "C") locks;
RETURN QUERY
        WITH newer AS (
            SELECT 1 AS present FROM content.deployments
            WHERE entity_type = $3
              AND entity_pointers && $7
              AND (entity_timestamp > to_timestamp($6 / 1000.0)
                   OR (entity_timestamp = to_timestamp($6 / 1000.0) AND lower(entity_id) > lower($4)))
            LIMIT 1
        ),
        overwrote AS (
            SELECT dep1.id, dep1.entity_pointers
            FROM content.deployments AS dep1
            LEFT JOIN content.deployments AS dep2 ON dep1.deleter_deployment = dep2.id
            WHERE dep1.entity_type = $3
              AND dep1.entity_pointers && $7
              AND (dep1.entity_timestamp < to_timestamp($6 / 1000.0)
                   OR (dep1.entity_timestamp = to_timestamp($6 / 1000.0) AND lower(dep1.entity_id) < lower($4)))
              AND (dep2.id IS NULL
                   OR dep2.entity_timestamp > to_timestamp($6 / 1000.0)
                   OR (dep2.entity_timestamp = to_timestamp($6 / 1000.0) AND lower(dep2.entity_id) > lower($4)))
        ),
        ins AS (
            INSERT INTO content.deployments
                (deployer_address, version, entity_type, entity_id, entity_metadata,
                 entity_timestamp, entity_pointers, local_timestamp, auth_chain)
            SELECT $1, $2, $3, $4, $5, to_timestamp($6 / 1000.0), $7, now(), $8
            WHERE NOT EXISTS (SELECT 1 FROM newer)
            ON CONFLICT (entity_id) DO NOTHING
            RETURNING id
        ),
        provenance AS (
            INSERT INTO content.local_entities (entity_id, signer)
            SELECT $4, $11 FROM ins
            ON CONFLICT (entity_id) DO NOTHING
        ),
        files AS (
            INSERT INTO content.content_files (deployment, content_hash, key)
            SELECT ins.id, c.hash, c.key
            FROM ins, unnest($9::text[], $10::text[]) AS c(hash, key)
        ),
        pointed AS (
            INSERT INTO content.active_pointers (pointer, entity_id, entity_type)
            SELECT p.pointer, $4, $3 FROM ins, unnest($7::text[]) AS p(pointer)
            ON CONFLICT (pointer) DO UPDATE
                SET entity_id = EXCLUDED.entity_id, entity_type = EXCLUDED.entity_type
                WHERE NOT EXISTS (
                    SELECT 1 FROM content.deployments cur
                    WHERE cur.entity_id = active_pointers.entity_id
                      AND (cur.entity_timestamp > to_timestamp($6 / 1000.0)
                           OR (cur.entity_timestamp = to_timestamp($6 / 1000.0)
                               AND lower(cur.entity_id) > lower(EXCLUDED.entity_id)))
                )
        ),
        cleared AS (
            DELETE FROM content.active_pointers
            WHERE EXISTS (SELECT 1 FROM ins)
              AND pointer IN (SELECT unnest(entity_pointers) FROM overwrote)
              AND NOT (pointer = ANY($7))
            RETURNING pointer
        ),
        overwritten AS (
            UPDATE content.deployments SET deleter_deployment = ins.id
            FROM ins
            WHERE deployments.id IN (SELECT id FROM overwrote)
            RETURNING deployments.id
        )
        SELECT (SELECT id FROM ins) AS dep_id,
               EXISTS (SELECT 1 FROM newer) AS newer,
               (SELECT count(*) FROM overwritten)::int AS overwrote,
               (SELECT count(*) FROM cleared)::int AS cleared
        ;
END $deploy$;

CREATE FUNCTION content.unpublish_scene_be255dae6005(text) RETURNS TABLE (entity_id text, tombstoned bigint, pointer text, replacement text) LANGUAGE plpgsql AS $unpublish$ #variable_conflict use_column
DECLARE v_entity_id text; v_pointers text[];
BEGIN
SELECT d.entity_id, d.entity_pointers INTO v_entity_id, v_pointers
  FROM content.active_pointers ap
  JOIN content.deployments d ON d.entity_id = ap.entity_id
  JOIN content.local_entities le ON le.entity_id = d.entity_id AND le.tombstoned_at IS NULL
 WHERE ap.pointer = $1 AND d.entity_type = 'scene';
IF NOT FOUND THEN RETURN; END IF;
PERFORM pg_advisory_xact_lock(hashtext(k)) FROM (SELECT k FROM (SELECT DISTINCT unnest(v_pointers) AS k) d ORDER BY k COLLATE "C") locks;
RETURN QUERY
WITH cur AS (
    SELECT d.id
    FROM content.active_pointers ap
    JOIN content.deployments d ON d.entity_id = ap.entity_id
    JOIN content.local_entities le ON le.entity_id = d.entity_id AND le.tombstoned_at IS NULL
    WHERE ap.pointer = $1 AND d.entity_type = 'scene' AND d.entity_id = v_entity_id
), tomb AS (
    UPDATE content.local_entities SET tombstoned_at = timezone('UTC', now())
    WHERE entity_id = v_entity_id AND tombstoned_at IS NULL AND EXISTS (SELECT 1 FROM cur)
    RETURNING entity_id
), restored AS (
    UPDATE content.deployments SET deleter_deployment = NULL
    WHERE deleter_deployment = (SELECT id FROM cur) AND EXISTS (SELECT 1 FROM tomb)
    RETURNING id
), held AS (
    SELECT ap.pointer
    FROM content.active_pointers ap
    WHERE ap.entity_id = v_entity_id AND EXISTS (SELECT 1 FROM tomb)
), candidates AS MATERIALIZED (
    SELECT d.entity_id, d.entity_pointers, d.entity_timestamp
    FROM content.deployments d
    WHERE d.entity_pointers && v_pointers
      AND d.entity_type = 'scene'
      AND d.entity_id <> v_entity_id
      AND (d.deleter_deployment IS NULL OR d.deleter_deployment = (SELECT id FROM cur))
      AND NOT EXISTS (
          SELECT 1 FROM content.local_entities le
          WHERE le.entity_id = d.entity_id AND le.tombstoned_at IS NOT NULL)
      AND EXISTS (SELECT 1 FROM tomb)
), replacement AS (
    SELECT DISTINCT ON (h.pointer) h.pointer, c.entity_id
    FROM held h
    JOIN candidates c ON c.entity_pointers @> ARRAY[h.pointer::text]
    ORDER BY h.pointer, c.entity_timestamp DESC, lower(c.entity_id) DESC
), repointed AS (
    UPDATE content.active_pointers ap SET entity_id = r.entity_id, entity_type = 'scene'
    FROM replacement r
    WHERE ap.pointer = r.pointer
    RETURNING ap.pointer
), removed AS (
    DELETE FROM content.active_pointers ap
    WHERE ap.entity_id = v_entity_id
      AND ap.pointer IN (SELECT pointer FROM held)
      AND NOT EXISTS (SELECT 1 FROM replacement r WHERE r.pointer = ap.pointer)
    RETURNING ap.pointer
), notified AS (
    SELECT pg_notify('new_deployment', 'scene:' || v_entity_id) AS sent
)
SELECT v_entity_id AS entity_id,
       (SELECT count(*) FROM tomb) AS tombstoned,
       h.pointer::text AS pointer,
       r.entity_id AS replacement
FROM (SELECT 1) AS marker
CROSS JOIN notified
LEFT JOIN held h ON true
LEFT JOIN replacement r ON r.pointer = h.pointer
ORDER BY h.pointer;
END $unpublish$;
