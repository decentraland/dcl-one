-- Catalyst v2: current-state DDL for an empty `catalyst` database.
-- Design/review contract: DATABASE.md. The inherited indexes below are a
-- baseline; query/layout changes require representative workload evidence.
--
-- RENDERED FILE. The source is schema/*.sql, concatenated in name order by
-- build.rs (one file per domain: 00 content + meta, 10 explore, 20 worlds,
-- 30 marketplace, 40 social, 50 create: builder + camera_reel). Edit those;
-- regenerate this copy with
-- `catalyst schema print > schema.sql` (a unit test fails when it drifts).
--
-- Applied once, by an operator, with `catalyst schema apply` (or psql -f). No
-- IF NOT EXISTS: a second apply on a populated database must fail, not no-op.

-- ════ schema/00_content.sql ════

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
VALUES (1, 'catalyst', 1, timezone('UTC', now()));

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

-- ════ schema/10_explore.sql ════

-- Catalyst v2: explore domain: places, worlds catalogue, events, map and the
-- explorer-facing routes (src/explore).
--
-- Derived from catalyrst/crates/catalyrst-places/migrations/0000..0008 and
-- catalyrst/crates/catalyrst-events/migrations/0001..0003 as the old services
-- applied them, plus the tables catalyrst-places created at runtime
-- (PlacesComponent::ensure_local_schema: user_favorites, user_likes, seen_nonces),
-- collapsed into one create under the `explore` schema. Not carried over:
-- place_reports_local, pois, signed_actions_places, signed_actions_events,
-- event_profile_settings (stage 5 writes), the place_raw_positions_gin index
-- (only the retired content-derive sync read it) and the grant-replay DO blocks.
--
-- Authority: `explore.place` and `explore.event` are MIRRORS of the upstream
-- catalogues (src/explore/places/mirror.rs, src/explore/events/mirror.rs);
-- `explore.place_world_local`, `explore.events_local`,
-- `explore.event_attendance_local`, `explore.schedules_local`,
-- `explore.user_favorites` and `explore.user_likes` are LOCAL authority. The
-- mirrors never write a *_local table, the event mirror skips rows that carry an
-- events_local overlay, and the read view `explore.place_indexed` is the union of
-- the mirrored leg and the local leg. `explore.mirror_state` holds each mirror's
-- in-flight cursor and its last complete pass, so a restart resumes and
-- /api/status can say how fresh the catalogue is.
--
-- Every timestamp column is `timestamp without time zone` holding UTC (the pool
-- pins TimeZone=UTC); the old migrations declared them timestamptz.

CREATE SCHEMA explore;

-- ── places and worlds catalogue ────────────────────────────────────────────

-- The mirrored leg. world/world_name are GENERATED from raw so an unnamed
-- upstream row reads as a place, never as a nameless world (0003);
-- description_plain and image_url are the write-time half of the read-side
-- sanitizer (0004): description_plain runs as many strip passes as
-- src/explore/sanitize.rs (MAX_SANITIZE_PASSES = 5) and then drops every angle
-- bracket, and query.rs pins its expression byte for byte.
CREATE TABLE explore.place (
    id              text PRIMARY KEY,
    base_position   text NOT NULL,
    title           text,
    description     text,
    creator_address text,
    content_rating  text,
    categories      text[] NOT NULL DEFAULT '{}',
    likes           integer NOT NULL DEFAULT 0,
    dislikes        integer NOT NULL DEFAULT 0,
    favorites       integer NOT NULL DEFAULT 0,
    deployed_at     timestamp without time zone,
    disabled        boolean NOT NULL DEFAULT false,
    highlighted     boolean NOT NULL DEFAULT false,
    raw             jsonb NOT NULL,
    fetched_at      timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    world_name      text GENERATED ALWAYS AS (NULLIF(btrim(raw->>'world_name'), '')) STORED,
    world           boolean NOT NULL GENERATED ALWAYS AS (
        CASE lower(btrim(raw->>'world'))
            WHEN 'true' THEN true
            WHEN 't' THEN true
            WHEN 'yes' THEN true
            WHEN 'on' THEN true
            WHEN '1' THEN true
            ELSE false
        END
        AND NULLIF(btrim(raw->>'world_name'), '') IS NOT NULL
    ) STORED,
    description_plain text GENERATED ALWAYS AS (
        CASE WHEN strpos(coalesce(description, ''), '<') = 0
            AND strpos(coalesce(description, ''), '>') = 0
            THEN coalesce(description, '')
            ELSE regexp_replace(regexp_replace(regexp_replace(regexp_replace(regexp_replace(regexp_replace(coalesce(description, ''),
                '</?[a-zA-Z][^>]*>', '', 'g'),
                '</?[a-zA-Z][^>]*>', '', 'g'),
                '</?[a-zA-Z][^>]*>', '', 'g'),
                '</?[a-zA-Z][^>]*>', '', 'g'),
                '</?[a-zA-Z][^>]*>', '', 'g'),
                '[<>]', '', 'g')
        END
    ) STORED,
    image_url       text GENERATED ALWAYS AS (
        CASE WHEN raw->>'image' ~* '^https?://[^[:space:]"''`<>]+$'
             THEN raw->>'image'
        END
    ) STORED
);

CREATE INDEX place_world_name_col_idx ON explore.place (lower(world_name));
CREATE INDEX place_fetched_at_idx ON explore.place (fetched_at);
CREATE INDEX place_positions_path_gin ON explore.place USING gin ((raw->'positions') jsonb_path_ops);

-- No index on base_position: the destinations feed probes road_positions by
-- it (rp.position = base_position, served by road_positions_pkey), the ranking
-- CASE base_position WHEN is not sargable and PARCEL_ALIAS_SQL reads the leg;
-- the old place_base_position_idx served no statement (metrics/schema-review.md).

-- like_score ordering, one partial index per leg the list query cuts to a page;
-- PlaceOrderBy::LikeScore.column() (src/explore/places/query.rs raw_float8_sql)
-- must stay byte-identical to this expression or the planner ignores it.
CREATE INDEX place_like_score_guarded_place_only_idx
    ON explore.place ((CASE WHEN jsonb_typeof(raw->'like_score') IN ('number', 'string') AND raw->>'like_score' ~ '^-?(([0-9]{1,255}|[0-9]{100}[0-9]{1,208})(\.([0-9]{1,255}|[0-9]{100}[0-9]{1,223}))?|[0-9]{1,3}(\.[0-9]{1,17})?[eE][+-]?([0-9]{1,2}|[12][0-9]{2}|30[0-5])|[0-9](\.[0-9]{1,17})?[eE][+-]?30[6-7]|1(\.(7[0-9]?|[0-6][0-9]{0,16}))?[eE][+-]?308)$' THEN (raw->>'like_score')::float8 END) DESC NULLS LAST, deployed_at DESC)
    WHERE disabled IS FALSE AND world IS FALSE;

CREATE INDEX place_like_score_guarded_world_only_idx
    ON explore.place ((CASE WHEN jsonb_typeof(raw->'like_score') IN ('number', 'string') AND raw->>'like_score' ~ '^-?(([0-9]{1,255}|[0-9]{100}[0-9]{1,208})(\.([0-9]{1,255}|[0-9]{100}[0-9]{1,223}))?|[0-9]{1,3}(\.[0-9]{1,17})?[eE][+-]?([0-9]{1,2}|[12][0-9]{2}|30[0-5])|[0-9](\.[0-9]{1,17})?[eE][+-]?30[6-7]|1(\.(7[0-9]?|[0-6][0-9]{0,16}))?[eE][+-]?308)$' THEN (raw->>'like_score')::float8 END) DESC NULLS LAST, deployed_at DESC)
    WHERE disabled IS FALSE AND world IS TRUE;

-- The local leg: worlds this node serves itself. Plain world/world_name columns
-- plus a CHECK, so an unnamed world is rejected at write time. Same column
-- order as explore.place; place_indexed lists the columns explicitly anyway.
CREATE TABLE explore.place_world_local (
    id              text PRIMARY KEY,
    base_position   text NOT NULL,
    title           text,
    description     text,
    creator_address text,
    content_rating  text,
    categories      text[] NOT NULL DEFAULT '{}',
    likes           integer NOT NULL DEFAULT 0,
    dislikes        integer NOT NULL DEFAULT 0,
    favorites       integer NOT NULL DEFAULT 0,
    deployed_at     timestamp without time zone,
    disabled        boolean NOT NULL DEFAULT false,
    highlighted     boolean NOT NULL DEFAULT false,
    raw             jsonb NOT NULL,
    fetched_at      timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    world_name      text,
    world           boolean NOT NULL DEFAULT false,
    description_plain text GENERATED ALWAYS AS (
        CASE WHEN strpos(coalesce(description, ''), '<') = 0
            AND strpos(coalesce(description, ''), '>') = 0
            THEN coalesce(description, '')
            ELSE regexp_replace(regexp_replace(regexp_replace(regexp_replace(regexp_replace(regexp_replace(coalesce(description, ''),
                '</?[a-zA-Z][^>]*>', '', 'g'),
                '</?[a-zA-Z][^>]*>', '', 'g'),
                '</?[a-zA-Z][^>]*>', '', 'g'),
                '</?[a-zA-Z][^>]*>', '', 'g'),
                '</?[a-zA-Z][^>]*>', '', 'g'),
                '[<>]', '', 'g')
        END
    ) STORED,
    image_url       text GENERATED ALWAYS AS (
        CASE WHEN raw->>'image' ~* '^https?://[^[:space:]"''`<>]+$'
             THEN raw->>'image'
        END
    ) STORED,
    CONSTRAINT place_world_local_world_named
        CHECK (world IS FALSE OR (world_name IS NOT NULL AND btrim(world_name) <> ''))
);

CREATE INDEX place_world_local_world_name_col_idx ON explore.place_world_local (lower(world_name));
CREATE INDEX place_world_local_positions_path_gin
    ON explore.place_world_local USING gin ((raw->'positions') jsonb_path_ops);

-- The read surface for every place/world query: the mirrored leg and the local leg.
CREATE VIEW explore.place_indexed AS
    SELECT id, base_position, title, description, creator_address, content_rating,
           categories, likes, dislikes, favorites, deployed_at, disabled, highlighted,
           raw, fetched_at, world_name, world, description_plain, image_url
    FROM explore.place
    UNION ALL
    SELECT id, base_position, title, description, creator_address, content_rating,
           categories, likes, dislikes, favorites, deployed_at, disabled, highlighted,
           raw, fetched_at, world_name, world, description_plain, image_url
    FROM explore.place_world_local;

-- Operator moderation of a catalogue row (src/explore/places/moderation.rs;
-- the console's places/{place-disable,place-highlight,place-rating,
-- world-highlight,world-rating}): local authority over a mirrored or local
-- place, one nullable field per decision (NULL: no override, upstream's value
-- shows). The writer projects a decision onto explore.place /
-- explore.place_world_local in the same transaction, and the places mirror's
-- upsert re-applies the non-NULL fields to every row it writes, so a pass
-- never undoes an operator. No FK to explore.place: the sweep may delete a
-- place upstream dropped, and the decision stays for its return. The PK
-- serves the mirror's per-row probe and the writer's upsert.
CREATE TABLE explore.place_moderation (
    place_id        text PRIMARY KEY,
    disabled        boolean,
    highlighted     boolean,
    content_rating  text,
    reason          text,
    set_by          text NOT NULL,
    updated_at      timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    CONSTRAINT place_moderation_decided
        CHECK (disabled IS NOT NULL OR highlighted IS NOT NULL OR content_rating IS NOT NULL),
    CONSTRAINT place_moderation_rating_known
        CHECK (content_rating IS NULL OR content_rating IN ('RP', 'E', 'T', 'A', 'R'))
);

-- Viewer interactions (local authority; stage 5 opens the writes). The reads
-- fold them into the page statement (viewer_columns in src/explore/places/rows.rs).
CREATE TABLE explore.user_favorites (
    "user"        text NOT NULL,
    entity_id     text NOT NULL,
    user_activity double precision NOT NULL DEFAULT 0,
    created_at    timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    PRIMARY KEY ("user", entity_id)
);
CREATE INDEX user_favorites_entity_idx ON explore.user_favorites (entity_id);

CREATE TABLE explore.user_likes (
    "user"        text NOT NULL,
    entity_id     text NOT NULL,
    "like"        boolean NOT NULL,
    user_activity double precision NOT NULL DEFAULT 0,
    created_at    timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    updated_at    timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    PRIMARY KEY ("user", entity_id)
);
CREATE INDEX user_likes_entity_idx ON explore.user_likes (entity_id);

-- ── events ─────────────────────────────────────────────────────────────────

-- The mirrored leg of /api/events (src/explore/events/mirror.rs).
CREATE TABLE explore.event (
    id              text PRIMARY KEY,
    name            text NOT NULL,
    start_at        timestamp without time zone,
    finish_at       timestamp without time zone,
    next_start_at   timestamp without time zone,
    next_finish_at  timestamp without time zone,
    duration_ms     bigint,
    recurrent       boolean NOT NULL DEFAULT false,
    highlighted     boolean NOT NULL DEFAULT false,
    trending        boolean NOT NULL DEFAULT false,
    approved        boolean NOT NULL DEFAULT false,
    attending       boolean,
    community_id    text,
    user_creator    text,
    coordinates_x   integer,
    coordinates_y   integer,
    description     text,
    raw             jsonb NOT NULL,
    fetched_at      timestamp without time zone NOT NULL DEFAULT timezone('UTC', now())
);

-- (coordinates_x = $x AND coordinates_y = $y) destination clauses of the list.
CREATE INDEX event_coords_idx ON explore.event (coordinates_x, coordinates_y);
-- The admin `approved=false` filter and the `approved IS TRUE` prefix of every
-- public list; next_finish_at is not in any predicate (the lists compute the
-- effective finish from raw->'recurrent_dates', EFF_NEXT_FINISH_SQL) and rides
-- along from the old schema. No index on next_start_at: the one statement that
-- orders by it (moderation_pending, DESC NULLS LAST) cannot use a btree in
-- either direction, so the old event_next_start_idx served nothing.
CREATE INDEX event_approved_live_idx ON explore.event (approved, next_finish_at);
-- The sweep's fetched_at < $1 candidates and its ORDER BY fetched_at confirmation set.
CREATE INDEX event_fetched_at_idx ON explore.event (fetched_at);

-- Signed event-create/edit overlay merged over the mirrored row; a row with an
-- overlay is never overwritten or swept by the mirror.
CREATE TABLE explore.events_local (
    id              text PRIMARY KEY,
    signer          text NOT NULL,
    signed_payload  jsonb NOT NULL DEFAULT '{}'::jsonb,
    signed_at       timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    updated_at      timestamp without time zone NOT NULL DEFAULT timezone('UTC', now())
);

-- RSVP state written by signed-fetch going/cancel actions; read by every
-- authenticated event list (the viewer's `attending`) and by attendee listings.
CREATE TABLE explore.event_attendance_local (
    event_id        text NOT NULL,
    signer          text NOT NULL,
    signed_payload  jsonb NOT NULL DEFAULT '{}'::jsonb,
    action          text NOT NULL,
    signed_at       timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    PRIMARY KEY (event_id, signer),
    CONSTRAINT event_attendance_local_signer_lowercase CHECK (signer = lower(signer))
);
-- The viewer subqueries bind the lowercased wallet with `signer = $1 AND action = 'going'`
-- (no event_id bound); the PK serves the per-event probes.
CREATE INDEX event_attendance_local_signer_action_idx
    ON explore.event_attendance_local (signer, action);

CREATE TABLE explore.schedules_local (
    id            text PRIMARY KEY,
    name          text NOT NULL,
    description   text,
    image         text,
    theme         text,
    background    jsonb NOT NULL DEFAULT '[]'::jsonb,
    active_since  timestamp without time zone,
    active_until  timestamp without time zone,
    active        boolean NOT NULL DEFAULT true,
    created_at    timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    updated_at    timestamp without time zone NOT NULL DEFAULT timezone('UTC', now())
);

-- The local moderator allow-list: a wallet here sees pending/rejected events in
-- the lists and may sign schedule/profile-settings actions (stage 5).
CREATE TABLE explore.moderators (
    address     text PRIMARY KEY,
    added_by    text,
    added_at    bigint NOT NULL
);

-- The per-signer replay store shared by the places and events signed actions.
CREATE TABLE explore.seen_nonces (
    signer     text NOT NULL,
    nonce      text NOT NULL,
    expires_at bigint NOT NULL,
    PRIMARY KEY (signer, nonce)
);
-- The replay store's expiry sweep, `DELETE FROM seen_nonces WHERE expires_at < $1`,
-- arrives with the signed-action writer (stage 5); the PK serves the replay probe.
CREATE INDEX idx_seen_nonces_expires ON explore.seen_nonces (expires_at);

-- ── mirror cursors and pass markers ────────────────────────────────────────

-- One row per mirror (places, worlds, events). next_offset is the in-flight
-- cursor: set while a pass runs, NULL once it finishes, so a process killed
-- mid-pass resumes at that page with the same pass_started_at (the sweep and
-- the unlisted check are computed from it). last_complete_pass_at moves only
-- when every page was fetched and every row upserted; /api/status reports it
-- and its lag.
CREATE TABLE explore.mirror_state (
    mirror                   text PRIMARY KEY,
    source_url               text NOT NULL,
    pass_started_at          timestamp without time zone,
    next_offset              bigint,
    pass_rows                bigint NOT NULL DEFAULT 0,
    last_pass_at             timestamp without time zone,
    last_pass_complete       boolean,
    last_pass_rows           bigint,
    last_pass_swept          bigint,
    last_complete_pass_at    timestamp without time zone,
    last_complete_pass_rows  bigint,
    last_error               text,
    updated_at               timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    CONSTRAINT mirror_state_known_mirror CHECK (mirror IN ('places', 'worlds', 'events'))
);

-- ── Genesis City roads ─────────────────────────────────────────────────────

-- Road parcels, keyed by "x,y". Roads register as ordinary places with the
-- Foundation as their contact and a generated title, so no place column
-- identifies them; the destinations feed excludes them with an indexed probe.
-- Seeded from upstream places/src/__data__/RoadCoordinates.json (9,434 parcels).
CREATE TABLE explore.road_positions (
    position text PRIMARY KEY
);

INSERT INTO explore.road_positions (position) VALUES
    ('-150,62'), ('-150,61'), ('-150,33'), ('-150,27'), ('-150,0'), ('-150,-20'), ('-149,62'), ('-149,61'), ('-149,33'), ('-149,27'), ('-149,0'), ('-149,-20'), ('-148,62'), ('-148,61'), ('-148,33'), ('-148,27'), ('-148,26'), ('-148,25'), ('-148,24'), ('-148,0'),
    ('-148,-20'), ('-148,-49'), ('-148,-50'), ('-148,-51'), ('-148,-52'), ('-148,-53'), ('-148,-54'), ('-148,-55'), ('-148,-58'), ('-148,-59'), ('-148,-60'), ('-148,-61'), ('-148,-62'), ('-148,-63'), ('-148,-64'), ('-148,-65'), ('-148,-70'), ('-148,-74'), ('-147,62'), ('-147,61'),
    ('-147,33'), ('-147,27'), ('-147,24'), ('-147,0'), ('-147,-20'), ('-147,-49'), ('-147,-55'), ('-147,-58'), ('-147,-65'), ('-147,-70'), ('-147,-74'), ('-146,62'), ('-146,61'), ('-146,33'), ('-146,27'), ('-146,24'), ('-146,0'), ('-146,-20'), ('-146,-49'), ('-146,-55'),
    ('-146,-58'), ('-146,-65'), ('-146,-70'), ('-146,-74'), ('-145,62'), ('-145,61'), ('-145,33'), ('-145,27'), ('-145,26'), ('-145,25'), ('-145,24'), ('-145,0'), ('-145,-20'), ('-145,-49'), ('-145,-55'), ('-145,-58'), ('-145,-65'), ('-145,-70'), ('-145,-74'), ('-144,62'),
    ('-144,61'), ('-144,33'), ('-144,27'), ('-144,0'), ('-144,-20'), ('-144,-26'), ('-144,-27'), ('-144,-28'), ('-144,-29'), ('-144,-30'), ('-144,-31'), ('-144,-32'), ('-144,-33'), ('-144,-34'), ('-144,-35'), ('-144,-36'), ('-144,-37'), ('-144,-38'), ('-144,-39'), ('-144,-40'),
    ('-144,-41'), ('-144,-42'), ('-144,-43'), ('-144,-44'), ('-144,-49'), ('-144,-55'), ('-144,-58'), ('-144,-65'), ('-144,-70'), ('-144,-74'), ('-143,62'), ('-143,61'), ('-143,33'), ('-143,27'), ('-143,0'), ('-143,-20'), ('-143,-26'), ('-143,-44'), ('-143,-49'), ('-143,-55'),
    ('-143,-58'), ('-143,-65'), ('-143,-70'), ('-143,-74'), ('-142,62'), ('-142,61'), ('-142,33'), ('-142,27'), ('-142,26'), ('-142,25'), ('-142,24'), ('-142,0'), ('-142,-20'), ('-142,-26'), ('-142,-44'), ('-142,-49'), ('-142,-55'), ('-142,-58'), ('-142,-62'), ('-142,-63'),
    ('-142,-64'), ('-142,-65'), ('-142,-66'), ('-142,-67'), ('-142,-68'), ('-142,-69'), ('-142,-70'), ('-142,-71'), ('-142,-72'), ('-142,-73'), ('-142,-74'), ('-141,62'), ('-141,61'), ('-141,33'), ('-141,27'), ('-141,24'), ('-141,0'), ('-141,-20'), ('-141,-26'), ('-141,-44'),
    ('-141,-49'), ('-141,-55'), ('-141,-58'), ('-141,-65'), ('-141,-70'), ('-141,-74'), ('-140,62'), ('-140,61'), ('-140,33'), ('-140,27'), ('-140,24'), ('-140,0'), ('-140,-20'), ('-140,-26'), ('-140,-44'), ('-140,-49'), ('-140,-55'), ('-140,-58'), ('-140,-65'), ('-140,-70'),
    ('-140,-74'), ('-139,62'), ('-139,61'), ('-139,33'), ('-139,27'), ('-139,26'), ('-139,25'), ('-139,24'), ('-139,0'), ('-139,-20'), ('-139,-26'), ('-139,-44'), ('-139,-49'), ('-139,-55'), ('-139,-58'), ('-139,-65'), ('-139,-70'), ('-139,-74'), ('-138,62'), ('-138,61'),
    ('-138,33'), ('-138,27'), ('-138,24'), ('-138,0'), ('-138,-20'), ('-138,-26'), ('-138,-44'), ('-138,-49'), ('-138,-55'), ('-138,-58'), ('-138,-65'), ('-138,-70'), ('-138,-74'), ('-137,62'), ('-137,61'), ('-137,33'), ('-137,27'), ('-137,24'), ('-137,0'), ('-137,-20'),
    ('-137,-26'), ('-137,-44'), ('-137,-49'), ('-137,-55'), ('-137,-58'), ('-137,-62'), ('-137,-63'), ('-137,-64'), ('-137,-65'), ('-137,-66'), ('-137,-67'), ('-137,-68'), ('-137,-69'), ('-137,-70'), ('-137,-74'), ('-136,62'), ('-136,61'), ('-136,33'), ('-136,27'), ('-136,24'),
    ('-136,23'), ('-136,22'), ('-136,21'), ('-136,0'), ('-136,-20'), ('-136,-26'), ('-136,-44'), ('-136,-49'), ('-136,-55'), ('-136,-58'), ('-136,-62'), ('-136,-74'), ('-135,62'), ('-135,61'), ('-135,33'), ('-135,27'), ('-135,24'), ('-135,21'), ('-135,0'), ('-135,-20'),
    ('-135,-26'), ('-135,-27'), ('-135,-28'), ('-135,-29'), ('-135,-30'), ('-135,-31'), ('-135,-32'), ('-135,-33'), ('-135,-34'), ('-135,-35'), ('-135,-36'), ('-135,-37'), ('-135,-38'), ('-135,-39'), ('-135,-40'), ('-135,-41'), ('-135,-42'), ('-135,-43'), ('-135,-44'), ('-135,-49'),
    ('-135,-50'), ('-135,-51'), ('-135,-52'), ('-135,-53'), ('-135,-54'), ('-135,-55'), ('-135,-58'), ('-135,-62'), ('-135,-74'), ('-134,62'), ('-134,61'), ('-134,33'), ('-134,27'), ('-134,24'), ('-134,21'), ('-134,0'), ('-134,-20'), ('-134,-32'), ('-134,-39'), ('-134,-44'),
    ('-134,-52'), ('-134,-58'), ('-134,-62'), ('-134,-63'), ('-134,-64'), ('-134,-65'), ('-134,-66'), ('-134,-67'), ('-134,-68'), ('-134,-69'), ('-134,-70'), ('-134,-74'), ('-133,62'), ('-133,61'), ('-133,33'), ('-133,27'), ('-133,24'), ('-133,23'), ('-133,22'), ('-133,21'),
    ('-133,20'), ('-133,0'), ('-133,-20'), ('-133,-32'), ('-133,-39'), ('-133,-44'), ('-133,-52'), ('-133,-58'), ('-133,-70'), ('-133,-74'), ('-132,62'), ('-132,61'), ('-132,33'), ('-132,27'), ('-132,0'), ('-132,-20'), ('-132,-32'), ('-132,-39'), ('-132,-44'), ('-132,-52'),
    ('-132,-58'), ('-132,-70'), ('-132,-74'), ('-131,62'), ('-131,61'), ('-131,33'), ('-131,27'), ('-131,0'), ('-131,-20'), ('-131,-32'), ('-131,-39'), ('-131,-44'), ('-131,-45'), ('-131,-46'), ('-131,-47'), ('-131,-48'), ('-131,-49'), ('-131,-50'), ('-131,-51'), ('-131,-52'),
    ('-131,-53'), ('-131,-54'), ('-131,-55'), ('-131,-56'), ('-131,-57'), ('-131,-58'), ('-131,-62'), ('-131,-63'), ('-131,-64'), ('-131,-65'), ('-131,-66'), ('-131,-67'), ('-131,-68'), ('-131,-69'), ('-131,-70'), ('-131,-74'), ('-130,62'), ('-130,61'), ('-130,33'), ('-130,27'),
    ('-130,24'), ('-130,23'), ('-130,22'), ('-130,21'), ('-130,0'), ('-130,-20'), ('-130,-29'), ('-130,-30'), ('-130,-31'), ('-130,-32'), ('-130,-33'), ('-130,-34'), ('-130,-35'), ('-130,-38'), ('-130,-39'), ('-130,-40'), ('-130,-41'), ('-130,-42'), ('-130,-74'), ('-129,62'),
    ('-129,61'), ('-129,33'), ('-129,27'), ('-129,24'), ('-129,21'), ('-129,0'), ('-129,-20'), ('-129,-29'), ('-129,-35'), ('-129,-38'), ('-129,-42'), ('-129,-74'), ('-128,62'), ('-128,61'), ('-128,33'), ('-128,27'), ('-128,24'), ('-128,21'), ('-128,0'), ('-128,-20'),
    ('-128,-29'), ('-128,-35'), ('-128,-38'), ('-128,-42'), ('-128,-46'), ('-128,-47'), ('-128,-48'), ('-128,-49'), ('-128,-50'), ('-128,-51'), ('-128,-52'), ('-128,-53'), ('-128,-54'), ('-128,-55'), ('-128,-56'), ('-128,-57'), ('-128,-58'), ('-128,-59'), ('-128,-60'), ('-128,-61'),
    ('-128,-62'), ('-128,-63'), ('-128,-64'), ('-128,-65'), ('-128,-66'), ('-128,-67'), ('-128,-68'), ('-128,-69'), ('-128,-70'), ('-128,-71'), ('-128,-72'), ('-128,-73'), ('-128,-74'), ('-127,62'), ('-127,61'), ('-127,33'), ('-127,27'), ('-127,26'), ('-127,25'), ('-127,24'),
    ('-127,23'), ('-127,22'), ('-127,21'), ('-127,20'), ('-127,0'), ('-127,-20'), ('-127,-29'), ('-127,-35'), ('-127,-36'), ('-127,-37'), ('-127,-38'), ('-127,-42'), ('-127,-46'), ('-126,62'), ('-126,61'), ('-126,33'), ('-126,27'), ('-126,0'), ('-126,-20'), ('-126,-29'),
    ('-126,-35'), ('-126,-38'), ('-126,-42'), ('-126,-46'), ('-125,62'), ('-125,61'), ('-125,33'), ('-125,27'), ('-125,0'), ('-125,-20'), ('-125,-29'), ('-125,-35'), ('-125,-38'), ('-125,-42'), ('-125,-46'), ('-125,-57'), ('-125,-58'), ('-125,-59'), ('-125,-60'), ('-125,-61'),
    ('-125,-62'), ('-124,62'), ('-124,61'), ('-124,33'), ('-124,27'), ('-124,24'), ('-124,23'), ('-124,22'), ('-124,21'), ('-124,0'), ('-124,-20'), ('-124,-29'), ('-124,-30'), ('-124,-31'), ('-124,-32'), ('-124,-33'), ('-124,-34'), ('-124,-35'), ('-124,-38'), ('-124,-39'),
    ('-124,-40'), ('-124,-41'), ('-124,-42'), ('-124,-43'), ('-124,-44'), ('-124,-45'), ('-124,-46'), ('-124,-51'), ('-124,-52'), ('-124,-53'), ('-124,-54'), ('-124,-57'), ('-124,-62'), ('-124,-63'), ('-124,-64'), ('-124,-65'), ('-124,-66'), ('-124,-67'), ('-124,-68'), ('-124,-69'),
    ('-124,-70'), ('-124,-71'), ('-124,-72'), ('-124,-73'), ('-124,-74'), ('-124,-75'), ('-124,-76'), ('-124,-77'), ('-124,-78'), ('-124,-79'), ('-124,-80'), ('-124,-81'), ('-124,-82'), ('-124,-83'), ('-124,-84'), ('-124,-85'), ('-124,-86'), ('-124,-87'), ('-124,-88'), ('-124,-89'),
    ('-124,-90'), ('-124,-91'), ('-124,-92'), ('-124,-93'), ('-124,-94'), ('-124,-95'), ('-124,-96'), ('-123,62'), ('-123,61'), ('-123,33'), ('-123,27'), ('-123,24'), ('-123,21'), ('-123,0'), ('-123,-20'), ('-123,-32'), ('-123,-42'), ('-123,-46'), ('-123,-51'), ('-123,-54'),
    ('-123,-57'), ('-123,-62'), ('-122,62'), ('-122,61'), ('-122,33'), ('-122,27'), ('-122,24'), ('-122,21'), ('-122,0'), ('-122,-20'), ('-122,-32'), ('-122,-42'), ('-122,-46'), ('-122,-51'), ('-122,-54'), ('-122,-57'), ('-122,-62'), ('-121,62'), ('-121,61'), ('-121,33'),
    ('-121,27'), ('-121,26'), ('-121,25'), ('-121,24'), ('-121,23'), ('-121,22'), ('-121,21'), ('-121,0'), ('-121,-20'), ('-121,-27'), ('-121,-28'), ('-121,-29'), ('-121,-30'), ('-121,-31'), ('-121,-32'), ('-121,-33'), ('-121,-34'), ('-121,-35'), ('-121,-36'), ('-121,-37'),
    ('-121,-38'), ('-121,-39'), ('-121,-40'), ('-121,-41'), ('-121,-42'), ('-121,-46'), ('-121,-51'), ('-121,-54'), ('-121,-57'), ('-121,-62'), ('-120,62'), ('-120,61'), ('-120,33'), ('-120,27'), ('-120,24'), ('-120,0'), ('-120,-20'), ('-120,-32'), ('-120,-42'), ('-120,-46'),
    ('-120,-51'), ('-120,-54'), ('-120,-57'), ('-120,-62'), ('-119,62'), ('-119,61'), ('-119,33'), ('-119,27'), ('-119,24'), ('-119,0'), ('-119,-20'), ('-119,-32'), ('-119,-42'), ('-119,-46'), ('-119,-51'), ('-119,-54'), ('-119,-57'), ('-119,-60'), ('-119,-61'), ('-119,-62'),
    ('-119,-63'), ('-119,-64'), ('-118,62'), ('-118,61'), ('-118,33'), ('-118,27'), ('-118,24'), ('-118,23'), ('-118,22'), ('-118,21'), ('-118,0'), ('-118,-20'), ('-118,-29'), ('-118,-30'), ('-118,-31'), ('-118,-32'), ('-118,-33'), ('-118,-34'), ('-118,-35'), ('-118,-36'),
    ('-118,-37'), ('-118,-38'), ('-118,-39'), ('-118,-42'), ('-118,-46'), ('-118,-47'), ('-118,-48'), ('-118,-51'), ('-118,-54'), ('-118,-57'), ('-118,-60'), ('-118,-64'), ('-117,62'), ('-117,61'), ('-117,33'), ('-117,27'), ('-117,24'), ('-117,21'), ('-117,0'), ('-117,-20'),
    ('-117,-29'), ('-117,-39'), ('-117,-42'), ('-117,-46'), ('-117,-51'), ('-117,-54'), ('-117,-57'), ('-117,-60'), ('-117,-64'), ('-116,62'), ('-116,61'), ('-116,33'), ('-116,29'), ('-116,28'), ('-116,27'), ('-116,24'), ('-116,21'), ('-116,0'), ('-116,-20'), ('-116,-29'),
    ('-116,-39'), ('-116,-42'), ('-116,-46'), ('-116,-51'), ('-116,-54'), ('-116,-57'), ('-116,-60'), ('-116,-64'), ('-115,62'), ('-115,61'), ('-115,33'), ('-115,29'), ('-115,24'), ('-115,23'), ('-115,22'), ('-115,21'), ('-115,0'), ('-115,-20'), ('-115,-27'), ('-115,-28'),
    ('-115,-29'), ('-115,-39'), ('-115,-40'), ('-115,-41'), ('-115,-42'), ('-115,-43'), ('-115,-44'), ('-115,-45'), ('-115,-46'), ('-115,-51'), ('-115,-54'), ('-115,-57'), ('-115,-60'), ('-115,-64'), ('-114,62'), ('-114,61'), ('-114,33'), ('-114,29'), ('-114,24'), ('-114,0'),
    ('-114,-20'), ('-114,-29'), ('-114,-39'), ('-114,-42'), ('-114,-46'), ('-114,-51'), ('-114,-54'), ('-114,-55'), ('-114,-56'), ('-114,-57'), ('-114,-58'), ('-114,-59'), ('-114,-60'), ('-114,-64'), ('-114,-65'), ('-114,-66'), ('-114,-67'), ('-114,-68'), ('-114,-69'), ('-113,62'),
    ('-113,61'), ('-113,33'), ('-113,29'), ('-113,24'), ('-113,0'), ('-113,-20'), ('-113,-29'), ('-113,-39'), ('-113,-42'), ('-113,-46'), ('-113,-47'), ('-113,-48'), ('-113,-49'), ('-113,-50'), ('-113,-51'), ('-113,-54'), ('-113,-59'), ('-113,-65'), ('-113,-69'), ('-112,62'),
    ('-112,61'), ('-112,33'), ('-112,29'), ('-112,28'), ('-112,27'), ('-112,26'), ('-112,25'), ('-112,24'), ('-112,0'), ('-112,-20'), ('-112,-29'), ('-112,-39'), ('-112,-42'), ('-112,-46'), ('-112,-51'), ('-112,-54'), ('-112,-59'), ('-112,-65'), ('-112,-69'), ('-111,62'),
    ('-111,61'), ('-111,33'), ('-111,27'), ('-111,0'), ('-111,-20'), ('-111,-29'), ('-111,-30'), ('-111,-31'), ('-111,-32'), ('-111,-33'), ('-111,-34'), ('-111,-35'), ('-111,-36'), ('-111,-37'), ('-111,-38'), ('-111,-39'), ('-111,-42'), ('-111,-46'), ('-111,-51'), ('-111,-54'),
    ('-111,-57'), ('-111,-58'), ('-111,-59'), ('-111,-60'), ('-111,-61'), ('-111,-62'), ('-111,-63'), ('-111,-64'), ('-111,-65'), ('-111,-66'), ('-111,-67'), ('-111,-68'), ('-111,-69'), ('-110,62'), ('-110,61'), ('-110,33'), ('-110,27'), ('-110,0'), ('-110,-20'), ('-110,-42'),
    ('-110,-46'), ('-110,-51'), ('-110,-54'), ('-110,-57'), ('-110,-64'), ('-110,-69'), ('-109,62'), ('-109,61'), ('-109,33'), ('-109,29'), ('-109,28'), ('-109,27'), ('-109,26'), ('-109,25'), ('-109,0'), ('-109,-20'), ('-109,-42'), ('-109,-46'), ('-109,-51'), ('-109,-54'),
    ('-109,-57'), ('-109,-64'), ('-109,-69'), ('-108,62'), ('-108,61'), ('-108,33'), ('-108,29'), ('-108,25'), ('-108,0'), ('-108,-20'), ('-108,-29'), ('-108,-32'), ('-108,-39'), ('-108,-42'), ('-108,-46'), ('-108,-51'), ('-108,-54'), ('-108,-57'), ('-108,-69'), ('-107,62'),
    ('-107,61'), ('-107,33'), ('-107,29'), ('-107,25'), ('-107,0'), ('-107,-20'), ('-107,-29'), ('-107,-32'), ('-107,-39'), ('-107,-42'), ('-107,-46'), ('-107,-51'), ('-107,-54'), ('-107,-57'), ('-107,-69'), ('-106,62'), ('-106,61'), ('-106,33'), ('-106,29'), ('-106,25'),
    ('-106,0'), ('-106,-20'), ('-106,-27'), ('-106,-28'), ('-106,-29'), ('-106,-30'), ('-106,-31'), ('-106,-32'), ('-106,-33'), ('-106,-34'), ('-106,-35'), ('-106,-36'), ('-106,-37'), ('-106,-38'), ('-106,-39'), ('-106,-42'), ('-106,-46'), ('-106,-51'), ('-106,-54'), ('-106,-55'),
    ('-106,-56'), ('-106,-57'), ('-106,-58'), ('-106,-59'), ('-106,-62'), ('-106,-63'), ('-106,-64'), ('-106,-65'), ('-106,-66'), ('-106,-67'), ('-106,-68'), ('-106,-69'), ('-105,62'), ('-105,61'), ('-105,33'), ('-105,29'), ('-105,28'), ('-105,27'), ('-105,26'), ('-105,25'),
    ('-105,0'), ('-105,-20'), ('-105,-29'), ('-105,-32'), ('-105,-39'), ('-105,-40'), ('-105,-41'), ('-105,-42'), ('-105,-43'), ('-105,-44'), ('-105,-45'), ('-105,-46'), ('-105,-51'), ('-105,-54'), ('-105,-58'), ('-105,-62'), ('-105,-69'), ('-104,62'), ('-104,61'), ('-104,33'),
    ('-104,27'), ('-104,0'), ('-104,-20'), ('-104,-29'), ('-104,-32'), ('-104,-39'), ('-104,-42'), ('-104,-46'), ('-104,-51'), ('-104,-54'), ('-104,-58'), ('-104,-62'), ('-104,-69'), ('-103,62'), ('-103,61'), ('-103,33'), ('-103,27'), ('-103,0'), ('-103,-20'), ('-103,-29')
;

INSERT INTO explore.road_positions (position) VALUES
    ('-103,-30'), ('-103,-31'), ('-103,-32'), ('-103,-33'), ('-103,-34'), ('-103,-35'), ('-103,-36'), ('-103,-37'), ('-103,-38'), ('-103,-39'), ('-103,-42'), ('-103,-46'), ('-103,-47'), ('-103,-48'), ('-103,-49'), ('-103,-50'), ('-103,-51'), ('-103,-54'), ('-103,-57'), ('-103,-58'),
    ('-103,-59'), ('-103,-60'), ('-103,-61'), ('-103,-62'), ('-103,-69'), ('-102,62'), ('-102,61'), ('-102,33'), ('-102,29'), ('-102,28'), ('-102,27'), ('-102,26'), ('-102,25'), ('-102,0'), ('-102,-20'), ('-102,-42'), ('-102,-46'), ('-102,-51'), ('-102,-54'), ('-102,-62'),
    ('-102,-69'), ('-101,62'), ('-101,61'), ('-101,33'), ('-101,29'), ('-101,25'), ('-101,0'), ('-101,-20'), ('-101,-42'), ('-101,-46'), ('-101,-51'), ('-101,-54'), ('-101,-62'), ('-101,-69'), ('-100,62'), ('-100,61'), ('-100,33'), ('-100,29'), ('-100,25'), ('-100,0'),
    ('-100,-20'), ('-100,-29'), ('-100,-30'), ('-100,-31'), ('-100,-32'), ('-100,-33'), ('-100,-34'), ('-100,-35'), ('-100,-36'), ('-100,-37'), ('-100,-38'), ('-100,-39'), ('-100,-40'), ('-100,-41'), ('-100,-42'), ('-100,-43'), ('-100,-44'), ('-100,-45'), ('-100,-46'), ('-100,-47'),
    ('-100,-48'), ('-100,-49'), ('-100,-50'), ('-100,-51'), ('-100,-52'), ('-100,-53'), ('-100,-54'), ('-100,-55'), ('-100,-56'), ('-100,-57'), ('-100,-58'), ('-100,-59'), ('-100,-62'), ('-100,-69'), ('-99,62'), ('-99,61'), ('-99,33'), ('-99,29'), ('-99,25'), ('-99,0'),
    ('-99,-20'), ('-99,-29'), ('-99,-59'), ('-99,-62'), ('-99,-69'), ('-98,62'), ('-98,61'), ('-98,33'), ('-98,29'), ('-98,28'), ('-98,27'), ('-98,26'), ('-98,25'), ('-98,0'), ('-98,-20'), ('-98,-29'), ('-98,-59'), ('-98,-60'), ('-98,-61'), ('-98,-62'),
    ('-98,-69'), ('-97,62'), ('-97,61'), ('-97,33'), ('-97,27'), ('-97,0'), ('-97,-20'), ('-97,-29'), ('-97,-59'), ('-97,-62'), ('-97,-69'), ('-96,62'), ('-96,61'), ('-96,33'), ('-96,27'), ('-96,0'), ('-96,-20'), ('-96,-29'), ('-96,-59'), ('-96,-62'),
    ('-96,-69'), ('-95,62'), ('-95,61'), ('-95,33'), ('-95,27'), ('-95,0'), ('-95,-20'), ('-95,-29'), ('-95,-59'), ('-95,-62'), ('-95,-69'), ('-94,62'), ('-94,61'), ('-94,33'), ('-94,32'), ('-94,31'), ('-94,30'), ('-94,29'), ('-94,28'), ('-94,27'),
    ('-94,26'), ('-94,25'), ('-94,24'), ('-94,23'), ('-94,22'), ('-94,21'), ('-94,20'), ('-94,19'), ('-94,18'), ('-94,17'), ('-94,16'), ('-94,15'), ('-94,14'), ('-94,13'), ('-94,12'), ('-94,11'), ('-94,10'), ('-94,9'), ('-94,8'), ('-94,7'),
    ('-94,6'), ('-94,5'), ('-94,4'), ('-94,3'), ('-94,2'), ('-94,1'), ('-94,0'), ('-94,-1'), ('-94,-2'), ('-94,-3'), ('-94,-4'), ('-94,-5'), ('-94,-6'), ('-94,-7'), ('-94,-8'), ('-94,-9'), ('-94,-10'), ('-94,-11'), ('-94,-12'), ('-94,-13'),
    ('-94,-14'), ('-94,-15'), ('-94,-16'), ('-94,-17'), ('-94,-18'), ('-94,-19'), ('-94,-20'), ('-94,-21'), ('-94,-22'), ('-94,-23'), ('-94,-24'), ('-94,-25'), ('-94,-26'), ('-94,-27'), ('-94,-28'), ('-94,-29'), ('-94,-59'), ('-94,-62'), ('-94,-69'), ('-93,62'),
    ('-93,61'), ('-93,11'), ('-93,10'), ('-93,9'), ('-93,8'), ('-93,7'), ('-93,6'), ('-93,5'), ('-93,4'), ('-93,3'), ('-93,2'), ('-93,1'), ('-93,0'), ('-93,-1'), ('-93,-2'), ('-93,-3'), ('-93,-4'), ('-93,-5'), ('-93,-6'), ('-93,-7'),
    ('-93,-8'), ('-93,-9'), ('-93,-10'), ('-93,-11'), ('-93,-12'), ('-93,-13'), ('-93,-14'), ('-93,-15'), ('-93,-16'), ('-93,-17'), ('-93,-18'), ('-93,-19'), ('-93,-20'), ('-93,-21'), ('-93,-22'), ('-93,-23'), ('-93,-24'), ('-93,-25'), ('-93,-26'), ('-93,-27'),
    ('-93,-28'), ('-93,-29'), ('-93,-59'), ('-93,-62'), ('-93,-69'), ('-92,62'), ('-92,61'), ('-92,11'), ('-92,10'), ('-92,-11'), ('-92,-12'), ('-92,-29'), ('-92,-59'), ('-92,-62'), ('-92,-69'), ('-91,62'), ('-91,61'), ('-91,11'), ('-91,10'), ('-91,-11'),
    ('-91,-12'), ('-91,-29'), ('-91,-59'), ('-91,-62'), ('-91,-69'), ('-90,62'), ('-90,61'), ('-90,11'), ('-90,10'), ('-90,-11'), ('-90,-12'), ('-90,-29'), ('-90,-59'), ('-90,-62'), ('-90,-69'), ('-89,62'), ('-89,61'), ('-89,11'), ('-89,10'), ('-89,-11'),
    ('-89,-12'), ('-89,-29'), ('-89,-59'), ('-89,-60'), ('-89,-61'), ('-89,-62'), ('-89,-69'), ('-88,62'), ('-88,61'), ('-88,11'), ('-88,10'), ('-88,-11'), ('-88,-12'), ('-88,-29'), ('-88,-59'), ('-88,-62'), ('-88,-69'), ('-87,62'), ('-87,61'), ('-87,11'),
    ('-87,10'), ('-87,-11'), ('-87,-12'), ('-87,-29'), ('-87,-30'), ('-87,-31'), ('-87,-32'), ('-87,-33'), ('-87,-34'), ('-87,-35'), ('-87,-36'), ('-87,-37'), ('-87,-38'), ('-87,-39'), ('-87,-40'), ('-87,-41'), ('-87,-42'), ('-87,-43'), ('-87,-44'), ('-87,-45'),
    ('-87,-46'), ('-87,-47'), ('-87,-48'), ('-87,-49'), ('-87,-50'), ('-87,-51'), ('-87,-52'), ('-87,-53'), ('-87,-54'), ('-87,-55'), ('-87,-56'), ('-87,-57'), ('-87,-58'), ('-87,-59'), ('-87,-62'), ('-87,-63'), ('-87,-64'), ('-87,-65'), ('-87,-66'), ('-87,-67'),
    ('-87,-68'), ('-87,-69'), ('-86,62'), ('-86,61'), ('-86,11'), ('-86,10'), ('-86,-11'), ('-86,-12'), ('-86,-43'), ('-86,-44'), ('-85,62'), ('-85,61'), ('-85,11'), ('-85,10'), ('-85,-11'), ('-85,-12'), ('-85,-43'), ('-85,-44'), ('-84,62'), ('-84,61'),
    ('-84,11'), ('-84,10'), ('-84,-11'), ('-84,-12'), ('-83,62'), ('-83,61'), ('-83,11'), ('-83,10'), ('-83,-11'), ('-83,-12'), ('-82,62'), ('-82,61'), ('-82,11'), ('-82,10'), ('-82,-11'), ('-82,-12'), ('-81,62'), ('-81,61'), ('-81,11'), ('-81,10'),
    ('-81,-11'), ('-81,-12'), ('-80,62'), ('-80,61'), ('-80,11'), ('-80,10'), ('-80,-11'), ('-80,-12'), ('-79,62'), ('-79,61'), ('-79,11'), ('-79,10'), ('-79,-11'), ('-79,-12'), ('-78,62'), ('-78,61'), ('-78,11'), ('-78,10'), ('-78,-11'), ('-78,-12'),
    ('-77,62'), ('-77,61'), ('-77,11'), ('-77,10'), ('-77,-11'), ('-77,-12'), ('-76,62'), ('-76,61'), ('-76,11'), ('-76,10'), ('-76,-11'), ('-76,-12'), ('-75,62'), ('-75,61'), ('-75,11'), ('-75,10'), ('-75,-11'), ('-75,-12'), ('-74,62'), ('-74,61'),
    ('-74,11'), ('-74,10'), ('-74,-11'), ('-74,-12'), ('-73,150'), ('-73,149'), ('-73,148'), ('-73,147'), ('-73,146'), ('-73,145'), ('-73,144'), ('-73,143'), ('-73,142'), ('-73,141'), ('-73,140'), ('-73,139'), ('-73,138'), ('-73,137'), ('-73,136'), ('-73,135'),
    ('-73,134'), ('-73,133'), ('-73,132'), ('-73,131'), ('-73,130'), ('-73,129'), ('-73,128'), ('-73,127'), ('-73,126'), ('-73,125'), ('-73,124'), ('-73,123'), ('-73,122'), ('-73,121'), ('-73,120'), ('-73,119'), ('-73,118'), ('-73,117'), ('-73,116'), ('-73,115'),
    ('-73,114'), ('-73,113'), ('-73,112'), ('-73,111'), ('-73,110'), ('-73,109'), ('-73,108'), ('-73,107'), ('-73,106'), ('-73,105'), ('-73,104'), ('-73,103'), ('-73,102'), ('-73,101'), ('-73,100'), ('-73,99'), ('-73,98'), ('-73,97'), ('-73,96'), ('-73,95'),
    ('-73,94'), ('-73,93'), ('-73,92'), ('-73,91'), ('-73,90'), ('-73,89'), ('-73,88'), ('-73,87'), ('-73,86'), ('-73,85'), ('-73,84'), ('-73,83'), ('-73,82'), ('-73,81'), ('-73,80'), ('-73,79'), ('-73,78'), ('-73,77'), ('-73,76'), ('-73,75'),
    ('-73,74'), ('-73,73'), ('-73,72'), ('-73,71'), ('-73,70'), ('-73,69'), ('-73,68'), ('-73,67'), ('-73,66'), ('-73,65'), ('-73,64'), ('-73,63'), ('-73,62'), ('-73,61'), ('-73,60'), ('-73,59'), ('-73,58'), ('-73,57'), ('-73,56'), ('-73,55'),
    ('-73,54'), ('-73,53'), ('-73,52'), ('-73,51'), ('-73,50'), ('-73,11'), ('-73,10'), ('-73,-11'), ('-73,-12'), ('-73,-50'), ('-73,-51'), ('-73,-52'), ('-73,-53'), ('-73,-54'), ('-73,-55'), ('-73,-56'), ('-73,-57'), ('-73,-58'), ('-73,-59'), ('-73,-60'),
    ('-73,-61'), ('-73,-62'), ('-73,-63'), ('-73,-64'), ('-73,-65'), ('-73,-66'), ('-73,-67'), ('-73,-68'), ('-73,-69'), ('-73,-70'), ('-73,-71'), ('-73,-72'), ('-73,-73'), ('-72,150'), ('-72,149'), ('-72,148'), ('-72,147'), ('-72,146'), ('-72,145'), ('-72,144'),
    ('-72,143'), ('-72,142'), ('-72,141'), ('-72,140'), ('-72,139'), ('-72,138'), ('-72,137'), ('-72,136'), ('-72,135'), ('-72,134'), ('-72,133'), ('-72,132'), ('-72,131'), ('-72,130'), ('-72,129'), ('-72,128'), ('-72,127'), ('-72,126'), ('-72,125'), ('-72,124'),
    ('-72,123'), ('-72,122'), ('-72,121'), ('-72,120'), ('-72,119'), ('-72,118'), ('-72,117'), ('-72,116'), ('-72,115'), ('-72,114'), ('-72,113'), ('-72,112'), ('-72,111'), ('-72,110'), ('-72,109'), ('-72,108'), ('-72,107'), ('-72,106'), ('-72,105'), ('-72,104'),
    ('-72,103'), ('-72,102'), ('-72,101'), ('-72,100'), ('-72,99'), ('-72,98'), ('-72,97'), ('-72,96'), ('-72,95'), ('-72,94'), ('-72,93'), ('-72,92'), ('-72,91'), ('-72,90'), ('-72,89'), ('-72,88'), ('-72,87'), ('-72,86'), ('-72,85'), ('-72,84'),
    ('-72,83'), ('-72,82'), ('-72,81'), ('-72,80'), ('-72,79'), ('-72,78'), ('-72,77'), ('-72,76'), ('-72,75'), ('-72,74'), ('-72,73'), ('-72,72'), ('-72,71'), ('-72,70'), ('-72,69'), ('-72,68'), ('-72,67'), ('-72,66'), ('-72,65'), ('-72,64'),
    ('-72,63'), ('-72,62'), ('-72,61'), ('-72,60'), ('-72,59'), ('-72,58'), ('-72,57'), ('-72,56'), ('-72,55'), ('-72,54'), ('-72,53'), ('-72,52'), ('-72,51'), ('-72,50'), ('-72,11'), ('-72,10'), ('-72,9'), ('-72,8'), ('-72,7'), ('-72,6'),
    ('-72,5'), ('-72,4'), ('-72,3'), ('-72,2'), ('-72,1'), ('-72,0'), ('-72,-1'), ('-72,-2'), ('-72,-3'), ('-72,-4'), ('-72,-5'), ('-72,-6'), ('-72,-7'), ('-72,-8'), ('-72,-9'), ('-72,-10'), ('-72,-11'), ('-72,-12'), ('-72,-50'), ('-72,-51'),
    ('-72,-52'), ('-72,-53'), ('-72,-54'), ('-72,-55'), ('-72,-56'), ('-72,-57'), ('-72,-58'), ('-72,-59'), ('-72,-60'), ('-72,-61'), ('-72,-62'), ('-72,-63'), ('-72,-64'), ('-72,-65'), ('-72,-66'), ('-72,-67'), ('-72,-68'), ('-72,-69'), ('-72,-70'), ('-72,-71'),
    ('-72,-72'), ('-72,-73'), ('-71,115'), ('-71,73'), ('-71,72'), ('-71,51'), ('-71,50'), ('-71,11'), ('-71,10'), ('-71,9'), ('-71,8'), ('-71,7'), ('-71,6'), ('-71,5'), ('-71,4'), ('-71,3'), ('-71,2'), ('-71,1'), ('-71,0'), ('-71,-1'),
    ('-71,-2'), ('-71,-3'), ('-71,-4'), ('-71,-5'), ('-71,-6'), ('-71,-7'), ('-71,-8'), ('-71,-9'), ('-71,-10'), ('-71,-11'), ('-71,-12'), ('-71,-50'), ('-71,-51'), ('-71,-72'), ('-71,-73'), ('-70,115'), ('-70,73'), ('-70,72'), ('-70,51'), ('-70,50'),
    ('-70,0'), ('-70,-50'), ('-70,-51'), ('-70,-72'), ('-70,-73'), ('-69,130'), ('-69,129'), ('-69,128'), ('-69,127'), ('-69,124'), ('-69,123'), ('-69,122'), ('-69,121'), ('-69,120'), ('-69,119'), ('-69,118'), ('-69,117'), ('-69,116'), ('-69,115'), ('-69,73'),
    ('-69,72'), ('-69,51'), ('-69,50'), ('-69,0'), ('-69,-1'), ('-69,-50'), ('-69,-51'), ('-69,-72'), ('-69,-73'), ('-68,130'), ('-68,127'), ('-68,124'), ('-68,121'), ('-68,73'), ('-68,72'), ('-68,51'), ('-68,50'), ('-68,0'), ('-68,-1'), ('-68,-50'),
    ('-68,-51'), ('-68,-72'), ('-68,-73'), ('-67,130'), ('-67,127'), ('-67,124'), ('-67,121'), ('-67,73'), ('-67,72'), ('-67,51'), ('-67,50'), ('-67,0'), ('-67,-1'), ('-67,-50'), ('-67,-51'), ('-67,-72'), ('-67,-73'), ('-66,130'), ('-66,129'), ('-66,128'),
    ('-66,127'), ('-66,126'), ('-66,125'), ('-66,124'), ('-66,123'), ('-66,122'), ('-66,121'), ('-66,118'), ('-66,117'), ('-66,116'), ('-66,115'), ('-66,73'), ('-66,72'), ('-66,51'), ('-66,50'), ('-66,0'), ('-66,-1'), ('-66,-50'), ('-66,-51'), ('-66,-72'),
    ('-66,-73'), ('-65,127'), ('-65,118'), ('-65,115'), ('-65,73'), ('-65,72'), ('-65,51'), ('-65,50'), ('-65,0'), ('-65,-1'), ('-65,-50'), ('-65,-51'), ('-65,-72'), ('-65,-73'), ('-64,127'), ('-64,118'), ('-64,115'), ('-64,73'), ('-64,72'), ('-64,51'),
    ('-64,50'), ('-64,0'), ('-64,-1'), ('-64,-50'), ('-64,-51'), ('-64,-72'), ('-64,-73'), ('-63,130'), ('-63,129'), ('-63,128'), ('-63,127'), ('-63,126'), ('-63,125'), ('-63,124'), ('-63,123'), ('-63,122'), ('-63,121'), ('-63,118'), ('-63,117'), ('-63,116'),
    ('-63,115'), ('-63,73'), ('-63,72'), ('-63,51'), ('-63,50'), ('-63,0'), ('-63,-1'), ('-63,-50'), ('-63,-51'), ('-63,-72'), ('-63,-73'), ('-62,130'), ('-62,127'), ('-62,124'), ('-62,121'), ('-62,73'), ('-62,72'), ('-62,51'), ('-62,50'), ('-62,0'),
    ('-62,-1'), ('-62,-50'), ('-62,-51'), ('-62,-72'), ('-62,-73'), ('-61,130'), ('-61,127'), ('-61,124'), ('-61,121'), ('-61,73'), ('-61,72'), ('-61,51'), ('-61,50'), ('-61,0'), ('-61,-1'), ('-61,-50'), ('-61,-51'), ('-61,-72'), ('-61,-73'), ('-60,130'),
    ('-60,129'), ('-60,128'), ('-60,127'), ('-60,124'), ('-60,123'), ('-60,122'), ('-60,121'), ('-60,120'), ('-60,119'), ('-60,118'), ('-60,117'), ('-60,116'), ('-60,115'), ('-60,73'), ('-60,72'), ('-60,51'), ('-60,50'), ('-60,0'), ('-60,-1'), ('-60,-50'),
    ('-60,-51'), ('-60,-72'), ('-60,-73'), ('-59,118'), ('-59,115'), ('-59,73'), ('-59,72'), ('-59,51'), ('-59,50'), ('-59,0'), ('-59,-1'), ('-59,-50'), ('-59,-51'), ('-59,-72'), ('-59,-73'), ('-58,118'), ('-58,115'), ('-58,73'), ('-58,72'), ('-58,51'),
    ('-58,50'), ('-58,0'), ('-58,-1'), ('-58,-50'), ('-58,-51'), ('-58,-72'), ('-58,-73'), ('-58,-98'), ('-58,-99'), ('-58,-100'), ('-58,-101'), ('-58,-104'), ('-58,-105'), ('-58,-106'), ('-58,-107'), ('-57,118'), ('-57,117'), ('-57,116'), ('-57,115'), ('-57,73'),
    ('-57,72'), ('-57,51'), ('-57,50'), ('-57,0'), ('-57,-1'), ('-57,-50'), ('-57,-51'), ('-57,-72'), ('-57,-73'), ('-57,-96'), ('-57,-97'), ('-57,-98'), ('-57,-101'), ('-57,-102'), ('-57,-103'), ('-57,-104'), ('-57,-107'), ('-57,-108'), ('-57,-109'), ('-56,115')
;

INSERT INTO explore.road_positions (position) VALUES
    ('-56,73'), ('-56,72'), ('-56,51'), ('-56,50'), ('-56,0'), ('-56,-1'), ('-56,-50'), ('-56,-51'), ('-56,-72'), ('-56,-73'), ('-56,-96'), ('-56,-109'), ('-55,115'), ('-55,73'), ('-55,72'), ('-55,51'), ('-55,50'), ('-55,0'), ('-55,-1'), ('-55,-50'),
    ('-55,-51'), ('-55,-72'), ('-55,-73'), ('-55,-96'), ('-55,-109'), ('-54,115'), ('-54,73'), ('-54,72'), ('-54,51'), ('-54,50'), ('-54,0'), ('-54,-1'), ('-54,-50'), ('-54,-51'), ('-54,-72'), ('-54,-73'), ('-54,-96'), ('-54,-98'), ('-54,-99'), ('-54,-100'),
    ('-54,-101'), ('-54,-104'), ('-54,-105'), ('-54,-106'), ('-54,-107'), ('-54,-109'), ('-53,115'), ('-53,73'), ('-53,72'), ('-53,51'), ('-53,50'), ('-53,0'), ('-53,-1'), ('-53,-50'), ('-53,-51'), ('-53,-72'), ('-53,-73'), ('-53,-96'), ('-53,-97'), ('-53,-98'),
    ('-53,-101'), ('-53,-102'), ('-53,-103'), ('-53,-104'), ('-53,-107'), ('-53,-108'), ('-53,-109'), ('-52,130'), ('-52,129'), ('-52,128'), ('-52,127'), ('-52,124'), ('-52,123'), ('-52,122'), ('-52,121'), ('-52,120'), ('-52,119'), ('-52,118'), ('-52,117'), ('-52,116'),
    ('-52,115'), ('-52,114'), ('-52,113'), ('-52,112'), ('-52,111'), ('-52,110'), ('-52,109'), ('-52,108'), ('-52,107'), ('-52,106'), ('-52,105'), ('-52,104'), ('-52,103'), ('-52,102'), ('-52,101'), ('-52,100'), ('-52,99'), ('-52,98'), ('-52,97'), ('-52,96'),
    ('-52,95'), ('-52,94'), ('-52,93'), ('-52,92'), ('-52,91'), ('-52,90'), ('-52,89'), ('-52,88'), ('-52,87'), ('-52,86'), ('-52,85'), ('-52,84'), ('-52,83'), ('-52,82'), ('-52,81'), ('-52,80'), ('-52,79'), ('-52,78'), ('-52,77'), ('-52,76'),
    ('-52,75'), ('-52,74'), ('-52,73'), ('-52,72'), ('-52,51'), ('-52,50'), ('-52,0'), ('-52,-1'), ('-52,-50'), ('-52,-51'), ('-52,-72'), ('-52,-73'), ('-52,-96'), ('-52,-109'), ('-51,130'), ('-51,127'), ('-51,124'), ('-51,121'), ('-51,73'), ('-51,72'),
    ('-51,71'), ('-51,70'), ('-51,69'), ('-51,68'), ('-51,67'), ('-51,66'), ('-51,65'), ('-51,64'), ('-51,63'), ('-51,62'), ('-51,61'), ('-51,60'), ('-51,59'), ('-51,58'), ('-51,57'), ('-51,56'), ('-51,55'), ('-51,54'), ('-51,53'), ('-51,52'),
    ('-51,51'), ('-51,50'), ('-51,49'), ('-51,48'), ('-51,47'), ('-51,46'), ('-51,45'), ('-51,44'), ('-51,43'), ('-51,42'), ('-51,41'), ('-51,40'), ('-51,39'), ('-51,38'), ('-51,37'), ('-51,36'), ('-51,35'), ('-51,34'), ('-51,33'), ('-51,32'),
    ('-51,31'), ('-51,30'), ('-51,29'), ('-51,28'), ('-51,27'), ('-51,26'), ('-51,25'), ('-51,24'), ('-51,23'), ('-51,22'), ('-51,21'), ('-51,20'), ('-51,19'), ('-51,18'), ('-51,17'), ('-51,16'), ('-51,15'), ('-51,14'), ('-51,13'), ('-51,12'),
    ('-51,11'), ('-51,10'), ('-51,9'), ('-51,8'), ('-51,7'), ('-51,6'), ('-51,5'), ('-51,4'), ('-51,3'), ('-51,2'), ('-51,1'), ('-51,0'), ('-51,-1'), ('-51,-2'), ('-51,-3'), ('-51,-4'), ('-51,-5'), ('-51,-6'), ('-51,-7'), ('-51,-8'),
    ('-51,-9'), ('-51,-10'), ('-51,-11'), ('-51,-12'), ('-51,-13'), ('-51,-14'), ('-51,-15'), ('-51,-16'), ('-51,-17'), ('-51,-18'), ('-51,-19'), ('-51,-20'), ('-51,-21'), ('-51,-22'), ('-51,-23'), ('-51,-24'), ('-51,-25'), ('-51,-26'), ('-51,-27'), ('-51,-28'),
    ('-51,-29'), ('-51,-30'), ('-51,-31'), ('-51,-32'), ('-51,-33'), ('-51,-34'), ('-51,-35'), ('-51,-36'), ('-51,-37'), ('-51,-38'), ('-51,-39'), ('-51,-40'), ('-51,-41'), ('-51,-42'), ('-51,-43'), ('-51,-44'), ('-51,-45'), ('-51,-46'), ('-51,-47'), ('-51,-48'),
    ('-51,-49'), ('-51,-50'), ('-51,-51'), ('-51,-52'), ('-51,-53'), ('-51,-54'), ('-51,-55'), ('-51,-56'), ('-51,-57'), ('-51,-58'), ('-51,-59'), ('-51,-60'), ('-51,-61'), ('-51,-62'), ('-51,-63'), ('-51,-64'), ('-51,-65'), ('-51,-66'), ('-51,-67'), ('-51,-68'),
    ('-51,-69'), ('-51,-70'), ('-51,-71'), ('-51,-72'), ('-51,-73'), ('-51,-96'), ('-51,-109'), ('-50,130'), ('-50,127'), ('-50,124'), ('-50,121'), ('-50,73'), ('-50,72'), ('-50,71'), ('-50,70'), ('-50,69'), ('-50,68'), ('-50,67'), ('-50,66'), ('-50,65'),
    ('-50,64'), ('-50,63'), ('-50,62'), ('-50,61'), ('-50,60'), ('-50,59'), ('-50,58'), ('-50,57'), ('-50,56'), ('-50,55'), ('-50,54'), ('-50,53'), ('-50,52'), ('-50,51'), ('-50,50'), ('-50,49'), ('-50,48'), ('-50,47'), ('-50,46'), ('-50,45'),
    ('-50,44'), ('-50,43'), ('-50,42'), ('-50,41'), ('-50,40'), ('-50,39'), ('-50,38'), ('-50,37'), ('-50,36'), ('-50,35'), ('-50,34'), ('-50,33'), ('-50,32'), ('-50,31'), ('-50,30'), ('-50,29'), ('-50,28'), ('-50,27'), ('-50,26'), ('-50,25'),
    ('-50,24'), ('-50,23'), ('-50,22'), ('-50,21'), ('-50,20'), ('-50,19'), ('-50,18'), ('-50,17'), ('-50,16'), ('-50,15'), ('-50,14'), ('-50,13'), ('-50,12'), ('-50,11'), ('-50,10'), ('-50,9'), ('-50,8'), ('-50,7'), ('-50,6'), ('-50,5'),
    ('-50,4'), ('-50,3'), ('-50,2'), ('-50,1'), ('-50,0'), ('-50,-1'), ('-50,-2'), ('-50,-3'), ('-50,-4'), ('-50,-5'), ('-50,-6'), ('-50,-7'), ('-50,-8'), ('-50,-9'), ('-50,-10'), ('-50,-11'), ('-50,-12'), ('-50,-13'), ('-50,-14'), ('-50,-15'),
    ('-50,-16'), ('-50,-17'), ('-50,-18'), ('-50,-19'), ('-50,-20'), ('-50,-21'), ('-50,-22'), ('-50,-23'), ('-50,-24'), ('-50,-25'), ('-50,-26'), ('-50,-27'), ('-50,-28'), ('-50,-29'), ('-50,-30'), ('-50,-31'), ('-50,-32'), ('-50,-33'), ('-50,-34'), ('-50,-35'),
    ('-50,-36'), ('-50,-37'), ('-50,-38'), ('-50,-39'), ('-50,-40'), ('-50,-41'), ('-50,-42'), ('-50,-43'), ('-50,-44'), ('-50,-45'), ('-50,-46'), ('-50,-47'), ('-50,-48'), ('-50,-49'), ('-50,-50'), ('-50,-51'), ('-50,-52'), ('-50,-53'), ('-50,-54'), ('-50,-55'),
    ('-50,-56'), ('-50,-57'), ('-50,-58'), ('-50,-59'), ('-50,-60'), ('-50,-61'), ('-50,-62'), ('-50,-63'), ('-50,-64'), ('-50,-65'), ('-50,-66'), ('-50,-67'), ('-50,-68'), ('-50,-69'), ('-50,-70'), ('-50,-71'), ('-50,-72'), ('-50,-73'), ('-50,-96'), ('-50,-98'),
    ('-50,-99'), ('-50,-100'), ('-50,-101'), ('-50,-104'), ('-50,-105'), ('-50,-106'), ('-50,-107'), ('-50,-109'), ('-49,130'), ('-49,129'), ('-49,128'), ('-49,127'), ('-49,126'), ('-49,125'), ('-49,124'), ('-49,123'), ('-49,122'), ('-49,121'), ('-49,118'), ('-49,117'),
    ('-49,116'), ('-49,115'), ('-49,51'), ('-49,50'), ('-49,44'), ('-49,0'), ('-49,-1'), ('-49,-50'), ('-49,-51'), ('-49,-60'), ('-49,-69'), ('-49,-70'), ('-49,-96'), ('-49,-97'), ('-49,-98'), ('-49,-101'), ('-49,-102'), ('-49,-103'), ('-49,-104'), ('-49,-107'),
    ('-49,-108'), ('-49,-109'), ('-48,127'), ('-48,118'), ('-48,115'), ('-48,51'), ('-48,50'), ('-48,44'), ('-48,0'), ('-48,-1'), ('-48,-50'), ('-48,-51'), ('-48,-60'), ('-48,-69'), ('-48,-70'), ('-48,-96'), ('-48,-109'), ('-47,127'), ('-47,118'), ('-47,115'),
    ('-47,51'), ('-47,50'), ('-47,44'), ('-47,41'), ('-47,40'), ('-47,39'), ('-47,38'), ('-47,35'), ('-47,34'), ('-47,33'), ('-47,32'), ('-47,29'), ('-47,28'), ('-47,27'), ('-47,26'), ('-47,23'), ('-47,22'), ('-47,21'), ('-47,20'), ('-47,15'),
    ('-47,14'), ('-47,13'), ('-47,12'), ('-47,9'), ('-47,8'), ('-47,7'), ('-47,6'), ('-47,0'), ('-47,-1'), ('-47,-50'), ('-47,-51'), ('-47,-54'), ('-47,-55'), ('-47,-56'), ('-47,-57'), ('-47,-58'), ('-47,-59'), ('-47,-60'), ('-47,-61'), ('-47,-62'),
    ('-47,-63'), ('-47,-66'), ('-47,-67'), ('-47,-68'), ('-47,-69'), ('-47,-70'), ('-47,-71'), ('-47,-72'), ('-47,-73'), ('-47,-78'), ('-47,-79'), ('-47,-80'), ('-47,-81'), ('-47,-82'), ('-47,-96'), ('-47,-109'), ('-46,130'), ('-46,129'), ('-46,128'), ('-46,127'),
    ('-46,126'), ('-46,125'), ('-46,124'), ('-46,123'), ('-46,122'), ('-46,121'), ('-46,118'), ('-46,117'), ('-46,116'), ('-46,115'), ('-46,51'), ('-46,50'), ('-46,44'), ('-46,41'), ('-46,38'), ('-46,35'), ('-46,32'), ('-46,29'), ('-46,26'), ('-46,23'),
    ('-46,20'), ('-46,15'), ('-46,12'), ('-46,9'), ('-46,6'), ('-46,0'), ('-46,-1'), ('-46,-50'), ('-46,-51'), ('-46,-54'), ('-46,-57'), ('-46,-60'), ('-46,-63'), ('-46,-66'), ('-46,-73'), ('-46,-78'), ('-46,-82'), ('-46,-96'), ('-46,-98'), ('-46,-99'),
    ('-46,-100'), ('-46,-101'), ('-46,-104'), ('-46,-105'), ('-46,-106'), ('-46,-107'), ('-46,-109'), ('-45,130'), ('-45,127'), ('-45,124'), ('-45,121'), ('-45,51'), ('-45,50'), ('-45,44'), ('-45,41'), ('-45,38'), ('-45,35'), ('-45,32'), ('-45,29'), ('-45,26'),
    ('-45,23'), ('-45,20'), ('-45,15'), ('-45,12'), ('-45,9'), ('-45,6'), ('-45,0'), ('-45,-1'), ('-45,-50'), ('-45,-51'), ('-45,-54'), ('-45,-57'), ('-45,-60'), ('-45,-63'), ('-45,-66'), ('-45,-73'), ('-45,-78'), ('-45,-80'), ('-45,-82'), ('-45,-96'),
    ('-45,-97'), ('-45,-98'), ('-45,-101'), ('-45,-102'), ('-45,-103'), ('-45,-104'), ('-45,-107'), ('-45,-108'), ('-45,-109'), ('-44,130'), ('-44,127'), ('-44,124'), ('-44,121'), ('-44,51'), ('-44,50'), ('-44,49'), ('-44,48'), ('-44,47'), ('-44,46'), ('-44,45'),
    ('-44,44'), ('-44,41'), ('-44,40'), ('-44,39'), ('-44,38'), ('-44,37'), ('-44,36'), ('-44,35'), ('-44,34'), ('-44,33'), ('-44,32'), ('-44,29'), ('-44,28'), ('-44,27'), ('-44,26'), ('-44,25'), ('-44,24'), ('-44,23'), ('-44,22'), ('-44,21'),
    ('-44,20'), ('-44,15'), ('-44,14'), ('-44,13'), ('-44,12'), ('-44,11'), ('-44,10'), ('-44,9'), ('-44,8'), ('-44,7'), ('-44,6'), ('-44,0'), ('-44,-1'), ('-44,-50'), ('-44,-51'), ('-44,-54'), ('-44,-57'), ('-44,-60'), ('-44,-61'), ('-44,-62'),
    ('-44,-63'), ('-44,-66'), ('-44,-73'), ('-44,-78'), ('-44,-82'), ('-44,-96'), ('-44,-109'), ('-43,130'), ('-43,129'), ('-43,128'), ('-43,127'), ('-43,124'), ('-43,123'), ('-43,122'), ('-43,121'), ('-43,120'), ('-43,119'), ('-43,118'), ('-43,117'), ('-43,116'),
    ('-43,115'), ('-43,51'), ('-43,50'), ('-43,38'), ('-43,35'), ('-43,23'), ('-43,12'), ('-43,9'), ('-43,0'), ('-43,-1'), ('-43,-50'), ('-43,-51'), ('-43,-54'), ('-43,-57'), ('-43,-60'), ('-43,-66'), ('-43,-73'), ('-43,-78'), ('-43,-79'), ('-43,-80'),
    ('-43,-81'), ('-43,-82'), ('-43,-96'), ('-43,-109'), ('-42,118'), ('-42,115'), ('-42,51'), ('-42,50'), ('-42,38'), ('-42,35'), ('-42,23'), ('-42,12'), ('-42,9'), ('-42,0'), ('-42,-1'), ('-42,-50'), ('-42,-51'), ('-42,-54'), ('-42,-57'), ('-42,-60'),
    ('-42,-66'), ('-42,-73'), ('-42,-96'), ('-42,-109'), ('-41,118'), ('-41,115'), ('-41,51'), ('-41,50'), ('-41,47'), ('-41,46'), ('-41,45'), ('-41,44'), ('-41,41'), ('-41,40'), ('-41,39'), ('-41,38'), ('-41,37'), ('-41,36'), ('-41,35'), ('-41,34'),
    ('-41,33'), ('-41,32'), ('-41,29'), ('-41,28'), ('-41,27'), ('-41,26'), ('-41,25'), ('-41,24'), ('-41,23'), ('-41,22'), ('-41,21'), ('-41,20'), ('-41,15'), ('-41,14'), ('-41,13'), ('-41,12'), ('-41,9'), ('-41,8'), ('-41,7'), ('-41,6'),
    ('-41,0'), ('-41,-1'), ('-41,-50'), ('-41,-51'), ('-41,-54'), ('-41,-57'), ('-41,-60'), ('-41,-61'), ('-41,-62'), ('-41,-66'), ('-41,-73'), ('-41,-96'), ('-41,-109'), ('-40,118'), ('-40,117'), ('-40,116'), ('-40,115'), ('-40,51'), ('-40,50'), ('-40,47'),
    ('-40,44'), ('-40,41'), ('-40,32'), ('-40,29'), ('-40,26'), ('-40,23'), ('-40,20'), ('-40,15'), ('-40,12'), ('-40,9'), ('-40,6'), ('-40,0'), ('-40,-1'), ('-40,-50'), ('-40,-51'), ('-40,-54'), ('-40,-57'), ('-40,-60'), ('-40,-62'), ('-40,-66'),
    ('-40,-73'), ('-40,-78'), ('-40,-79'), ('-40,-80'), ('-40,-81'), ('-40,-82'), ('-40,-83'), ('-40,-84'), ('-40,-85'), ('-40,-86'), ('-40,-90'), ('-40,-91'), ('-40,-92'), ('-40,-93'), ('-40,-96'), ('-40,-98'), ('-40,-99'), ('-40,-100'), ('-40,-101'), ('-40,-104'),
    ('-40,-105'), ('-40,-106'), ('-40,-107'), ('-40,-109'), ('-39,115'), ('-39,51'), ('-39,50'), ('-39,47'), ('-39,44'), ('-39,41'), ('-39,32'), ('-39,29'), ('-39,26'), ('-39,23'), ('-39,20'), ('-39,15'), ('-39,12'), ('-39,9'), ('-39,6'), ('-39,0'),
    ('-39,-1'), ('-39,-50'), ('-39,-51'), ('-39,-54'), ('-39,-55'), ('-39,-56'), ('-39,-57'), ('-39,-60'), ('-39,-61'), ('-39,-62'), ('-39,-66'), ('-39,-67'), ('-39,-68'), ('-39,-69'), ('-39,-70'), ('-39,-71'), ('-39,-72'), ('-39,-73'), ('-39,-78'), ('-39,-86'),
    ('-39,-90'), ('-39,-93'), ('-39,-96'), ('-39,-97'), ('-39,-98'), ('-39,-101'), ('-39,-102'), ('-39,-103'), ('-39,-104'), ('-39,-107'), ('-39,-108'), ('-39,-109'), ('-38,115'), ('-38,51'), ('-38,50'), ('-38,47'), ('-38,46'), ('-38,45'), ('-38,44'), ('-38,43'),
    ('-38,42'), ('-38,41'), ('-38,32'), ('-38,29'), ('-38,28'), ('-38,27'), ('-38,26'), ('-38,23'), ('-38,22'), ('-38,21'), ('-38,20'), ('-38,15'), ('-38,14'), ('-38,13'), ('-38,12'), ('-38,9'), ('-38,8'), ('-38,7'), ('-38,6'), ('-38,0'),
    ('-38,-1'), ('-38,-50'), ('-38,-51'), ('-38,-60'), ('-38,-67'), ('-38,-72'), ('-38,-78'), ('-38,-86'), ('-38,-90'), ('-38,-93'), ('-38,-96'), ('-38,-109'), ('-37,139'), ('-37,138'), ('-37,137'), ('-37,136'), ('-37,135'), ('-37,134'), ('-37,133'), ('-37,132'),
    ('-37,131'), ('-37,130'), ('-37,129'), ('-37,128'), ('-37,127'), ('-37,126'), ('-37,125'), ('-37,124'), ('-37,123'), ('-37,122'), ('-37,121'), ('-37,120'), ('-37,119'), ('-37,118'), ('-37,117'), ('-37,116'), ('-37,115'), ('-37,114'), ('-37,113'), ('-37,112'),
    ('-37,111'), ('-37,110'), ('-37,109'), ('-37,108'), ('-37,107'), ('-37,106'), ('-37,105'), ('-37,104'), ('-37,103'), ('-37,102'), ('-37,101'), ('-37,100'), ('-37,99'), ('-37,98'), ('-37,97'), ('-37,96'), ('-37,95'), ('-37,94'), ('-37,93'), ('-37,51')
;

INSERT INTO explore.road_positions (position) VALUES
    ('-37,50'), ('-37,44'), ('-37,41'), ('-37,32'), ('-37,29'), ('-37,15'), ('-37,6'), ('-37,0'), ('-37,-1'), ('-37,-50'), ('-37,-51'), ('-37,-60'), ('-37,-67'), ('-37,-72'), ('-37,-78'), ('-37,-86'), ('-37,-90'), ('-37,-93'), ('-37,-96'), ('-37,-109'),
    ('-36,139'), ('-36,138'), ('-36,137'), ('-36,136'), ('-36,135'), ('-36,134'), ('-36,133'), ('-36,132'), ('-36,131'), ('-36,130'), ('-36,129'), ('-36,128'), ('-36,127'), ('-36,126'), ('-36,125'), ('-36,124'), ('-36,123'), ('-36,122'), ('-36,121'), ('-36,120'),
    ('-36,119'), ('-36,118'), ('-36,117'), ('-36,116'), ('-36,115'), ('-36,114'), ('-36,113'), ('-36,112'), ('-36,111'), ('-36,110'), ('-36,109'), ('-36,108'), ('-36,107'), ('-36,106'), ('-36,105'), ('-36,104'), ('-36,103'), ('-36,102'), ('-36,101'), ('-36,100'),
    ('-36,99'), ('-36,98'), ('-36,97'), ('-36,96'), ('-36,95'), ('-36,94'), ('-36,93'), ('-36,51'), ('-36,50'), ('-36,44'), ('-36,41'), ('-36,32'), ('-36,29'), ('-36,15'), ('-36,6'), ('-36,0'), ('-36,-1'), ('-36,-50'), ('-36,-51'), ('-36,-54'),
    ('-36,-55'), ('-36,-56'), ('-36,-57'), ('-36,-58'), ('-36,-59'), ('-36,-60'), ('-36,-61'), ('-36,-62'), ('-36,-63'), ('-36,-64'), ('-36,-65'), ('-36,-66'), ('-36,-67'), ('-36,-72'), ('-36,-78'), ('-36,-86'), ('-36,-90'), ('-36,-93'), ('-36,-96'), ('-36,-98'),
    ('-36,-99'), ('-36,-100'), ('-36,-101'), ('-36,-104'), ('-36,-105'), ('-36,-106'), ('-36,-107'), ('-36,-109'), ('-35,139'), ('-35,138'), ('-35,94'), ('-35,93'), ('-35,51'), ('-35,50'), ('-35,47'), ('-35,46'), ('-35,45'), ('-35,44'), ('-35,43'), ('-35,42'),
    ('-35,41'), ('-35,32'), ('-35,29'), ('-35,28'), ('-35,27'), ('-35,26'), ('-35,23'), ('-35,22'), ('-35,21'), ('-35,20'), ('-35,15'), ('-35,12'), ('-35,11'), ('-35,10'), ('-35,9'), ('-35,6'), ('-35,5'), ('-35,4'), ('-35,3'), ('-35,0'),
    ('-35,-1'), ('-35,-50'), ('-35,-51'), ('-35,-54'), ('-35,-67'), ('-35,-72'), ('-35,-78'), ('-35,-86'), ('-35,-90'), ('-35,-93'), ('-35,-96'), ('-35,-97'), ('-35,-98'), ('-35,-101'), ('-35,-102'), ('-35,-103'), ('-35,-104'), ('-35,-107'), ('-35,-108'), ('-35,-109'),
    ('-34,139'), ('-34,138'), ('-34,94'), ('-34,93'), ('-34,51'), ('-34,50'), ('-34,47'), ('-34,44'), ('-34,41'), ('-34,32'), ('-34,29'), ('-34,26'), ('-34,23'), ('-34,20'), ('-34,15'), ('-34,12'), ('-34,9'), ('-34,6'), ('-34,3'), ('-34,0'),
    ('-34,-1'), ('-34,-50'), ('-34,-51'), ('-34,-54'), ('-34,-67'), ('-34,-72'), ('-34,-78'), ('-34,-86'), ('-34,-90'), ('-34,-93'), ('-34,-96'), ('-34,-109'), ('-33,139'), ('-33,138'), ('-33,94'), ('-33,93'), ('-33,51'), ('-33,50'), ('-33,47'), ('-33,44'),
    ('-33,41'), ('-33,32'), ('-33,29'), ('-33,26'), ('-33,23'), ('-33,20'), ('-33,15'), ('-33,12'), ('-33,9'), ('-33,6'), ('-33,3'), ('-33,0'), ('-33,-1'), ('-33,-50'), ('-33,-51'), ('-33,-54'), ('-33,-67'), ('-33,-72'), ('-33,-78'), ('-33,-86'),
    ('-33,-90'), ('-33,-93'), ('-33,-96'), ('-33,-109'), ('-32,139'), ('-32,138'), ('-32,94'), ('-32,93'), ('-32,51'), ('-32,50'), ('-32,47'), ('-32,46'), ('-32,45'), ('-32,44'), ('-32,41'), ('-32,40'), ('-32,39'), ('-32,38'), ('-32,37'), ('-32,36'),
    ('-32,35'), ('-32,34'), ('-32,33'), ('-32,32'), ('-32,31'), ('-32,30'), ('-32,29'), ('-32,28'), ('-32,27'), ('-32,26'), ('-32,23'), ('-32,22'), ('-32,21'), ('-32,20'), ('-32,19'), ('-32,18'), ('-32,17'), ('-32,16'), ('-32,15'), ('-32,12'),
    ('-32,11'), ('-32,10'), ('-32,9'), ('-32,6'), ('-32,5'), ('-32,4'), ('-32,3'), ('-32,2'), ('-32,1'), ('-32,0'), ('-32,-1'), ('-32,-50'), ('-32,-51'), ('-32,-54'), ('-32,-67'), ('-32,-70'), ('-32,-71'), ('-32,-72'), ('-32,-73'), ('-32,-74'),
    ('-32,-78'), ('-32,-79'), ('-32,-80'), ('-32,-81'), ('-32,-82'), ('-32,-83'), ('-32,-84'), ('-32,-85'), ('-32,-86'), ('-32,-90'), ('-32,-91'), ('-32,-92'), ('-32,-93'), ('-32,-96'), ('-32,-98'), ('-32,-99'), ('-32,-100'), ('-32,-101'), ('-32,-104'), ('-32,-105'),
    ('-32,-106'), ('-32,-107'), ('-32,-109'), ('-31,139'), ('-31,138'), ('-31,94'), ('-31,93'), ('-31,51'), ('-31,50'), ('-31,32'), ('-31,29'), ('-31,18'), ('-31,12'), ('-31,0'), ('-31,-1'), ('-31,-50'), ('-31,-51'), ('-31,-54'), ('-31,-67'), ('-31,-70'),
    ('-31,-74'), ('-31,-96'), ('-31,-97'), ('-31,-98'), ('-31,-101'), ('-31,-102'), ('-31,-103'), ('-31,-104'), ('-31,-107'), ('-31,-108'), ('-31,-109'), ('-30,139'), ('-30,138'), ('-30,94'), ('-30,93'), ('-30,51'), ('-30,50'), ('-30,32'), ('-30,29'), ('-30,18'),
    ('-30,12'), ('-30,0'), ('-30,-1'), ('-30,-50'), ('-30,-51'), ('-30,-54'), ('-30,-67'), ('-30,-70'), ('-30,-74'), ('-30,-96'), ('-30,-109'), ('-29,139'), ('-29,138'), ('-29,94'), ('-29,93'), ('-29,51'), ('-29,50'), ('-29,47'), ('-29,46'), ('-29,45'),
    ('-29,44'), ('-29,41'), ('-29,40'), ('-29,39'), ('-29,38'), ('-29,37'), ('-29,36'), ('-29,35'), ('-29,34'), ('-29,33'), ('-29,32'), ('-29,31'), ('-29,30'), ('-29,29'), ('-29,28'), ('-29,27'), ('-29,26'), ('-29,25'), ('-29,24'), ('-29,23'),
    ('-29,22'), ('-29,21'), ('-29,18'), ('-29,17'), ('-29,16'), ('-29,15'), ('-29,12'), ('-29,11'), ('-29,10'), ('-29,9'), ('-29,6'), ('-29,5'), ('-29,4'), ('-29,3'), ('-29,0'), ('-29,-1'), ('-29,-50'), ('-29,-51'), ('-29,-54'), ('-29,-67'),
    ('-29,-70'), ('-29,-74'), ('-29,-78'), ('-29,-79'), ('-29,-80'), ('-29,-81'), ('-29,-82'), ('-29,-83'), ('-29,-84'), ('-29,-85'), ('-29,-86'), ('-29,-87'), ('-29,-88'), ('-29,-89'), ('-29,-90'), ('-29,-91'), ('-29,-92'), ('-29,-93'), ('-29,-96'), ('-29,-109'),
    ('-28,139'), ('-28,138'), ('-28,94'), ('-28,93'), ('-28,51'), ('-28,50'), ('-28,47'), ('-28,44'), ('-28,41'), ('-28,38'), ('-28,35'), ('-28,32'), ('-28,29'), ('-28,21'), ('-28,18'), ('-28,15'), ('-28,12'), ('-28,9'), ('-28,6'), ('-28,3'),
    ('-28,0'), ('-28,-1'), ('-28,-50'), ('-28,-51'), ('-28,-54'), ('-28,-67'), ('-28,-70'), ('-28,-71'), ('-28,-72'), ('-28,-73'), ('-28,-74'), ('-28,-78'), ('-28,-93'), ('-28,-96'), ('-28,-98'), ('-28,-99'), ('-28,-100'), ('-28,-101'), ('-28,-104'), ('-28,-105'),
    ('-28,-106'), ('-28,-107'), ('-28,-109'), ('-27,139'), ('-27,138'), ('-27,94'), ('-27,93'), ('-27,51'), ('-27,50'), ('-27,47'), ('-27,44'), ('-27,41'), ('-27,38'), ('-27,35'), ('-27,32'), ('-27,29'), ('-27,21'), ('-27,18'), ('-27,15'), ('-27,12'),
    ('-27,9'), ('-27,6'), ('-27,3'), ('-27,0'), ('-27,-1'), ('-27,-50'), ('-27,-51'), ('-27,-54'), ('-27,-67'), ('-27,-72'), ('-27,-78'), ('-27,-93'), ('-27,-96'), ('-27,-97'), ('-27,-98'), ('-27,-101'), ('-27,-102'), ('-27,-103'), ('-27,-104'), ('-27,-107'),
    ('-27,-108'), ('-27,-109'), ('-27,-124'), ('-27,-141'), ('-26,139'), ('-26,138'), ('-26,94'), ('-26,93'), ('-26,51'), ('-26,50'), ('-26,47'), ('-26,46'), ('-26,45'), ('-26,44'), ('-26,41'), ('-26,40'), ('-26,39'), ('-26,38'), ('-26,35'), ('-26,34'),
    ('-26,33'), ('-26,32'), ('-26,29'), ('-26,21'), ('-26,18'), ('-26,17'), ('-26,16'), ('-26,15'), ('-26,12'), ('-26,11'), ('-26,10'), ('-26,9'), ('-26,8'), ('-26,7'), ('-26,6'), ('-26,5'), ('-26,4'), ('-26,3'), ('-26,2'), ('-26,1'),
    ('-26,0'), ('-26,-1'), ('-26,-50'), ('-26,-51'), ('-26,-54'), ('-26,-67'), ('-26,-72'), ('-26,-78'), ('-26,-93'), ('-26,-96'), ('-26,-109'), ('-26,-124'), ('-26,-141'), ('-25,139'), ('-25,138'), ('-25,94'), ('-25,93'), ('-25,51'), ('-25,50'), ('-25,44'),
    ('-25,41'), ('-25,29'), ('-25,21'), ('-25,18'), ('-25,0'), ('-25,-1'), ('-25,-50'), ('-25,-51'), ('-25,-54'), ('-25,-67'), ('-25,-70'), ('-25,-71'), ('-25,-72'), ('-25,-73'), ('-25,-74'), ('-25,-78'), ('-25,-93'), ('-25,-96'), ('-25,-109'), ('-25,-124'),
    ('-25,-141'), ('-24,139'), ('-24,138'), ('-24,94'), ('-24,93'), ('-24,51'), ('-24,50'), ('-24,44'), ('-24,41'), ('-24,29'), ('-24,21'), ('-24,18'), ('-24,0'), ('-24,-1'), ('-24,-50'), ('-24,-51'), ('-24,-54'), ('-24,-67'), ('-24,-70'), ('-24,-74'),
    ('-24,-78'), ('-24,-93'), ('-24,-96'), ('-24,-98'), ('-24,-99'), ('-24,-100'), ('-24,-101'), ('-24,-104'), ('-24,-105'), ('-24,-106'), ('-24,-107'), ('-24,-109'), ('-24,-124'), ('-24,-141'), ('-23,139'), ('-23,138'), ('-23,94'), ('-23,93'), ('-23,51'), ('-23,50'),
    ('-23,47'), ('-23,46'), ('-23,45'), ('-23,44'), ('-23,43'), ('-23,42'), ('-23,41'), ('-23,40'), ('-23,39'), ('-23,38'), ('-23,35'), ('-23,34'), ('-23,33'), ('-23,32'), ('-23,29'), ('-23,21'), ('-23,18'), ('-23,17'), ('-23,16'), ('-23,15'),
    ('-23,14'), ('-23,13'), ('-23,12'), ('-23,11'), ('-23,10'), ('-23,9'), ('-23,6'), ('-23,5'), ('-23,4'), ('-23,3'), ('-23,0'), ('-23,-1'), ('-23,-50'), ('-23,-51'), ('-23,-54'), ('-23,-67'), ('-23,-70'), ('-23,-74'), ('-23,-78'), ('-23,-93'),
    ('-23,-96'), ('-23,-97'), ('-23,-98'), ('-23,-101'), ('-23,-102'), ('-23,-103'), ('-23,-104'), ('-23,-107'), ('-23,-108'), ('-23,-109'), ('-23,-124'), ('-23,-141'), ('-22,139'), ('-22,138'), ('-22,94'), ('-22,93'), ('-22,51'), ('-22,50'), ('-22,47'), ('-22,44'),
    ('-22,41'), ('-22,38'), ('-22,35'), ('-22,32'), ('-22,29'), ('-22,21'), ('-22,16'), ('-22,12'), ('-22,9'), ('-22,6'), ('-22,3'), ('-22,0'), ('-22,-1'), ('-22,-50'), ('-22,-51'), ('-22,-54'), ('-22,-67'), ('-22,-70'), ('-22,-74'), ('-22,-78'),
    ('-22,-93'), ('-22,-96'), ('-22,-109'), ('-22,-124'), ('-22,-141'), ('-21,139'), ('-21,138'), ('-21,94'), ('-21,93'), ('-21,51'), ('-21,50'), ('-21,47'), ('-21,44'), ('-21,41'), ('-21,38'), ('-21,35'), ('-21,32'), ('-21,29'), ('-21,28'), ('-21,27'),
    ('-21,26'), ('-21,25'), ('-21,24'), ('-21,23'), ('-21,22'), ('-21,21'), ('-21,20'), ('-21,16'), ('-21,12'), ('-21,9'), ('-21,6'), ('-21,3'), ('-21,0'), ('-21,-1'), ('-21,-50'), ('-21,-51'), ('-21,-54'), ('-21,-55'), ('-21,-56'), ('-21,-57'),
    ('-21,-58'), ('-21,-59'), ('-21,-60'), ('-21,-61'), ('-21,-62'), ('-21,-63'), ('-21,-64'), ('-21,-65'), ('-21,-66'), ('-21,-67'), ('-21,-70'), ('-21,-71'), ('-21,-72'), ('-21,-73'), ('-21,-74'), ('-21,-78'), ('-21,-93'), ('-21,-96'), ('-21,-109'), ('-21,-121'),
    ('-21,-122'), ('-21,-123'), ('-21,-124'), ('-21,-125'), ('-21,-126'), ('-21,-127'), ('-21,-128'), ('-21,-129'), ('-21,-130'), ('-21,-131'), ('-21,-132'), ('-21,-133'), ('-21,-134'), ('-21,-135'), ('-21,-136'), ('-21,-137'), ('-21,-138'), ('-21,-139'), ('-21,-140'), ('-21,-141'),
    ('-21,-142'), ('-21,-143'), ('-21,-144'), ('-20,139'), ('-20,138'), ('-20,94'), ('-20,93'), ('-20,51'), ('-20,50'), ('-20,47'), ('-20,46'), ('-20,45'), ('-20,44'), ('-20,41'), ('-20,40'), ('-20,39'), ('-20,38'), ('-20,35'), ('-20,34'), ('-20,33'),
    ('-20,32'), ('-20,21'), ('-20,20'), ('-20,19'), ('-20,18'), ('-20,17'), ('-20,16'), ('-20,15'), ('-20,14'), ('-20,12'), ('-20,11'), ('-20,10'), ('-20,9'), ('-20,8'), ('-20,7'), ('-20,6'), ('-20,5'), ('-20,4'), ('-20,3'), ('-20,0'),
    ('-20,-1'), ('-20,-50'), ('-20,-51'), ('-20,-60'), ('-20,-61'), ('-20,-72'), ('-20,-78'), ('-20,-93'), ('-20,-96'), ('-20,-98'), ('-20,-99'), ('-20,-100'), ('-20,-101'), ('-20,-104'), ('-20,-105'), ('-20,-106'), ('-20,-107'), ('-20,-109'), ('-20,-121'), ('-20,-131'),
    ('-20,-134'), ('-20,-144'), ('-19,139'), ('-19,138'), ('-19,94'), ('-19,93'), ('-19,51'), ('-19,50'), ('-19,32'), ('-19,20'), ('-19,16'), ('-19,6'), ('-19,0'), ('-19,-1'), ('-19,-50'), ('-19,-51'), ('-19,-60'), ('-19,-61'), ('-19,-72'), ('-19,-78'),
    ('-19,-93'), ('-19,-96'), ('-19,-97'), ('-19,-98'), ('-19,-101'), ('-19,-102'), ('-19,-103'), ('-19,-104'), ('-19,-107'), ('-19,-108'), ('-19,-109'), ('-19,-121'), ('-19,-131'), ('-19,-134'), ('-19,-144'), ('-18,139'), ('-18,138'), ('-18,94'), ('-18,93'), ('-18,51'),
    ('-18,50'), ('-18,32'), ('-18,31'), ('-18,30'), ('-18,29'), ('-18,28'), ('-18,27'), ('-18,26'), ('-18,25'), ('-18,24'), ('-18,23'), ('-18,20'), ('-18,16'), ('-18,6'), ('-18,0'), ('-18,-1'), ('-18,-50'), ('-18,-51'), ('-18,-54'), ('-18,-55'),
    ('-18,-56'), ('-18,-57'), ('-18,-58'), ('-18,-59'), ('-18,-60'), ('-18,-61'), ('-18,-62'), ('-18,-63'), ('-18,-64'), ('-18,-65'), ('-18,-66'), ('-18,-67'), ('-18,-70'), ('-18,-71'), ('-18,-72'), ('-18,-73'), ('-18,-74'), ('-18,-78'), ('-18,-93'), ('-18,-96'),
    ('-18,-109'), ('-18,-121'), ('-18,-122'), ('-18,-123'), ('-18,-124'), ('-18,-125'), ('-18,-126'), ('-18,-127'), ('-18,-128'), ('-18,-129'), ('-18,-130'), ('-18,-131'), ('-18,-132'), ('-18,-133'), ('-18,-134'), ('-18,-135'), ('-18,-136'), ('-18,-137'), ('-18,-138'), ('-18,-139'),
    ('-18,-140'), ('-18,-141'), ('-18,-142'), ('-18,-143'), ('-18,-144'), ('-17,139'), ('-17,138'), ('-17,94'), ('-17,93'), ('-17,51'), ('-17,50'), ('-17,32'), ('-17,29'), ('-17,26'), ('-17,23'), ('-17,20'), ('-17,16'), ('-17,15'), ('-17,14'), ('-17,12'),
    ('-17,11'), ('-17,10'), ('-17,9'), ('-17,6'), ('-17,5'), ('-17,4'), ('-17,3'), ('-17,0'), ('-17,-1'), ('-17,-50'), ('-17,-51'), ('-17,-54'), ('-17,-67'), ('-17,-70'), ('-17,-74'), ('-17,-78'), ('-17,-93'), ('-17,-96'), ('-17,-109'), ('-17,-121'),
    ('-17,-124'), ('-17,-141'), ('-17,-144'), ('-16,139'), ('-16,138'), ('-16,94'), ('-16,93'), ('-16,51'), ('-16,50'), ('-16,32'), ('-16,29'), ('-16,26'), ('-16,23'), ('-16,22'), ('-16,21'), ('-16,20'), ('-16,19'), ('-16,18'), ('-16,17'), ('-16,14')
;

INSERT INTO explore.road_positions (position) VALUES
    ('-16,12'), ('-16,9'), ('-16,6'), ('-16,3'), ('-16,0'), ('-16,-1'), ('-16,-50'), ('-16,-51'), ('-16,-54'), ('-16,-67'), ('-16,-70'), ('-16,-74'), ('-16,-78'), ('-16,-93'), ('-16,-96'), ('-16,-98'), ('-16,-99'), ('-16,-100'), ('-16,-101'), ('-16,-104'),
    ('-16,-105'), ('-16,-106'), ('-16,-107'), ('-16,-109'), ('-16,-121'), ('-16,-124'), ('-16,-141'), ('-16,-144'), ('-15,139'), ('-15,138'), ('-15,94'), ('-15,93'), ('-15,51'), ('-15,50'), ('-15,47'), ('-15,46'), ('-15,45'), ('-15,44'), ('-15,41'), ('-15,40'),
    ('-15,39'), ('-15,38'), ('-15,37'), ('-15,36'), ('-15,35'), ('-15,34'), ('-15,33'), ('-15,32'), ('-15,29'), ('-15,28'), ('-15,27'), ('-15,26'), ('-15,23'), ('-15,20'), ('-15,17'), ('-15,14'), ('-15,12'), ('-15,9'), ('-15,6'), ('-15,3'),
    ('-15,0'), ('-15,-1'), ('-15,-50'), ('-15,-51'), ('-15,-54'), ('-15,-67'), ('-15,-70'), ('-15,-74'), ('-15,-78'), ('-15,-93'), ('-15,-96'), ('-15,-97'), ('-15,-98'), ('-15,-101'), ('-15,-102'), ('-15,-103'), ('-15,-104'), ('-15,-107'), ('-15,-108'), ('-15,-109'),
    ('-15,-121'), ('-15,-124'), ('-15,-127'), ('-15,-128'), ('-15,-129'), ('-15,-130'), ('-15,-131'), ('-15,-132'), ('-15,-133'), ('-15,-134'), ('-15,-135'), ('-15,-136'), ('-15,-137'), ('-15,-138'), ('-15,-141'), ('-15,-144'), ('-14,139'), ('-14,138'), ('-14,94'), ('-14,93'),
    ('-14,51'), ('-14,50'), ('-14,47'), ('-14,44'), ('-14,41'), ('-14,38'), ('-14,23'), ('-14,20'), ('-14,17'), ('-14,16'), ('-14,15'), ('-14,14'), ('-14,13'), ('-14,12'), ('-14,11'), ('-14,10'), ('-14,9'), ('-14,6'), ('-14,5'), ('-14,4'),
    ('-14,3'), ('-14,0'), ('-14,-1'), ('-14,-50'), ('-14,-51'), ('-14,-54'), ('-14,-67'), ('-14,-70'), ('-14,-71'), ('-14,-72'), ('-14,-73'), ('-14,-74'), ('-14,-78'), ('-14,-79'), ('-14,-80'), ('-14,-81'), ('-14,-82'), ('-14,-83'), ('-14,-84'), ('-14,-85'),
    ('-14,-86'), ('-14,-87'), ('-14,-88'), ('-14,-89'), ('-14,-90'), ('-14,-91'), ('-14,-92'), ('-14,-93'), ('-14,-96'), ('-14,-109'), ('-14,-121'), ('-14,-124'), ('-14,-127'), ('-14,-131'), ('-14,-134'), ('-14,-138'), ('-14,-141'), ('-14,-144'), ('-13,139'), ('-13,138'),
    ('-13,94'), ('-13,93'), ('-13,51'), ('-13,50'), ('-13,47'), ('-13,44'), ('-13,41'), ('-13,38'), ('-13,23'), ('-13,14'), ('-13,9'), ('-13,6'), ('-13,0'), ('-13,-1'), ('-13,-50'), ('-13,-51'), ('-13,-54'), ('-13,-67'), ('-13,-72'), ('-13,-96'),
    ('-13,-109'), ('-13,-110'), ('-13,-111'), ('-13,-112'), ('-13,-113'), ('-13,-114'), ('-13,-115'), ('-13,-116'), ('-13,-117'), ('-13,-118'), ('-13,-119'), ('-13,-120'), ('-13,-121'), ('-13,-124'), ('-13,-125'), ('-13,-126'), ('-13,-127'), ('-13,-128'), ('-13,-129'), ('-13,-131'),
    ('-13,-134'), ('-13,-136'), ('-13,-137'), ('-13,-138'), ('-13,-139'), ('-13,-140'), ('-13,-141'), ('-13,-144'), ('-13,-145'), ('-13,-146'), ('-13,-147'), ('-13,-148'), ('-13,-149'), ('-13,-150'), ('-12,139'), ('-12,138'), ('-12,94'), ('-12,93'), ('-12,51'), ('-12,50'),
    ('-12,47'), ('-12,46'), ('-12,45'), ('-12,44'), ('-12,43'), ('-12,42'), ('-12,41'), ('-12,40'), ('-12,39'), ('-12,38'), ('-12,35'), ('-12,34'), ('-12,33'), ('-12,32'), ('-12,31'), ('-12,30'), ('-12,29'), ('-12,28'), ('-12,27'), ('-12,26'),
    ('-12,23'), ('-12,22'), ('-12,21'), ('-12,20'), ('-12,17'), ('-12,16'), ('-12,15'), ('-12,14'), ('-12,9'), ('-12,6'), ('-12,0'), ('-12,-1'), ('-12,-50'), ('-12,-51'), ('-12,-54'), ('-12,-67'), ('-12,-72'), ('-12,-96'), ('-12,-98'), ('-12,-99'),
    ('-12,-100'), ('-12,-101'), ('-12,-104'), ('-12,-105'), ('-12,-106'), ('-12,-107'), ('-12,-109'), ('-12,-121'), ('-12,-124'), ('-12,-127'), ('-12,-129'), ('-12,-131'), ('-12,-132'), ('-12,-133'), ('-12,-134'), ('-12,-136'), ('-12,-138'), ('-12,-141'), ('-12,-144'), ('-11,139'),
    ('-11,138'), ('-11,94'), ('-11,93'), ('-11,92'), ('-11,91'), ('-11,90'), ('-11,89'), ('-11,88'), ('-11,87'), ('-11,86'), ('-11,85'), ('-11,84'), ('-11,83'), ('-11,82'), ('-11,81'), ('-11,80'), ('-11,79'), ('-11,78'), ('-11,77'), ('-11,76'),
    ('-11,75'), ('-11,74'), ('-11,73'), ('-11,72'), ('-11,71'), ('-11,51'), ('-11,50'), ('-11,44'), ('-11,35'), ('-11,32'), ('-11,29'), ('-11,26'), ('-11,23'), ('-11,20'), ('-11,17'), ('-11,14'), ('-11,11'), ('-11,10'), ('-11,9'), ('-11,8'),
    ('-11,7'), ('-11,6'), ('-11,5'), ('-11,4'), ('-11,3'), ('-11,2'), ('-11,1'), ('-11,0'), ('-11,-1'), ('-11,-2'), ('-11,-3'), ('-11,-4'), ('-11,-5'), ('-11,-6'), ('-11,-7'), ('-11,-8'), ('-11,-9'), ('-11,-10'), ('-11,-11'), ('-11,-50'),
    ('-11,-51'), ('-11,-54'), ('-11,-67'), ('-11,-70'), ('-11,-71'), ('-11,-72'), ('-11,-73'), ('-11,-74'), ('-11,-75'), ('-11,-76'), ('-11,-77'), ('-11,-78'), ('-11,-79'), ('-11,-80'), ('-11,-81'), ('-11,-82'), ('-11,-83'), ('-11,-84'), ('-11,-85'), ('-11,-86'),
    ('-11,-87'), ('-11,-88'), ('-11,-89'), ('-11,-90'), ('-11,-91'), ('-11,-92'), ('-11,-93'), ('-11,-96'), ('-11,-97'), ('-11,-98'), ('-11,-101'), ('-11,-102'), ('-11,-103'), ('-11,-104'), ('-11,-107'), ('-11,-108'), ('-11,-109'), ('-11,-121'), ('-11,-124'), ('-11,-127'),
    ('-11,-129'), ('-11,-136'), ('-11,-138'), ('-11,-141'), ('-11,-144'), ('-10,139'), ('-10,138'), ('-10,94'), ('-10,93'), ('-10,92'), ('-10,91'), ('-10,90'), ('-10,89'), ('-10,88'), ('-10,87'), ('-10,86'), ('-10,85'), ('-10,84'), ('-10,83'), ('-10,82'),
    ('-10,81'), ('-10,80'), ('-10,79'), ('-10,78'), ('-10,77'), ('-10,76'), ('-10,75'), ('-10,74'), ('-10,73'), ('-10,72'), ('-10,71'), ('-10,51'), ('-10,50'), ('-10,44'), ('-10,35'), ('-10,32'), ('-10,29'), ('-10,26'), ('-10,23'), ('-10,20'),
    ('-10,17'), ('-10,14'), ('-10,11'), ('-10,10'), ('-10,9'), ('-10,8'), ('-10,7'), ('-10,6'), ('-10,5'), ('-10,4'), ('-10,3'), ('-10,2'), ('-10,1'), ('-10,0'), ('-10,-1'), ('-10,-2'), ('-10,-3'), ('-10,-4'), ('-10,-5'), ('-10,-6'),
    ('-10,-7'), ('-10,-8'), ('-10,-9'), ('-10,-10'), ('-10,-11'), ('-10,-50'), ('-10,-51'), ('-10,-54'), ('-10,-67'), ('-10,-70'), ('-10,-71'), ('-10,-72'), ('-10,-73'), ('-10,-74'), ('-10,-75'), ('-10,-76'), ('-10,-77'), ('-10,-78'), ('-10,-79'), ('-10,-80'),
    ('-10,-81'), ('-10,-82'), ('-10,-83'), ('-10,-84'), ('-10,-85'), ('-10,-86'), ('-10,-87'), ('-10,-88'), ('-10,-89'), ('-10,-90'), ('-10,-91'), ('-10,-92'), ('-10,-93'), ('-10,-94'), ('-10,-95'), ('-10,-96'), ('-10,-109'), ('-10,-121'), ('-10,-122'), ('-10,-123'),
    ('-10,-124'), ('-10,-127'), ('-10,-129'), ('-10,-136'), ('-10,-138'), ('-10,-141'), ('-10,-142'), ('-10,-143'), ('-10,-144'), ('-9,139'), ('-9,138'), ('-9,94'), ('-9,93'), ('-9,72'), ('-9,71'), ('-9,51'), ('-9,50'), ('-9,47'), ('-9,46'), ('-9,45'),
    ('-9,44'), ('-9,43'), ('-9,42'), ('-9,41'), ('-9,40'), ('-9,39'), ('-9,38'), ('-9,35'), ('-9,34'), ('-9,33'), ('-9,32'), ('-9,29'), ('-9,28'), ('-9,27'), ('-9,26'), ('-9,23'), ('-9,22'), ('-9,21'), ('-9,20'), ('-9,17'),
    ('-9,16'), ('-9,15'), ('-9,14'), ('-9,13'), ('-9,12'), ('-9,11'), ('-9,10'), ('-9,-10'), ('-9,-11'), ('-9,-50'), ('-9,-51'), ('-9,-54'), ('-9,-67'), ('-9,-70'), ('-9,-71'), ('-9,-92'), ('-9,-93'), ('-9,-96'), ('-9,-109'), ('-9,-121'),
    ('-9,-124'), ('-9,-127'), ('-9,-129'), ('-9,-130'), ('-9,-131'), ('-9,-132'), ('-9,-133'), ('-9,-134'), ('-9,-135'), ('-9,-136'), ('-9,-138'), ('-9,-141'), ('-9,-144'), ('-8,139'), ('-8,138'), ('-8,94'), ('-8,93'), ('-8,72'), ('-8,71'), ('-8,51'),
    ('-8,50'), ('-8,47'), ('-8,44'), ('-8,41'), ('-8,38'), ('-8,26'), ('-8,20'), ('-8,11'), ('-8,10'), ('-8,-10'), ('-8,-11'), ('-8,-50'), ('-8,-51'), ('-8,-54'), ('-8,-67'), ('-8,-70'), ('-8,-71'), ('-8,-92'), ('-8,-93'), ('-8,-96'),
    ('-8,-98'), ('-8,-99'), ('-8,-100'), ('-8,-101'), ('-8,-104'), ('-8,-105'), ('-8,-106'), ('-8,-107'), ('-8,-109'), ('-8,-121'), ('-8,-124'), ('-8,-127'), ('-8,-138'), ('-8,-141'), ('-8,-144'), ('-7,139'), ('-7,138'), ('-7,94'), ('-7,93'), ('-7,72'),
    ('-7,71'), ('-7,51'), ('-7,50'), ('-7,47'), ('-7,44'), ('-7,41'), ('-7,38'), ('-7,26'), ('-7,20'), ('-7,11'), ('-7,10'), ('-7,-10'), ('-7,-11'), ('-7,-50'), ('-7,-51'), ('-7,-54'), ('-7,-67'), ('-7,-70'), ('-7,-71'), ('-7,-92'),
    ('-7,-93'), ('-7,-96'), ('-7,-97'), ('-7,-98'), ('-7,-101'), ('-7,-102'), ('-7,-103'), ('-7,-104'), ('-7,-107'), ('-7,-108'), ('-7,-109'), ('-7,-121'), ('-7,-124'), ('-7,-127'), ('-7,-138'), ('-7,-141'), ('-7,-144'), ('-6,139'), ('-6,138'), ('-6,94'),
    ('-6,93'), ('-6,72'), ('-6,71'), ('-6,51'), ('-6,50'), ('-6,47'), ('-6,46'), ('-6,45'), ('-6,44'), ('-6,41'), ('-6,40'), ('-6,39'), ('-6,38'), ('-6,37'), ('-6,36'), ('-6,35'), ('-6,34'), ('-6,33'), ('-6,32'), ('-6,29'),
    ('-6,28'), ('-6,27'), ('-6,26'), ('-6,23'), ('-6,22'), ('-6,21'), ('-6,20'), ('-6,19'), ('-6,18'), ('-6,17'), ('-6,16'), ('-6,15'), ('-6,14'), ('-6,13'), ('-6,12'), ('-6,11'), ('-6,10'), ('-6,-10'), ('-6,-11'), ('-6,-50'),
    ('-6,-51'), ('-6,-54'), ('-6,-67'), ('-6,-70'), ('-6,-71'), ('-6,-92'), ('-6,-93'), ('-6,-96'), ('-6,-109'), ('-6,-121'), ('-6,-124'), ('-6,-127'), ('-6,-128'), ('-6,-129'), ('-6,-130'), ('-6,-131'), ('-6,-132'), ('-6,-133'), ('-6,-134'), ('-6,-135'),
    ('-6,-136'), ('-6,-137'), ('-6,-138'), ('-6,-141'), ('-6,-144'), ('-5,139'), ('-5,138'), ('-5,94'), ('-5,93'), ('-5,72'), ('-5,71'), ('-5,51'), ('-5,50'), ('-5,35'), ('-5,32'), ('-5,29'), ('-5,26'), ('-5,23'), ('-5,20'), ('-5,17'),
    ('-5,14'), ('-5,11'), ('-5,10'), ('-5,-10'), ('-5,-11'), ('-5,-50'), ('-5,-51'), ('-5,-54'), ('-5,-67'), ('-5,-70'), ('-5,-71'), ('-5,-92'), ('-5,-93'), ('-5,-96'), ('-5,-109'), ('-5,-121'), ('-5,-124'), ('-5,-131'), ('-5,-134'), ('-5,-141'),
    ('-5,-144'), ('-4,139'), ('-4,138'), ('-4,94'), ('-4,93'), ('-4,72'), ('-4,71'), ('-4,51'), ('-4,50'), ('-4,35'), ('-4,32'), ('-4,29'), ('-4,26'), ('-4,23'), ('-4,20'), ('-4,17'), ('-4,14'), ('-4,11'), ('-4,10'), ('-4,-10'),
    ('-4,-11'), ('-4,-50'), ('-4,-51'), ('-4,-54'), ('-4,-67'), ('-4,-70'), ('-4,-71'), ('-4,-92'), ('-4,-93'), ('-4,-96'), ('-4,-98'), ('-4,-99'), ('-4,-100'), ('-4,-101'), ('-4,-104'), ('-4,-105'), ('-4,-106'), ('-4,-107'), ('-4,-109'), ('-4,-121'),
    ('-4,-124'), ('-4,-131'), ('-4,-134'), ('-4,-141'), ('-4,-144'), ('-3,139'), ('-3,138'), ('-3,94'), ('-3,93'), ('-3,72'), ('-3,71'), ('-3,51'), ('-3,50'), ('-3,35'), ('-3,34'), ('-3,33'), ('-3,32'), ('-3,29'), ('-3,28'), ('-3,27'),
    ('-3,26'), ('-3,23'), ('-3,22'), ('-3,21'), ('-3,20'), ('-3,17'), ('-3,16'), ('-3,15'), ('-3,14'), ('-3,11'), ('-3,10'), ('-3,-10'), ('-3,-11'), ('-3,-50'), ('-3,-51'), ('-3,-54'), ('-3,-55'), ('-3,-56'), ('-3,-57'), ('-3,-58'),
    ('-3,-59'), ('-3,-60'), ('-3,-61'), ('-3,-62'), ('-3,-63'), ('-3,-64'), ('-3,-65'), ('-3,-66'), ('-3,-67'), ('-3,-70'), ('-3,-71'), ('-3,-92'), ('-3,-93'), ('-3,-96'), ('-3,-97'), ('-3,-98'), ('-3,-101'), ('-3,-102'), ('-3,-103'), ('-3,-104'),
    ('-3,-107'), ('-3,-108'), ('-3,-109'), ('-3,-121'), ('-3,-122'), ('-3,-123'), ('-3,-124'), ('-3,-125'), ('-3,-126'), ('-3,-127'), ('-3,-128'), ('-3,-129'), ('-3,-130'), ('-3,-131'), ('-3,-132'), ('-3,-133'), ('-3,-134'), ('-3,-135'), ('-3,-136'), ('-3,-137'),
    ('-3,-138'), ('-3,-139'), ('-3,-140'), ('-3,-141'), ('-3,-142'), ('-3,-143'), ('-3,-144'), ('-2,139'), ('-2,138'), ('-2,94'), ('-2,93'), ('-2,72'), ('-2,71'), ('-2,51'), ('-2,50'), ('-2,32'), ('-2,26'), ('-2,11'), ('-2,10'), ('-2,-10'),
    ('-2,-11'), ('-2,-50'), ('-2,-51'), ('-2,-57'), ('-2,-64'), ('-2,-70'), ('-2,-71'), ('-2,-92'), ('-2,-93'), ('-2,-96'), ('-2,-109'), ('-2,-124'), ('-2,-131'), ('-2,-134'), ('-2,-141'), ('-1,139'), ('-1,138'), ('-1,94'), ('-1,93'), ('-1,72'),
    ('-1,71'), ('-1,51'), ('-1,50'), ('-1,32'), ('-1,26'), ('-1,11'), ('-1,10'), ('-1,-10'), ('-1,-11'), ('-1,-50'), ('-1,-51'), ('-1,-57'), ('-1,-64'), ('-1,-70'), ('-1,-71'), ('-1,-92'), ('-1,-93'), ('-1,-96'), ('-1,-109'), ('-1,-124'),
    ('-1,-131'), ('-1,-134'), ('-1,-141'), ('0,139'), ('0,138'), ('0,94'), ('0,93'), ('0,72'), ('0,71'), ('0,70'), ('0,69'), ('0,68'), ('0,67'), ('0,66'), ('0,65'), ('0,64'), ('0,63'), ('0,62'), ('0,61'), ('0,60'),
    ('0,59'), ('0,58'), ('0,57'), ('0,56'), ('0,55'), ('0,54'), ('0,53'), ('0,52'), ('0,51'), ('0,50'), ('0,49'), ('0,48'), ('0,47'), ('0,46'), ('0,45'), ('0,44'), ('0,43'), ('0,42'), ('0,41'), ('0,40'),
    ('0,39'), ('0,38'), ('0,37'), ('0,36'), ('0,35'), ('0,34'), ('0,33'), ('0,32'), ('0,31'), ('0,30'), ('0,29'), ('0,28'), ('0,27'), ('0,26'), ('0,25'), ('0,24'), ('0,23'), ('0,22'), ('0,21'), ('0,20'),
    ('0,19'), ('0,18'), ('0,17'), ('0,16'), ('0,15'), ('0,14'), ('0,13'), ('0,11'), ('0,10'), ('0,-10'), ('0,-11'), ('0,-12'), ('0,-13'), ('0,-14'), ('0,-15'), ('0,-16'), ('0,-17'), ('0,-18'), ('0,-19'), ('0,-20')
;

INSERT INTO explore.road_positions (position) VALUES
    ('0,-21'), ('0,-22'), ('0,-23'), ('0,-24'), ('0,-25'), ('0,-26'), ('0,-27'), ('0,-28'), ('0,-29'), ('0,-30'), ('0,-31'), ('0,-32'), ('0,-33'), ('0,-34'), ('0,-35'), ('0,-36'), ('0,-37'), ('0,-38'), ('0,-39'), ('0,-40'),
    ('0,-41'), ('0,-42'), ('0,-43'), ('0,-44'), ('0,-45'), ('0,-46'), ('0,-47'), ('0,-48'), ('0,-49'), ('0,-50'), ('0,-51'), ('0,-52'), ('0,-53'), ('0,-54'), ('0,-55'), ('0,-56'), ('0,-57'), ('0,-58'), ('0,-59'), ('0,-60'),
    ('0,-61'), ('0,-62'), ('0,-63'), ('0,-64'), ('0,-65'), ('0,-66'), ('0,-67'), ('0,-68'), ('0,-70'), ('0,-71'), ('0,-92'), ('0,-93'), ('0,-94'), ('0,-95'), ('0,-96'), ('0,-97'), ('0,-98'), ('0,-99'), ('0,-100'), ('0,-101'),
    ('0,-102'), ('0,-103'), ('0,-104'), ('0,-105'), ('0,-106'), ('0,-107'), ('0,-108'), ('0,-109'), ('0,-110'), ('0,-111'), ('0,-112'), ('0,-113'), ('0,-114'), ('0,-115'), ('0,-116'), ('0,-117'), ('0,-118'), ('0,-119'), ('0,-120'), ('0,-121'),
    ('0,-122'), ('0,-123'), ('0,-124'), ('0,-125'), ('0,-126'), ('0,-127'), ('0,-128'), ('0,-129'), ('0,-130'), ('0,-131'), ('0,-132'), ('0,-133'), ('0,-134'), ('0,-135'), ('0,-136'), ('0,-137'), ('0,-138'), ('0,-139'), ('0,-140'), ('0,-141'),
    ('0,-142'), ('0,-143'), ('0,-144'), ('0,-145'), ('0,-146'), ('0,-147'), ('0,-148'), ('0,-149'), ('0,-150'), ('1,139'), ('1,138'), ('1,137'), ('1,136'), ('1,135'), ('1,134'), ('1,133'), ('1,94'), ('1,93'), ('1,72'), ('1,71'),
    ('1,70'), ('1,69'), ('1,68'), ('1,67'), ('1,66'), ('1,65'), ('1,64'), ('1,63'), ('1,62'), ('1,61'), ('1,60'), ('1,59'), ('1,58'), ('1,57'), ('1,56'), ('1,55'), ('1,54'), ('1,53'), ('1,52'), ('1,51'),
    ('1,50'), ('1,49'), ('1,48'), ('1,47'), ('1,46'), ('1,45'), ('1,44'), ('1,43'), ('1,42'), ('1,41'), ('1,40'), ('1,39'), ('1,38'), ('1,37'), ('1,36'), ('1,35'), ('1,34'), ('1,33'), ('1,32'), ('1,31'),
    ('1,30'), ('1,29'), ('1,28'), ('1,27'), ('1,26'), ('1,25'), ('1,24'), ('1,23'), ('1,22'), ('1,21'), ('1,20'), ('1,19'), ('1,18'), ('1,17'), ('1,16'), ('1,15'), ('1,14'), ('1,13'), ('1,12'), ('1,11'),
    ('1,10'), ('1,-10'), ('1,-11'), ('1,-12'), ('1,-13'), ('1,-14'), ('1,-15'), ('1,-16'), ('1,-17'), ('1,-18'), ('1,-19'), ('1,-20'), ('1,-21'), ('1,-22'), ('1,-23'), ('1,-24'), ('1,-25'), ('1,-26'), ('1,-27'), ('1,-28'),
    ('1,-29'), ('1,-30'), ('1,-31'), ('1,-32'), ('1,-33'), ('1,-34'), ('1,-35'), ('1,-36'), ('1,-37'), ('1,-38'), ('1,-39'), ('1,-40'), ('1,-41'), ('1,-42'), ('1,-43'), ('1,-44'), ('1,-45'), ('1,-46'), ('1,-47'), ('1,-48'),
    ('1,-49'), ('1,-50'), ('1,-51'), ('1,-52'), ('1,-53'), ('1,-54'), ('1,-55'), ('1,-56'), ('1,-57'), ('1,-58'), ('1,-59'), ('1,-60'), ('1,-61'), ('1,-62'), ('1,-63'), ('1,-64'), ('1,-65'), ('1,-66'), ('1,-67'), ('1,-68'),
    ('1,-69'), ('1,-70'), ('1,-71'), ('1,-92'), ('1,-93'), ('1,-94'), ('1,-95'), ('1,-96'), ('1,-97'), ('1,-98'), ('1,-99'), ('1,-100'), ('1,-101'), ('1,-102'), ('1,-103'), ('1,-104'), ('1,-105'), ('1,-106'), ('1,-107'), ('1,-108'),
    ('1,-109'), ('1,-110'), ('1,-111'), ('1,-112'), ('1,-113'), ('1,-114'), ('1,-115'), ('1,-116'), ('1,-117'), ('1,-118'), ('1,-119'), ('1,-120'), ('1,-121'), ('1,-122'), ('1,-123'), ('1,-124'), ('1,-125'), ('1,-126'), ('1,-127'), ('1,-128'),
    ('1,-129'), ('1,-130'), ('1,-131'), ('1,-132'), ('1,-133'), ('1,-134'), ('1,-135'), ('1,-136'), ('1,-137'), ('1,-138'), ('1,-139'), ('1,-140'), ('1,-141'), ('1,-142'), ('1,-143'), ('1,-144'), ('1,-145'), ('1,-146'), ('1,-147'), ('1,-148'),
    ('1,-149'), ('1,-150'), ('2,139'), ('2,138'), ('2,137'), ('2,136'), ('2,135'), ('2,134'), ('2,133'), ('2,94'), ('2,93'), ('2,72'), ('2,71'), ('2,51'), ('2,50'), ('2,11'), ('2,10'), ('2,-10'), ('2,-11'), ('2,-14'),
    ('2,-17'), ('2,-20'), ('2,-23'), ('2,-50'), ('2,-51'), ('2,-57'), ('2,-64'), ('2,-70'), ('2,-71'), ('2,-92'), ('2,-93'), ('2,-96'), ('2,-109'), ('2,-124'), ('2,-131'), ('2,-134'), ('2,-141'), ('3,139'), ('3,138'), ('3,134'),
    ('3,133'), ('3,94'), ('3,93'), ('3,72'), ('3,71'), ('3,51'), ('3,50'), ('3,11'), ('3,10'), ('3,-10'), ('3,-11'), ('3,-14'), ('3,-17'), ('3,-20'), ('3,-23'), ('3,-50'), ('3,-51'), ('3,-57'), ('3,-64'), ('3,-70'),
    ('3,-71'), ('3,-92'), ('3,-93'), ('3,-96'), ('3,-109'), ('3,-124'), ('3,-131'), ('3,-134'), ('3,-141'), ('4,139'), ('4,138'), ('4,134'), ('4,133'), ('4,94'), ('4,93'), ('4,72'), ('4,71'), ('4,51'), ('4,50'), ('4,11'),
    ('4,10'), ('4,-10'), ('4,-11'), ('4,-12'), ('4,-13'), ('4,-14'), ('4,-17'), ('4,-20'), ('4,-23'), ('4,-24'), ('4,-25'), ('4,-50'), ('4,-51'), ('4,-54'), ('4,-55'), ('4,-56'), ('4,-57'), ('4,-58'), ('4,-59'), ('4,-60'),
    ('4,-63'), ('4,-64'), ('4,-65'), ('4,-66'), ('4,-67'), ('4,-70'), ('4,-71'), ('4,-92'), ('4,-93'), ('4,-96'), ('4,-97'), ('4,-98'), ('4,-101'), ('4,-102'), ('4,-103'), ('4,-104'), ('4,-107'), ('4,-108'), ('4,-109'), ('4,-121'),
    ('4,-122'), ('4,-123'), ('4,-124'), ('4,-125'), ('4,-126'), ('4,-127'), ('4,-128'), ('4,-129'), ('4,-130'), ('4,-131'), ('4,-132'), ('4,-133'), ('4,-134'), ('4,-135'), ('4,-136'), ('4,-137'), ('4,-138'), ('4,-139'), ('4,-140'), ('4,-141'),
    ('4,-142'), ('4,-143'), ('4,-144'), ('5,139'), ('5,138'), ('5,134'), ('5,133'), ('5,94'), ('5,93'), ('5,72'), ('5,71'), ('5,51'), ('5,50'), ('5,11'), ('5,10'), ('5,-10'), ('5,-11'), ('5,-14'), ('5,-17'), ('5,-20'),
    ('5,-23'), ('5,-50'), ('5,-51'), ('5,-54'), ('5,-60'), ('5,-63'), ('5,-67'), ('5,-70'), ('5,-71'), ('5,-92'), ('5,-93'), ('5,-96'), ('5,-98'), ('5,-99'), ('5,-100'), ('5,-101'), ('5,-104'), ('5,-105'), ('5,-106'), ('5,-107'),
    ('5,-109'), ('5,-121'), ('5,-124'), ('5,-131'), ('5,-134'), ('5,-141'), ('5,-144'), ('6,139'), ('6,138'), ('6,134'), ('6,133'), ('6,94'), ('6,93'), ('6,72'), ('6,71'), ('6,51'), ('6,50'), ('6,11'), ('6,10'), ('6,-10'),
    ('6,-11'), ('6,-14'), ('6,-15'), ('6,-16'), ('6,-17'), ('6,-20'), ('6,-23'), ('6,-24'), ('6,-25'), ('6,-26'), ('6,-50'), ('6,-51'), ('6,-54'), ('6,-60'), ('6,-63'), ('6,-67'), ('6,-70'), ('6,-71'), ('6,-92'), ('6,-93'),
    ('6,-96'), ('6,-109'), ('6,-121'), ('6,-124'), ('6,-131'), ('6,-134'), ('6,-141'), ('6,-144'), ('7,139'), ('7,138'), ('7,134'), ('7,133'), ('7,94'), ('7,93'), ('7,72'), ('7,71'), ('7,51'), ('7,50'), ('7,11'), ('7,10'),
    ('7,-10'), ('7,-11'), ('7,-14'), ('7,-17'), ('7,-20'), ('7,-23'), ('7,-26'), ('7,-50'), ('7,-51'), ('7,-52'), ('7,-53'), ('7,-54'), ('7,-60'), ('7,-61'), ('7,-62'), ('7,-63'), ('7,-67'), ('7,-70'), ('7,-71'), ('7,-92'),
    ('7,-93'), ('7,-96'), ('7,-109'), ('7,-121'), ('7,-124'), ('7,-127'), ('7,-128'), ('7,-129'), ('7,-130'), ('7,-131'), ('7,-132'), ('7,-133'), ('7,-134'), ('7,-135'), ('7,-136'), ('7,-137'), ('7,-138'), ('7,-141'), ('7,-144'), ('8,139'),
    ('8,138'), ('8,134'), ('8,133'), ('8,94'), ('8,93'), ('8,72'), ('8,71'), ('8,51'), ('8,50'), ('8,11'), ('8,10'), ('8,-10'), ('8,-11'), ('8,-14'), ('8,-17'), ('8,-18'), ('8,-19'), ('8,-20'), ('8,-23'), ('8,-50'),
    ('8,-51'), ('8,-54'), ('8,-60'), ('8,-63'), ('8,-67'), ('8,-70'), ('8,-71'), ('8,-92'), ('8,-93'), ('8,-96'), ('8,-97'), ('8,-98'), ('8,-101'), ('8,-102'), ('8,-103'), ('8,-104'), ('8,-107'), ('8,-108'), ('8,-109'), ('8,-121'),
    ('8,-124'), ('8,-127'), ('8,-138'), ('8,-141'), ('8,-144'), ('9,139'), ('9,138'), ('9,134'), ('9,133'), ('9,94'), ('9,93'), ('9,72'), ('9,71'), ('9,51'), ('9,50'), ('9,11'), ('9,10'), ('9,-10'), ('9,-11'), ('9,-14'),
    ('9,-17'), ('9,-20'), ('9,-23'), ('9,-24'), ('9,-25'), ('9,-26'), ('9,-27'), ('9,-50'), ('9,-51'), ('9,-54'), ('9,-60'), ('9,-63'), ('9,-67'), ('9,-70'), ('9,-71'), ('9,-92'), ('9,-93'), ('9,-96'), ('9,-98'), ('9,-99'),
    ('9,-100'), ('9,-101'), ('9,-104'), ('9,-105'), ('9,-106'), ('9,-107'), ('9,-109'), ('9,-121'), ('9,-124'), ('9,-127'), ('9,-138'), ('9,-141'), ('9,-144'), ('10,139'), ('10,138'), ('10,134'), ('10,133'), ('10,94'), ('10,93'), ('10,72'),
    ('10,71'), ('10,51'), ('10,50'), ('10,11'), ('10,10'), ('10,-10'), ('10,-11'), ('10,-14'), ('10,-17'), ('10,-20'), ('10,-21'), ('10,-22'), ('10,-23'), ('10,-27'), ('10,-50'), ('10,-51'), ('10,-54'), ('10,-55'), ('10,-56'), ('10,-57'),
    ('10,-58'), ('10,-59'), ('10,-60'), ('10,-63'), ('10,-64'), ('10,-65'), ('10,-66'), ('10,-67'), ('10,-68'), ('10,-69'), ('10,-70'), ('10,-71'), ('10,-92'), ('10,-93'), ('10,-96'), ('10,-109'), ('10,-121'), ('10,-122'), ('10,-123'), ('10,-124'),
    ('10,-127'), ('10,-129'), ('10,-130'), ('10,-131'), ('10,-132'), ('10,-133'), ('10,-134'), ('10,-135'), ('10,-136'), ('10,-138'), ('10,-141'), ('10,-142'), ('10,-143'), ('10,-144'), ('11,139'), ('11,138'), ('11,134'), ('11,133'), ('11,94'), ('11,93'),
    ('11,92'), ('11,91'), ('11,90'), ('11,89'), ('11,88'), ('11,87'), ('11,86'), ('11,85'), ('11,84'), ('11,83'), ('11,82'), ('11,81'), ('11,80'), ('11,79'), ('11,78'), ('11,77'), ('11,76'), ('11,75'), ('11,74'), ('11,73'),
    ('11,72'), ('11,71'), ('11,51'), ('11,50'), ('11,11'), ('11,10'), ('11,9'), ('11,8'), ('11,7'), ('11,6'), ('11,5'), ('11,4'), ('11,3'), ('11,2'), ('11,1'), ('11,0'), ('11,-1'), ('11,-2'), ('11,-3'), ('11,-4'),
    ('11,-5'), ('11,-6'), ('11,-7'), ('11,-8'), ('11,-9'), ('11,-10'), ('11,-11'), ('11,-14'), ('11,-17'), ('11,-20'), ('11,-23'), ('11,-50'), ('11,-51'), ('11,-57'), ('11,-67'), ('11,-70'), ('11,-71'), ('11,-72'), ('11,-73'), ('11,-74'),
    ('11,-75'), ('11,-76'), ('11,-77'), ('11,-78'), ('11,-79'), ('11,-80'), ('11,-81'), ('11,-82'), ('11,-83'), ('11,-84'), ('11,-85'), ('11,-86'), ('11,-87'), ('11,-88'), ('11,-89'), ('11,-90'), ('11,-91'), ('11,-92'), ('11,-93'), ('11,-94'),
    ('11,-95'), ('11,-96'), ('11,-109'), ('11,-121'), ('11,-124'), ('11,-127'), ('11,-129'), ('11,-136'), ('11,-138'), ('11,-141'), ('11,-144'), ('12,139'), ('12,138'), ('12,134'), ('12,133'), ('12,94'), ('12,93'), ('12,92'), ('12,91'), ('12,90'),
    ('12,89'), ('12,88'), ('12,87'), ('12,86'), ('12,85'), ('12,84'), ('12,83'), ('12,82'), ('12,81'), ('12,80'), ('12,79'), ('12,78'), ('12,77'), ('12,76'), ('12,75'), ('12,74'), ('12,73'), ('12,72'), ('12,71'), ('12,51'),
    ('12,50'), ('12,11'), ('12,10'), ('12,9'), ('12,8'), ('12,7'), ('12,6'), ('12,5'), ('12,4'), ('12,3'), ('12,2'), ('12,1'), ('12,0'), ('12,-1'), ('12,-2'), ('12,-3'), ('12,-4'), ('12,-5'), ('12,-6'), ('12,-7'),
    ('12,-8'), ('12,-9'), ('12,-10'), ('12,-11'), ('12,-12'), ('12,-13'), ('12,-14'), ('12,-17'), ('12,-20'), ('12,-23'), ('12,-24'), ('12,-25'), ('12,-26'), ('12,-27'), ('12,-28'), ('12,-50'), ('12,-51'), ('12,-57'), ('12,-67'), ('12,-70'),
    ('12,-71'), ('12,-72'), ('12,-73'), ('12,-74'), ('12,-75'), ('12,-76'), ('12,-77'), ('12,-78'), ('12,-79'), ('12,-80'), ('12,-81'), ('12,-82'), ('12,-83'), ('12,-84'), ('12,-85'), ('12,-86'), ('12,-87'), ('12,-88'), ('12,-89'), ('12,-90'),
    ('12,-91'), ('12,-92'), ('12,-93'), ('12,-96'), ('12,-97'), ('12,-98'), ('12,-101'), ('12,-102'), ('12,-103'), ('12,-104'), ('12,-107'), ('12,-108'), ('12,-109'), ('12,-121'), ('12,-124'), ('12,-127'), ('12,-129'), ('12,-136'), ('12,-138'), ('12,-141'),
    ('12,-144'), ('13,139'), ('13,138'), ('13,134'), ('13,133'), ('13,94'), ('13,93'), ('13,51'), ('13,50'), ('13,1'), ('13,-3'), ('13,-11'), ('13,-14'), ('13,-17'), ('13,-20'), ('13,-23'), ('13,-28'), ('13,-50'), ('13,-51'), ('13,-52'),
    ('13,-53'), ('13,-54'), ('13,-55'), ('13,-56'), ('13,-57'), ('13,-58'), ('13,-59'), ('13,-60'), ('13,-61'), ('13,-62'), ('13,-63'), ('13,-64'), ('13,-65'), ('13,-66'), ('13,-67'), ('13,-76'), ('13,-79'), ('13,-82'), ('13,-87'), ('13,-96'),
    ('13,-98'), ('13,-99'), ('13,-100'), ('13,-101'), ('13,-104'), ('13,-105'), ('13,-106'), ('13,-107'), ('13,-109'), ('13,-121'), ('13,-124'), ('13,-127'), ('13,-129'), ('13,-131'), ('13,-132'), ('13,-133'), ('13,-134'), ('13,-136'), ('13,-138'), ('13,-141')
;

INSERT INTO explore.road_positions (position) VALUES
    ('13,-144'), ('14,139'), ('14,138'), ('14,134'), ('14,133'), ('14,94'), ('14,93'), ('14,51'), ('14,50'), ('14,1'), ('14,0'), ('14,-3'), ('14,-11'), ('14,-14'), ('14,-17'), ('14,-20'), ('14,-23'), ('14,-50'), ('14,-51'), ('14,-57'),
    ('14,-67'), ('14,-76'), ('14,-79'), ('14,-82'), ('14,-87'), ('14,-96'), ('14,-109'), ('14,-110'), ('14,-111'), ('14,-112'), ('14,-113'), ('14,-114'), ('14,-115'), ('14,-116'), ('14,-117'), ('14,-118'), ('14,-119'), ('14,-120'), ('14,-121'), ('14,-124'),
    ('14,-125'), ('14,-126'), ('14,-127'), ('14,-128'), ('14,-129'), ('14,-131'), ('14,-134'), ('14,-136'), ('14,-137'), ('14,-138'), ('14,-139'), ('14,-140'), ('14,-141'), ('14,-144'), ('14,-145'), ('14,-146'), ('14,-147'), ('14,-148'), ('14,-149'), ('14,-150'),
    ('15,139'), ('15,138'), ('15,134'), ('15,133'), ('15,94'), ('15,93'), ('15,51'), ('15,50'), ('15,1'), ('15,0'), ('15,-1'), ('15,-2'), ('15,-3'), ('15,-4'), ('15,-5'), ('15,-6'), ('15,-7'), ('15,-8'), ('15,-9'), ('15,-10'),
    ('15,-11'), ('15,-12'), ('15,-13'), ('15,-14'), ('15,-15'), ('15,-16'), ('15,-17'), ('15,-20'), ('15,-23'), ('15,-24'), ('15,-25'), ('15,-26'), ('15,-27'), ('15,-28'), ('15,-29'), ('15,-50'), ('15,-51'), ('15,-57'), ('15,-67'), ('15,-71'),
    ('15,-76'), ('15,-79'), ('15,-82'), ('15,-85'), ('15,-86'), ('15,-87'), ('15,-88'), ('15,-89'), ('15,-96'), ('15,-109'), ('15,-121'), ('15,-124'), ('15,-127'), ('15,-131'), ('15,-134'), ('15,-138'), ('15,-141'), ('15,-144'), ('15,-150'), ('16,139'),
    ('16,138'), ('16,134'), ('16,133'), ('16,94'), ('16,93'), ('16,51'), ('16,50'), ('16,1'), ('16,0'), ('16,-5'), ('16,-14'), ('16,-17'), ('16,-20'), ('16,-23'), ('16,-26'), ('16,-29'), ('16,-50'), ('16,-51'), ('16,-54'), ('16,-55'),
    ('16,-56'), ('16,-57'), ('16,-58'), ('16,-59'), ('16,-60'), ('16,-61'), ('16,-62'), ('16,-63'), ('16,-64'), ('16,-67'), ('16,-71'), ('16,-72'), ('16,-73'), ('16,-76'), ('16,-79'), ('16,-82'), ('16,-85'), ('16,-89'), ('16,-96'), ('16,-97'),
    ('16,-98'), ('16,-101'), ('16,-102'), ('16,-103'), ('16,-104'), ('16,-107'), ('16,-108'), ('16,-109'), ('16,-121'), ('16,-124'), ('16,-127'), ('16,-128'), ('16,-129'), ('16,-130'), ('16,-131'), ('16,-132'), ('16,-133'), ('16,-134'), ('16,-135'), ('16,-136'),
    ('16,-137'), ('16,-138'), ('16,-141'), ('16,-144'), ('16,-150'), ('17,139'), ('17,138'), ('17,134'), ('17,133'), ('17,94'), ('17,93'), ('17,51'), ('17,50'), ('17,1'), ('17,0'), ('17,-5'), ('17,-14'), ('17,-17'), ('17,-20'), ('17,-23'),
    ('17,-26'), ('17,-29'), ('17,-50'), ('17,-51'), ('17,-54'), ('17,-64'), ('17,-67'), ('17,-71'), ('17,-76'), ('17,-79'), ('17,-82'), ('17,-85'), ('17,-89'), ('17,-96'), ('17,-98'), ('17,-99'), ('17,-100'), ('17,-101'), ('17,-104'), ('17,-105'),
    ('17,-106'), ('17,-107'), ('17,-109'), ('17,-121'), ('17,-124'), ('17,-141'), ('17,-144'), ('17,-150'), ('18,139'), ('18,138'), ('18,134'), ('18,133'), ('18,132'), ('18,131'), ('18,130'), ('18,129'), ('18,128'), ('18,127'), ('18,126'), ('18,125'),
    ('18,124'), ('18,123'), ('18,122'), ('18,121'), ('18,120'), ('18,94'), ('18,93'), ('18,51'), ('18,50'), ('18,1'), ('18,0'), ('18,-1'), ('18,-2'), ('18,-3'), ('18,-4'), ('18,-5'), ('18,-6'), ('18,-7'), ('18,-8'), ('18,-9'),
    ('18,-10'), ('18,-11'), ('18,-12'), ('18,-13'), ('18,-14'), ('18,-15'), ('18,-16'), ('18,-17'), ('18,-18'), ('18,-19'), ('18,-20'), ('18,-23'), ('18,-26'), ('18,-27'), ('18,-28'), ('18,-29'), ('18,-50'), ('18,-51'), ('18,-54'), ('18,-64'),
    ('18,-67'), ('18,-71'), ('18,-76'), ('18,-79'), ('18,-82'), ('18,-85'), ('18,-89'), ('18,-96'), ('18,-109'), ('18,-121'), ('18,-124'), ('18,-141'), ('18,-144'), ('18,-150'), ('19,139'), ('19,138'), ('19,134'), ('19,133'), ('19,132'), ('19,131'),
    ('19,130'), ('19,129'), ('19,128'), ('19,127'), ('19,126'), ('19,125'), ('19,124'), ('19,123'), ('19,122'), ('19,121'), ('19,120'), ('19,94'), ('19,93'), ('19,51'), ('19,50'), ('19,1'), ('19,0'), ('19,-7'), ('19,-17'), ('19,-20'),
    ('19,-23'), ('19,-50'), ('19,-51'), ('19,-52'), ('19,-53'), ('19,-54'), ('19,-64'), ('19,-65'), ('19,-66'), ('19,-67'), ('19,-68'), ('19,-69'), ('19,-70'), ('19,-71'), ('19,-76'), ('19,-79'), ('19,-82'), ('19,-85'), ('19,-89'), ('19,-96'),
    ('19,-109'), ('19,-117'), ('19,-121'), ('19,-122'), ('19,-123'), ('19,-124'), ('19,-125'), ('19,-126'), ('19,-127'), ('19,-128'), ('19,-129'), ('19,-130'), ('19,-131'), ('19,-132'), ('19,-133'), ('19,-134'), ('19,-135'), ('19,-136'), ('19,-137'), ('19,-138'),
    ('19,-139'), ('19,-140'), ('19,-141'), ('19,-142'), ('19,-143'), ('19,-144'), ('19,-150'), ('20,139'), ('20,138'), ('20,121'), ('20,120'), ('20,94'), ('20,93'), ('20,51'), ('20,50'), ('20,1'), ('20,0'), ('20,-7'), ('20,-17'), ('20,-20'),
    ('20,-23'), ('20,-24'), ('20,-25'), ('20,-26'), ('20,-27'), ('20,-28'), ('20,-29'), ('20,-31'), ('20,-50'), ('20,-51'), ('20,-54'), ('20,-64'), ('20,-67'), ('20,-71'), ('20,-76'), ('20,-79'), ('20,-80'), ('20,-81'), ('20,-82'), ('20,-83'),
    ('20,-84'), ('20,-85'), ('20,-89'), ('20,-90'), ('20,-91'), ('20,-92'), ('20,-93'), ('20,-96'), ('20,-109'), ('20,-112'), ('20,-113'), ('20,-114'), ('20,-115'), ('20,-116'), ('20,-117'), ('20,-121'), ('20,-131'), ('20,-134'), ('20,-144'), ('20,-150'),
    ('21,139'), ('21,138'), ('21,121'), ('21,120'), ('21,94'), ('21,93'), ('21,51'), ('21,50'), ('21,1'), ('21,0'), ('21,-1'), ('21,-2'), ('21,-3'), ('21,-4'), ('21,-5'), ('21,-6'), ('21,-7'), ('21,-8'), ('21,-9'), ('21,-10'),
    ('21,-11'), ('21,-12'), ('21,-13'), ('21,-14'), ('21,-15'), ('21,-16'), ('21,-17'), ('21,-18'), ('21,-19'), ('21,-20'), ('21,-21'), ('21,-29'), ('21,-31'), ('21,-33'), ('21,-34'), ('21,-35'), ('21,-36'), ('21,-50'), ('21,-51'), ('21,-54'),
    ('21,-64'), ('21,-67'), ('21,-71'), ('21,-72'), ('21,-73'), ('21,-74'), ('21,-75'), ('21,-76'), ('21,-79'), ('21,-84'), ('21,-90'), ('21,-112'), ('21,-116'), ('21,-117'), ('21,-121'), ('21,-131'), ('21,-134'), ('21,-144'), ('21,-150'), ('22,139'),
    ('22,138'), ('22,121'), ('22,120'), ('22,94'), ('22,93'), ('22,51'), ('22,50'), ('22,1'), ('22,0'), ('22,-9'), ('22,-20'), ('22,-21'), ('22,-22'), ('22,-23'), ('22,-24'), ('22,-25'), ('22,-26'), ('22,-27'), ('22,-29'), ('22,-31'),
    ('22,-33'), ('22,-36'), ('22,-38'), ('22,-39'), ('22,-50'), ('22,-51'), ('22,-54'), ('22,-64'), ('22,-67'), ('22,-71'), ('22,-76'), ('22,-79'), ('22,-84'), ('22,-90'), ('22,-113'), ('22,-114'), ('22,-115'), ('22,-117'), ('22,-121'), ('22,-122'),
    ('22,-123'), ('22,-124'), ('22,-125'), ('22,-126'), ('22,-127'), ('22,-128'), ('22,-129'), ('22,-130'), ('22,-131'), ('22,-132'), ('22,-133'), ('22,-134'), ('22,-135'), ('22,-136'), ('22,-137'), ('22,-138'), ('22,-139'), ('22,-140'), ('22,-141'), ('22,-142'),
    ('22,-143'), ('22,-144'), ('22,-150'), ('23,139'), ('23,138'), ('23,121'), ('23,120'), ('23,94'), ('23,93'), ('23,51'), ('23,50'), ('23,1'), ('23,0'), ('23,-9'), ('23,-21'), ('23,-27'), ('23,-29'), ('23,-31'), ('23,-33'), ('23,-36'),
    ('23,-39'), ('23,-41'), ('23,-42'), ('23,-50'), ('23,-51'), ('23,-54'), ('23,-55'), ('23,-56'), ('23,-57'), ('23,-58'), ('23,-59'), ('23,-60'), ('23,-61'), ('23,-62'), ('23,-63'), ('23,-64'), ('23,-67'), ('23,-71'), ('23,-76'), ('23,-79'),
    ('23,-82'), ('23,-83'), ('23,-84'), ('23,-85'), ('23,-86'), ('23,-87'), ('23,-88'), ('23,-89'), ('23,-90'), ('23,-91'), ('23,-92'), ('23,-93'), ('23,-117'), ('23,-124'), ('23,-141'), ('23,-150'), ('24,139'), ('24,138'), ('24,121'), ('24,120'),
    ('24,94'), ('24,93'), ('24,51'), ('24,50'), ('24,1'), ('24,0'), ('24,-1'), ('24,-2'), ('24,-3'), ('24,-4'), ('24,-5'), ('24,-6'), ('24,-7'), ('24,-8'), ('24,-9'), ('24,-10'), ('24,-11'), ('24,-12'), ('24,-13'), ('24,-14'),
    ('24,-15'), ('24,-16'), ('24,-17'), ('24,-18'), ('24,-19'), ('24,-20'), ('24,-21'), ('24,-23'), ('24,-24'), ('24,-25'), ('24,-27'), ('24,-29'), ('24,-31'), ('24,-33'), ('24,-34'), ('24,-35'), ('24,-36'), ('24,-39'), ('24,-42'), ('24,-44'),
    ('24,-45'), ('24,-50'), ('24,-51'), ('24,-82'), ('24,-89'), ('24,-112'), ('24,-113'), ('24,-114'), ('24,-115'), ('24,-116'), ('24,-117'), ('24,-124'), ('24,-141'), ('24,-150'), ('25,139'), ('25,138'), ('25,121'), ('25,120'), ('25,94'), ('25,93'),
    ('25,51'), ('25,50'), ('25,1'), ('25,0'), ('25,-3'), ('25,-5'), ('25,-8'), ('25,-11'), ('25,-14'), ('25,-19'), ('25,-21'), ('25,-23'), ('25,-25'), ('25,-27'), ('25,-29'), ('25,-31'), ('25,-36'), ('25,-39'), ('25,-42'), ('25,-45'),
    ('25,-47'), ('25,-50'), ('25,-51'), ('25,-82'), ('25,-89'), ('25,-112'), ('25,-114'), ('25,-116'), ('25,-117'), ('25,-124'), ('25,-141'), ('25,-150'), ('26,139'), ('26,138'), ('26,121'), ('26,120'), ('26,119'), ('26,94'), ('26,93'), ('26,51'),
    ('26,50'), ('26,1'), ('26,0'), ('26,-3'), ('26,-5'), ('26,-8'), ('26,-11'), ('26,-14'), ('26,-19'), ('26,-21'), ('26,-23'), ('26,-27'), ('26,-29'), ('26,-31'), ('26,-36'), ('26,-39'), ('26,-42'), ('26,-45'), ('26,-47'), ('26,-50'),
    ('26,-51'), ('26,-54'), ('26,-57'), ('26,-64'), ('26,-67'), ('26,-71'), ('26,-76'), ('26,-79'), ('26,-82'), ('26,-91'), ('26,-94'), ('26,-112'), ('26,-114'), ('26,-116'), ('26,-117'), ('26,-124'), ('26,-141'), ('26,-150'), ('27,139'), ('27,138'),
    ('27,121'), ('27,120'), ('27,119'), ('27,94'), ('27,93'), ('27,51'), ('27,50'), ('27,1'), ('27,0'), ('27,-5'), ('27,-6'), ('27,-8'), ('27,-11'), ('27,-14'), ('27,-15'), ('27,-16'), ('27,-17'), ('27,-19'), ('27,-21'), ('27,-23'),
    ('27,-24'), ('27,-25'), ('27,-26'), ('27,-27'), ('27,-29'), ('27,-30'), ('27,-31'), ('27,-32'), ('27,-33'), ('27,-34'), ('27,-35'), ('27,-36'), ('27,-37'), ('27,-38'), ('27,-39'), ('27,-40'), ('27,-41'), ('27,-42'), ('27,-43'), ('27,-44'),
    ('27,-45'), ('27,-46'), ('27,-47'), ('27,-48'), ('27,-49'), ('27,-50'), ('27,-51'), ('27,-54'), ('27,-57'), ('27,-64'), ('27,-67'), ('27,-71'), ('27,-76'), ('27,-79'), ('27,-82'), ('27,-91'), ('27,-94'), ('27,-117'), ('27,-124'), ('27,-141'),
    ('27,-150'), ('28,139'), ('28,138'), ('28,120'), ('28,119'), ('28,94'), ('28,93'), ('28,51'), ('28,50'), ('28,1'), ('28,0'), ('28,-8'), ('28,-9'), ('28,-11'), ('28,-14'), ('28,-17'), ('28,-19'), ('28,-21'), ('28,-29'), ('28,-33'),
    ('28,-41'), ('28,-50'), ('28,-51'), ('28,-52'), ('28,-53'), ('28,-54'), ('28,-55'), ('28,-56'), ('28,-57'), ('28,-58'), ('28,-59'), ('28,-60'), ('28,-61'), ('28,-62'), ('28,-63'), ('28,-64'), ('28,-67'), ('28,-71'), ('28,-76'), ('28,-79'),
    ('28,-80'), ('28,-81'), ('28,-82'), ('28,-83'), ('28,-84'), ('28,-87'), ('28,-88'), ('28,-89'), ('28,-90'), ('28,-91'), ('28,-92'), ('28,-93'), ('28,-94'), ('28,-97'), ('28,-113'), ('28,-114'), ('28,-115'), ('28,-117'), ('28,-150'), ('29,139'),
    ('29,138'), ('29,120'), ('29,119'), ('29,94'), ('29,93'), ('29,51'), ('29,50'), ('29,1'), ('29,0'), ('29,-11'), ('29,-12'), ('29,-14'), ('29,-17'), ('29,-19'), ('29,-21'), ('29,-22'), ('29,-23'), ('29,-24'), ('29,-25'), ('29,-26'),
    ('29,-27'), ('29,-28'), ('29,-29'), ('29,-30'), ('29,-33'), ('29,-41'), ('29,-50'), ('29,-51'), ('29,-54'), ('29,-57'), ('29,-64'), ('29,-65'), ('29,-66'), ('29,-67'), ('29,-68'), ('29,-69'), ('29,-70'), ('29,-71'), ('29,-76'), ('29,-79'),
    ('29,-83'), ('29,-87'), ('29,-94'), ('29,-97'), ('29,-112'), ('29,-116'), ('29,-117'), ('29,-150'), ('30,139'), ('30,138'), ('30,120'), ('30,119'), ('30,94'), ('30,93'), ('30,51'), ('30,50'), ('30,1'), ('30,0'), ('30,-14'), ('30,-15'),
    ('30,-16'), ('30,-17'), ('30,-19'), ('30,-21'), ('30,-29'), ('30,-30'), ('30,-31'), ('30,-32'), ('30,-33'), ('30,-34'), ('30,-35'), ('30,-36'), ('30,-37'), ('30,-38'), ('30,-39'), ('30,-40'), ('30,-41'), ('30,-42'), ('30,-43'), ('30,-44'),
    ('30,-45'), ('30,-46'), ('30,-47'), ('30,-48'), ('30,-49'), ('30,-50'), ('30,-51'), ('30,-54'), ('30,-57'), ('30,-64'), ('30,-67'), ('30,-71'), ('30,-76'), ('30,-79'), ('30,-83'), ('30,-87'), ('30,-94'), ('30,-95'), ('30,-96'), ('30,-97'),
    ('30,-113'), ('30,-115'), ('30,-117'), ('30,-150'), ('31,139'), ('31,138'), ('31,120'), ('31,119'), ('31,94'), ('31,93'), ('31,51'), ('31,50'), ('31,1'), ('31,0'), ('31,-19'), ('31,-21'), ('31,-22'), ('31,-23'), ('31,-24'), ('31,-25'),
    ('31,-26'), ('31,-27'), ('31,-30'), ('31,-33'), ('31,-43'), ('31,-50'), ('31,-51'), ('31,-54'), ('31,-55'), ('31,-56'), ('31,-57'), ('31,-58'), ('31,-59'), ('31,-60'), ('31,-61'), ('31,-62'), ('31,-63'), ('31,-64'), ('31,-67'), ('31,-71')
;

INSERT INTO explore.road_positions (position) VALUES
    ('31,-72'), ('31,-73'), ('31,-74'), ('31,-75'), ('31,-76'), ('31,-79'), ('31,-82'), ('31,-83'), ('31,-84'), ('31,-85'), ('31,-86'), ('31,-87'), ('31,-94'), ('31,-97'), ('31,-117'), ('31,-150'), ('32,139'), ('32,138'), ('32,120'), ('32,119'),
    ('32,94'), ('32,93'), ('32,51'), ('32,50'), ('32,1'), ('32,0'), ('32,-27'), ('32,-30'), ('32,-33'), ('32,-43'), ('32,-50'), ('32,-51'), ('32,-67'), ('32,-71'), ('32,-76'), ('32,-79'), ('32,-87'), ('32,-94'), ('32,-97'), ('32,-112'),
    ('32,-113'), ('32,-114'), ('32,-115'), ('32,-116'), ('32,-117'), ('32,-150'), ('33,139'), ('33,138'), ('33,120'), ('33,119'), ('33,94'), ('33,93'), ('33,51'), ('33,50'), ('33,1'), ('33,0'), ('33,-21'), ('33,-22'), ('33,-23'), ('33,-24'),
    ('33,-27'), ('33,-28'), ('33,-29'), ('33,-30'), ('33,-31'), ('33,-32'), ('33,-33'), ('33,-34'), ('33,-35'), ('33,-36'), ('33,-37'), ('33,-38'), ('33,-39'), ('33,-40'), ('33,-41'), ('33,-42'), ('33,-43'), ('33,-44'), ('33,-45'), ('33,-46'),
    ('33,-47'), ('33,-48'), ('33,-49'), ('33,-50'), ('33,-51'), ('33,-67'), ('33,-71'), ('33,-76'), ('33,-79'), ('33,-87'), ('33,-94'), ('33,-97'), ('33,-112'), ('33,-114'), ('33,-116'), ('33,-117'), ('33,-150'), ('34,139'), ('34,138'), ('34,137'),
    ('34,136'), ('34,135'), ('34,134'), ('34,133'), ('34,132'), ('34,131'), ('34,130'), ('34,129'), ('34,128'), ('34,127'), ('34,126'), ('34,125'), ('34,124'), ('34,123'), ('34,122'), ('34,121'), ('34,120'), ('34,119'), ('34,118'), ('34,117'),
    ('34,116'), ('34,115'), ('34,114'), ('34,113'), ('34,112'), ('34,111'), ('34,110'), ('34,109'), ('34,108'), ('34,107'), ('34,106'), ('34,105'), ('34,104'), ('34,103'), ('34,102'), ('34,101'), ('34,100'), ('34,99'), ('34,98'), ('34,97'),
    ('34,96'), ('34,95'), ('34,94'), ('34,93'), ('34,51'), ('34,50'), ('34,1'), ('34,0'), ('34,-21'), ('34,-24'), ('34,-27'), ('34,-30'), ('34,-33'), ('34,-36'), ('34,-45'), ('34,-50'), ('34,-51'), ('34,-54'), ('34,-55'), ('34,-56'),
    ('34,-57'), ('34,-58'), ('34,-59'), ('34,-60'), ('34,-61'), ('34,-62'), ('34,-63'), ('34,-64'), ('34,-65'), ('34,-66'), ('34,-67'), ('34,-68'), ('34,-69'), ('34,-70'), ('34,-71'), ('34,-72'), ('34,-73'), ('34,-74'), ('34,-75'), ('34,-76'),
    ('34,-77'), ('34,-78'), ('34,-79'), ('34,-80'), ('34,-81'), ('34,-82'), ('34,-83'), ('34,-84'), ('34,-87'), ('34,-94'), ('34,-97'), ('34,-112'), ('34,-114'), ('34,-116'), ('34,-117'), ('34,-150'), ('35,139'), ('35,138'), ('35,137'), ('35,136'),
    ('35,135'), ('35,134'), ('35,133'), ('35,132'), ('35,131'), ('35,130'), ('35,129'), ('35,128'), ('35,127'), ('35,126'), ('35,125'), ('35,124'), ('35,123'), ('35,122'), ('35,121'), ('35,120'), ('35,119'), ('35,118'), ('35,117'), ('35,116'),
    ('35,115'), ('35,114'), ('35,113'), ('35,112'), ('35,111'), ('35,110'), ('35,109'), ('35,108'), ('35,107'), ('35,106'), ('35,105'), ('35,104'), ('35,103'), ('35,102'), ('35,101'), ('35,100'), ('35,99'), ('35,98'), ('35,97'), ('35,96'),
    ('35,95'), ('35,94'), ('35,93'), ('35,51'), ('35,50'), ('35,1'), ('35,0'), ('35,-21'), ('35,-24'), ('35,-27'), ('35,-30'), ('35,-33'), ('35,-36'), ('35,-45'), ('35,-50'), ('35,-51'), ('35,-54'), ('35,-84'), ('35,-87'), ('35,-94'),
    ('35,-97'), ('35,-117'), ('35,-150'), ('36,51'), ('36,50'), ('36,1'), ('36,0'), ('36,-21'), ('36,-22'), ('36,-23'), ('36,-24'), ('36,-25'), ('36,-26'), ('36,-27'), ('36,-30'), ('36,-33'), ('36,-34'), ('36,-35'), ('36,-36'), ('36,-37'),
    ('36,-38'), ('36,-39'), ('36,-40'), ('36,-41'), ('36,-42'), ('36,-43'), ('36,-44'), ('36,-45'), ('36,-46'), ('36,-47'), ('36,-48'), ('36,-49'), ('36,-50'), ('36,-51'), ('36,-54'), ('36,-84'), ('36,-85'), ('36,-86'), ('36,-87'), ('36,-94'),
    ('36,-97'), ('36,-112'), ('36,-113'), ('36,-114'), ('36,-115'), ('36,-116'), ('36,-117'), ('36,-150'), ('37,51'), ('37,50'), ('37,1'), ('37,0'), ('37,-27'), ('37,-30'), ('37,-33'), ('37,-36'), ('37,-39'), ('37,-47'), ('37,-50'), ('37,-51'),
    ('37,-54'), ('37,-84'), ('37,-87'), ('37,-94'), ('37,-97'), ('37,-113'), ('37,-117'), ('37,-150'), ('38,51'), ('38,50'), ('38,1'), ('38,0'), ('38,-22'), ('38,-27'), ('38,-30'), ('38,-33'), ('38,-36'), ('38,-39'), ('38,-47'), ('38,-50'),
    ('38,-51'), ('38,-54'), ('38,-84'), ('38,-87'), ('38,-94'), ('38,-97'), ('38,-113'), ('38,-114'), ('38,-115'), ('38,-116'), ('38,-117'), ('38,-150'), ('39,51'), ('39,50'), ('39,1'), ('39,0'), ('39,-22'), ('39,-23'), ('39,-24'), ('39,-25'),
    ('39,-26'), ('39,-27'), ('39,-30'), ('39,-33'), ('39,-36'), ('39,-37'), ('39,-38'), ('39,-39'), ('39,-40'), ('39,-41'), ('39,-42'), ('39,-43'), ('39,-44'), ('39,-45'), ('39,-46'), ('39,-47'), ('39,-48'), ('39,-49'), ('39,-50'), ('39,-51'),
    ('39,-54'), ('39,-84'), ('39,-87'), ('39,-94'), ('39,-97'), ('39,-117'), ('39,-150'), ('40,51'), ('40,50'), ('40,1'), ('40,0'), ('40,-27'), ('40,-30'), ('40,-33'), ('40,-36'), ('40,-39'), ('40,-50'), ('40,-51'), ('40,-52'), ('40,-53'),
    ('40,-54'), ('40,-84'), ('40,-87'), ('40,-94'), ('40,-97'), ('40,-112'), ('40,-117'), ('40,-150'), ('41,51'), ('41,50'), ('41,1'), ('41,0'), ('41,-23'), ('41,-27'), ('41,-28'), ('41,-29'), ('41,-30'), ('41,-33'), ('41,-36'), ('41,-39'),
    ('41,-50'), ('41,-51'), ('41,-52'), ('41,-53'), ('41,-54'), ('41,-84'), ('41,-87'), ('41,-94'), ('41,-97'), ('41,-112'), ('41,-113'), ('41,-114'), ('41,-115'), ('41,-116'), ('41,-117'), ('41,-150'), ('42,51'), ('42,50'), ('42,1'), ('42,0'),
    ('42,-23'), ('42,-24'), ('42,-25'), ('42,-26'), ('42,-27'), ('42,-30'), ('42,-33'), ('42,-36'), ('42,-39'), ('42,-50'), ('42,-51'), ('42,-54'), ('42,-84'), ('42,-87'), ('42,-94'), ('42,-97'), ('42,-112'), ('42,-117'), ('42,-150'), ('43,51'),
    ('43,50'), ('43,1'), ('43,0'), ('43,-27'), ('43,-30'), ('43,-31'), ('43,-32'), ('43,-33'), ('43,-36'), ('43,-39'), ('43,-50'), ('43,-51'), ('43,-54'), ('43,-84'), ('43,-87'), ('43,-94'), ('43,-97'), ('43,-117'), ('43,-150'), ('44,51'),
    ('44,50'), ('44,1'), ('44,0'), ('44,-24'), ('44,-27'), ('44,-30'), ('44,-33'), ('44,-36'), ('44,-39'), ('44,-50'), ('44,-51'), ('44,-54'), ('44,-84'), ('44,-87'), ('44,-94'), ('44,-97'), ('44,-112'), ('44,-113'), ('44,-114'), ('44,-115'),
    ('44,-116'), ('44,-117'), ('44,-150'), ('45,51'), ('45,50'), ('45,1'), ('45,0'), ('45,-24'), ('45,-25'), ('45,-26'), ('45,-27'), ('45,-30'), ('45,-33'), ('45,-34'), ('45,-35'), ('45,-36'), ('45,-39'), ('45,-50'), ('45,-51'), ('45,-54'),
    ('45,-84'), ('45,-85'), ('45,-86'), ('45,-87'), ('45,-94'), ('45,-95'), ('45,-96'), ('45,-97'), ('45,-112'), ('45,-115'), ('45,-117'), ('45,-150'), ('46,51'), ('46,50'), ('46,1'), ('46,0'), ('46,-27'), ('46,-30'), ('46,-33'), ('46,-36'),
    ('46,-39'), ('46,-50'), ('46,-51'), ('46,-54'), ('46,-84'), ('46,-87'), ('46,-94'), ('46,-97'), ('46,-113'), ('46,-114'), ('46,-116'), ('46,-117'), ('46,-150'), ('47,51'), ('47,50'), ('47,1'), ('47,0'), ('47,-25'), ('47,-26'), ('47,-27'),
    ('47,-30'), ('47,-33'), ('47,-36'), ('47,-37'), ('47,-38'), ('47,-39'), ('47,-50'), ('47,-51'), ('47,-54'), ('47,-55'), ('47,-56'), ('47,-57'), ('47,-58'), ('47,-59'), ('47,-60'), ('47,-61'), ('47,-62'), ('47,-63'), ('47,-64'), ('47,-65'),
    ('47,-66'), ('47,-67'), ('47,-68'), ('47,-69'), ('47,-70'), ('47,-71'), ('47,-72'), ('47,-73'), ('47,-74'), ('47,-75'), ('47,-76'), ('47,-77'), ('47,-78'), ('47,-79'), ('47,-80'), ('47,-81'), ('47,-82'), ('47,-83'), ('47,-84'), ('47,-87'),
    ('47,-88'), ('47,-89'), ('47,-90'), ('47,-91'), ('47,-92'), ('47,-93'), ('47,-94'), ('47,-97'), ('47,-117'), ('47,-150'), ('48,51'), ('48,50'), ('48,1'), ('48,0'), ('48,-27'), ('48,-30'), ('48,-33'), ('48,-36'), ('48,-39'), ('48,-50'),
    ('48,-51'), ('48,-68'), ('48,-69'), ('48,-113'), ('48,-114'), ('48,-115'), ('48,-116'), ('48,-117'), ('48,-150'), ('49,51'), ('49,50'), ('49,1'), ('49,0'), ('49,-27'), ('49,-30'), ('49,-33'), ('49,-36'), ('49,-39'), ('49,-50'), ('49,-51'),
    ('49,-68'), ('49,-69'), ('49,-112'), ('49,-115'), ('49,-117'), ('49,-150'), ('50,73'), ('50,72'), ('50,71'), ('50,70'), ('50,69'), ('50,68'), ('50,67'), ('50,66'), ('50,65'), ('50,64'), ('50,63'), ('50,62'), ('50,61'), ('50,60'),
    ('50,59'), ('50,58'), ('50,57'), ('50,56'), ('50,55'), ('50,54'), ('50,53'), ('50,52'), ('50,51'), ('50,50'), ('50,49'), ('50,48'), ('50,47'), ('50,46'), ('50,45'), ('50,44'), ('50,43'), ('50,42'), ('50,41'), ('50,40'),
    ('50,39'), ('50,38'), ('50,37'), ('50,36'), ('50,35'), ('50,34'), ('50,33'), ('50,32'), ('50,31'), ('50,30'), ('50,29'), ('50,28'), ('50,27'), ('50,26'), ('50,25'), ('50,24'), ('50,23'), ('50,22'), ('50,21'), ('50,20'),
    ('50,19'), ('50,18'), ('50,17'), ('50,16'), ('50,15'), ('50,14'), ('50,13'), ('50,12'), ('50,11'), ('50,10'), ('50,9'), ('50,8'), ('50,7'), ('50,6'), ('50,5'), ('50,4'), ('50,3'), ('50,2'), ('50,1'), ('50,0'),
    ('50,-1'), ('50,-2'), ('50,-3'), ('50,-4'), ('50,-5'), ('50,-6'), ('50,-7'), ('50,-8'), ('50,-9'), ('50,-10'), ('50,-11'), ('50,-12'), ('50,-13'), ('50,-14'), ('50,-15'), ('50,-16'), ('50,-17'), ('50,-18'), ('50,-19'), ('50,-20'),
    ('50,-21'), ('50,-22'), ('50,-23'), ('50,-24'), ('50,-25'), ('50,-26'), ('50,-27'), ('50,-28'), ('50,-29'), ('50,-30'), ('50,-31'), ('50,-32'), ('50,-33'), ('50,-34'), ('50,-35'), ('50,-36'), ('50,-37'), ('50,-38'), ('50,-39'), ('50,-40'),
    ('50,-41'), ('50,-42'), ('50,-43'), ('50,-44'), ('50,-45'), ('50,-46'), ('50,-47'), ('50,-48'), ('50,-49'), ('50,-50'), ('50,-51'), ('50,-52'), ('50,-53'), ('50,-54'), ('50,-55'), ('50,-56'), ('50,-57'), ('50,-58'), ('50,-59'), ('50,-60'),
    ('50,-61'), ('50,-62'), ('50,-63'), ('50,-64'), ('50,-65'), ('50,-66'), ('50,-67'), ('50,-68'), ('50,-69'), ('50,-70'), ('50,-71'), ('50,-72'), ('50,-73'), ('50,-113'), ('50,-114'), ('50,-115'), ('50,-116'), ('50,-117'), ('50,-150'), ('51,73'),
    ('51,72'), ('51,71'), ('51,70'), ('51,69'), ('51,68'), ('51,67'), ('51,66'), ('51,65'), ('51,64'), ('51,63'), ('51,62'), ('51,61'), ('51,60'), ('51,59'), ('51,58'), ('51,57'), ('51,56'), ('51,55'), ('51,54'), ('51,53'),
    ('51,52'), ('51,51'), ('51,50'), ('51,49'), ('51,48'), ('51,47'), ('51,46'), ('51,45'), ('51,44'), ('51,43'), ('51,42'), ('51,41'), ('51,40'), ('51,39'), ('51,38'), ('51,37'), ('51,36'), ('51,35'), ('51,34'), ('51,33'),
    ('51,32'), ('51,31'), ('51,30'), ('51,29'), ('51,28'), ('51,27'), ('51,26'), ('51,25'), ('51,24'), ('51,23'), ('51,22'), ('51,21'), ('51,20'), ('51,19'), ('51,18'), ('51,17'), ('51,16'), ('51,15'), ('51,14'), ('51,13'),
    ('51,12'), ('51,11'), ('51,10'), ('51,9'), ('51,8'), ('51,7'), ('51,6'), ('51,5'), ('51,4'), ('51,3'), ('51,2'), ('51,1'), ('51,0'), ('51,-1'), ('51,-2'), ('51,-3'), ('51,-4'), ('51,-5'), ('51,-6'), ('51,-7'),
    ('51,-8'), ('51,-9'), ('51,-10'), ('51,-11'), ('51,-12'), ('51,-13'), ('51,-14'), ('51,-15'), ('51,-16'), ('51,-17'), ('51,-18'), ('51,-19'), ('51,-20'), ('51,-21'), ('51,-22'), ('51,-23'), ('51,-24'), ('51,-25'), ('51,-26'), ('51,-27'),
    ('51,-28'), ('51,-29'), ('51,-30'), ('51,-31'), ('51,-32'), ('51,-33'), ('51,-34'), ('51,-35'), ('51,-36'), ('51,-37'), ('51,-38'), ('51,-39'), ('51,-40'), ('51,-41'), ('51,-42'), ('51,-43'), ('51,-44'), ('51,-45'), ('51,-46'), ('51,-47'),
    ('51,-48'), ('51,-49'), ('51,-50'), ('51,-51'), ('51,-52'), ('51,-53'), ('51,-54'), ('51,-55'), ('51,-56'), ('51,-57'), ('51,-58'), ('51,-59'), ('51,-60'), ('51,-61'), ('51,-62'), ('51,-63'), ('51,-64'), ('51,-65'), ('51,-66'), ('51,-67'),
    ('51,-68'), ('51,-69'), ('51,-70'), ('51,-71'), ('51,-72'), ('51,-73'), ('51,-117'), ('51,-150'), ('52,73'), ('52,72'), ('52,51'), ('52,50'), ('52,1'), ('52,0'), ('52,-50'), ('52,-51'), ('52,-72'), ('52,-73'), ('52,-112'), ('52,-113'),
    ('52,-114'), ('52,-115'), ('52,-116'), ('52,-117'), ('52,-150'), ('53,73'), ('53,72'), ('53,51'), ('53,50'), ('53,1'), ('53,0'), ('53,-50'), ('53,-51'), ('53,-72'), ('53,-73'), ('53,-116'), ('53,-117'), ('53,-150'), ('54,73'), ('54,72'),
    ('54,51'), ('54,50'), ('54,1'), ('54,0'), ('54,-50'), ('54,-51'), ('54,-72'), ('54,-73'), ('54,-116'), ('54,-117'), ('54,-150'), ('55,73'), ('55,72'), ('55,51'), ('55,50'), ('55,1'), ('55,0'), ('55,-50'), ('55,-51'), ('55,-72')
;

INSERT INTO explore.road_positions (position) VALUES
    ('55,-73'), ('55,-117'), ('55,-150'), ('56,73'), ('56,72'), ('56,51'), ('56,50'), ('56,1'), ('56,0'), ('56,-50'), ('56,-51'), ('56,-72'), ('56,-73'), ('56,-113'), ('56,-114'), ('56,-115'), ('56,-116'), ('56,-117'), ('56,-150'), ('57,73'),
    ('57,72'), ('57,51'), ('57,50'), ('57,1'), ('57,0'), ('57,-50'), ('57,-51'), ('57,-72'), ('57,-73'), ('57,-112'), ('57,-115'), ('57,-117'), ('57,-150'), ('58,73'), ('58,72'), ('58,51'), ('58,50'), ('58,1'), ('58,0'), ('58,-50'),
    ('58,-51'), ('58,-72'), ('58,-73'), ('58,-113'), ('58,-114'), ('58,-115'), ('58,-116'), ('58,-117'), ('58,-150'), ('59,73'), ('59,72'), ('59,51'), ('59,50'), ('59,1'), ('59,0'), ('59,-50'), ('59,-51'), ('59,-72'), ('59,-73'), ('59,-117'),
    ('59,-150'), ('60,150'), ('60,149'), ('60,148'), ('60,147'), ('60,146'), ('60,145'), ('60,144'), ('60,143'), ('60,142'), ('60,141'), ('60,140'), ('60,139'), ('60,138'), ('60,137'), ('60,136'), ('60,135'), ('60,134'), ('60,133'), ('60,132'),
    ('60,131'), ('60,130'), ('60,129'), ('60,128'), ('60,127'), ('60,126'), ('60,125'), ('60,124'), ('60,123'), ('60,122'), ('60,121'), ('60,120'), ('60,119'), ('60,118'), ('60,117'), ('60,116'), ('60,115'), ('60,114'), ('60,113'), ('60,112'),
    ('60,111'), ('60,110'), ('60,109'), ('60,108'), ('60,107'), ('60,106'), ('60,105'), ('60,104'), ('60,103'), ('60,102'), ('60,101'), ('60,100'), ('60,99'), ('60,98'), ('60,97'), ('60,96'), ('60,95'), ('60,94'), ('60,93'), ('60,92'),
    ('60,91'), ('60,90'), ('60,89'), ('60,88'), ('60,87'), ('60,86'), ('60,85'), ('60,84'), ('60,83'), ('60,82'), ('60,81'), ('60,80'), ('60,79'), ('60,78'), ('60,77'), ('60,76'), ('60,75'), ('60,74'), ('60,73'), ('60,72'),
    ('60,51'), ('60,50'), ('60,1'), ('60,0'), ('60,-50'), ('60,-51'), ('60,-72'), ('60,-73'), ('60,-112'), ('60,-113'), ('60,-114'), ('60,-115'), ('60,-116'), ('60,-117'), ('60,-150'), ('61,150'), ('61,149'), ('61,148'), ('61,147'), ('61,146'),
    ('61,145'), ('61,144'), ('61,143'), ('61,142'), ('61,141'), ('61,140'), ('61,139'), ('61,138'), ('61,137'), ('61,136'), ('61,135'), ('61,134'), ('61,133'), ('61,132'), ('61,131'), ('61,130'), ('61,129'), ('61,128'), ('61,127'), ('61,126'),
    ('61,125'), ('61,124'), ('61,123'), ('61,122'), ('61,121'), ('61,120'), ('61,119'), ('61,118'), ('61,117'), ('61,116'), ('61,115'), ('61,114'), ('61,113'), ('61,112'), ('61,111'), ('61,110'), ('61,109'), ('61,108'), ('61,107'), ('61,106'),
    ('61,105'), ('61,104'), ('61,103'), ('61,102'), ('61,101'), ('61,100'), ('61,99'), ('61,98'), ('61,97'), ('61,96'), ('61,95'), ('61,94'), ('61,93'), ('61,92'), ('61,91'), ('61,90'), ('61,89'), ('61,88'), ('61,87'), ('61,86'),
    ('61,85'), ('61,84'), ('61,83'), ('61,82'), ('61,81'), ('61,80'), ('61,79'), ('61,78'), ('61,77'), ('61,76'), ('61,75'), ('61,74'), ('61,73'), ('61,72'), ('61,51'), ('61,50'), ('61,1'), ('61,0'), ('61,-50'), ('61,-51'),
    ('61,-72'), ('61,-73'), ('61,-113'), ('61,-117'), ('61,-150'), ('62,73'), ('62,72'), ('62,51'), ('62,50'), ('62,1'), ('62,0'), ('62,-50'), ('62,-51'), ('62,-72'), ('62,-73'), ('62,-113'), ('62,-114'), ('62,-115'), ('62,-116'), ('62,-117'),
    ('62,-150'), ('63,73'), ('63,72'), ('63,51'), ('63,50'), ('63,1'), ('63,0'), ('63,-50'), ('63,-51'), ('63,-72'), ('63,-73'), ('63,-117'), ('63,-150'), ('64,73'), ('64,72'), ('64,51'), ('64,50'), ('64,1'), ('64,0'), ('64,-50'),
    ('64,-51'), ('64,-72'), ('64,-73'), ('64,-112'), ('64,-113'), ('64,-114'), ('64,-115'), ('64,-116'), ('64,-117'), ('64,-150'), ('65,73'), ('65,72'), ('65,51'), ('65,50'), ('65,1'), ('65,0'), ('65,-50'), ('65,-51'), ('65,-72'), ('65,-73'),
    ('65,-112'), ('65,-116'), ('65,-117'), ('65,-150'), ('66,73'), ('66,72'), ('66,51'), ('66,50'), ('66,1'), ('66,0'), ('66,-50'), ('66,-51'), ('66,-72'), ('66,-73'), ('66,-113'), ('66,-114'), ('66,-115'), ('66,-117'), ('66,-150'), ('67,73'),
    ('67,72'), ('67,51'), ('67,50'), ('67,1'), ('67,0'), ('67,-50'), ('67,-51'), ('67,-72'), ('67,-73'), ('67,-117'), ('67,-150'), ('68,73'), ('68,72'), ('68,51'), ('68,50'), ('68,1'), ('68,0'), ('68,-50'), ('68,-51'), ('68,-72'),
    ('68,-73'), ('68,-150'), ('69,73'), ('69,72'), ('69,51'), ('69,50'), ('69,1'), ('69,0'), ('69,-50'), ('69,-51'), ('69,-72'), ('69,-73'), ('69,-150'), ('70,73'), ('70,72'), ('70,51'), ('70,50'), ('70,12'), ('70,11'), ('70,10'),
    ('70,9'), ('70,8'), ('70,7'), ('70,6'), ('70,5'), ('70,4'), ('70,3'), ('70,2'), ('70,1'), ('70,0'), ('70,-1'), ('70,-2'), ('70,-3'), ('70,-4'), ('70,-5'), ('70,-6'), ('70,-7'), ('70,-8'), ('70,-9'), ('70,-10'),
    ('70,-11'), ('70,-50'), ('70,-51'), ('70,-72'), ('70,-73'), ('70,-150'), ('71,73'), ('71,72'), ('71,51'), ('71,50'), ('71,12'), ('71,11'), ('71,10'), ('71,9'), ('71,8'), ('71,7'), ('71,6'), ('71,5'), ('71,4'), ('71,3'),
    ('71,2'), ('71,1'), ('71,0'), ('71,-1'), ('71,-2'), ('71,-3'), ('71,-4'), ('71,-5'), ('71,-6'), ('71,-7'), ('71,-8'), ('71,-9'), ('71,-10'), ('71,-11'), ('71,-50'), ('71,-51'), ('71,-72'), ('71,-73'), ('71,-150'), ('72,73'),
    ('72,72'), ('72,71'), ('72,70'), ('72,69'), ('72,68'), ('72,67'), ('72,66'), ('72,65'), ('72,64'), ('72,63'), ('72,62'), ('72,61'), ('72,60'), ('72,59'), ('72,58'), ('72,57'), ('72,56'), ('72,55'), ('72,54'), ('72,53'),
    ('72,52'), ('72,51'), ('72,50'), ('72,12'), ('72,11'), ('72,-10'), ('72,-11'), ('72,-38'), ('72,-39'), ('72,-40'), ('72,-41'), ('72,-42'), ('72,-43'), ('72,-44'), ('72,-45'), ('72,-46'), ('72,-47'), ('72,-48'), ('72,-49'), ('72,-50'),
    ('72,-51'), ('72,-52'), ('72,-53'), ('72,-54'), ('72,-55'), ('72,-56'), ('72,-57'), ('72,-58'), ('72,-59'), ('72,-60'), ('72,-61'), ('72,-62'), ('72,-63'), ('72,-64'), ('72,-65'), ('72,-66'), ('72,-67'), ('72,-68'), ('72,-69'), ('72,-70'),
    ('72,-71'), ('72,-72'), ('72,-73'), ('72,-74'), ('72,-75'), ('72,-76'), ('72,-77'), ('72,-78'), ('72,-79'), ('72,-80'), ('72,-81'), ('72,-82'), ('72,-83'), ('72,-84'), ('72,-85'), ('72,-86'), ('72,-87'), ('72,-88'), ('72,-89'), ('72,-90'),
    ('72,-91'), ('72,-92'), ('72,-93'), ('72,-94'), ('72,-95'), ('72,-96'), ('72,-97'), ('72,-98'), ('72,-99'), ('72,-100'), ('72,-101'), ('72,-102'), ('72,-103'), ('72,-104'), ('72,-105'), ('72,-106'), ('72,-107'), ('72,-108'), ('72,-109'), ('72,-110'),
    ('72,-111'), ('72,-112'), ('72,-113'), ('72,-114'), ('72,-115'), ('72,-116'), ('72,-117'), ('72,-118'), ('72,-119'), ('72,-120'), ('72,-121'), ('72,-122'), ('72,-123'), ('72,-124'), ('72,-125'), ('72,-126'), ('72,-127'), ('72,-128'), ('72,-129'), ('72,-130'),
    ('72,-131'), ('72,-132'), ('72,-133'), ('72,-134'), ('72,-135'), ('72,-136'), ('72,-137'), ('72,-138'), ('72,-139'), ('72,-140'), ('72,-141'), ('72,-142'), ('72,-143'), ('72,-144'), ('72,-145'), ('72,-146'), ('72,-147'), ('72,-148'), ('72,-149'), ('72,-150'),
    ('73,73'), ('73,72'), ('73,71'), ('73,70'), ('73,69'), ('73,68'), ('73,67'), ('73,66'), ('73,65'), ('73,64'), ('73,63'), ('73,62'), ('73,61'), ('73,60'), ('73,59'), ('73,58'), ('73,57'), ('73,56'), ('73,55'), ('73,54'),
    ('73,53'), ('73,52'), ('73,51'), ('73,50'), ('73,12'), ('73,11'), ('73,-10'), ('73,-11'), ('73,-38'), ('73,-39'), ('73,-40'), ('73,-41'), ('73,-42'), ('73,-43'), ('73,-44'), ('73,-45'), ('73,-46'), ('73,-47'), ('73,-48'), ('73,-49'),
    ('73,-50'), ('73,-51'), ('73,-52'), ('73,-53'), ('73,-54'), ('73,-55'), ('73,-56'), ('73,-57'), ('73,-58'), ('73,-59'), ('73,-60'), ('73,-61'), ('73,-62'), ('73,-63'), ('73,-64'), ('73,-65'), ('73,-66'), ('73,-67'), ('73,-68'), ('73,-69'),
    ('73,-70'), ('73,-71'), ('73,-72'), ('73,-73'), ('73,-74'), ('73,-75'), ('73,-76'), ('73,-77'), ('73,-78'), ('73,-79'), ('73,-80'), ('73,-81'), ('73,-82'), ('73,-83'), ('73,-84'), ('73,-85'), ('73,-86'), ('73,-87'), ('73,-88'), ('73,-89'),
    ('73,-90'), ('73,-91'), ('73,-92'), ('73,-93'), ('73,-94'), ('73,-95'), ('73,-96'), ('73,-97'), ('73,-98'), ('73,-99'), ('73,-100'), ('73,-101'), ('73,-102'), ('73,-103'), ('73,-104'), ('73,-105'), ('73,-106'), ('73,-107'), ('73,-108'), ('73,-109'),
    ('73,-110'), ('73,-111'), ('73,-112'), ('73,-113'), ('73,-114'), ('73,-115'), ('73,-116'), ('73,-117'), ('73,-118'), ('73,-119'), ('73,-120'), ('73,-121'), ('73,-122'), ('73,-123'), ('73,-124'), ('73,-125'), ('73,-126'), ('73,-127'), ('73,-128'), ('73,-129'),
    ('73,-130'), ('73,-131'), ('73,-132'), ('73,-133'), ('73,-134'), ('73,-135'), ('73,-136'), ('73,-137'), ('73,-138'), ('73,-139'), ('73,-140'), ('73,-141'), ('73,-142'), ('73,-143'), ('73,-144'), ('73,-145'), ('73,-146'), ('73,-147'), ('73,-148'), ('73,-149'),
    ('73,-150'), ('74,58'), ('74,57'), ('74,12'), ('74,11'), ('74,-10'), ('74,-11'), ('74,-38'), ('74,-150'), ('75,58'), ('75,57'), ('75,12'), ('75,11'), ('75,-10'), ('75,-11'), ('75,-38'), ('75,-150'), ('76,58'), ('76,57'), ('76,12'),
    ('76,11'), ('76,-10'), ('76,-11'), ('76,-38'), ('76,-150'), ('77,58'), ('77,57'), ('77,12'), ('77,11'), ('77,-10'), ('77,-11'), ('77,-38'), ('77,-150'), ('78,58'), ('78,57'), ('78,12'), ('78,11'), ('78,-10'), ('78,-11'), ('78,-38'),
    ('78,-150'), ('79,58'), ('79,57'), ('79,12'), ('79,11'), ('79,-10'), ('79,-11'), ('79,-38'), ('79,-150'), ('80,58'), ('80,57'), ('80,12'), ('80,11'), ('80,-10'), ('80,-11'), ('80,-38'), ('80,-150'), ('81,58'), ('81,57'), ('81,12'),
    ('81,11'), ('81,-10'), ('81,-11'), ('81,-38'), ('81,-150'), ('82,58'), ('82,57'), ('82,12'), ('82,11'), ('82,-10'), ('82,-11'), ('82,-38'), ('82,-150'), ('83,58'), ('83,57'), ('83,12'), ('83,11'), ('83,-10'), ('83,-11'), ('83,-38'),
    ('83,-150'), ('84,58'), ('84,57'), ('84,12'), ('84,11'), ('84,-10'), ('84,-11'), ('84,-38'), ('84,-150'), ('85,58'), ('85,57'), ('85,12'), ('85,11'), ('85,-10'), ('85,-11'), ('85,-38'), ('85,-150'), ('86,58'), ('86,57'), ('86,12'),
    ('86,11'), ('86,-10'), ('86,-11'), ('86,-38'), ('86,-150'), ('87,58'), ('87,57'), ('87,12'), ('87,11'), ('87,-10'), ('87,-11'), ('87,-38'), ('87,-150'), ('88,58'), ('88,57'), ('88,12'), ('88,11'), ('88,-10'), ('88,-11'), ('88,-38'),
    ('88,-150'), ('89,58'), ('89,57'), ('89,12'), ('89,11'), ('89,-10'), ('89,-11'), ('89,-38'), ('89,-150'), ('90,58'), ('90,57'), ('90,12'), ('90,11'), ('90,-10'), ('90,-11'), ('90,-38'), ('90,-150'), ('91,58'), ('91,57'), ('91,12'),
    ('91,11'), ('91,-10'), ('91,-11'), ('91,-38'), ('91,-150'), ('92,58'), ('92,57'), ('92,34'), ('92,33'), ('92,32'), ('92,31'), ('92,30'), ('92,29'), ('92,28'), ('92,27'), ('92,26'), ('92,25'), ('92,24'), ('92,23'), ('92,22'),
    ('92,21'), ('92,20'), ('92,19'), ('92,18'), ('92,17'), ('92,16'), ('92,15'), ('92,14'), ('92,13'), ('92,12'), ('92,11'), ('92,10'), ('92,9'), ('92,8'), ('92,7'), ('92,6'), ('92,5'), ('92,4'), ('92,3'), ('92,2'),
    ('92,1'), ('92,0'), ('92,-1'), ('92,-2'), ('92,-3'), ('92,-4'), ('92,-5'), ('92,-6'), ('92,-7'), ('92,-8'), ('92,-9'), ('92,-10'), ('92,-11'), ('92,-38'), ('92,-150'), ('93,58'), ('93,57'), ('93,35'), ('93,34'), ('93,33'),
    ('93,32'), ('93,31'), ('93,30'), ('93,29'), ('93,28'), ('93,27'), ('93,26'), ('93,25'), ('93,24'), ('93,23'), ('93,22'), ('93,21'), ('93,20'), ('93,19'), ('93,18'), ('93,17'), ('93,16'), ('93,15'), ('93,14'), ('93,13'),
    ('93,12'), ('93,11'), ('93,10'), ('93,9'), ('93,8'), ('93,7'), ('93,6'), ('93,5'), ('93,4'), ('93,3'), ('93,2'), ('93,1'), ('93,0'), ('93,-1'), ('93,-2'), ('93,-3'), ('93,-4'), ('93,-5'), ('93,-6'), ('93,-7'),
    ('93,-8'), ('93,-9'), ('93,-10'), ('93,-11'), ('93,-38'), ('93,-150'), ('94,58'), ('94,57'), ('94,0'), ('94,-1'), ('94,-38'), ('94,-150'), ('95,58'), ('95,57'), ('95,0'), ('95,-1'), ('95,-38'), ('95,-150'), ('96,58'), ('96,57'),
    ('96,0'), ('96,-1'), ('96,-38'), ('96,-150'), ('97,58'), ('97,57'), ('97,0'), ('97,-1'), ('97,-38'), ('97,-150'), ('98,58'), ('98,57'), ('98,0'), ('98,-1'), ('98,-38'), ('98,-150'), ('99,58'), ('99,57'), ('99,0'), ('99,-1'),
    ('99,-38'), ('99,-150'), ('100,58'), ('100,57'), ('100,0'), ('100,-1'), ('100,-38'), ('100,-150'), ('101,58'), ('101,57'), ('101,0'), ('101,-1'), ('101,-38'), ('101,-150'), ('102,58'), ('102,57'), ('102,0'), ('102,-1'), ('102,-38'), ('102,-150')
;

INSERT INTO explore.road_positions (position) VALUES
    ('103,58'), ('103,57'), ('103,0'), ('103,-1'), ('103,-38'), ('103,-150'), ('104,58'), ('104,57'), ('104,0'), ('104,-1'), ('104,-38'), ('104,-150'), ('105,58'), ('105,57'), ('105,0'), ('105,-1'), ('105,-38'), ('105,-150'), ('106,58'), ('106,57'),
    ('106,0'), ('106,-1'), ('106,-38'), ('106,-150'), ('107,58'), ('107,57'), ('107,0'), ('107,-1'), ('107,-38'), ('107,-150'), ('108,58'), ('108,57'), ('108,0'), ('108,-1'), ('108,-38'), ('108,-150'), ('109,58'), ('109,57'), ('109,0'), ('109,-1'),
    ('109,-38'), ('109,-150'), ('110,58'), ('110,57'), ('110,0'), ('110,-1'), ('110,-38'), ('110,-150'), ('111,58'), ('111,57'), ('111,0'), ('111,-1'), ('111,-38'), ('111,-150'), ('112,58'), ('112,57'), ('112,0'), ('112,-1'), ('112,-38'), ('112,-150'),
    ('113,58'), ('113,57'), ('113,0'), ('113,-1'), ('113,-38'), ('113,-150'), ('114,58'), ('114,57'), ('114,0'), ('114,-1'), ('114,-38'), ('114,-150'), ('115,58'), ('115,57'), ('115,0'), ('115,-1'), ('115,-38'), ('115,-150'), ('116,58'), ('116,57'),
    ('116,0'), ('116,-1'), ('116,-38'), ('116,-150'), ('117,58'), ('117,57'), ('117,0'), ('117,-1'), ('117,-38'), ('117,-150'), ('118,58'), ('118,57'), ('118,0'), ('118,-1'), ('118,-38'), ('118,-150'), ('119,58'), ('119,57'), ('119,0'), ('119,-1'),
    ('119,-38'), ('119,-150'), ('120,58'), ('120,57'), ('120,0'), ('120,-1'), ('120,-38'), ('120,-150'), ('121,58'), ('121,57'), ('121,0'), ('121,-1'), ('121,-38'), ('121,-150'), ('122,58'), ('122,57'), ('122,0'), ('122,-1'), ('122,-38'), ('122,-150'),
    ('123,58'), ('123,57'), ('123,0'), ('123,-1'), ('123,-38'), ('123,-150'), ('124,58'), ('124,57'), ('124,0'), ('124,-1'), ('124,-38'), ('124,-150'), ('125,58'), ('125,57'), ('125,0'), ('125,-1'), ('125,-38'), ('125,-150'), ('126,58'), ('126,57'),
    ('126,0'), ('126,-1'), ('126,-38'), ('126,-150'), ('127,58'), ('127,57'), ('127,0'), ('127,-1'), ('127,-38'), ('127,-150'), ('128,58'), ('128,57'), ('128,0'), ('128,-1'), ('128,-38'), ('128,-150'), ('129,58'), ('129,57'), ('129,0'), ('129,-1'),
    ('129,-38'), ('129,-150'), ('130,58'), ('130,57'), ('130,0'), ('130,-1'), ('130,-38'), ('130,-150'), ('131,58'), ('131,57'), ('131,0'), ('131,-1'), ('131,-38'), ('131,-150'), ('132,58'), ('132,57'), ('132,0'), ('132,-1'), ('132,-38'), ('132,-150'),
    ('133,58'), ('133,57'), ('133,0'), ('133,-1'), ('133,-38'), ('133,-150'), ('134,58'), ('134,57'), ('134,0'), ('134,-1'), ('134,-38'), ('134,-150'), ('135,58'), ('135,57'), ('135,0'), ('135,-1'), ('135,-38'), ('135,-150'), ('136,58'), ('136,57'),
    ('136,0'), ('136,-1'), ('136,-38'), ('136,-150'), ('137,58'), ('137,57'), ('137,0'), ('137,-1'), ('137,-38'), ('137,-150'), ('138,58'), ('138,57'), ('138,0'), ('138,-1'), ('138,-38'), ('138,-150'), ('139,58'), ('139,57'), ('139,0'), ('139,-1'),
    ('139,-38'), ('139,-150'), ('140,58'), ('140,57'), ('140,0'), ('140,-1'), ('140,-38'), ('140,-150'), ('141,58'), ('141,57'), ('141,0'), ('141,-1'), ('141,-38'), ('141,-150'), ('142,58'), ('142,57'), ('142,0'), ('142,-1'), ('142,-38'), ('142,-150'),
    ('143,58'), ('143,57'), ('143,0'), ('143,-1'), ('143,-38'), ('143,-150'), ('144,58'), ('144,57'), ('144,0'), ('144,-1'), ('144,-38'), ('144,-150'), ('145,58'), ('145,57'), ('145,0'), ('145,-1'), ('145,-38'), ('145,-150'), ('146,58'), ('146,57'),
    ('146,0'), ('146,-1'), ('146,-38'), ('146,-150'), ('147,58'), ('147,57'), ('147,0'), ('147,-1'), ('147,-38'), ('147,-150'), ('148,58'), ('148,57'), ('148,0'), ('148,-1'), ('148,-38'), ('148,-150'), ('149,58'), ('149,57'), ('149,0'), ('149,-38'),
    ('149,-150'), ('150,58'), ('150,57'), ('150,0'), ('150,-1'), ('150,-2'), ('150,-3'), ('150,-4'), ('150,-5'), ('150,-6'), ('150,-7'), ('150,-8'), ('150,-9'), ('150,-10'), ('150,-11'), ('150,-12'), ('150,-13'), ('150,-14'), ('150,-15'), ('150,-16'),
    ('150,-17'), ('150,-18'), ('150,-19'), ('150,-20'), ('150,-21'), ('150,-22'), ('150,-23'), ('150,-24'), ('150,-25'), ('150,-26'), ('150,-27'), ('150,-28'), ('150,-29'), ('150,-30'), ('150,-31'), ('150,-32'), ('150,-33'), ('150,-34'), ('150,-35'), ('150,-36'),
    ('150,-37'), ('150,-38'), ('150,-39'), ('150,-40'), ('150,-41'), ('150,-42'), ('150,-43'), ('150,-44'), ('150,-45'), ('150,-46'), ('150,-47'), ('150,-48'), ('150,-49'), ('150,-50'), ('150,-51'), ('150,-52'), ('150,-53'), ('150,-54'), ('150,-55'), ('150,-56'),
    ('150,-57'), ('150,-58'), ('150,-59'), ('150,-60'), ('150,-61'), ('150,-62'), ('150,-63'), ('150,-64'), ('150,-65'), ('150,-66'), ('150,-67'), ('150,-68'), ('150,-69'), ('150,-70'), ('150,-71'), ('150,-72'), ('150,-73'), ('150,-74'), ('150,-75'), ('150,-76'),
    ('150,-77'), ('150,-78'), ('150,-79'), ('150,-80'), ('150,-81'), ('150,-82'), ('150,-83'), ('150,-84'), ('150,-85'), ('150,-86'), ('150,-87'), ('150,-88'), ('150,-89'), ('150,-90'), ('150,-91'), ('150,-92'), ('150,-93'), ('150,-94'), ('150,-95'), ('150,-96'),
    ('150,-97'), ('150,-98'), ('150,-99'), ('150,-100'), ('150,-101'), ('150,-102'), ('150,-103'), ('150,-104'), ('150,-105'), ('150,-106'), ('150,-107'), ('150,-108'), ('150,-109'), ('150,-110'), ('150,-111'), ('150,-112'), ('150,-113'), ('150,-114'), ('150,-115'), ('150,-116'),
    ('150,-117'), ('150,-118'), ('150,-119'), ('150,-120'), ('150,-121'), ('150,-122'), ('150,-123'), ('150,-124'), ('150,-125'), ('150,-126'), ('150,-127'), ('150,-128'), ('150,-129'), ('150,-130'), ('150,-131'), ('150,-132'), ('150,-133'), ('150,-134'), ('150,-135'), ('150,-136'),
    ('150,-137'), ('150,-138'), ('150,-139'), ('150,-140'), ('150,-141'), ('150,-142'), ('150,-143'), ('150,-144'), ('150,-145'), ('150,-146'), ('150,-147'), ('150,-148'), ('150,-149'), ('150,-150')
;

-- ── explorer statics: satellite renders, the world manifest, event posters ─

-- Each row below is a keyed index into the ONE blob store
-- (<STORAGE_ROOT_FOLDER>/contents/<shard>/<blob_cid>, hash-addressed and
-- immutable once landed): it names the upstream object the bytes came from,
-- when they landed (fetched_at) and when the upstream last confirmed them
-- (checked_at). No blob is ever unlinked by these writers (the store is
-- shared); a store-wide sweep must count blob_cid here as a reference.

-- The satellite region renders (src/explore/api/satellite.rs): one row per
-- region centre (the 17x17-parcel top-down PNG at <cx>/<cy>.png) and variant
-- ('full', or 'thumb': <cy>.t.png, sampled at zoom <= 4). Mirrored from
-- SATELLITE_MIRROR_UPSTREAM_URL/manifest.json by the `satellite` pass
-- (explore_api.mirror_passes): a row whose checked_at is at or after the
-- in-flight pass's started_at was done by that pass (the resume cursor); a
-- complete pass deletes the regions its manifest no longer lists. The PK
-- serves the per-region upsert/restamp and the whole-table index load.
CREATE TABLE explore.satellite_region (
    cx             integer NOT NULL,
    cy             integer NOT NULL,
    variant        text NOT NULL,
    blob_cid       text NOT NULL,
    byte_size      bigint NOT NULL,
    width          integer NOT NULL,
    height         integer NOT NULL,
    last_modified  text,
    etag           text,
    fetched_at     timestamp without time zone NOT NULL,
    checked_at     timestamp without time zone NOT NULL,
    PRIMARY KEY (cx, cy, variant),
    CONSTRAINT satellite_region_variant_known CHECK (variant IN ('full', 'thumb')),
    CONSTRAINT satellite_region_in_world CHECK (cx BETWEEN -512 AND 512 AND cy BETWEEN -512 AND 512),
    CONSTRAINT satellite_region_sized CHECK (byte_size > 0 AND width > 0 AND height > 0)
);

-- The genesis WorldManifest.json served at /world-manifest.json
-- (src/explore/api/world_manifest.rs), mirrored verbatim from
-- WORLD_MANIFEST_UPSTREAM_URL by the `world-manifest` pass. One row; a body
-- that is not an object of occupied/roads/empty arrays with at least one
-- occupied parcel never replaces it (the old timer's jq gate).
CREATE TABLE explore.world_manifest (
    id              smallint PRIMARY KEY,
    blob_cid        text NOT NULL,
    byte_size       bigint NOT NULL,
    occupied_count  integer NOT NULL,
    roads_count     integer NOT NULL,
    empty_count     integer NOT NULL,
    etag            text,
    last_modified   text,
    fetched_at      timestamp without time zone NOT NULL,
    checked_at      timestamp without time zone NOT NULL,
    CONSTRAINT world_manifest_singleton CHECK (id = 1),
    CONSTRAINT world_manifest_counts CHECK (occupied_count > 0 AND roads_count >= 0 AND empty_count >= 0 AND byte_size > 0)
);

-- The event poster bucket, events-assets-099ac00.<domain>/poster[-vertical]/<file>
-- (src/explore/api/posters.rs): 'upstream' rows are the read-through of
-- EVENTS_POSTER_UPSTREAM_URL on a first request, 'local' rows the events
-- write routes' uploads (the old events content store's <sha256>.<ext>
-- names); a read-through never replaces a local row. The PK serves the one
-- lookup, `kind = $1 AND file = $2`.
CREATE TABLE explore.poster (
    kind          text NOT NULL,
    file          text NOT NULL,
    blob_cid      text NOT NULL,
    content_type  text NOT NULL,
    byte_size     bigint NOT NULL,
    origin        text NOT NULL,
    fetched_at    timestamp without time zone NOT NULL,
    PRIMARY KEY (kind, file),
    CONSTRAINT poster_kind_known CHECK (kind IN ('poster', 'poster-vertical')),
    CONSTRAINT poster_origin_known CHECK (origin IN ('upstream', 'local')),
    CONSTRAINT poster_file_plain CHECK (file ~ '^[A-Za-z0-9][A-Za-z0-9._-]{0,199}$'),
    CONSTRAINT poster_type_known CHECK (content_type IN ('image/png', 'image/jpeg', 'image/gif', 'image/webp', 'application/octet-stream')),
    CONSTRAINT poster_sized CHECK (byte_size > 0)
);

-- The OpenSea metadata resolver's answers (src/explore/opensea.rs), the old
-- opensea.<domain> nginx proxy cache: derived, keyed by the request (chain as
-- asked, lowercased contract, the token id's digits), the whole envelope kept
-- as the text served (json, not jsonb: read as a unit, never queried into).
-- Fresh for 30 days after resolved_at; a stale row is re-resolved on the next
-- request and served while that fails; source names the leg that produced it.
CREATE TABLE explore.opensea_nft (
    chain             text NOT NULL,
    contract_address  text NOT NULL,
    token_id          text NOT NULL,
    document          json NOT NULL,
    source            text NOT NULL,
    resolved_at       timestamp without time zone NOT NULL,
    PRIMARY KEY (chain, contract_address, token_id),
    CONSTRAINT opensea_nft_chain_known CHECK (chain IN ('ethereum', 'matic', 'polygon')),
    CONSTRAINT opensea_nft_contract_lowercase CHECK (contract_address ~ '^0x[0-9a-f]{40}$'),
    CONSTRAINT opensea_nft_token_digits CHECK (token_id ~ '^[0-9]{1,78}$'),
    CONSTRAINT opensea_nft_source_known CHECK (source IN ('local', 'cryptokitties', 'chain', 'fallback'))
);
-- The hourly expiry job's `DELETE ... WHERE ctid IN (SELECT ctid ... WHERE
-- resolved_at < $1 LIMIT 1000)` (rows past twice the freshness window).
CREATE INDEX opensea_nft_resolved_at_idx ON explore.opensea_nft (resolved_at);

-- ════ schema/11_explore_api.sql ════

-- Catalyst v2: explore.api domain: the explorer-api surface unity-explorer hits
-- at boot (realm-provider, dcl-lists, config denylist, feature flags, the
-- api.decentraland.org tile grid; the asset-bundle registry reads are a cached
-- upstream reader with no table). Derived from
-- catalyrst/crates/catalyrst-places/migrations/0001_lists.sql (lists) and the
-- file/asset-backed state of catalyrst-explorer-api and catalyrst-map.
--
-- Mirrored rows are the upstream's; the local overlay lives in the *_local
-- tables and is never touched by a mirror pass, so a mirror can neither
-- overwrite nor delete what an operator set here. Every mirror records its
-- passes in mirror_passes: a row whose completed_at is at or after started_at
-- is a complete pass, and the cursor is what the next pass resumes from.

CREATE SCHEMA explore_api;

CREATE TABLE explore_api.mirror_passes (
    mirror text PRIMARY KEY,
    passes bigint NOT NULL DEFAULT 0,
    items bigint NOT NULL DEFAULT 0,
    cursor text,
    started_at timestamp without time zone,
    completed_at timestamp without time zone,
    last_error text
);

-- dcl-lists: POST /pois and POST /banned-names, one row per value.
CREATE TABLE explore_api.lists (
    list text NOT NULL,
    value text NOT NULL,
    first_seen_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    PRIMARY KEY (list, value)
);

-- present = true adds a value the upstream lacks; false hides a mirrored one.
CREATE TABLE explore_api.lists_local (
    list text NOT NULL,
    value text NOT NULL,
    present boolean NOT NULL,
    updated_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    PRIMARY KEY (list, value)
);

-- config.decentraland.org/denylist.json users[].wallet; keyed lowercase, served as received.
CREATE TABLE explore_api.denylist_users (
    wallet text PRIMARY KEY,
    wallet_raw text NOT NULL,
    first_seen_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now())
);

CREATE TABLE explore_api.denylist_users_local (
    wallet text PRIMARY KEY,
    present boolean NOT NULL,
    updated_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now())
);

-- feature-flags.decentraland.org/<app>.json as received (flags + variants).
CREATE TABLE explore_api.feature_flags (
    app text PRIMARY KEY,
    document jsonb NOT NULL,
    etag text,
    fetched_at timestamp without time zone NOT NULL
);

CREATE TABLE explore_api.feature_flags_local (
    app text NOT NULL,
    name text NOT NULL,
    value boolean NOT NULL,
    updated_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    PRIMARY KEY (app, name)
);

-- The api.decentraland.org /v2/tiles grid (about 92k tiles, 38 MB of JSON),
-- gzip-compressed as one row so a restart renders map.png before the first
-- pass; last_updated_at is the upstream /v2/tiles/info stamp the grid carries.
CREATE TABLE explore_api.map_grid (
    id smallint PRIMARY KEY CHECK (id = 1),
    last_updated_at bigint NOT NULL,
    etag text,
    tile_count integer NOT NULL,
    body_gzip bytea NOT NULL,
    fetched_at timestamp without time zone NOT NULL
);

-- ════ schema/20_worlds.sql ════

-- Catalyst v2: worlds domain: the world catalogue mirror, the world
-- scenes/files mirror, the worlds read routes (/world/{name}/about,
-- /world/{name}/scenes, /index, /worlds/status), and (stage 5) the permission
-- core, world settings, world/player/env storage and the access log behind
-- the local publish, permission, settings, comms and storage routes.
--
-- Derived from catalyrst/crates/catalyrst-worlds/migrations 0001 (worlds,
-- world_scenes, world_permissions, blocked), 0002 (world_access_log), 0003
-- (world_permission_parcels), 0004 (lower(name) indexes), 0007..0009
-- (world_storage, player_storage, env_variables and their place-aware size
-- indexes, lowercase world names), 0010 (settings_version), 0011
-- (world_scenes.updated_at), 0012 (realm_name_override), 0013
-- (preview_wearable_urns), collapsed to the current shape. Workload, keys,
-- invariants and the query behind every index: schema/WORKLOADS.md, "worlds".
--
-- Workload (measured 2026-09-24 against worlds-content-server.decentraland.org):
-- the upstream catalogue lists 1,667 worlds and 2,517 scenes; GET /index is one
-- 4.3 MB document per catalogue pass. A scene entity is a few KB of JSON and
-- references tens to hundreds of hash-addressed files (MBs) that go to the
-- shared blob store, not here. Reads are one world at a time by name
-- (/about, /scenes), plus the index scan and two counts for /status. Writes
-- are one bounded transaction per mirrored world and one per catalogue pass.
--
-- Authority. Mirrored rows are distinguishable from local publishes by
-- `mirror_source` (the upstream URL) being NOT NULL; a local publish writes
-- NULL. The mirror never touches a world that holds a local scene, and only
-- retires scenes it wrote itself; it never writes owner, access, permissions
-- or storage rows, so a mirrored world grants nobody deploy, stream or host
-- authority here. The zero-address `deployer` on mirrored scenes is kept for
-- the wire (the old /scenes shape and the old marker).
--
-- Conventions: world names are stored lowercase (CHECKed; every old local
-- write path inserted lower($1) and the catalogue lists lowercase names) and
-- read with name = $key, the lowercase key every reader binds
-- (src/worlds/identity.rs::WorldName; a raw input is bound as lower($1)), so
-- the PKs serve every lookup and no lower() expression index exists; wallet
-- addresses are lowercase 0x + 40
-- hex (social::Address); parcels are canonical "x,y"; every timestamp column
-- is `timestamp without time zone` holding UTC (the pool pins TimeZone=UTC);
-- the current time is a bind parameter, never an index predicate. No FK
-- crosses a schema: the scene admins and bans a world scene is moderated by
-- are `social.scene_admins`/`social.scene_bans` keyed by the scene entity id
-- (src/worlds/policy.rs reads them; comms owns their writes).
--
-- The federation listing mirror of 0005/0006 (peers' public /worlds listings,
-- a different protocol from the upstream /index catalogue) is the
-- `worlds-federation` section at the end: worlds.federation_worlds and
-- worlds.federation_peer_status, renamed because worlds.remote_worlds here is
-- the catalogue. Not carried over: the old `worlds.size` column (unused after
-- upstream 0016; a world's size is sum(world_scenes.size)).
--
-- Domain agents append ONLY inside the `-- ==== section: ... ====` blocks at
-- the end (scene host, storage routes, worlds federation), after re-reading
-- this file.

CREATE SCHEMA worlds;

-- ── worlds and scenes ──────────────────────────────────────────────────────

-- One row per world this node knows: mirrored (mirror_source) or published
-- here. `owner` is the wallet the last local publish resolved as the NAME
-- owner (personal world: the address itself; else the name authority), the
-- second step of the owner lookup (src/worlds/policy.rs, "owner"); it is
-- never written by the mirror and never by a permission edit alone (those
-- create the row with the signer as owner only after the signer proved
-- ownership). The access setting is three typed columns (the old JSONB
-- document): the kind, the bcrypt hash for shared-secret, the NFT urn for
-- nft-ownership; an allow-list's wallets are world_permissions rows with
-- permission = 'access' and its communities world_access_communities rows.
-- `settings_version` is bumped by every settings-affecting write (deploy
-- refresh, PUT /settings, access edits, mirror ingest) under the row lock so
-- a settings consumer can refuse an out-of-order update.
CREATE TABLE worlds.worlds (
    name text NOT NULL,
    owner text,
    access_type text NOT NULL DEFAULT 'unrestricted',
    access_secret_hash text,
    access_nft text,
    blocked_since timestamp without time zone,
    spawn_coordinates text,
    title text,
    description text,
    content_rating text,
    skybox_time integer,
    categories text[],
    single_player boolean NOT NULL DEFAULT false,
    show_in_places boolean NOT NULL DEFAULT true,
    thumbnail_hash text,
    preview_wearable_urns text[],
    settings_version bigint NOT NULL DEFAULT 0,
    realm_name_override text,
    mirror_source text,
    created_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    updated_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    CONSTRAINT worlds_pkey PRIMARY KEY (name),
    CONSTRAINT worlds_name_lowercase CHECK (name = lower(name) AND btrim(name) <> ''),
    CONSTRAINT worlds_owner_lowercase_address CHECK (owner IS NULL OR owner ~ '^0x[0-9a-f]{40}$'),
    CONSTRAINT worlds_access_type_check
        CHECK (access_type IN ('unrestricted', 'shared-secret', 'nft-ownership', 'allow-list')),
    CONSTRAINT worlds_access_secret_check
        CHECK ((access_type = 'shared-secret') = (access_secret_hash IS NOT NULL)),
    CONSTRAINT worlds_access_nft_check
        CHECK ((access_type = 'nft-ownership') = (access_nft IS NOT NULL)),
    CONSTRAINT worlds_content_rating_check
        CHECK (content_rating IS NULL OR content_rating IN ('RP', 'E', 'T', 'A', 'R')),
    CONSTRAINT worlds_categories_check
        CHECK (categories IS NULL OR cardinality(categories) <= 20),
    CONSTRAINT worlds_preview_wearables_check
        CHECK (preview_wearable_urns IS NULL OR cardinality(preview_wearable_urns) <= 8),
    CONSTRAINT worlds_settings_version_nonneg CHECK (settings_version >= 0)
);

-- /world/{name}/about, /world/{name}/scenes, the mirror, the permission core
-- and the room resolver look worlds up by the lowercase key and read one
-- row: the PK is the one-world-per-name invariant and the one index (the
-- stage-4 readers' lower(name) expression index went with their predicate,
-- 2026-09-25).

CREATE TABLE worlds.world_scenes (
    world_name text NOT NULL,
    entity_id text NOT NULL,
    deployment_auth_chain json NOT NULL,
    entity jsonb NOT NULL,
    deployer text NOT NULL,
    parcels text[] NOT NULL,
    size bigint NOT NULL,
    mirror_source text,
    created_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    updated_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    CONSTRAINT world_scenes_pkey PRIMARY KEY (world_name, entity_id),
    CONSTRAINT world_scenes_size_nonneg CHECK (size >= 0),
    CONSTRAINT fk_world_name FOREIGN KEY (world_name)
        REFERENCES worlds.worlds (name) ON DELETE CASCADE
);

-- The about/scenes lookup and its newest-first order; also the room
-- resolver's "active scene of a world" (ORDER BY created_at DESC LIMIT 1) and
-- the deploy path's overlap probe (`parcels && $2` under the same prefix).
-- Keyed on the stored lowercase column (every reader binds the key), so the
-- PK prefix and this index are the two shapes one predicate uses.
CREATE INDEX world_scenes_world_name_created_idx
    ON worlds.world_scenes (world_name, created_at DESC);

-- ── the permission core ────────────────────────────────────────────────────

-- An allow-list grant on a world: `deployment` (may publish), `streaming`
-- (may stream), `access` (may enter an allow-list world). Owners are not
-- rows: ownership is resolved from the name authority (policy.rs). A grant
-- is WORLD-WIDE when it has no world_permission_parcels row and
-- PARCEL-SCOPED when it has one or more; granting world-wide clears the
-- parcel rows (the old upstream rule). Keyed on the world's lowercase name
-- and the lowercase address, the spellings every old statement compared
-- with lower(); the FK follows the world's lifecycle.
CREATE TABLE worlds.world_permissions (
    world_name text NOT NULL,
    permission text NOT NULL,
    address text NOT NULL,
    created_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    updated_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    CONSTRAINT world_permissions_pkey PRIMARY KEY (world_name, permission, address),
    CONSTRAINT world_permissions_permission_check
        CHECK (permission IN ('deployment', 'streaming', 'access')),
    CONSTRAINT world_permissions_address_lowercase CHECK (address ~ '^0x[0-9a-f]{40}$'),
    CONSTRAINT world_permissions_world_lowercase CHECK (world_name = lower(world_name)),
    CONSTRAINT fk_world_permissions_world FOREIGN KEY (world_name)
        REFERENCES worlds.worlds (name) ON DELETE CASCADE
);

-- The parcels a grant is scoped to, canonical "x,y". Cascades with the
-- grant; the PK is the membership probe of the coverage predicate
-- (policy.rs::has_permission_covering) and of the parcel listing.
CREATE TABLE worlds.world_permission_parcels (
    world_name text NOT NULL,
    permission text NOT NULL,
    address text NOT NULL,
    parcel text NOT NULL,
    created_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    CONSTRAINT world_permission_parcels_pkey PRIMARY KEY (world_name, permission, address, parcel),
    CONSTRAINT world_permission_parcels_canonical CHECK (parcel ~ '^-?(0|[1-9][0-9]*),-?(0|[1-9][0-9]*)$'),
    CONSTRAINT fk_world_permission_parcels_grant FOREIGN KEY (world_name, permission, address)
        REFERENCES worlds.world_permissions (world_name, permission, address) ON DELETE CASCADE
);

-- The communities an allow-list world admits (the old access.communities).
-- No FK to social.communities (different lifecycle); reconciled on read.
CREATE TABLE worlds.world_access_communities (
    world_name text NOT NULL,
    community_id uuid NOT NULL,
    created_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    CONSTRAINT world_access_communities_pkey PRIMARY KEY (world_name, community_id),
    CONSTRAINT world_access_communities_world_lowercase CHECK (world_name = lower(world_name)),
    CONSTRAINT fk_world_access_communities_world FOREIGN KEY (world_name)
        REFERENCES worlds.worlds (name) ON DELETE CASCADE
);

-- Wallets an operator blocked on this node (the old `blocked` table: the
-- over-storage / platform block the admin routes manage). A blocked wallet
-- is refused every world room join (policy.rs, "blocked"; the old comms
-- mint's wallet_blocked probe); the old deploy path did not consult it and
-- neither does can_deploy.
CREATE TABLE worlds.blocked_wallets (
    wallet text NOT NULL,
    created_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    updated_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    CONSTRAINT blocked_wallets_pkey PRIMARY KEY (wallet),
    CONSTRAINT blocked_wallets_lowercase_address CHECK (wallet ~ '^0x[0-9a-f]{40}$')
);

-- ── access log ─────────────────────────────────────────────────────────────

-- Who entered/left which world room, from the LiveKit webhook's
-- participant_joined/left events (the old world_access_log). Append-only,
-- batched inserts; read by the bearer-gated admin listing.
CREATE TABLE worlds.world_access_log (
    id bigint GENERATED ALWAYS AS IDENTITY,
    world_name text NOT NULL,
    address text NOT NULL,
    action text NOT NULL,
    room text NOT NULL,
    created_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    CONSTRAINT world_access_log_pkey PRIMARY KEY (id),
    CONSTRAINT world_access_log_action_check CHECK (action IN ('join', 'leave')),
    CONSTRAINT world_access_log_world_lowercase CHECK (world_name = lower(world_name)),
    CONSTRAINT world_access_log_address_lowercase CHECK (address = lower(address))
);

-- The admin listing `WHERE ($1 IS NULL OR world_name = $1) AND ($2 IS NULL OR
-- address = $2) ORDER BY created_at DESC, id DESC LIMIT/OFFSET`: one index per
-- filter arm and one for the unfiltered walk (the old three, on the stored
-- lowercase columns instead of lower() expressions).
CREATE INDEX world_access_log_world_idx ON worlds.world_access_log (world_name, created_at DESC, id DESC);
CREATE INDEX world_access_log_address_idx ON worlds.world_access_log (address, created_at DESC, id DESC);
CREATE INDEX world_access_log_created_idx ON worlds.world_access_log (created_at DESC, id DESC);

-- ── world, player and env storage ──────────────────────────────────────────

-- Three key/value namespaces a running scene reads and writes through the
-- scoped storage API (the old catalyrst-world-storage tables), scoped by
-- `(realm, scene_namespace)` (src/worlds/identity.rs::SceneNamespace):
--   realm           the world's lowercase name for a world scene, 'main' for
--                   any shared realm (Genesis City; the old GENESIS_CITY_REALM)
--   scene_namespace 'world' for a world scene (the old world-storage keyed
--                   rows on the Places id and Places kept one row per world
--                   name, so every scene of a world shared one scope and a
--                   redeploy that moved its base kept it); for a Genesis
--                   scene the canonical base parcel of its active deployment
--                   (the LAND place the old Places id stood for; a moved base
--                   is a new place, as it was)
-- and NOT the deployment's content hash: a redeploy of the scene keeps its
-- rows, and an undeploy does not touch them (no FK to world_scenes; a
-- world's rows survive its world row too). Only the explicit clear routes
-- (DELETE /values, /players, /env, owner/deployer authorized) remove them.
-- `value_size` is the serialized value's byte length, kept beside the value
-- so the quota aggregate never reads the (TOASTed) jsonb. env values are
-- AES-256-GCM ciphertext; plaintext never reaches the database.
CREATE TABLE worlds.world_storage (
    realm text NOT NULL,
    scene_namespace text NOT NULL,
    key text NOT NULL,
    value jsonb NOT NULL,
    value_size integer NOT NULL,
    created_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    updated_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    CONSTRAINT world_storage_pkey PRIMARY KEY (realm, scene_namespace, key),
    CONSTRAINT world_storage_realm_lowercase CHECK (realm = lower(realm) AND realm <> ''),
    CONSTRAINT world_storage_key_len CHECK (octet_length(key) BETWEEN 1 AND 1020),
    CONSTRAINT world_storage_size_nonneg CHECK (value_size >= 0)
);

CREATE TABLE worlds.player_storage (
    realm text NOT NULL,
    scene_namespace text NOT NULL,
    player_address text NOT NULL,
    key text NOT NULL,
    value jsonb NOT NULL,
    value_size integer NOT NULL,
    created_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    updated_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    CONSTRAINT player_storage_pkey PRIMARY KEY (realm, scene_namespace, player_address, key),
    CONSTRAINT player_storage_realm_lowercase CHECK (realm = lower(realm) AND realm <> ''),
    CONSTRAINT player_storage_player_lowercase_address CHECK (player_address ~ '^0x[0-9a-f]{40}$'),
    CONSTRAINT player_storage_key_len CHECK (octet_length(key) BETWEEN 1 AND 1020),
    CONSTRAINT player_storage_size_nonneg CHECK (value_size >= 0)
);

CREATE TABLE worlds.env_variables (
    realm text NOT NULL,
    scene_namespace text NOT NULL,
    key text NOT NULL,
    value_enc bytea NOT NULL,
    value_size integer NOT NULL,
    created_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    updated_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    CONSTRAINT env_variables_pkey PRIMARY KEY (realm, scene_namespace, key),
    CONSTRAINT env_variables_realm_lowercase CHECK (realm = lower(realm) AND realm <> ''),
    CONSTRAINT env_variables_key_len CHECK (octet_length(key) BETWEEN 1 AND 1020),
    CONSTRAINT env_variables_size_nonneg CHECK (value_size >= 0)
);

-- The per-write quota aggregate, taken under pg_advisory_xact_lock of the
-- namespace scope: `SELECT COALESCE(MAX(value_size) FILTER (WHERE
-- scene_namespace = $2 AND key = $3), 0), COALESCE(SUM(value_size), 0) FROM
-- <table> WHERE realm = $1 [AND scene_namespace = $2]` (a world's quota spans
-- all its scenes; a shared realm's is per scene), and the /usage routes.
-- value_size in the leaf keeps them index-only; the PK cannot (it lacks the
-- size). The player index leads with the player so a player's world-wide
-- total is one range. Inherited from upstream 1782950400000 and unmeasured
-- on v2 (WORKLOADS.md names the alternative).
CREATE INDEX world_storage_size_idx
    ON worlds.world_storage (realm, scene_namespace) INCLUDE (key, value_size);
CREATE INDEX player_storage_size_idx
    ON worlds.player_storage (realm, player_address, scene_namespace) INCLUDE (key, value_size);
CREATE INDEX env_variables_size_idx
    ON worlds.env_variables (realm, scene_namespace) INCLUDE (key, value_size);

-- ── mirrors ────────────────────────────────────────────────────────────────

-- The catalogue: one row per world the upstream /index listed, with the scene
-- ids it advertised, so the scenes mirror can tell an unchanged world from the
-- catalogue alone. `unlisted_at` is set by a complete catalogue pass that no
-- longer lists the world; the scenes pass then retires its mirrored scenes.
CREATE TABLE worlds.remote_worlds (
    world_name text NOT NULL,
    source text NOT NULL,
    scene_ids text[] NOT NULL,
    newest_timestamp_ms bigint,
    first_seen_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    listed_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    unlisted_at timestamp without time zone,
    mirrored_at timestamp without time zone,
    last_error text,
    CONSTRAINT remote_worlds_pkey PRIMARY KEY (world_name),
    CONSTRAINT remote_worlds_name_lowercase CHECK (world_name = lower(world_name))
);

-- One row per mirror ('catalogue', 'scenes'). `cursor` is the last world name
-- the scenes pass completed (NULL between passes), persisted after every
-- batch so a restart resumes there; `pass_completed_at` with a NULL cursor is
-- the complete-pass proof, `stats` its counts.
CREATE TABLE worlds.mirror_passes (
    mirror text NOT NULL,
    source text NOT NULL,
    pass_number bigint NOT NULL DEFAULT 0,
    pass_started_at timestamp without time zone,
    pass_completed_at timestamp without time zone,
    last_attempt_at timestamp without time zone,
    cursor text,
    stats jsonb NOT NULL DEFAULT '{}'::jsonb,
    last_error text,
    CONSTRAINT mirror_passes_pkey PRIMARY KEY (mirror),
    CONSTRAINT mirror_passes_known_mirror CHECK (mirror IN ('catalogue', 'scenes'))
);

-- ══ Domain-agent sections: append inside the matching block only ═══════════

-- ==== section: scene-host ====
-- Left to the authoritative scene host agent. The old catalyrst-scene-state
-- persisted nothing: on restart it re-resolved the world's active scene
-- (/world/{name}/about scenesUrn[0], now rooms.rs::resolve), reloaded the
-- files from the blob store and minted a fresh storage delegation; live
-- CRDT/tick state and client entity ranges stay in the runtime; player state
-- is player_storage above. A host lease (fencing epoch per (realm,
-- scene_namespace), for a host running as its own process or twice) is the
-- one relation the restart contract may need; WORKLOADS.md, "scene host"
-- keeps its design under "NOT created" until the host lands with a consumer.
-- Landed 2026-09-25 (src/worlds/scene_state): no relation. One in-process
-- host; a restart re-resolves through rooms.rs (world_active_scene,
-- genesis_scene, genesis_scene_at), reloads game.js/main.crdt from the blob
-- store and re-mints the delegation (SCENE_HOST_AUTHORITATIVE_KEY in-process,
-- the old deploy-signer envelope, or the external minter); the
-- scene-host-sweep job re-reads the same rows to reload a redeployed scene
-- and re-admit every socket. scene_host_leases stays NOT created until a
-- second host instance exists.
-- ==== end section: scene-host ====

-- ==== section: storage-routes ====
-- Left to the storage routes agent: nothing expected. A storage_usage
-- projection replacing the three size indexes is a measured decision
-- (WORKLOADS.md, "world_storage, player_storage, env_variables").
-- ==== end section: storage-routes ====

-- ==== section: worlds-federation ====
-- The worlds federation read mirror (src/worlds/fed): what admitted peers say
-- on their own public /worlds listings, held under the peer that said it and
-- served only on /federation/worlds/* qualified by peerId. Migrations 0005
-- (remote_worlds, remote_peer_status) and 0006 (de-admission tombstone),
-- renamed: v2's worlds.remote_worlds is the stage-4 catalogue.
--
-- Authority: none. A peer's row is a content claim, never an ownership or
-- permission fact: there is no owner, access, permissions, blocked_since,
-- deployer or auth-chain column, so the `owner` a peer prints on every entry
-- has nowhere to land (the poller's deserialiser has no field for it either).
-- No FK to worlds.worlds in either direction: an ACL row cannot attach to a
-- mirrored-only name (world_permissions FKs worlds(name), which the mirror
-- never writes), a peer's listing cannot constrain or cascade into the local
-- tables, and the same name held locally and claimed by a peer is two
-- independent rows. The room resolver, the permission core, /index and
-- /about never read this table, so a mirrored world grants nobody host
-- authority and never shadows a local publish.
--
-- Workload (WORKLOADS.md, "federation_worlds"): a poll replaces one peer's
-- rows in one transaction (DELETE ... WHERE peer_id = $1 AND hidden_since IS
-- NULL, then 500-row upserts); the public listing pages ORDER BY peer_id,
-- world_name, filtered to the admitted set; the boot sweep deletes the rows
-- of peers no longer admitted. Peers: a handful; rows per peer: the
-- production listing is 1.6k, capped at WORLDS_FED_MAX_WORLDS_PER_PEER (10k).
-- `observed_at` is our clock at the poll (the per-row staleness signal),
-- `last_deployed_at` the peer's text, parsed and never read by a branch.
-- `hidden_since` is the local operator veto: the poller's DELETE spares it
-- and its UPDATE arm never names it, so a peer cannot un-hide itself by
-- re-listing, and the sweep spares vetoed rows so a re-admission does not
-- republish a vetoed world. Every timestamp is UTC without time zone and the
-- current time is a bind parameter.
CREATE TABLE worlds.federation_worlds (
    peer_id text NOT NULL,
    world_name text NOT NULL,
    title text,
    description text,
    content_rating text,
    categories text[],
    thumbnail_hash text,
    deployed_scenes bigint NOT NULL DEFAULT 0,
    last_deployed_at timestamp without time zone,
    observed_at timestamp without time zone NOT NULL,
    hidden_since timestamp without time zone,
    CONSTRAINT federation_worlds_pkey PRIMARY KEY (peer_id, world_name),
    CONSTRAINT federation_worlds_peer_id_lowercase CHECK (peer_id = lower(peer_id) AND peer_id <> ''),
    CONSTRAINT federation_worlds_name_lowercase CHECK (world_name = lower(world_name) AND world_name <> ''),
    CONSTRAINT federation_worlds_deployed_scenes_nonneg CHECK (deployed_scenes >= 0)
);

-- One row per peer this node ever attempted, admitted or since removed. The
-- poller writes last_attempt_at on every poll and last_success_at only on a
-- complete one, so an unreachable peer reads as STALE (attempt > success,
-- last_error set), never as EMPTY; a peer that never answered has no
-- last_success_at at all (hasEverSucceeded: false on the wire). The boot
-- sweep records a de-admission here, bounded, when it deletes the peer's
-- unbounded per-world rows: deadmitted_at (cleared on re-admission) and the
-- cumulative deadmitted_worlds_deleted. Read whole (ORDER BY peer_id) by the
-- two public routes; the old partial index on deadmitted_at answered no
-- statement and is not created.
CREATE TABLE worlds.federation_peer_status (
    peer_id text NOT NULL,
    last_attempt_at timestamp without time zone,
    last_success_at timestamp without time zone,
    last_error text,
    worlds_observed bigint NOT NULL DEFAULT 0,
    entries_skipped bigint NOT NULL DEFAULT 0,
    truncated boolean NOT NULL DEFAULT false,
    deadmitted_at timestamp without time zone,
    deadmitted_worlds_deleted bigint NOT NULL DEFAULT 0,
    CONSTRAINT federation_peer_status_pkey PRIMARY KEY (peer_id),
    CONSTRAINT federation_peer_status_id_lowercase CHECK (peer_id = lower(peer_id) AND peer_id <> ''),
    CONSTRAINT federation_peer_status_counts_nonneg
        CHECK (worlds_observed >= 0 AND entries_skipped >= 0 AND deadmitted_worlds_deleted >= 0)
);
-- ==== end section: worlds-federation ====

-- ════ schema/30_marketplace.sql ════

-- Catalyst v2: marketplace domain: the off-chain trades mirror and its pass state.
--
-- Derived from catalyrst/crates/catalyrst-market/migrations/0002_trades_schema.sql
-- (trades, trade_assets and the three typed asset sub-tables) as the trades
-- poller (src/marketplace/trades.rs) fills them. Not carried over: the
-- mv_trades materialized view and squid_trades.* (0004, 0005, 0011, 0012),
-- which join squid_marketplace.nft/item for owner and supply checks and have
-- no data source in v2; the wallet-signed federation log (0001, the
-- market_*_local tables), favorites, usage grants, coupons and item neighbours,
-- which arrive with marketplace writes in stage 5.
--
-- Mirrored rows stay distinguishable from local authority: `mirrored_from`
-- names the upstream a trade was copied from and is NULL for a trade signed
-- into this node; the mirror inserts with ON CONFLICT DO NOTHING, so it never
-- overwrites a local row that carries the same signature. `trades_mirror` is
-- the one-row pass marker: a restart resumes at `next_skip` and a pass whose
-- `completed_at` is newer than its `started_at` walked every upstream page.
-- Every timestamp column is `timestamp without time zone` holding UTC.

CREATE SCHEMA marketplace;

CREATE TYPE marketplace.trade_type AS ENUM ('bid', 'public_nft_order', 'public_item_order');
CREATE TYPE marketplace.asset_direction_type AS ENUM ('sent', 'received');

CREATE TABLE marketplace.trades (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    network text NOT NULL,
    chain_id integer NOT NULL,
    signature text NOT NULL UNIQUE,
    hashed_signature text NOT NULL UNIQUE,
    checks jsonb NOT NULL,
    signer varchar(42) NOT NULL,
    type marketplace.trade_type NOT NULL,
    expires_at timestamp(3) without time zone NOT NULL,
    effective_since timestamp(3) without time zone NOT NULL,
    contract text NOT NULL DEFAULT '',
    created_at timestamp(3) without time zone NOT NULL DEFAULT timezone('UTC', now())::timestamp(3),
    mirrored_from text,
    mirrored_at timestamp without time zone,
    CONSTRAINT trades_signer_lowercase CHECK (signer = lower(signer))
);

-- No index on signer: the old component's signer listing (trades_for_address)
-- was never routed and /v1/trades takes no ?signer=; idx_trades_signer_created
-- was dropped 2026-09-25 (zero scans, no statement).
-- /v1/trades: ORDER BY created_at DESC LIMIT OFFSET.
CREATE INDEX idx_trades_created_at ON marketplace.trades (created_at DESC);
-- No index on type: the old `type =` filters ran on the squid-backed
-- unified_trades view that v2 does not carry (metrics/schema-review.md).

CREATE TABLE marketplace.trade_assets (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    trade_id uuid NOT NULL REFERENCES marketplace.trades(id) ON DELETE CASCADE,
    direction marketplace.asset_direction_type NOT NULL,
    asset_type smallint NOT NULL,
    contract_address varchar(42) NOT NULL,
    beneficiary varchar(42),
    extra text,
    created_at timestamp(3) without time zone NOT NULL DEFAULT timezone('UTC', now())::timestamp(3),
    CONSTRAINT trade_assets_asset_type_known CHECK (asset_type IN (1, 2, 3, 4))
);
-- 1 and 2 (ERC20, USD-pegged MANA) carry a trade_assets_erc20 row, 3 (ERC721)
-- an erc721 row, 4 (collection item) an item row: exactly one sub-row per
-- asset, held by insert_trade's transaction (a cross-table invariant, not a CHECK).

CREATE INDEX idx_trade_assets_trade ON marketplace.trade_assets (trade_id);

CREATE TABLE marketplace.trade_assets_erc721 (
    asset_id uuid NOT NULL UNIQUE REFERENCES marketplace.trade_assets(id) ON DELETE CASCADE,
    token_id text NOT NULL
);

CREATE TABLE marketplace.trade_assets_erc20 (
    asset_id uuid NOT NULL UNIQUE REFERENCES marketplace.trade_assets(id) ON DELETE CASCADE,
    amount numeric(78,0) NOT NULL CHECK (amount >= 0)
);

CREATE TABLE marketplace.trade_assets_item (
    asset_id uuid NOT NULL UNIQUE REFERENCES marketplace.trade_assets(id) ON DELETE CASCADE,
    item_id text NOT NULL
);

CREATE TABLE marketplace.trades_mirror (
    id integer PRIMARY KEY CHECK (id = 1),
    upstream_url text NOT NULL,
    pass bigint NOT NULL DEFAULT 0,
    next_skip bigint NOT NULL DEFAULT 0,
    started_at timestamp without time zone,
    completed_at timestamp without time zone,
    pages_fetched bigint NOT NULL DEFAULT 0,
    rows_fetched bigint NOT NULL DEFAULT 0,
    rows_inserted bigint NOT NULL DEFAULT 0,
    rows_failed bigint NOT NULL DEFAULT 0,
    bytes_downloaded bigint NOT NULL DEFAULT 0,
    list_downloads bigint NOT NULL DEFAULT 0,
    last_error text,
    updated_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now())
);

-- ── Stage 5: the chain-derived mirrors ──────────────────────────────────────
--
-- v2 has no squid database. The catalog, items, contracts and collections
-- routes read these two mirrors of the L1/L2 collections subgraphs, filled by
-- `collections-mirror` and `items-mirror` (jobs with a cursor and a pass
-- marker per network in `mirror_state`); every other route reads a subgraph
-- through a bounded, cached reader or is deferred (schema/WORKLOADS.md, "the
-- market half of the data plane"). Identity is (network, contract, item):
-- the wire's `contract-item` id is rendered from the key. Subgraph timestamps
-- stay the subgraph's unix seconds (bigint): the wire emits numbers and no
-- query filters them as time; `mirrored_at` (UTC) is a row's freshness.

CREATE TABLE marketplace.collections (
    network text NOT NULL,
    contract_address varchar(42) NOT NULL,
    chain_id integer NOT NULL,
    name text NOT NULL,
    symbol text,
    creator varchar(42) NOT NULL,
    owner varchar(42) NOT NULL,
    urn text NOT NULL,
    is_completed boolean NOT NULL,
    is_approved boolean NOT NULL,
    is_editable boolean NOT NULL,
    items_count integer NOT NULL DEFAULT 0,
    minters text[] NOT NULL DEFAULT '{}',
    managers text[] NOT NULL DEFAULT '{}',
    search_is_store_minter boolean NOT NULL DEFAULT false,
    search_text text,
    created_at bigint NOT NULL,
    updated_at bigint NOT NULL,
    reviewed_at bigint NOT NULL,
    first_listed_at bigint,
    mirrored_at timestamp without time zone NOT NULL,
    PRIMARY KEY (network, contract_address),
    CONSTRAINT collections_network_known CHECK (network IN ('ethereum', 'matic')),
    CONSTRAINT collections_contract_lowercase CHECK (contract_address = lower(contract_address)),
    CONSTRAINT collections_items_count_nonneg CHECK (items_count >= 0)
);

-- /v1/contracts: WHERE is_approved ORDER BY name COLLATE "C" LIMIT OFFSET.
CREATE INDEX collections_approved_name_idx ON marketplace.collections (name COLLATE "C") WHERE is_approved;
-- /v1/collections?creator= and /v3/catalog/creators.
CREATE INDEX collections_creator_idx ON marketplace.collections (creator);

CREATE TABLE marketplace.items (
    network text NOT NULL,
    contract_address varchar(42) NOT NULL,
    item_id numeric(78,0) NOT NULL,
    chain_id integer NOT NULL,
    item_type text NOT NULL,
    urn text NOT NULL,
    creator varchar(42) NOT NULL,
    beneficiary varchar(42),
    rarity text NOT NULL,
    total_supply numeric(78,0) NOT NULL,
    max_supply numeric(78,0) NOT NULL,
    available numeric(78,0) NOT NULL,
    price numeric(78,0) NOT NULL,
    image text,
    name text NOT NULL,
    description text,
    category text NOT NULL,
    body_shapes text[] NOT NULL DEFAULT '{}',
    is_smart boolean NOT NULL DEFAULT false,
    emote_loop boolean,
    emote_has_sound boolean,
    emote_has_geometry boolean,
    emote_outcome_type text,
    search_is_store_minter boolean NOT NULL DEFAULT false,
    search_is_collection_approved boolean NOT NULL DEFAULT false,
    created_at bigint NOT NULL,
    updated_at bigint NOT NULL,
    reviewed_at bigint NOT NULL,
    sold_at bigint NOT NULL DEFAULT 0,
    first_listed_at bigint,
    min_price numeric(78,0),
    max_listing_price numeric(78,0),
    min_listing_price numeric(78,0),
    listings integer,
    owners integer,
    mirrored_at timestamp without time zone NOT NULL,
    PRIMARY KEY (network, contract_address, item_id),
    CONSTRAINT items_urn_key UNIQUE (urn),
    CONSTRAINT items_network_known CHECK (network IN ('ethereum', 'matic')),
    CONSTRAINT items_contract_lowercase CHECK (contract_address = lower(contract_address)),
    CONSTRAINT items_type_known CHECK (item_type IN ('wearable_v1', 'wearable_v2', 'smart_wearable_v1', 'emote_v1')),
    CONSTRAINT items_available_nonneg CHECK (available >= 0),
    CONSTRAINT items_price_nonneg CHECK (price >= 0)
);

-- /v1/items?creator= and the creators rail.
CREATE INDEX items_creator_idx ON marketplace.items (creator);
-- /v3/catalog/items, the default sort: WHERE search_is_collection_approved
-- ORDER BY created_at DESC LIMIT 24 (an index scan that stops at the LIMIT
-- since the page dropped its COUNT(*) OVER(); METRICS.md Q9) and its total
-- (an index-only count). The /v1,/v2 catalog sorts are on first_listed_at,
-- sold_at and the price CASEs and read the heap until a measured page says
-- otherwise.
CREATE INDEX items_approved_created_idx ON marketplace.items (created_at DESC) WHERE search_is_collection_approved;

-- One row per (mirror, network): the in-flight keyset (persisted per page so
-- a kill -9 resumes at that page) and the last complete pass, which a route
-- over the mirror requires before answering (NULL = the unavailable envelope).
CREATE TABLE marketplace.mirror_state (
    mirror text NOT NULL,
    network text NOT NULL,
    source_url text NOT NULL,
    pass_started_at timestamp without time zone,
    cursor_updated_at bigint,
    cursor_id text,
    pages_fetched bigint NOT NULL DEFAULT 0,
    rows_upserted bigint NOT NULL DEFAULT 0,
    last_complete_pass_at timestamp without time zone,
    last_complete_pass_rows bigint,
    last_error text,
    updated_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    PRIMARY KEY (mirror, network),
    CONSTRAINT mirror_state_known_mirror CHECK (mirror IN ('collections', 'items')),
    CONSTRAINT mirror_state_network_known CHECK (network IN ('ethereum', 'matic'))
);

-- ── Stage 5: local authority (behind DATA_WRITES_ENABLED) ───────────────────
--
-- Favorites (the old `favorites` schema of the marketplace favorites
-- service): lists, picks and per-list grants. `item_id` is the wire's
-- `contract-itemId`; no FK to items (a pick may precede the mirror row).

CREATE TABLE marketplace.lists (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    name text NOT NULL,
    description text,
    user_address varchar(42) NOT NULL,
    is_private boolean NOT NULL DEFAULT false,
    permission text,
    created_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    updated_at timestamp without time zone,
    CONSTRAINT lists_user_lowercase CHECK (user_address = lower(user_address)),
    CONSTRAINT lists_permission_known CHECK (permission IS NULL OR permission IN ('edit', 'view'))
);

-- /v1/lists for a wallet.
CREATE INDEX lists_user_idx ON marketplace.lists (user_address);

CREATE TABLE marketplace.picks (
    item_id text NOT NULL,
    user_address varchar(42) NOT NULL,
    list_id uuid NOT NULL REFERENCES marketplace.lists(id) ON DELETE CASCADE,
    created_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    PRIMARY KEY (item_id, user_address, list_id),
    CONSTRAINT picks_item_lowercase CHECK (item_id = lower(item_id)),
    CONSTRAINT picks_user_lowercase CHECK (user_address = lower(user_address))
);

-- The per-list page (WHERE list_id ORDER BY created_at DESC).
CREATE INDEX picks_list_id_idx ON marketplace.picks (list_id, created_at DESC);
-- The catalog's pick counts: GROUP BY item_id WHERE item_id = ANY($1).
CREATE INDEX picks_item_idx ON marketplace.picks (item_id);

CREATE TABLE marketplace.list_acl (
    list_id uuid NOT NULL REFERENCES marketplace.lists(id) ON DELETE CASCADE,
    permission text NOT NULL,
    grantee text NOT NULL,
    PRIMARY KEY (list_id, permission, grantee),
    CONSTRAINT list_acl_permission_known CHECK (permission IN ('edit', 'view'))
);

-- The shop's hard-coded shared default list (upstream 1678303321034 +
-- 1687172729802): owned by the zero address, every user picks into it and
-- reads back only their own picks.
INSERT INTO marketplace.lists (id, name, description, user_address, is_private)
VALUES ('70ab6873-4a03-4eb2-b331-4b8be0e0b8af', 'Wishlist', 'Find all your wished items here',
        '0x0000000000000000000000000000000000000000', false);

-- The one cancellation record of a v3 trade this node knows: the signer's
-- own signed cancellation or an operator's force-cancel. The bids/orders
-- status rule is `open` unless expired or present here; on-chain executions
-- and signature-index invalidation wait for the trade_events indexer
-- (schema/WORKLOADS.md, "NOT created").
CREATE TABLE marketplace.trade_cancellations (
    target_hashed_signature text PRIMARY KEY,
    kind text NOT NULL,
    signer varchar(42) NOT NULL,
    signature text,
    reason text NOT NULL DEFAULT '',
    cancelled_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    CONSTRAINT trade_cancellations_kind_known CHECK (kind IN ('signer', 'operator')),
    CONSTRAINT trade_cancellations_signer_lowercase CHECK (signer = lower(signer)),
    CONSTRAINT trade_cancellations_signed_when_signer CHECK (kind <> 'signer' OR signature IS NOT NULL)
);

-- Operator moderation (the old 0003_admin_moderation.sql, keyed on
-- marketplace.trades.hashed_signature). No FK to trades: a flag outlives a
-- swept mirrored trade as history.
CREATE TABLE marketplace.moderation_flags (
    target_hash text PRIMARY KEY,
    target_kind text NOT NULL,
    severity text NOT NULL DEFAULT 'review',
    reason text NOT NULL DEFAULT '',
    flagged_by text NOT NULL,
    flagged_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    CONSTRAINT moderation_flags_kind_known CHECK (target_kind IN ('bid', 'order', 'trade')),
    CONSTRAINT moderation_flags_severity_known CHECK (severity IN ('review', 'hide', 'block'))
);

-- /admin/market/flags?kind= newest first.
CREATE INDEX moderation_flags_kind_idx ON marketplace.moderation_flags (target_kind, flagged_at DESC);

CREATE TABLE marketplace.disputes (
    trade_hash text PRIMARY KEY,
    status text NOT NULL DEFAULT 'open',
    reason text NOT NULL DEFAULT '',
    resolution text NOT NULL DEFAULT '',
    opened_by text NOT NULL,
    opened_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    resolved_by text,
    resolved_at timestamp without time zone,
    CONSTRAINT disputes_status_known CHECK (status IN ('open', 'resolved', 'rejected')),
    CONSTRAINT disputes_resolved_iff_closed CHECK ((status = 'open') = (resolved_at IS NULL))
);

-- The open-disputes list: WHERE status = 'open' ORDER BY opened_at DESC.
CREATE INDEX disputes_status_idx ON marketplace.disputes (status, opened_at DESC);

CREATE TABLE marketplace.admin_audit (
    id bigint PRIMARY KEY GENERATED ALWAYS AS IDENTITY,
    actor text NOT NULL,
    action text NOT NULL,
    target_kind text NOT NULL,
    target_hash text NOT NULL,
    detail jsonb NOT NULL DEFAULT '{}'::jsonb,
    created_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now())
);

-- /admin/market/audit?target=: the target's history, newest first.
CREATE INDEX admin_audit_target_idx ON marketplace.admin_audit (target_kind, target_hash, id DESC);

-- ── Stage 5 remainder: the marketplace chain indexes and coupons ───────────
--
-- Two fresh eth_getLogs indexers in the land_authz shape (schema/WORKLOADS.md,
-- "the marketplace chain indexes"): `nft` (the ERC-721 Transfer logs of every
-- collection the collections mirror knows, the DCLRegistrar's names and the
-- legacy Marketplace orders; LAND and estates stay land_authz's facts) and
-- `trade_events` (the off-chain marketplace contracts' Traded,
-- SignatureCancelled and signature-index logs). Each keeps an append-only log
-- keyed by the chain's position, a durable per-network cursor advanced in the
-- same transaction as the log rows of its window, and a deterministic fold
-- (`fold_through`: the block the last fold covered, whatever its publish;
-- the next fold starts there). Readers trust an index only once its cursor
-- is published (`folded_block`: folded within the head lag, recently):
-- before that the routes answer the unavailable envelope. Block times are
-- the chain's unix seconds.

CREATE TABLE marketplace.chain_cursor (
    indexer text NOT NULL,
    network text NOT NULL,
    start_block bigint NOT NULL,
    last_block bigint NOT NULL,
    last_block_hash text,
    last_block_time bigint,
    fold_through bigint,
    folded_block bigint,
    folded_at timestamp without time zone,
    full_history boolean NOT NULL,
    reorg_rewinds integer NOT NULL DEFAULT 0,
    updated_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    PRIMARY KEY (indexer, network),
    CONSTRAINT chain_cursor_indexer_known CHECK (indexer IN ('nft', 'trade_events')),
    CONSTRAINT chain_cursor_network_known CHECK (network IN ('ethereum', 'matic')),
    CONSTRAINT chain_cursor_fold_within_indexed CHECK (fold_through IS NULL OR fold_through <= last_block),
    CONSTRAINT chain_cursor_folded_within_indexed CHECK (folded_block IS NULL OR folded_block <= last_block),
    CONSTRAINT chain_cursor_hash_shape CHECK (last_block_hash IS NULL OR last_block_hash ~ '^0x[0-9a-f]{64}$')
);

-- The collections the nft walk has taken in, and from which block. A
-- collection the mirror reports after the walk passed its creation gets a
-- backfill range [backfill_from_block, walked_from_block - 1], walked by its
-- own lane (progress in backfilled_to_block) before the network publishes;
-- its completion sets refold_owed, which the next fold clears after
-- refolding every token of the contract (the backfilled rows sit below the
-- fold's block marker).
CREATE TABLE marketplace.nft_contract (
    network text NOT NULL,
    contract_address varchar(42) NOT NULL,
    walked_from_block bigint NOT NULL,
    backfill_from_block bigint,
    backfilled_to_block bigint,
    refold_owed boolean NOT NULL DEFAULT false,
    added_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    PRIMARY KEY (network, contract_address),
    CONSTRAINT nft_contract_network_known CHECK (network IN ('ethereum', 'matic')),
    CONSTRAINT nft_contract_lowercase CHECK (contract_address = lower(contract_address)),
    CONSTRAINT nft_contract_backfill_before_walk CHECK (backfill_from_block IS NULL OR backfill_from_block < walked_from_block),
    CONSTRAINT nft_contract_backfill_progress CHECK (backfilled_to_block IS NULL OR (backfill_from_block IS NOT NULL AND backfilled_to_block < walked_from_block))
);

-- The nft log. `contract_address`/`token_id` name the asset (the emitter for
-- a transfer, a v1 issue, a name and a LAND/Estate Update; the order's
-- nftAddress for the three legacy order kinds, whose emitter is the
-- marketplace); `amount` is the order price or the v1 issued id; `detail`
-- the legacy order id, the v1 wearable id, the registered subdomain or the
-- Update's data string.
CREATE TABLE marketplace.nft_event (
    network text NOT NULL,
    block_number bigint NOT NULL,
    log_index integer NOT NULL,
    block_time bigint NOT NULL,
    kind text NOT NULL,
    emitter varchar(42) NOT NULL,
    contract_address varchar(42) NOT NULL,
    token_id numeric(78,0) NOT NULL,
    from_address varchar(42),
    to_address varchar(42),
    amount numeric(78,0),
    expires_at bigint,
    detail text,
    PRIMARY KEY (network, block_number, log_index),
    CONSTRAINT nft_event_network_known CHECK (network IN ('ethereum', 'matic')),
    CONSTRAINT nft_event_kind_known CHECK (kind IN (
        'transfer', 'issue_v1', 'name_registered', 'order_created', 'order_successful', 'order_cancelled', 'land_data')),
    CONSTRAINT nft_event_contract_lowercase CHECK (contract_address = lower(contract_address) AND emitter = lower(emitter)),
    CONSTRAINT nft_event_token_nonneg CHECK (token_id >= 0)
);

-- The fold's per-token legs (DISTINCT ON (contract_address, token_id) ORDER
-- BY block_number, log_index) and a refold's "events of these tokens".
CREATE INDEX nft_event_token_idx ON marketplace.nft_event (network, contract_address, token_id, block_number, log_index);

-- The ownership fold of the collections' and names' transfers: owner is the
-- latest Transfer's `to` (NULL once burned), the mint the first; item_id and
-- issued_id decode a v2 token id (item << 216 | issued), wearable_id and
-- issued_id come from a v1 Issue log, ens_subdomain from NameRegistered,
-- sold_at from the latest legacy OrderSuccessful.
CREATE TABLE marketplace.nft (
    network text NOT NULL,
    contract_address varchar(42) NOT NULL,
    token_id numeric(78,0) NOT NULL,
    owner varchar(42),
    item_id numeric(78,0),
    issued_id numeric(78,0),
    wearable_id text,
    ens_subdomain text,
    minted_block bigint NOT NULL,
    minted_at bigint NOT NULL,
    transferred_block bigint NOT NULL,
    transferred_log integer NOT NULL,
    transferred_at bigint NOT NULL,
    sold_at bigint,
    PRIMARY KEY (network, contract_address, token_id),
    CONSTRAINT nft_network_known CHECK (network IN ('ethereum', 'matic'))
);

-- /v1/nfts?owner=, the wallet leg of /v3/catalog/suggested and the
-- co-ownership scan: WHERE owner = $1.
CREATE INDEX nft_owner_idx ON marketplace.nft (owner) WHERE owner IS NOT NULL;
-- The copies of an item: WHERE (network, contract_address, item_id) = ...
-- (the index-served /v1/nfts over a matched item set, ?itemId=).
CREATE INDEX nft_item_idx ON marketplace.nft (network, contract_address, item_id) WHERE item_id IS NOT NULL;

-- The legacy Marketplace order per token, folded: the latest order event
-- decides (created = open, successful = sold, cancelled = cancelled); a
-- reader treats `open` as live only while unexpired and while the seller
-- still owns the token. expires_at is milliseconds (a seconds value from the
-- 2018 contract is scaled by the fold), created_at/updated_at unix seconds.
CREATE TABLE marketplace.nft_order (
    network text NOT NULL,
    contract_address varchar(42) NOT NULL,
    token_id numeric(78,0) NOT NULL,
    order_id text NOT NULL,
    marketplace_address varchar(42) NOT NULL,
    status text NOT NULL,
    seller varchar(42) NOT NULL,
    buyer varchar(42),
    price numeric(78,0) NOT NULL,
    expires_at bigint NOT NULL,
    created_at bigint NOT NULL,
    updated_at bigint NOT NULL,
    updated_block bigint NOT NULL,
    updated_log integer NOT NULL,
    PRIMARY KEY (network, contract_address, token_id),
    CONSTRAINT nft_order_status_known CHECK (status IN ('open', 'sold', 'cancelled')),
    CONSTRAINT nft_order_price_nonneg CHECK (price >= 0)
);

-- The open legacy orders of a contract set (the prices histogram, isOnSale,
-- the estate stats): WHERE status = 'open' AND expires_at > $now.
CREATE INDEX nft_order_open_idx ON marketplace.nft_order (network, contract_address, expires_at) WHERE status = 'open';

-- LAND and estate names and descriptions, folded from the registries'
-- Update(assetId, holder, operator, data) logs: the latest one decides; the
-- data string is the registries' CSV (`0,"name","description"`), anything
-- else leaves both NULL.
CREATE TABLE marketplace.land_data (
    token_address varchar(42) NOT NULL,
    token_id numeric(78,0) NOT NULL,
    name text,
    description text,
    updated_block bigint NOT NULL,
    updated_log integer NOT NULL,
    PRIMARY KEY (token_address, token_id)
);

-- The off-chain marketplace's log. `hashed_signature` is keccak256 of the
-- trade signature (marketplace.trades.hashed_signature), `caller` the
-- transaction's msg sender (for a cancellation, whose signature it cancels),
-- `new_index` the raised signer or contract signature index, `signer` and
-- `assets` the Traded event's trade (its sent/received arrays, read as a unit).
CREATE TABLE marketplace.trade_event (
    network text NOT NULL,
    block_number bigint NOT NULL,
    log_index integer NOT NULL,
    block_time bigint NOT NULL,
    marketplace_address varchar(42) NOT NULL,
    kind text NOT NULL,
    caller varchar(42) NOT NULL,
    hashed_signature text,
    new_index numeric(78,0),
    signer varchar(42),
    assets jsonb,
    tx_hash text NOT NULL,
    PRIMARY KEY (network, block_number, log_index),
    CONSTRAINT trade_event_network_known CHECK (network IN ('ethereum', 'matic')),
    CONSTRAINT trade_event_kind_known CHECK (kind IN ('traded', 'signature_cancelled', 'signer_index_increased', 'contract_index_increased')),
    CONSTRAINT trade_event_signature_kinds CHECK ((kind IN ('traded', 'signature_cancelled')) = (hashed_signature IS NOT NULL)),
    CONSTRAINT trade_event_index_kinds CHECK ((kind IN ('signer_index_increased', 'contract_index_increased')) = (new_index IS NOT NULL)),
    CONSTRAINT trade_event_lowercase CHECK (marketplace_address = lower(marketplace_address) AND caller = lower(caller))
);

-- A trade's executions and cancellations: WHERE hashed_signature = ANY($1).
CREATE INDEX trade_event_signature_idx ON marketplace.trade_event (hashed_signature, kind) WHERE hashed_signature IS NOT NULL;
-- The current signer / contract index of a marketplace: the latest row of
-- (network, marketplace_address, kind, caller) by block.
CREATE INDEX trade_event_index_idx ON marketplace.trade_event (network, marketplace_address, kind, caller, block_number DESC, log_index DESC) WHERE new_index IS NOT NULL;

-- The chain's verdict on each stored trade, folded from trade_event: `sold`
-- once its Traded executions reach checks.uses, `cancelled` when its signer
-- cancelled the signature (v1/v2 by keccak256 of the signature, v3 by the
-- EIP-712 digest, computed here from the row) or a signer or contract
-- signature index moved away from the one it was signed with, else `open`.
-- Expiry and local cancellations stay read-time rules. Derived: a rebuild
-- refolds every trade; a trade row deleted takes its verdict with it.
CREATE TABLE marketplace.trade_chain_state (
    trade_id uuid PRIMARY KEY REFERENCES marketplace.trades(id) ON DELETE CASCADE,
    network text NOT NULL,
    digest text,
    executions integer NOT NULL DEFAULT 0,
    status text NOT NULL,
    executed_at bigint,
    folded_block bigint NOT NULL,
    CONSTRAINT trade_chain_state_status_known CHECK (status IN ('open', 'sold', 'cancelled')),
    CONSTRAINT trade_chain_state_executions_nonneg CHECK (executions >= 0)
);

-- A v3 cancellation names the digest: WHERE digest = ANY($1) (the fold's touched set).
CREATE INDEX trade_chain_state_digest_idx ON marketplace.trade_chain_state (digest) WHERE digest IS NOT NULL;

-- Item co-ownership neighbours (/v3/catalog/suggested), derived from the
-- nft fold by the `item-neighbours` job: for every item held with others,
-- its top neighbours by a cosine over the wallets holding both, shrunk by
-- the co-owner count. Rebuilt whole in one transaction per run; the meta
-- row names the fold it was built from.
CREATE TABLE marketplace.item_neighbors (
    network text NOT NULL,
    contract_address varchar(42) NOT NULL,
    item_id numeric(78,0) NOT NULL,
    neighbor_network text NOT NULL,
    neighbor_contract_address varchar(42) NOT NULL,
    neighbor_item_id numeric(78,0) NOT NULL,
    sim real NOT NULL,
    support integer NOT NULL,
    rank smallint NOT NULL,
    PRIMARY KEY (network, contract_address, item_id, neighbor_network, neighbor_contract_address, neighbor_item_id),
    CONSTRAINT item_neighbors_sim_range CHECK (sim > 0 AND sim <= 1),
    CONSTRAINT item_neighbors_rank_positive CHECK (rank >= 1)
);

CREATE TABLE marketplace.item_neighbors_meta (
    id integer PRIMARY KEY CHECK (id = 1),
    built_at timestamp without time zone NOT NULL,
    algorithm text NOT NULL,
    wallets integer NOT NULL,
    items integer NOT NULL,
    rows_written integer NOT NULL,
    source_folded_block_matic bigint,
    source_folded_block_ethereum bigint
);

-- Creator-signed coupons (the old 0013_coupons.sql under marketplace, local
-- authority behind DATA_WRITES_ENABLED) and their on-chain state as the
-- `coupon-state-refresh` job last read it from the CouponManager.
CREATE TABLE marketplace.coupons (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    network text NOT NULL,
    chain_id integer NOT NULL,
    signer varchar(42) NOT NULL,
    signature text NOT NULL UNIQUE,
    hashed_signature text NOT NULL UNIQUE,
    state_key text NOT NULL UNIQUE,
    coupon_manager varchar(42) NOT NULL,
    coupon_address varchar(42) NOT NULL,
    checks jsonb NOT NULL,
    discount_type smallint NOT NULL,
    discount_ppm integer NOT NULL,
    root text NOT NULL,
    collections text[] NOT NULL,
    effective_since timestamp(3) without time zone NOT NULL,
    expires_at timestamp(3) without time zone NOT NULL,
    created_at timestamp(3) without time zone NOT NULL DEFAULT timezone('UTC', now())::timestamp(3),
    CONSTRAINT coupons_signer_lowercase CHECK (signer = lower(signer)),
    CONSTRAINT coupons_discount_ppm_within_bounds CHECK (discount_ppm >= 50000 AND discount_ppm <= 700000),
    CONSTRAINT coupons_effective_before_expiry CHECK (effective_since < expires_at)
);

-- GET /v1/coupons?signer=: WHERE signer = $1 ORDER BY created_at DESC LIMIT OFFSET.
CREATE INDEX coupons_signer_created_idx ON marketplace.coupons (signer, created_at DESC);
-- The refresh job's due set: WHERE expires_at > $now AND effective_since <= $now + 1 day.
CREATE INDEX coupons_expires_idx ON marketplace.coupons (expires_at);

CREATE TABLE marketplace.coupon_state (
    coupon_id uuid PRIMARY KEY REFERENCES marketplace.coupons(id) ON DELETE CASCADE,
    uses integer NOT NULL DEFAULT 0,
    cancelled boolean NOT NULL DEFAULT false,
    revoked boolean NOT NULL DEFAULT false,
    checked_at timestamp(3) without time zone NOT NULL DEFAULT timezone('UTC', now())::timestamp(3),
    CONSTRAINT coupon_state_uses_nonneg CHECK (uses >= 0)
);

-- ════ schema/31_economy.sql ════

-- Catalyst v2: economy domain: the meta-transaction relay log and quota, the
-- Landiler broker/escrow/NAME custody ledgers, the escrow lease overlay and
-- the MANA spot archive.
--
-- Derived from catalyrst/crates/catalyrst-economy/migrations/0001..0008 and
-- catalyrst/crates/catalyrst-price/migrations/0001..0003, folded into one
-- create each (the ALTERs inline), re-keyed with the DATABASE.md constraints
-- and every timestamp `timestamp without time zone` holding UTC (the old
-- tables mixed timestamp and timestamptz). No squid copy: the relay's
-- collection allowlist reads marketplace.collections (the mirror).
--
-- Every row here is local authority written behind DATA_WRITES_ENABLED; the
-- relayer key, the OZ credentials and the admin bearer live in *_FILE paths
-- or the unit's EnvironmentFile and never in this schema. Money is
-- numeric(78,0) wei, never a float. Rebuild/measurement notes:
-- schema/WORKLOADS.md, "economy".

CREATE SCHEMA economy;

-- The relay log and the quota reservation in one relation: a reservation is
-- `tx_hash NULL, session_id set` (inserted under a per-user advisory lock
-- before the broadcast), a relayed tx is `tx_hash set, session_id NULL`.
-- The quota count binds the request's clock, never now().
CREATE TABLE economy.transactions (
    id bigint PRIMARY KEY GENERATED ALWAYS AS IDENTITY,
    tx_hash text UNIQUE,
    session_id text UNIQUE,
    user_address varchar(42) NOT NULL,
    chain_id integer NOT NULL,
    contract_address varchar(42) NOT NULL,
    created_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    CONSTRAINT transactions_user_lowercase CHECK (user_address = lower(user_address)),
    CONSTRAINT transactions_contract_lowercase CHECK (contract_address = lower(contract_address)),
    CONSTRAINT transactions_reserved_or_relayed CHECK (tx_hash IS NOT NULL OR session_id IS NOT NULL)
);

-- The per-day quota count (WHERE user_address = $1 AND created_at >= $now - interval '1 day')
-- and GET /v1/transactions/{address}.
CREATE INDEX transactions_user_created_idx ON economy.transactions (user_address, created_at);

-- Relayer-paid NFT purchases routed to the Landiler escrow. A row is claimed
-- by idempotency key BEFORE the broadcast; the reconciler settles it from
-- receipts. `price_wei` is the execution-time MANA charge; the two usd
-- columns keep a USD-pegged trade's signed amount and the oracle rate used.
CREATE TABLE economy.broker_purchases (
    id bigint PRIMARY KEY GENERATED ALWAYS AS IDENTITY,
    idempotency_key text,
    tx_hash text,
    forward_tx_hash text,
    collection varchar(42) NOT NULL,
    item_id text,
    token_id text,
    minted_token_id text,
    buyer_address varchar(42),
    escrow_address varchar(42) NOT NULL,
    price_wei numeric(78,0) NOT NULL,
    usd_amount_wei numeric(78,0),
    mana_usd_rate_wei numeric(78,0),
    trade_hashed_signature text,
    chain_id bigint NOT NULL,
    mode text NOT NULL,
    status text NOT NULL DEFAULT 'sent',
    created_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    updated_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    CONSTRAINT broker_purchases_status_known
        CHECK (status IN ('pending', 'sent', 'reverted', 'bought', 'forwarding', 'confirmed', 'error')),
    CONSTRAINT broker_purchases_mode_known
        CHECK (mode IN ('mint', 'secondary', 'trade', 'name-mint', 'name-secondary')),
    CONSTRAINT broker_purchases_collection_lowercase CHECK (collection = lower(collection)),
    CONSTRAINT broker_purchases_price_nonneg CHECK (price_wei >= 0)
);

-- The claim: INSERT ... ON CONFLICT (idempotency_key) WHERE idempotency_key IS NOT NULL DO NOTHING.
CREATE UNIQUE INDEX broker_purchases_idempotency_key_uidx
    ON economy.broker_purchases (idempotency_key) WHERE idempotency_key IS NOT NULL;
-- A second logical purchase of a one-use signed trade is refused before any gas is spent.
CREATE UNIQUE INDEX broker_purchases_trade_sig_uidx
    ON economy.broker_purchases (trade_hashed_signature)
    WHERE trade_hashed_signature IS NOT NULL AND status NOT IN ('reverted', 'error');
-- The reconciler's bounded due set: WHERE status IN (...) ORDER BY id LIMIT 100.
CREATE INDEX broker_purchases_unsettled_idx
    ON economy.broker_purchases (id) WHERE status IN ('sent', 'bought', 'forwarding');
-- The admin console's per-escrow listing.
CREATE INDEX broker_purchases_escrow_address_idx ON economy.broker_purchases (escrow_address);

-- LandilerEscrow lifecycle calls (reclaim before unlock, release at/after).
CREATE TABLE economy.escrow_actions (
    id bigint PRIMARY KEY GENERATED ALWAYS AS IDENTITY,
    action text NOT NULL,
    collection varchar(42),
    token_id text,
    buyer varchar(42),
    escrow_address varchar(42),
    tx_hash text,
    idempotency_key text,
    status text NOT NULL DEFAULT 'sent',
    chain_id bigint,
    created_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    updated_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    CONSTRAINT escrow_actions_action_known CHECK (action IN ('reclaim', 'release')),
    CONSTRAINT escrow_actions_status_known CHECK (status IN ('pending', 'sent', 'confirmed', 'reverted', 'error'))
);

CREATE UNIQUE INDEX escrow_actions_idempotency_key_uidx
    ON economy.escrow_actions (idempotency_key) WHERE idempotency_key IS NOT NULL;
-- The reconciler's due set.
CREATE INDEX escrow_actions_unsettled_idx ON economy.escrow_actions (id) WHERE status = 'sent';
CREATE INDEX escrow_actions_escrow_address_idx ON economy.escrow_actions (escrow_address);

-- Relayer-custodied NAME transfers (the manage/release leg of the NAME lease).
CREATE TABLE economy.name_transfers (
    id bigint PRIMARY KEY GENERATED ALWAYS AS IDENTITY,
    idempotency_key text NOT NULL UNIQUE,
    registrar varchar(42) NOT NULL,
    token_id text NOT NULL,
    from_address varchar(42) NOT NULL,
    to_address varchar(42) NOT NULL,
    chain_id bigint NOT NULL,
    tx_hash text,
    status text NOT NULL DEFAULT 'pending',
    created_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    updated_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    CONSTRAINT name_transfers_status_known CHECK (status IN ('pending', 'sent', 'confirmed', 'reverted', 'error'))
);

-- The reconciler's due set.
CREATE INDEX name_transfers_unsettled_idx ON economy.name_transfers (id) WHERE status = 'sent';
-- The per-name history.
CREATE INDEX name_transfers_token_id_idx ON economy.name_transfers (registrar, token_id);

-- The escrow lease overlay: while the escrow holds a bought NFT, the buyer may
-- use it. Written by the broker on confirmation; read by the lambdas' owned
-- items routes (UNION ALL over the readers) and the profile filter.
CREATE TABLE economy.usage_grants (
    id bigint PRIMARY KEY GENERATED ALWAYS AS IDENTITY,
    grantee_address varchar(42) NOT NULL,
    urn text NOT NULL,
    token_id text,
    collection varchar(42),
    category text NOT NULL,
    escrow_ref text,
    granted_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    unlock_at timestamp without time zone NOT NULL,
    status text NOT NULL DEFAULT 'active',
    CONSTRAINT usage_grants_grantee_lowercase CHECK (grantee_address = lower(grantee_address)),
    CONSTRAINT usage_grants_category_known CHECK (category IN ('wearable', 'emote')),
    CONSTRAINT usage_grants_status_known CHECK (status IN ('active', 'revoked', 'released'))
);

-- The overlay: WHERE grantee_address = $1 AND status = 'active' AND unlock_at > $now.
CREATE INDEX usage_grants_grantee_idx ON economy.usage_grants (grantee_address) WHERE status = 'active';
-- The deploy validator's per-item probe.
CREATE INDEX usage_grants_urn_idx ON economy.usage_grants (urn);
-- The broker's idempotent grant write (token_id NULL for a primary mint).
CREATE UNIQUE INDEX usage_grants_natural_key
    ON economy.usage_grants (escrow_ref, urn, token_id) NULLS NOT DISTINCT
    WHERE escrow_ref IS NOT NULL;

-- The MANA spot archive: one row per CoinGecko poll (the mana-price-poll job).
CREATE TABLE economy.price_snapshots (
    id bigint PRIMARY KEY GENERATED ALWAYS AS IDENTITY,
    taken_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    source text NOT NULL DEFAULT 'coingecko',
    source_updated_at timestamp without time zone,
    mana_usd numeric(20,8) NOT NULL,
    mana_eth numeric(28,18),
    mana_btc numeric(28,18),
    mana_matic numeric(28,18),
    matic_usd numeric(20,8),
    mana_market_cap_usd numeric(20,2),
    mana_volume_24h_usd numeric(20,2),
    mana_price_change_24h_pct numeric(10,4),
    CONSTRAINT price_snapshots_mana_usd_positive CHECK (mana_usd > 0)
);

-- latest_coingecko: WHERE source = 'coingecko' ORDER BY taken_at DESC LIMIT 1.
CREATE INDEX price_snapshots_taken_at_idx ON economy.price_snapshots (taken_at DESC) WHERE source = 'coingecko';

-- The operator's manual spot (admin bearer), exact numeric.
CREATE TABLE economy.price_overrides (
    token_id text NOT NULL,
    vs_currency text NOT NULL,
    value numeric NOT NULL,
    note text,
    updated_by text,
    updated_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    PRIMARY KEY (token_id, vs_currency),
    CONSTRAINT price_overrides_key_lowercase CHECK (token_id = lower(token_id) AND vs_currency = lower(vs_currency))
);

CREATE TABLE economy.price_override_audit (
    id bigint PRIMARY KEY GENERATED ALWAYS AS IDENTITY,
    action text NOT NULL,
    token_id text NOT NULL,
    vs_currency text NOT NULL,
    value numeric,
    note text,
    admin text NOT NULL,
    detail jsonb NOT NULL DEFAULT '{}'::jsonb,
    created_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    CONSTRAINT price_override_audit_action_known CHECK (action IN ('override.set', 'override.clear'))
);

-- The console's per-key history, newest first.
CREATE INDEX price_override_audit_key_idx ON economy.price_override_audit (token_id, vs_currency, id DESC);

-- ════ schema/32_signatures.sql ════

-- Catalyst v2: signatures domain: the rentals listings (LAND/Estate rental
-- offers signed off-chain) of the old catalyrst-signatures service.
--
-- Derived from catalyrst/crates/catalyrst-signatures/migrations/0001_rentals.sql
-- (itself the consolidated decentraland/signatures-server migrations):
-- `metadata` becomes `rental_assets` keyed by (contract, token) with the
-- wire's `nft_id` as a UNIQUE, uuids come from gen_random_uuid (no uuid-ossp),
-- every timestamp is `timestamp without time zone` UTC. The asset's owner and
-- LAND columns come from the marketplace subgraph reader at create/refresh
-- time (the old squid.nft probe); `fetched_at` is their freshness. The
-- rentals-subgraph status sync had no job in the old crate and is deferred,
-- so no sync row exists. Writes are local authority behind DATA_WRITES_ENABLED.

CREATE SCHEMA signatures;

CREATE TYPE signatures.rental_status AS ENUM ('open', 'executed', 'cancelled', 'claimed');

CREATE TABLE signatures.rental_assets (
    contract_address varchar(42) NOT NULL,
    token_id numeric(78,0) NOT NULL,
    nft_id text NOT NULL UNIQUE,
    network text NOT NULL,
    chain_id integer NOT NULL,
    category text NOT NULL,
    search_text text NOT NULL,
    distance_to_plaza smallint,
    adjacent_to_road boolean,
    estate_size smallint,
    nft_created_at timestamp without time zone NOT NULL,
    nft_updated_at timestamp without time zone NOT NULL,
    fetched_at timestamp without time zone NOT NULL,
    PRIMARY KEY (contract_address, token_id),
    CONSTRAINT rental_assets_contract_lowercase CHECK (contract_address = lower(contract_address)),
    CONSTRAINT rental_assets_category_known CHECK (category IN ('parcel', 'estate'))
);

CREATE TABLE signatures.rentals (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    contract_address varchar(42) NOT NULL,
    token_id numeric(78,0) NOT NULL,
    network text NOT NULL,
    chain_id integer NOT NULL,
    expiration timestamp without time zone NOT NULL,
    nonces text[] NOT NULL,
    signature text NOT NULL UNIQUE,
    rental_contract_address varchar(42) NOT NULL,
    status signatures.rental_status NOT NULL DEFAULT 'open',
    target varchar(42) NOT NULL DEFAULT '0x0000000000000000000000000000000000000000',
    rented_days integer,
    period_chosen uuid,
    created_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    updated_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    started_at timestamp without time zone,
    FOREIGN KEY (contract_address, token_id) REFERENCES signatures.rental_assets(contract_address, token_id) ON DELETE CASCADE,
    CONSTRAINT rentals_nonces_three CHECK (array_length(nonces, 1) = 3)
);

-- One open listing per asset: the 409 of the create route.
CREATE UNIQUE INDEX rentals_open_asset_key ON signatures.rentals (contract_address, token_id) WHERE status = 'open';
-- The per-asset history and the DISTINCT ON (asset) of the listing.
CREATE INDEX rentals_asset_idx ON signatures.rentals (contract_address, token_id, created_at DESC);
-- The listing's default sort.
CREATE INDEX rentals_created_idx ON signatures.rentals (created_at DESC);

CREATE TABLE signatures.rental_listings (
    id uuid PRIMARY KEY REFERENCES signatures.rentals(id) ON DELETE CASCADE,
    lessor varchar(42) NOT NULL,
    tenant varchar(42),
    CONSTRAINT rental_listings_lessor_lowercase CHECK (lessor = lower(lessor)),
    CONSTRAINT rental_listings_tenant_lowercase CHECK (tenant IS NULL OR tenant = lower(tenant))
);

-- ?lessor= and ?tenant=.
CREATE INDEX rental_listings_lessor_idx ON signatures.rental_listings (lessor);
CREATE INDEX rental_listings_tenant_idx ON signatures.rental_listings (tenant) WHERE tenant IS NOT NULL;

CREATE TABLE signatures.rental_periods (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    rental_id uuid NOT NULL REFERENCES signatures.rental_listings(id) ON DELETE CASCADE,
    min_days integer NOT NULL,
    max_days integer NOT NULL,
    price_per_day numeric(78,0) NOT NULL,
    CONSTRAINT rental_periods_min_nonneg CHECK (min_days >= 0),
    CONSTRAINT rental_periods_max_ge_min CHECK (max_days >= min_days),
    CONSTRAINT rental_periods_price_nonneg CHECK (price_per_day >= 0)
);

-- The periods aggregate of every listing row and the DISTINCT rental_id sub-select of the day filters.
CREATE INDEX rental_periods_rental_idx ON signatures.rental_periods (rental_id);

-- ════ schema/33_land_authz.sql ════

-- Catalyst v2: land_authz domain: the one fresh chain index. Who may deploy
-- to a parcel, folded from the LAND and Estate registries' logs.
--
-- Derived from catalyrst/crates/catalyrst-land-authz/migrations/0001..0002 as
-- the indexer (events.rs, indexer.rs) and the resolver (resolve.rs) use them,
-- with the squid joins removed: the old resolver read squid_marketplace.parcel
-- and .estate for coordinates, ownership and estate membership. Here the
-- coordinates are the token id (token_x/token_y), the owner is the latest
-- Transfer (its `account` is `to`) and estate membership is the Estate
-- registry's AddLand/RemoveLand logs, two topics the indexer adds. The
-- schema is created here (the old bootstrap script created it out of band);
-- every timestamp is `timestamp without time zone` UTC, block times are the
-- chain's unix seconds.
--
-- Readers use this index only after index_cursor.folded_block is set (a
-- completed full replay); until then the marketplace subgraph reader answers.

CREATE SCHEMA land_authz;

-- The log, append-only, keyed by the chain's own position.
CREATE TABLE land_authz.authz_event (
    block_number bigint NOT NULL,
    log_index integer NOT NULL,
    block_time bigint NOT NULL,
    token_address varchar(42) NOT NULL,
    kind text NOT NULL,
    token_id numeric(78,0),
    estate_id numeric(78,0),
    account varchar(42),
    operator varchar(42),
    approved boolean,
    PRIMARY KEY (block_number, log_index),
    CONSTRAINT authz_event_kind_known CHECK (kind IN (
        'update_operator', 'approval', 'transfer', 'update_manager', 'approved_for_all', 'add_land', 'remove_land')),
    CONSTRAINT authz_event_token_address_lowercase CHECK (token_address = lower(token_address)),
    CONSTRAINT authz_event_estate_kinds_carry_estate CHECK ((kind IN ('add_land', 'remove_land')) = (estate_id IS NOT NULL))
);

-- The per-token legs of the fold: DISTINCT ON (token_address, token_id) ORDER BY block_number DESC, log_index DESC.
CREATE INDEX authz_event_token_idx ON land_authz.authz_event (token_address, token_id, block_number, log_index)
    WHERE token_id IS NOT NULL;
-- The account legs: DISTINCT ON (token_address, account, operator, kind).
CREATE INDEX authz_event_account_idx ON land_authz.authz_event (token_address, account, operator, kind, block_number, log_index)
    WHERE account IS NOT NULL AND operator IS NOT NULL;

-- LAND packs its signed coordinates into the two 128-bit halves of the token
-- id; the fold and the reverse lookups read coordinates the way the contract
-- wrote them.
CREATE FUNCTION land_authz.decode_coord(half numeric) RETURNS integer
LANGUAGE sql IMMUTABLE AS $$
  SELECT (CASE WHEN half >= 170141183460469231731687303715884105728::numeric
               THEN half - 340282366920938463463374607431768211456::numeric
               ELSE half END)::integer
$$;

CREATE FUNCTION land_authz.token_x(token_id numeric) RETURNS integer
LANGUAGE sql IMMUTABLE AS $$
  SELECT land_authz.decode_coord(div(token_id, 340282366920938463463374607431768211456::numeric))
$$;

CREATE FUNCTION land_authz.token_y(token_id numeric) RETURNS integer
LANGUAGE sql IMMUTABLE AS $$
  SELECT land_authz.decode_coord(mod(token_id, 340282366920938463463374607431768211456::numeric))
$$;

-- Per-token rights, derived: `operator` from the latest Approval,
-- `update_operator` from the latest UpdateOperator, `owner` from the latest
-- Transfer (which also clears both operators); the zero address is NULL.
-- x/y are NULL for estate tokens.
CREATE TABLE land_authz.token_right (
    token_address varchar(42) NOT NULL,
    token_id numeric(78,0) NOT NULL,
    x integer,
    y integer,
    owner varchar(42),
    operator varchar(42),
    update_operator varchar(42),
    updated_block bigint NOT NULL,
    updated_log integer NOT NULL,
    PRIMARY KEY (token_address, token_id)
);

-- The parcel lookup by coordinates (parcel_subject, rights_batch over unnest).
CREATE INDEX token_right_xy_idx ON land_authz.token_right (x, y) WHERE x IS NOT NULL;
-- The reverse lookups of lands-permissions.
CREATE INDEX token_right_update_operator_idx ON land_authz.token_right (update_operator) WHERE update_operator IS NOT NULL;
CREATE INDEX token_right_operator_idx ON land_authz.token_right (operator) WHERE operator IS NOT NULL;
-- The parcels/estates an address owns (the worlds policy LAND leg).
CREATE INDEX token_right_owner_idx ON land_authz.token_right (token_address, owner) WHERE owner IS NOT NULL;

-- Estate membership, derived: the latest AddLand puts a LAND in an estate, a
-- later RemoveLand deletes the row.
CREATE TABLE land_authz.estate_land (
    land_token_id numeric(78,0) PRIMARY KEY,
    estate_token_id numeric(78,0) NOT NULL,
    updated_block bigint NOT NULL,
    updated_log integer NOT NULL
);

-- The parcels of an estate (parcels_updatable_by's estate branch, estate size).
CREATE INDEX estate_land_estate_idx ON land_authz.estate_land (estate_token_id);

-- Account-wide rights per registry: UpdateManager and ApprovalForAll
-- authorise an operator over everything an account holds in one contract.
CREATE TABLE land_authz.account_right (
    token_address varchar(42) NOT NULL,
    account varchar(42) NOT NULL,
    operator varchar(42) NOT NULL,
    kind text NOT NULL,
    is_approved boolean NOT NULL,
    updated_block bigint NOT NULL,
    updated_log integer NOT NULL,
    PRIMARY KEY (token_address, account, operator, kind),
    CONSTRAINT account_right_kind_known CHECK (kind IN ('update_manager', 'approved_for_all'))
);

-- The grant legs (account_grants) and their reverse.
CREATE INDEX account_right_account_idx ON land_authz.account_right (account, token_address, kind) WHERE is_approved;
CREATE INDEX account_right_operator_idx ON land_authz.account_right (operator, kind) WHERE is_approved;

-- Durable progress per network: `last_block` advances per inserted chunk,
-- `folded_block` is the publish marker (set only after the three folds
-- committed on a row whose full replay ran); a repair scan rewinds it.
CREATE TABLE land_authz.index_cursor (
    network text PRIMARY KEY,
    land_registry varchar(42) NOT NULL,
    estate_registry varchar(42) NOT NULL,
    last_block bigint NOT NULL,
    folded_block bigint,
    folded_at timestamp without time zone,
    full_replay_started boolean NOT NULL DEFAULT false,
    updated_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    CONSTRAINT index_cursor_network_known CHECK (network IN ('ethereum-mainnet', 'ethereum-sepolia')),
    CONSTRAINT index_cursor_folded_within_indexed CHECK (folded_block IS NULL OR folded_block <= last_block)
);

-- ════ schema/34_presence.sql ════

-- Catalyst v2: presence domain: the user-count history archive (one snapshot
-- per collector pass: archipelago peers, islands and hot scenes, per-scene
-- occupancy, per-world membership, a per-day rollup). Distinct from live
-- presence (comms' voice presence, the worlds presence map), which follows
-- the transport and is never persisted.
--
-- Derived from catalyrst/crates/catalyrst-presence/migrations/0001_presence.sql
-- with every timestamp `timestamp without time zone` UTC, bigint identities,
-- the address arrays kept as jsonb (read as a unit, never filtered by member)
-- with their count checked, and the five indexes no statement read dropped
-- (schema/WORKLOADS.md, "presence"). The collector writes a pass in one
-- transaction behind DATA_WRITES_ENABLED.

CREATE SCHEMA presence;

CREATE TABLE presence.snapshots (
    id bigint PRIMARY KEY GENERATED ALWAYS AS IDENTITY,
    taken_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    peers_count integer NOT NULL DEFAULT 0,
    islands_count integer NOT NULL DEFAULT 0,
    hot_scenes_count integer NOT NULL DEFAULT 0,
    scenes_polled integer NOT NULL DEFAULT 0,
    scene_users_total integer NOT NULL DEFAULT 0,
    worlds_polled integer NOT NULL DEFAULT 0,
    active_worlds integer NOT NULL DEFAULT 0,
    world_users_total integer NOT NULL DEFAULT 0,
    worlds_live_total integer
);

-- current: ORDER BY taken_at DESC LIMIT 1 (and the latest-snapshot sub-select of the two current lists).
CREATE INDEX snapshots_taken_at_idx ON presence.snapshots (taken_at DESC);

CREATE TABLE presence.peer_snapshots (
    snapshot_id bigint NOT NULL REFERENCES presence.snapshots(id) ON DELETE CASCADE,
    address varchar(42) NOT NULL,
    parcel_x integer,
    parcel_y integer,
    position_x double precision,
    position_y double precision,
    position_z double precision,
    last_ping bigint,
    PRIMARY KEY (snapshot_id, address)
);

CREATE TABLE presence.island_snapshots (
    snapshot_id bigint NOT NULL REFERENCES presence.snapshots(id) ON DELETE CASCADE,
    island_id text NOT NULL,
    peer_count integer,
    max_peers integer,
    center_x double precision,
    center_y double precision,
    center_z double precision,
    radius double precision,
    PRIMARY KEY (snapshot_id, island_id)
);

CREATE TABLE presence.hot_scene_snapshots (
    snapshot_id bigint NOT NULL REFERENCES presence.snapshots(id) ON DELETE CASCADE,
    scene_id text NOT NULL,
    name text,
    base_x integer,
    base_y integer,
    users_count integer,
    parcel_count integer,
    creator text,
    description text,
    PRIMARY KEY (snapshot_id, scene_id)
);

-- Per-scene occupancy: `pointer` is the base parcel "x,y", `realm` the
-- genesis realm; `addresses` the lowercase wallets in the scene.
CREATE TABLE presence.scene_occupancy (
    snapshot_id bigint NOT NULL REFERENCES presence.snapshots(id) ON DELETE CASCADE,
    taken_at timestamp without time zone NOT NULL,
    pointer text NOT NULL,
    scene_name text,
    realm text NOT NULL DEFAULT 'main',
    addresses jsonb NOT NULL,
    count integer NOT NULL,
    PRIMARY KEY (snapshot_id, pointer, realm),
    CONSTRAINT scene_occupancy_count_matches CHECK (count = jsonb_array_length(addresses))
);

-- scene_history: WHERE pointer = $1 ORDER BY taken_at DESC LIMIT $2.
CREATE INDEX scene_occupancy_pointer_idx ON presence.scene_occupancy (pointer, taken_at DESC);

CREATE TABLE presence.world_membership (
    snapshot_id bigint NOT NULL REFERENCES presence.snapshots(id) ON DELETE CASCADE,
    taken_at timestamp without time zone NOT NULL,
    world_name text NOT NULL,
    addresses jsonb NOT NULL,
    count integer NOT NULL,
    live_users integer,
    PRIMARY KEY (snapshot_id, world_name),
    CONSTRAINT world_membership_count_matches CHECK (count = jsonb_array_length(addresses))
);

-- world_history: WHERE world_name = $1 ORDER BY taken_at DESC LIMIT $2.
CREATE INDEX world_membership_world_idx ON presence.world_membership (world_name, taken_at DESC);

-- The per-UTC-day rollup the collector upserts after each pass; derived,
-- rebuilt by re-aggregating the day's snapshots.
CREATE TABLE presence.daily_stats (
    date date PRIMARY KEY,
    snapshots_taken integer NOT NULL DEFAULT 0,
    peak_peers integer NOT NULL DEFAULT 0,
    avg_peers double precision NOT NULL DEFAULT 0,
    peak_hot_scene_users integer NOT NULL DEFAULT 0,
    peak_scene_users integer NOT NULL DEFAULT 0,
    peak_world_users integer NOT NULL DEFAULT 0
);

-- ════ schema/40_social.sql ════

-- Catalyst v2: social domain: friendships, blocks, mutes, private calls,
-- communities (members, bans, requests, places, posts, thumbnails, voice
-- presence), scene and platform moderation (the comms-gatekeeper tables),
-- notifications with explicit delivery state, subscriptions, badges,
-- referral and the translation cache.
--
-- Designed from the workload in schema/WORKLOADS.md ("social"), not copied:
-- the old bundle's five databases (communities, its unused twin social_rpc,
-- comms_gatekeeper, notifications, badges, media) collapse into ONE schema;
-- the merges and the tables deliberately left to the domain agents are
-- recorded there, per table, with the query behind every index below.
-- Sources: catalyrst/crates/catalyrst-social-service/migrations 0001..0013,
-- catalyrst-comms/migrations 0001..0009, catalyrst-notifications/migrations
-- 0001..0008, catalyrst-badges/migrations 0001..0005,
-- catalyrst-media/migrations 0001.
--
-- Conventions: wallet addresses are lowercase 0x + 40 hex text
-- (src/social/identity.rs::Address binds and decodes them); community ids are
-- uuid; place ids are the places catalogue text id or a world name; every
-- timestamp column is `timestamp without time zone` holding UTC (the pool pins
-- TimeZone=UTC); the current time is always a bind parameter, never in an
-- index predicate. No FK crosses a schema (community_places.place_id,
-- scene_*.place_id name explore/worlds rows and reconcile on read).
--
-- Domain agents append ONLY inside the four `-- ==== section: ... ====`
-- blocks at the end (communities federation log, comms MLS, notification
-- producers, badges seed), after re-reading this file.

CREATE SCHEMA social;

-- ── settings ───────────────────────────────────────────────────────────────

-- One row per wallet that changed a setting; an absent row means the
-- defaults (the read path COALESCEs). Merges social_settings (social-rpc) and
-- private_messages_privacy (gatekeeper): one fact, one row.
CREATE TABLE social.user_settings (
    address text NOT NULL,
    private_messages_privacy text NOT NULL DEFAULT 'only_friends',
    blocked_users_messages_visibility text NOT NULL DEFAULT 'show_messages',
    show_situation_reactions text NOT NULL DEFAULT 'show',
    updated_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    CONSTRAINT user_settings_pkey PRIMARY KEY (address),
    CONSTRAINT user_settings_private_messages_privacy_check
        CHECK (private_messages_privacy IN ('only_friends', 'all')),
    CONSTRAINT user_settings_blocked_visibility_check
        CHECK (blocked_users_messages_visibility IN ('show_messages', 'do_not_show_messages')),
    CONSTRAINT user_settings_situation_reactions_check
        CHECK (show_situation_reactions IN ('show', 'hide'))
);

-- ── friendships ────────────────────────────────────────────────────────────

-- One canonical row per unordered pair. last_action/last_actor/last_action_at
-- are the projection of the latest friendship_actions row and is_active the
-- projection of that (accept => true, everything else => false); all four
-- move in the same statement as the action insert (rpc db.rs
-- apply_friendship_action), so the pending-request state is a keyed column,
-- not a re-derivation over the history.
CREATE TABLE social.friendships (
    id uuid NOT NULL,
    address_requester text NOT NULL,
    address_requested text NOT NULL,
    is_active boolean NOT NULL DEFAULT false,
    last_action text NOT NULL,
    last_actor text NOT NULL,
    last_action_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    created_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    updated_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    CONSTRAINT friendships_pkey PRIMARY KEY (id),
    CONSTRAINT friendships_not_self_check CHECK (address_requester <> address_requested),
    CONSTRAINT friendships_last_action_check
        CHECK (last_action IN ('request', 'accept', 'cancel', 'reject', 'delete', 'block')),
    CONSTRAINT friendships_last_actor_check CHECK (last_actor IN (address_requester, address_requested)),
    CONSTRAINT friendships_active_projection_check CHECK (is_active = (last_action = 'accept'))
);

-- The unordered pair is unique (the 0009 race fix); the insert conflicts on
-- these two expressions and the pair status probe is written on them.
CREATE UNIQUE INDEX friendships_unordered_pair_uidx
    ON social.friendships (LEAST(address_requester, address_requested),
                           GREATEST(address_requester, address_requested));
-- Every friends read is `address_requester = $me OR address_requested = $me`
-- (friends list, mutual friends, counts): one arm each, bitmap OR.
CREATE INDEX friendships_requester_idx ON social.friendships (address_requester);
CREATE INDEX friendships_requested_idx ON social.friendships (address_requested);
-- The pending requests received/sent by a wallet (`last_action = 'request'
-- AND (arm = $me) AND last_actor <>/= $me ORDER BY last_action_at DESC, id
-- DESC`): one arm each over the pending rows only, so the page and its total
-- are bounded by the wallet's pending count, not its degree.
CREATE INDEX friendships_pending_requester_idx
    ON social.friendships (address_requester, last_action_at DESC, id DESC) WHERE last_action = 'request';
CREATE INDEX friendships_pending_requested_idx
    ON social.friendships (address_requested, last_action_at DESC, id DESC) WHERE last_action = 'request';

-- Append-only history; the latest row per friendship is the status.
CREATE TABLE social.friendship_actions (
    id uuid NOT NULL,
    friendship_id uuid NOT NULL REFERENCES social.friendships (id) ON DELETE CASCADE,
    action text NOT NULL,
    acting_user text NOT NULL,
    metadata json,
    "timestamp" timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    CONSTRAINT friendship_actions_pkey PRIMARY KEY (id),
    CONSTRAINT friendship_actions_action_check
        CHECK (action IN ('request', 'accept', 'cancel', 'reject', 'delete', 'block'))
);

-- The "last action" lateral of the status probes and the request message
-- lookup of a pending-requests page row.
CREATE INDEX friendship_actions_friendship_time_idx
    ON social.friendship_actions (friendship_id, "timestamp" DESC, id DESC);

-- Directional: A blocking B is not B blocking A.
CREATE TABLE social.blocks (
    blocker_address text NOT NULL,
    blocked_address text NOT NULL,
    blocked_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    CONSTRAINT blocks_pkey PRIMARY KEY (blocker_address, blocked_address),
    CONSTRAINT blocks_not_self_check CHECK (blocker_address <> blocked_address)
);

-- "Who blocked me" (GetBlockingStatus) and the per-row blocked_by EXISTS.
CREATE INDEX blocks_blocked_idx ON social.blocks (blocked_address);

CREATE TABLE social.user_mutes (
    muter_address text NOT NULL,
    muted_address text NOT NULL,
    muted_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    CONSTRAINT user_mutes_pkey PRIMARY KEY (muter_address, muted_address),
    CONSTRAINT user_mutes_not_self_check CHECK (muter_address <> muted_address)
);

CREATE TABLE social.friend_messages (
    id bigint GENERATED ALWAYS AS IDENTITY,
    sender_address text NOT NULL,
    recipient_address text NOT NULL,
    body text NOT NULL,
    created_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    CONSTRAINT friend_messages_pkey PRIMARY KEY (id),
    CONSTRAINT friend_messages_not_self_check CHECK (sender_address <> recipient_address)
);

-- The conversation page (`created_at < $before ORDER BY created_at DESC, id
-- DESC LIMIT n`) and its count, written on the pair expressions so one index
-- serves both directions.
CREATE INDEX friend_messages_pair_idx
    ON social.friend_messages (LEAST(sender_address, recipient_address),
                               GREATEST(sender_address, recipient_address),
                               created_at DESC, id DESC);

-- ── private voice calls ────────────────────────────────────────────────────

-- The call; its LiveKit room is 'voice-chat-private-' || id. Live while
-- created_at >= $now - PRIVATE_VOICE_CHAT_EXPIRATION_TIME (bind parameter);
-- the sweep deletes expired calls in bounded created_at order.
CREATE TABLE social.private_voice_chats (
    id uuid NOT NULL,
    caller_address text NOT NULL,
    callee_address text NOT NULL,
    created_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    CONSTRAINT private_voice_chats_pkey PRIMARY KEY (id),
    CONSTRAINT private_voice_chats_caller_key UNIQUE (caller_address),
    CONSTRAINT private_voice_chats_callee_key UNIQUE (callee_address),
    CONSTRAINT private_voice_chats_no_self_call_check CHECK (caller_address <> callee_address)
);

-- The expiry sweep and the admin "active calls" read.
CREATE INDEX private_voice_chats_created_idx ON social.private_voice_chats (created_at);

-- Presence in the call room (the old comms voice_chat_users); follows the
-- transport's lifecycle and the call row's.
CREATE TABLE social.private_voice_chat_participants (
    call_id uuid NOT NULL REFERENCES social.private_voice_chats (id) ON DELETE CASCADE,
    address text NOT NULL,
    status text NOT NULL,
    joined_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    status_updated_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    CONSTRAINT private_voice_chat_participants_pkey PRIMARY KEY (call_id, address),
    CONSTRAINT private_voice_chat_participants_status_check
        CHECK (status IN ('connected', 'connection_interrupted', 'disconnected', 'not_connected'))
);

-- The presence sweep: `status = X AND status_updated_at <= $now - ttl`.
CREATE INDEX private_voice_chat_participants_status_idx
    ON social.private_voice_chat_participants (status, status_updated_at);

-- ── communities ────────────────────────────────────────────────────────────

-- active = false is the (federation-visible) deletion; suspended is the local
-- operator hold (a separate axis, 0005). ranking_score has no writer yet
-- (derived column, source "none"; stays 0), editors_choice is set by the
-- admin route; both are read by the listing's ORDER BY.
CREATE TABLE social.communities (
    id uuid NOT NULL,
    name text NOT NULL,
    description text NOT NULL DEFAULT '',
    owner_address text NOT NULL,
    private boolean NOT NULL DEFAULT false,
    active boolean NOT NULL DEFAULT true,
    unlisted boolean NOT NULL DEFAULT false,
    editors_choice boolean NOT NULL DEFAULT false,
    ranking_score real NOT NULL DEFAULT 0,
    suspended boolean NOT NULL DEFAULT false,
    suspended_at timestamp without time zone,
    suspended_by text,
    suspension_reason text,
    created_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    updated_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    CONSTRAINT communities_pkey PRIMARY KEY (id),
    CONSTRAINT communities_suspension_check
        CHECK ((suspended AND suspended_at IS NOT NULL AND suspended_by IS NOT NULL)
            OR (NOT suspended AND suspended_at IS NULL AND suspended_by IS NULL AND suspension_reason IS NULL))
);

-- The public listing's ORDER BY; the listing carries exactly this predicate.
CREATE INDEX communities_ranking_idx
    ON social.communities (editors_choice DESC, ranking_score DESC, name ASC)
    WHERE active AND NOT unlisted AND NOT suspended;
-- "communities I own" and the ownership checks on delete/transfer.
CREATE INDEX communities_owner_idx ON social.communities (owner_address);

-- One spelling for a moderator ('moderator'; 'mod' is accepted on input by
-- identity.rs). A banned wallet has no row: the ban write deletes it in the
-- same transaction, and rejoin is refused while community_bans.active.
CREATE TABLE social.community_members (
    community_id uuid NOT NULL REFERENCES social.communities (id) ON DELETE CASCADE,
    member_address text NOT NULL,
    role text NOT NULL,
    joined_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    CONSTRAINT community_members_pkey PRIMARY KEY (community_id, member_address),
    CONSTRAINT community_members_role_check CHECK (role IN ('owner', 'moderator', 'member'))
);

-- Exactly one owner per community (communities.owner_address names it; the
-- transfer writes both rows in one transaction).
CREATE UNIQUE INDEX community_members_owner_uidx
    ON social.community_members (community_id) WHERE role = 'owner';
-- "my communities", onlyMemberOf, friends-in-community, invite candidates.
CREATE INDEX community_members_member_idx
    ON social.community_members (member_address, community_id);

CREATE TABLE social.community_bans (
    community_id uuid NOT NULL REFERENCES social.communities (id) ON DELETE CASCADE,
    banned_address text NOT NULL,
    banned_by text NOT NULL,
    banned_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    reason text,
    active boolean NOT NULL DEFAULT true,
    unbanned_by text,
    unbanned_at timestamp without time zone,
    CONSTRAINT community_bans_pkey PRIMARY KEY (community_id, banned_address),
    CONSTRAINT community_bans_lift_check
        CHECK ((active AND unbanned_by IS NULL AND unbanned_at IS NULL)
            OR (NOT active AND unbanned_by IS NOT NULL AND unbanned_at IS NOT NULL))
);

-- The moderator's bans listing (ORDER BY banned_at) and its count.
CREATE INDEX community_bans_active_idx
    ON social.community_bans (community_id, banned_at) WHERE active;

-- Invites (by owner/moderator) and join requests (by the wallet). Terminal
-- statuses are never reopened; a new request is a new row.
CREATE TABLE social.community_requests (
    id uuid NOT NULL,
    community_id uuid NOT NULL REFERENCES social.communities (id) ON DELETE CASCADE,
    member_address text NOT NULL,
    type text NOT NULL,
    status text NOT NULL DEFAULT 'pending',
    created_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    updated_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    CONSTRAINT community_requests_pkey PRIMARY KEY (id),
    CONSTRAINT community_requests_type_check CHECK (type IN ('invite', 'request_to_join')),
    CONSTRAINT community_requests_status_check
        CHECK (status IN ('pending', 'accepted', 'rejected', 'cancelled'))
);

-- At most one pending request per (community, wallet), any type; also the
-- private-community visibility gate ("pending invite for me").
CREATE UNIQUE INDEX community_requests_pending_uidx
    ON social.community_requests (community_id, member_address) WHERE status = 'pending';
-- The moderator's listing/count by status (optionally type).
CREATE INDEX community_requests_community_idx
    ON social.community_requests (community_id, status, created_at DESC);
-- "my invites / my requests".
CREATE INDEX community_requests_member_idx
    ON social.community_requests (member_address, status, created_at DESC);

-- place_id is explore.place.id (uuid text) or a world name; no cross-schema
-- FK: a place the mirror dropped is filtered on read.
CREATE TABLE social.community_places (
    place_id text NOT NULL,
    community_id uuid NOT NULL REFERENCES social.communities (id) ON DELETE CASCADE,
    added_by text NOT NULL,
    added_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    CONSTRAINT community_places_pkey PRIMARY KEY (place_id, community_id)
);

-- The community's places listing (ORDER BY added_at DESC) and count.
CREATE INDEX community_places_community_idx
    ON social.community_places (community_id, added_at DESC);

CREATE TABLE social.community_posts (
    id uuid NOT NULL DEFAULT gen_random_uuid(),
    community_id uuid NOT NULL REFERENCES social.communities (id) ON DELETE CASCADE,
    author_address text NOT NULL,
    content text NOT NULL,
    created_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    CONSTRAINT community_posts_pkey PRIMARY KEY (id)
);

-- The feed (newest first) and the keyset tie-breaker of the new endpoint.
CREATE INDEX community_posts_feed_idx
    ON social.community_posts (community_id, created_at DESC, id DESC);

CREATE TABLE social.community_post_likes (
    post_id uuid NOT NULL REFERENCES social.community_posts (id) ON DELETE CASCADE,
    user_address text NOT NULL,
    liked_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    CONSTRAINT community_post_likes_pkey PRIMARY KEY (post_id, user_address)
);

-- The bytes live in the ONE blob store; has_thumbnail on the wire = row exists.
CREATE TABLE social.community_thumbnails (
    community_id uuid NOT NULL REFERENCES social.communities (id) ON DELETE CASCADE,
    thumbnail_hash text NOT NULL,
    updated_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    CONSTRAINT community_thumbnails_pkey PRIMARY KEY (community_id)
);

-- Presence in the community voice room ('voice-chat-community-' ||
-- community_id); presence is not permission. Participant/moderator counts and
-- "has an active voice chat" derive from this table.
CREATE TABLE social.community_voice_chat_users (
    community_id uuid NOT NULL REFERENCES social.communities (id) ON DELETE CASCADE,
    address text NOT NULL,
    is_moderator boolean NOT NULL DEFAULT false,
    status text NOT NULL,
    sid text,
    joined_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    status_updated_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    created_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    CONSTRAINT community_voice_chat_users_pkey PRIMARY KEY (community_id, address),
    CONSTRAINT community_voice_chat_users_status_check
        CHECK (status IN ('connected', 'connection_interrupted', 'disconnected', 'not_connected'))
);

-- The sweep and the active-rooms/moderator-count aggregates.
CREATE INDEX community_voice_chat_users_status_idx
    ON social.community_voice_chat_users (status, status_updated_at);

-- ── scene moderation (comms-gatekeeper, place-scoped) ──────────────────────

-- Ownership of a place is resolved through explore/worlds, not stored here.
-- These three move to `worlds` unchanged when worlds/comms lands.
CREATE TABLE social.scene_admins (
    place_id text NOT NULL,
    admin_address text NOT NULL,
    added_by text NOT NULL,
    added_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    active boolean NOT NULL DEFAULT true,
    removed_at timestamp without time zone,
    CONSTRAINT scene_admins_pkey PRIMARY KEY (place_id, admin_address),
    CONSTRAINT scene_admins_removed_check
        CHECK ((active AND removed_at IS NULL) OR (NOT active AND removed_at IS NOT NULL))
);

CREATE TABLE social.scene_bans (
    place_id text NOT NULL,
    banned_address text NOT NULL,
    banned_by text NOT NULL,
    banned_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    CONSTRAINT scene_bans_pkey PRIMARY KEY (place_id, banned_address)
);

-- The bans listing (ORDER BY banned_at DESC) with its count(*) OVER ().
CREATE INDEX scene_bans_place_time_idx ON social.scene_bans (place_id, banned_at DESC);

-- ── platform moderation ────────────────────────────────────────────────────

-- A ban is in force iff lifted_at IS NULL AND (expires_at IS NULL OR
-- expires_at > $now). One in-force ban per address, enforced by the ban
-- write under pg_advisory_xact_lock(hashtext(address)) (the expiry half is
-- time-dependent, so no unique index can express it).
CREATE TABLE social.user_bans (
    id uuid NOT NULL DEFAULT gen_random_uuid(),
    banned_address text NOT NULL,
    banned_by text NOT NULL,
    reason text NOT NULL,
    custom_message text,
    banned_device_id text,
    banned_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    expires_at timestamp without time zone,
    lifted_at timestamp without time zone,
    lifted_by text,
    created_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    CONSTRAINT user_bans_pkey PRIMARY KEY (id),
    CONSTRAINT user_bans_lift_check
        CHECK ((lifted_at IS NULL AND lifted_by IS NULL) OR (lifted_at IS NOT NULL AND lifted_by IS NOT NULL))
);

-- The connection-ban probe (every LiveKit token), get_status (latest in
-- force) and the admin "active bans" listing; expiry is filtered per row.
CREATE INDEX user_bans_open_by_address_idx
    ON social.user_bans (banned_address, banned_at DESC) WHERE lifted_at IS NULL;
-- The device half of the probe (a banned device under a new wallet).
CREATE INDEX user_bans_open_by_device_idx
    ON social.user_bans (banned_device_id) WHERE lifted_at IS NULL AND banned_device_id IS NOT NULL;

CREATE TABLE social.user_warnings (
    id uuid NOT NULL DEFAULT gen_random_uuid(),
    warned_address text NOT NULL,
    warned_by text NOT NULL,
    reason text NOT NULL,
    warned_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    CONSTRAINT user_warnings_pkey PRIMARY KEY (id)
);

-- A wallet's warnings, newest first.
CREATE INDEX user_warnings_address_idx ON social.user_warnings (warned_address, warned_at DESC);

-- Latest connection facts per player, upserted best-effort on token minting
-- (0006); the device id is what a ban snapshots.
CREATE TABLE social.player_connections (
    address text NOT NULL,
    ip_address text,
    device_id text,
    created_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    updated_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    CONSTRAINT player_connections_pkey PRIMARY KEY (address)
);

-- ── notifications ──────────────────────────────────────────────────────────

-- In-app delivery state is read/read_at on the row; occurred_at is the wire's
-- epoch-ms `timestamp`/`from` at ms precision, converted at the edge.
CREATE TABLE social.notifications (
    id uuid NOT NULL,
    address text NOT NULL,
    type text NOT NULL,
    metadata jsonb NOT NULL DEFAULT '{}'::jsonb,
    broadcast_key text,
    occurred_at timestamp(3) without time zone NOT NULL,
    read boolean NOT NULL DEFAULT false,
    read_at timestamp(3) without time zone,
    created_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    CONSTRAINT notifications_pkey PRIMARY KEY (id),
    CONSTRAINT notifications_read_check CHECK ((read AND read_at IS NOT NULL) OR (NOT read AND read_at IS NULL))
);

-- The bell's poll (`address = $me AND occurred_at > $from ORDER BY
-- occurred_at DESC LIMIT n`) and its keyset tie-breaker.
CREATE INDEX notifications_address_time_idx
    ON social.notifications (address, occurred_at DESC, id DESC);
-- The onlyUnread poll (`address = $1 AND NOT read ORDER BY occurred_at DESC,
-- id DESC`: the statement spells NOT read so the generic plan matches the
-- partial predicate, and carries the keyset tie-breaker).
CREATE INDEX notifications_unread_idx
    ON social.notifications (address, occurred_at DESC, id DESC) WHERE NOT read;
-- A re-run broadcast is idempotent per recipient.
CREATE UNIQUE INDEX notifications_broadcast_uidx
    ON social.notifications (address, broadcast_key) WHERE broadcast_key IS NOT NULL;

-- Explicit delivery state for the out-of-process channels; the worker claims
-- a bounded due set (`state = 'pending' AND next_attempt_at <= $now`) with
-- FOR UPDATE SKIP LOCKED, sends outside the transaction, records the outcome.
CREATE TABLE social.notification_deliveries (
    notification_id uuid NOT NULL REFERENCES social.notifications (id) ON DELETE CASCADE,
    channel text NOT NULL,
    state text NOT NULL DEFAULT 'pending',
    attempts integer NOT NULL DEFAULT 0,
    next_attempt_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    last_error text,
    updated_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    CONSTRAINT notification_deliveries_pkey PRIMARY KEY (notification_id, channel),
    CONSTRAINT notification_deliveries_channel_check CHECK (channel IN ('email', 'push')),
    CONSTRAINT notification_deliveries_state_check CHECK (state IN ('pending', 'sent', 'failed', 'skipped')),
    CONSTRAINT notification_deliveries_attempts_check CHECK (attempts >= 0)
);

-- The worker's due set; `now` is the bind, never the predicate.
CREATE INDEX notification_deliveries_due_idx
    ON social.notification_deliveries (next_attempt_at, notification_id) WHERE state = 'pending';

-- details is the @dcl/schemas SubscriptionDetails document, read and written
-- as a unit; the 0001 unconfirmed_email/email_confirmation_token columns were
-- superseded by unconfirmed_emails (0002) and are not carried.
CREATE TABLE social.subscriptions (
    address text NOT NULL,
    email text,
    details jsonb NOT NULL DEFAULT '{}'::jsonb,
    is_credits_workflow boolean NOT NULL DEFAULT false,
    updated_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    CONSTRAINT subscriptions_pkey PRIMARY KEY (address)
);

-- The cross-account "email already used" check.
CREATE INDEX subscriptions_email_idx ON social.subscriptions (lower(email)) WHERE email IS NOT NULL;

-- A pending email change never overwrites the confirmed one until its code
-- is presented.
CREATE TABLE social.unconfirmed_emails (
    address text NOT NULL,
    email text NOT NULL,
    code text NOT NULL,
    source text NOT NULL DEFAULT 'account',
    created_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    CONSTRAINT unconfirmed_emails_pkey PRIMARY KEY (address)
);

-- Per-scope opt-outs (scope = 'community', scope_id = the community id).
CREATE TABLE social.subscription_opt_outs (
    address text NOT NULL,
    scope text NOT NULL,
    scope_id text NOT NULL,
    created_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    CONSTRAINT subscription_opt_outs_pkey PRIMARY KEY (address, scope, scope_id)
);

-- "Online since" proxy: the most recent authenticated list fetch, upserted
-- with GREATEST so replicas never move it backwards.
CREATE TABLE social.notification_reader_seen (
    address text NOT NULL,
    last_fetch_at timestamp(3) without time zone NOT NULL,
    CONSTRAINT notification_reader_seen_pkey PRIMARY KEY (address)
);

-- ── badges ─────────────────────────────────────────────────────────────────

-- Reference data (seeded by the badges module/agent; schema.sql carries no
-- data rows). assets is the { "2d": {...}, "3d": {...} } document.
CREATE TABLE social.badge_definitions (
    id text NOT NULL,
    name text NOT NULL,
    description text,
    category text,
    is_tier boolean NOT NULL DEFAULT false,
    assets jsonb NOT NULL DEFAULT '{}'::jsonb,
    CONSTRAINT badge_definitions_pkey PRIMARY KEY (id)
);

CREATE TABLE social.badge_tiers (
    badge_id text NOT NULL REFERENCES social.badge_definitions (id) ON DELETE CASCADE,
    tier_id text NOT NULL,
    tier_name text NOT NULL,
    description text,
    assets jsonb NOT NULL DEFAULT '{}'::jsonb,
    criteria_steps integer NOT NULL DEFAULT 0,
    ordinal integer NOT NULL DEFAULT 0,
    CONSTRAINT badge_tiers_pkey PRIMARY KEY (badge_id, tier_id)
);

-- granted_by distinguishes a console grant from an event-consumer write.
CREATE TABLE social.user_badge_progress (
    address text NOT NULL,
    badge_id text NOT NULL REFERENCES social.badge_definitions (id) ON DELETE CASCADE,
    steps_done integer NOT NULL DEFAULT 0,
    completed_at timestamp without time zone,
    last_completed_tier_id text,
    granted_by text,
    updated_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    CONSTRAINT user_badge_progress_pkey PRIMARY KEY (address, badge_id),
    CONSTRAINT user_badge_progress_steps_check CHECK (steps_done >= 0)
);

CREATE TABLE social.user_achieved_tiers (
    address text NOT NULL,
    badge_id text NOT NULL REFERENCES social.badge_definitions (id) ON DELETE CASCADE,
    tier_id text NOT NULL,
    completed_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    granted_by text,
    granted_at timestamp without time zone,
    CONSTRAINT user_achieved_tiers_pkey PRIMARY KEY (address, badge_id, tier_id)
);

-- Append-only; revoke deletes the user rows, so provenance lands here.
CREATE TABLE social.badge_admin_audit (
    id bigint GENERATED ALWAYS AS IDENTITY,
    action text NOT NULL,
    address text NOT NULL,
    badge_id text NOT NULL,
    tier_id text,
    actor text NOT NULL,
    acted_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    CONSTRAINT badge_admin_audit_pkey PRIMARY KEY (id),
    CONSTRAINT badge_admin_audit_action_check CHECK (action IN ('grant', 'revoke'))
);

-- ── referral ───────────────────────────────────────────────────────────────

-- One row per invited wallet; first referrer wins (ON CONFLICT DO NOTHING).
CREATE TABLE social.referral_progress (
    invited_user text NOT NULL,
    referrer text NOT NULL,
    status text NOT NULL DEFAULT 'pending',
    created_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    updated_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    CONSTRAINT referral_progress_pkey PRIMARY KEY (invited_user),
    CONSTRAINT referral_progress_status_check
        CHECK (status IN ('pending', 'signed_up', 'tier_granted', 'rejected_ip_match')),
    CONSTRAINT referral_progress_not_self_check CHECK (invited_user <> referrer)
);

-- The referrer's `COUNT(*) WHERE status = 'tier_granted'` and progress list.
CREATE INDEX referral_progress_referrer_idx ON social.referral_progress (referrer, status);

-- The invitedUsersAccepted count each referrer last saw.
CREATE TABLE social.referral_progress_viewed (
    referrer text NOT NULL,
    invites_accepted_viewed bigint NOT NULL DEFAULT 0,
    updated_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    CONSTRAINT referral_progress_viewed_pkey PRIMARY KEY (referrer)
);

-- ── media: translation cache ───────────────────────────────────────────────

-- Cache of an external engine's answer per (backend, target_lang,
-- sha256(source text)); rebuild = ask the engine again.
CREATE TABLE social.translation_cache (
    backend text NOT NULL,
    target_lang text NOT NULL,
    text_sha256 bytea NOT NULL,
    translated_text text NOT NULL,
    detected_language text NOT NULL DEFAULT 'en',
    detected_confidence real NOT NULL DEFAULT 1.0,
    created_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    CONSTRAINT translation_cache_pkey PRIMARY KEY (backend, target_lang, text_sha256),
    CONSTRAINT translation_cache_hash_len_check CHECK (octet_length(text_sha256) = 32)
);

-- ══ Domain-agent sections: append inside the matching block only ═══════════

-- ==== section: communities ====
-- Left to the communities agent: the federation signed-action log
-- (communities_local, community_*_log, seen_nonces) if the federation routes
-- are ported. Rule (WORKLOADS.md): ONE append-only community_signed_actions
-- keyed by signature_hash; projections into the keyed relations above run in
-- the same transaction; community_role_current is not recreated.
-- ==== end section: communities ====

-- ==== section: comms ====
-- Left to the comms agent: the MLS delivery-service tables (mls_key_packages,
-- mls_groups, mls_group_members, mls_commits, mls_message_blobs,
-- mls_message_refs) and fed_instance_identity. Opaque bytes; large blobs by
-- hash in the blob store. archipelago_v4_assignments is NOT created here
-- (WORKLOADS.md, "comms_control"). Two route classes of the old gatekeeper
-- are left out and their tables are NOT created until the routes are ported
-- (DATABASE.md: every index names a query in the tree; dead DDL becomes a
-- migration once deployed): /scene-stream-access (scene_stream_access, the
-- LiveKit ingress credentials, one active row per place) and the player
-- reports (player_reports, player_report_evidence: presign -> upload ->
-- submit, evidence bytes in the ONE blob store). WORKLOADS.md keeps both
-- designs under "not created".
-- The gatekeeper slice (src/social/comms: scene adapter, scene admins/bans,
-- platform bans, private-message and voice tokens, community voice, the
-- LiveKit webhook) adds no relation: it reads and writes the tables above
-- (WORKLOADS.md, "Query sites of the comms gatekeeper"). MLS and
-- fed_instance_identity were not ported and this section stays theirs.
-- ==== end section: comms ====

-- ==== section: notifications ====
-- The friend_first_wear producer's durable state (WORKLOADS.md, "worn_history,
-- first_wear_baseline, first_wear_cursor"). land_transfer_cursor is NOT created:
-- its source (squid_marketplace.transfer) does not exist in v2.

-- Every marketplace URN a wallet has ever worn in a deployed profile; the PK
-- is the claim that stops a second notification for the same first wear.
CREATE TABLE social.worn_history (
    address text NOT NULL,
    urn text NOT NULL,
    first_seen timestamp(3) without time zone NOT NULL,
    CONSTRAINT worn_history_pkey PRIMARY KEY (address, urn)
);

-- A wallet with a row is eligible; its first profile seen seeds worn_history
-- silently (an empty outfit still gets a row, or its real first wear would be
-- swallowed by the seed path).
CREATE TABLE social.first_wear_baseline (
    address text NOT NULL,
    seeded_at timestamp(3) without time zone NOT NULL,
    CONSTRAINT first_wear_baseline_pkey PRIMARY KEY (address)
);

-- The entity_timestamp high-water mark over content.deployments profiles;
-- initialised to now on the first pass so history is never replayed.
CREATE TABLE social.first_wear_cursor (
    id integer NOT NULL DEFAULT 1,
    last_seen timestamp without time zone NOT NULL,
    CONSTRAINT first_wear_cursor_pkey PRIMARY KEY (id),
    CONSTRAINT first_wear_cursor_singleton_check CHECK (id = 1)
);
-- ==== end section: notifications ====

-- ==== section: badges ====
-- Reference rows (WORKLOADS.md, "badge seed rows"): the old crate's fixture
-- (catalyrst-badges/migrations/0002_seed_fixture.sql with the 0005 asset
-- fix-up applied), so `catalyst schema apply` is the one bootstrap. Asset URLs
-- keep the upstream host; the badges module rewrites it to
-- BADGES_PUBLIC_ASSET_BASE_URL at serve time and serves the embedded PNGs at
-- /badges/assets/{path}. Never per-user rows.
INSERT INTO social.badge_definitions (id, name, description, category, is_tier, assets) VALUES
  ('open_for_business', 'Open for Business', 'Set up a store and publish a collection.', 'Builder', false,
   '{"2d":{"normal":"https://badges.decentraland.org/assets/open_for_business/2d/normal.png","hrm":"https://badges.decentraland.org/assets/open_for_business/2d/hrm.png","baseColor":"https://badges.decentraland.org/assets/open_for_business/2d/baseColor.png"},"3d":{"normal":"https://badges.decentraland.org/assets/open_for_business/3d/normal.png","hrm":"https://badges.decentraland.org/assets/open_for_business/3d/hrm.png","baseColor":"https://badges.decentraland.org/assets/open_for_business/3d/baseColor.png"}}'::jsonb),
  ('decentraland_citizen', 'Decentraland Citizen', 'Log in to Decentraland for the first time.', 'Explorer', false,
   '{"2d":{"normal":"https://badges.decentraland.org/assets/decentraland_citizen/2d/normal.png","hrm":"","baseColor":""},"3d":{"normal":"https://badges.decentraland.org/assets/decentraland_citizen/3d/normal.png","hrm":"https://badges.decentraland.org/assets/decentraland_citizen/3d/hrm.png","baseColor":"https://badges.decentraland.org/assets/decentraland_citizen/3d/baseColor.png"}}'::jsonb),
  ('walkabout', 'Walkabout', 'Walk a cumulative distance across Decentraland.', 'Explorer', true,
   '{"2d":{"normal":"https://badges.decentraland.org/assets/walkabout/2d/normal.png","hrm":"","baseColor":""},"3d":{"normal":"https://badges.decentraland.org/assets/walkabout/3d/normal.png","hrm":"https://badges.decentraland.org/assets/walkabout/3d/hrm.png","baseColor":"https://badges.decentraland.org/assets/walkabout/3d/baseColor.png"}}'::jsonb),
  ('emotionista', 'Emotionista', 'Play emotes in-world.', 'Socializer', true,
   '{"2d":{"normal":"https://badges.decentraland.org/assets/emotionista/2d/normal.png","hrm":"","baseColor":""},"3d":{"normal":"https://badges.decentraland.org/assets/emotionista/3d/normal.png","hrm":"https://badges.decentraland.org/assets/emotionista/3d/hrm.png","baseColor":"https://badges.decentraland.org/assets/emotionista/3d/baseColor.png"}}'::jsonb);

INSERT INTO social.badge_tiers (badge_id, tier_id, tier_name, description, assets, criteria_steps, ordinal) VALUES
  ('walkabout', 'walkabout-starter', 'Starter', 'Walk 1,000 meters.', '{"2d":{"normal":"https://badges.decentraland.org/assets/walkabout/starter/2d/normal.png"}}'::jsonb, 1000, 0),
  ('walkabout', 'walkabout-bronze', 'Bronze', 'Walk 10,000 meters.', '{"2d":{"normal":"https://badges.decentraland.org/assets/walkabout/bronze/2d/normal.png"}}'::jsonb, 10000, 1),
  ('walkabout', 'walkabout-silver', 'Silver', 'Walk 100,000 meters.', '{"2d":{"normal":"https://badges.decentraland.org/assets/walkabout/silver/2d/normal.png"}}'::jsonb, 100000, 2),
  ('walkabout', 'walkabout-gold', 'Gold', 'Walk 1,000,000 meters.', '{"2d":{"normal":"https://badges.decentraland.org/assets/walkabout/gold/2d/normal.png"}}'::jsonb, 1000000, 3),
  ('emotionista', 'emotionista-bronze', 'Bronze', 'Play 10 emotes.', '{"2d":{"normal":"https://badges.decentraland.org/assets/emotionista/bronze/2d/normal.png"}}'::jsonb, 10, 0),
  ('emotionista', 'emotionista-silver', 'Silver', 'Play 100 emotes.', '{"2d":{"normal":"https://badges.decentraland.org/assets/emotionista/silver/2d/normal.png"}}'::jsonb, 100, 1),
  ('emotionista', 'emotionista-gold', 'Gold', 'Play 1000 emotes.', '{"2d":{"normal":"https://badges.decentraland.org/assets/emotionista/gold/2d/normal.png"}}'::jsonb, 1000, 2);
-- ==== end section: badges ====

-- ════ schema/50_create.sql ════

-- Catalyst v2: the creator plane (the old catalyrst-create bundle): `builder`,
-- the pre-publish draft store behind builder-api.decentraland.org (creator
-- drafts, uploads, curation state, collection publication and linked
-- publication state, the asset-packs catalogue, newsletter opt-ins), and
-- `camera_reel`, the player photo gallery behind camera-reel-service.
--
-- Two schemas in one file, not one `create` schema: `create` is a reserved
-- word (every statement would quote it), and DATABASE.md names a schema
-- after an authority and lifecycle: a draft is the creator's, ends when the
-- collection is published (the chain is the authority afterwards and the row
-- becomes its local record) and is joined only to other builder rows; a
-- photo is the player's, lives under moderation until its owner or a
-- moderator deletes it, and is joined to nothing here. No statement joins
-- the two, so nothing is gained by sharing a namespace. They share one
-- lifecycle boundary only: both are LOCAL authority behind
-- CREATE_WRITES_ENABLED (default off, 503 with Retry-After, the way
-- src/social/writes.rs gates the social writes), and both are restored with
-- the ONE blob store, where their bytes live.
--
-- Designed from the workload in schema/WORKLOADS.md ("builder and
-- camera_reel"), not copied. Sources:
-- catalyrst/crates/catalyrst-builder/migrations 0001..0007 and
-- catalyrst/crates/catalyrst-camera-reel/migrations 20260609..20260619.
-- Not carried: `uploaded_contents.bytes` (a 20 MB bytea per upload, kept in
-- Postgres for want of a store; v2 stores the bytes in the ONE blob store by
-- hash and keeps the acceptance record here), the camera-reel `url` /
-- `thumbnail_url` columns (a copy of the API_URL setting; the wire URL
-- renders from the hash), the camera-reel per-user `(user_address,
-- is_public)` and review-status indexes and the builder `curation_status`
-- index (no statement reads them; WORKLOADS.md names each), and the
-- catalog.json file tree plus its LRU pull cache (the catalogue is a one-row
-- reference relation; pulled asset-pack files are ordinary blobs).
--
-- Conventions: wallet addresses are lowercase 0x + 40 hex text; every
-- timestamp column is `timestamp without time zone` holding UTC (the pool
-- pins TimeZone=UTC; the old migrations declared timestamptz); the current
-- time is a bind parameter wherever it decides anything (an index
-- predicate, a due-work filter, a cursor, withheld_at) and the
-- updated_at/built_at stamps are timezone('UTC', now()) in the statement;
-- every index names its query in WORKLOADS.md. No FK crosses a schema:
-- camera_reel.images.place_id names an explore.place row by key and
-- reconciles on read (the mirror can drop a place; the photo stays the
-- player's fact); item_contents.hash and images.*_hash name blobs in the
-- ONE store, which is never a foreign-key target.
--
-- Domain agents append ONLY inside the two `-- ==== section: ... ====`
-- blocks at the end (builder, camera_reel), after re-reading this file.

CREATE SCHEMA builder;
CREATE SCHEMA camera_reel;

-- ══ builder ═════════════════════════════════════════════════════════════════

-- ── collections ────────────────────────────────────────────────────────────

-- A creator's collection draft. `eth_address` is the owner (the signed-fetch
-- signer at creation; every write re-checks it). A standard collection is
-- published through collection_publications (contract_address and the
-- collections-v2 urn land on publication); a linked (third-party)
-- collection carries its provider urn from the draft on (third_party_id is
-- the urn minus its last segment) and is published through
-- linked_collection_publications. `publication_pending` freezes the draft
-- and its items while either publication row is in flight; see WORKLOADS.md
-- for the cross-table rule it projects.
CREATE TABLE builder.collections (
    id uuid NOT NULL,
    name text NOT NULL,
    eth_address text NOT NULL,
    salt text,
    contract_address text,
    urn_suffix text,
    third_party_id text,
    is_published boolean NOT NULL DEFAULT false,
    is_approved boolean NOT NULL DEFAULT false,
    publication_pending boolean NOT NULL DEFAULT false,
    created_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    updated_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    CONSTRAINT collections_pkey PRIMARY KEY (id),
    CONSTRAINT collections_contract_address_key UNIQUE (contract_address),
    CONSTRAINT collections_eth_address_lowercase_check CHECK (eth_address = lower(eth_address)),
    CONSTRAINT collections_name_check CHECK (name = btrim(name) AND name <> ''),
    CONSTRAINT collections_salt_len_check CHECK (salt IS NULL OR char_length(salt) <= 128),
    CONSTRAINT collections_third_party_urn_check
        CHECK (third_party_id IS NULL
               OR (urn_suffix IS NOT NULL AND left(urn_suffix, char_length(third_party_id) + 1) = third_party_id || ':')),
    CONSTRAINT collections_contract_published_check CHECK (contract_address IS NULL OR is_published),
    CONSTRAINT collections_pending_not_published_check CHECK (NOT (publication_pending AND is_published))
);

-- "My collections" (GET /v1/collections: eth_address = $1 AND (NOT
-- is_published OR third_party_id IS NOT NULL) ORDER BY updated_at DESC), a
-- per-owner set of tens of rows.
CREATE INDEX collections_owner_idx ON builder.collections (eth_address);

-- ── items ──────────────────────────────────────────────────────────────────

-- An item draft (wearable or emote), owned by `eth_address`, optionally in a
-- collection of the same owner. `data`, `metrics` and `mappings` are the
-- Creator Hub documents read and written as a unit; `contents` is the
-- item_contents relation. `curation_status` is the committee's tristate and
-- `is_approved` its boolean projection (declared below, moved together by
-- the curation PATCH). The seven columns after `is_published` are the
-- explorer's BuilderApiDtos fields (wire compatibility); the copied code
-- writes urn_suffix/blockchain_item_id/local_content_hash on publication
-- and never in_catalyst/total_supply/content_hash/catalyst_content_hash
-- (WORKLOADS.md names them as writer-less).
CREATE TABLE builder.items (
    id uuid NOT NULL,
    collection_id uuid,
    eth_address text NOT NULL,
    name text NOT NULL,
    description text,
    thumbnail text,
    video text,
    type text NOT NULL,
    rarity text,
    price text,
    beneficiary text,
    data jsonb NOT NULL DEFAULT '{}'::jsonb,
    metrics jsonb,
    utility text,
    mappings jsonb,
    urn_suffix text,
    blockchain_item_id text,
    is_published boolean NOT NULL DEFAULT false,
    curation_status text NOT NULL DEFAULT 'pending',
    is_approved boolean NOT NULL DEFAULT false,
    in_catalyst boolean NOT NULL DEFAULT false,
    total_supply bigint NOT NULL DEFAULT 0,
    local_content_hash text,
    content_hash text,
    catalyst_content_hash text,
    created_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    updated_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    CONSTRAINT items_pkey PRIMARY KEY (id),
    CONSTRAINT items_collection_id_fkey FOREIGN KEY (collection_id)
        REFERENCES builder.collections (id) ON DELETE CASCADE,
    CONSTRAINT items_eth_address_lowercase_check CHECK (eth_address = lower(eth_address)),
    CONSTRAINT items_name_check CHECK (name = btrim(name) AND name <> ''),
    CONSTRAINT items_type_check CHECK (type IN ('wearable', 'emote')),
    CONSTRAINT items_rarity_check
        CHECK (rarity IS NULL OR rarity IN ('unique', 'mythic', 'exotic', 'legendary', 'epic', 'rare', 'uncommon', 'common')),
    CONSTRAINT items_price_check CHECK (price IS NULL OR price ~ '^[0-9]{1,78}$'),
    CONSTRAINT items_beneficiary_check CHECK (beneficiary IS NULL OR beneficiary ~ '^0x[0-9a-fA-F]{40}$'),
    CONSTRAINT items_data_object_check CHECK (jsonb_typeof(data) = 'object'),
    CONSTRAINT items_metrics_object_check CHECK (metrics IS NULL OR jsonb_typeof(metrics) = 'object'),
    CONSTRAINT items_curation_status_check CHECK (curation_status IN ('pending', 'approved', 'rejected')),
    CONSTRAINT items_approved_projection_check CHECK (is_approved = (curation_status = 'approved')),
    CONSTRAINT items_published_identity_check
        CHECK (NOT is_published OR (urn_suffix IS NOT NULL AND blockchain_item_id IS NOT NULL)),
    CONSTRAINT items_total_supply_check CHECK (total_supply >= 0)
);

-- The collection's items in creation order: GET /v1/collections/{id}/items
-- (collection_id = $1 [AND name ILIKE] ORDER BY created_at LIMIT/OFFSET with
-- count(*) OVER ()), the publication snapshot (ORDER BY created_at, id), the
-- item_count per listed collection, the curation PATCH (collection_id = $1
-- AND id = ANY($2)) and the `FOR UPDATE` sweep before a publication begins.
CREATE INDEX items_collection_created_idx ON builder.items (collection_id, created_at, id);

-- "My item drafts" (GET /v1/items: eth_address = $1 AND NOT is_published
-- ... ORDER BY updated_at DESC) and the owner probe of every item write.
CREATE INDEX items_owner_updated_idx ON builder.items (eth_address, updated_at DESC);

-- ── uploads ────────────────────────────────────────────────────────────────

-- The record that this node accepted `hash` through POST
-- /v1/items/{id}/files: the bytes are in the ONE blob store (never here),
-- `size` is what the 20 MB per-request cap was checked against,
-- `uploaded_by` the signer whose upload first stored it (a re-upload of the
-- same bytes is ON CONFLICT DO NOTHING). Rows are never deleted: a draft may
-- still name the hash, and the blob store is immutable.
CREATE TABLE builder.uploaded_contents (
    hash text NOT NULL,
    size bigint NOT NULL,
    uploaded_by text NOT NULL,
    created_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    CONSTRAINT uploaded_contents_pkey PRIMARY KEY (hash),
    CONSTRAINT uploaded_contents_size_check CHECK (size >= 0 AND size <= 20971520),
    CONSTRAINT uploaded_contents_uploaded_by_lowercase_check CHECK (uploaded_by = lower(uploaded_by))
);

-- The item's `contents` map (file name -> content hash). The FK to
-- uploaded_contents is the old "upload item content before saving its
-- metadata" rule, declared: a draft can only name a hash this node accepted,
-- so the old snapshot's `missing_content` probe is always false here.
CREATE TABLE builder.item_contents (
    item_id uuid NOT NULL,
    file text NOT NULL,
    hash text NOT NULL,
    CONSTRAINT item_contents_pkey PRIMARY KEY (item_id, file),
    CONSTRAINT item_contents_item_id_fkey FOREIGN KEY (item_id)
        REFERENCES builder.items (id) ON DELETE CASCADE,
    CONSTRAINT item_contents_hash_fkey FOREIGN KEY (hash)
        REFERENCES builder.uploaded_contents (hash),
    CONSTRAINT item_contents_file_check
        CHECK (file <> '' AND char_length(file) <= 256 AND position('\' in file) = 0)
);

-- ── publication ────────────────────────────────────────────────────────────

-- One standard publication attempt per collection: the reviewed Polygon
-- factory parameters (`preparation`, keyed by its content `revision`), the
-- wallet-signing claim, the submitted transaction and the verified outcome.
-- prepared -> signing -> submitted -> published | reverted; a reverted row is
-- replaced in place by the next begin, a cancel deletes an unsubmitted row.
-- The two CHECKs are the state machine: a transaction hash exists from
-- `submitted` on, a contract address only when `published`. v2 never holds a
-- signing key: the client signs; this row records what it submitted.
CREATE TABLE builder.collection_publications (
    collection_id uuid NOT NULL,
    preparation jsonb NOT NULL,
    status text NOT NULL,
    tx_hash text,
    contract_address text,
    created_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    updated_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    CONSTRAINT collection_publications_pkey PRIMARY KEY (collection_id),
    CONSTRAINT collection_publications_collection_id_fkey FOREIGN KEY (collection_id)
        REFERENCES builder.collections (id) ON DELETE CASCADE,
    CONSTRAINT collection_publications_tx_hash_key UNIQUE (tx_hash),
    CONSTRAINT collection_publications_contract_address_key UNIQUE (contract_address),
    CONSTRAINT collection_publications_status_check
        CHECK (status IN ('prepared', 'signing', 'submitted', 'published', 'reverted')),
    CONSTRAINT collection_publications_tx_hash_state_check
        CHECK ((status IN ('prepared', 'signing')) = (tx_hash IS NULL)),
    CONSTRAINT collection_publications_contract_state_check
        CHECK ((status = 'published') = (contract_address IS NOT NULL)),
    CONSTRAINT collection_publications_preparation_check
        CHECK (jsonb_typeof(preparation) = 'object' AND preparation ? 'revision')
);

-- One linked (third-party) publication attempt per collection: the frozen
-- draft `snapshot` and its `preparation`, the stable nonce (`salt`) the
-- provider-slot cheque is signed over, the owner-signed `cheque`, and the
-- Foundation submission receipt (`forum_url`).
-- prepared -> signing -> authorized -> submitted; cancel only while
-- prepared. The CHECKs: a cheque exists exactly from `authorized` on, a forum
-- url only once submitted.
CREATE TABLE builder.linked_collection_publications (
    collection_id uuid NOT NULL,
    preparation jsonb NOT NULL,
    snapshot jsonb NOT NULL,
    salt text NOT NULL,
    status text NOT NULL,
    cheque jsonb,
    forum_url text,
    created_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    updated_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    CONSTRAINT linked_collection_publications_pkey PRIMARY KEY (collection_id),
    CONSTRAINT linked_collection_publications_collection_id_fkey FOREIGN KEY (collection_id)
        REFERENCES builder.collections (id) ON DELETE CASCADE,
    CONSTRAINT linked_collection_publications_salt_key UNIQUE (salt),
    CONSTRAINT linked_collection_publications_salt_check CHECK (salt ~ '^0x[0-9a-f]{64}$'),
    CONSTRAINT linked_collection_publications_status_check
        CHECK (status IN ('prepared', 'signing', 'authorized', 'submitted')),
    CONSTRAINT linked_collection_publications_cheque_state_check
        CHECK ((status IN ('prepared', 'signing')) = (cheque IS NULL)),
    CONSTRAINT linked_collection_publications_forum_state_check
        CHECK (forum_url IS NULL OR status = 'submitted'),
    CONSTRAINT linked_collection_publications_preparation_check
        CHECK (jsonb_typeof(preparation) = 'object' AND preparation ? 'revision')
);

-- ── asset packs ────────────────────────────────────────────────────────────

-- Reference data: the asset-packs catalogue GET /v1/assetPacks serves whole
-- (the `data` array of the builder-server envelope), written by the operator
-- command that builds it from the packs tree (the old `catalyrst-builder
-- catalog`), never by a request. The hash -> file-name map GET
-- /contents/{hash} needs for a content type derives from `packs` at load;
-- the files themselves are blobs in the ONE store, pulled through from the
-- content bucket on a miss and verified against their CID.
CREATE TABLE builder.asset_catalog (
    id integer NOT NULL DEFAULT 1,
    packs jsonb NOT NULL,
    source text NOT NULL,
    built_at timestamp without time zone NOT NULL,
    CONSTRAINT asset_catalog_pkey PRIMARY KEY (id),
    CONSTRAINT asset_catalog_singleton_check CHECK (id = 1),
    CONSTRAINT asset_catalog_packs_check CHECK (jsonb_typeof(packs) = 'array')
);

-- ── newsletter ─────────────────────────────────────────────────────────────

-- POST /v1/newsletter: the durable opt-in (the forward to the newsletter
-- SaaS is best-effort and stateless). Lowercased, trimmed, validated by the
-- write path; a repeat updates `source`.
CREATE TABLE builder.newsletter_subscriptions (
    email text NOT NULL,
    source text NOT NULL,
    created_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    CONSTRAINT newsletter_subscriptions_pkey PRIMARY KEY (email),
    CONSTRAINT newsletter_subscriptions_email_check CHECK (email = lower(btrim(email)) AND email <> '')
);

-- ══ camera_reel ═════════════════════════════════════════════════════════════

-- A player's photo: the full image and its 640x360 thumbnail are blobs in
-- the ONE store named by `image_hash`/`thumbnail_hash` (the wire's `url` /
-- `thumbnailUrl` render as `<API_URL>/api/images/<hash>`); `metadata` is the
-- client document (`Metadata`: userName, userAddress, dateTime, realm,
-- scene, visiblePeople, placeId) returned as a unit; `place_id` is its
-- placeId projected for the place galleries and names an explore.place row
-- by key (no FK: mirrored catalogue, different lifecycle). `user_address` is
-- the verified signer (the write path refuses a metadata.userAddress that is
-- not the signer; the CHECK holds the pair together). `review_status` is the
-- moderator's: `rejected` hides the row and its blobs from every read.
CREATE TABLE camera_reel.images (
    id uuid NOT NULL,
    user_address text NOT NULL,
    image_hash text NOT NULL,
    thumbnail_hash text NOT NULL,
    is_public boolean NOT NULL DEFAULT false,
    review_status text NOT NULL DEFAULT 'ok',
    metadata jsonb NOT NULL,
    place_id text GENERATED ALWAYS AS (metadata->>'placeId') STORED,
    created_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    CONSTRAINT images_pkey PRIMARY KEY (id),
    CONSTRAINT images_user_address_lowercase_check CHECK (user_address = lower(user_address)),
    CONSTRAINT images_metadata_object_check CHECK (jsonb_typeof(metadata) = 'object'),
    CONSTRAINT images_metadata_owner_check CHECK (lower(metadata->>'userAddress') = user_address),
    CONSTRAINT images_review_status_check CHECK (review_status IN ('ok', 'flagged', 'rejected'))
);

-- The owner's gallery and count (GET /api/users/{address}[/images]:
-- user_address = $1 [AND is_public AND review_status <> 'rejected' for a
-- viewer who is not the owner] ORDER BY created_at DESC, id DESC), the
-- per-user limit count inside the upload insert, and the keyset tie-breaker
-- of the new page shape. One index instead of the old (user_address) +
-- (user_address, is_public): the per-user set is at most MAX_IMAGES_PER_USER.
CREATE INDEX images_user_created_idx ON camera_reel.images (user_address, created_at DESC, id DESC);

-- The place galleries (GET /api/places/{id}/images, POST /api/places/images:
-- place_id = $1 / = ANY($1) ORDER BY created_at DESC, id DESC), which only
-- ever read public, non-rejected rows, so the predicate is the index's.
CREATE INDEX images_place_public_created_idx ON camera_reel.images (place_id, created_at DESC, id DESC)
    WHERE is_public AND review_status <> 'rejected';

-- GET /api/images/{hash}: a blob is served only while a non-rejected row
-- names it as image or thumbnail (image_hash = $1 OR thumbnail_hash = $1,
-- bitmap OR of the two).
CREATE INDEX images_image_hash_idx ON camera_reel.images (image_hash);
CREATE INDEX images_thumbnail_hash_idx ON camera_reel.images (thumbnail_hash);

-- ══ Domain-agent sections: append inside the matching block only ═══════════

-- ==== section: builder ====
-- Left to the builder agent: nothing is expected here. The on-chain reads
-- (/v1/{address}/collections, /v1/{address}/items, the curation console's
-- committee and under-review listings) read the marketplace subgraph readers
-- (src/marketplace/graph.rs), not a squid copy, so they add no table; the
-- Polygon receipt verification of a submitted publication reads the chain
-- and writes collection_publications above.
-- ==== end section: builder ====

-- ==== section: camera_reel ====
-- World-name galleries (`name.dcl.eth`) resolve to place ids through
-- explore.place_indexed (lower(world_name) = lower($1)), the old HTTP hop to
-- the places API made a local read; no cache table.
--
-- A blob this node withholds everywhere because the camera reel's moderation
-- said so: the image and thumbnail hashes of a rejected row (`reason` =
-- 'rejected'; released when the moderator sets the row back to ok/flagged)
-- and of a deleted row (`reason` = 'deleted'; the old service unlinked those
-- bytes from its private tree, the ONE store keeps them). The reel's own
-- route already answers 404 from `images.review_status`; this relation is
-- what makes the verdict hold on the content server's `/contents/{hash}`
-- (root and worlds nest) and the builder's: the process loads it into the
-- shared in-memory `content::Denylist` at boot and on every write. A hash is
-- withheld only while no non-rejected `images` row and no
-- `content.content_files` / `builder.uploaded_contents` row names it (an
-- owner's delete must not hide a deployed file that happens to be the same
-- bytes); world scene files are named inside `worlds.world_scenes.entity`
-- and are not checked (WORKLOADS.md, "Blob lifecycle"). `image_id` is the
-- row whose verdict wrote it, kept as a fact (no FK: a deleted row is gone).
CREATE TABLE camera_reel.withheld_blobs (
    hash text NOT NULL,
    image_id uuid NOT NULL,
    reason text NOT NULL,
    withheld_at timestamp without time zone NOT NULL,
    CONSTRAINT withheld_blobs_pkey PRIMARY KEY (hash),
    CONSTRAINT withheld_blobs_reason_check CHECK (reason IN ('rejected', 'deleted'))
);
-- ==== end section: camera_reel ====

-- ════ schema/60_ops.sql ════

-- Catalyst v2: the operator plane and the remaining services: telemetry (the
-- Sentry/Segment sinks and the issues/flags/experiments console), credits
-- (the Marketplace Credits ledger, Stripe purchases, carts, checkouts and
-- authorizations), governance (the mirror of governance.decentraland.org),
-- quests (the SDK7 quests service) and ops (the ONE operator audit).
--
-- Designed from the workload in schema/WORKLOADS.md ("telemetry, credits,
-- governance, quests and ops"), not copied: the old crates' four databases
-- (`catalyrst` search_path=telemetry, `credits`, `governance`, `quests`)
-- collapse into four schemas of `catalyst`; every merge, every dropped
-- index and every table left NOT created is recorded there with the query
-- behind each index below. Sources: catalyrst/crates/catalyrst-telemetry/
-- migrations 0001..0011, catalyrst-credits/migrations 0001..0021,
-- catalyrst-governance/migrations 0001..0002, catalyrst-quests/migrations
-- 0001 (itself the fold of decentraland/quests 20230113..20230913) and the
-- old server's admin_audit (catalyrst-server/src/admin/audit.rs).
--
-- Conventions, as in the other files: wallet addresses are lowercase 0x + 40
-- hex text where this node writes them (CHECKed) and kept as the upstream
-- sent them where they are mirrored (governance); every timestamp is
-- `timestamp without time zone` holding UTC (the pool pins TimeZone=UTC; the
-- old tables were timestamptz); the current time is a bind parameter in
-- every due, expiry, window and purge predicate, never in an index
-- predicate; credits amounts are numeric (never a float in SQL or Rust), wei
-- is numeric(78,0). No FK crosses a schema: `credits.ledger.admin_audit_id`
-- names an `ops.admin_audit` row and `credits.fulfillment_outbox.external_ref`
-- an economy broker purchase, reconciled on read.
--
-- Authority: telemetry.events is an INGEST SINK (not user authority; its own
-- TELEMETRY_INGEST_ENABLED switch plus the operator toggle in
-- telemetry.ingest_settings); the credits ledger, checkouts, authorizations,
-- quests progress, the telemetry console writes and the operator audit are
-- LOCAL authority behind OPS_WRITES_ENABLED (default false; 503 envelope like
-- src/social/writes.rs); governance is MIRRORED (upsert by upstream id, the
-- raw document kept as the wire).
--
-- Domain agents append ONLY inside the `-- ==== section: ... ====` blocks at
-- the end, after re-reading this file.

CREATE SCHEMA telemetry;
CREATE SCHEMA credits;
CREATE SCHEMA governance;
CREATE SCHEMA quests;
CREATE SCHEMA ops;

-- ══════════════════════════════════════════════════════════════════════════
-- telemetry
-- ══════════════════════════════════════════════════════════════════════════

-- One row per Sentry envelope item / store event / Segment call, appended by
-- the bounded ingest writer (256-row UNNEST inserts, a 10,000-row queue, a
-- per-project daily quota); never updated; deleted only by the retention
-- job and the operator's purge/bulk-delete. `body` is the payload as sent
-- (the sinks reserialize nothing they are asked to keep; a decoded request
-- is at most 2 MiB, the route body cap, and the no-item envelope fallback
-- keeps a 4 KiB prefix, so no row is wider than that). `fingerprint` is
-- the issues grouping key: the first line of the title expression with
-- markup, timestamps, URLs and numbers normalised, STORED so the issues view
-- groups on a column and issue_state / issue_merges key on the same value
-- (migration 0002/0003 of the old service; the expression inside left() is
-- duplicated in the dashboard's TITLE1 and must stay identical). It is cut
-- at 512 characters (at most 2,048 bytes) so events_fingerprint_received_idx
-- can always hold it: an event whose first title line ran past the btree's
-- 8,191-byte entry limit was refused by the insert (and lost) before the
-- cut; TITLE1 stays uncut for display, the drill-down compares the column.
-- `invalid_reason` is the ingest-side contract verdict for a Segment event
-- whose shape defeats telemetry-contract.json: stored, never rejected.
CREATE TABLE telemetry.events (
    id bigint GENERATED ALWAYS AS IDENTITY,
    received_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    source text NOT NULL,
    project text NOT NULL DEFAULT '',
    event_kind text NOT NULL DEFAULT '',
    body jsonb NOT NULL,
    invalid_reason text,
    fingerprint text GENERATED ALWAYS AS (
        left(regexp_replace(
          regexp_replace(
            regexp_replace(
              regexp_replace(
                split_part(
                  COALESCE(
                    NULLIF(body->>'message',''),
                    NULLIF(body#>>'{logentry,message}',''),
                    NULLIF(body#>>'{exception,values,0,type}','')
                      || COALESCE(': ' || (body#>>'{exception,values,0,value}'), ''),
                    NULLIF(body->>'transaction',''),
                    NULLIF(body->>'event',''),
                    CASE WHEN event_kind = 'session'
                         THEN 'session (' || COALESCE(NULLIF(body->>'status',''),
                                CASE WHEN (body->>'init')::boolean THEN 'started' ELSE 'update' END) || ')'
                    END,
                    CASE WHEN body->>'userId' IS NOT NULL
                         THEN 'identify ' || (body->>'userId')
                    END,
                    '(' || event_kind || ')'),
                  E'\n', 1),
                '<[^>]*>', '', 'g'),
              '^[0-9]{2}:[0-9]{2}:[0-9]{2}\.[0-9]+ - ', ''),
            'https?://[^ ]+', 'URL', 'g'),
          '-?[0-9]+', 'N', 'g'), 512)
    ) STORED,
    CONSTRAINT events_pkey PRIMARY KEY (id),
    CONSTRAINT events_source_check CHECK (source IN ('sentry', 'segment'))
);

-- The window every console read starts from (received_at > $now - $hours,
-- newest first), the export's ORDER BY, the bulk `before` filter and the
-- retention job's `received_at < $cutoff ORDER BY received_at LIMIT $batch`.
CREATE INDEX events_received_at_idx ON telemetry.events (received_at DESC);
-- The source facet (prefix), the sessions health view (source = 'sentry' AND
-- event_kind = 'session'), the client_error retention rule (source =
-- 'segment' AND event_kind = 'track' AND body->>'event' = 'client_error')
-- and the per-source stats rollup, each inside the window.
CREATE INDEX events_source_kind_received_idx ON telemetry.events (source, event_kind, received_at DESC);
-- The issue drill-down (fingerprint = $fp within the window), stats?fingerprint=
-- and the bulk delete/export by fingerprint. The issues GROUP BY fingerprint
-- over a window is a hash aggregate over the window scan, not this index.
CREATE INDEX events_fingerprint_received_idx ON telemetry.events (fingerprint, received_at DESC);
-- The level facet (body->>'level' = $level) and the by_level rollup.
CREATE INDEX events_level_received_idx ON telemetry.events ((body->>'level'), received_at DESC);
-- GET /dash/experiments (LIST_SQL): the aggregate over every Segment event
-- carrying properties.exp_key, grouped by (exp_key, variant, event); the
-- predicate is the query's verbatim so the bitmap scan visits only the pages
-- holding qualifying rows (old node, 2026-09-17: 85k of 933k rows, 9k of
-- 32k heap pages, 144 ms -> memoised + indexed).
CREATE INDEX events_experiments_idx ON telemetry.events (
    source,
    (body->'properties'->>'exp_key'),
    (body->'properties'->>'variant'),
    (body->>'event')
) WHERE source = 'segment' AND body->'properties'->>'exp_key' IS NOT NULL;
-- The creator hub's scene-error feed (sites/packages/data/.../scene-errors.server.ts
-- reads this table directly): Scene Error events of one Genesis base parcel
-- or one world, newest first.
CREATE INDEX events_scene_errors_genesis_idx ON telemetry.events ((body->'properties'->>'scene_base'), received_at DESC)
    WHERE source = 'segment' AND body->>'event' = 'Scene Error';
CREATE INDEX events_scene_errors_world_idx ON telemetry.events ((lower(body->'properties'->>'scene_world')), received_at DESC)
    WHERE source = 'segment' AND body->>'event' = 'Scene Error';

-- Per-issue workflow state keyed by the grouping fingerprint; an absent row
-- is 'unresolved'. A resolved issue whose newest event is later than
-- updated_at reads as regressed (unresolved) in the issues view, Sentry's
-- auto-reopen; the row itself is only changed by the console.
CREATE TABLE telemetry.issue_state (
    fingerprint text NOT NULL,
    status text NOT NULL DEFAULT 'unresolved',
    assignee text,
    note text,
    updated_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    CONSTRAINT issue_state_pkey PRIMARY KEY (fingerprint),
    CONSTRAINT issue_state_status_check CHECK (status IN ('unresolved', 'resolved', 'ignored'))
);

-- Regroup: a source fingerprint folded into a canonical one; the console's
-- issues view resolves through it, the merge is also an ops.admin_audit row.
CREATE TABLE telemetry.issue_merges (
    source_fingerprint text NOT NULL,
    canonical_fingerprint text NOT NULL,
    merged_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    CONSTRAINT issue_merges_pkey PRIMARY KEY (source_fingerprint),
    CONSTRAINT issue_merges_not_self_check CHECK (source_fingerprint <> canonical_fingerprint)
);
-- The reverse lookup (every source folded into one canonical) on the issue page.
CREATE INDEX issue_merges_canonical_idx ON telemetry.issue_merges (canonical_fingerprint);

-- Operator lifecycle state of a release string.
CREATE TABLE telemetry.release_state (
    release text NOT NULL,
    state text NOT NULL DEFAULT 'active',
    note text,
    updated_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    CONSTRAINT release_state_pkey PRIMARY KEY (release),
    CONSTRAINT release_state_state_check CHECK (state IN ('active', 'archived', 'broken'))
);

-- The operator's runtime ingest kill switch (the old admin_settings
-- key/value row 'ingest_enabled', now one typed row). Read once at boot and
-- on every toggle into an in-memory atomic; the hot insert path never reads
-- it. TELEMETRY_INGEST_ENABLED (deploy posture) gates the routes with a 503
-- in front of this; with it on and this row false the sinks answer success
-- and store nothing, as the old service did.
CREATE TABLE telemetry.ingest_settings (
    id integer NOT NULL,
    enabled boolean NOT NULL DEFAULT true,
    updated_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    CONSTRAINT ingest_settings_pkey PRIMARY KEY (id),
    CONSTRAINT ingest_settings_singleton_check CHECK (id = 1)
);
INSERT INTO telemetry.ingest_settings (id, enabled) VALUES (1, true);

-- Per-project (write key / Sentry project) daily accept limit. A project
-- with no row takes the `*` row, else TELEMETRY_DEFAULT_DAILY_LIMIT (50,000;
-- the old "no row means unlimited" is gone: the project is the client's
-- URL segment or write key). Enforced O(1) per event against in-memory
-- per-UTC-day counters seeded from this table; the counters are process
-- state (a restart resets the day's count, accepted as the old service
-- did). The process-wide rows and bytes ceilings (TELEMETRY_MAX_ROWS_PER_
-- MINUTE, TELEMETRY_MAX_BYTES_PER_MINUTE) sit behind the quota and are not
-- a table.
CREATE TABLE telemetry.project_quotas (
    project text NOT NULL,
    daily_limit bigint NOT NULL,
    updated_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    CONSTRAINT project_quotas_pkey PRIMARY KEY (project),
    CONSTRAINT project_quotas_limit_check CHECK (daily_limit >= 0)
);

-- The sites A/B layer's per-experiment operator overrides (kill switch,
-- forced variant, free flags), read by GET /dash/experiments?key=.
CREATE TABLE telemetry.experiment_overrides (
    exp_key text NOT NULL,
    killed boolean NOT NULL DEFAULT false,
    forced_variant text,
    flags jsonb NOT NULL DEFAULT '{}'::jsonb,
    updated_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    CONSTRAINT experiment_overrides_pkey PRIMARY KEY (exp_key)
);

-- Feature-flag overrides merged over the upstream flags document by
-- /dash/flags and the public /flags page: on / off / forced (+ variant).
CREATE TABLE telemetry.flag_overrides (
    flag text NOT NULL,
    state text NOT NULL DEFAULT 'on',
    forced_variant text,
    updated_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    CONSTRAINT flag_overrides_pkey PRIMARY KEY (flag),
    CONSTRAINT flag_overrides_state_check CHECK (state IN ('on', 'off', 'forced'))
);

-- Audience groups: an explicit member list or a deterministic percentage
-- rollout (FNV-1a of "<group>:<user_key>", computed in Rust). A matching
-- group target beats the global override, which beats the upstream config;
-- priority (highest wins, then name) breaks ties between groups.
CREATE TABLE telemetry.flag_groups (
    name text NOT NULL,
    description text NOT NULL DEFAULT '',
    members text[] NOT NULL DEFAULT '{}',
    rollout_pct integer NOT NULL DEFAULT 0,
    priority integer NOT NULL DEFAULT 0,
    updated_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    CONSTRAINT flag_groups_pkey PRIMARY KEY (name),
    CONSTRAINT flag_groups_rollout_check CHECK (rollout_pct BETWEEN 0 AND 100)
);

CREATE TABLE telemetry.flag_group_targets (
    flag text NOT NULL,
    group_name text NOT NULL,
    state text NOT NULL DEFAULT 'on',
    forced_variant text,
    updated_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    CONSTRAINT flag_group_targets_pkey PRIMARY KEY (flag, group_name),
    CONSTRAINT flag_group_targets_group_fkey FOREIGN KEY (group_name)
        REFERENCES telemetry.flag_groups (name) ON DELETE CASCADE,
    CONSTRAINT flag_group_targets_state_check CHECK (state IN ('on', 'off', 'forced'))
);

CREATE TABLE telemetry.experiment_group_targets (
    exp_key text NOT NULL,
    group_name text NOT NULL,
    killed boolean NOT NULL DEFAULT false,
    forced_variant text,
    flags jsonb NOT NULL DEFAULT '{}'::jsonb,
    updated_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    CONSTRAINT experiment_group_targets_pkey PRIMARY KEY (exp_key, group_name),
    CONSTRAINT experiment_group_targets_group_fkey FOREIGN KEY (group_name)
        REFERENCES telemetry.flag_groups (name) ON DELETE CASCADE
);

-- A display tag: which product area a flag or experiment is filed under,
-- apart from the override tables so filing never overrides.
CREATE TABLE telemetry.product_areas (
    kind text NOT NULL,
    name text NOT NULL,
    area text NOT NULL,
    updated_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    CONSTRAINT product_areas_pkey PRIMARY KEY (kind, name),
    CONSTRAINT product_areas_kind_check CHECK (kind IN ('flag', 'experiment'))
);

-- ══════════════════════════════════════════════════════════════════════════
-- credits
-- ══════════════════════════════════════════════════════════════════════════

-- One row per wallet that touched the program: the old user_program
-- (enrolment) and user_credits (balance) rows shared the key and always
-- moved together, so they are one row. `available` is the TOTAL spendable
-- balance and `earned_available` the slice that came from claims; both are
-- projections of `ledger` (signed sum per bucket) written in the SAME
-- transaction as the ledger row, under `SELECT ... FOR UPDATE` on this row
-- (the wallet lock every spend, grant, refund, revoke and authorization
-- takes); the reconciler asserts the equality and records any drift as an
-- explicit adjustment ledger row rather than tolerating it. Earned credits
-- no longer expire (old migration 0017), so no expiry column.
CREATE TABLE credits.wallets (
    address text NOT NULL,
    has_started_program boolean NOT NULL DEFAULT true,
    available numeric NOT NULL DEFAULT 0,
    earned_available numeric NOT NULL DEFAULT 0,
    is_blocked_for_claiming boolean NOT NULL DEFAULT false,
    created_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    updated_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    CONSTRAINT wallets_pkey PRIMARY KEY (address),
    CONSTRAINT wallets_address_check CHECK (address ~ '^0x[0-9a-f]{40}$'),
    CONSTRAINT wallets_available_check CHECK (available >= 0),
    CONSTRAINT wallets_earned_bound_check CHECK (earned_available >= 0 AND earned_available <= available)
);

-- The append-only money fact: every movement of a wallet's balance, tagged
-- with the bucket it moved. credit = grant, claim, purchase, refund; debit =
-- spend, consume, expire (the sign is the kind's, `amount` is never
-- negative). `tx_ref` is the cause (a checkout id, a Stripe event id, an
-- authorization id, an operator reason key); `admin_audit_id` names the
-- ops.admin_audit row of an operator grant/revoke/refund (no FK across the
-- schema; the audit row is written first in the same transaction).
CREATE TABLE credits.ledger (
    id bigint GENERATED ALWAYS AS IDENTITY,
    address text NOT NULL,
    kind text NOT NULL,
    bucket text NOT NULL DEFAULT 'paid',
    amount numeric NOT NULL,
    tx_ref text,
    captcha_ok boolean NOT NULL DEFAULT false,
    admin_audit_id bigint,
    created_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    CONSTRAINT ledger_pkey PRIMARY KEY (id),
    CONSTRAINT ledger_address_check CHECK (address ~ '^0x[0-9a-f]{40}$'),
    CONSTRAINT ledger_kind_check CHECK (kind IN ('grant', 'claim', 'expire', 'consume', 'spend', 'purchase', 'refund')),
    CONSTRAINT ledger_bucket_check CHECK (bucket IN ('earned', 'paid')),
    CONSTRAINT ledger_amount_check CHECK (amount >= 0)
);

-- The wallet's ledger page (/admin/credits/ledger?address= ORDER BY id DESC
-- LIMIT/OFFSET) and the reconciler's per-wallet signed sums.
CREATE INDEX ledger_address_id_idx ON credits.ledger (address, id DESC);
-- The refund window: SUM(spend) - SUM(refund) of one tx_ref for one wallet
-- (refund_in_tx clamps a refund to what that cause actually debited).
CREATE INDEX ledger_tx_ref_idx ON credits.ledger (tx_ref, address) WHERE tx_ref IS NOT NULL;

-- Idempotent money writes: the three old tables (grant / spend / refund
-- idempotency) had the same columns and the same rule -- the first write
-- for a key records its result inside the write's transaction, a replay
-- returns that result without moving money -- so they are one relation
-- keyed by (scope, key). `amount` is what was REQUESTED (the replay guard
-- compares against it), `applied` what actually moved (a clamped refund),
-- `available` the balance the original answer carried.
CREATE TABLE credits.idempotency (
    scope text NOT NULL,
    idempotency_key text NOT NULL,
    address text NOT NULL,
    amount numeric NOT NULL,
    applied numeric,
    available numeric NOT NULL,
    tx_ref text,
    actor text,
    created_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    CONSTRAINT idempotency_pkey PRIMARY KEY (scope, idempotency_key),
    CONSTRAINT idempotency_scope_check CHECK (scope IN ('grant', 'spend', 'refund'))
);

-- Buyable Credit packs; `credits` is the numeric Credits granted on
-- purchase, `price_cents` the fiat charge in minor units. Product
-- configuration: the four tiers at the 0.10 USD/Credit peg are seeded here
-- (the old migration 0014) and edited by /admin/credits/packs.
CREATE TABLE credits.packs (
    sku text NOT NULL,
    title text NOT NULL,
    credits numeric NOT NULL,
    price_cents bigint NOT NULL,
    currency text NOT NULL,
    active boolean NOT NULL DEFAULT true,
    sort_order integer NOT NULL DEFAULT 0,
    created_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    CONSTRAINT packs_pkey PRIMARY KEY (sku),
    CONSTRAINT packs_credits_check CHECK (credits > 0),
    CONSTRAINT packs_price_check CHECK (price_cents >= 0)
);
INSERT INTO credits.packs (sku, title, credits, price_cents, currency, active, sort_order) VALUES
    ('credits_usd_5',  'Starter',  50,   500, 'usd', true, 1),
    ('credits_usd_15', 'Plus',    150,  1500, 'usd', true, 2),
    ('credits_usd_25', 'Pro',     250,  2500, 'usd', true, 3),
    ('credits_usd_50', 'Max',     500,  5000, 'usd', true, 4);

-- Stripe delivery de-duplication: one row per event id, `processed_at`
-- stamped only after every side effect committed, so a delivery recorded
-- but not processed is re-driven on Stripe's retry (the effects are
-- idempotent through `idempotency`) and a crash between the two never drops
-- a paid customer's Credits. The webhook signature (Stripe-Signature, HMAC
-- over t.body, 5 minute tolerance) is verified before this row exists.
CREATE TABLE credits.stripe_events (
    event_id text NOT NULL,
    type text NOT NULL,
    payload jsonb,
    received_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    processed_at timestamp without time zone,
    CONSTRAINT stripe_events_pkey PRIMARY KEY (event_id)
);

-- One row per pack purchase or Credits order: a PaymentIntent (`method` =
-- 'card', the intent flow), a Checkout Session (`order_id` +
-- `stripe_checkout_session`, the explorer's order flow) or a MANA top-up
-- (`method` = 'mana', no Stripe columns). `credits` and `amount_cents` are
-- captured at creation and the webhook grants from THIS row, never from the
-- event's metadata. `refunded_cents` is the cumulative fiat clawed back
-- (charge.refunded carries a cumulative figure; each event refunds the
-- delta), `revoked_credits` the Credits removed for it, structurally bounded
-- by the grant.
CREATE TABLE credits.purchases (
    id bigint GENERATED ALWAYS AS IDENTITY,
    address text NOT NULL,
    sku text NOT NULL,
    credits numeric NOT NULL,
    amount_cents bigint NOT NULL,
    currency text NOT NULL,
    method text NOT NULL DEFAULT 'card',
    status text NOT NULL DEFAULT 'pending',
    order_id text,
    stripe_payment_intent text,
    stripe_checkout_session text,
    stripe_event_id text,
    refunded_cents bigint NOT NULL DEFAULT 0,
    revoked_credits numeric NOT NULL DEFAULT 0,
    failure_reason text,
    created_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    updated_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    CONSTRAINT purchases_pkey PRIMARY KEY (id),
    CONSTRAINT purchases_address_check CHECK (address ~ '^0x[0-9a-f]{40}$'),
    CONSTRAINT purchases_status_check
        CHECK (status IN ('pending', 'paid', 'refunded', 'disputed', 'failed', 'abandoned')),
    CONSTRAINT purchases_refunded_bound_check CHECK (refunded_cents >= 0 AND refunded_cents <= amount_cents),
    CONSTRAINT purchases_revoked_bound_check CHECK (revoked_credits >= 0 AND revoked_credits <= credits)
);

-- The webhook's lookups: one purchase per PaymentIntent / Checkout Session /
-- order id (NULL for the flows that have none).
CREATE UNIQUE INDEX purchases_payment_intent_uidx ON credits.purchases (stripe_payment_intent)
    WHERE stripe_payment_intent IS NOT NULL;
CREATE UNIQUE INDEX purchases_checkout_session_uidx ON credits.purchases (stripe_checkout_session)
    WHERE stripe_checkout_session IS NOT NULL;
CREATE UNIQUE INDEX purchases_order_id_uidx ON credits.purchases (order_id) WHERE order_id IS NOT NULL;
-- The reconciler's paid-vs-granted sums per wallet and /admin/credits/purchases?address=.
CREATE INDEX purchases_address_idx ON credits.purchases (address, id DESC);

-- One open cart per wallet; the address is the natural key.
CREATE TABLE credits.carts (
    id bigint GENERATED ALWAYS AS IDENTITY,
    address text NOT NULL,
    created_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    updated_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    CONSTRAINT carts_pkey PRIMARY KEY (id),
    CONSTRAINT carts_address_key UNIQUE (address),
    CONSTRAINT carts_address_check CHECK (address ~ '^0x[0-9a-f]{40}$')
);

-- A cart line, identified by (collection, item_id): the item id alone is a
-- per-collection blockchain index (nearly every collection has an item "0"),
-- which the old UNIQUE (cart_id, item_id) got wrong. `unit_price_credits`
-- is the server price captured at add time and re-computed at checkout; the
-- client's price is never read.
CREATE TABLE credits.cart_items (
    id bigint GENERATED ALWAYS AS IDENTITY,
    cart_id bigint NOT NULL,
    collection text NOT NULL,
    item_id text NOT NULL,
    urn text NOT NULL,
    category text NOT NULL,
    qty integer NOT NULL DEFAULT 1,
    unit_price_credits numeric NOT NULL,
    added_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    CONSTRAINT cart_items_pkey PRIMARY KEY (id),
    CONSTRAINT cart_items_cart_fkey FOREIGN KEY (cart_id) REFERENCES credits.carts (id) ON DELETE CASCADE,
    CONSTRAINT cart_items_line_key UNIQUE (cart_id, collection, item_id),
    CONSTRAINT cart_items_collection_check CHECK (collection ~ '^0x[0-9a-f]{40}$'),
    CONSTRAINT cart_items_qty_check CHECK (qty > 0),
    CONSTRAINT cart_items_price_check CHECK (unit_price_credits >= 0)
);

-- One row per checkout attempt, keyed by the client's Idempotency-Key so a
-- retried POST /checkout re-finds its row instead of debiting twice.
-- `total_credits` is the server-priced total debited ONCE (reject-if-short,
-- never a clamp) as the ledger spend row `ledger_id`; the outbox below then
-- fulfils each unit asynchronously. 'reversing' freezes a checkout whose
-- line failed terminally while the undelivered portion is refunded; it
-- resolves to 'reversed' (nothing delivered) or 'failed' (partial).
CREATE TABLE credits.checkouts (
    id bigint GENERATED ALWAYS AS IDENTITY,
    idempotency_key text NOT NULL,
    address text NOT NULL,
    total_credits numeric NOT NULL DEFAULT 0,
    status text NOT NULL DEFAULT 'reserving',
    ledger_id bigint,
    created_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    updated_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    CONSTRAINT checkouts_pkey PRIMARY KEY (id),
    CONSTRAINT checkouts_idempotency_key_key UNIQUE (idempotency_key),
    CONSTRAINT checkouts_address_check CHECK (address ~ '^0x[0-9a-f]{40}$'),
    CONSTRAINT checkouts_status_check
        CHECK (status IN ('reserving', 'debited', 'fulfilling', 'fulfilled', 'failed', 'reversing', 'reversed')),
    CONSTRAINT checkouts_total_check CHECK (total_credits >= 0)
);

-- /admin/credits/checkouts?address= and the wallet's own checkout history, newest first.
CREATE INDEX checkouts_address_id_idx ON credits.checkouts (address, id DESC);

-- The fulfilment outbox: one row per charged UNIT (a line of qty N is N
-- rows, one broker buy each), drained by the `credits-fulfillment` job.
-- `mode` selects the economy broker path; `token_id` + `basis_wei` pin a
-- secondary listing (MarketplaceV2 binds both on-chain, so fulfilment can
-- never pay a different amount than the buyer was charged for), `trade_id`
-- pins a v3 signed trade (re-fetched from marketplace.trades and checked
-- still open at fulfilment); `external_ref` is the broker's purchase
-- reference once confirmed. SUM(unit_price_credits) over a checkout's rows
-- equals its total_credits by construction.
CREATE TABLE credits.fulfillment_outbox (
    id bigint GENERATED ALWAYS AS IDENTITY,
    checkout_id bigint NOT NULL,
    item_id text NOT NULL,
    collection text NOT NULL,
    urn text NOT NULL,
    token_id text,
    trade_id text,
    basis_wei numeric(78,0),
    unit_price_credits numeric NOT NULL,
    mode text NOT NULL,
    status text NOT NULL DEFAULT 'pending',
    attempts integer NOT NULL DEFAULT 0,
    last_error text,
    external_ref text,
    created_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    updated_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    CONSTRAINT fulfillment_outbox_pkey PRIMARY KEY (id),
    CONSTRAINT fulfillment_outbox_checkout_fkey FOREIGN KEY (checkout_id) REFERENCES credits.checkouts (id),
    CONSTRAINT fulfillment_outbox_collection_check CHECK (collection ~ '^0x[0-9a-f]{40}$'),
    CONSTRAINT fulfillment_outbox_mode_check CHECK (mode IN ('primary', 'secondary', 'trade')),
    CONSTRAINT fulfillment_outbox_status_check CHECK (status IN ('pending', 'sent', 'confirmed', 'failed')),
    CONSTRAINT fulfillment_outbox_attempts_check CHECK (attempts >= 0)
);

-- The worker's due set: pending rows (attempts < $max, joined to a
-- 'fulfilling' checkout by PK) claimed one at a time FOR UPDATE SKIP LOCKED
-- in id order; the index holds only the waiting rows.
CREATE INDEX fulfillment_outbox_pending_idx ON credits.fulfillment_outbox (id) WHERE status = 'pending';
-- A checkout's lines (the checkout view, compensation, the confirmed sum the reconciler checks).
CREATE INDEX fulfillment_outbox_checkout_idx ON credits.fulfillment_outbox (checkout_id, id);
-- The escrow grant lookup by broker reference (/admin/grants/{escrow_ref}/*, the reconciler's escrow pass).
CREATE INDEX fulfillment_outbox_external_ref_idx ON credits.fulfillment_outbox (external_ref)
    WHERE external_ref IS NOT NULL;

-- A Credits pledge against an on-chain CreditsManager purchase: the wallet
-- authorises `usd_cents` (priced to `amount_wei` at the oracle rate), the
-- pledge counts against the balance until it is consumed (a CreditUsed log
-- the usage indexer records; see "NOT created"), released by the client or
-- expired by the sweep (10 minute TTL). Reserved under the wallets row lock
-- so the outstanding pledged total never exceeds the balance.
CREATE TABLE credits.authorizations (
    id text NOT NULL,
    address text NOT NULL,
    usd_cents bigint NOT NULL,
    amount_wei numeric(78,0) NOT NULL,
    trade_id text,
    contract_address text,
    item_id text,
    source text,
    status text NOT NULL DEFAULT 'authorized',
    idempotency_key text,
    expires_at timestamp without time zone NOT NULL,
    created_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    CONSTRAINT authorizations_pkey PRIMARY KEY (id),
    CONSTRAINT authorizations_address_check CHECK (address ~ '^0x[0-9a-f]{40}$'),
    CONSTRAINT authorizations_status_check CHECK (status IN ('authorized', 'released', 'consumed', 'expired')),
    CONSTRAINT authorizations_usd_check CHECK (usd_cents > 0)
);

-- The idempotent reserve (the client's key, one authorization per key).
CREATE UNIQUE INDEX authorizations_idempotency_key_uidx ON credits.authorizations (idempotency_key)
    WHERE idempotency_key IS NOT NULL;
-- The outstanding budget: SUM(usd_cents) WHERE address = $1 AND status =
-- 'authorized' AND expires_at > $now, and the wallet's own list.
CREATE INDEX authorizations_address_status_expiry_idx ON credits.authorizations (address, status, expires_at);
-- The expiry sweep (status = 'authorized' AND expires_at <= $now), run by the fulfilment tick.
CREATE INDEX authorizations_expiry_idx ON credits.authorizations (expires_at) WHERE status = 'authorized';

-- Short-lived slider captchas for the claim flow: GET /captcha invalidates
-- the wallet's open challenges and inserts one (120 s TTL, the PNG is
-- regenerated from answer_x), POST /captcha consumes the newest open one
-- and compares within tolerance, then the hCaptcha token when a provider is
-- configured (CREDITS_CAPTCHA_SECRET(_FILE), verified server-side).
CREATE TABLE credits.captcha_challenges (
    id bigint GENERATED ALWAYS AS IDENTITY,
    address text NOT NULL,
    answer_x numeric NOT NULL,
    expires_at timestamp without time zone NOT NULL,
    consumed_at timestamp without time zone,
    CONSTRAINT captcha_challenges_pkey PRIMARY KEY (id),
    CONSTRAINT captcha_challenges_address_check CHECK (address ~ '^0x[0-9a-f]{40}$')
);

-- The wallet's open challenge (WHERE address = $1 AND consumed_at IS NULL
-- ORDER BY id DESC LIMIT 1) and the invalidating UPDATE over the same set.
CREATE INDEX captcha_challenges_open_idx ON credits.captcha_challenges (address, id DESC) WHERE consumed_at IS NULL;
-- The purge (expires_at < $now - 1 day, batched), part of the ops retention job.
CREATE INDEX captcha_challenges_expires_idx ON credits.captcha_challenges (expires_at);

-- ══════════════════════════════════════════════════════════════════════════
-- governance
-- ══════════════════════════════════════════════════════════════════════════

-- Mirrors of the Decentraland governance API, upserted by upstream id by
-- the `governance-sync` job (a bounded window of recently updated proposals
-- per pass, the active projects' updates, every budget, vesting and member
-- list). `raw` is the document served on the wire; the typed columns are
-- the fields the list filters, the activity feed and the joins read
-- (DATABASE.md 3). `fetched_at` is the mirror's freshness stamp and the
-- listing order the old API kept.
CREATE TABLE governance.proposals (
    id text NOT NULL,
    title text,
    description text,
    type text,
    status text,
    author text,
    snapshot_id text,
    snapshot_space text,
    snapshot_network text,
    snapshot_proposal jsonb,
    discourse_id integer,
    discourse_topic_id integer,
    discourse_topic_slug text,
    start_at timestamp without time zone,
    finish_at timestamp without time zone,
    created_at timestamp without time zone,
    updated_at timestamp without time zone,
    enacted boolean,
    enacted_by text,
    enacted_description text,
    enacting_tx text,
    passed_by text,
    passed_description text,
    rejected_by text,
    rejected_description text,
    deleted boolean,
    deleted_by text,
    required_to_pass integer,
    vesting_addresses jsonb,
    configuration jsonb,
    raw jsonb NOT NULL,
    fetched_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    CONSTRAINT proposals_pkey PRIMARY KEY (id)
);

-- GET /proposals: ORDER BY fetched_at DESC, id (the tie-breaker rides the same index).
CREATE INDEX proposals_fetched_at_idx ON governance.proposals (fetched_at DESC, id);
-- The ?status= and ?type= filters of the same listing.
CREATE INDEX proposals_status_idx ON governance.proposals (status);
CREATE INDEX proposals_type_idx ON governance.proposals (type);
-- GET /activity: the newest proposals (created_at DESC) and the latest
-- finished ones (finish_at < $now ORDER BY finish_at DESC), each LIMIT $n.
CREATE INDEX proposals_created_at_idx ON governance.proposals (created_at DESC);
CREATE INDEX proposals_finish_at_idx ON governance.proposals (finish_at DESC);

CREATE TABLE governance.projects (
    id text NOT NULL,
    proposal_id text,
    title text,
    status text,
    type text,
    author text,
    enacting_tx text,
    enacted_description text,
    configuration jsonb,
    vesting_addresses jsonb,
    funding jsonb,
    latest_update jsonb,
    created_at timestamp without time zone,
    updated_at timestamp without time zone,
    raw jsonb NOT NULL,
    fetched_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    CONSTRAINT projects_pkey PRIMARY KEY (id)
);

CREATE TABLE governance.project_updates (
    id text NOT NULL,
    project_id text,
    proposal_id text,
    status text,
    health text,
    introduction text,
    highlights text,
    blockers text,
    next_steps text,
    additional_notes text,
    author text,
    due_date timestamp without time zone,
    completion_date timestamp without time zone,
    discourse_topic_id integer,
    discourse_topic_slug text,
    created_at timestamp without time zone,
    updated_at timestamp without time zone,
    raw jsonb NOT NULL,
    fetched_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    CONSTRAINT project_updates_pkey PRIMARY KEY (id)
);

-- GET /projects/{id}: the project's updates aggregated in created_at order.
CREATE INDEX project_updates_project_idx ON governance.project_updates (project_id, created_at);
-- GET /activity: the newest updates across projects.
CREATE INDEX project_updates_created_at_idx ON governance.project_updates (created_at DESC);

CREATE TABLE governance.budgets (
    id text NOT NULL,
    start_at timestamp without time zone,
    finish_at timestamp without time zone,
    total bigint,
    allocated bigint,
    categories jsonb,
    raw jsonb NOT NULL,
    fetched_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    CONSTRAINT budgets_pkey PRIMARY KEY (id)
);

-- start_at / finish_at stay text: upstream sends dates outside the
-- timestamp range for some vestings.
CREATE TABLE governance.vestings (
    address text NOT NULL,
    token text,
    status text,
    total bigint,
    vested bigint,
    released bigint,
    releasable bigint,
    start_at text,
    finish_at text,
    raw jsonb NOT NULL,
    fetched_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    CONSTRAINT vestings_pkey PRIMARY KEY (address)
);

-- Committee and council membership, replaced per role on every pass; the
-- key order serves GET /members?role= ORDER BY role, address.
CREATE TABLE governance.members (
    role text NOT NULL,
    address text NOT NULL,
    fetched_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    CONSTRAINT members_pkey PRIMARY KEY (role, address),
    CONSTRAINT members_role_check CHECK (role IN ('committee', 'council'))
);

-- The mirror's durable progress: last_sync / last_backfill stamps (the old
-- sync_state), never discarded by a cache rebuild.
CREATE TABLE governance.mirror_state (
    key text NOT NULL,
    value text,
    updated_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    CONSTRAINT mirror_state_pkey PRIMARY KEY (key)
);

-- ══════════════════════════════════════════════════════════════════════════
-- quests
-- ══════════════════════════════════════════════════════════════════════════

-- A quest definition (the protobuf `QuestDefinition` bytes the SDK sends)
-- with the one-to-one facts the old service kept in marker tables:
-- `deactivated_at` (deactivated_quests), `previous_quest_id` (quest_updates:
-- a quest supersedes at most one predecessor and is superseded at most
-- once, so the pair is unique both ways) and the reward webhook
-- (quest_reward_hooks). Quests are immutable once created; an "update"
-- creates a new quest row, deactivates the predecessor and links them.
CREATE TABLE quests.quests (
    id uuid NOT NULL,
    name text NOT NULL,
    description text NOT NULL,
    definition bytea NOT NULL,
    creator_address text NOT NULL,
    image_url text NOT NULL DEFAULT '',
    previous_quest_id uuid,
    deactivated_at timestamp without time zone,
    reward_webhook_url text,
    reward_request_body jsonb,
    created_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    updated_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    CONSTRAINT quests_pkey PRIMARY KEY (id),
    CONSTRAINT quests_previous_fkey FOREIGN KEY (previous_quest_id) REFERENCES quests.quests (id),
    CONSTRAINT quests_creator_address_check CHECK (creator_address ~ '^0x[0-9a-f]{40}$'),
    CONSTRAINT quests_not_own_predecessor_check CHECK (previous_quest_id IS NULL OR previous_quest_id <> id),
    CONSTRAINT quests_reward_body_needs_url_check CHECK (reward_request_body IS NULL OR reward_webhook_url IS NOT NULL)
);

-- A quest is superseded once (the update path's `updatable` probe and the
-- creator listing's "not yet updated" join).
CREATE UNIQUE INDEX quests_previous_quest_uidx ON quests.quests (previous_quest_id) WHERE previous_quest_id IS NOT NULL;
-- GET /api/creators/{address}/quests ORDER BY created_at DESC.
CREATE INDEX quests_creator_created_idx ON quests.quests (creator_address, created_at DESC);

-- The rewards a completed instance earns (many per quest; `position` keeps the SDK's order).
CREATE TABLE quests.quest_reward_items (
    quest_id uuid NOT NULL,
    position integer NOT NULL,
    reward_name text NOT NULL,
    reward_image text NOT NULL,
    CONSTRAINT quest_reward_items_pkey PRIMARY KEY (quest_id, position),
    CONSTRAINT quest_reward_items_quest_fkey FOREIGN KEY (quest_id) REFERENCES quests.quests (id) ON DELETE CASCADE
);

-- A player's run of a quest. `completed_at` / `abandoned_at` replace the
-- completed_quest_instances / abandoned_quest_instances marker tables (one
-- fact each, UNIQUE per instance there); an instance is one or the other,
-- never both. The live progress is the fold of the instance's events over
-- the definition (never stored); the processor recomputes it per event.
CREATE TABLE quests.quest_instances (
    id uuid NOT NULL,
    quest_id uuid NOT NULL,
    user_address text NOT NULL,
    started_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    completed_at timestamp without time zone,
    abandoned_at timestamp without time zone,
    CONSTRAINT quest_instances_pkey PRIMARY KEY (id),
    CONSTRAINT quest_instances_quest_fkey FOREIGN KEY (quest_id) REFERENCES quests.quests (id),
    CONSTRAINT quest_instances_user_address_check CHECK (user_address ~ '^0x[0-9a-f]{40}$'),
    CONSTRAINT quest_instances_one_end_check CHECK (completed_at IS NULL OR abandoned_at IS NULL)
);

-- The player's instances (the WS `GetAllQuests` and the "already started this
-- quest" probe: WHERE user_address = $1 [AND quest_id = $2] AND abandoned_at IS NULL).
CREATE INDEX quest_instances_user_quest_idx ON quests.quest_instances (user_address, quest_id) WHERE abandoned_at IS NULL;
-- GET /api/quests/{id}/instances and /stats (a quest's active instances, paged).
CREATE INDEX quest_instances_quest_idx ON quests.quest_instances (quest_id, started_at) WHERE abandoned_at IS NULL;

-- One row per accepted `Event` (protobuf bytes), the instance's append-only
-- history folded into its state; removed only by the reset routes.
CREATE TABLE quests.quest_events (
    id uuid NOT NULL,
    quest_instance_id uuid NOT NULL,
    user_address text NOT NULL,
    event bytea NOT NULL,
    occurred_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    CONSTRAINT quest_events_pkey PRIMARY KEY (id),
    CONSTRAINT quest_events_instance_fkey FOREIGN KEY (quest_instance_id) REFERENCES quests.quest_instances (id),
    CONSTRAINT quest_events_user_address_check CHECK (user_address ~ '^0x[0-9a-f]{40}$')
);

-- The fold: an instance's events in order (every state computation, the reset's DELETE).
CREATE INDEX quest_events_instance_idx ON quests.quest_events (quest_instance_id, occurred_at, id);

-- ══════════════════════════════════════════════════════════════════════════
-- ops
-- ══════════════════════════════════════════════════════════════════════════

-- The ONE operator audit: every mutation an operator makes through the
-- console or the admin bearers (sync pause/resume/force, cache flush,
-- telemetry purge/quota/regroup/release/issue state/flags, credits
-- grant/revoke/refund/block/packs, rpc method and upstream edits, ...),
-- append-only: the application only ever INSERTs, in the same transaction
-- as the write it records where the write is SQL. `actor` is the
-- authenticated identity (a console wallet, `token:<name>` for a bearer,
-- `loopback`); `domain` + `action` the verb (`content.sync.pause`,
-- `telemetry.purge`, `credits.grant`), `target` the row or key acted on
-- (an issue fingerprint, a wallet, a checkout id), `detail` the request as
-- received (never a secret), `result` the outcome. The old per-service
-- audit tables (telemetry admin_audit, credits admin_audit, the server's
-- content admin_audit) were the same fact under three shapes.
CREATE TABLE ops.admin_audit (
    id bigint GENERATED ALWAYS AS IDENTITY,
    at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    actor text NOT NULL,
    domain text NOT NULL,
    action text NOT NULL,
    target text,
    detail jsonb NOT NULL DEFAULT '{}'::jsonb,
    result text NOT NULL DEFAULT 'ok',
    CONSTRAINT admin_audit_pkey PRIMARY KEY (id),
    CONSTRAINT admin_audit_domain_check CHECK (domain IN (
        'content', 'telemetry', 'credits', 'governance', 'quests', 'rpc', 'registry',
        'explore', 'worlds', 'social', 'create', 'market', 'economy', 'ops'))
);

-- GET /admin/api/audit (newest first, paged) and its ?domain=/?action= filter.
CREATE INDEX admin_audit_at_idx ON ops.admin_audit (at DESC);
CREATE INDEX admin_audit_domain_action_idx ON ops.admin_audit (domain, action, at DESC);
-- The per-target history (an issue's audit drill-down, a wallet's operator history).
CREATE INDEX admin_audit_target_idx ON ops.admin_audit (target, at DESC) WHERE target IS NOT NULL;
-- ?actor= (what one operator did).
CREATE INDEX admin_audit_actor_idx ON ops.admin_audit (actor, at DESC);

-- ==== section: telemetry ====
-- The retention job's client_error rule (source = 'segment' AND event_kind =
-- 'track' AND body->>'event' = 'client_error' AND received_at < $cutoff
-- ORDER BY received_at LIMIT $batch): without it each 5,000-row batch was a
-- bitmap scan of every segment/track row older than the cutoff filtered on
-- the body (514k rows removed by filter, 38k heap pages, 236 ms per batch at
-- 10^6 rows, metrics/telemetry-console.md); on the 2% of rows it covers the
-- batch is an index scan of exactly the rows it deletes.
CREATE INDEX events_client_error_received_idx ON telemetry.events (received_at)
    WHERE source = 'segment' AND event_kind = 'track' AND body->>'event' = 'client_error';
-- Still open here: monthly range partitions (purge = DROP PARTITION, the PK
-- widened to (received_at, id)) if the events table passes 10^7 rows or a
-- purge pass outlasts the ingest interval; the read-only role grant the
-- sites SSR needs on telemetry.events (deploy/nix/postgresql.nix).
-- ==== end section: telemetry ====

-- ==== section: credits ====
-- The CreditUsed usage index (src/credits/usage.rs; WORKLOADS.md "credits",
-- "usages, usage_index_cursor"): the CreditsManager contract's
-- `CreditUsed(address indexed _sender, bytes32 indexed _creditId, (uint256
-- value, uint256 expiresAt, bytes32 salt) _credit, uint256 _value)` logs,
-- read with eth_getLogs over the collections chain (RPC_ENDPOINT_POLYGON)
-- up to its finalized block, one row per log. Chain facts, append-only:
-- a log is never updated, and a replay re-inserts the same key.
CREATE TABLE credits.usages (
    chain_id bigint NOT NULL,
    block_number bigint NOT NULL,
    log_index integer NOT NULL,
    contract text NOT NULL,
    tx_hash text NOT NULL,
    block_time timestamp without time zone,
    sender text NOT NULL,
    credit_id text NOT NULL,
    salt text NOT NULL,
    credit_value numeric(78,0) NOT NULL,
    credit_expires_at numeric(78,0) NOT NULL,
    value_wei numeric(78,0) NOT NULL,
    indexed_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    CONSTRAINT usages_pkey PRIMARY KEY (chain_id, block_number, log_index),
    CONSTRAINT usages_contract_check CHECK (contract ~ '^0x[0-9a-f]{40}$'),
    CONSTRAINT usages_sender_check CHECK (sender ~ '^0x[0-9a-f]{40}$'),
    CONSTRAINT usages_credit_id_check CHECK (credit_id ~ '^0x[0-9a-f]{64}$'),
    CONSTRAINT usages_salt_check CHECK (salt ~ '^0x[0-9a-f]{64}$'),
    CONSTRAINT usages_tx_hash_check CHECK (tx_hash ~ '^0x[0-9a-f]{64}$')
);

-- The PK serves the fold's incremental range (chain_id = $1 AND
-- block_number > $since AND block_number <= $through). This one serves the
-- other direction: the usages of one salt (a credit this node signed: the
-- reconciler's full pass and GET /credits/usage-index/status's lookups).
CREATE INDEX usages_salt_idx ON credits.usages (salt);

-- The indexer's durable progress, one row per chain: `last_block` is what
-- the log holds (advanced in the transaction that inserts a range's rows),
-- `folded_block` the block the last published fold covered, with
-- `folded_block_time` its chain time (the bound of the authorization expiry
-- sweep: a pledge is expired only once the index has read past its
-- expiry) and `folded_at` the wall clock of that publish (readers trust
-- the index while it is at most 10 minutes old and folded_block =
-- last_block). `contract` refuses a node reconfigured to another manager
-- without a replay.
CREATE TABLE credits.usage_index_cursor (
    chain_id bigint NOT NULL,
    contract text NOT NULL,
    last_block bigint NOT NULL,
    folded_block bigint,
    folded_block_time timestamp without time zone,
    folded_at timestamp without time zone,
    full_replay_started boolean NOT NULL DEFAULT false,
    updated_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    CONSTRAINT usage_index_cursor_pkey PRIMARY KEY (chain_id),
    CONSTRAINT usage_index_cursor_contract_check CHECK (contract ~ '^0x[0-9a-f]{40}$'),
    CONSTRAINT usage_index_cursor_folded_check CHECK (folded_block IS NULL OR folded_block <= last_block)
);
-- ==== end section: credits ====

-- ==== section: governance ====
-- The archive mirrors (src/governance/archive; WORKLOADS.md "governance",
-- "the archive mirrors"), replacing the old SNAPSHOT_DATABASE_URL and
-- DISCOURSE_DATABASE_URL side pools: the Snapshot hub's proposals and votes
-- of the configured spaces (SNAPSHOT_SPACE, SNAPSHOT_SPACE_COUNCIL) and
-- the Discourse posts of the topics governance proposals link. Mirrored,
-- never local authority; progress lives in governance.mirror_state.

-- One row per hub proposal of a mirrored space: the head of GET
-- /proposals/{id}/votes (choices, scores, scores_total, votes_count).
-- Refreshed while `state` is not 'closed' or `scores_state` not 'final'.
CREATE TABLE governance.snapshot_proposals (
    id text NOT NULL,
    space text NOT NULL,
    state text NOT NULL,
    scores_state text,
    start_at timestamp without time zone,
    end_at timestamp without time zone,
    created_at timestamp without time zone,
    choices jsonb NOT NULL DEFAULT '[]'::jsonb,
    scores jsonb NOT NULL DEFAULT '[]'::jsonb,
    scores_total double precision NOT NULL DEFAULT 0,
    votes_count integer NOT NULL DEFAULT 0,
    fetched_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    CONSTRAINT snapshot_proposals_pkey PRIMARY KEY (id)
);

-- One vote per (proposal, voter), Snapshot's own rule: a re-vote replaces
-- the voter's row (a new hub id, a later `created_at`). No FK to
-- snapshot_proposals: the space-wide votes stream may run ahead of the
-- proposals page within a pass; the routes read votes by proposal id and a
-- vote whose proposal is not mirrored is simply never served. `voter` keeps
-- the hub's casing (mirrored, like governance.proposals.author).
CREATE TABLE governance.snapshot_votes (
    proposal_id text NOT NULL,
    voter text NOT NULL,
    id text NOT NULL,
    space text NOT NULL,
    choice jsonb NOT NULL,
    vp double precision NOT NULL DEFAULT 0,
    reason text,
    created_at timestamp without time zone NOT NULL,
    fetched_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    CONSTRAINT snapshot_votes_pkey PRIMARY KEY (proposal_id, voter)
);

-- The PK prefix serves GET /proposals/{id}/votes (WHERE proposal_id = $1,
-- sorted in the statement: at most 20,000 rows). This one serves the two
-- time-windowed reads: /votes/engagement (created_at >= $cutoff, grouped)
-- and the vote half of /activity (ORDER BY created_at DESC LIMIT $n).
CREATE INDEX snapshot_votes_created_idx ON governance.snapshot_votes (created_at DESC);

-- One row per Discourse topic a governance proposal links
-- (governance.proposals.discourse_topic_id), with the topic's counters as
-- of `fetched_at`; `status` 'missing' is a topic the forum answered 403,
-- 404 or 410 for (deleted or private).
CREATE TABLE governance.discourse_topics (
    topic_id integer NOT NULL,
    status text NOT NULL DEFAULT 'ok',
    posts_count integer,
    highest_post_number integer,
    last_posted_at timestamp without time zone,
    fetched_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    CONSTRAINT discourse_topics_pkey PRIMARY KEY (topic_id),
    CONSTRAINT discourse_topics_status_check CHECK (status IN ('ok', 'missing'))
);

-- The weekly re-verify due set (fetched_at < $stale_before ORDER BY
-- fetched_at LIMIT $n): edits, hides and deletions the posts feed misses.
CREATE INDEX discourse_topics_fetched_idx ON governance.discourse_topics (fetched_at);

-- The posts of the mirrored topics. `raw` is the Markdown the comments
-- route serves as `text`; a post the topic's stream no longer lists is
-- stamped `deleted_at` (the stream is the forum's current visible set).
CREATE TABLE governance.discourse_posts (
    id integer NOT NULL,
    topic_id integer NOT NULL,
    post_number integer NOT NULL,
    username text NOT NULL,
    raw text NOT NULL DEFAULT '',
    created_at timestamp without time zone NOT NULL,
    updated_at timestamp without time zone,
    hidden boolean NOT NULL DEFAULT false,
    deleted_at timestamp without time zone,
    fetched_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    CONSTRAINT discourse_posts_pkey PRIMARY KEY (id),
    CONSTRAINT discourse_posts_topic_fkey FOREIGN KEY (topic_id)
        REFERENCES governance.discourse_topics (topic_id) ON DELETE CASCADE
);

-- GET /proposals/{id}/comments (WHERE topic_id = $1 AND post_number > 1 AND
-- NOT hidden AND deleted_at IS NULL ORDER BY created_at DESC LIMIT $2, with
-- the count over the same set), the deletion stamp of a topic refresh
-- (WHERE topic_id = $1 AND deleted_at IS NULL AND id <> ALL($2)) and the
-- FK's cascade: a topic holds tens of posts, so the flags filter in the heap.
CREATE INDEX discourse_posts_topic_created_idx ON governance.discourse_posts (topic_id, created_at DESC);
-- ==== end section: governance ====

-- ==== section: quests ====
-- ==== end section: quests ====

-- ==== section: ops ====
-- (ops agent: a persisted job report or sync-control row if a restart must
-- keep them; both are process state today, see WORKLOADS.md "ops")
-- ==== end section: ops ====
