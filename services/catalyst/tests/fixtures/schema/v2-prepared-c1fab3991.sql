--
-- PostgreSQL database dump
--


-- Dumped from database version 18.6
-- Dumped by pg_dump version 18.6

SET statement_timeout = 0;
SET lock_timeout = 0;
SET idle_in_transaction_session_timeout = 0;
SET client_encoding = 'UTF8';
SET standard_conforming_strings = on;
SELECT pg_catalog.set_config('search_path', '', false);
SET check_function_bodies = false;
SET xmloption = content;
SET client_min_messages = warning;
SET row_security = off;

--
-- Name: builder; Type: SCHEMA; Schema: -; Owner: -
--

CREATE SCHEMA builder;


--
-- Name: camera_reel; Type: SCHEMA; Schema: -; Owner: -
--

CREATE SCHEMA camera_reel;


--
-- Name: chat; Type: SCHEMA; Schema: -; Owner: -
--

CREATE SCHEMA chat;


--
-- Name: content; Type: SCHEMA; Schema: -; Owner: -
--

CREATE SCHEMA content;


--
-- Name: credits; Type: SCHEMA; Schema: -; Owner: -
--

CREATE SCHEMA credits;


--
-- Name: economy; Type: SCHEMA; Schema: -; Owner: -
--

CREATE SCHEMA economy;


--
-- Name: explore; Type: SCHEMA; Schema: -; Owner: -
--

CREATE SCHEMA explore;


--
-- Name: explore_api; Type: SCHEMA; Schema: -; Owner: -
--

CREATE SCHEMA explore_api;


--
-- Name: governance; Type: SCHEMA; Schema: -; Owner: -
--

CREATE SCHEMA governance;


--
-- Name: land_authz; Type: SCHEMA; Schema: -; Owner: -
--

CREATE SCHEMA land_authz;


--
-- Name: meta; Type: SCHEMA; Schema: -; Owner: -
--

CREATE SCHEMA meta;


--
-- Name: ops; Type: SCHEMA; Schema: -; Owner: -
--

CREATE SCHEMA ops;


--
-- Name: presence; Type: SCHEMA; Schema: -; Owner: -
--

CREATE SCHEMA presence;


--
-- Name: quests; Type: SCHEMA; Schema: -; Owner: -
--

CREATE SCHEMA quests;


--
-- Name: shop; Type: SCHEMA; Schema: -; Owner: -
--

CREATE SCHEMA shop;


--
-- Name: signatures; Type: SCHEMA; Schema: -; Owner: -
--

CREATE SCHEMA signatures;


--
-- Name: social; Type: SCHEMA; Schema: -; Owner: -
--

CREATE SCHEMA social;


--
-- Name: telemetry; Type: SCHEMA; Schema: -; Owner: -
--

CREATE SCHEMA telemetry;


--
-- Name: worlds; Type: SCHEMA; Schema: -; Owner: -
--

CREATE SCHEMA worlds;


--
-- Name: en_US.utf8; Type: COLLATION; Schema: public; Owner: -
--



--
-- Name: asset_direction_type; Type: TYPE; Schema: shop; Owner: -
--

CREATE TYPE shop.asset_direction_type AS ENUM (
    'sent',
    'received'
);


--
-- Name: trade_type; Type: TYPE; Schema: shop; Owner: -
--

CREATE TYPE shop.trade_type AS ENUM (
    'bid',
    'public_nft_order',
    'public_item_order'
);


--
-- Name: rental_status; Type: TYPE; Schema: signatures; Owner: -
--

CREATE TYPE signatures.rental_status AS ENUM (
    'open',
    'executed',
    'cancelled',
    'claimed'
);


--
-- Name: image_wearable_items(jsonb); Type: FUNCTION; Schema: camera_reel; Owner: -
--

CREATE FUNCTION camera_reel.image_wearable_items(metadata jsonb) RETURNS text[]
    LANGUAGE sql IMMUTABLE PARALLEL SAFE
    AS $$
  SELECT coalesce(
           array_agg(DISTINCT lower(split_part(wearable, ':', 5) || '-' || split_part(wearable, ':', 6))),
           ARRAY[]::text[])
  FROM jsonb_array_elements(CASE WHEN jsonb_typeof(metadata -> 'visiblePeople') = 'array'
                                 THEN metadata -> 'visiblePeople' ELSE '[]'::jsonb END) AS person,
       jsonb_array_elements_text(CASE WHEN jsonb_typeof(person -> 'wearables') = 'array'
                                      THEN person -> 'wearables' ELSE '[]'::jsonb END) AS wearable
  WHERE wearable LIKE 'urn:decentraland:%:collections-v2:%'
$$;


--
-- Name: deploy_entity_218dd4c4d67c(text, text, text, text, json, double precision, text[], json, text[], text[], text); Type: FUNCTION; Schema: content; Owner: -
--

CREATE FUNCTION content.deploy_entity_218dd4c4d67c(text, text, text, text, json, double precision, text[], json, text[], text[], text) RETURNS TABLE(dep_id integer, newer boolean, overwrote integer, cleared integer)
    LANGUAGE plpgsql
    AS $_$ #variable_conflict use_column
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
END $_$;


--
-- Name: deploy_entity_66918b614b85(text, text, text, text, json, double precision, text[], json, text[], text[], text); Type: FUNCTION; Schema: content; Owner: -
--

CREATE FUNCTION content.deploy_entity_66918b614b85(text, text, text, text, json, double precision, text[], json, text[], text[], text) RETURNS TABLE(dep_id integer, newer boolean, overwrote integer, cleared integer)
    LANGUAGE plpgsql
    AS $_$ #variable_conflict use_column
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
END $_$;


--
-- Name: notify_new_deployment(); Type: FUNCTION; Schema: content; Owner: -
--

CREATE FUNCTION content.notify_new_deployment() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
BEGIN
    PERFORM pg_notify('new_deployment', NEW.entity_type || ':' || NEW.entity_id);
    RETURN NEW;
END;
$$;


--
-- Name: unpublish_scene_be255dae6005(text); Type: FUNCTION; Schema: content; Owner: -
--

CREATE FUNCTION content.unpublish_scene_be255dae6005(text) RETURNS TABLE(entity_id text, tombstoned bigint, pointer text, replacement text)
    LANGUAGE plpgsql
    AS $_$ #variable_conflict use_column
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
END $_$;


--
-- Name: capture_place_removal(); Type: FUNCTION; Schema: explore; Owner: -
--

CREATE FUNCTION explore.capture_place_removal() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
DECLARE
    place_key text;
    snapshot record;
    was_removed boolean;
    now_removed boolean;
    effective_reason text;
BEGIN
    place_key := CASE WHEN TG_OP = 'DELETE' THEN OLD.id ELSE NEW.id END;
    PERFORM pg_advisory_xact_lock(hashtext('explore.place-removal:' || place_key));
    SELECT p.* INTO snapshot FROM (
        SELECT id, creator_address, world, world_name, base_position, raw, disabled, 0 AS priority FROM explore.place_world_local WHERE id = place_key
        UNION ALL
        SELECT id, creator_address, world, world_name, base_position, raw, disabled, 1 AS priority FROM explore.place WHERE id = place_key
    ) p ORDER BY priority LIMIT 1;
    IF NOT FOUND THEN
        IF TG_OP <> 'DELETE' THEN RETURN NULL; END IF;
        snapshot := OLD;
        effective_reason := COALESCE((SELECT reason FROM explore.place_moderation WHERE place_id = place_key AND disabled IS NOT NULL), OLD.raw->>'disabled_reason');
        now_removed := effective_reason IS DISTINCT FROM 'opt_out';
    ELSE
        effective_reason := COALESCE((SELECT reason FROM explore.place_moderation WHERE place_id = place_key AND disabled IS NOT NULL), snapshot.raw->>'disabled_reason');
        now_removed := snapshot.disabled AND effective_reason IS DISTINCT FROM 'opt_out';
    END IF;
    INSERT INTO explore.place_removal_state (place_id, removed) VALUES (place_key, false) ON CONFLICT DO NOTHING;
    SELECT removed INTO was_removed FROM explore.place_removal_state WHERE place_id = place_key FOR UPDATE;
    IF now_removed AND NOT was_removed THEN
        INSERT INTO explore.place_removal_pending (place_id, owner_address, world, world_name, base_position, positions)
        VALUES (place_key, CASE WHEN snapshot.world THEN COALESCE(snapshot.raw->>'owner', snapshot.creator_address) ELSE snapshot.raw->>'owner' END,
            snapshot.world, snapshot.world_name, snapshot.base_position, COALESCE(snapshot.raw->'positions', jsonb_build_array(snapshot.base_position)));
    END IF;
    UPDATE explore.place_removal_state SET removed = now_removed WHERE place_id = place_key AND removed IS DISTINCT FROM now_removed;
    IF NOT now_removed AND NOT EXISTS (SELECT 1 FROM explore.place WHERE id = place_key) AND NOT EXISTS (SELECT 1 FROM explore.place_world_local WHERE id = place_key) AND NOT EXISTS (SELECT 1 FROM explore.place_removal_pending WHERE place_id = place_key) THEN
        DELETE FROM explore.place_removal_state WHERE place_id = place_key;
    END IF;
    RETURN NULL;
END;
$$;


--
-- Name: decode_coord(numeric); Type: FUNCTION; Schema: land_authz; Owner: -
--

CREATE FUNCTION land_authz.decode_coord(half numeric) RETURNS integer
    LANGUAGE sql IMMUTABLE
    AS $$
  SELECT (CASE WHEN half >= 170141183460469231731687303715884105728::numeric
               THEN half - 340282366920938463463374607431768211456::numeric
               ELSE half END)::integer
$$;


--
-- Name: scene_collaborators_current(text, text, text, text[], timestamp without time zone); Type: FUNCTION; Schema: land_authz; Owner: -
--

CREATE FUNCTION land_authz.scene_collaborators_current(configured_network text, configured_land text, configured_estate text, pointers text[], entity_timestamp timestamp without time zone) RETURNS boolean
    LANGUAGE sql STABLE
    AS $_$
  WITH ready AS (
    SELECT folded_block FROM land_authz.index_cursor
    WHERE network = configured_network
      AND land_registry = configured_land AND estate_registry = configured_estate
      AND full_replay_started AND history_timestamps_ready
      AND folded_block IS NOT NULL
      AND folded_at BETWEEN timezone('UTC', now()) - interval '30 minutes'
                        AND timezone('UTC', now())
  ), parcels AS (
    SELECT p.pointer, t.token_id, t.owner, el.estate_token_id, et.owner AS estate_owner
    FROM unnest(pointers) AS p(pointer)
    LEFT JOIN land_authz.token_right t
      ON t.token_address = configured_land
     AND t.x = CASE WHEN p.pointer ~ '^-?[0-9]{1,9},-?[0-9]{1,9}$'
                        THEN split_part(p.pointer, ',', 1)::integer END
     AND t.y = CASE WHEN p.pointer ~ '^-?[0-9]{1,9},-?[0-9]{1,9}$'
                        THEN split_part(p.pointer, ',', 2)::integer END
     AND p.pointer = t.x::text || ',' || t.y::text
    LEFT JOIN land_authz.estate_land el ON el.land_token_id = t.token_id
    LEFT JOIN land_authz.token_right et
      ON et.token_address = configured_estate AND et.token_id = el.estate_token_id
  )
  SELECT COALESCE(
    entity_timestamp > timestamp '1970-01-01'
    AND entity_timestamp <= timezone('UTC', now())
    AND cardinality(pointers) > 0 AND EXISTS (SELECT 1 FROM ready)
    AND (SELECT bool_and(COALESCE(
      p.token_id IS NOT NULL AND p.owner IS NOT NULL
      AND p.owner <> '0x0000000000000000000000000000000000000000'
      AND ((p.owner = configured_estate) = (p.estate_token_id IS NOT NULL))
      AND lt.block_number <= (SELECT folded_block FROM ready)
      AND lt.block_time > 0
      AND lt.block_time < extract(epoch FROM entity_timestamp - interval '15 minutes')
      AND (membership.block_number IS NULL OR (
        membership.block_number <= (SELECT folded_block FROM ready)
        AND membership.block_time > 0
        AND membership.block_time < extract(epoch FROM entity_timestamp - interval '15 minutes')))
      AND (p.estate_token_id IS NULL OR (
        p.estate_owner IS NOT NULL
        AND p.estate_owner <> '0x0000000000000000000000000000000000000000'
        AND et.block_number <= (SELECT folded_block FROM ready)
        AND et.block_time > 0
        AND et.block_time < extract(epoch FROM entity_timestamp - interval '15 minutes')))
    , false)) FROM parcels p
    LEFT JOIN LATERAL (
      SELECT block_number, block_time FROM land_authz.authz_event
      WHERE token_address = configured_land AND token_id = p.token_id AND kind = 'transfer'
      ORDER BY block_number DESC, log_index DESC LIMIT 1
    ) lt ON true
    LEFT JOIN LATERAL (
      SELECT block_number, block_time FROM land_authz.authz_event
      WHERE token_address = configured_estate AND token_id = p.token_id
        AND kind IN ('add_land', 'remove_land')
      ORDER BY block_number DESC, log_index DESC LIMIT 1
    ) membership ON true
    LEFT JOIN LATERAL (
      SELECT block_number, block_time FROM land_authz.authz_event
      WHERE token_address = configured_estate AND token_id = p.estate_token_id AND kind = 'transfer'
      ORDER BY block_number DESC, log_index DESC LIMIT 1
    ) et ON true), false)
$_$;


--
-- Name: token_x(numeric); Type: FUNCTION; Schema: land_authz; Owner: -
--

CREATE FUNCTION land_authz.token_x(token_id numeric) RETURNS integer
    LANGUAGE sql IMMUTABLE
    AS $$
  SELECT land_authz.decode_coord(div(token_id, 340282366920938463463374607431768211456::numeric))
$$;


--
-- Name: token_y(numeric); Type: FUNCTION; Schema: land_authz; Owner: -
--

CREATE FUNCTION land_authz.token_y(token_id numeric) RETURNS integer
    LANGUAGE sql IMMUTABLE
    AS $$
  SELECT land_authz.decode_coord(mod(token_id, 340282366920938463463374607431768211456::numeric))
$$;


--
-- Name: logs_permissions(text); Type: FUNCTION; Schema: worlds; Owner: -
--

CREATE FUNCTION worlds.logs_permissions(text) RETURNS text[]
    LANGUAGE sql IMMUTABLE PARALLEL SAFE
    AS $_$
    SELECT ARRAY(
        SELECT lower(btrim(e::text, '"'))
        FROM json_array_elements(CASE WHEN json_typeof($1::json) = 'array' THEN $1::json ELSE '[]'::json END) e
        WHERE json_typeof(e) = 'string')
$_$;


SET default_tablespace = '';

SET default_table_access_method = heap;

--
-- Name: asset_catalog; Type: TABLE; Schema: builder; Owner: -
--

CREATE TABLE builder.asset_catalog (
    id integer DEFAULT 1 NOT NULL,
    packs jsonb NOT NULL,
    source text NOT NULL,
    built_at timestamp without time zone NOT NULL,
    CONSTRAINT asset_catalog_packs_check CHECK ((jsonb_typeof(packs) = 'array'::text)),
    CONSTRAINT asset_catalog_singleton_check CHECK ((id = 1))
);


--
-- Name: asset_contents; Type: TABLE; Schema: builder; Owner: -
--

CREATE TABLE builder.asset_contents (
    asset_id uuid NOT NULL,
    file text NOT NULL,
    hash text NOT NULL,
    CONSTRAINT asset_contents_file_check CHECK (((file <> ''::text) AND (char_length(file) <= 256) AND (POSITION(('\'::text) IN (file)) = 0)))
);


--
-- Name: asset_packs; Type: TABLE; Schema: builder; Owner: -
--

CREATE TABLE builder.asset_packs (
    id uuid NOT NULL,
    project_id uuid NOT NULL,
    title text NOT NULL,
    thumbnail text,
    is_deleted boolean DEFAULT false NOT NULL,
    created_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    updated_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    CONSTRAINT asset_packs_title_check CHECK (((char_length(title) >= 3) AND (char_length(title) <= 20)))
);


--
-- Name: assets; Type: TABLE; Schema: builder; Owner: -
--

CREATE TABLE builder.assets (
    id uuid NOT NULL,
    asset_pack_id uuid NOT NULL,
    ordinal integer NOT NULL,
    legacy_id text,
    name text NOT NULL,
    model text NOT NULL,
    script text,
    thumbnail text,
    tags text[] DEFAULT '{}'::text[] NOT NULL,
    category text NOT NULL,
    metrics jsonb DEFAULT '{}'::jsonb NOT NULL,
    parameters jsonb DEFAULT '[]'::jsonb NOT NULL,
    actions jsonb DEFAULT '[]'::jsonb NOT NULL,
    CONSTRAINT assets_name_check CHECK (((char_length(name) >= 3) AND (char_length(name) <= 50))),
    CONSTRAINT assets_ordinal_check CHECK ((ordinal >= 0))
);


--
-- Name: collection_publications; Type: TABLE; Schema: builder; Owner: -
--

CREATE TABLE builder.collection_publications (
    collection_id uuid NOT NULL,
    preparation jsonb NOT NULL,
    status text NOT NULL,
    tx_hash text,
    contract_address text,
    created_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    updated_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    CONSTRAINT collection_publications_contract_state_check CHECK (((status = 'published'::text) = (contract_address IS NOT NULL))),
    CONSTRAINT collection_publications_preparation_check CHECK (((jsonb_typeof(preparation) = 'object'::text) AND (preparation ? 'revision'::text))),
    CONSTRAINT collection_publications_status_check CHECK ((status = ANY (ARRAY['prepared'::text, 'signing'::text, 'submitted'::text, 'published'::text, 'reverted'::text]))),
    CONSTRAINT collection_publications_tx_hash_state_check CHECK (((status = ANY (ARRAY['prepared'::text, 'signing'::text])) = (tx_hash IS NULL)))
);


--
-- Name: collections; Type: TABLE; Schema: builder; Owner: -
--

CREATE TABLE builder.collections (
    id uuid NOT NULL,
    name text NOT NULL,
    eth_address text NOT NULL,
    salt text,
    contract_address text,
    urn_suffix text,
    third_party_id text,
    is_published boolean DEFAULT false NOT NULL,
    is_approved boolean DEFAULT false NOT NULL,
    publication_pending boolean DEFAULT false NOT NULL,
    created_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    updated_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    CONSTRAINT collections_contract_published_check CHECK (((contract_address IS NULL) OR is_published)),
    CONSTRAINT collections_eth_address_lowercase_check CHECK ((eth_address = lower(eth_address))),
    CONSTRAINT collections_name_check CHECK (((name = btrim(name)) AND (name <> ''::text))),
    CONSTRAINT collections_pending_not_published_check CHECK ((NOT (publication_pending AND is_published))),
    CONSTRAINT collections_salt_len_check CHECK (((salt IS NULL) OR (char_length(salt) <= 128))),
    CONSTRAINT collections_third_party_urn_check CHECK (((third_party_id IS NULL) OR ((urn_suffix IS NOT NULL) AND ("left"(urn_suffix, (char_length(third_party_id) + 1)) = (third_party_id || ':'::text)))))
);


--
-- Name: item_contents; Type: TABLE; Schema: builder; Owner: -
--

CREATE TABLE builder.item_contents (
    item_id uuid NOT NULL,
    file text NOT NULL,
    hash text NOT NULL,
    CONSTRAINT item_contents_file_check CHECK (((file <> ''::text) AND (char_length(file) <= 256) AND (POSITION(('\'::text) IN (file)) = 0)))
);


--
-- Name: items; Type: TABLE; Schema: builder; Owner: -
--

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
    data jsonb DEFAULT '{}'::jsonb NOT NULL,
    metrics jsonb,
    utility text,
    mappings jsonb,
    urn_suffix text,
    blockchain_item_id text,
    is_published boolean DEFAULT false NOT NULL,
    curation_status text DEFAULT 'pending'::text NOT NULL,
    is_approved boolean DEFAULT false NOT NULL,
    in_catalyst boolean DEFAULT false NOT NULL,
    total_supply bigint DEFAULT 0 NOT NULL,
    local_content_hash text,
    content_hash text,
    catalyst_content_hash text,
    created_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    updated_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    CONSTRAINT items_approved_projection_check CHECK ((is_approved = (curation_status = 'approved'::text))),
    CONSTRAINT items_beneficiary_check CHECK (((beneficiary IS NULL) OR (beneficiary ~ '^0x[0-9a-fA-F]{40}$'::text))),
    CONSTRAINT items_curation_status_check CHECK ((curation_status = ANY (ARRAY['pending'::text, 'approved'::text, 'rejected'::text]))),
    CONSTRAINT items_data_object_check CHECK ((jsonb_typeof(data) = 'object'::text)),
    CONSTRAINT items_eth_address_lowercase_check CHECK ((eth_address = lower(eth_address))),
    CONSTRAINT items_metrics_object_check CHECK (((metrics IS NULL) OR (jsonb_typeof(metrics) = 'object'::text))),
    CONSTRAINT items_name_check CHECK (((name = btrim(name)) AND (name <> ''::text))),
    CONSTRAINT items_price_check CHECK (((price IS NULL) OR (price ~ '^[0-9]{1,78}$'::text))),
    CONSTRAINT items_published_identity_check CHECK (((NOT is_published) OR ((urn_suffix IS NOT NULL) AND (blockchain_item_id IS NOT NULL)))),
    CONSTRAINT items_rarity_check CHECK (((rarity IS NULL) OR (rarity = ANY (ARRAY['unique'::text, 'mythic'::text, 'exotic'::text, 'legendary'::text, 'epic'::text, 'rare'::text, 'uncommon'::text, 'common'::text])))),
    CONSTRAINT items_total_supply_check CHECK ((total_supply >= 0)),
    CONSTRAINT items_type_check CHECK ((type = ANY (ARRAY['wearable'::text, 'emote'::text])))
);


--
-- Name: linked_collection_publications; Type: TABLE; Schema: builder; Owner: -
--

CREATE TABLE builder.linked_collection_publications (
    collection_id uuid NOT NULL,
    preparation jsonb NOT NULL,
    snapshot jsonb NOT NULL,
    salt text NOT NULL,
    status text NOT NULL,
    cheque jsonb,
    forum_url text,
    created_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    updated_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    CONSTRAINT linked_collection_publications_cheque_state_check CHECK (((status = ANY (ARRAY['prepared'::text, 'signing'::text])) = (cheque IS NULL))),
    CONSTRAINT linked_collection_publications_forum_state_check CHECK (((forum_url IS NULL) OR (status = 'submitted'::text))),
    CONSTRAINT linked_collection_publications_preparation_check CHECK (((jsonb_typeof(preparation) = 'object'::text) AND (preparation ? 'revision'::text))),
    CONSTRAINT linked_collection_publications_salt_check CHECK ((salt ~ '^0x[0-9a-f]{64}$'::text)),
    CONSTRAINT linked_collection_publications_status_check CHECK ((status = ANY (ARRAY['prepared'::text, 'signing'::text, 'authorized'::text, 'submitted'::text])))
);


--
-- Name: newsletter_subscriptions; Type: TABLE; Schema: builder; Owner: -
--

CREATE TABLE builder.newsletter_subscriptions (
    email text NOT NULL,
    source text NOT NULL,
    created_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    CONSTRAINT newsletter_subscriptions_email_check CHECK (((email = lower(btrim(email))) AND (email <> ''::text)))
);


--
-- Name: project_files; Type: TABLE; Schema: builder; Owner: -
--

CREATE TABLE builder.project_files (
    project_id uuid NOT NULL,
    path text NOT NULL,
    hash text NOT NULL,
    size bigint NOT NULL,
    CONSTRAINT project_files_path_check CHECK ((((char_length(path) >= 1) AND (char_length(path) <= 512)) AND (POSITION(('\'::text) IN (path)) = 0) AND (POSITION(('//'::text) IN (path)) = 0) AND ("left"(path, 1) <> '/'::text) AND ("right"(path, 1) <> '/'::text) AND (path !~ '(^|/)\.\.?(/|$)'::text))),
    CONSTRAINT project_files_size_check CHECK (((size >= 0) AND (size <= 20971520)))
);


--
-- Name: project_manifests; Type: TABLE; Schema: builder; Owner: -
--

CREATE TABLE builder.project_manifests (
    project_id uuid NOT NULL,
    manifest jsonb NOT NULL,
    CONSTRAINT project_manifests_manifest_check CHECK ((jsonb_typeof(manifest) = 'object'::text))
);


--
-- Name: projects; Type: TABLE; Schema: builder; Owner: -
--

CREATE TABLE builder.projects (
    id uuid NOT NULL,
    eth_address text NOT NULL,
    title text NOT NULL,
    description text DEFAULT ''::text NOT NULL,
    scene_id uuid NOT NULL,
    cols integer NOT NULL,
    rows integer NOT NULL,
    parcels integer DEFAULT 0 NOT NULL,
    transforms integer DEFAULT 0 NOT NULL,
    gltf_shapes integer DEFAULT 0 NOT NULL,
    nft_shapes integer DEFAULT 0 NOT NULL,
    scripts integer DEFAULT 0 NOT NULL,
    entities integer DEFAULT 0 NOT NULL,
    thumbnail text,
    creation_coords text,
    is_public boolean DEFAULT false NOT NULL,
    is_template boolean DEFAULT false NOT NULL,
    template_status text,
    video text,
    is_deleted boolean DEFAULT false NOT NULL,
    files_version bigint DEFAULT 0 NOT NULL,
    manifest_revision bigint DEFAULT 0 NOT NULL,
    created_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    updated_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    CONSTRAINT projects_description_check CHECK ((char_length(description) <= 4096)),
    CONSTRAINT projects_eth_address_lowercase_check CHECK ((eth_address = lower(eth_address))),
    CONSTRAINT projects_layout_check CHECK ((((cols >= 1) AND (cols <= 1000)) AND ((rows >= 1) AND (rows <= 1000)))),
    CONSTRAINT projects_title_check CHECK ((char_length(title) <= 512)),
    CONSTRAINT projects_versions_check CHECK (((files_version >= 0) AND (manifest_revision >= 0)))
);


--
-- Name: uploaded_contents; Type: TABLE; Schema: builder; Owner: -
--

CREATE TABLE builder.uploaded_contents (
    hash text NOT NULL,
    size bigint NOT NULL,
    uploaded_by text NOT NULL,
    created_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    CONSTRAINT uploaded_contents_size_check CHECK (((size >= 0) AND (size <= 20971520))),
    CONSTRAINT uploaded_contents_uploaded_by_lowercase_check CHECK ((uploaded_by = lower(uploaded_by)))
);


--
-- Name: images; Type: TABLE; Schema: camera_reel; Owner: -
--

CREATE TABLE camera_reel.images (
    id uuid NOT NULL,
    user_address text NOT NULL,
    image_hash text NOT NULL,
    thumbnail_hash text NOT NULL,
    is_public boolean DEFAULT false NOT NULL,
    review_status text DEFAULT 'ok'::text NOT NULL,
    metadata jsonb NOT NULL,
    place_id text GENERATED ALWAYS AS ((metadata ->> 'placeId'::text)) STORED,
    created_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    ranking_published boolean DEFAULT true NOT NULL,
    CONSTRAINT images_metadata_object_check CHECK ((jsonb_typeof(metadata) = 'object'::text)),
    CONSTRAINT images_metadata_owner_check CHECK ((lower((metadata ->> 'userAddress'::text)) = user_address)),
    CONSTRAINT images_review_status_check CHECK ((review_status = ANY (ARRAY['ok'::text, 'flagged'::text, 'rejected'::text]))),
    CONSTRAINT images_user_address_lowercase_check CHECK ((user_address = lower(user_address)))
);


--
-- Name: withheld_blobs; Type: TABLE; Schema: camera_reel; Owner: -
--

CREATE TABLE camera_reel.withheld_blobs (
    hash text NOT NULL,
    image_id uuid NOT NULL,
    reason text NOT NULL,
    withheld_at timestamp without time zone NOT NULL,
    CONSTRAINT withheld_blobs_reason_check CHECK ((reason = ANY (ARRAY['rejected'::text, 'deleted'::text])))
);


--
-- Name: actions; Type: TABLE; Schema: chat; Owner: -
--

CREATE TABLE chat.actions (
    id uuid NOT NULL,
    wallet text NOT NULL,
    operation text NOT NULL,
    prepared jsonb NOT NULL,
    state text DEFAULT 'pending'::text NOT NULL,
    expires_at timestamp without time zone NOT NULL,
    created_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    CONSTRAINT actions_state_check CHECK ((state = ANY (ARRAY['pending'::text, 'submitted'::text, 'read'::text]))),
    CONSTRAINT actions_wallet_check CHECK ((wallet ~ '^0x[0-9a-f]{40}$'::text))
);


--
-- Name: channels; Type: TABLE; Schema: chat; Owner: -
--

CREATE TABLE chat.channels (
    community_id uuid NOT NULL,
    name text NOT NULL,
    private boolean DEFAULT false NOT NULL,
    config jsonb DEFAULT '{}'::jsonb NOT NULL,
    created_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    updated_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    CONSTRAINT channels_config_object_check CHECK ((jsonb_typeof(config) = 'object'::text)),
    CONSTRAINT channels_general_open_check CHECK (((name <> 'general'::text) OR (NOT private))),
    CONSTRAINT channels_name_check CHECK ((name ~ '^[a-z0-9-]{1,32}$'::text))
);


--
-- Name: community_activity; Type: TABLE; Schema: chat; Owner: -
--

CREATE TABLE chat.community_activity (
    seq bigint NOT NULL,
    community_id uuid NOT NULL,
    kind text NOT NULL,
    object text NOT NULL,
    actor text NOT NULL,
    summary text NOT NULL,
    created_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    silent boolean DEFAULT false NOT NULL,
    CONSTRAINT community_activity_actor_check CHECK (((actor = ''::text) OR (actor ~ '^0x[0-9a-f]{40}$'::text))),
    CONSTRAINT community_activity_kind_check CHECK ((kind = ANY (ARRAY['announcement'::text, 'hangout'::text, 'lounge'::text]))),
    CONSTRAINT community_activity_summary_length_check CHECK ((char_length(summary) <= 140))
);


--
-- Name: community_activity_seq_seq; Type: SEQUENCE; Schema: chat; Owner: -
--

ALTER TABLE chat.community_activity ALTER COLUMN seq ADD GENERATED ALWAYS AS IDENTITY (
    SEQUENCE NAME chat.community_activity_seq_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1
);


--
-- Name: community_live; Type: TABLE; Schema: chat; Owner: -
--

CREATE TABLE chat.community_live (
    community_id uuid NOT NULL,
    session_started_at timestamp without time zone NOT NULL,
    seen_at timestamp without time zone NOT NULL,
    observer text NOT NULL,
    CONSTRAINT community_live_observer_check CHECK ((observer ~ '^0x[0-9a-f]{40}$'::text)),
    CONSTRAINT community_live_session_order_check CHECK ((session_started_at <= seen_at))
);


--
-- Name: messages; Type: TABLE; Schema: chat; Owner: -
--

CREATE TABLE chat.messages (
    seq bigint NOT NULL,
    id uuid NOT NULL,
    community_id uuid NOT NULL,
    channel text NOT NULL,
    wallet text NOT NULL,
    text text NOT NULL,
    scene jsonb,
    created_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    removed_at timestamp without time zone,
    CONSTRAINT messages_scene_object_check CHECK (((scene IS NULL) OR (jsonb_typeof(scene) = 'object'::text))),
    CONSTRAINT messages_text_length_check CHECK ((char_length(text) <= 4000)),
    CONSTRAINT messages_wallet_check CHECK ((wallet ~ '^0x[0-9a-f]{40}$'::text))
);


--
-- Name: messages_seq_seq; Type: SEQUENCE; Schema: chat; Owner: -
--

ALTER TABLE chat.messages ALTER COLUMN seq ADD GENERATED ALWAYS AS IDENTITY (
    SEQUENCE NAME chat.messages_seq_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1
);


--
-- Name: pins; Type: TABLE; Schema: chat; Owner: -
--

CREATE TABLE chat.pins (
    message_id uuid NOT NULL,
    pinned_by text NOT NULL,
    pinned_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    CONSTRAINT pins_pinned_by_check CHECK ((pinned_by ~ '^0x[0-9a-f]{40}$'::text))
);


--
-- Name: reactions; Type: TABLE; Schema: chat; Owner: -
--

CREATE TABLE chat.reactions (
    message_id uuid NOT NULL,
    wallet text NOT NULL,
    emoji text NOT NULL,
    created_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    CONSTRAINT reactions_emoji_check CHECK ((emoji = ANY (ARRAY['heart'::text, 'celebrate'::text]))),
    CONSTRAINT reactions_wallet_check CHECK ((wallet ~ '^0x[0-9a-f]{40}$'::text))
);


--
-- Name: replies; Type: TABLE; Schema: chat; Owner: -
--

CREATE TABLE chat.replies (
    seq bigint NOT NULL,
    id uuid NOT NULL,
    parent_id uuid NOT NULL,
    wallet text NOT NULL,
    text text NOT NULL,
    created_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    CONSTRAINT replies_text_length_check CHECK ((char_length(text) <= 4000)),
    CONSTRAINT replies_wallet_check CHECK ((wallet ~ '^0x[0-9a-f]{40}$'::text))
);


--
-- Name: replies_seq_seq; Type: SEQUENCE; Schema: chat; Owner: -
--

ALTER TABLE chat.replies ALTER COLUMN seq ADD GENERATED ALWAYS AS IDENTITY (
    SEQUENCE NAME chat.replies_seq_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1
);


--
-- Name: reports; Type: TABLE; Schema: chat; Owner: -
--

CREATE TABLE chat.reports (
    message_id uuid NOT NULL,
    wallet text NOT NULL,
    created_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    CONSTRAINT reports_wallet_check CHECK ((wallet ~ '^0x[0-9a-f]{40}$'::text))
);


--
-- Name: active_pointers; Type: TABLE; Schema: content; Owner: -
--

CREATE TABLE content.active_pointers (
    pointer character varying NOT NULL,
    entity_id text NOT NULL,
    entity_type text
);


--
-- Name: content_files; Type: TABLE; Schema: content; Owner: -
--

CREATE TABLE content.content_files (
    deployment integer NOT NULL,
    content_hash text NOT NULL,
    key text NOT NULL
);


--
-- Name: denylist; Type: TABLE; Schema: content; Owner: -
--

CREATE TABLE content.denylist (
    id text NOT NULL,
    added_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    added_by text NOT NULL,
    CONSTRAINT denylist_id_check CHECK (((id <> ''::text) AND (octet_length(id) <= 512)))
);


--
-- Name: deployments; Type: TABLE; Schema: content; Owner: -
--

CREATE TABLE content.deployments (
    id integer NOT NULL,
    deployer_address text NOT NULL,
    version text NOT NULL,
    entity_type text NOT NULL,
    entity_id text NOT NULL,
    entity_metadata json,
    entity_timestamp timestamp without time zone NOT NULL,
    entity_pointers text[] NOT NULL,
    local_timestamp timestamp without time zone NOT NULL,
    auth_chain json NOT NULL,
    deleter_deployment integer
);


--
-- Name: deployments_id_seq; Type: SEQUENCE; Schema: content; Owner: -
--

CREATE SEQUENCE content.deployments_id_seq
    AS integer
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: deployments_id_seq; Type: SEQUENCE OWNED BY; Schema: content; Owner: -
--

ALTER SEQUENCE content.deployments_id_seq OWNED BY content.deployments.id;


--
-- Name: failed_deployments; Type: TABLE; Schema: content; Owner: -
--

CREATE TABLE content.failed_deployments (
    entity_id text NOT NULL,
    entity_type text NOT NULL,
    failure_time timestamp without time zone NOT NULL,
    reason text NOT NULL,
    auth_chain json NOT NULL,
    error_description text NOT NULL,
    snapshot_hash text NOT NULL,
    retry_count integer DEFAULT 0 NOT NULL,
    next_retry_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL
);


--
-- Name: forward_outbox; Type: TABLE; Schema: content; Owner: -
--

CREATE TABLE content.forward_outbox (
    id bigint NOT NULL,
    entity_id text NOT NULL,
    target text NOT NULL,
    status text DEFAULT 'queued'::text NOT NULL,
    attempts integer DEFAULT 0 NOT NULL,
    next_attempt_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    last_error text,
    upstream_status smallint,
    created_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    updated_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    CONSTRAINT forward_outbox_attempts_check CHECK ((attempts >= 0)),
    CONSTRAINT forward_outbox_status_check CHECK ((status = ANY (ARRAY['queued'::text, 'sending'::text, 'accepted'::text, 'rejected'::text, 'failed'::text, 'superseded'::text]))),
    CONSTRAINT forward_outbox_upstream_status_check CHECK (((upstream_status >= 100) AND (upstream_status <= 599)))
);


--
-- Name: forward_outbox_id_seq; Type: SEQUENCE; Schema: content; Owner: -
--

ALTER TABLE content.forward_outbox ALTER COLUMN id ADD GENERATED ALWAYS AS IDENTITY (
    SEQUENCE NAME content.forward_outbox_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1
);


--
-- Name: local_entities; Type: TABLE; Schema: content; Owner: -
--

CREATE TABLE content.local_entities (
    entity_id text NOT NULL,
    signer text NOT NULL,
    origin text DEFAULT 'land-publish'::text NOT NULL,
    published_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    tombstoned_at timestamp without time zone
);


--
-- Name: processed_snapshots; Type: TABLE; Schema: content; Owner: -
--

CREATE TABLE content.processed_snapshots (
    hash text NOT NULL,
    process_time timestamp without time zone NOT NULL
);


--
-- Name: server_sync_cursors; Type: TABLE; Schema: content; Owner: -
--

CREATE TABLE content.server_sync_cursors (
    server_url text NOT NULL,
    cursor_ms bigint NOT NULL,
    updated_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL
);


--
-- Name: snapshots; Type: TABLE; Schema: content; Owner: -
--

CREATE TABLE content.snapshots (
    hash text,
    init_timestamp timestamp without time zone NOT NULL,
    end_timestamp timestamp without time zone NOT NULL,
    replaced_hashes text[] NOT NULL,
    number_of_entities integer NOT NULL,
    generation_time timestamp without time zone NOT NULL
);


--
-- Name: system_properties; Type: TABLE; Schema: content; Owner: -
--

CREATE TABLE content.system_properties (
    key text NOT NULL,
    value text NOT NULL
);


--
-- Name: authorization_lines; Type: TABLE; Schema: credits; Owner: -
--

CREATE TABLE credits.authorization_lines (
    authorization_id text NOT NULL,
    "position" integer NOT NULL,
    quoted_cents bigint NOT NULL,
    charged_cents bigint NOT NULL,
    sale_kind text NOT NULL,
    asset_kind text NOT NULL,
    asset_chain_id bigint NOT NULL,
    asset_contract text NOT NULL,
    asset_item_id numeric(78,0),
    asset_token_id numeric(78,0),
    trade_id text,
    contract_address text,
    item_id text,
    registered_name text,
    CONSTRAINT authorization_lines_asset_contract_check CHECK ((asset_contract ~ '^0x[0-9a-f]{40}$'::text)),
    CONSTRAINT authorization_lines_asset_kind_check CHECK ((asset_kind = ANY (ARRAY['wearable'::text, 'emote'::text, 'name'::text]))),
    CONSTRAINT authorization_lines_charged_check CHECK (((charged_cents >= quoted_cents) AND ((charged_cents % (10)::bigint) = 0))),
    CONSTRAINT authorization_lines_position_check CHECK ((("position" >= 0) AND ("position" < 30))),
    CONSTRAINT authorization_lines_quoted_check CHECK ((quoted_cents > 0)),
    CONSTRAINT authorization_lines_sale_kind_check CHECK ((sale_kind = ANY (ARRAY['primary_mint'::text, 'primary_name_registration'::text])))
);


--
-- Name: authorizations; Type: TABLE; Schema: credits; Owner: -
--

CREATE TABLE credits.authorizations (
    id text NOT NULL,
    address text NOT NULL,
    usd_cents bigint NOT NULL,
    amount_wei numeric(78,0) NOT NULL,
    trade_id text,
    contract_address text,
    item_id text,
    source text,
    status text DEFAULT 'authorized'::text NOT NULL,
    idempotency_key text,
    expires_at timestamp without time zone NOT NULL,
    created_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    chain_id bigint,
    contract text,
    credit_hash text,
    quoted_cents bigint,
    oracle_rate numeric(78,0),
    oracle_decimals smallint,
    submitted_tx_hash text,
    submitted_at timestamp without time zone,
    retired_at timestamp without time zone,
    consumed_at timestamp without time zone,
    CONSTRAINT authorizations_address_check CHECK ((address ~ '^0x[0-9a-f]{40}$'::text)),
    CONSTRAINT authorizations_credit_hash_check CHECK (((credit_hash IS NULL) OR (credit_hash ~ '^0x[0-9a-f]{64}$'::text))),
    CONSTRAINT authorizations_status_check CHECK ((status = ANY (ARRAY['authorized'::text, 'retired'::text, 'released'::text, 'consumed'::text, 'expired'::text]))),
    CONSTRAINT authorizations_submitted_tx_check CHECK (((submitted_tx_hash IS NULL) OR (submitted_tx_hash ~ '^0x[0-9a-f]{64}$'::text))),
    CONSTRAINT authorizations_usd_check CHECK ((usd_cents > 0))
);


--
-- Name: captcha_challenges; Type: TABLE; Schema: credits; Owner: -
--

CREATE TABLE credits.captcha_challenges (
    id bigint NOT NULL,
    address text NOT NULL,
    answer_x numeric NOT NULL,
    expires_at timestamp without time zone NOT NULL,
    consumed_at timestamp without time zone,
    CONSTRAINT captcha_challenges_address_check CHECK ((address ~ '^0x[0-9a-f]{40}$'::text))
);


--
-- Name: captcha_challenges_id_seq; Type: SEQUENCE; Schema: credits; Owner: -
--

ALTER TABLE credits.captcha_challenges ALTER COLUMN id ADD GENERATED ALWAYS AS IDENTITY (
    SEQUENCE NAME credits.captcha_challenges_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1
);


--
-- Name: cart_items; Type: TABLE; Schema: credits; Owner: -
--

CREATE TABLE credits.cart_items (
    id bigint NOT NULL,
    cart_id bigint NOT NULL,
    collection text NOT NULL,
    item_id text NOT NULL,
    urn text NOT NULL,
    category text NOT NULL,
    qty integer DEFAULT 1 NOT NULL,
    unit_price_credits numeric NOT NULL,
    added_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    CONSTRAINT cart_items_collection_check CHECK ((collection ~ '^0x[0-9a-f]{40}$'::text)),
    CONSTRAINT cart_items_price_check CHECK ((unit_price_credits >= (0)::numeric)),
    CONSTRAINT cart_items_qty_check CHECK ((qty > 0))
);


--
-- Name: cart_items_id_seq; Type: SEQUENCE; Schema: credits; Owner: -
--

ALTER TABLE credits.cart_items ALTER COLUMN id ADD GENERATED ALWAYS AS IDENTITY (
    SEQUENCE NAME credits.cart_items_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1
);


--
-- Name: carts; Type: TABLE; Schema: credits; Owner: -
--

CREATE TABLE credits.carts (
    id bigint NOT NULL,
    address text NOT NULL,
    created_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    updated_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    CONSTRAINT carts_address_check CHECK ((address ~ '^0x[0-9a-f]{40}$'::text))
);


--
-- Name: carts_id_seq; Type: SEQUENCE; Schema: credits; Owner: -
--

ALTER TABLE credits.carts ALTER COLUMN id ADD GENERATED ALWAYS AS IDENTITY (
    SEQUENCE NAME credits.carts_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1
);


--
-- Name: checkouts; Type: TABLE; Schema: credits; Owner: -
--

CREATE TABLE credits.checkouts (
    id bigint NOT NULL,
    idempotency_key text NOT NULL,
    address text NOT NULL,
    total_credits numeric DEFAULT 0 NOT NULL,
    status text DEFAULT 'reserving'::text NOT NULL,
    ledger_id bigint,
    created_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    updated_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    CONSTRAINT checkouts_address_check CHECK ((address ~ '^0x[0-9a-f]{40}$'::text)),
    CONSTRAINT checkouts_status_check CHECK ((status = ANY (ARRAY['reserving'::text, 'debited'::text, 'fulfilling'::text, 'fulfilled'::text, 'failed'::text, 'reversing'::text, 'reversed'::text]))),
    CONSTRAINT checkouts_total_check CHECK ((total_credits >= (0)::numeric))
);


--
-- Name: checkouts_id_seq; Type: SEQUENCE; Schema: credits; Owner: -
--

ALTER TABLE credits.checkouts ALTER COLUMN id ADD GENERATED ALWAYS AS IDENTITY (
    SEQUENCE NAME credits.checkouts_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1
);


--
-- Name: fulfillment_outbox; Type: TABLE; Schema: credits; Owner: -
--

CREATE TABLE credits.fulfillment_outbox (
    id bigint NOT NULL,
    checkout_id bigint NOT NULL,
    item_id text NOT NULL,
    collection text NOT NULL,
    urn text NOT NULL,
    token_id text,
    trade_id text,
    basis_wei numeric(78,0),
    unit_price_credits numeric NOT NULL,
    mode text NOT NULL,
    status text DEFAULT 'pending'::text NOT NULL,
    attempts integer DEFAULT 0 NOT NULL,
    last_error text,
    external_ref text,
    created_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    updated_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    CONSTRAINT fulfillment_outbox_attempts_check CHECK ((attempts >= 0)),
    CONSTRAINT fulfillment_outbox_collection_check CHECK ((collection ~ '^0x[0-9a-f]{40}$'::text)),
    CONSTRAINT fulfillment_outbox_mode_check CHECK ((mode = ANY (ARRAY['primary'::text, 'secondary'::text, 'trade'::text]))),
    CONSTRAINT fulfillment_outbox_status_check CHECK ((status = ANY (ARRAY['pending'::text, 'sent'::text, 'confirmed'::text, 'failed'::text])))
);


--
-- Name: fulfillment_outbox_id_seq; Type: SEQUENCE; Schema: credits; Owner: -
--

ALTER TABLE credits.fulfillment_outbox ALTER COLUMN id ADD GENERATED ALWAYS AS IDENTITY (
    SEQUENCE NAME credits.fulfillment_outbox_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1
);


--
-- Name: idempotency; Type: TABLE; Schema: credits; Owner: -
--

CREATE TABLE credits.idempotency (
    scope text NOT NULL,
    idempotency_key text NOT NULL,
    address text NOT NULL,
    amount numeric NOT NULL,
    applied numeric,
    available numeric NOT NULL,
    tx_ref text,
    actor text,
    created_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    CONSTRAINT idempotency_scope_check CHECK ((scope = ANY (ARRAY['grant'::text, 'spend'::text, 'refund'::text])))
);


--
-- Name: ledger; Type: TABLE; Schema: credits; Owner: -
--

CREATE TABLE credits.ledger (
    id bigint NOT NULL,
    address text NOT NULL,
    kind text NOT NULL,
    bucket text DEFAULT 'paid'::text NOT NULL,
    amount numeric NOT NULL,
    tx_ref text,
    captcha_ok boolean DEFAULT false NOT NULL,
    admin_audit_id bigint,
    created_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    CONSTRAINT ledger_address_check CHECK ((address ~ '^0x[0-9a-f]{40}$'::text)),
    CONSTRAINT ledger_amount_check CHECK ((amount >= (0)::numeric)),
    CONSTRAINT ledger_bucket_check CHECK ((bucket = ANY (ARRAY['earned'::text, 'paid'::text]))),
    CONSTRAINT ledger_kind_check CHECK ((kind = ANY (ARRAY['grant'::text, 'claim'::text, 'expire'::text, 'consume'::text, 'spend'::text, 'purchase'::text, 'refund'::text])))
);


--
-- Name: ledger_id_seq; Type: SEQUENCE; Schema: credits; Owner: -
--

ALTER TABLE credits.ledger ALTER COLUMN id ADD GENERATED ALWAYS AS IDENTITY (
    SEQUENCE NAME credits.ledger_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1
);


--
-- Name: packs; Type: TABLE; Schema: credits; Owner: -
--

CREATE TABLE credits.packs (
    sku text NOT NULL,
    title text NOT NULL,
    credits numeric NOT NULL,
    price_cents bigint NOT NULL,
    currency text NOT NULL,
    active boolean DEFAULT true NOT NULL,
    sort_order integer DEFAULT 0 NOT NULL,
    created_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    CONSTRAINT packs_credits_check CHECK ((credits > (0)::numeric)),
    CONSTRAINT packs_price_check CHECK ((price_cents >= 0))
);


--
-- Name: purchases; Type: TABLE; Schema: credits; Owner: -
--

CREATE TABLE credits.purchases (
    id bigint NOT NULL,
    address text NOT NULL,
    sku text NOT NULL,
    credits numeric NOT NULL,
    amount_cents bigint NOT NULL,
    currency text NOT NULL,
    method text DEFAULT 'card'::text NOT NULL,
    status text DEFAULT 'pending'::text NOT NULL,
    order_id text,
    stripe_payment_intent text,
    stripe_checkout_session text,
    stripe_event_id text,
    refunded_cents bigint DEFAULT 0 NOT NULL,
    revoked_credits numeric DEFAULT 0 NOT NULL,
    failure_reason text,
    created_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    updated_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    quantity integer DEFAULT 1 NOT NULL,
    source text,
    CONSTRAINT purchases_address_check CHECK ((address ~ '^0x[0-9a-f]{40}$'::text)),
    CONSTRAINT purchases_quantity_check CHECK ((quantity >= 1)),
    CONSTRAINT purchases_refunded_bound_check CHECK (((refunded_cents >= 0) AND (refunded_cents <= amount_cents))),
    CONSTRAINT purchases_revoked_bound_check CHECK (((revoked_credits >= (0)::numeric) AND (revoked_credits <= credits))),
    CONSTRAINT purchases_source_check CHECK ((source = ANY (ARRAY['website'::text, 'client'::text, 'builder'::text]))),
    CONSTRAINT purchases_status_check CHECK ((status = ANY (ARRAY['pending'::text, 'paid'::text, 'refunded'::text, 'disputed'::text, 'failed'::text, 'abandoned'::text])))
);


--
-- Name: purchases_id_seq; Type: SEQUENCE; Schema: credits; Owner: -
--

ALTER TABLE credits.purchases ALTER COLUMN id ADD GENERATED ALWAYS AS IDENTITY (
    SEQUENCE NAME credits.purchases_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1
);


--
-- Name: revocations; Type: TABLE; Schema: credits; Owner: -
--

CREATE TABLE credits.revocations (
    credit_hash text NOT NULL,
    authorization_id text NOT NULL,
    chain_id bigint NOT NULL,
    contract text NOT NULL,
    reason text NOT NULL,
    status text DEFAULT 'pending'::text NOT NULL,
    tx_hash text,
    attempts integer DEFAULT 0 NOT NULL,
    last_error text,
    created_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    updated_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    CONSTRAINT revocations_attempts_check CHECK ((attempts >= 0)),
    CONSTRAINT revocations_contract_check CHECK ((contract ~ '^0x[0-9a-f]{40}$'::text)),
    CONSTRAINT revocations_credit_hash_check CHECK ((credit_hash ~ '^0x[0-9a-f]{64}$'::text)),
    CONSTRAINT revocations_status_check CHECK ((status = ANY (ARRAY['pending'::text, 'sent'::text, 'confirmed'::text, 'failed'::text])))
);


--
-- Name: stripe_events; Type: TABLE; Schema: credits; Owner: -
--

CREATE TABLE credits.stripe_events (
    event_id text NOT NULL,
    type text NOT NULL,
    payload jsonb,
    received_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    processed_at timestamp without time zone
);


--
-- Name: usage_index_cursor; Type: TABLE; Schema: credits; Owner: -
--

CREATE TABLE credits.usage_index_cursor (
    chain_id bigint NOT NULL,
    contract text NOT NULL,
    last_block bigint NOT NULL,
    folded_block bigint,
    folded_block_time timestamp without time zone,
    folded_at timestamp without time zone,
    full_replay_started boolean DEFAULT false NOT NULL,
    updated_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    CONSTRAINT usage_index_cursor_contract_check CHECK ((contract ~ '^0x[0-9a-f]{40}$'::text)),
    CONSTRAINT usage_index_cursor_folded_check CHECK (((folded_block IS NULL) OR (folded_block <= last_block)))
);


--
-- Name: usage_settlements; Type: TABLE; Schema: credits; Owner: -
--

CREATE TABLE credits.usage_settlements (
    chain_id bigint NOT NULL,
    block_number bigint NOT NULL,
    log_index integer NOT NULL,
    authorization_id text NOT NULL,
    address text NOT NULL,
    kind text NOT NULL,
    value_wei numeric(78,0) NOT NULL,
    debited_credits numeric NOT NULL,
    tx_ref text NOT NULL,
    settled_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    CONSTRAINT usage_settlements_address_check CHECK ((address ~ '^0x[0-9a-f]{40}$'::text)),
    CONSTRAINT usage_settlements_debited_check CHECK ((debited_credits >= (0)::numeric)),
    CONSTRAINT usage_settlements_kind_check CHECK ((kind = ANY (ARRAY['purchase'::text, 'remainder'::text])))
);


--
-- Name: usages; Type: TABLE; Schema: credits; Owner: -
--

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
    indexed_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    CONSTRAINT usages_contract_check CHECK ((contract ~ '^0x[0-9a-f]{40}$'::text)),
    CONSTRAINT usages_credit_id_check CHECK ((credit_id ~ '^0x[0-9a-f]{64}$'::text)),
    CONSTRAINT usages_salt_check CHECK ((salt ~ '^0x[0-9a-f]{64}$'::text)),
    CONSTRAINT usages_sender_check CHECK ((sender ~ '^0x[0-9a-f]{40}$'::text)),
    CONSTRAINT usages_tx_hash_check CHECK ((tx_hash ~ '^0x[0-9a-f]{64}$'::text))
);


--
-- Name: wallets; Type: TABLE; Schema: credits; Owner: -
--

CREATE TABLE credits.wallets (
    address text NOT NULL,
    has_started_program boolean DEFAULT true NOT NULL,
    available numeric DEFAULT 0 NOT NULL,
    earned_available numeric DEFAULT 0 NOT NULL,
    is_blocked_for_claiming boolean DEFAULT false NOT NULL,
    created_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    updated_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    CONSTRAINT wallets_address_check CHECK ((address ~ '^0x[0-9a-f]{40}$'::text)),
    CONSTRAINT wallets_available_check CHECK ((available >= (0)::numeric)),
    CONSTRAINT wallets_earned_bound_check CHECK (((earned_available >= (0)::numeric) AND (earned_available <= available)))
);


--
-- Name: price_override_audit; Type: TABLE; Schema: economy; Owner: -
--

CREATE TABLE economy.price_override_audit (
    id bigint NOT NULL,
    action text NOT NULL,
    token_id text NOT NULL,
    vs_currency text NOT NULL,
    value numeric,
    note text,
    admin text NOT NULL,
    detail jsonb DEFAULT '{}'::jsonb NOT NULL,
    created_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    CONSTRAINT price_override_audit_action_known CHECK ((action = ANY (ARRAY['override.set'::text, 'override.clear'::text])))
);


--
-- Name: price_override_audit_id_seq; Type: SEQUENCE; Schema: economy; Owner: -
--

ALTER TABLE economy.price_override_audit ALTER COLUMN id ADD GENERATED ALWAYS AS IDENTITY (
    SEQUENCE NAME economy.price_override_audit_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1
);


--
-- Name: price_overrides; Type: TABLE; Schema: economy; Owner: -
--

CREATE TABLE economy.price_overrides (
    token_id text NOT NULL,
    vs_currency text NOT NULL,
    value numeric NOT NULL,
    note text,
    updated_by text,
    updated_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    CONSTRAINT price_overrides_key_lowercase CHECK (((token_id = lower(token_id)) AND (vs_currency = lower(vs_currency))))
);


--
-- Name: price_snapshots; Type: TABLE; Schema: economy; Owner: -
--

CREATE TABLE economy.price_snapshots (
    id bigint NOT NULL,
    taken_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    source text DEFAULT 'coingecko'::text NOT NULL,
    source_updated_at timestamp without time zone,
    mana_usd numeric(20,8) NOT NULL,
    mana_eth numeric(28,18),
    mana_btc numeric(28,18),
    mana_matic numeric(28,18),
    matic_usd numeric(20,8),
    mana_market_cap_usd numeric(20,2),
    mana_volume_24h_usd numeric(20,2),
    mana_price_change_24h_pct numeric(10,4),
    CONSTRAINT price_snapshots_mana_usd_positive CHECK ((mana_usd > (0)::numeric))
);


--
-- Name: price_snapshots_id_seq; Type: SEQUENCE; Schema: economy; Owner: -
--

ALTER TABLE economy.price_snapshots ALTER COLUMN id ADD GENERATED ALWAYS AS IDENTITY (
    SEQUENCE NAME economy.price_snapshots_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1
);


--
-- Name: transactions; Type: TABLE; Schema: economy; Owner: -
--

CREATE TABLE economy.transactions (
    id bigint NOT NULL,
    tx_hash text,
    session_id text,
    user_address character varying(42) NOT NULL,
    chain_id integer NOT NULL,
    contract_address character varying(42) NOT NULL,
    created_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    CONSTRAINT transactions_contract_lowercase CHECK (((contract_address)::text = lower((contract_address)::text))),
    CONSTRAINT transactions_reserved_or_relayed CHECK (((tx_hash IS NOT NULL) OR (session_id IS NOT NULL))),
    CONSTRAINT transactions_user_lowercase CHECK (((user_address)::text = lower((user_address)::text)))
);


--
-- Name: transactions_id_seq; Type: SEQUENCE; Schema: economy; Owner: -
--

ALTER TABLE economy.transactions ALTER COLUMN id ADD GENERATED ALWAYS AS IDENTITY (
    SEQUENCE NAME economy.transactions_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1
);


--
-- Name: event; Type: TABLE; Schema: explore; Owner: -
--

CREATE TABLE explore.event (
    id text NOT NULL,
    name text NOT NULL,
    start_at timestamp without time zone,
    finish_at timestamp without time zone,
    next_start_at timestamp without time zone,
    next_finish_at timestamp without time zone,
    duration_ms bigint,
    recurrent boolean DEFAULT false NOT NULL,
    highlighted boolean DEFAULT false NOT NULL,
    trending boolean DEFAULT false NOT NULL,
    approved boolean DEFAULT false NOT NULL,
    attending boolean,
    community_id text,
    user_creator text,
    coordinates_x integer,
    coordinates_y integer,
    description text,
    raw jsonb NOT NULL,
    fetched_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    origin text DEFAULT 'upstream'::text NOT NULL,
    CONSTRAINT event_origin_known CHECK ((origin = ANY (ARRAY['upstream'::text, 'local'::text])))
);


--
-- Name: event_attendance_local; Type: TABLE; Schema: explore; Owner: -
--

CREATE TABLE explore.event_attendance_local (
    event_id text NOT NULL,
    signer text NOT NULL,
    signed_payload jsonb DEFAULT '{}'::jsonb NOT NULL,
    action text NOT NULL,
    signed_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    CONSTRAINT event_attendance_local_signer_lowercase CHECK ((signer = lower(signer)))
);


--
-- Name: event_moderation; Type: TABLE; Schema: explore; Owner: -
--

CREATE TABLE explore.event_moderation (
    event_id text NOT NULL,
    approved boolean,
    rejected boolean,
    highlighted boolean,
    trending boolean,
    hidden boolean,
    approved_by text,
    rejected_by text,
    rejection_reason text,
    deleted_by text,
    deleted_reason text,
    set_by text NOT NULL,
    set_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    CONSTRAINT event_moderation_decided CHECK (((approved IS NOT NULL) OR (rejected IS NOT NULL) OR (highlighted IS NOT NULL) OR (trending IS NOT NULL) OR (hidden IS NOT NULL)))
);


--
-- Name: event_profile_settings; Type: TABLE; Schema: explore; Owner: -
--

CREATE TABLE explore.event_profile_settings (
    "user" text NOT NULL,
    email text,
    email_verified boolean DEFAULT false NOT NULL,
    use_local_time boolean DEFAULT true NOT NULL,
    notify_by_email boolean DEFAULT false NOT NULL,
    notify_by_browser boolean DEFAULT false NOT NULL,
    permissions jsonb DEFAULT '[]'::jsonb NOT NULL,
    created_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    updated_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    permissions_granted boolean DEFAULT false NOT NULL,
    CONSTRAINT event_profile_settings_user_lowercase CHECK (("user" = lower("user")))
);


--
-- Name: events_local; Type: TABLE; Schema: explore; Owner: -
--

CREATE TABLE explore.events_local (
    id text NOT NULL,
    signer text NOT NULL,
    signed_payload jsonb DEFAULT '{}'::jsonb NOT NULL,
    signed_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    updated_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL
);


--
-- Name: mirror_state; Type: TABLE; Schema: explore; Owner: -
--

CREATE TABLE explore.mirror_state (
    mirror text NOT NULL,
    source_url text NOT NULL,
    pass_started_at timestamp without time zone,
    next_offset bigint,
    pass_rows bigint DEFAULT 0 NOT NULL,
    last_pass_at timestamp without time zone,
    last_pass_complete boolean,
    last_pass_rows bigint,
    last_pass_swept bigint,
    last_complete_pass_at timestamp without time zone,
    last_complete_pass_rows bigint,
    last_error text,
    updated_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    CONSTRAINT mirror_state_known_mirror CHECK ((mirror = ANY (ARRAY['places'::text, 'worlds'::text, 'events'::text])))
);


--
-- Name: moderators; Type: TABLE; Schema: explore; Owner: -
--

CREATE TABLE explore.moderators (
    address text NOT NULL,
    added_by text,
    added_at bigint NOT NULL
);


--
-- Name: opensea_nft; Type: TABLE; Schema: explore; Owner: -
--

CREATE TABLE explore.opensea_nft (
    chain text NOT NULL,
    contract_address text NOT NULL,
    token_id text NOT NULL,
    document json NOT NULL,
    source text NOT NULL,
    resolved_at timestamp without time zone NOT NULL,
    CONSTRAINT opensea_nft_chain_known CHECK ((chain = ANY (ARRAY['ethereum'::text, 'matic'::text, 'polygon'::text]))),
    CONSTRAINT opensea_nft_contract_lowercase CHECK ((contract_address ~ '^0x[0-9a-f]{40}$'::text)),
    CONSTRAINT opensea_nft_source_known CHECK ((source = ANY (ARRAY['local'::text, 'cryptokitties'::text, 'chain'::text, 'fallback'::text]))),
    CONSTRAINT opensea_nft_token_digits CHECK ((token_id ~ '^[0-9]{1,78}$'::text))
);


--
-- Name: place; Type: TABLE; Schema: explore; Owner: -
--

CREATE TABLE explore.place (
    id text NOT NULL,
    base_position text NOT NULL,
    title text,
    description text,
    creator_address text,
    content_rating text,
    categories text[] DEFAULT '{}'::text[] NOT NULL,
    likes integer DEFAULT 0 NOT NULL,
    dislikes integer DEFAULT 0 NOT NULL,
    favorites integer DEFAULT 0 NOT NULL,
    deployed_at timestamp without time zone,
    disabled boolean DEFAULT false NOT NULL,
    highlighted boolean DEFAULT false NOT NULL,
    raw jsonb NOT NULL,
    fetched_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    world_name text GENERATED ALWAYS AS (NULLIF(btrim((raw ->> 'world_name'::text)), ''::text)) STORED,
    world boolean GENERATED ALWAYS AS ((
CASE lower(btrim((raw ->> 'world'::text)))
    WHEN 'true'::text THEN true
    WHEN 't'::text THEN true
    WHEN 'yes'::text THEN true
    WHEN 'on'::text THEN true
    WHEN '1'::text THEN true
    ELSE false
END AND (NULLIF(btrim((raw ->> 'world_name'::text)), ''::text) IS NOT NULL))) STORED NOT NULL,
    description_plain text GENERATED ALWAYS AS (
CASE
    WHEN ((strpos(COALESCE(description, ''::text), '<'::text) = 0) AND (strpos(COALESCE(description, ''::text), '>'::text) = 0)) THEN COALESCE(description, ''::text)
    ELSE regexp_replace(regexp_replace(regexp_replace(regexp_replace(regexp_replace(regexp_replace(COALESCE(description, ''::text), '</?[a-zA-Z][^>]*>'::text, ''::text, 'g'::text), '</?[a-zA-Z][^>]*>'::text, ''::text, 'g'::text), '</?[a-zA-Z][^>]*>'::text, ''::text, 'g'::text), '</?[a-zA-Z][^>]*>'::text, ''::text, 'g'::text), '</?[a-zA-Z][^>]*>'::text, ''::text, 'g'::text), '[<>]'::text, ''::text, 'g'::text)
END) STORED,
    image_url text GENERATED ALWAYS AS (
CASE
    WHEN ((raw ->> 'image'::text) ~* '^https?://[^[:space:]"''`<>]+$'::text) THEN (raw ->> 'image'::text)
    ELSE NULL::text
END) STORED,
    origin text DEFAULT 'upstream'::text NOT NULL,
    CONSTRAINT place_origin_known CHECK ((origin = ANY (ARRAY['upstream'::text, 'local'::text])))
);


--
-- Name: place_world_local; Type: TABLE; Schema: explore; Owner: -
--

CREATE TABLE explore.place_world_local (
    id text NOT NULL,
    base_position text NOT NULL,
    title text,
    description text,
    creator_address text,
    content_rating text,
    categories text[] DEFAULT '{}'::text[] NOT NULL,
    likes integer DEFAULT 0 NOT NULL,
    dislikes integer DEFAULT 0 NOT NULL,
    favorites integer DEFAULT 0 NOT NULL,
    deployed_at timestamp without time zone,
    disabled boolean DEFAULT false NOT NULL,
    highlighted boolean DEFAULT false NOT NULL,
    raw jsonb NOT NULL,
    fetched_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    world_name text,
    world boolean DEFAULT false NOT NULL,
    description_plain text GENERATED ALWAYS AS (
CASE
    WHEN ((strpos(COALESCE(description, ''::text), '<'::text) = 0) AND (strpos(COALESCE(description, ''::text), '>'::text) = 0)) THEN COALESCE(description, ''::text)
    ELSE regexp_replace(regexp_replace(regexp_replace(regexp_replace(regexp_replace(regexp_replace(COALESCE(description, ''::text), '</?[a-zA-Z][^>]*>'::text, ''::text, 'g'::text), '</?[a-zA-Z][^>]*>'::text, ''::text, 'g'::text), '</?[a-zA-Z][^>]*>'::text, ''::text, 'g'::text), '</?[a-zA-Z][^>]*>'::text, ''::text, 'g'::text), '</?[a-zA-Z][^>]*>'::text, ''::text, 'g'::text), '[<>]'::text, ''::text, 'g'::text)
END) STORED,
    image_url text GENERATED ALWAYS AS (
CASE
    WHEN ((raw ->> 'image'::text) ~* '^https?://[^[:space:]"''`<>]+$'::text) THEN (raw ->> 'image'::text)
    ELSE NULL::text
END) STORED,
    CONSTRAINT place_world_local_world_named CHECK (((world IS FALSE) OR ((world_name IS NOT NULL) AND (btrim(world_name) <> ''::text))))
);


--
-- Name: place_indexed; Type: VIEW; Schema: explore; Owner: -
--

CREATE VIEW explore.place_indexed AS
 SELECT place.id,
    place.base_position,
    place.title,
    place.description,
    place.creator_address,
    place.content_rating,
    place.categories,
    place.likes,
    place.dislikes,
    place.favorites,
    place.deployed_at,
    place.disabled,
    place.highlighted,
    place.raw,
    place.fetched_at,
    place.world_name,
    place.world,
    place.description_plain,
    place.image_url
   FROM explore.place
UNION ALL
 SELECT place_world_local.id,
    place_world_local.base_position,
    place_world_local.title,
    place_world_local.description,
    place_world_local.creator_address,
    place_world_local.content_rating,
    place_world_local.categories,
    place_world_local.likes,
    place_world_local.dislikes,
    place_world_local.favorites,
    place_world_local.deployed_at,
    place_world_local.disabled,
    place_world_local.highlighted,
    place_world_local.raw,
    place_world_local.fetched_at,
    place_world_local.world_name,
    place_world_local.world,
    place_world_local.description_plain,
    place_world_local.image_url
   FROM explore.place_world_local;


--
-- Name: place_interaction_counts; Type: TABLE; Schema: explore; Owner: -
--

CREATE TABLE explore.place_interaction_counts (
    entity_id text NOT NULL,
    favorites integer DEFAULT 0 NOT NULL,
    likes integer DEFAULT 0 NOT NULL,
    dislikes integer DEFAULT 0 NOT NULL,
    updated_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    CONSTRAINT place_interaction_counts_nonnegative CHECK (((favorites >= 0) AND (likes >= 0) AND (dislikes >= 0)))
);


--
-- Name: place_moderation; Type: TABLE; Schema: explore; Owner: -
--

CREATE TABLE explore.place_moderation (
    place_id text NOT NULL,
    disabled boolean,
    highlighted boolean,
    content_rating text,
    reason text,
    set_by text NOT NULL,
    updated_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    CONSTRAINT place_moderation_decided CHECK (((disabled IS NOT NULL) OR (highlighted IS NOT NULL) OR (content_rating IS NOT NULL))),
    CONSTRAINT place_moderation_rating_known CHECK (((content_rating IS NULL) OR (content_rating = ANY (ARRAY['RP'::text, 'E'::text, 'T'::text, 'A'::text, 'R'::text]))))
);


--
-- Name: place_ranking; Type: TABLE; Schema: explore; Owner: -
--

CREATE TABLE explore.place_ranking (
    place_id text NOT NULL,
    "overlay" jsonb NOT NULL,
    set_by text NOT NULL,
    updated_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    CONSTRAINT place_ranking_known_keys CHECK ((("overlay" <> '{}'::jsonb) AND ((("overlay" - 'ranking'::text) - 'exclude_from_ranking'::text) = '{}'::jsonb)))
);


--
-- Name: place_removal_pending; Type: TABLE; Schema: explore; Owner: -
--

CREATE TABLE explore.place_removal_pending (
    event_id uuid DEFAULT gen_random_uuid() NOT NULL,
    place_id text NOT NULL,
    owner_address text,
    world boolean NOT NULL,
    world_name text,
    base_position text NOT NULL,
    positions jsonb NOT NULL,
    removed_at timestamp without time zone DEFAULT timezone('UTC'::text, clock_timestamp()) NOT NULL,
    cleaned_at timestamp without time zone,
    retry_at timestamp without time zone DEFAULT timezone('UTC'::text, clock_timestamp()) NOT NULL,
    attempts integer DEFAULT 0 NOT NULL,
    CONSTRAINT place_removal_pending_attempts_check CHECK ((attempts >= 0))
);


--
-- Name: place_removal_state; Type: TABLE; Schema: explore; Owner: -
--

CREATE TABLE explore.place_removal_state (
    place_id text NOT NULL,
    removed boolean NOT NULL
);


--
-- Name: place_reports; Type: TABLE; Schema: explore; Owner: -
--

CREATE TABLE explore.place_reports (
    id bigint NOT NULL,
    filename text NOT NULL,
    entity_id text,
    reporter text NOT NULL,
    signed_url text NOT NULL,
    upload_token_hash bytea,
    payload jsonb DEFAULT '{}'::jsonb NOT NULL,
    status text DEFAULT 'open'::text NOT NULL,
    resolution text,
    moderator_notes text,
    resolved_by text,
    resolved_at timestamp without time zone,
    created_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    CONSTRAINT place_reports_reporter_lowercase CHECK ((reporter = lower(reporter))),
    CONSTRAINT place_reports_status_known CHECK ((status = ANY (ARRAY['open'::text, 'resolved'::text, 'dismissed'::text, 'actioned'::text]))),
    CONSTRAINT place_reports_upload_token_hash_length CHECK (((upload_token_hash IS NULL) OR (octet_length(upload_token_hash) = 32)))
);


--
-- Name: place_reports_id_seq; Type: SEQUENCE; Schema: explore; Owner: -
--

ALTER TABLE explore.place_reports ALTER COLUMN id ADD GENERATED ALWAYS AS IDENTITY (
    SEQUENCE NAME explore.place_reports_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1
);


--
-- Name: poster; Type: TABLE; Schema: explore; Owner: -
--

CREATE TABLE explore.poster (
    kind text NOT NULL,
    file text NOT NULL,
    blob_cid text NOT NULL,
    content_type text NOT NULL,
    byte_size bigint NOT NULL,
    origin text NOT NULL,
    fetched_at timestamp without time zone NOT NULL,
    CONSTRAINT poster_file_plain CHECK ((file ~ '^[A-Za-z0-9][A-Za-z0-9._-]{0,199}$'::text)),
    CONSTRAINT poster_kind_known CHECK ((kind = ANY (ARRAY['poster'::text, 'poster-vertical'::text]))),
    CONSTRAINT poster_origin_known CHECK ((origin = ANY (ARRAY['upstream'::text, 'local'::text]))),
    CONSTRAINT poster_sized CHECK ((byte_size > 0)),
    CONSTRAINT poster_type_known CHECK ((content_type = ANY (ARRAY['image/png'::text, 'image/jpeg'::text, 'image/gif'::text, 'image/webp'::text, 'application/octet-stream'::text])))
);


--
-- Name: road_positions; Type: TABLE; Schema: explore; Owner: -
--

CREATE TABLE explore.road_positions (
    "position" text NOT NULL
);


--
-- Name: satellite_region; Type: TABLE; Schema: explore; Owner: -
--

CREATE TABLE explore.satellite_region (
    cx integer NOT NULL,
    cy integer NOT NULL,
    variant text NOT NULL,
    blob_cid text NOT NULL,
    byte_size bigint NOT NULL,
    width integer NOT NULL,
    height integer NOT NULL,
    last_modified text,
    etag text,
    fetched_at timestamp without time zone NOT NULL,
    checked_at timestamp without time zone NOT NULL,
    CONSTRAINT satellite_region_in_world CHECK ((((cx >= '-512'::integer) AND (cx <= 512)) AND ((cy >= '-512'::integer) AND (cy <= 512)))),
    CONSTRAINT satellite_region_sized CHECK (((byte_size > 0) AND (width > 0) AND (height > 0))),
    CONSTRAINT satellite_region_variant_known CHECK ((variant = ANY (ARRAY['full'::text, 'thumb'::text])))
);


--
-- Name: schedules_local; Type: TABLE; Schema: explore; Owner: -
--

CREATE TABLE explore.schedules_local (
    id text NOT NULL,
    name text NOT NULL,
    description text,
    image text,
    theme text,
    background jsonb DEFAULT '[]'::jsonb NOT NULL,
    active_since timestamp without time zone,
    active_until timestamp without time zone,
    active boolean DEFAULT true NOT NULL,
    created_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    updated_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL
);


--
-- Name: seen_nonces; Type: TABLE; Schema: explore; Owner: -
--

CREATE TABLE explore.seen_nonces (
    signer text NOT NULL,
    nonce text NOT NULL,
    expires_at bigint NOT NULL
);


--
-- Name: signed_actions; Type: TABLE; Schema: explore; Owner: -
--

CREATE TABLE explore.signed_actions (
    signature_hash text NOT NULL,
    seq bigint NOT NULL,
    scope text NOT NULL,
    signer text NOT NULL,
    subject_id text,
    action_type text NOT NULL,
    message_payload jsonb NOT NULL,
    signed_at bigint NOT NULL,
    received_at bigint NOT NULL,
    origin_peer text,
    CONSTRAINT signed_actions_scope_known CHECK ((scope = ANY (ARRAY['places'::text, 'events'::text]))),
    CONSTRAINT signed_actions_signer_lowercase CHECK ((signer = lower(signer)))
);


--
-- Name: signed_actions_seq_seq; Type: SEQUENCE; Schema: explore; Owner: -
--

ALTER TABLE explore.signed_actions ALTER COLUMN seq ADD GENERATED ALWAYS AS IDENTITY (
    SEQUENCE NAME explore.signed_actions_seq_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1
);


--
-- Name: user_favorites; Type: TABLE; Schema: explore; Owner: -
--

CREATE TABLE explore.user_favorites (
    "user" text NOT NULL,
    entity_id text NOT NULL,
    user_activity double precision DEFAULT 0 NOT NULL,
    created_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL
);


--
-- Name: user_likes; Type: TABLE; Schema: explore; Owner: -
--

CREATE TABLE explore.user_likes (
    "user" text NOT NULL,
    entity_id text NOT NULL,
    "like" boolean NOT NULL,
    user_activity double precision DEFAULT 0 NOT NULL,
    created_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    updated_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL
);


--
-- Name: world_manifest; Type: TABLE; Schema: explore; Owner: -
--

CREATE TABLE explore.world_manifest (
    id smallint NOT NULL,
    blob_cid text NOT NULL,
    byte_size bigint NOT NULL,
    occupied_count integer NOT NULL,
    roads_count integer NOT NULL,
    empty_count integer NOT NULL,
    etag text,
    last_modified text,
    fetched_at timestamp without time zone NOT NULL,
    checked_at timestamp without time zone NOT NULL,
    CONSTRAINT world_manifest_counts CHECK (((occupied_count > 0) AND (roads_count >= 0) AND (empty_count >= 0) AND (byte_size > 0))),
    CONSTRAINT world_manifest_singleton CHECK ((id = 1))
);


--
-- Name: world_scene_place; Type: TABLE; Schema: explore; Owner: -
--

CREATE TABLE explore.world_scene_place (
    world_name text NOT NULL,
    identity uuid NOT NULL,
    entity_id text,
    parcels text[] NOT NULL,
    active boolean DEFAULT true NOT NULL,
    created_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    updated_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    CONSTRAINT world_scene_place_deployment CHECK ((active = (entity_id IS NOT NULL))),
    CONSTRAINT world_scene_place_parcels_nonempty CHECK ((cardinality(parcels) > 0)),
    CONSTRAINT world_scene_place_world_lowercase CHECK (((world_name = lower(world_name)) AND (world_name <> ''::text)))
);


--
-- Name: denylist_users; Type: TABLE; Schema: explore_api; Owner: -
--

CREATE TABLE explore_api.denylist_users (
    wallet text NOT NULL,
    wallet_raw text NOT NULL,
    first_seen_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL
);


--
-- Name: denylist_users_local; Type: TABLE; Schema: explore_api; Owner: -
--

CREATE TABLE explore_api.denylist_users_local (
    wallet text NOT NULL,
    present boolean NOT NULL,
    updated_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL
);


--
-- Name: feature_flags; Type: TABLE; Schema: explore_api; Owner: -
--

CREATE TABLE explore_api.feature_flags (
    app text NOT NULL,
    document jsonb NOT NULL,
    etag text,
    fetched_at timestamp without time zone NOT NULL
);


--
-- Name: feature_flags_local; Type: TABLE; Schema: explore_api; Owner: -
--

CREATE TABLE explore_api.feature_flags_local (
    app text NOT NULL,
    name text NOT NULL,
    value boolean NOT NULL,
    updated_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL
);


--
-- Name: lists; Type: TABLE; Schema: explore_api; Owner: -
--

CREATE TABLE explore_api.lists (
    list text NOT NULL,
    value text NOT NULL,
    first_seen_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL
);


--
-- Name: lists_local; Type: TABLE; Schema: explore_api; Owner: -
--

CREATE TABLE explore_api.lists_local (
    list text NOT NULL,
    value text NOT NULL,
    present boolean NOT NULL,
    updated_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL
);


--
-- Name: map_grid; Type: TABLE; Schema: explore_api; Owner: -
--

CREATE TABLE explore_api.map_grid (
    id smallint NOT NULL,
    last_updated_at bigint NOT NULL,
    etag text,
    tile_count integer NOT NULL,
    body_gzip bytea NOT NULL,
    fetched_at timestamp without time zone NOT NULL,
    CONSTRAINT map_grid_id_check CHECK ((id = 1))
);


--
-- Name: mirror_passes; Type: TABLE; Schema: explore_api; Owner: -
--

CREATE TABLE explore_api.mirror_passes (
    mirror text NOT NULL,
    passes bigint DEFAULT 0 NOT NULL,
    items bigint DEFAULT 0 NOT NULL,
    cursor text,
    started_at timestamp without time zone,
    completed_at timestamp without time zone,
    last_error text
);


--
-- Name: budgets; Type: TABLE; Schema: governance; Owner: -
--

CREATE TABLE governance.budgets (
    id text NOT NULL,
    start_at timestamp without time zone,
    finish_at timestamp without time zone,
    total bigint,
    allocated bigint,
    categories jsonb,
    raw jsonb NOT NULL,
    fetched_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL
);


--
-- Name: discourse_posts; Type: TABLE; Schema: governance; Owner: -
--

CREATE TABLE governance.discourse_posts (
    id integer NOT NULL,
    topic_id integer NOT NULL,
    post_number integer NOT NULL,
    username text NOT NULL,
    raw text DEFAULT ''::text NOT NULL,
    created_at timestamp without time zone NOT NULL,
    updated_at timestamp without time zone,
    hidden boolean DEFAULT false NOT NULL,
    deleted_at timestamp without time zone,
    fetched_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL
);


--
-- Name: discourse_topics; Type: TABLE; Schema: governance; Owner: -
--

CREATE TABLE governance.discourse_topics (
    topic_id integer NOT NULL,
    status text DEFAULT 'ok'::text NOT NULL,
    posts_count integer,
    highest_post_number integer,
    last_posted_at timestamp without time zone,
    fetched_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    CONSTRAINT discourse_topics_status_check CHECK ((status = ANY (ARRAY['ok'::text, 'missing'::text])))
);


--
-- Name: members; Type: TABLE; Schema: governance; Owner: -
--

CREATE TABLE governance.members (
    role text NOT NULL,
    address text NOT NULL,
    fetched_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    CONSTRAINT members_role_check CHECK ((role = ANY (ARRAY['committee'::text, 'council'::text])))
);


--
-- Name: mirror_state; Type: TABLE; Schema: governance; Owner: -
--

CREATE TABLE governance.mirror_state (
    key text NOT NULL,
    value text,
    updated_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL
);


--
-- Name: project_updates; Type: TABLE; Schema: governance; Owner: -
--

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
    fetched_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL
);


--
-- Name: projects; Type: TABLE; Schema: governance; Owner: -
--

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
    fetched_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL
);


--
-- Name: proposals; Type: TABLE; Schema: governance; Owner: -
--

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
    fetched_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL
);


--
-- Name: snapshot_proposals; Type: TABLE; Schema: governance; Owner: -
--

CREATE TABLE governance.snapshot_proposals (
    id text NOT NULL,
    space text NOT NULL,
    state text NOT NULL,
    scores_state text,
    start_at timestamp without time zone,
    end_at timestamp without time zone,
    created_at timestamp without time zone,
    choices jsonb DEFAULT '[]'::jsonb NOT NULL,
    scores jsonb DEFAULT '[]'::jsonb NOT NULL,
    scores_total double precision DEFAULT 0 NOT NULL,
    votes_count integer DEFAULT 0 NOT NULL,
    fetched_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL
);


--
-- Name: snapshot_votes; Type: TABLE; Schema: governance; Owner: -
--

CREATE TABLE governance.snapshot_votes (
    proposal_id text NOT NULL,
    voter text NOT NULL,
    id text NOT NULL,
    space text NOT NULL,
    choice jsonb NOT NULL,
    vp double precision DEFAULT 0 NOT NULL,
    reason text,
    created_at timestamp without time zone NOT NULL,
    fetched_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL
);


--
-- Name: vestings; Type: TABLE; Schema: governance; Owner: -
--

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
    fetched_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL
);


--
-- Name: account_right; Type: TABLE; Schema: land_authz; Owner: -
--

CREATE TABLE land_authz.account_right (
    token_address character varying(42) NOT NULL,
    account character varying(42) NOT NULL,
    operator character varying(42) NOT NULL,
    kind text NOT NULL,
    is_approved boolean NOT NULL,
    updated_block bigint NOT NULL,
    updated_log integer NOT NULL,
    CONSTRAINT account_right_kind_known CHECK ((kind = ANY (ARRAY['update_manager'::text, 'approved_for_all'::text])))
);


--
-- Name: authz_event; Type: TABLE; Schema: land_authz; Owner: -
--

CREATE TABLE land_authz.authz_event (
    block_number bigint NOT NULL,
    log_index integer NOT NULL,
    block_time bigint NOT NULL,
    token_address character varying(42) NOT NULL,
    kind text NOT NULL,
    token_id numeric(78,0),
    estate_id numeric(78,0),
    account character varying(42),
    operator character varying(42),
    approved boolean,
    CONSTRAINT authz_event_estate_kinds_carry_estate CHECK (((kind = ANY (ARRAY['add_land'::text, 'remove_land'::text])) = (estate_id IS NOT NULL))),
    CONSTRAINT authz_event_kind_known CHECK ((kind = ANY (ARRAY['update_operator'::text, 'approval'::text, 'transfer'::text, 'update_manager'::text, 'approved_for_all'::text, 'add_land'::text, 'remove_land'::text]))),
    CONSTRAINT authz_event_token_address_lowercase CHECK (((token_address)::text = lower((token_address)::text)))
);


--
-- Name: estate_land; Type: TABLE; Schema: land_authz; Owner: -
--

CREATE TABLE land_authz.estate_land (
    land_token_id numeric(78,0) NOT NULL,
    estate_token_id numeric(78,0) NOT NULL,
    updated_block bigint NOT NULL,
    updated_log integer NOT NULL
);


--
-- Name: index_cursor; Type: TABLE; Schema: land_authz; Owner: -
--

CREATE TABLE land_authz.index_cursor (
    network text NOT NULL,
    land_registry character varying(42) NOT NULL,
    estate_registry character varying(42) NOT NULL,
    last_block bigint NOT NULL,
    folded_block bigint,
    folded_at timestamp without time zone,
    full_replay_started boolean DEFAULT false NOT NULL,
    updated_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    history_timestamps_ready boolean DEFAULT false NOT NULL,
    CONSTRAINT index_cursor_folded_within_indexed CHECK (((folded_block IS NULL) OR (folded_block <= last_block))),
    CONSTRAINT index_cursor_network_known CHECK ((network = ANY (ARRAY['ethereum-mainnet'::text, 'ethereum-sepolia'::text])))
);


--
-- Name: token_right; Type: TABLE; Schema: land_authz; Owner: -
--

CREATE TABLE land_authz.token_right (
    token_address character varying(42) NOT NULL,
    token_id numeric(78,0) NOT NULL,
    x integer,
    y integer,
    owner character varying(42),
    operator character varying(42),
    update_operator character varying(42),
    updated_block bigint NOT NULL,
    updated_log integer NOT NULL
);


--
-- Name: schema; Type: TABLE; Schema: meta; Owner: -
--

CREATE TABLE meta.schema (
    id integer NOT NULL,
    identity text NOT NULL,
    version integer NOT NULL,
    applied_at timestamp without time zone NOT NULL,
    CONSTRAINT schema_id_check CHECK ((id = 1))
);


--
-- Name: schema_steps; Type: TABLE; Schema: meta; Owner: -
--

CREATE TABLE meta.schema_steps (
    step text NOT NULL,
    sha256 text NOT NULL,
    created_fresh boolean NOT NULL,
    applied_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    CONSTRAINT schema_steps_sha256_check CHECK ((sha256 ~ '^[0-9a-f]{64}$'::text))
);


--
-- Name: admin_audit; Type: TABLE; Schema: ops; Owner: -
--

CREATE TABLE ops.admin_audit (
    id bigint NOT NULL,
    at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    actor text NOT NULL,
    domain text NOT NULL,
    action text NOT NULL,
    target text,
    detail jsonb DEFAULT '{}'::jsonb NOT NULL,
    result text DEFAULT 'ok'::text NOT NULL,
    CONSTRAINT admin_audit_domain_check CHECK ((domain = ANY (ARRAY['content'::text, 'telemetry'::text, 'credits'::text, 'governance'::text, 'quests'::text, 'rpc'::text, 'registry'::text, 'explore'::text, 'worlds'::text, 'social'::text, 'create'::text, 'market'::text, 'economy'::text, 'ops'::text])))
);


--
-- Name: admin_audit_id_seq; Type: SEQUENCE; Schema: ops; Owner: -
--

ALTER TABLE ops.admin_audit ALTER COLUMN id ADD GENERATED ALWAYS AS IDENTITY (
    SEQUENCE NAME ops.admin_audit_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1
);


--
-- Name: daily_stats; Type: TABLE; Schema: presence; Owner: -
--

CREATE TABLE presence.daily_stats (
    date date NOT NULL,
    snapshots_taken integer DEFAULT 0 NOT NULL,
    peak_peers integer DEFAULT 0 NOT NULL,
    avg_peers double precision DEFAULT 0 NOT NULL,
    peak_hot_scene_users integer DEFAULT 0 NOT NULL,
    peak_scene_users integer DEFAULT 0 NOT NULL,
    peak_world_users integer DEFAULT 0 NOT NULL
);


--
-- Name: hot_scene_snapshots; Type: TABLE; Schema: presence; Owner: -
--

CREATE TABLE presence.hot_scene_snapshots (
    snapshot_id bigint NOT NULL,
    scene_id text NOT NULL,
    name text,
    base_x integer,
    base_y integer,
    users_count integer,
    parcel_count integer,
    creator text,
    description text
);


--
-- Name: island_snapshots; Type: TABLE; Schema: presence; Owner: -
--

CREATE TABLE presence.island_snapshots (
    snapshot_id bigint NOT NULL,
    island_id text NOT NULL,
    peer_count integer,
    max_peers integer,
    center_x double precision,
    center_y double precision,
    center_z double precision,
    radius double precision
);


--
-- Name: peer_snapshots; Type: TABLE; Schema: presence; Owner: -
--

CREATE TABLE presence.peer_snapshots (
    snapshot_id bigint NOT NULL,
    address character varying(42) NOT NULL,
    parcel_x integer,
    parcel_y integer,
    position_x double precision,
    position_y double precision,
    position_z double precision,
    last_ping bigint
);


--
-- Name: scene_occupancy; Type: TABLE; Schema: presence; Owner: -
--

CREATE TABLE presence.scene_occupancy (
    snapshot_id bigint NOT NULL,
    taken_at timestamp without time zone NOT NULL,
    pointer text NOT NULL,
    scene_name text,
    realm text DEFAULT 'main'::text NOT NULL,
    addresses jsonb NOT NULL,
    count integer NOT NULL,
    CONSTRAINT scene_occupancy_count_matches CHECK ((count = jsonb_array_length(addresses)))
);


--
-- Name: snapshots; Type: TABLE; Schema: presence; Owner: -
--

CREATE TABLE presence.snapshots (
    id bigint NOT NULL,
    taken_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    peers_count integer DEFAULT 0 NOT NULL,
    islands_count integer DEFAULT 0 NOT NULL,
    hot_scenes_count integer DEFAULT 0 NOT NULL,
    scenes_polled integer DEFAULT 0 NOT NULL,
    scene_users_total integer DEFAULT 0 NOT NULL,
    worlds_polled integer DEFAULT 0 NOT NULL,
    active_worlds integer DEFAULT 0 NOT NULL,
    world_users_total integer DEFAULT 0 NOT NULL,
    worlds_live_total integer
);


--
-- Name: snapshots_id_seq; Type: SEQUENCE; Schema: presence; Owner: -
--

ALTER TABLE presence.snapshots ALTER COLUMN id ADD GENERATED ALWAYS AS IDENTITY (
    SEQUENCE NAME presence.snapshots_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1
);


--
-- Name: world_membership; Type: TABLE; Schema: presence; Owner: -
--

CREATE TABLE presence.world_membership (
    snapshot_id bigint NOT NULL,
    taken_at timestamp without time zone NOT NULL,
    world_name text NOT NULL,
    addresses jsonb NOT NULL,
    count integer NOT NULL,
    live_users integer,
    CONSTRAINT world_membership_count_matches CHECK ((count = jsonb_array_length(addresses)))
);


--
-- Name: quest_events; Type: TABLE; Schema: quests; Owner: -
--

CREATE TABLE quests.quest_events (
    id uuid NOT NULL,
    quest_instance_id uuid NOT NULL,
    user_address text NOT NULL,
    event bytea NOT NULL,
    occurred_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    CONSTRAINT quest_events_user_address_check CHECK ((user_address ~ '^0x[0-9a-f]{40}$'::text))
);


--
-- Name: quest_instances; Type: TABLE; Schema: quests; Owner: -
--

CREATE TABLE quests.quest_instances (
    id uuid NOT NULL,
    quest_id uuid NOT NULL,
    user_address text NOT NULL,
    started_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    completed_at timestamp without time zone,
    abandoned_at timestamp without time zone,
    CONSTRAINT quest_instances_one_end_check CHECK (((completed_at IS NULL) OR (abandoned_at IS NULL))),
    CONSTRAINT quest_instances_user_address_check CHECK ((user_address ~ '^0x[0-9a-f]{40}$'::text))
);


--
-- Name: quest_reward_items; Type: TABLE; Schema: quests; Owner: -
--

CREATE TABLE quests.quest_reward_items (
    quest_id uuid NOT NULL,
    "position" integer NOT NULL,
    reward_name text NOT NULL,
    reward_image text NOT NULL
);


--
-- Name: quests; Type: TABLE; Schema: quests; Owner: -
--

CREATE TABLE quests.quests (
    id uuid NOT NULL,
    name text NOT NULL,
    description text NOT NULL,
    definition bytea NOT NULL,
    creator_address text NOT NULL,
    image_url text DEFAULT ''::text NOT NULL,
    previous_quest_id uuid,
    deactivated_at timestamp without time zone,
    reward_webhook_url text,
    reward_request_body jsonb,
    created_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    updated_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    CONSTRAINT quests_creator_address_check CHECK ((creator_address ~ '^0x[0-9a-f]{40}$'::text)),
    CONSTRAINT quests_not_own_predecessor_check CHECK (((previous_quest_id IS NULL) OR (previous_quest_id <> id))),
    CONSTRAINT quests_reward_body_needs_url_check CHECK (((reward_request_body IS NULL) OR (reward_webhook_url IS NOT NULL)))
);


--
-- Name: admin_audit; Type: TABLE; Schema: shop; Owner: -
--

CREATE TABLE shop.admin_audit (
    id bigint NOT NULL,
    actor text NOT NULL,
    action text NOT NULL,
    target_kind text NOT NULL,
    target_hash text NOT NULL,
    detail jsonb DEFAULT '{}'::jsonb NOT NULL,
    created_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL
);


--
-- Name: admin_audit_id_seq; Type: SEQUENCE; Schema: shop; Owner: -
--

ALTER TABLE shop.admin_audit ALTER COLUMN id ADD GENERATED ALWAYS AS IDENTITY (
    SEQUENCE NAME shop.admin_audit_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1
);


--
-- Name: broker_purchases; Type: TABLE; Schema: shop; Owner: -
--

CREATE TABLE shop.broker_purchases (
    id bigint NOT NULL,
    idempotency_key text,
    tx_hash text,
    forward_tx_hash text,
    collection character varying(42) NOT NULL,
    item_id text,
    token_id text,
    minted_token_id text,
    buyer_address character varying(42),
    escrow_address character varying(42) NOT NULL,
    price_wei numeric(78,0) NOT NULL,
    usd_amount_wei numeric(78,0),
    mana_usd_rate_wei numeric(78,0),
    trade_hashed_signature text,
    chain_id bigint NOT NULL,
    mode text NOT NULL,
    status text DEFAULT 'sent'::text NOT NULL,
    created_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    updated_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    CONSTRAINT broker_purchases_collection_lowercase CHECK (((collection)::text = lower((collection)::text))),
    CONSTRAINT broker_purchases_mode_valid CHECK ((mode = ANY (ARRAY['primary'::text, 'secondary'::text, 'trade'::text, 'name-mint'::text, 'name-secondary'::text]))),
    CONSTRAINT broker_purchases_price_nonneg CHECK ((price_wei >= (0)::numeric)),
    CONSTRAINT broker_purchases_status_known CHECK ((status = ANY (ARRAY['pending'::text, 'sent'::text, 'reverted'::text, 'bought'::text, 'forwarding'::text, 'confirmed'::text, 'error'::text])))
);


--
-- Name: broker_purchases_id_seq; Type: SEQUENCE; Schema: shop; Owner: -
--

ALTER TABLE shop.broker_purchases ALTER COLUMN id ADD GENERATED ALWAYS AS IDENTITY (
    SEQUENCE NAME shop.broker_purchases_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1
);


--
-- Name: campaign_collections; Type: TABLE; Schema: shop; Owner: -
--

CREATE TABLE shop.campaign_collections (
    tag text NOT NULL,
    network text NOT NULL,
    contract_address character varying(42) NOT NULL,
    added_by text NOT NULL,
    created_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    CONSTRAINT campaign_collections_contract_lowercase CHECK (((contract_address)::text = lower((contract_address)::text))),
    CONSTRAINT campaign_collections_network_known CHECK ((network = ANY (ARRAY['ethereum'::text, 'matic'::text])))
);


--
-- Name: campaigns; Type: TABLE; Schema: shop; Owner: -
--

CREATE TABLE shop.campaigns (
    id text NOT NULL,
    name text NOT NULL,
    tab_name text,
    main_tag text NOT NULL,
    tags text[] DEFAULT '{}'::text[] NOT NULL,
    starts_at timestamp without time zone,
    ends_at timestamp without time zone,
    enabled boolean DEFAULT false NOT NULL,
    banner jsonb DEFAULT '{}'::jsonb NOT NULL,
    updated_by text NOT NULL,
    updated_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    CONSTRAINT campaigns_id_slug CHECK ((id ~ '^[a-z0-9][a-z0-9-]{0,63}$'::text)),
    CONSTRAINT campaigns_window CHECK (((starts_at IS NULL) OR (ends_at IS NULL) OR (starts_at < ends_at)))
);


--
-- Name: chain_cursor; Type: TABLE; Schema: shop; Owner: -
--

CREATE TABLE shop.chain_cursor (
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
    reorg_rewinds integer DEFAULT 0 NOT NULL,
    updated_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    CONSTRAINT chain_cursor_fold_within_indexed CHECK (((fold_through IS NULL) OR (fold_through <= last_block))),
    CONSTRAINT chain_cursor_folded_within_indexed CHECK (((folded_block IS NULL) OR (folded_block <= last_block))),
    CONSTRAINT chain_cursor_hash_shape CHECK (((last_block_hash IS NULL) OR (last_block_hash ~ '^0x[0-9a-f]{64}$'::text))),
    CONSTRAINT chain_cursor_indexer_known CHECK ((indexer = ANY (ARRAY['nft'::text, 'trade_events'::text, 'rentals'::text, 'mana_transfers'::text]))),
    CONSTRAINT chain_cursor_network_known CHECK ((network = ANY (ARRAY['ethereum'::text, 'matic'::text])))
);


--
-- Name: collections; Type: TABLE; Schema: shop; Owner: -
--

CREATE TABLE shop.collections (
    network text NOT NULL,
    contract_address character varying(42) NOT NULL,
    chain_id integer NOT NULL,
    name text NOT NULL,
    symbol text,
    creator character varying(42) NOT NULL,
    owner character varying(42) NOT NULL,
    urn text NOT NULL,
    is_completed boolean NOT NULL,
    is_approved boolean NOT NULL,
    is_editable boolean NOT NULL,
    items_count integer DEFAULT 0 NOT NULL,
    minters text[] DEFAULT '{}'::text[] NOT NULL,
    managers text[] DEFAULT '{}'::text[] NOT NULL,
    search_is_store_minter boolean DEFAULT false NOT NULL,
    search_text text,
    created_at bigint NOT NULL,
    updated_at bigint NOT NULL,
    reviewed_at bigint NOT NULL,
    first_listed_at bigint,
    mirrored_at timestamp without time zone NOT NULL,
    CONSTRAINT collections_contract_lowercase CHECK (((contract_address)::text = lower((contract_address)::text))),
    CONSTRAINT collections_items_count_nonneg CHECK ((items_count >= 0)),
    CONSTRAINT collections_network_known CHECK ((network = ANY (ARRAY['ethereum'::text, 'matic'::text])))
);


--
-- Name: coupon_state; Type: TABLE; Schema: shop; Owner: -
--

CREATE TABLE shop.coupon_state (
    coupon_id uuid NOT NULL,
    uses integer DEFAULT 0 NOT NULL,
    cancelled boolean DEFAULT false NOT NULL,
    revoked boolean DEFAULT false NOT NULL,
    checked_at timestamp(3) without time zone DEFAULT (timezone('UTC'::text, now()))::timestamp(3) without time zone NOT NULL,
    CONSTRAINT coupon_state_uses_nonneg CHECK ((uses >= 0))
);


--
-- Name: coupons; Type: TABLE; Schema: shop; Owner: -
--

CREATE TABLE shop.coupons (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    network text NOT NULL,
    chain_id integer NOT NULL,
    signer character varying(42) NOT NULL,
    signature text NOT NULL,
    hashed_signature text NOT NULL,
    state_key text NOT NULL,
    coupon_manager character varying(42) NOT NULL,
    coupon_address character varying(42) NOT NULL,
    checks jsonb NOT NULL,
    discount_type smallint NOT NULL,
    discount_ppm integer NOT NULL,
    root text NOT NULL,
    collections text[] NOT NULL,
    effective_since timestamp(3) without time zone NOT NULL,
    expires_at timestamp(3) without time zone NOT NULL,
    created_at timestamp(3) without time zone DEFAULT (timezone('UTC'::text, now()))::timestamp(3) without time zone NOT NULL,
    CONSTRAINT coupons_discount_ppm_within_bounds CHECK (((discount_ppm >= 50000) AND (discount_ppm <= 700000))),
    CONSTRAINT coupons_effective_before_expiry CHECK ((effective_since < expires_at)),
    CONSTRAINT coupons_signer_lowercase CHECK (((signer)::text = lower((signer)::text)))
);


--
-- Name: disputes; Type: TABLE; Schema: shop; Owner: -
--

CREATE TABLE shop.disputes (
    trade_hash text NOT NULL,
    status text DEFAULT 'open'::text NOT NULL,
    reason text DEFAULT ''::text NOT NULL,
    resolution text DEFAULT ''::text NOT NULL,
    opened_by text NOT NULL,
    opened_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    resolved_by text,
    resolved_at timestamp without time zone,
    CONSTRAINT disputes_resolved_iff_closed CHECK (((status = 'open'::text) = (resolved_at IS NULL))),
    CONSTRAINT disputes_status_known CHECK ((status = ANY (ARRAY['open'::text, 'resolved'::text, 'rejected'::text])))
);


--
-- Name: escrow_actions; Type: TABLE; Schema: shop; Owner: -
--

CREATE TABLE shop.escrow_actions (
    id bigint NOT NULL,
    action text NOT NULL,
    collection character varying(42),
    token_id text,
    buyer character varying(42),
    escrow_address character varying(42),
    tx_hash text,
    idempotency_key text,
    status text DEFAULT 'sent'::text NOT NULL,
    chain_id bigint,
    created_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    updated_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    CONSTRAINT escrow_actions_action_known CHECK ((action = ANY (ARRAY['reclaim'::text, 'release'::text]))),
    CONSTRAINT escrow_actions_status_known CHECK ((status = ANY (ARRAY['pending'::text, 'sent'::text, 'confirmed'::text, 'reverted'::text, 'error'::text])))
);


--
-- Name: escrow_actions_id_seq; Type: SEQUENCE; Schema: shop; Owner: -
--

ALTER TABLE shop.escrow_actions ALTER COLUMN id ADD GENERATED ALWAYS AS IDENTITY (
    SEQUENCE NAME shop.escrow_actions_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1
);


--
-- Name: executions; Type: TABLE; Schema: shop; Owner: -
--

CREATE TABLE shop.executions (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    chain_id bigint NOT NULL,
    tx_hash text NOT NULL,
    kind text NOT NULL,
    actor character varying(42) NOT NULL,
    venue character varying(42) NOT NULL,
    contract_address character varying(42),
    token_id numeric(78,0),
    reference text,
    expected_price numeric(78,0),
    mana_token character varying(42),
    expected_seller character varying(42),
    expected_owner character varying(42),
    status text DEFAULT 'submitted'::text NOT NULL,
    block_number bigint,
    confirmed_by text,
    settlement jsonb,
    attempts integer DEFAULT 0 NOT NULL,
    last_error text,
    next_check_at timestamp without time zone NOT NULL,
    created_at timestamp without time zone NOT NULL,
    updated_at timestamp without time zone NOT NULL,
    CONSTRAINT executions_confirmed_by_known CHECK (((confirmed_by IS NULL) OR (confirmed_by = ANY (ARRAY['receipt'::text, 'index'::text, 'finality'::text])))),
    CONSTRAINT executions_kind_known CHECK ((kind = ANY (ARRAY['accept_trade'::text, 'execute_order'::text, 'accept_rental'::text, 'register_name'::text, 'transfer'::text, 'cancel_trade'::text, 'cancel_order'::text, 'claim_rental'::text, 'cancel_rental'::text, 'approve_mana'::text]))),
    CONSTRAINT executions_lowercase CHECK ((((actor)::text = lower((actor)::text)) AND ((venue)::text = lower((venue)::text)) AND ((contract_address IS NULL) OR ((contract_address)::text = lower((contract_address)::text))))),
    CONSTRAINT executions_price_nonneg CHECK (((expected_price IS NULL) OR (expected_price >= (0)::numeric))),
    CONSTRAINT executions_status_known CHECK ((status = ANY (ARRAY['submitted'::text, 'mined'::text, 'reverted'::text, 'settled'::text, 'mismatch'::text, 'dropped'::text]))),
    CONSTRAINT executions_tx_hash_shape CHECK ((tx_hash ~ '^0x[0-9a-f]{64}$'::text))
);


--
-- Name: fed_actions; Type: TABLE; Schema: shop; Owner: -
--

CREATE TABLE shop.fed_actions (
    signature_hash text NOT NULL,
    seq bigint NOT NULL,
    kind text NOT NULL,
    signer character varying(42) NOT NULL,
    item_id text,
    target_hash text,
    target_kind text,
    price numeric(78,0),
    expires_at bigint,
    fingerprint text,
    buyer text,
    tx_hash text,
    taken_at bigint,
    signed_at bigint NOT NULL,
    message_payload jsonb NOT NULL,
    received_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    CONSTRAINT fed_actions_buyer_lowercase CHECK ((buyer = lower(buyer))),
    CONSTRAINT fed_actions_cancel_shape CHECK ((((kind = 'cancel'::text) = (target_kind IS NOT NULL)) AND ((target_kind IS NULL) OR (target_kind = ANY (ARRAY['bid'::text, 'order'::text]))))),
    CONSTRAINT fed_actions_kind_known CHECK ((kind = ANY (ARRAY['bid'::text, 'order'::text, 'trade'::text, 'cancel'::text, 'accept'::text]))),
    CONSTRAINT fed_actions_listing_shape CHECK (((kind = ANY (ARRAY['bid'::text, 'order'::text])) = ((item_id IS NOT NULL) AND (price IS NOT NULL) AND (expires_at IS NOT NULL)))),
    CONSTRAINT fed_actions_price_nonneg CHECK ((price >= (0)::numeric)),
    CONSTRAINT fed_actions_signer_lowercase CHECK (((signer)::text = lower((signer)::text))),
    CONSTRAINT fed_actions_target_shape CHECK (((kind = ANY (ARRAY['trade'::text, 'cancel'::text, 'accept'::text])) = (target_hash IS NOT NULL))),
    CONSTRAINT fed_actions_trade_shape CHECK (((kind = 'trade'::text) = ((buyer IS NOT NULL) AND (tx_hash IS NOT NULL) AND (taken_at IS NOT NULL))))
);


--
-- Name: fed_actions_seq_seq; Type: SEQUENCE; Schema: shop; Owner: -
--

ALTER TABLE shop.fed_actions ALTER COLUMN seq ADD GENERATED ALWAYS AS IDENTITY (
    SEQUENCE NAME shop.fed_actions_seq_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1
);


--
-- Name: fed_seen_nonces; Type: TABLE; Schema: shop; Owner: -
--

CREATE TABLE shop.fed_seen_nonces (
    signer text NOT NULL,
    nonce text NOT NULL,
    expires_at bigint NOT NULL
);


--
-- Name: item_availability_watch; Type: TABLE; Schema: shop; Owner: -
--

CREATE TABLE shop.item_availability_watch (
    contract_address character varying(42) NOT NULL,
    item_id text NOT NULL,
    on_sale boolean,
    observed_at timestamp without time zone NOT NULL,
    changed_at timestamp without time zone,
    CONSTRAINT item_availability_watch_contract_lowercase CHECK (((contract_address)::text ~ '^0x[0-9a-f]{40}$'::text))
);


--
-- Name: item_neighbors; Type: TABLE; Schema: shop; Owner: -
--

CREATE TABLE shop.item_neighbors (
    network text NOT NULL,
    contract_address character varying(42) NOT NULL,
    item_id numeric(78,0) NOT NULL,
    neighbor_network text NOT NULL,
    neighbor_contract_address character varying(42) NOT NULL,
    neighbor_item_id numeric(78,0) NOT NULL,
    sim real NOT NULL,
    support integer NOT NULL,
    rank smallint NOT NULL,
    CONSTRAINT item_neighbors_rank_positive CHECK ((rank >= 1)),
    CONSTRAINT item_neighbors_sim_range CHECK (((sim > (0)::double precision) AND (sim <= (1)::double precision)))
);


--
-- Name: item_neighbors_meta; Type: TABLE; Schema: shop; Owner: -
--

CREATE TABLE shop.item_neighbors_meta (
    id integer NOT NULL,
    built_at timestamp without time zone NOT NULL,
    algorithm text NOT NULL,
    wallets integer NOT NULL,
    items integer NOT NULL,
    rows_written integer NOT NULL,
    source_folded_block_matic bigint,
    source_folded_block_ethereum bigint,
    CONSTRAINT item_neighbors_meta_id_check CHECK ((id = 1))
);


--
-- Name: item_notify_outbox; Type: TABLE; Schema: shop; Owner: -
--

CREATE TABLE shop.item_notify_outbox (
    id bigint NOT NULL,
    request_id bigint NOT NULL,
    contract_address character varying(42) NOT NULL,
    item_id text NOT NULL,
    item_name text,
    trigger text NOT NULL,
    status text DEFAULT 'pending'::text NOT NULL,
    attempts integer DEFAULT 0 NOT NULL,
    next_attempt_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    last_error text,
    provider_message_id text,
    created_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    updated_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    sent_at timestamp without time zone,
    CONSTRAINT item_notify_outbox_attempts_nonneg CHECK ((attempts >= 0)),
    CONSTRAINT item_notify_outbox_status_known CHECK ((status = ANY (ARRAY['pending'::text, 'sent'::text, 'failed'::text, 'skipped'::text])))
);


--
-- Name: item_notify_outbox_id_seq; Type: SEQUENCE; Schema: shop; Owner: -
--

ALTER TABLE shop.item_notify_outbox ALTER COLUMN id ADD GENERATED ALWAYS AS IDENTITY (
    SEQUENCE NAME shop.item_notify_outbox_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1
);


--
-- Name: item_notify_requests; Type: TABLE; Schema: shop; Owner: -
--

CREATE TABLE shop.item_notify_requests (
    id bigint NOT NULL,
    address character varying(42) NOT NULL,
    email text NOT NULL,
    contract_address character varying(42) NOT NULL,
    item_id text NOT NULL,
    chain_id integer NOT NULL,
    status text DEFAULT 'pending'::text NOT NULL,
    created_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    updated_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    notified_at timestamp without time zone,
    CONSTRAINT item_notify_requests_address_lowercase CHECK (((address)::text ~ '^0x[0-9a-f]{40}$'::text)),
    CONSTRAINT item_notify_requests_chain_positive CHECK ((chain_id >= 1)),
    CONSTRAINT item_notify_requests_contract_lowercase CHECK (((contract_address)::text ~ '^0x[0-9a-f]{40}$'::text)),
    CONSTRAINT item_notify_requests_email_length CHECK (((char_length(email) >= 3) AND (char_length(email) <= 254))),
    CONSTRAINT item_notify_requests_item_id_length CHECK (((char_length(item_id) >= 1) AND (char_length(item_id) <= 78))),
    CONSTRAINT item_notify_requests_status_known CHECK ((status = ANY (ARRAY['pending'::text, 'notified'::text, 'unsubscribed'::text])))
);


--
-- Name: item_notify_requests_id_seq; Type: SEQUENCE; Schema: shop; Owner: -
--

ALTER TABLE shop.item_notify_requests ALTER COLUMN id ADD GENERATED ALWAYS AS IDENTITY (
    SEQUENCE NAME shop.item_notify_requests_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1
);


--
-- Name: items; Type: TABLE; Schema: shop; Owner: -
--

CREATE TABLE shop.items (
    network text NOT NULL,
    contract_address character varying(42) NOT NULL,
    item_id numeric(78,0) NOT NULL,
    chain_id integer NOT NULL,
    item_type text NOT NULL,
    urn text NOT NULL,
    creator character varying(42) NOT NULL,
    beneficiary character varying(42),
    rarity text NOT NULL,
    total_supply numeric(78,0) NOT NULL,
    max_supply numeric(78,0) NOT NULL,
    available numeric(78,0) NOT NULL,
    price numeric(78,0) NOT NULL,
    image text,
    name text NOT NULL,
    description text,
    category text NOT NULL,
    body_shapes text[] DEFAULT '{}'::text[] NOT NULL,
    is_smart boolean DEFAULT false NOT NULL,
    emote_loop boolean,
    emote_has_sound boolean,
    emote_has_geometry boolean,
    emote_outcome_type text,
    search_is_store_minter boolean DEFAULT false NOT NULL,
    search_is_collection_approved boolean DEFAULT false NOT NULL,
    created_at bigint NOT NULL,
    updated_at bigint NOT NULL,
    reviewed_at bigint NOT NULL,
    sold_at bigint DEFAULT 0 NOT NULL,
    first_listed_at bigint,
    min_price numeric(78,0),
    max_listing_price numeric(78,0),
    min_listing_price numeric(78,0),
    listings integer,
    owners integer,
    mirrored_at timestamp without time zone NOT NULL,
    CONSTRAINT items_available_nonneg CHECK ((available >= (0)::numeric)),
    CONSTRAINT items_contract_lowercase CHECK (((contract_address)::text = lower((contract_address)::text))),
    CONSTRAINT items_network_known CHECK ((network = ANY (ARRAY['ethereum'::text, 'matic'::text]))),
    CONSTRAINT items_price_nonneg CHECK ((price >= (0)::numeric)),
    CONSTRAINT items_type_known CHECK ((item_type = ANY (ARRAY['wearable_v1'::text, 'wearable_v2'::text, 'smart_wearable_v1'::text, 'emote_v1'::text])))
);


--
-- Name: land_data; Type: TABLE; Schema: shop; Owner: -
--

CREATE TABLE shop.land_data (
    token_address character varying(42) NOT NULL,
    token_id numeric(78,0) NOT NULL,
    name text,
    description text,
    updated_block bigint NOT NULL,
    updated_log integer NOT NULL
);


--
-- Name: land_proximity; Type: TABLE; Schema: shop; Owner: -
--

CREATE TABLE shop.land_proximity (
    x smallint NOT NULL,
    y smallint NOT NULL,
    distance_to_plaza smallint NOT NULL,
    adjacent_to_road boolean NOT NULL,
    CONSTRAINT land_proximity_distance_range CHECK ((distance_to_plaza >= '-1'::integer))
);


--
-- Name: list_acl; Type: TABLE; Schema: shop; Owner: -
--

CREATE TABLE shop.list_acl (
    list_id uuid NOT NULL,
    permission text NOT NULL,
    grantee text NOT NULL,
    CONSTRAINT list_acl_permission_known CHECK ((permission = ANY (ARRAY['edit'::text, 'view'::text])))
);


--
-- Name: lists; Type: TABLE; Schema: shop; Owner: -
--

CREATE TABLE shop.lists (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    name text NOT NULL,
    description text,
    user_address character varying(42) NOT NULL,
    is_private boolean DEFAULT false NOT NULL,
    permission text,
    created_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    updated_at timestamp without time zone,
    CONSTRAINT lists_permission_known CHECK (((permission IS NULL) OR (permission = ANY (ARRAY['edit'::text, 'view'::text])))),
    CONSTRAINT lists_user_lowercase CHECK (((user_address)::text = lower((user_address)::text)))
);


--
-- Name: mana_transfer; Type: TABLE; Schema: shop; Owner: -
--

CREATE TABLE shop.mana_transfer (
    network text NOT NULL,
    block_number bigint NOT NULL,
    log_index integer NOT NULL,
    transaction_hash text NOT NULL,
    sender text NOT NULL,
    recipient text NOT NULL,
    amount numeric(78,0) NOT NULL,
    "timestamp" bigint NOT NULL,
    CONSTRAINT mana_transfer_amount_check CHECK (((amount >= (0)::numeric) AND (amount <= '115792089237316195423570985008687907853269984665640564039457584007913129639935'::numeric))),
    CONSTRAINT mana_transfer_block_number_check CHECK ((block_number >= 0)),
    CONSTRAINT mana_transfer_log_index_check CHECK ((log_index >= 0)),
    CONSTRAINT mana_transfer_network_check CHECK ((network = ANY (ARRAY['ethereum'::text, 'matic'::text]))),
    CONSTRAINT mana_transfer_recipient_check CHECK ((recipient ~ '^0x[0-9a-f]{40}$'::text)),
    CONSTRAINT mana_transfer_sender_check CHECK ((sender ~ '^0x[0-9a-f]{40}$'::text)),
    CONSTRAINT mana_transfer_timestamp_check CHECK (("timestamp" >= 0)),
    CONSTRAINT mana_transfer_transaction_hash_check CHECK ((transaction_hash ~ '^0x[0-9a-f]{64}$'::text))
);


--
-- Name: mirror_state; Type: TABLE; Schema: shop; Owner: -
--

CREATE TABLE shop.mirror_state (
    mirror text NOT NULL,
    network text NOT NULL,
    source_url text NOT NULL,
    pass_started_at timestamp without time zone,
    cursor_updated_at bigint,
    cursor_id text,
    pages_fetched bigint DEFAULT 0 NOT NULL,
    rows_upserted bigint DEFAULT 0 NOT NULL,
    last_complete_pass_at timestamp without time zone,
    last_complete_pass_rows bigint,
    last_error text,
    updated_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    CONSTRAINT mirror_state_known_mirror CHECK ((mirror = ANY (ARRAY['collections'::text, 'items'::text]))),
    CONSTRAINT mirror_state_network_known CHECK ((network = ANY (ARRAY['ethereum'::text, 'matic'::text])))
);


--
-- Name: moderation_flags; Type: TABLE; Schema: shop; Owner: -
--

CREATE TABLE shop.moderation_flags (
    target_hash text NOT NULL,
    target_kind text NOT NULL,
    severity text DEFAULT 'review'::text NOT NULL,
    reason text DEFAULT ''::text NOT NULL,
    flagged_by text NOT NULL,
    flagged_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    CONSTRAINT moderation_flags_kind_known CHECK ((target_kind = ANY (ARRAY['bid'::text, 'order'::text, 'trade'::text]))),
    CONSTRAINT moderation_flags_severity_known CHECK ((severity = ANY (ARRAY['review'::text, 'hide'::text, 'block'::text])))
);


--
-- Name: name_transfers; Type: TABLE; Schema: shop; Owner: -
--

CREATE TABLE shop.name_transfers (
    id bigint NOT NULL,
    idempotency_key text NOT NULL,
    registrar character varying(42) NOT NULL,
    token_id text NOT NULL,
    from_address character varying(42) NOT NULL,
    to_address character varying(42) NOT NULL,
    chain_id bigint NOT NULL,
    tx_hash text,
    status text DEFAULT 'pending'::text NOT NULL,
    created_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    updated_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    CONSTRAINT name_transfers_status_known CHECK ((status = ANY (ARRAY['pending'::text, 'sent'::text, 'confirmed'::text, 'reverted'::text, 'error'::text])))
);


--
-- Name: name_transfers_id_seq; Type: SEQUENCE; Schema: shop; Owner: -
--

ALTER TABLE shop.name_transfers ALTER COLUMN id ADD GENERATED ALWAYS AS IDENTITY (
    SEQUENCE NAME shop.name_transfers_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1
);


--
-- Name: nft; Type: TABLE; Schema: shop; Owner: -
--

CREATE TABLE shop.nft (
    network text NOT NULL,
    contract_address character varying(42) NOT NULL,
    token_id numeric(78,0) NOT NULL,
    owner character varying(42),
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
    CONSTRAINT nft_network_known CHECK ((network = ANY (ARRAY['ethereum'::text, 'matic'::text])))
);


--
-- Name: nft_contract; Type: TABLE; Schema: shop; Owner: -
--

CREATE TABLE shop.nft_contract (
    network text NOT NULL,
    contract_address character varying(42) NOT NULL,
    walked_from_block bigint NOT NULL,
    backfill_from_block bigint,
    backfilled_to_block bigint,
    refold_owed boolean DEFAULT false NOT NULL,
    added_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    CONSTRAINT nft_contract_backfill_before_walk CHECK (((backfill_from_block IS NULL) OR (backfill_from_block < walked_from_block))),
    CONSTRAINT nft_contract_backfill_progress CHECK (((backfilled_to_block IS NULL) OR ((backfill_from_block IS NOT NULL) AND (backfilled_to_block < walked_from_block)))),
    CONSTRAINT nft_contract_lowercase CHECK (((contract_address)::text = lower((contract_address)::text))),
    CONSTRAINT nft_contract_network_known CHECK ((network = ANY (ARRAY['ethereum'::text, 'matic'::text])))
);


--
-- Name: nft_event; Type: TABLE; Schema: shop; Owner: -
--

CREATE TABLE shop.nft_event (
    network text NOT NULL,
    block_number bigint NOT NULL,
    log_index integer NOT NULL,
    block_time bigint NOT NULL,
    kind text NOT NULL,
    emitter character varying(42) NOT NULL,
    contract_address character varying(42) NOT NULL,
    token_id numeric(78,0) NOT NULL,
    from_address character varying(42),
    to_address character varying(42),
    amount numeric(78,0),
    expires_at bigint,
    detail text,
    CONSTRAINT nft_event_contract_lowercase CHECK ((((contract_address)::text = lower((contract_address)::text)) AND ((emitter)::text = lower((emitter)::text)))),
    CONSTRAINT nft_event_kind_known CHECK ((kind = ANY (ARRAY['transfer'::text, 'issue_v1'::text, 'name_registered'::text, 'order_created'::text, 'order_successful'::text, 'order_cancelled'::text, 'land_data'::text]))),
    CONSTRAINT nft_event_network_known CHECK ((network = ANY (ARRAY['ethereum'::text, 'matic'::text]))),
    CONSTRAINT nft_event_token_nonneg CHECK ((token_id >= (0)::numeric))
);


--
-- Name: nft_order; Type: TABLE; Schema: shop; Owner: -
--

CREATE TABLE shop.nft_order (
    network text NOT NULL,
    contract_address character varying(42) NOT NULL,
    token_id numeric(78,0) NOT NULL,
    order_id text NOT NULL,
    marketplace_address character varying(42) NOT NULL,
    status text NOT NULL,
    seller character varying(42) NOT NULL,
    buyer character varying(42),
    price numeric(78,0) NOT NULL,
    expires_at bigint NOT NULL,
    created_at bigint NOT NULL,
    updated_at bigint NOT NULL,
    updated_block bigint NOT NULL,
    updated_log integer NOT NULL,
    CONSTRAINT nft_order_price_nonneg CHECK ((price >= (0)::numeric)),
    CONSTRAINT nft_order_status_known CHECK ((status = ANY (ARRAY['open'::text, 'sold'::text, 'cancelled'::text])))
);


--
-- Name: outfit_thumbnails; Type: TABLE; Schema: shop; Owner: -
--

CREATE TABLE shop.outfit_thumbnails (
    hash text NOT NULL,
    blob_cid text NOT NULL,
    content_type text NOT NULL,
    size integer NOT NULL,
    author_address character varying(42) NOT NULL,
    created_at bigint NOT NULL,
    CONSTRAINT outfit_thumbnails_author_lowercase CHECK (((author_address)::text ~ '^0x[0-9a-f]{40}$'::text)),
    CONSTRAINT outfit_thumbnails_hash_sha256 CHECK ((hash ~ '^[0-9a-f]{64}$'::text)),
    CONSTRAINT outfit_thumbnails_size_bounds CHECK (((size > 0) AND (size <= 1048576))),
    CONSTRAINT outfit_thumbnails_type_known CHECK ((content_type = ANY (ARRAY['image/png'::text, 'image/jpeg'::text, 'image/webp'::text])))
);


--
-- Name: outfits; Type: TABLE; Schema: shop; Owner: -
--

CREATE TABLE shop.outfits (
    id text NOT NULL,
    name text NOT NULL,
    thumbnail_hash text DEFAULT ''::text NOT NULL,
    items jsonb NOT NULL,
    body_shape text NOT NULL,
    gradient_from text DEFAULT ''::text NOT NULL,
    gradient_to text DEFAULT ''::text NOT NULL,
    author_address character varying(42) NOT NULL,
    published boolean DEFAULT false NOT NULL,
    created_at bigint NOT NULL,
    updated_at bigint NOT NULL,
    CONSTRAINT outfits_author_lowercase CHECK (((author_address)::text ~ '^0x[0-9a-f]{40}$'::text)),
    CONSTRAINT outfits_body_shape_known CHECK ((body_shape = ANY (ARRAY['male'::text, 'female'::text, 'unisex'::text]))),
    CONSTRAINT outfits_gradient_from_hex CHECK (((gradient_from = ''::text) OR (gradient_from ~ '^#[0-9a-f]{6}$'::text))),
    CONSTRAINT outfits_gradient_to_hex CHECK (((gradient_to = ''::text) OR (gradient_to ~ '^#[0-9a-f]{6}$'::text))),
    CONSTRAINT outfits_id_uuid CHECK ((id ~ '^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$'::text)),
    CONSTRAINT outfits_items_array CHECK (((jsonb_typeof(items) = 'array'::text) AND (jsonb_array_length(items) <= 20))),
    CONSTRAINT outfits_name_length CHECK (((char_length(name) >= 1) AND (char_length(name) <= 60))),
    CONSTRAINT outfits_publish_bar CHECK (((NOT published) OR ((thumbnail_hash <> ''::text) AND (gradient_from <> ''::text) AND (gradient_to <> ''::text) AND (jsonb_array_length(items) >= 1)))),
    CONSTRAINT outfits_thumbnail_sha256 CHECK (((thumbnail_hash = ''::text) OR (thumbnail_hash ~ '^[0-9a-f]{64}$'::text)))
);


--
-- Name: picks; Type: TABLE; Schema: shop; Owner: -
--

CREATE TABLE shop.picks (
    item_id text NOT NULL,
    user_address character varying(42) NOT NULL,
    list_id uuid NOT NULL,
    created_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    CONSTRAINT picks_item_lowercase CHECK ((item_id = lower(item_id))),
    CONSTRAINT picks_user_lowercase CHECK (((user_address)::text = lower((user_address)::text)))
);


--
-- Name: rental_event; Type: TABLE; Schema: shop; Owner: -
--

CREATE TABLE shop.rental_event (
    network text NOT NULL,
    block_number bigint NOT NULL,
    log_index integer NOT NULL,
    block_time bigint NOT NULL,
    rentals_address character varying(42) NOT NULL,
    kind text NOT NULL,
    contract_address character varying(42),
    token_id numeric(78,0),
    signer character varying(42),
    tenant character varying(42),
    operator character varying(42),
    rental_days integer,
    price_per_day numeric(78,0),
    is_extension boolean,
    sender character varying(42),
    signature text,
    new_index numeric(78,0),
    tx_hash text NOT NULL,
    CONSTRAINT rental_event_asset_kinds CHECK (((kind = ANY (ARRAY['rented'::text, 'claimed'::text, 'asset_index_updated'::text])) = ((contract_address IS NOT NULL) AND (token_id IS NOT NULL)))),
    CONSTRAINT rental_event_index_kinds CHECK (((kind = ANY (ARRAY['asset_index_updated'::text, 'signer_index_updated'::text, 'contract_index_updated'::text])) = (new_index IS NOT NULL))),
    CONSTRAINT rental_event_kind_known CHECK ((kind = ANY (ARRAY['rented'::text, 'claimed'::text, 'asset_index_updated'::text, 'signer_index_updated'::text, 'contract_index_updated'::text]))),
    CONSTRAINT rental_event_lowercase CHECK ((((rentals_address)::text = lower((rentals_address)::text)) AND ((contract_address IS NULL) OR ((contract_address)::text = lower((contract_address)::text))) AND ((signature IS NULL) OR (signature = lower(signature))))),
    CONSTRAINT rental_event_network_known CHECK ((network = 'ethereum'::text)),
    CONSTRAINT rental_event_rented_fields CHECK (((kind = 'rented'::text) = ((signature IS NOT NULL) AND (tenant IS NOT NULL) AND (rental_days IS NOT NULL) AND (price_per_day IS NOT NULL))))
);


--
-- Name: trade_assets; Type: TABLE; Schema: shop; Owner: -
--

CREATE TABLE shop.trade_assets (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    trade_id uuid NOT NULL,
    direction shop.asset_direction_type NOT NULL,
    asset_type smallint NOT NULL,
    contract_address character varying(42) NOT NULL,
    beneficiary character varying(42),
    extra text,
    created_at timestamp(3) without time zone DEFAULT (timezone('UTC'::text, now()))::timestamp(3) without time zone NOT NULL,
    CONSTRAINT trade_assets_asset_type_known CHECK ((asset_type = ANY (ARRAY[1, 2, 3, 4])))
);


--
-- Name: trade_assets_erc20; Type: TABLE; Schema: shop; Owner: -
--

CREATE TABLE shop.trade_assets_erc20 (
    asset_id uuid NOT NULL,
    amount numeric(78,0) NOT NULL,
    CONSTRAINT trade_assets_erc20_amount_check CHECK ((amount >= (0)::numeric))
);


--
-- Name: trade_assets_erc721; Type: TABLE; Schema: shop; Owner: -
--

CREATE TABLE shop.trade_assets_erc721 (
    asset_id uuid NOT NULL,
    token_id text NOT NULL
);


--
-- Name: trade_assets_item; Type: TABLE; Schema: shop; Owner: -
--

CREATE TABLE shop.trade_assets_item (
    asset_id uuid NOT NULL,
    item_id text NOT NULL
);


--
-- Name: trade_cancellations; Type: TABLE; Schema: shop; Owner: -
--

CREATE TABLE shop.trade_cancellations (
    target_hashed_signature text NOT NULL,
    kind text NOT NULL,
    signer character varying(42) NOT NULL,
    signature text,
    reason text DEFAULT ''::text NOT NULL,
    cancelled_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    CONSTRAINT trade_cancellations_kind_known CHECK ((kind = ANY (ARRAY['signer'::text, 'operator'::text]))),
    CONSTRAINT trade_cancellations_signed_when_signer CHECK (((kind <> 'signer'::text) OR (signature IS NOT NULL))),
    CONSTRAINT trade_cancellations_signer_lowercase CHECK (((signer)::text = lower((signer)::text)))
);


--
-- Name: trade_chain_state; Type: TABLE; Schema: shop; Owner: -
--

CREATE TABLE shop.trade_chain_state (
    trade_id uuid NOT NULL,
    network text NOT NULL,
    digest text,
    executions integer DEFAULT 0 NOT NULL,
    status text NOT NULL,
    executed_at bigint,
    folded_block bigint NOT NULL,
    CONSTRAINT trade_chain_state_executions_nonneg CHECK ((executions >= 0)),
    CONSTRAINT trade_chain_state_status_known CHECK ((status = ANY (ARRAY['open'::text, 'sold'::text, 'cancelled'::text])))
);


--
-- Name: trade_event; Type: TABLE; Schema: shop; Owner: -
--

CREATE TABLE shop.trade_event (
    network text NOT NULL,
    block_number bigint NOT NULL,
    log_index integer NOT NULL,
    block_time bigint NOT NULL,
    marketplace_address character varying(42) NOT NULL,
    kind text NOT NULL,
    caller character varying(42) NOT NULL,
    hashed_signature text,
    new_index numeric(78,0),
    signer character varying(42),
    assets jsonb,
    tx_hash text NOT NULL,
    CONSTRAINT trade_event_index_kinds CHECK (((kind = ANY (ARRAY['signer_index_increased'::text, 'contract_index_increased'::text])) = (new_index IS NOT NULL))),
    CONSTRAINT trade_event_kind_known CHECK ((kind = ANY (ARRAY['traded'::text, 'signature_cancelled'::text, 'signer_index_increased'::text, 'contract_index_increased'::text]))),
    CONSTRAINT trade_event_lowercase CHECK ((((marketplace_address)::text = lower((marketplace_address)::text)) AND ((caller)::text = lower((caller)::text)))),
    CONSTRAINT trade_event_network_known CHECK ((network = ANY (ARRAY['ethereum'::text, 'matic'::text]))),
    CONSTRAINT trade_event_signature_kinds CHECK (((kind = ANY (ARRAY['traded'::text, 'signature_cancelled'::text])) = (hashed_signature IS NOT NULL)))
);


--
-- Name: trades; Type: TABLE; Schema: shop; Owner: -
--

CREATE TABLE shop.trades (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    network text NOT NULL,
    chain_id integer NOT NULL,
    signature text NOT NULL,
    hashed_signature text NOT NULL,
    checks jsonb NOT NULL,
    signer character varying(42) NOT NULL,
    type shop.trade_type NOT NULL,
    expires_at timestamp(3) without time zone NOT NULL,
    effective_since timestamp(3) without time zone NOT NULL,
    contract text DEFAULT ''::text NOT NULL,
    created_at timestamp(3) without time zone DEFAULT (timezone('UTC'::text, now()))::timestamp(3) without time zone NOT NULL,
    mirrored_from text,
    mirrored_at timestamp without time zone,
    CONSTRAINT trades_signer_lowercase CHECK (((signer)::text = lower((signer)::text)))
);


--
-- Name: trades_mirror; Type: TABLE; Schema: shop; Owner: -
--

CREATE TABLE shop.trades_mirror (
    id integer NOT NULL,
    upstream_url text NOT NULL,
    pass bigint DEFAULT 0 NOT NULL,
    next_skip bigint DEFAULT 0 NOT NULL,
    started_at timestamp without time zone,
    completed_at timestamp without time zone,
    pages_fetched bigint DEFAULT 0 NOT NULL,
    rows_fetched bigint DEFAULT 0 NOT NULL,
    rows_inserted bigint DEFAULT 0 NOT NULL,
    rows_failed bigint DEFAULT 0 NOT NULL,
    bytes_downloaded bigint DEFAULT 0 NOT NULL,
    list_downloads bigint DEFAULT 0 NOT NULL,
    last_error text,
    updated_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    CONSTRAINT trades_mirror_id_check CHECK ((id = 1))
);


--
-- Name: treasury_drift_checks; Type: TABLE; Schema: shop; Owner: -
--

CREATE TABLE shop.treasury_drift_checks (
    id bigint NOT NULL,
    expected_treasury_usdc numeric(78,0) NOT NULL,
    actual_treasury_usdc numeric(78,0) NOT NULL,
    usdc_drift_bps bigint NOT NULL,
    expected_credits_manager_mana numeric(78,0) NOT NULL,
    actual_credits_manager_mana numeric(78,0) NOT NULL,
    mana_drift_bps bigint NOT NULL,
    usdc_receivable numeric(78,0) NOT NULL,
    healthy boolean NOT NULL,
    checked_at bigint NOT NULL
);


--
-- Name: treasury_drift_checks_id_seq; Type: SEQUENCE; Schema: shop; Owner: -
--

ALTER TABLE shop.treasury_drift_checks ALTER COLUMN id ADD GENERATED ALWAYS AS IDENTITY (
    SEQUENCE NAME shop.treasury_drift_checks_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1
);


--
-- Name: treasury_ledger; Type: TABLE; Schema: shop; Owner: -
--

CREATE TABLE shop.treasury_ledger (
    id uuid NOT NULL,
    type text NOT NULL,
    usdc_delta numeric(78,0) DEFAULT 0 NOT NULL,
    mana_delta numeric(78,0) DEFAULT 0 NOT NULL,
    reference text,
    metadata jsonb,
    created_at bigint NOT NULL,
    CONSTRAINT treasury_ledger_type_known CHECK ((type = ANY (ARRAY['usdc_deposit'::text, 'usdc_settlement'::text, 'refill'::text, 'fee_retained'::text, 'withdrawal_recorded'::text])))
);


--
-- Name: treasury_pending_refill; Type: TABLE; Schema: shop; Owner: -
--

CREATE TABLE shop.treasury_pending_refill (
    id uuid NOT NULL,
    idempotency_key text NOT NULL,
    status text NOT NULL,
    usdc_to_spend numeric(78,0) NOT NULL,
    mana_to_acquire numeric(78,0) NOT NULL,
    swap_nonce bigint,
    swap_raw_tx bytea,
    swap_tx_hash text,
    usdc_spent numeric(78,0),
    mana_acquired numeric(78,0),
    oracle_price numeric(78,0),
    transfer_nonce bigint,
    transfer_raw_tx bytea,
    transfer_tx_hash text,
    ledger_entry_id uuid,
    abort_reason text,
    created_at bigint NOT NULL,
    updated_at bigint NOT NULL,
    CONSTRAINT treasury_pending_refill_amounts_positive CHECK (((usdc_to_spend > (0)::numeric) AND (mana_to_acquire > (0)::numeric))),
    CONSTRAINT treasury_pending_refill_status_known CHECK ((status = ANY (ARRAY['pending'::text, 'swap_sent'::text, 'swapped'::text, 'transfer_sent'::text, 'transferred'::text, 'settled'::text, 'aborted'::text])))
);


--
-- Name: usage_grants; Type: TABLE; Schema: shop; Owner: -
--

CREATE TABLE shop.usage_grants (
    id bigint NOT NULL,
    grantee_address character varying(42) NOT NULL,
    urn text NOT NULL,
    token_id text,
    collection character varying(42),
    category text NOT NULL,
    escrow_ref text,
    granted_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    unlock_at timestamp without time zone NOT NULL,
    status text DEFAULT 'active'::text NOT NULL,
    CONSTRAINT usage_grants_category_known CHECK ((category = ANY (ARRAY['wearable'::text, 'emote'::text]))),
    CONSTRAINT usage_grants_grantee_lowercase CHECK (((grantee_address)::text = lower((grantee_address)::text))),
    CONSTRAINT usage_grants_status_known CHECK ((status = ANY (ARRAY['active'::text, 'revoked'::text, 'released'::text])))
);


--
-- Name: usage_grants_id_seq; Type: SEQUENCE; Schema: shop; Owner: -
--

ALTER TABLE shop.usage_grants ALTER COLUMN id ADD GENERATED ALWAYS AS IDENTITY (
    SEQUENCE NAME shop.usage_grants_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1
);


--
-- Name: rental_assets; Type: TABLE; Schema: signatures; Owner: -
--

CREATE TABLE signatures.rental_assets (
    contract_address character varying(42) NOT NULL,
    token_id numeric(78,0) NOT NULL,
    nft_id text NOT NULL,
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
    CONSTRAINT rental_assets_category_known CHECK ((category = ANY (ARRAY['parcel'::text, 'estate'::text]))),
    CONSTRAINT rental_assets_contract_lowercase CHECK (((contract_address)::text = lower((contract_address)::text)))
);


--
-- Name: rental_listings; Type: TABLE; Schema: signatures; Owner: -
--

CREATE TABLE signatures.rental_listings (
    id uuid NOT NULL,
    lessor character varying(42) NOT NULL,
    tenant character varying(42),
    CONSTRAINT rental_listings_lessor_lowercase CHECK (((lessor)::text = lower((lessor)::text))),
    CONSTRAINT rental_listings_tenant_lowercase CHECK (((tenant IS NULL) OR ((tenant)::text = lower((tenant)::text))))
);


--
-- Name: rental_periods; Type: TABLE; Schema: signatures; Owner: -
--

CREATE TABLE signatures.rental_periods (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    rental_id uuid NOT NULL,
    min_days integer NOT NULL,
    max_days integer NOT NULL,
    price_per_day numeric(78,0) NOT NULL,
    CONSTRAINT rental_periods_max_ge_min CHECK ((max_days >= min_days)),
    CONSTRAINT rental_periods_min_nonneg CHECK ((min_days >= 0)),
    CONSTRAINT rental_periods_price_nonneg CHECK ((price_per_day >= (0)::numeric))
);


--
-- Name: rentals; Type: TABLE; Schema: signatures; Owner: -
--

CREATE TABLE signatures.rentals (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    contract_address character varying(42) NOT NULL,
    token_id numeric(78,0) NOT NULL,
    network text NOT NULL,
    chain_id integer NOT NULL,
    expiration timestamp without time zone NOT NULL,
    nonces text[] NOT NULL,
    signature text NOT NULL,
    rental_contract_address character varying(42) NOT NULL,
    status signatures.rental_status DEFAULT 'open'::signatures.rental_status NOT NULL,
    target character varying(42) DEFAULT '0x0000000000000000000000000000000000000000'::character varying NOT NULL,
    rented_days integer,
    period_chosen uuid,
    created_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    updated_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    started_at timestamp without time zone,
    CONSTRAINT rentals_nonces_three CHECK ((array_length(nonces, 1) = 3))
);


--
-- Name: archipelago_v4_assignments; Type: TABLE; Schema: social; Owner: -
--

CREATE TABLE social.archipelago_v4_assignments (
    deployment_audience text NOT NULL,
    owner_address text NOT NULL,
    lane_key text NOT NULL,
    authority_incarnation text NOT NULL,
    assignment_revision bigint NOT NULL,
    owner_session text NOT NULL,
    owner_epoch bigint NOT NULL,
    fencing_token bigint NOT NULL,
    assignment_json jsonb,
    acknowledged_revision bigint DEFAULT 0 NOT NULL,
    updated_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL
);


--
-- Name: archipelago_v4_connection_epoch; Type: SEQUENCE; Schema: social; Owner: -
--

CREATE SEQUENCE social.archipelago_v4_connection_epoch
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: archipelago_v4_meta; Type: TABLE; Schema: social; Owner: -
--

CREATE TABLE social.archipelago_v4_meta (
    key text NOT NULL,
    value text NOT NULL
);


--
-- Name: badge_admin_audit; Type: TABLE; Schema: social; Owner: -
--

CREATE TABLE social.badge_admin_audit (
    id bigint NOT NULL,
    action text NOT NULL,
    address text NOT NULL,
    badge_id text NOT NULL,
    tier_id text,
    actor text NOT NULL,
    acted_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    CONSTRAINT badge_admin_audit_action_check CHECK ((action = ANY (ARRAY['grant'::text, 'revoke'::text])))
);


--
-- Name: badge_admin_audit_id_seq; Type: SEQUENCE; Schema: social; Owner: -
--

ALTER TABLE social.badge_admin_audit ALTER COLUMN id ADD GENERATED ALWAYS AS IDENTITY (
    SEQUENCE NAME social.badge_admin_audit_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1
);


--
-- Name: badge_definitions; Type: TABLE; Schema: social; Owner: -
--

CREATE TABLE social.badge_definitions (
    id text NOT NULL,
    name text NOT NULL,
    description text,
    category text,
    is_tier boolean DEFAULT false NOT NULL,
    assets jsonb DEFAULT '{}'::jsonb NOT NULL
);


--
-- Name: badge_tiers; Type: TABLE; Schema: social; Owner: -
--

CREATE TABLE social.badge_tiers (
    badge_id text NOT NULL,
    tier_id text NOT NULL,
    tier_name text NOT NULL,
    description text,
    assets jsonb DEFAULT '{}'::jsonb NOT NULL,
    criteria_steps integer DEFAULT 0 NOT NULL,
    ordinal integer DEFAULT 0 NOT NULL
);


--
-- Name: blocks; Type: TABLE; Schema: social; Owner: -
--

CREATE TABLE social.blocks (
    blocker_address text NOT NULL,
    blocked_address text NOT NULL,
    blocked_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    CONSTRAINT blocks_not_self_check CHECK ((blocker_address <> blocked_address))
);


--
-- Name: communities; Type: TABLE; Schema: social; Owner: -
--

CREATE TABLE social.communities (
    id uuid NOT NULL,
    name text NOT NULL,
    description text DEFAULT ''::text NOT NULL,
    owner_address text NOT NULL,
    private boolean DEFAULT false NOT NULL,
    active boolean DEFAULT true NOT NULL,
    unlisted boolean DEFAULT false NOT NULL,
    editors_choice boolean DEFAULT false NOT NULL,
    ranking_score real DEFAULT 0 NOT NULL,
    suspended boolean DEFAULT false NOT NULL,
    suspended_at timestamp without time zone,
    suspended_by text,
    suspension_reason text,
    created_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    updated_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    origin text DEFAULT 'local'::text NOT NULL,
    voice_notification_scope text,
    voice_notification_expires_at timestamp without time zone,
    voice_notification_generation uuid,
    ranking_events_count integer DEFAULT 0 NOT NULL,
    ranking_photos_count integer DEFAULT 0 NOT NULL,
    ranking_streams_count integer DEFAULT 0 NOT NULL,
    ranking_events_total_attendees integer DEFAULT 0 NOT NULL,
    ranking_streams_total_participants integer DEFAULT 0 NOT NULL,
    ranking_tracking_since timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    CONSTRAINT communities_origin_known CHECK ((origin = ANY (ARRAY['upstream'::text, 'local'::text]))),
    CONSTRAINT communities_ranking_nonnegative CHECK (((ranking_events_count >= 0) AND (ranking_photos_count >= 0) AND (ranking_streams_count >= 0) AND (ranking_events_total_attendees >= 0) AND (ranking_streams_total_participants >= 0))),
    CONSTRAINT communities_suspension_check CHECK (((suspended AND (suspended_at IS NOT NULL) AND (suspended_by IS NOT NULL)) OR ((NOT suspended) AND (suspended_at IS NULL) AND (suspended_by IS NULL) AND (suspension_reason IS NULL)))),
    CONSTRAINT communities_voice_notification_scope_check CHECK ((((voice_notification_scope IS NULL) AND (voice_notification_expires_at IS NULL) AND (voice_notification_generation IS NULL)) OR ((voice_notification_scope IS NOT NULL) AND (voice_notification_scope = ANY (ARRAY['all'::text, 'members'::text])) AND (voice_notification_expires_at IS NOT NULL) AND (voice_notification_generation IS NOT NULL))))
);


--
-- Name: community_bans; Type: TABLE; Schema: social; Owner: -
--

CREATE TABLE social.community_bans (
    community_id uuid NOT NULL,
    banned_address text NOT NULL,
    banned_by text NOT NULL,
    banned_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    reason text,
    active boolean DEFAULT true NOT NULL,
    unbanned_by text,
    unbanned_at timestamp without time zone,
    CONSTRAINT community_bans_lift_check CHECK (((active AND (unbanned_by IS NULL) AND (unbanned_at IS NULL)) OR ((NOT active) AND (unbanned_by IS NOT NULL) AND (unbanned_at IS NOT NULL))))
);


--
-- Name: community_compliance_cache; Type: TABLE; Schema: social; Owner: -
--

CREATE TABLE social.community_compliance_cache (
    model text NOT NULL,
    input_hash bytea NOT NULL,
    result jsonb NOT NULL,
    expires_at timestamp without time zone NOT NULL,
    CONSTRAINT community_compliance_cache_input_hash_check CHECK ((octet_length(input_hash) = 32))
);


--
-- Name: community_contents; Type: TABLE; Schema: social; Owner: -
--

CREATE TABLE social.community_contents (
    content_hash text NOT NULL,
    cid text NOT NULL,
    size integer NOT NULL,
    stored_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    CONSTRAINT community_contents_hash_shape CHECK ((content_hash ~ '^[0-9a-f]{64}$'::text)),
    CONSTRAINT community_contents_size_check CHECK (((size >= 0) AND (size <= 262144)))
);


--
-- Name: community_members; Type: TABLE; Schema: social; Owner: -
--

CREATE TABLE social.community_members (
    community_id uuid NOT NULL,
    member_address text NOT NULL,
    role text NOT NULL,
    joined_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    CONSTRAINT community_members_role_check CHECK ((role = ANY (ARRAY['owner'::text, 'moderator'::text, 'member'::text])))
);


--
-- Name: community_mirror; Type: TABLE; Schema: social; Owner: -
--

CREATE TABLE social.community_mirror (
    community_id uuid NOT NULL,
    source_url text NOT NULL,
    members_count bigint DEFAULT 0 NOT NULL,
    thumbnail_url text,
    fetched_at timestamp without time zone NOT NULL,
    CONSTRAINT community_mirror_members_count_check CHECK ((members_count >= 0))
);


--
-- Name: community_mirror_state; Type: TABLE; Schema: social; Owner: -
--

CREATE TABLE social.community_mirror_state (
    source_url text NOT NULL,
    pass_started_at timestamp without time zone,
    next_offset bigint,
    pass_rows bigint DEFAULT 0 NOT NULL,
    last_pass_at timestamp without time zone,
    last_pass_complete boolean,
    last_pass_rows bigint,
    last_pass_deactivated bigint,
    last_complete_pass_at timestamp without time zone,
    last_error text,
    updated_at timestamp without time zone NOT NULL
);


--
-- Name: community_places; Type: TABLE; Schema: social; Owner: -
--

CREATE TABLE social.community_places (
    place_id text NOT NULL,
    community_id uuid NOT NULL,
    added_by text NOT NULL,
    added_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL
);


--
-- Name: community_post_likes; Type: TABLE; Schema: social; Owner: -
--

CREATE TABLE social.community_post_likes (
    post_id uuid NOT NULL,
    user_address text NOT NULL,
    liked_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL
);


--
-- Name: community_posts; Type: TABLE; Schema: social; Owner: -
--

CREATE TABLE social.community_posts (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    community_id uuid NOT NULL,
    author_address text NOT NULL,
    content text NOT NULL,
    created_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL
);


--
-- Name: community_ranking_events; Type: TABLE; Schema: social; Owner: -
--

CREATE TABLE social.community_ranking_events (
    kind text NOT NULL,
    event_key text NOT NULL,
    community_id uuid NOT NULL,
    occurred_at timestamp without time zone NOT NULL,
    CONSTRAINT community_ranking_events_kind_check CHECK ((kind = ANY (ARRAY['photo'::text, 'event'::text, 'stream'::text])))
);


--
-- Name: community_requests; Type: TABLE; Schema: social; Owner: -
--

CREATE TABLE social.community_requests (
    id uuid NOT NULL,
    community_id uuid NOT NULL,
    member_address text NOT NULL,
    type text NOT NULL,
    status text DEFAULT 'pending'::text NOT NULL,
    created_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    updated_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    CONSTRAINT community_requests_status_check CHECK ((status = ANY (ARRAY['pending'::text, 'accepted'::text, 'rejected'::text, 'cancelled'::text]))),
    CONSTRAINT community_requests_type_check CHECK ((type = ANY (ARRAY['invite'::text, 'request_to_join'::text])))
);


--
-- Name: community_seen_nonces; Type: TABLE; Schema: social; Owner: -
--

CREATE TABLE social.community_seen_nonces (
    signer text NOT NULL,
    nonce text NOT NULL,
    expires_at bigint NOT NULL
);


--
-- Name: community_signed_actions; Type: TABLE; Schema: social; Owner: -
--

CREATE TABLE social.community_signed_actions (
    signature_hash text NOT NULL,
    seq bigint NOT NULL,
    community_id text NOT NULL,
    community_uuid uuid NOT NULL,
    kind text NOT NULL,
    signer text NOT NULL,
    target text,
    role text,
    action text,
    content_hash text,
    signed_at bigint NOT NULL,
    payload jsonb NOT NULL,
    received_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    CONSTRAINT community_signed_actions_action_check CHECK ((((kind = ANY (ARRAY['place'::text, 'request_status'::text])) = (action IS NOT NULL)) AND ((action IS NULL) OR (action = ANY (ARRAY['add'::text, 'remove'::text, 'accepted'::text, 'rejected'::text, 'cancelled'::text]))))),
    CONSTRAINT community_signed_actions_kind_check CHECK ((kind = ANY (ARRAY['create'::text, 'update'::text, 'delete'::text, 'role'::text, 'post'::text, 'post_delete'::text, 'post_like'::text, 'post_unlike'::text, 'place'::text, 'request_status'::text]))),
    CONSTRAINT community_signed_actions_post_check CHECK (((kind = 'post'::text) = (content_hash IS NOT NULL))),
    CONSTRAINT community_signed_actions_role_check CHECK ((((kind = ANY (ARRAY['create'::text, 'role'::text])) = (role IS NOT NULL)) AND ((role IS NULL) OR (role = ANY (ARRAY['owner'::text, 'mod'::text, 'member'::text, 'banned'::text, 'none'::text]))))),
    CONSTRAINT community_signed_actions_signer_lowercase CHECK ((signer = lower(signer))),
    CONSTRAINT community_signed_actions_target_check CHECK (((kind = ANY (ARRAY['update'::text, 'delete'::text, 'post'::text])) = (target IS NULL)))
);


--
-- Name: community_signed_actions_seq_seq; Type: SEQUENCE; Schema: social; Owner: -
--

ALTER TABLE social.community_signed_actions ALTER COLUMN seq ADD GENERATED ALWAYS AS IDENTITY (
    SEQUENCE NAME social.community_signed_actions_seq_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1
);


--
-- Name: community_thumbnails; Type: TABLE; Schema: social; Owner: -
--

CREATE TABLE social.community_thumbnails (
    community_id uuid NOT NULL,
    thumbnail_hash text NOT NULL,
    updated_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL
);


--
-- Name: community_voice_chat_users; Type: TABLE; Schema: social; Owner: -
--

CREATE TABLE social.community_voice_chat_users (
    community_id uuid NOT NULL,
    address text NOT NULL,
    is_moderator boolean DEFAULT false NOT NULL,
    status text NOT NULL,
    sid text,
    joined_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    status_updated_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    created_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    CONSTRAINT community_voice_chat_users_status_check CHECK ((status = ANY (ARRAY['connected'::text, 'connection_interrupted'::text, 'disconnected'::text, 'not_connected'::text])))
);


--
-- Name: event_notification_intents; Type: TABLE; Schema: social; Owner: -
--

CREATE TABLE social.event_notification_intents (
    id uuid NOT NULL,
    event_id text NOT NULL,
    kind text NOT NULL,
    event jsonb NOT NULL,
    local_event boolean NOT NULL,
    occurrence_start timestamp without time zone,
    occurred_at timestamp without time zone NOT NULL,
    dispatch_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    recipient_after text DEFAULT ''::text NOT NULL,
    processed_at timestamp without time zone,
    CONSTRAINT event_notification_intents_kind_check CHECK ((kind = ANY (ARRAY['event_approved'::text, 'event_rejected'::text, 'event_deleted'::text, 'event_created'::text, 'events_starts_soon'::text, 'events_started'::text]))),
    CONSTRAINT event_notification_intents_occurrence_check CHECK (((kind = ANY (ARRAY['events_starts_soon'::text, 'events_started'::text])) = (occurrence_start IS NOT NULL)))
);


--
-- Name: event_payloads; Type: TABLE; Schema: social; Owner: -
--

CREATE TABLE social.event_payloads (
    id uuid NOT NULL,
    payload bytea NOT NULL,
    expires_at timestamp without time zone NOT NULL,
    CONSTRAINT event_payloads_size_check CHECK (((octet_length(payload) > 0) AND (octet_length(payload) <= 536870912)))
);


--
-- Name: fed_instance_identity; Type: TABLE; Schema: social; Owner: -
--

CREATE TABLE social.fed_instance_identity (
    only_row boolean DEFAULT true NOT NULL,
    peer_id text NOT NULL,
    created_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    CONSTRAINT fed_instance_identity_only_row_check CHECK (only_row)
);


--
-- Name: first_wear_baseline; Type: TABLE; Schema: social; Owner: -
--

CREATE TABLE social.first_wear_baseline (
    address text NOT NULL,
    seeded_at timestamp(3) without time zone NOT NULL
);


--
-- Name: first_wear_cursor; Type: TABLE; Schema: social; Owner: -
--

CREATE TABLE social.first_wear_cursor (
    id integer DEFAULT 1 NOT NULL,
    last_seen timestamp without time zone NOT NULL,
    CONSTRAINT first_wear_cursor_singleton_check CHECK ((id = 1))
);


--
-- Name: friend_messages; Type: TABLE; Schema: social; Owner: -
--

CREATE TABLE social.friend_messages (
    id bigint NOT NULL,
    sender_address text NOT NULL,
    recipient_address text NOT NULL,
    body text NOT NULL,
    created_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    CONSTRAINT friend_messages_not_self_check CHECK ((sender_address <> recipient_address))
);


--
-- Name: friend_messages_id_seq; Type: SEQUENCE; Schema: social; Owner: -
--

ALTER TABLE social.friend_messages ALTER COLUMN id ADD GENERATED ALWAYS AS IDENTITY (
    SEQUENCE NAME social.friend_messages_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1
);


--
-- Name: friendship_actions; Type: TABLE; Schema: social; Owner: -
--

CREATE TABLE social.friendship_actions (
    id uuid NOT NULL,
    friendship_id uuid NOT NULL,
    action text NOT NULL,
    acting_user text NOT NULL,
    metadata json,
    "timestamp" timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    CONSTRAINT friendship_actions_action_check CHECK ((action = ANY (ARRAY['request'::text, 'accept'::text, 'cancel'::text, 'reject'::text, 'delete'::text, 'block'::text])))
);


--
-- Name: friendship_mutation_limits; Type: TABLE; Schema: social; Owner: -
--

CREATE TABLE social.friendship_mutation_limits (
    key text NOT NULL,
    attempts bigint NOT NULL,
    expires_at_ms bigint,
    CONSTRAINT friendship_mutation_limits_attempts_check CHECK ((attempts > 0))
);


--
-- Name: friendships; Type: TABLE; Schema: social; Owner: -
--

CREATE TABLE social.friendships (
    id uuid NOT NULL,
    address_requester text NOT NULL,
    address_requested text NOT NULL,
    is_active boolean DEFAULT false NOT NULL,
    last_action text NOT NULL,
    last_actor text NOT NULL,
    last_action_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    created_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    updated_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    CONSTRAINT friendships_active_projection_check CHECK ((is_active = (last_action = 'accept'::text))),
    CONSTRAINT friendships_last_action_check CHECK ((last_action = ANY (ARRAY['request'::text, 'accept'::text, 'cancel'::text, 'reject'::text, 'delete'::text, 'block'::text]))),
    CONSTRAINT friendships_last_actor_check CHECK (((last_actor = address_requester) OR (last_actor = address_requested))),
    CONSTRAINT friendships_not_self_check CHECK ((address_requester <> address_requested))
);


--
-- Name: island_admission_fences; Type: TABLE; Schema: social; Owner: -
--

CREATE TABLE social.island_admission_fences (
    wallet text NOT NULL,
    generation bigint DEFAULT 1 NOT NULL,
    token_floor bigint DEFAULT 0 NOT NULL,
    updated_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    CONSTRAINT island_admission_fences_generation_check CHECK ((generation > 0)),
    CONSTRAINT island_admission_fences_token_floor_check CHECK (((token_floor >= 0) AND (token_floor <= '9007199254740'::bigint))),
    CONSTRAINT island_admission_fences_wallet_check CHECK (((wallet <> ''::text) AND (wallet = lower(wallet))))
);


--
-- Name: island_cleanup_dispatches; Type: TABLE; Schema: social; Owner: -
--

CREATE TABLE social.island_cleanup_dispatches (
    dispatch_id uuid NOT NULL,
    wallet text NOT NULL,
    purpose text NOT NULL,
    pulse_source text,
    pulse_takeover_id text,
    enforcement_cause text,
    intent_root_key text,
    intent_target_key text,
    intent_generation uuid,
    room_name text NOT NULL,
    participant_identity text NOT NULL,
    cutoff bigint NOT NULL,
    fence_generation bigint NOT NULL,
    phase text NOT NULL,
    state text NOT NULL,
    quarantine_until timestamp without time zone,
    created_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    settled_at timestamp without time zone,
    CONSTRAINT island_cleanup_dispatches_check CHECK ((((purpose = 'pulse_takeover'::text) AND (pulse_source IS NOT NULL) AND (pulse_takeover_id IS NOT NULL) AND (pulse_source <> ''::text) AND (pulse_takeover_id <> ''::text) AND (enforcement_cause IS NULL) AND (intent_root_key IS NULL) AND (intent_target_key IS NULL) AND (intent_generation IS NULL)) OR ((purpose = 'island_enforcement'::text) AND (pulse_source IS NULL) AND (pulse_takeover_id IS NULL) AND (enforcement_cause IS NOT NULL) AND (enforcement_cause = ANY (ARRAY['connection'::text, 'platform'::text, 'block'::text, 'scene'::text])) AND (intent_root_key IS NOT NULL) AND (intent_root_key <> ''::text) AND (intent_target_key IS NOT NULL) AND (intent_target_key <> ''::text) AND (intent_generation IS NOT NULL)))),
    CONSTRAINT island_cleanup_dispatches_check1 CHECK (((state = 'quarantine'::text) = (quarantine_until IS NOT NULL))),
    CONSTRAINT island_cleanup_dispatches_check2 CHECK (((state = 'dispatched'::text) = (settled_at IS NULL))),
    CONSTRAINT island_cleanup_dispatches_cutoff_check CHECK (((cutoff >= 1) AND (cutoff <= '9007199254740'::bigint))),
    CONSTRAINT island_cleanup_dispatches_fence_generation_check CHECK ((fence_generation > 0)),
    CONSTRAINT island_cleanup_dispatches_participant_identity_check CHECK ((participant_identity <> ''::text)),
    CONSTRAINT island_cleanup_dispatches_phase_check CHECK ((phase = ANY (ARRAY['initial'::text, 'quarantine_final'::text]))),
    CONSTRAINT island_cleanup_dispatches_purpose_check CHECK ((purpose = ANY (ARRAY['pulse_takeover'::text, 'island_enforcement'::text]))),
    CONSTRAINT island_cleanup_dispatches_room_name_check CHECK ((room_name <> ''::text)),
    CONSTRAINT island_cleanup_dispatches_state_check CHECK ((state = ANY (ARRAY['dispatched'::text, 'quarantine'::text, 'confirmed'::text, 'rejected'::text, 'settled_late'::text, 'superseded'::text])))
);


--
-- Name: island_takeover_receipts; Type: TABLE; Schema: social; Owner: -
--

CREATE TABLE social.island_takeover_receipts (
    wallet text NOT NULL,
    source_incarnation text NOT NULL,
    takeover_id text NOT NULL,
    predecessor_session text NOT NULL,
    predecessor_cluster text NOT NULL,
    predecessor_realm text NOT NULL,
    latest_dispatch uuid,
    acknowledged boolean DEFAULT false NOT NULL,
    retired_by_source text,
    last_successor_session text NOT NULL,
    next_attempt_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    claim_id uuid,
    claim_until timestamp without time zone,
    created_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    CONSTRAINT island_takeover_receipts_check CHECK (((source_incarnation <> ''::text) AND (takeover_id <> ''::text) AND (predecessor_session <> ''::text) AND (predecessor_cluster <> ''::text))),
    CONSTRAINT island_takeover_receipts_check1 CHECK (((claim_id IS NULL) = (claim_until IS NULL))),
    CONSTRAINT island_takeover_receipts_check2 CHECK (((NOT acknowledged) OR (latest_dispatch IS NOT NULL))),
    CONSTRAINT island_takeover_receipts_retirement_check CHECK (((retired_by_source IS NULL) OR ((retired_by_source <> ''::text) AND (retired_by_source <> source_incarnation) AND (NOT acknowledged) AND (latest_dispatch IS NOT NULL))))
);


--
-- Name: land_transfer_cursor; Type: TABLE; Schema: social; Owner: -
--

CREATE TABLE social.land_transfer_cursor (
    id integer DEFAULT 1 NOT NULL,
    block_number bigint NOT NULL,
    log_index integer NOT NULL,
    updated_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    CONSTRAINT land_transfer_cursor_singleton_check CHECK ((id = 1))
);


--
-- Name: livekit_removal_intents; Type: TABLE; Schema: social; Owner: -
--

CREATE TABLE social.livekit_removal_intents (
    root_key text NOT NULL,
    target_key text DEFAULT ''::text NOT NULL,
    generation uuid NOT NULL,
    cause_kind text NOT NULL,
    wallet text NOT NULL,
    actor text NOT NULL,
    ban_id uuid,
    place_id text,
    scene_version timestamp without time zone,
    scope jsonb DEFAULT '[]'::jsonb NOT NULL,
    room_name text,
    room_sid text,
    participant_identity text,
    cursor_room text DEFAULT ''::text NOT NULL,
    cursor_sid text DEFAULT ''::text NOT NULL,
    cursor_identity text DEFAULT ''::text NOT NULL,
    attempts integer DEFAULT 0 NOT NULL,
    next_attempt_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    claim_id uuid,
    claim_until timestamp without time zone,
    created_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    CONSTRAINT livekit_removal_intents_attempts CHECK ((attempts >= 0)),
    CONSTRAINT livekit_removal_intents_cause CHECK ((cause_kind = ANY (ARRAY['platform'::text, 'scene'::text, 'block'::text, 'connection'::text]))),
    CONSTRAINT livekit_removal_intents_claim CHECK (((claim_id IS NULL) = (claim_until IS NULL))),
    CONSTRAINT livekit_removal_intents_scope CHECK ((jsonb_typeof(scope) = 'array'::text)),
    CONSTRAINT livekit_removal_intents_target CHECK (((target_key = ''::text) OR ((room_name IS NOT NULL) AND (room_sid IS NOT NULL) AND (participant_identity IS NOT NULL))))
);


--
-- Name: mls_commits; Type: TABLE; Schema: social; Owner: -
--

CREATE TABLE social.mls_commits (
    group_id text NOT NULL,
    epoch bigint NOT NULL,
    commit_cid text NOT NULL,
    welcome_cid text,
    committer text NOT NULL,
    commit_hash text NOT NULL,
    signed_at bigint NOT NULL,
    received_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL
);


--
-- Name: mls_group_members; Type: TABLE; Schema: social; Owner: -
--

CREATE TABLE social.mls_group_members (
    group_id text NOT NULL,
    member text NOT NULL,
    added_epoch bigint NOT NULL,
    removed_epoch bigint
);


--
-- Name: mls_groups; Type: TABLE; Schema: social; Owner: -
--

CREATE TABLE social.mls_groups (
    group_id text NOT NULL,
    creator text NOT NULL,
    group_kind text NOT NULL,
    community_id uuid,
    epoch_author text NOT NULL,
    current_epoch bigint DEFAULT 0 NOT NULL,
    ciphersuite integer NOT NULL,
    last_commit_hash text,
    created_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    updated_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    CONSTRAINT mls_groups_kind_check CHECK ((group_kind = ANY (ARRAY['dm'::text, 'channel'::text])))
);


--
-- Name: mls_key_packages; Type: TABLE; Schema: social; Owner: -
--

CREATE TABLE social.mls_key_packages (
    id bigint NOT NULL,
    owner text NOT NULL,
    ref_hash text NOT NULL,
    ciphersuite integer NOT NULL,
    key_package bytea NOT NULL,
    consumed_at timestamp without time zone,
    created_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL
);


--
-- Name: mls_key_packages_id_seq; Type: SEQUENCE; Schema: social; Owner: -
--

ALTER TABLE social.mls_key_packages ALTER COLUMN id ADD GENERATED ALWAYS AS IDENTITY (
    SEQUENCE NAME social.mls_key_packages_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1
);


--
-- Name: mls_message_refs; Type: TABLE; Schema: social; Owner: -
--

CREATE TABLE social.mls_message_refs (
    signature_hash text NOT NULL,
    group_id text NOT NULL,
    author text NOT NULL,
    epoch bigint NOT NULL,
    ciphertext_hash text NOT NULL,
    ciphertext_cid text NOT NULL,
    signed_at bigint NOT NULL,
    received_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL
);


--
-- Name: notification_deliveries; Type: TABLE; Schema: social; Owner: -
--

CREATE TABLE social.notification_deliveries (
    notification_id uuid NOT NULL,
    channel text NOT NULL,
    state text DEFAULT 'pending'::text NOT NULL,
    attempts integer DEFAULT 0 NOT NULL,
    next_attempt_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    last_error text,
    updated_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    CONSTRAINT notification_deliveries_attempts_check CHECK ((attempts >= 0)),
    CONSTRAINT notification_deliveries_channel_check CHECK ((channel = ANY (ARRAY['email'::text, 'push'::text]))),
    CONSTRAINT notification_deliveries_state_check CHECK ((state = ANY (ARRAY['pending'::text, 'sent'::text, 'failed'::text, 'skipped'::text])))
);


--
-- Name: notification_reader_seen; Type: TABLE; Schema: social; Owner: -
--

CREATE TABLE social.notification_reader_seen (
    address text NOT NULL,
    last_fetch_at timestamp(3) without time zone NOT NULL
);


--
-- Name: notifications; Type: TABLE; Schema: social; Owner: -
--

CREATE TABLE social.notifications (
    id uuid NOT NULL,
    address text NOT NULL,
    type text NOT NULL,
    metadata jsonb DEFAULT '{}'::jsonb NOT NULL,
    broadcast_key text,
    occurred_at timestamp(3) without time zone NOT NULL,
    read boolean DEFAULT false NOT NULL,
    read_at timestamp(3) without time zone,
    created_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    in_app boolean DEFAULT true NOT NULL,
    CONSTRAINT notifications_read_check CHECK (((read AND (read_at IS NOT NULL)) OR ((NOT read) AND (read_at IS NULL))))
);


--
-- Name: player_connections; Type: TABLE; Schema: social; Owner: -
--

CREATE TABLE social.player_connections (
    address text NOT NULL,
    ip_address text,
    device_id text,
    created_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    updated_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL
);


--
-- Name: player_report_evidence; Type: TABLE; Schema: social; Owner: -
--

CREATE TABLE social.player_report_evidence (
    report_id uuid NOT NULL,
    evidence_key text NOT NULL,
    reporter_address text NOT NULL,
    filename text NOT NULL,
    content_type text NOT NULL,
    declared_size bigint NOT NULL,
    content_hash text,
    uploaded_at timestamp without time zone,
    created_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    CONSTRAINT player_report_evidence_upload_check CHECK (((uploaded_at IS NULL) = (content_hash IS NULL)))
);


--
-- Name: player_reports; Type: TABLE; Schema: social; Owner: -
--

CREATE TABLE social.player_reports (
    id uuid NOT NULL,
    reporter_address text NOT NULL,
    reported_address text NOT NULL,
    reason text NOT NULL,
    description text NOT NULL,
    additional_comments text,
    evidence_keys text[] DEFAULT '{}'::text[] NOT NULL,
    status text DEFAULT 'open'::text NOT NULL,
    created_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL
);


--
-- Name: private_voice_chat_participants; Type: TABLE; Schema: social; Owner: -
--

CREATE TABLE social.private_voice_chat_participants (
    call_id uuid NOT NULL,
    address text NOT NULL,
    status text NOT NULL,
    joined_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    status_updated_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    CONSTRAINT private_voice_chat_participants_status_check CHECK ((status = ANY (ARRAY['connected'::text, 'connection_interrupted'::text, 'disconnected'::text, 'not_connected'::text])))
);


--
-- Name: private_voice_chats; Type: TABLE; Schema: social; Owner: -
--

CREATE TABLE social.private_voice_chats (
    id uuid NOT NULL,
    caller_address text NOT NULL,
    callee_address text NOT NULL,
    created_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    CONSTRAINT private_voice_chats_no_self_call_check CHECK ((caller_address <> callee_address))
);


--
-- Name: profile_images; Type: TABLE; Schema: social; Owner: -
--

CREATE TABLE social.profile_images (
    entity_id text NOT NULL,
    kind text NOT NULL,
    blob_cid text NOT NULL,
    byte_size integer NOT NULL,
    source text NOT NULL,
    stored_at timestamp without time zone NOT NULL,
    CONSTRAINT profile_images_kind_check CHECK ((kind = ANY (ARRAY['face'::text, 'body'::text]))),
    CONSTRAINT profile_images_size_check CHECK (((byte_size > 0) AND (byte_size <= 8388608))),
    CONSTRAINT profile_images_source_check CHECK ((source = ANY (ARRAY['render'::text, 'origin'::text])))
);


--
-- Name: referral_emails; Type: TABLE; Schema: social; Owner: -
--

CREATE TABLE social.referral_emails (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    referrer text NOT NULL,
    email text NOT NULL,
    created_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    updated_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    CONSTRAINT referral_emails_email_length_check CHECK ((char_length(email) <= 254))
);


--
-- Name: referral_progress; Type: TABLE; Schema: social; Owner: -
--

CREATE TABLE social.referral_progress (
    invited_user text NOT NULL,
    referrer text NOT NULL,
    status text DEFAULT 'pending'::text NOT NULL,
    created_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    updated_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    invited_user_ip text,
    signed_up_at timestamp without time zone,
    tier_granted boolean DEFAULT false NOT NULL,
    tier_granted_at timestamp without time zone,
    login_days date[] DEFAULT '{}'::date[] NOT NULL,
    CONSTRAINT referral_progress_not_self_check CHECK ((invited_user <> referrer)),
    CONSTRAINT referral_progress_status_check CHECK ((status = ANY (ARRAY['pending'::text, 'signed_up'::text, 'tier_granted'::text, 'rejected_ip_match'::text])))
);


--
-- Name: referral_progress_viewed; Type: TABLE; Schema: social; Owner: -
--

CREATE TABLE social.referral_progress_viewed (
    referrer text NOT NULL,
    invites_accepted_viewed bigint DEFAULT 0 NOT NULL,
    updated_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL
);


--
-- Name: referral_reward_images; Type: TABLE; Schema: social; Owner: -
--

CREATE TABLE social.referral_reward_images (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    referrer text NOT NULL,
    reward_image_url text NOT NULL,
    tier integer NOT NULL,
    created_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    CONSTRAINT referral_reward_images_tier_check CHECK ((tier > 0))
);


--
-- Name: registry_profile_cache; Type: TABLE; Schema: social; Owner: -
--

CREATE TABLE social.registry_profile_cache (
    profile_key text NOT NULL,
    profile jsonb NOT NULL,
    expires_at timestamp without time zone NOT NULL,
    CONSTRAINT registry_profile_cache_object CHECK ((jsonb_typeof(profile) = 'object'::text))
);


--
-- Name: scene_admins; Type: TABLE; Schema: social; Owner: -
--

CREATE TABLE social.scene_admins (
    place_id text NOT NULL,
    admin_address text NOT NULL,
    added_by text NOT NULL,
    added_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    active boolean DEFAULT true NOT NULL,
    removed_at timestamp without time zone,
    CONSTRAINT scene_admins_removed_check CHECK (((active AND (removed_at IS NULL)) OR ((NOT active) AND (removed_at IS NOT NULL))))
);


--
-- Name: scene_bans; Type: TABLE; Schema: social; Owner: -
--

CREATE TABLE social.scene_bans (
    place_id text NOT NULL,
    banned_address text NOT NULL,
    banned_by text NOT NULL,
    banned_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL
);


--
-- Name: scene_stream_access; Type: TABLE; Schema: social; Owner: -
--

CREATE TABLE social.scene_stream_access (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    place_id text NOT NULL,
    streaming_url text NOT NULL,
    streaming_key text NOT NULL,
    ingress_id text,
    room_id text,
    generated_by text NOT NULL,
    created_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    expiration_time timestamp without time zone,
    streaming boolean DEFAULT false NOT NULL,
    streaming_start_time timestamp without time zone,
    streaming_state_version bigint DEFAULT 0 NOT NULL,
    streaming_checked_at timestamp without time zone,
    cleanup_retry_at timestamp without time zone,
    active boolean DEFAULT true NOT NULL
);


--
-- Name: scene_stream_cleanup; Type: TABLE; Schema: social; Owner: -
--

CREATE TABLE social.scene_stream_cleanup (
    access_id uuid NOT NULL,
    place_id text NOT NULL,
    ingress_id text,
    notification_kind text,
    occurred_at timestamp without time zone NOT NULL,
    due_at timestamp without time zone NOT NULL,
    claim_token uuid,
    claim_until timestamp without time zone,
    attempts integer DEFAULT 0 NOT NULL,
    ingress_deleted boolean DEFAULT false NOT NULL,
    recipient text,
    metadata jsonb,
    source_context jsonb,
    CONSTRAINT scene_stream_cleanup_attempts_check CHECK ((attempts >= 0)),
    CONSTRAINT scene_stream_cleanup_notification_kind_check CHECK ((notification_kind = ANY (ARRAY['streaming_key_reset'::text, 'streaming_key_revoke'::text, 'streaming_key_expired'::text, 'streaming_time_exceeded'::text])))
);


--
-- Name: subscription_opt_outs; Type: TABLE; Schema: social; Owner: -
--

CREATE TABLE social.subscription_opt_outs (
    address text NOT NULL,
    scope text NOT NULL,
    scope_id text NOT NULL,
    created_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL
);


--
-- Name: subscriptions; Type: TABLE; Schema: social; Owner: -
--

CREATE TABLE social.subscriptions (
    address text NOT NULL,
    email text,
    details jsonb DEFAULT '{}'::jsonb NOT NULL,
    is_credits_workflow boolean DEFAULT false NOT NULL,
    updated_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL
);


--
-- Name: translation_cache; Type: TABLE; Schema: social; Owner: -
--

CREATE TABLE social.translation_cache (
    backend text NOT NULL,
    target_lang text NOT NULL,
    text_sha256 bytea NOT NULL,
    translated_text text NOT NULL,
    detected_language text DEFAULT 'en'::text NOT NULL,
    detected_confidence real DEFAULT 1.0 NOT NULL,
    created_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    CONSTRAINT translation_cache_hash_len_check CHECK ((octet_length(text_sha256) = 32))
);


--
-- Name: unconfirmed_emails; Type: TABLE; Schema: social; Owner: -
--

CREATE TABLE social.unconfirmed_emails (
    address text NOT NULL,
    email text NOT NULL,
    code text NOT NULL,
    source text DEFAULT 'account'::text NOT NULL,
    created_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL
);


--
-- Name: user_achieved_tiers; Type: TABLE; Schema: social; Owner: -
--

CREATE TABLE social.user_achieved_tiers (
    address text NOT NULL,
    badge_id text NOT NULL,
    tier_id text NOT NULL,
    completed_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    granted_by text,
    granted_at timestamp without time zone
);


--
-- Name: user_badge_progress; Type: TABLE; Schema: social; Owner: -
--

CREATE TABLE social.user_badge_progress (
    address text NOT NULL,
    badge_id text NOT NULL,
    steps_done integer DEFAULT 0 NOT NULL,
    completed_at timestamp without time zone,
    last_completed_tier_id text,
    granted_by text,
    updated_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    CONSTRAINT user_badge_progress_steps_check CHECK ((steps_done >= 0))
);


--
-- Name: user_bans; Type: TABLE; Schema: social; Owner: -
--

CREATE TABLE social.user_bans (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    banned_address text NOT NULL,
    banned_by text NOT NULL,
    reason text NOT NULL,
    custom_message text,
    banned_device_id text,
    banned_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    expires_at timestamp without time zone,
    lifted_at timestamp without time zone,
    lifted_by text,
    created_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    CONSTRAINT user_bans_lift_check CHECK ((((lifted_at IS NULL) AND (lifted_by IS NULL)) OR ((lifted_at IS NOT NULL) AND (lifted_by IS NOT NULL))))
);


--
-- Name: user_mutes; Type: TABLE; Schema: social; Owner: -
--

CREATE TABLE social.user_mutes (
    muter_address text NOT NULL,
    muted_address text NOT NULL,
    muted_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    CONSTRAINT user_mutes_not_self_check CHECK ((muter_address <> muted_address))
);


--
-- Name: user_settings; Type: TABLE; Schema: social; Owner: -
--

CREATE TABLE social.user_settings (
    address text NOT NULL,
    private_messages_privacy text DEFAULT 'only_friends'::text NOT NULL,
    blocked_users_messages_visibility text DEFAULT 'show_messages'::text NOT NULL,
    show_situation_reactions text DEFAULT 'show'::text NOT NULL,
    updated_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    CONSTRAINT user_settings_blocked_visibility_check CHECK ((blocked_users_messages_visibility = ANY (ARRAY['show_messages'::text, 'do_not_show_messages'::text]))),
    CONSTRAINT user_settings_private_messages_privacy_check CHECK ((private_messages_privacy = ANY (ARRAY['only_friends'::text, 'all'::text]))),
    CONSTRAINT user_settings_situation_reactions_check CHECK ((show_situation_reactions = ANY (ARRAY['show'::text, 'hide'::text])))
);


--
-- Name: user_warnings; Type: TABLE; Schema: social; Owner: -
--

CREATE TABLE social.user_warnings (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    warned_address text NOT NULL,
    warned_by text NOT NULL,
    reason text NOT NULL,
    warned_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL
);


--
-- Name: worn_history; Type: TABLE; Schema: social; Owner: -
--

CREATE TABLE social.worn_history (
    address text NOT NULL,
    urn text NOT NULL,
    first_seen timestamp(3) without time zone NOT NULL
);


--
-- Name: events; Type: TABLE; Schema: telemetry; Owner: -
--

CREATE TABLE telemetry.events (
    id bigint NOT NULL,
    received_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    source text NOT NULL,
    project text DEFAULT ''::text NOT NULL,
    event_kind text DEFAULT ''::text NOT NULL,
    body jsonb NOT NULL,
    invalid_reason text,
    fingerprint text GENERATED ALWAYS AS ("left"(regexp_replace(regexp_replace(regexp_replace(regexp_replace(split_part(COALESCE(NULLIF((body ->> 'message'::text), ''::text), NULLIF((body #>> '{logentry,message}'::text[]), ''::text), (NULLIF((body #>> '{exception,values,0,type}'::text[]), ''::text) || COALESCE((': '::text || (body #>> '{exception,values,0,value}'::text[])), ''::text)), NULLIF((body ->> 'transaction'::text), ''::text), NULLIF((body ->> 'event'::text), ''::text),
CASE
    WHEN (event_kind = 'session'::text) THEN (('session ('::text || COALESCE(NULLIF((body ->> 'status'::text), ''::text),
    CASE
        WHEN ((body ->> 'init'::text))::boolean THEN 'started'::text
        ELSE 'update'::text
    END)) || ')'::text)
    ELSE NULL::text
END,
CASE
    WHEN ((body ->> 'userId'::text) IS NOT NULL) THEN ('identify '::text || (body ->> 'userId'::text))
    ELSE NULL::text
END, (('('::text || event_kind) || ')'::text)), '
'::text, 1), '<[^>]*>'::text, ''::text, 'g'::text), '^[0-9]{2}:[0-9]{2}:[0-9]{2}\.[0-9]+ - '::text, ''::text), 'https?://[^ ]+'::text, 'URL'::text, 'g'::text), '-?[0-9]+'::text, 'N'::text, 'g'::text), 512)) STORED,
    CONSTRAINT events_source_check CHECK ((source = ANY (ARRAY['sentry'::text, 'segment'::text])))
);


--
-- Name: events_id_seq; Type: SEQUENCE; Schema: telemetry; Owner: -
--

ALTER TABLE telemetry.events ALTER COLUMN id ADD GENERATED ALWAYS AS IDENTITY (
    SEQUENCE NAME telemetry.events_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1
);


--
-- Name: experiment_group_targets; Type: TABLE; Schema: telemetry; Owner: -
--

CREATE TABLE telemetry.experiment_group_targets (
    exp_key text NOT NULL,
    group_name text NOT NULL,
    killed boolean DEFAULT false NOT NULL,
    forced_variant text,
    flags jsonb DEFAULT '{}'::jsonb NOT NULL,
    updated_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL
);


--
-- Name: experiment_overrides; Type: TABLE; Schema: telemetry; Owner: -
--

CREATE TABLE telemetry.experiment_overrides (
    exp_key text NOT NULL,
    killed boolean DEFAULT false NOT NULL,
    forced_variant text,
    flags jsonb DEFAULT '{}'::jsonb NOT NULL,
    updated_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL
);


--
-- Name: flag_group_targets; Type: TABLE; Schema: telemetry; Owner: -
--

CREATE TABLE telemetry.flag_group_targets (
    flag text NOT NULL,
    group_name text NOT NULL,
    state text DEFAULT 'on'::text NOT NULL,
    forced_variant text,
    updated_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    CONSTRAINT flag_group_targets_state_check CHECK ((state = ANY (ARRAY['on'::text, 'off'::text, 'forced'::text])))
);


--
-- Name: flag_groups; Type: TABLE; Schema: telemetry; Owner: -
--

CREATE TABLE telemetry.flag_groups (
    name text NOT NULL,
    description text DEFAULT ''::text NOT NULL,
    members text[] DEFAULT '{}'::text[] NOT NULL,
    rollout_pct integer DEFAULT 0 NOT NULL,
    priority integer DEFAULT 0 NOT NULL,
    updated_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    CONSTRAINT flag_groups_rollout_check CHECK (((rollout_pct >= 0) AND (rollout_pct <= 100)))
);


--
-- Name: flag_overrides; Type: TABLE; Schema: telemetry; Owner: -
--

CREATE TABLE telemetry.flag_overrides (
    flag text NOT NULL,
    state text DEFAULT 'on'::text NOT NULL,
    forced_variant text,
    updated_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    CONSTRAINT flag_overrides_state_check CHECK ((state = ANY (ARRAY['on'::text, 'off'::text, 'forced'::text])))
);


--
-- Name: ingest_settings; Type: TABLE; Schema: telemetry; Owner: -
--

CREATE TABLE telemetry.ingest_settings (
    id integer NOT NULL,
    enabled boolean DEFAULT true NOT NULL,
    updated_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    CONSTRAINT ingest_settings_singleton_check CHECK ((id = 1))
);


--
-- Name: issue_merges; Type: TABLE; Schema: telemetry; Owner: -
--

CREATE TABLE telemetry.issue_merges (
    source_fingerprint text NOT NULL,
    canonical_fingerprint text NOT NULL,
    merged_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    CONSTRAINT issue_merges_not_self_check CHECK ((source_fingerprint <> canonical_fingerprint))
);


--
-- Name: issue_state; Type: TABLE; Schema: telemetry; Owner: -
--

CREATE TABLE telemetry.issue_state (
    fingerprint text NOT NULL,
    status text DEFAULT 'unresolved'::text NOT NULL,
    assignee text,
    note text,
    updated_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    CONSTRAINT issue_state_status_check CHECK ((status = ANY (ARRAY['unresolved'::text, 'resolved'::text, 'ignored'::text])))
);


--
-- Name: product_areas; Type: TABLE; Schema: telemetry; Owner: -
--

CREATE TABLE telemetry.product_areas (
    kind text NOT NULL,
    name text NOT NULL,
    area text NOT NULL,
    updated_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    CONSTRAINT product_areas_kind_check CHECK ((kind = ANY (ARRAY['flag'::text, 'experiment'::text])))
);


--
-- Name: project_quotas; Type: TABLE; Schema: telemetry; Owner: -
--

CREATE TABLE telemetry.project_quotas (
    project text NOT NULL,
    daily_limit bigint NOT NULL,
    updated_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    CONSTRAINT project_quotas_limit_check CHECK ((daily_limit >= 0))
);


--
-- Name: release_state; Type: TABLE; Schema: telemetry; Owner: -
--

CREATE TABLE telemetry.release_state (
    release text NOT NULL,
    state text DEFAULT 'active'::text NOT NULL,
    note text,
    updated_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    CONSTRAINT release_state_state_check CHECK ((state = ANY (ARRAY['active'::text, 'archived'::text, 'broken'::text])))
);


--
-- Name: blocked_wallets; Type: TABLE; Schema: worlds; Owner: -
--

CREATE TABLE worlds.blocked_wallets (
    wallet text NOT NULL,
    created_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    updated_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    CONSTRAINT blocked_wallets_lowercase_address CHECK ((wallet ~ '^0x[0-9a-f]{40}$'::text))
);


--
-- Name: env_variables; Type: TABLE; Schema: worlds; Owner: -
--

CREATE TABLE worlds.env_variables (
    realm text NOT NULL,
    scene_namespace text NOT NULL,
    key text NOT NULL,
    value_enc bytea NOT NULL,
    value_size integer NOT NULL,
    created_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    updated_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    CONSTRAINT env_variables_key_len CHECK (((octet_length(key) >= 1) AND (octet_length(key) <= 1020))),
    CONSTRAINT env_variables_realm_lowercase CHECK (((realm = lower(realm)) AND (realm <> ''::text))),
    CONSTRAINT env_variables_size_nonneg CHECK ((value_size >= 0))
);


--
-- Name: federation_peer_status; Type: TABLE; Schema: worlds; Owner: -
--

CREATE TABLE worlds.federation_peer_status (
    peer_id text NOT NULL,
    last_attempt_at timestamp without time zone,
    last_success_at timestamp without time zone,
    last_error text,
    worlds_observed bigint DEFAULT 0 NOT NULL,
    entries_skipped bigint DEFAULT 0 NOT NULL,
    truncated boolean DEFAULT false NOT NULL,
    deadmitted_at timestamp without time zone,
    deadmitted_worlds_deleted bigint DEFAULT 0 NOT NULL,
    CONSTRAINT federation_peer_status_counts_nonneg CHECK (((worlds_observed >= 0) AND (entries_skipped >= 0) AND (deadmitted_worlds_deleted >= 0))),
    CONSTRAINT federation_peer_status_id_lowercase CHECK (((peer_id = lower(peer_id)) AND (peer_id <> ''::text)))
);


--
-- Name: federation_worlds; Type: TABLE; Schema: worlds; Owner: -
--

CREATE TABLE worlds.federation_worlds (
    peer_id text NOT NULL,
    world_name text NOT NULL,
    title text,
    description text,
    content_rating text,
    categories text[],
    thumbnail_hash text,
    deployed_scenes bigint DEFAULT 0 NOT NULL,
    last_deployed_at timestamp without time zone,
    observed_at timestamp without time zone NOT NULL,
    hidden_since timestamp without time zone,
    CONSTRAINT federation_worlds_deployed_scenes_nonneg CHECK ((deployed_scenes >= 0)),
    CONSTRAINT federation_worlds_name_lowercase CHECK (((world_name = lower(world_name)) AND (world_name <> ''::text))),
    CONSTRAINT federation_worlds_peer_id_lowercase CHECK (((peer_id = lower(peer_id)) AND (peer_id <> ''::text)))
);


--
-- Name: forward_outbox; Type: TABLE; Schema: worlds; Owner: -
--

CREATE TABLE worlds.forward_outbox (
    id bigint NOT NULL,
    entity_id text NOT NULL,
    world_name text NOT NULL,
    target text NOT NULL,
    status text DEFAULT 'queued'::text NOT NULL,
    attempts integer DEFAULT 0 NOT NULL,
    next_attempt_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    last_error text,
    upstream_status smallint,
    created_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    updated_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    CONSTRAINT forward_outbox_attempts_check CHECK ((attempts >= 0)),
    CONSTRAINT forward_outbox_status_check CHECK ((status = ANY (ARRAY['queued'::text, 'sending'::text, 'accepted'::text, 'rejected'::text, 'failed'::text, 'superseded'::text]))),
    CONSTRAINT forward_outbox_upstream_status_check CHECK (((upstream_status >= 100) AND (upstream_status <= 599))),
    CONSTRAINT forward_outbox_world_name_lowercase CHECK (((world_name = lower(world_name)) AND (world_name <> ''::text)))
);


--
-- Name: forward_outbox_id_seq; Type: SEQUENCE; Schema: worlds; Owner: -
--

ALTER TABLE worlds.forward_outbox ALTER COLUMN id ADD GENERATED ALWAYS AS IDENTITY (
    SEQUENCE NAME worlds.forward_outbox_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1
);


--
-- Name: mirror_passes; Type: TABLE; Schema: worlds; Owner: -
--

CREATE TABLE worlds.mirror_passes (
    mirror text NOT NULL,
    source text NOT NULL,
    pass_number bigint DEFAULT 0 NOT NULL,
    pass_started_at timestamp without time zone,
    pass_completed_at timestamp without time zone,
    last_attempt_at timestamp without time zone,
    cursor text,
    stats jsonb DEFAULT '{}'::jsonb NOT NULL,
    last_error text,
    CONSTRAINT mirror_passes_known_mirror CHECK ((mirror = ANY (ARRAY['catalogue'::text, 'scenes'::text])))
);


--
-- Name: player_storage; Type: TABLE; Schema: worlds; Owner: -
--

CREATE TABLE worlds.player_storage (
    realm text NOT NULL,
    scene_namespace text NOT NULL,
    player_address text NOT NULL,
    key text NOT NULL,
    value jsonb NOT NULL,
    value_size integer NOT NULL,
    created_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    updated_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    CONSTRAINT player_storage_key_len CHECK (((octet_length(key) >= 1) AND (octet_length(key) <= 1020))),
    CONSTRAINT player_storage_player_lowercase_address CHECK ((player_address ~ '^0x[0-9a-f]{40}$'::text)),
    CONSTRAINT player_storage_realm_lowercase CHECK (((realm = lower(realm)) AND (realm <> ''::text))),
    CONSTRAINT player_storage_size_nonneg CHECK ((value_size >= 0))
);


--
-- Name: remote_worlds; Type: TABLE; Schema: worlds; Owner: -
--

CREATE TABLE worlds.remote_worlds (
    world_name text NOT NULL,
    source text NOT NULL,
    scene_ids text[] NOT NULL,
    newest_timestamp_ms bigint,
    first_seen_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    listed_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    unlisted_at timestamp without time zone,
    mirrored_at timestamp without time zone,
    last_error text,
    CONSTRAINT remote_worlds_name_lowercase CHECK ((world_name = lower(world_name)))
);


--
-- Name: world_access_communities; Type: TABLE; Schema: worlds; Owner: -
--

CREATE TABLE worlds.world_access_communities (
    world_name text NOT NULL,
    community_id uuid NOT NULL,
    created_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    CONSTRAINT world_access_communities_world_lowercase CHECK ((world_name = lower(world_name)))
);


--
-- Name: world_access_log; Type: TABLE; Schema: worlds; Owner: -
--

CREATE TABLE worlds.world_access_log (
    id bigint NOT NULL,
    world_name text NOT NULL,
    address text NOT NULL,
    action text NOT NULL,
    room text NOT NULL,
    created_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    CONSTRAINT world_access_log_action_check CHECK ((action = ANY (ARRAY['join'::text, 'leave'::text]))),
    CONSTRAINT world_access_log_address_lowercase CHECK ((address = lower(address))),
    CONSTRAINT world_access_log_world_lowercase CHECK ((world_name = lower(world_name)))
);


--
-- Name: world_access_log_id_seq; Type: SEQUENCE; Schema: worlds; Owner: -
--

ALTER TABLE worlds.world_access_log ALTER COLUMN id ADD GENERATED ALWAYS AS IDENTITY (
    SEQUENCE NAME worlds.world_access_log_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1
);


--
-- Name: world_permission_parcels; Type: TABLE; Schema: worlds; Owner: -
--

CREATE TABLE worlds.world_permission_parcels (
    world_name text NOT NULL,
    permission text NOT NULL,
    address text NOT NULL,
    parcel text NOT NULL,
    created_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    CONSTRAINT world_permission_parcels_canonical CHECK ((parcel ~ '^-?(0|[1-9][0-9]*),-?(0|[1-9][0-9]*)$'::text))
);


--
-- Name: world_permissions; Type: TABLE; Schema: worlds; Owner: -
--

CREATE TABLE worlds.world_permissions (
    world_name text NOT NULL,
    permission text NOT NULL,
    address text NOT NULL,
    created_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    updated_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    CONSTRAINT world_permissions_address_lowercase CHECK ((address ~ '^0x[0-9a-f]{40}$'::text)),
    CONSTRAINT world_permissions_permission_check CHECK ((permission = ANY (ARRAY['deployment'::text, 'streaming'::text, 'access'::text]))),
    CONSTRAINT world_permissions_world_lowercase CHECK ((world_name = lower(world_name)))
);


--
-- Name: world_scene_identity; Type: TABLE; Schema: worlds; Owner: -
--

CREATE TABLE worlds.world_scene_identity (
    world_name text NOT NULL,
    identity uuid NOT NULL,
    parcels text[] NOT NULL,
    active boolean DEFAULT true NOT NULL,
    created_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    updated_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    CONSTRAINT world_scene_identity_parcels_nonempty CHECK ((cardinality(parcels) > 0)),
    CONSTRAINT world_scene_identity_world_lowercase CHECK (((world_name = lower(world_name)) AND (world_name <> ''::text)))
);


--
-- Name: world_scenes; Type: TABLE; Schema: worlds; Owner: -
--

CREATE TABLE worlds.world_scenes (
    world_name text NOT NULL,
    entity_id text NOT NULL,
    deployment_auth_chain json NOT NULL,
    entity jsonb NOT NULL,
    deployer text NOT NULL,
    parcels text[] NOT NULL,
    size bigint NOT NULL,
    mirror_source text,
    created_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    updated_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    CONSTRAINT world_scenes_size_nonneg CHECK ((size >= 0))
);


--
-- Name: world_storage; Type: TABLE; Schema: worlds; Owner: -
--

CREATE TABLE worlds.world_storage (
    realm text NOT NULL,
    scene_namespace text NOT NULL,
    key text NOT NULL,
    value jsonb NOT NULL,
    value_size integer NOT NULL,
    created_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    updated_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    CONSTRAINT world_storage_key_len CHECK (((octet_length(key) >= 1) AND (octet_length(key) <= 1020))),
    CONSTRAINT world_storage_realm_lowercase CHECK (((realm = lower(realm)) AND (realm <> ''::text))),
    CONSTRAINT world_storage_size_nonneg CHECK ((value_size >= 0))
);


--
-- Name: worlds; Type: TABLE; Schema: worlds; Owner: -
--

CREATE TABLE worlds.worlds (
    name text NOT NULL,
    owner text,
    access_type text DEFAULT 'unrestricted'::text NOT NULL,
    access_secret_hash text,
    access_nft text,
    blocked_since timestamp without time zone,
    spawn_coordinates text,
    title text,
    description text,
    content_rating text,
    skybox_time integer,
    categories text[],
    single_player boolean DEFAULT false NOT NULL,
    show_in_places boolean DEFAULT true NOT NULL,
    thumbnail_hash text,
    preview_wearable_urns text[],
    settings_version bigint DEFAULT 0 NOT NULL,
    realm_name_override text,
    mirror_source text,
    created_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    updated_at timestamp without time zone DEFAULT timezone('UTC'::text, now()) NOT NULL,
    CONSTRAINT worlds_access_nft_check CHECK (((access_type = 'nft-ownership'::text) = (access_nft IS NOT NULL))),
    CONSTRAINT worlds_access_secret_check CHECK (((access_type = 'shared-secret'::text) = (access_secret_hash IS NOT NULL))),
    CONSTRAINT worlds_access_type_check CHECK ((access_type = ANY (ARRAY['unrestricted'::text, 'shared-secret'::text, 'nft-ownership'::text, 'allow-list'::text]))),
    CONSTRAINT worlds_categories_check CHECK (((categories IS NULL) OR (cardinality(categories) <= 20))),
    CONSTRAINT worlds_content_rating_check CHECK (((content_rating IS NULL) OR (content_rating = ANY (ARRAY['RP'::text, 'E'::text, 'T'::text, 'A'::text, 'R'::text])))),
    CONSTRAINT worlds_name_lowercase CHECK (((name = lower(name)) AND (btrim(name) <> ''::text))),
    CONSTRAINT worlds_owner_lowercase_address CHECK (((owner IS NULL) OR (owner ~ '^0x[0-9a-f]{40}$'::text))),
    CONSTRAINT worlds_preview_wearables_check CHECK (((preview_wearable_urns IS NULL) OR (cardinality(preview_wearable_urns) <= 8))),
    CONSTRAINT worlds_settings_version_nonneg CHECK ((settings_version >= 0))
);


--
-- Name: deployments id; Type: DEFAULT; Schema: content; Owner: -
--

ALTER TABLE ONLY content.deployments ALTER COLUMN id SET DEFAULT nextval('content.deployments_id_seq'::regclass);


--
-- Name: asset_catalog asset_catalog_pkey; Type: CONSTRAINT; Schema: builder; Owner: -
--

ALTER TABLE ONLY builder.asset_catalog
    ADD CONSTRAINT asset_catalog_pkey PRIMARY KEY (id);


--
-- Name: asset_contents asset_contents_pkey; Type: CONSTRAINT; Schema: builder; Owner: -
--

ALTER TABLE ONLY builder.asset_contents
    ADD CONSTRAINT asset_contents_pkey PRIMARY KEY (asset_id, file);


--
-- Name: asset_packs asset_packs_pkey; Type: CONSTRAINT; Schema: builder; Owner: -
--

ALTER TABLE ONLY builder.asset_packs
    ADD CONSTRAINT asset_packs_pkey PRIMARY KEY (id);


--
-- Name: assets assets_pkey; Type: CONSTRAINT; Schema: builder; Owner: -
--

ALTER TABLE ONLY builder.assets
    ADD CONSTRAINT assets_pkey PRIMARY KEY (id);


--
-- Name: collection_publications collection_publications_contract_address_key; Type: CONSTRAINT; Schema: builder; Owner: -
--

ALTER TABLE ONLY builder.collection_publications
    ADD CONSTRAINT collection_publications_contract_address_key UNIQUE (contract_address);


--
-- Name: collection_publications collection_publications_pkey; Type: CONSTRAINT; Schema: builder; Owner: -
--

ALTER TABLE ONLY builder.collection_publications
    ADD CONSTRAINT collection_publications_pkey PRIMARY KEY (collection_id);


--
-- Name: collection_publications collection_publications_tx_hash_key; Type: CONSTRAINT; Schema: builder; Owner: -
--

ALTER TABLE ONLY builder.collection_publications
    ADD CONSTRAINT collection_publications_tx_hash_key UNIQUE (tx_hash);


--
-- Name: collections collections_contract_address_key; Type: CONSTRAINT; Schema: builder; Owner: -
--

ALTER TABLE ONLY builder.collections
    ADD CONSTRAINT collections_contract_address_key UNIQUE (contract_address);


--
-- Name: collections collections_pkey; Type: CONSTRAINT; Schema: builder; Owner: -
--

ALTER TABLE ONLY builder.collections
    ADD CONSTRAINT collections_pkey PRIMARY KEY (id);


--
-- Name: item_contents item_contents_pkey; Type: CONSTRAINT; Schema: builder; Owner: -
--

ALTER TABLE ONLY builder.item_contents
    ADD CONSTRAINT item_contents_pkey PRIMARY KEY (item_id, file);


--
-- Name: items items_pkey; Type: CONSTRAINT; Schema: builder; Owner: -
--

ALTER TABLE ONLY builder.items
    ADD CONSTRAINT items_pkey PRIMARY KEY (id);


--
-- Name: linked_collection_publications linked_collection_publications_pkey; Type: CONSTRAINT; Schema: builder; Owner: -
--

ALTER TABLE ONLY builder.linked_collection_publications
    ADD CONSTRAINT linked_collection_publications_pkey PRIMARY KEY (collection_id);


--
-- Name: linked_collection_publications linked_collection_publications_salt_key; Type: CONSTRAINT; Schema: builder; Owner: -
--

ALTER TABLE ONLY builder.linked_collection_publications
    ADD CONSTRAINT linked_collection_publications_salt_key UNIQUE (salt);


--
-- Name: newsletter_subscriptions newsletter_subscriptions_pkey; Type: CONSTRAINT; Schema: builder; Owner: -
--

ALTER TABLE ONLY builder.newsletter_subscriptions
    ADD CONSTRAINT newsletter_subscriptions_pkey PRIMARY KEY (email);


--
-- Name: project_files project_files_pkey; Type: CONSTRAINT; Schema: builder; Owner: -
--

ALTER TABLE ONLY builder.project_files
    ADD CONSTRAINT project_files_pkey PRIMARY KEY (project_id, path);


--
-- Name: project_manifests project_manifests_pkey; Type: CONSTRAINT; Schema: builder; Owner: -
--

ALTER TABLE ONLY builder.project_manifests
    ADD CONSTRAINT project_manifests_pkey PRIMARY KEY (project_id);


--
-- Name: projects projects_pkey; Type: CONSTRAINT; Schema: builder; Owner: -
--

ALTER TABLE ONLY builder.projects
    ADD CONSTRAINT projects_pkey PRIMARY KEY (id);


--
-- Name: uploaded_contents uploaded_contents_pkey; Type: CONSTRAINT; Schema: builder; Owner: -
--

ALTER TABLE ONLY builder.uploaded_contents
    ADD CONSTRAINT uploaded_contents_pkey PRIMARY KEY (hash);


--
-- Name: images images_pkey; Type: CONSTRAINT; Schema: camera_reel; Owner: -
--

ALTER TABLE ONLY camera_reel.images
    ADD CONSTRAINT images_pkey PRIMARY KEY (id);


--
-- Name: withheld_blobs withheld_blobs_pkey; Type: CONSTRAINT; Schema: camera_reel; Owner: -
--

ALTER TABLE ONLY camera_reel.withheld_blobs
    ADD CONSTRAINT withheld_blobs_pkey PRIMARY KEY (hash);


--
-- Name: actions actions_pkey; Type: CONSTRAINT; Schema: chat; Owner: -
--

ALTER TABLE ONLY chat.actions
    ADD CONSTRAINT actions_pkey PRIMARY KEY (id);


--
-- Name: channels channels_pkey; Type: CONSTRAINT; Schema: chat; Owner: -
--

ALTER TABLE ONLY chat.channels
    ADD CONSTRAINT channels_pkey PRIMARY KEY (community_id, name);


--
-- Name: community_activity community_activity_pkey; Type: CONSTRAINT; Schema: chat; Owner: -
--

ALTER TABLE ONLY chat.community_activity
    ADD CONSTRAINT community_activity_pkey PRIMARY KEY (seq);


--
-- Name: community_live community_live_pkey; Type: CONSTRAINT; Schema: chat; Owner: -
--

ALTER TABLE ONLY chat.community_live
    ADD CONSTRAINT community_live_pkey PRIMARY KEY (community_id);


--
-- Name: messages messages_pkey; Type: CONSTRAINT; Schema: chat; Owner: -
--

ALTER TABLE ONLY chat.messages
    ADD CONSTRAINT messages_pkey PRIMARY KEY (seq);


--
-- Name: pins pins_pkey; Type: CONSTRAINT; Schema: chat; Owner: -
--

ALTER TABLE ONLY chat.pins
    ADD CONSTRAINT pins_pkey PRIMARY KEY (message_id);


--
-- Name: reactions reactions_pkey; Type: CONSTRAINT; Schema: chat; Owner: -
--

ALTER TABLE ONLY chat.reactions
    ADD CONSTRAINT reactions_pkey PRIMARY KEY (message_id, wallet, emoji);


--
-- Name: replies replies_pkey; Type: CONSTRAINT; Schema: chat; Owner: -
--

ALTER TABLE ONLY chat.replies
    ADD CONSTRAINT replies_pkey PRIMARY KEY (seq);


--
-- Name: reports reports_pkey; Type: CONSTRAINT; Schema: chat; Owner: -
--

ALTER TABLE ONLY chat.reports
    ADD CONSTRAINT reports_pkey PRIMARY KEY (message_id, wallet);


--
-- Name: active_pointers active_pointers_pkey; Type: CONSTRAINT; Schema: content; Owner: -
--

ALTER TABLE ONLY content.active_pointers
    ADD CONSTRAINT active_pointers_pkey PRIMARY KEY (pointer);


--
-- Name: content_files content_files_uniq_deployment_key; Type: CONSTRAINT; Schema: content; Owner: -
--

ALTER TABLE ONLY content.content_files
    ADD CONSTRAINT content_files_uniq_deployment_key UNIQUE (deployment, key);


--
-- Name: denylist denylist_pkey; Type: CONSTRAINT; Schema: content; Owner: -
--

ALTER TABLE ONLY content.denylist
    ADD CONSTRAINT denylist_pkey PRIMARY KEY (id);


--
-- Name: deployments deployments_entity_id_key; Type: CONSTRAINT; Schema: content; Owner: -
--

ALTER TABLE ONLY content.deployments
    ADD CONSTRAINT deployments_entity_id_key UNIQUE (entity_id);


--
-- Name: deployments deployments_pkey; Type: CONSTRAINT; Schema: content; Owner: -
--

ALTER TABLE ONLY content.deployments
    ADD CONSTRAINT deployments_pkey PRIMARY KEY (id);


--
-- Name: failed_deployments failed_deployments_pkey; Type: CONSTRAINT; Schema: content; Owner: -
--

ALTER TABLE ONLY content.failed_deployments
    ADD CONSTRAINT failed_deployments_pkey PRIMARY KEY (entity_id);


--
-- Name: forward_outbox forward_outbox_entity_id_key; Type: CONSTRAINT; Schema: content; Owner: -
--

ALTER TABLE ONLY content.forward_outbox
    ADD CONSTRAINT forward_outbox_entity_id_key UNIQUE (entity_id);


--
-- Name: forward_outbox forward_outbox_pkey; Type: CONSTRAINT; Schema: content; Owner: -
--

ALTER TABLE ONLY content.forward_outbox
    ADD CONSTRAINT forward_outbox_pkey PRIMARY KEY (id);


--
-- Name: local_entities local_entities_pkey; Type: CONSTRAINT; Schema: content; Owner: -
--

ALTER TABLE ONLY content.local_entities
    ADD CONSTRAINT local_entities_pkey PRIMARY KEY (entity_id);


--
-- Name: processed_snapshots processed_snapshots_pkey; Type: CONSTRAINT; Schema: content; Owner: -
--

ALTER TABLE ONLY content.processed_snapshots
    ADD CONSTRAINT processed_snapshots_pkey PRIMARY KEY (hash);


--
-- Name: server_sync_cursors server_sync_cursors_pkey; Type: CONSTRAINT; Schema: content; Owner: -
--

ALTER TABLE ONLY content.server_sync_cursors
    ADD CONSTRAINT server_sync_cursors_pkey PRIMARY KEY (server_url);


--
-- Name: snapshots snapshots_pkey; Type: CONSTRAINT; Schema: content; Owner: -
--

ALTER TABLE ONLY content.snapshots
    ADD CONSTRAINT snapshots_pkey PRIMARY KEY (init_timestamp, end_timestamp);


--
-- Name: system_properties system_properties_pkey; Type: CONSTRAINT; Schema: content; Owner: -
--

ALTER TABLE ONLY content.system_properties
    ADD CONSTRAINT system_properties_pkey PRIMARY KEY (key);


--
-- Name: authorization_lines authorization_lines_pkey; Type: CONSTRAINT; Schema: credits; Owner: -
--

ALTER TABLE ONLY credits.authorization_lines
    ADD CONSTRAINT authorization_lines_pkey PRIMARY KEY (authorization_id, "position");


--
-- Name: authorizations authorizations_pkey; Type: CONSTRAINT; Schema: credits; Owner: -
--

ALTER TABLE ONLY credits.authorizations
    ADD CONSTRAINT authorizations_pkey PRIMARY KEY (id);


--
-- Name: captcha_challenges captcha_challenges_pkey; Type: CONSTRAINT; Schema: credits; Owner: -
--

ALTER TABLE ONLY credits.captcha_challenges
    ADD CONSTRAINT captcha_challenges_pkey PRIMARY KEY (id);


--
-- Name: cart_items cart_items_line_key; Type: CONSTRAINT; Schema: credits; Owner: -
--

ALTER TABLE ONLY credits.cart_items
    ADD CONSTRAINT cart_items_line_key UNIQUE (cart_id, collection, item_id);


--
-- Name: cart_items cart_items_pkey; Type: CONSTRAINT; Schema: credits; Owner: -
--

ALTER TABLE ONLY credits.cart_items
    ADD CONSTRAINT cart_items_pkey PRIMARY KEY (id);


--
-- Name: carts carts_address_key; Type: CONSTRAINT; Schema: credits; Owner: -
--

ALTER TABLE ONLY credits.carts
    ADD CONSTRAINT carts_address_key UNIQUE (address);


--
-- Name: carts carts_pkey; Type: CONSTRAINT; Schema: credits; Owner: -
--

ALTER TABLE ONLY credits.carts
    ADD CONSTRAINT carts_pkey PRIMARY KEY (id);


--
-- Name: checkouts checkouts_idempotency_key_key; Type: CONSTRAINT; Schema: credits; Owner: -
--

ALTER TABLE ONLY credits.checkouts
    ADD CONSTRAINT checkouts_idempotency_key_key UNIQUE (idempotency_key);


--
-- Name: checkouts checkouts_pkey; Type: CONSTRAINT; Schema: credits; Owner: -
--

ALTER TABLE ONLY credits.checkouts
    ADD CONSTRAINT checkouts_pkey PRIMARY KEY (id);


--
-- Name: fulfillment_outbox fulfillment_outbox_pkey; Type: CONSTRAINT; Schema: credits; Owner: -
--

ALTER TABLE ONLY credits.fulfillment_outbox
    ADD CONSTRAINT fulfillment_outbox_pkey PRIMARY KEY (id);


--
-- Name: idempotency idempotency_pkey; Type: CONSTRAINT; Schema: credits; Owner: -
--

ALTER TABLE ONLY credits.idempotency
    ADD CONSTRAINT idempotency_pkey PRIMARY KEY (scope, idempotency_key);


--
-- Name: ledger ledger_pkey; Type: CONSTRAINT; Schema: credits; Owner: -
--

ALTER TABLE ONLY credits.ledger
    ADD CONSTRAINT ledger_pkey PRIMARY KEY (id);


--
-- Name: packs packs_pkey; Type: CONSTRAINT; Schema: credits; Owner: -
--

ALTER TABLE ONLY credits.packs
    ADD CONSTRAINT packs_pkey PRIMARY KEY (sku);


--
-- Name: purchases purchases_pkey; Type: CONSTRAINT; Schema: credits; Owner: -
--

ALTER TABLE ONLY credits.purchases
    ADD CONSTRAINT purchases_pkey PRIMARY KEY (id);


--
-- Name: revocations revocations_pkey; Type: CONSTRAINT; Schema: credits; Owner: -
--

ALTER TABLE ONLY credits.revocations
    ADD CONSTRAINT revocations_pkey PRIMARY KEY (credit_hash);


--
-- Name: stripe_events stripe_events_pkey; Type: CONSTRAINT; Schema: credits; Owner: -
--

ALTER TABLE ONLY credits.stripe_events
    ADD CONSTRAINT stripe_events_pkey PRIMARY KEY (event_id);


--
-- Name: usage_index_cursor usage_index_cursor_pkey; Type: CONSTRAINT; Schema: credits; Owner: -
--

ALTER TABLE ONLY credits.usage_index_cursor
    ADD CONSTRAINT usage_index_cursor_pkey PRIMARY KEY (chain_id);


--
-- Name: usage_settlements usage_settlements_pkey; Type: CONSTRAINT; Schema: credits; Owner: -
--

ALTER TABLE ONLY credits.usage_settlements
    ADD CONSTRAINT usage_settlements_pkey PRIMARY KEY (chain_id, block_number, log_index);


--
-- Name: usages usages_pkey; Type: CONSTRAINT; Schema: credits; Owner: -
--

ALTER TABLE ONLY credits.usages
    ADD CONSTRAINT usages_pkey PRIMARY KEY (chain_id, block_number, log_index);


--
-- Name: wallets wallets_pkey; Type: CONSTRAINT; Schema: credits; Owner: -
--

ALTER TABLE ONLY credits.wallets
    ADD CONSTRAINT wallets_pkey PRIMARY KEY (address);


--
-- Name: price_override_audit price_override_audit_pkey; Type: CONSTRAINT; Schema: economy; Owner: -
--

ALTER TABLE ONLY economy.price_override_audit
    ADD CONSTRAINT price_override_audit_pkey PRIMARY KEY (id);


--
-- Name: price_overrides price_overrides_pkey; Type: CONSTRAINT; Schema: economy; Owner: -
--

ALTER TABLE ONLY economy.price_overrides
    ADD CONSTRAINT price_overrides_pkey PRIMARY KEY (token_id, vs_currency);


--
-- Name: price_snapshots price_snapshots_pkey; Type: CONSTRAINT; Schema: economy; Owner: -
--

ALTER TABLE ONLY economy.price_snapshots
    ADD CONSTRAINT price_snapshots_pkey PRIMARY KEY (id);


--
-- Name: transactions transactions_pkey; Type: CONSTRAINT; Schema: economy; Owner: -
--

ALTER TABLE ONLY economy.transactions
    ADD CONSTRAINT transactions_pkey PRIMARY KEY (id);


--
-- Name: transactions transactions_session_id_key; Type: CONSTRAINT; Schema: economy; Owner: -
--

ALTER TABLE ONLY economy.transactions
    ADD CONSTRAINT transactions_session_id_key UNIQUE (session_id);


--
-- Name: transactions transactions_tx_hash_key; Type: CONSTRAINT; Schema: economy; Owner: -
--

ALTER TABLE ONLY economy.transactions
    ADD CONSTRAINT transactions_tx_hash_key UNIQUE (tx_hash);


--
-- Name: event_attendance_local event_attendance_local_pkey; Type: CONSTRAINT; Schema: explore; Owner: -
--

ALTER TABLE ONLY explore.event_attendance_local
    ADD CONSTRAINT event_attendance_local_pkey PRIMARY KEY (event_id, signer);


--
-- Name: event_moderation event_moderation_pkey; Type: CONSTRAINT; Schema: explore; Owner: -
--

ALTER TABLE ONLY explore.event_moderation
    ADD CONSTRAINT event_moderation_pkey PRIMARY KEY (event_id);


--
-- Name: event event_pkey; Type: CONSTRAINT; Schema: explore; Owner: -
--

ALTER TABLE ONLY explore.event
    ADD CONSTRAINT event_pkey PRIMARY KEY (id);


--
-- Name: event_profile_settings event_profile_settings_pkey; Type: CONSTRAINT; Schema: explore; Owner: -
--

ALTER TABLE ONLY explore.event_profile_settings
    ADD CONSTRAINT event_profile_settings_pkey PRIMARY KEY ("user");


--
-- Name: events_local events_local_pkey; Type: CONSTRAINT; Schema: explore; Owner: -
--

ALTER TABLE ONLY explore.events_local
    ADD CONSTRAINT events_local_pkey PRIMARY KEY (id);


--
-- Name: mirror_state mirror_state_pkey; Type: CONSTRAINT; Schema: explore; Owner: -
--

ALTER TABLE ONLY explore.mirror_state
    ADD CONSTRAINT mirror_state_pkey PRIMARY KEY (mirror);


--
-- Name: moderators moderators_pkey; Type: CONSTRAINT; Schema: explore; Owner: -
--

ALTER TABLE ONLY explore.moderators
    ADD CONSTRAINT moderators_pkey PRIMARY KEY (address);


--
-- Name: opensea_nft opensea_nft_pkey; Type: CONSTRAINT; Schema: explore; Owner: -
--

ALTER TABLE ONLY explore.opensea_nft
    ADD CONSTRAINT opensea_nft_pkey PRIMARY KEY (chain, contract_address, token_id);


--
-- Name: place_interaction_counts place_interaction_counts_pkey; Type: CONSTRAINT; Schema: explore; Owner: -
--

ALTER TABLE ONLY explore.place_interaction_counts
    ADD CONSTRAINT place_interaction_counts_pkey PRIMARY KEY (entity_id);


--
-- Name: place_moderation place_moderation_pkey; Type: CONSTRAINT; Schema: explore; Owner: -
--

ALTER TABLE ONLY explore.place_moderation
    ADD CONSTRAINT place_moderation_pkey PRIMARY KEY (place_id);


--
-- Name: place place_pkey; Type: CONSTRAINT; Schema: explore; Owner: -
--

ALTER TABLE ONLY explore.place
    ADD CONSTRAINT place_pkey PRIMARY KEY (id);


--
-- Name: place_ranking place_ranking_pkey; Type: CONSTRAINT; Schema: explore; Owner: -
--

ALTER TABLE ONLY explore.place_ranking
    ADD CONSTRAINT place_ranking_pkey PRIMARY KEY (place_id);


--
-- Name: place_removal_pending place_removal_pending_pkey; Type: CONSTRAINT; Schema: explore; Owner: -
--

ALTER TABLE ONLY explore.place_removal_pending
    ADD CONSTRAINT place_removal_pending_pkey PRIMARY KEY (event_id);


--
-- Name: place_removal_state place_removal_state_pkey; Type: CONSTRAINT; Schema: explore; Owner: -
--

ALTER TABLE ONLY explore.place_removal_state
    ADD CONSTRAINT place_removal_state_pkey PRIMARY KEY (place_id);


--
-- Name: place_reports place_reports_filename_key; Type: CONSTRAINT; Schema: explore; Owner: -
--

ALTER TABLE ONLY explore.place_reports
    ADD CONSTRAINT place_reports_filename_key UNIQUE (filename);


--
-- Name: place_reports place_reports_pkey; Type: CONSTRAINT; Schema: explore; Owner: -
--

ALTER TABLE ONLY explore.place_reports
    ADD CONSTRAINT place_reports_pkey PRIMARY KEY (id);


--
-- Name: place_world_local place_world_local_pkey; Type: CONSTRAINT; Schema: explore; Owner: -
--

ALTER TABLE ONLY explore.place_world_local
    ADD CONSTRAINT place_world_local_pkey PRIMARY KEY (id);


--
-- Name: poster poster_pkey; Type: CONSTRAINT; Schema: explore; Owner: -
--

ALTER TABLE ONLY explore.poster
    ADD CONSTRAINT poster_pkey PRIMARY KEY (kind, file);


--
-- Name: road_positions road_positions_pkey; Type: CONSTRAINT; Schema: explore; Owner: -
--

ALTER TABLE ONLY explore.road_positions
    ADD CONSTRAINT road_positions_pkey PRIMARY KEY ("position");


--
-- Name: satellite_region satellite_region_pkey; Type: CONSTRAINT; Schema: explore; Owner: -
--

ALTER TABLE ONLY explore.satellite_region
    ADD CONSTRAINT satellite_region_pkey PRIMARY KEY (cx, cy, variant);


--
-- Name: schedules_local schedules_local_pkey; Type: CONSTRAINT; Schema: explore; Owner: -
--

ALTER TABLE ONLY explore.schedules_local
    ADD CONSTRAINT schedules_local_pkey PRIMARY KEY (id);


--
-- Name: seen_nonces seen_nonces_pkey; Type: CONSTRAINT; Schema: explore; Owner: -
--

ALTER TABLE ONLY explore.seen_nonces
    ADD CONSTRAINT seen_nonces_pkey PRIMARY KEY (signer, nonce);


--
-- Name: signed_actions signed_actions_pkey; Type: CONSTRAINT; Schema: explore; Owner: -
--

ALTER TABLE ONLY explore.signed_actions
    ADD CONSTRAINT signed_actions_pkey PRIMARY KEY (signature_hash);


--
-- Name: signed_actions signed_actions_seq_key; Type: CONSTRAINT; Schema: explore; Owner: -
--

ALTER TABLE ONLY explore.signed_actions
    ADD CONSTRAINT signed_actions_seq_key UNIQUE (seq);


--
-- Name: user_favorites user_favorites_pkey; Type: CONSTRAINT; Schema: explore; Owner: -
--

ALTER TABLE ONLY explore.user_favorites
    ADD CONSTRAINT user_favorites_pkey PRIMARY KEY ("user", entity_id);


--
-- Name: user_likes user_likes_pkey; Type: CONSTRAINT; Schema: explore; Owner: -
--

ALTER TABLE ONLY explore.user_likes
    ADD CONSTRAINT user_likes_pkey PRIMARY KEY ("user", entity_id);


--
-- Name: world_manifest world_manifest_pkey; Type: CONSTRAINT; Schema: explore; Owner: -
--

ALTER TABLE ONLY explore.world_manifest
    ADD CONSTRAINT world_manifest_pkey PRIMARY KEY (id);


--
-- Name: world_scene_place world_scene_place_pkey; Type: CONSTRAINT; Schema: explore; Owner: -
--

ALTER TABLE ONLY explore.world_scene_place
    ADD CONSTRAINT world_scene_place_pkey PRIMARY KEY (identity);


--
-- Name: denylist_users_local denylist_users_local_pkey; Type: CONSTRAINT; Schema: explore_api; Owner: -
--

ALTER TABLE ONLY explore_api.denylist_users_local
    ADD CONSTRAINT denylist_users_local_pkey PRIMARY KEY (wallet);


--
-- Name: denylist_users denylist_users_pkey; Type: CONSTRAINT; Schema: explore_api; Owner: -
--

ALTER TABLE ONLY explore_api.denylist_users
    ADD CONSTRAINT denylist_users_pkey PRIMARY KEY (wallet);


--
-- Name: feature_flags_local feature_flags_local_pkey; Type: CONSTRAINT; Schema: explore_api; Owner: -
--

ALTER TABLE ONLY explore_api.feature_flags_local
    ADD CONSTRAINT feature_flags_local_pkey PRIMARY KEY (app, name);


--
-- Name: feature_flags feature_flags_pkey; Type: CONSTRAINT; Schema: explore_api; Owner: -
--

ALTER TABLE ONLY explore_api.feature_flags
    ADD CONSTRAINT feature_flags_pkey PRIMARY KEY (app);


--
-- Name: lists_local lists_local_pkey; Type: CONSTRAINT; Schema: explore_api; Owner: -
--

ALTER TABLE ONLY explore_api.lists_local
    ADD CONSTRAINT lists_local_pkey PRIMARY KEY (list, value);


--
-- Name: lists lists_pkey; Type: CONSTRAINT; Schema: explore_api; Owner: -
--

ALTER TABLE ONLY explore_api.lists
    ADD CONSTRAINT lists_pkey PRIMARY KEY (list, value);


--
-- Name: map_grid map_grid_pkey; Type: CONSTRAINT; Schema: explore_api; Owner: -
--

ALTER TABLE ONLY explore_api.map_grid
    ADD CONSTRAINT map_grid_pkey PRIMARY KEY (id);


--
-- Name: mirror_passes mirror_passes_pkey; Type: CONSTRAINT; Schema: explore_api; Owner: -
--

ALTER TABLE ONLY explore_api.mirror_passes
    ADD CONSTRAINT mirror_passes_pkey PRIMARY KEY (mirror);


--
-- Name: budgets budgets_pkey; Type: CONSTRAINT; Schema: governance; Owner: -
--

ALTER TABLE ONLY governance.budgets
    ADD CONSTRAINT budgets_pkey PRIMARY KEY (id);


--
-- Name: discourse_posts discourse_posts_pkey; Type: CONSTRAINT; Schema: governance; Owner: -
--

ALTER TABLE ONLY governance.discourse_posts
    ADD CONSTRAINT discourse_posts_pkey PRIMARY KEY (id);


--
-- Name: discourse_topics discourse_topics_pkey; Type: CONSTRAINT; Schema: governance; Owner: -
--

ALTER TABLE ONLY governance.discourse_topics
    ADD CONSTRAINT discourse_topics_pkey PRIMARY KEY (topic_id);


--
-- Name: members members_pkey; Type: CONSTRAINT; Schema: governance; Owner: -
--

ALTER TABLE ONLY governance.members
    ADD CONSTRAINT members_pkey PRIMARY KEY (role, address);


--
-- Name: mirror_state mirror_state_pkey; Type: CONSTRAINT; Schema: governance; Owner: -
--

ALTER TABLE ONLY governance.mirror_state
    ADD CONSTRAINT mirror_state_pkey PRIMARY KEY (key);


--
-- Name: project_updates project_updates_pkey; Type: CONSTRAINT; Schema: governance; Owner: -
--

ALTER TABLE ONLY governance.project_updates
    ADD CONSTRAINT project_updates_pkey PRIMARY KEY (id);


--
-- Name: projects projects_pkey; Type: CONSTRAINT; Schema: governance; Owner: -
--

ALTER TABLE ONLY governance.projects
    ADD CONSTRAINT projects_pkey PRIMARY KEY (id);


--
-- Name: proposals proposals_pkey; Type: CONSTRAINT; Schema: governance; Owner: -
--

ALTER TABLE ONLY governance.proposals
    ADD CONSTRAINT proposals_pkey PRIMARY KEY (id);


--
-- Name: snapshot_proposals snapshot_proposals_pkey; Type: CONSTRAINT; Schema: governance; Owner: -
--

ALTER TABLE ONLY governance.snapshot_proposals
    ADD CONSTRAINT snapshot_proposals_pkey PRIMARY KEY (id);


--
-- Name: snapshot_votes snapshot_votes_pkey; Type: CONSTRAINT; Schema: governance; Owner: -
--

ALTER TABLE ONLY governance.snapshot_votes
    ADD CONSTRAINT snapshot_votes_pkey PRIMARY KEY (proposal_id, voter);


--
-- Name: vestings vestings_pkey; Type: CONSTRAINT; Schema: governance; Owner: -
--

ALTER TABLE ONLY governance.vestings
    ADD CONSTRAINT vestings_pkey PRIMARY KEY (address);


--
-- Name: account_right account_right_pkey; Type: CONSTRAINT; Schema: land_authz; Owner: -
--

ALTER TABLE ONLY land_authz.account_right
    ADD CONSTRAINT account_right_pkey PRIMARY KEY (token_address, account, operator, kind);


--
-- Name: authz_event authz_event_pkey; Type: CONSTRAINT; Schema: land_authz; Owner: -
--

ALTER TABLE ONLY land_authz.authz_event
    ADD CONSTRAINT authz_event_pkey PRIMARY KEY (block_number, log_index);


--
-- Name: estate_land estate_land_pkey; Type: CONSTRAINT; Schema: land_authz; Owner: -
--

ALTER TABLE ONLY land_authz.estate_land
    ADD CONSTRAINT estate_land_pkey PRIMARY KEY (land_token_id);


--
-- Name: index_cursor index_cursor_pkey; Type: CONSTRAINT; Schema: land_authz; Owner: -
--

ALTER TABLE ONLY land_authz.index_cursor
    ADD CONSTRAINT index_cursor_pkey PRIMARY KEY (network);


--
-- Name: token_right token_right_pkey; Type: CONSTRAINT; Schema: land_authz; Owner: -
--

ALTER TABLE ONLY land_authz.token_right
    ADD CONSTRAINT token_right_pkey PRIMARY KEY (token_address, token_id);


--
-- Name: schema schema_pkey; Type: CONSTRAINT; Schema: meta; Owner: -
--

ALTER TABLE ONLY meta.schema
    ADD CONSTRAINT schema_pkey PRIMARY KEY (id);


--
-- Name: schema_steps schema_steps_pkey; Type: CONSTRAINT; Schema: meta; Owner: -
--

ALTER TABLE ONLY meta.schema_steps
    ADD CONSTRAINT schema_steps_pkey PRIMARY KEY (step);


--
-- Name: admin_audit admin_audit_pkey; Type: CONSTRAINT; Schema: ops; Owner: -
--

ALTER TABLE ONLY ops.admin_audit
    ADD CONSTRAINT admin_audit_pkey PRIMARY KEY (id);


--
-- Name: daily_stats daily_stats_pkey; Type: CONSTRAINT; Schema: presence; Owner: -
--

ALTER TABLE ONLY presence.daily_stats
    ADD CONSTRAINT daily_stats_pkey PRIMARY KEY (date);


--
-- Name: hot_scene_snapshots hot_scene_snapshots_pkey; Type: CONSTRAINT; Schema: presence; Owner: -
--

ALTER TABLE ONLY presence.hot_scene_snapshots
    ADD CONSTRAINT hot_scene_snapshots_pkey PRIMARY KEY (snapshot_id, scene_id);


--
-- Name: island_snapshots island_snapshots_pkey; Type: CONSTRAINT; Schema: presence; Owner: -
--

ALTER TABLE ONLY presence.island_snapshots
    ADD CONSTRAINT island_snapshots_pkey PRIMARY KEY (snapshot_id, island_id);


--
-- Name: peer_snapshots peer_snapshots_pkey; Type: CONSTRAINT; Schema: presence; Owner: -
--

ALTER TABLE ONLY presence.peer_snapshots
    ADD CONSTRAINT peer_snapshots_pkey PRIMARY KEY (snapshot_id, address);


--
-- Name: scene_occupancy scene_occupancy_pkey; Type: CONSTRAINT; Schema: presence; Owner: -
--

ALTER TABLE ONLY presence.scene_occupancy
    ADD CONSTRAINT scene_occupancy_pkey PRIMARY KEY (snapshot_id, pointer, realm);


--
-- Name: snapshots snapshots_pkey; Type: CONSTRAINT; Schema: presence; Owner: -
--

ALTER TABLE ONLY presence.snapshots
    ADD CONSTRAINT snapshots_pkey PRIMARY KEY (id);


--
-- Name: world_membership world_membership_pkey; Type: CONSTRAINT; Schema: presence; Owner: -
--

ALTER TABLE ONLY presence.world_membership
    ADD CONSTRAINT world_membership_pkey PRIMARY KEY (snapshot_id, world_name);


--
-- Name: quest_events quest_events_pkey; Type: CONSTRAINT; Schema: quests; Owner: -
--

ALTER TABLE ONLY quests.quest_events
    ADD CONSTRAINT quest_events_pkey PRIMARY KEY (id);


--
-- Name: quest_instances quest_instances_pkey; Type: CONSTRAINT; Schema: quests; Owner: -
--

ALTER TABLE ONLY quests.quest_instances
    ADD CONSTRAINT quest_instances_pkey PRIMARY KEY (id);


--
-- Name: quest_reward_items quest_reward_items_pkey; Type: CONSTRAINT; Schema: quests; Owner: -
--

ALTER TABLE ONLY quests.quest_reward_items
    ADD CONSTRAINT quest_reward_items_pkey PRIMARY KEY (quest_id, "position");


--
-- Name: quests quests_pkey; Type: CONSTRAINT; Schema: quests; Owner: -
--

ALTER TABLE ONLY quests.quests
    ADD CONSTRAINT quests_pkey PRIMARY KEY (id);


--
-- Name: admin_audit admin_audit_pkey; Type: CONSTRAINT; Schema: shop; Owner: -
--

ALTER TABLE ONLY shop.admin_audit
    ADD CONSTRAINT admin_audit_pkey PRIMARY KEY (id);


--
-- Name: broker_purchases broker_purchases_pkey; Type: CONSTRAINT; Schema: shop; Owner: -
--

ALTER TABLE ONLY shop.broker_purchases
    ADD CONSTRAINT broker_purchases_pkey PRIMARY KEY (id);


--
-- Name: campaign_collections campaign_collections_pkey; Type: CONSTRAINT; Schema: shop; Owner: -
--

ALTER TABLE ONLY shop.campaign_collections
    ADD CONSTRAINT campaign_collections_pkey PRIMARY KEY (tag, network, contract_address);


--
-- Name: campaigns campaigns_pkey; Type: CONSTRAINT; Schema: shop; Owner: -
--

ALTER TABLE ONLY shop.campaigns
    ADD CONSTRAINT campaigns_pkey PRIMARY KEY (id);


--
-- Name: chain_cursor chain_cursor_pkey; Type: CONSTRAINT; Schema: shop; Owner: -
--

ALTER TABLE ONLY shop.chain_cursor
    ADD CONSTRAINT chain_cursor_pkey PRIMARY KEY (indexer, network);


--
-- Name: collections collections_pkey; Type: CONSTRAINT; Schema: shop; Owner: -
--

ALTER TABLE ONLY shop.collections
    ADD CONSTRAINT collections_pkey PRIMARY KEY (network, contract_address);


--
-- Name: coupon_state coupon_state_pkey; Type: CONSTRAINT; Schema: shop; Owner: -
--

ALTER TABLE ONLY shop.coupon_state
    ADD CONSTRAINT coupon_state_pkey PRIMARY KEY (coupon_id);


--
-- Name: coupons coupons_hashed_signature_key; Type: CONSTRAINT; Schema: shop; Owner: -
--

ALTER TABLE ONLY shop.coupons
    ADD CONSTRAINT coupons_hashed_signature_key UNIQUE (hashed_signature);


--
-- Name: coupons coupons_pkey; Type: CONSTRAINT; Schema: shop; Owner: -
--

ALTER TABLE ONLY shop.coupons
    ADD CONSTRAINT coupons_pkey PRIMARY KEY (id);


--
-- Name: coupons coupons_signature_key; Type: CONSTRAINT; Schema: shop; Owner: -
--

ALTER TABLE ONLY shop.coupons
    ADD CONSTRAINT coupons_signature_key UNIQUE (signature);


--
-- Name: coupons coupons_state_key_key; Type: CONSTRAINT; Schema: shop; Owner: -
--

ALTER TABLE ONLY shop.coupons
    ADD CONSTRAINT coupons_state_key_key UNIQUE (state_key);


--
-- Name: disputes disputes_pkey; Type: CONSTRAINT; Schema: shop; Owner: -
--

ALTER TABLE ONLY shop.disputes
    ADD CONSTRAINT disputes_pkey PRIMARY KEY (trade_hash);


--
-- Name: escrow_actions escrow_actions_pkey; Type: CONSTRAINT; Schema: shop; Owner: -
--

ALTER TABLE ONLY shop.escrow_actions
    ADD CONSTRAINT escrow_actions_pkey PRIMARY KEY (id);


--
-- Name: executions executions_chain_id_tx_hash_key; Type: CONSTRAINT; Schema: shop; Owner: -
--

ALTER TABLE ONLY shop.executions
    ADD CONSTRAINT executions_chain_id_tx_hash_key UNIQUE (chain_id, tx_hash);


--
-- Name: executions executions_pkey; Type: CONSTRAINT; Schema: shop; Owner: -
--

ALTER TABLE ONLY shop.executions
    ADD CONSTRAINT executions_pkey PRIMARY KEY (id);


--
-- Name: fed_actions fed_actions_pkey; Type: CONSTRAINT; Schema: shop; Owner: -
--

ALTER TABLE ONLY shop.fed_actions
    ADD CONSTRAINT fed_actions_pkey PRIMARY KEY (signature_hash);


--
-- Name: fed_actions fed_actions_seq_key; Type: CONSTRAINT; Schema: shop; Owner: -
--

ALTER TABLE ONLY shop.fed_actions
    ADD CONSTRAINT fed_actions_seq_key UNIQUE (seq);


--
-- Name: fed_seen_nonces fed_seen_nonces_pkey; Type: CONSTRAINT; Schema: shop; Owner: -
--

ALTER TABLE ONLY shop.fed_seen_nonces
    ADD CONSTRAINT fed_seen_nonces_pkey PRIMARY KEY (signer, nonce);


--
-- Name: item_availability_watch item_availability_watch_pkey; Type: CONSTRAINT; Schema: shop; Owner: -
--

ALTER TABLE ONLY shop.item_availability_watch
    ADD CONSTRAINT item_availability_watch_pkey PRIMARY KEY (contract_address, item_id);


--
-- Name: item_neighbors_meta item_neighbors_meta_pkey; Type: CONSTRAINT; Schema: shop; Owner: -
--

ALTER TABLE ONLY shop.item_neighbors_meta
    ADD CONSTRAINT item_neighbors_meta_pkey PRIMARY KEY (id);


--
-- Name: item_neighbors item_neighbors_pkey; Type: CONSTRAINT; Schema: shop; Owner: -
--

ALTER TABLE ONLY shop.item_neighbors
    ADD CONSTRAINT item_neighbors_pkey PRIMARY KEY (network, contract_address, item_id, neighbor_network, neighbor_contract_address, neighbor_item_id);


--
-- Name: item_notify_outbox item_notify_outbox_pkey; Type: CONSTRAINT; Schema: shop; Owner: -
--

ALTER TABLE ONLY shop.item_notify_outbox
    ADD CONSTRAINT item_notify_outbox_pkey PRIMARY KEY (id);


--
-- Name: item_notify_requests item_notify_requests_addr_item_uniq; Type: CONSTRAINT; Schema: shop; Owner: -
--

ALTER TABLE ONLY shop.item_notify_requests
    ADD CONSTRAINT item_notify_requests_addr_item_uniq UNIQUE (address, contract_address, item_id);


--
-- Name: item_notify_requests item_notify_requests_pkey; Type: CONSTRAINT; Schema: shop; Owner: -
--

ALTER TABLE ONLY shop.item_notify_requests
    ADD CONSTRAINT item_notify_requests_pkey PRIMARY KEY (id);


--
-- Name: items items_pkey; Type: CONSTRAINT; Schema: shop; Owner: -
--

ALTER TABLE ONLY shop.items
    ADD CONSTRAINT items_pkey PRIMARY KEY (network, contract_address, item_id);


--
-- Name: items items_urn_key; Type: CONSTRAINT; Schema: shop; Owner: -
--

ALTER TABLE ONLY shop.items
    ADD CONSTRAINT items_urn_key UNIQUE (urn);


--
-- Name: land_data land_data_pkey; Type: CONSTRAINT; Schema: shop; Owner: -
--

ALTER TABLE ONLY shop.land_data
    ADD CONSTRAINT land_data_pkey PRIMARY KEY (token_address, token_id);


--
-- Name: land_proximity land_proximity_pkey; Type: CONSTRAINT; Schema: shop; Owner: -
--

ALTER TABLE ONLY shop.land_proximity
    ADD CONSTRAINT land_proximity_pkey PRIMARY KEY (x, y);


--
-- Name: list_acl list_acl_pkey; Type: CONSTRAINT; Schema: shop; Owner: -
--

ALTER TABLE ONLY shop.list_acl
    ADD CONSTRAINT list_acl_pkey PRIMARY KEY (list_id, permission, grantee);


--
-- Name: lists lists_name_user_unique; Type: CONSTRAINT; Schema: shop; Owner: -
--

ALTER TABLE ONLY shop.lists
    ADD CONSTRAINT lists_name_user_unique UNIQUE (name, user_address);


--
-- Name: lists lists_pkey; Type: CONSTRAINT; Schema: shop; Owner: -
--

ALTER TABLE ONLY shop.lists
    ADD CONSTRAINT lists_pkey PRIMARY KEY (id);


--
-- Name: mana_transfer mana_transfer_pkey; Type: CONSTRAINT; Schema: shop; Owner: -
--

ALTER TABLE ONLY shop.mana_transfer
    ADD CONSTRAINT mana_transfer_pkey PRIMARY KEY (network, block_number, log_index);


--
-- Name: mirror_state mirror_state_pkey; Type: CONSTRAINT; Schema: shop; Owner: -
--

ALTER TABLE ONLY shop.mirror_state
    ADD CONSTRAINT mirror_state_pkey PRIMARY KEY (mirror, network);


--
-- Name: moderation_flags moderation_flags_pkey; Type: CONSTRAINT; Schema: shop; Owner: -
--

ALTER TABLE ONLY shop.moderation_flags
    ADD CONSTRAINT moderation_flags_pkey PRIMARY KEY (target_hash);


--
-- Name: name_transfers name_transfers_idempotency_key_key; Type: CONSTRAINT; Schema: shop; Owner: -
--

ALTER TABLE ONLY shop.name_transfers
    ADD CONSTRAINT name_transfers_idempotency_key_key UNIQUE (idempotency_key);


--
-- Name: name_transfers name_transfers_pkey; Type: CONSTRAINT; Schema: shop; Owner: -
--

ALTER TABLE ONLY shop.name_transfers
    ADD CONSTRAINT name_transfers_pkey PRIMARY KEY (id);


--
-- Name: nft_contract nft_contract_pkey; Type: CONSTRAINT; Schema: shop; Owner: -
--

ALTER TABLE ONLY shop.nft_contract
    ADD CONSTRAINT nft_contract_pkey PRIMARY KEY (network, contract_address);


--
-- Name: nft_event nft_event_pkey; Type: CONSTRAINT; Schema: shop; Owner: -
--

ALTER TABLE ONLY shop.nft_event
    ADD CONSTRAINT nft_event_pkey PRIMARY KEY (network, block_number, log_index);


--
-- Name: nft_order nft_order_pkey; Type: CONSTRAINT; Schema: shop; Owner: -
--

ALTER TABLE ONLY shop.nft_order
    ADD CONSTRAINT nft_order_pkey PRIMARY KEY (network, contract_address, token_id);


--
-- Name: nft nft_pkey; Type: CONSTRAINT; Schema: shop; Owner: -
--

ALTER TABLE ONLY shop.nft
    ADD CONSTRAINT nft_pkey PRIMARY KEY (network, contract_address, token_id);


--
-- Name: outfit_thumbnails outfit_thumbnails_pkey; Type: CONSTRAINT; Schema: shop; Owner: -
--

ALTER TABLE ONLY shop.outfit_thumbnails
    ADD CONSTRAINT outfit_thumbnails_pkey PRIMARY KEY (hash);


--
-- Name: outfits outfits_pkey; Type: CONSTRAINT; Schema: shop; Owner: -
--

ALTER TABLE ONLY shop.outfits
    ADD CONSTRAINT outfits_pkey PRIMARY KEY (id);


--
-- Name: picks picks_pkey; Type: CONSTRAINT; Schema: shop; Owner: -
--

ALTER TABLE ONLY shop.picks
    ADD CONSTRAINT picks_pkey PRIMARY KEY (item_id, user_address, list_id);


--
-- Name: rental_event rental_event_pkey; Type: CONSTRAINT; Schema: shop; Owner: -
--

ALTER TABLE ONLY shop.rental_event
    ADD CONSTRAINT rental_event_pkey PRIMARY KEY (network, block_number, log_index);


--
-- Name: trade_assets_erc20 trade_assets_erc20_asset_id_key; Type: CONSTRAINT; Schema: shop; Owner: -
--

ALTER TABLE ONLY shop.trade_assets_erc20
    ADD CONSTRAINT trade_assets_erc20_asset_id_key UNIQUE (asset_id);


--
-- Name: trade_assets_erc721 trade_assets_erc721_asset_id_key; Type: CONSTRAINT; Schema: shop; Owner: -
--

ALTER TABLE ONLY shop.trade_assets_erc721
    ADD CONSTRAINT trade_assets_erc721_asset_id_key UNIQUE (asset_id);


--
-- Name: trade_assets_item trade_assets_item_asset_id_key; Type: CONSTRAINT; Schema: shop; Owner: -
--

ALTER TABLE ONLY shop.trade_assets_item
    ADD CONSTRAINT trade_assets_item_asset_id_key UNIQUE (asset_id);


--
-- Name: trade_assets trade_assets_pkey; Type: CONSTRAINT; Schema: shop; Owner: -
--

ALTER TABLE ONLY shop.trade_assets
    ADD CONSTRAINT trade_assets_pkey PRIMARY KEY (id);


--
-- Name: trade_cancellations trade_cancellations_pkey; Type: CONSTRAINT; Schema: shop; Owner: -
--

ALTER TABLE ONLY shop.trade_cancellations
    ADD CONSTRAINT trade_cancellations_pkey PRIMARY KEY (target_hashed_signature);


--
-- Name: trade_chain_state trade_chain_state_pkey; Type: CONSTRAINT; Schema: shop; Owner: -
--

ALTER TABLE ONLY shop.trade_chain_state
    ADD CONSTRAINT trade_chain_state_pkey PRIMARY KEY (trade_id);


--
-- Name: trade_event trade_event_pkey; Type: CONSTRAINT; Schema: shop; Owner: -
--

ALTER TABLE ONLY shop.trade_event
    ADD CONSTRAINT trade_event_pkey PRIMARY KEY (network, block_number, log_index);


--
-- Name: trades trades_hashed_signature_key; Type: CONSTRAINT; Schema: shop; Owner: -
--

ALTER TABLE ONLY shop.trades
    ADD CONSTRAINT trades_hashed_signature_key UNIQUE (hashed_signature);


--
-- Name: trades_mirror trades_mirror_pkey; Type: CONSTRAINT; Schema: shop; Owner: -
--

ALTER TABLE ONLY shop.trades_mirror
    ADD CONSTRAINT trades_mirror_pkey PRIMARY KEY (id);


--
-- Name: trades trades_pkey; Type: CONSTRAINT; Schema: shop; Owner: -
--

ALTER TABLE ONLY shop.trades
    ADD CONSTRAINT trades_pkey PRIMARY KEY (id);


--
-- Name: trades trades_signature_key; Type: CONSTRAINT; Schema: shop; Owner: -
--

ALTER TABLE ONLY shop.trades
    ADD CONSTRAINT trades_signature_key UNIQUE (signature);


--
-- Name: treasury_drift_checks treasury_drift_checks_pkey; Type: CONSTRAINT; Schema: shop; Owner: -
--

ALTER TABLE ONLY shop.treasury_drift_checks
    ADD CONSTRAINT treasury_drift_checks_pkey PRIMARY KEY (id);


--
-- Name: treasury_ledger treasury_ledger_pkey; Type: CONSTRAINT; Schema: shop; Owner: -
--

ALTER TABLE ONLY shop.treasury_ledger
    ADD CONSTRAINT treasury_ledger_pkey PRIMARY KEY (id);


--
-- Name: treasury_pending_refill treasury_pending_refill_idempotency_key_key; Type: CONSTRAINT; Schema: shop; Owner: -
--

ALTER TABLE ONLY shop.treasury_pending_refill
    ADD CONSTRAINT treasury_pending_refill_idempotency_key_key UNIQUE (idempotency_key);


--
-- Name: treasury_pending_refill treasury_pending_refill_pkey; Type: CONSTRAINT; Schema: shop; Owner: -
--

ALTER TABLE ONLY shop.treasury_pending_refill
    ADD CONSTRAINT treasury_pending_refill_pkey PRIMARY KEY (id);


--
-- Name: usage_grants usage_grants_pkey; Type: CONSTRAINT; Schema: shop; Owner: -
--

ALTER TABLE ONLY shop.usage_grants
    ADD CONSTRAINT usage_grants_pkey PRIMARY KEY (id);


--
-- Name: rental_assets rental_assets_nft_id_key; Type: CONSTRAINT; Schema: signatures; Owner: -
--

ALTER TABLE ONLY signatures.rental_assets
    ADD CONSTRAINT rental_assets_nft_id_key UNIQUE (nft_id);


--
-- Name: rental_assets rental_assets_pkey; Type: CONSTRAINT; Schema: signatures; Owner: -
--

ALTER TABLE ONLY signatures.rental_assets
    ADD CONSTRAINT rental_assets_pkey PRIMARY KEY (contract_address, token_id);


--
-- Name: rental_listings rental_listings_pkey; Type: CONSTRAINT; Schema: signatures; Owner: -
--

ALTER TABLE ONLY signatures.rental_listings
    ADD CONSTRAINT rental_listings_pkey PRIMARY KEY (id);


--
-- Name: rental_periods rental_periods_pkey; Type: CONSTRAINT; Schema: signatures; Owner: -
--

ALTER TABLE ONLY signatures.rental_periods
    ADD CONSTRAINT rental_periods_pkey PRIMARY KEY (id);


--
-- Name: rentals rentals_pkey; Type: CONSTRAINT; Schema: signatures; Owner: -
--

ALTER TABLE ONLY signatures.rentals
    ADD CONSTRAINT rentals_pkey PRIMARY KEY (id);


--
-- Name: rentals rentals_signature_key; Type: CONSTRAINT; Schema: signatures; Owner: -
--

ALTER TABLE ONLY signatures.rentals
    ADD CONSTRAINT rentals_signature_key UNIQUE (signature);


--
-- Name: archipelago_v4_assignments archipelago_v4_assignments_pkey; Type: CONSTRAINT; Schema: social; Owner: -
--

ALTER TABLE ONLY social.archipelago_v4_assignments
    ADD CONSTRAINT archipelago_v4_assignments_pkey PRIMARY KEY (deployment_audience, owner_address, lane_key);


--
-- Name: archipelago_v4_meta archipelago_v4_meta_pkey; Type: CONSTRAINT; Schema: social; Owner: -
--

ALTER TABLE ONLY social.archipelago_v4_meta
    ADD CONSTRAINT archipelago_v4_meta_pkey PRIMARY KEY (key);


--
-- Name: badge_admin_audit badge_admin_audit_pkey; Type: CONSTRAINT; Schema: social; Owner: -
--

ALTER TABLE ONLY social.badge_admin_audit
    ADD CONSTRAINT badge_admin_audit_pkey PRIMARY KEY (id);


--
-- Name: badge_definitions badge_definitions_pkey; Type: CONSTRAINT; Schema: social; Owner: -
--

ALTER TABLE ONLY social.badge_definitions
    ADD CONSTRAINT badge_definitions_pkey PRIMARY KEY (id);


--
-- Name: badge_tiers badge_tiers_pkey; Type: CONSTRAINT; Schema: social; Owner: -
--

ALTER TABLE ONLY social.badge_tiers
    ADD CONSTRAINT badge_tiers_pkey PRIMARY KEY (badge_id, tier_id);


--
-- Name: blocks blocks_pkey; Type: CONSTRAINT; Schema: social; Owner: -
--

ALTER TABLE ONLY social.blocks
    ADD CONSTRAINT blocks_pkey PRIMARY KEY (blocker_address, blocked_address);


--
-- Name: communities communities_pkey; Type: CONSTRAINT; Schema: social; Owner: -
--

ALTER TABLE ONLY social.communities
    ADD CONSTRAINT communities_pkey PRIMARY KEY (id);


--
-- Name: community_bans community_bans_pkey; Type: CONSTRAINT; Schema: social; Owner: -
--

ALTER TABLE ONLY social.community_bans
    ADD CONSTRAINT community_bans_pkey PRIMARY KEY (community_id, banned_address);


--
-- Name: community_compliance_cache community_compliance_cache_pkey; Type: CONSTRAINT; Schema: social; Owner: -
--

ALTER TABLE ONLY social.community_compliance_cache
    ADD CONSTRAINT community_compliance_cache_pkey PRIMARY KEY (model, input_hash);


--
-- Name: community_contents community_contents_cid_key; Type: CONSTRAINT; Schema: social; Owner: -
--

ALTER TABLE ONLY social.community_contents
    ADD CONSTRAINT community_contents_cid_key UNIQUE (cid);


--
-- Name: community_contents community_contents_pkey; Type: CONSTRAINT; Schema: social; Owner: -
--

ALTER TABLE ONLY social.community_contents
    ADD CONSTRAINT community_contents_pkey PRIMARY KEY (content_hash);


--
-- Name: community_members community_members_pkey; Type: CONSTRAINT; Schema: social; Owner: -
--

ALTER TABLE ONLY social.community_members
    ADD CONSTRAINT community_members_pkey PRIMARY KEY (community_id, member_address);


--
-- Name: community_mirror community_mirror_pkey; Type: CONSTRAINT; Schema: social; Owner: -
--

ALTER TABLE ONLY social.community_mirror
    ADD CONSTRAINT community_mirror_pkey PRIMARY KEY (community_id);


--
-- Name: community_mirror_state community_mirror_state_pkey; Type: CONSTRAINT; Schema: social; Owner: -
--

ALTER TABLE ONLY social.community_mirror_state
    ADD CONSTRAINT community_mirror_state_pkey PRIMARY KEY (source_url);


--
-- Name: community_places community_places_pkey; Type: CONSTRAINT; Schema: social; Owner: -
--

ALTER TABLE ONLY social.community_places
    ADD CONSTRAINT community_places_pkey PRIMARY KEY (place_id, community_id);


--
-- Name: community_post_likes community_post_likes_pkey; Type: CONSTRAINT; Schema: social; Owner: -
--

ALTER TABLE ONLY social.community_post_likes
    ADD CONSTRAINT community_post_likes_pkey PRIMARY KEY (post_id, user_address);


--
-- Name: community_posts community_posts_pkey; Type: CONSTRAINT; Schema: social; Owner: -
--

ALTER TABLE ONLY social.community_posts
    ADD CONSTRAINT community_posts_pkey PRIMARY KEY (id);


--
-- Name: community_ranking_events community_ranking_events_pkey; Type: CONSTRAINT; Schema: social; Owner: -
--

ALTER TABLE ONLY social.community_ranking_events
    ADD CONSTRAINT community_ranking_events_pkey PRIMARY KEY (kind, event_key, community_id);


--
-- Name: community_requests community_requests_pkey; Type: CONSTRAINT; Schema: social; Owner: -
--

ALTER TABLE ONLY social.community_requests
    ADD CONSTRAINT community_requests_pkey PRIMARY KEY (id);


--
-- Name: community_seen_nonces community_seen_nonces_pkey; Type: CONSTRAINT; Schema: social; Owner: -
--

ALTER TABLE ONLY social.community_seen_nonces
    ADD CONSTRAINT community_seen_nonces_pkey PRIMARY KEY (signer, nonce);


--
-- Name: community_signed_actions community_signed_actions_pkey; Type: CONSTRAINT; Schema: social; Owner: -
--

ALTER TABLE ONLY social.community_signed_actions
    ADD CONSTRAINT community_signed_actions_pkey PRIMARY KEY (signature_hash);


--
-- Name: community_signed_actions community_signed_actions_seq_key; Type: CONSTRAINT; Schema: social; Owner: -
--

ALTER TABLE ONLY social.community_signed_actions
    ADD CONSTRAINT community_signed_actions_seq_key UNIQUE (seq);


--
-- Name: community_thumbnails community_thumbnails_pkey; Type: CONSTRAINT; Schema: social; Owner: -
--

ALTER TABLE ONLY social.community_thumbnails
    ADD CONSTRAINT community_thumbnails_pkey PRIMARY KEY (community_id);


--
-- Name: community_voice_chat_users community_voice_chat_users_pkey; Type: CONSTRAINT; Schema: social; Owner: -
--

ALTER TABLE ONLY social.community_voice_chat_users
    ADD CONSTRAINT community_voice_chat_users_pkey PRIMARY KEY (community_id, address);


--
-- Name: event_notification_intents event_notification_intents_pkey; Type: CONSTRAINT; Schema: social; Owner: -
--

ALTER TABLE ONLY social.event_notification_intents
    ADD CONSTRAINT event_notification_intents_pkey PRIMARY KEY (id);


--
-- Name: event_payloads event_payloads_pkey; Type: CONSTRAINT; Schema: social; Owner: -
--

ALTER TABLE ONLY social.event_payloads
    ADD CONSTRAINT event_payloads_pkey PRIMARY KEY (id);


--
-- Name: fed_instance_identity fed_instance_identity_pkey; Type: CONSTRAINT; Schema: social; Owner: -
--

ALTER TABLE ONLY social.fed_instance_identity
    ADD CONSTRAINT fed_instance_identity_pkey PRIMARY KEY (only_row);


--
-- Name: first_wear_baseline first_wear_baseline_pkey; Type: CONSTRAINT; Schema: social; Owner: -
--

ALTER TABLE ONLY social.first_wear_baseline
    ADD CONSTRAINT first_wear_baseline_pkey PRIMARY KEY (address);


--
-- Name: first_wear_cursor first_wear_cursor_pkey; Type: CONSTRAINT; Schema: social; Owner: -
--

ALTER TABLE ONLY social.first_wear_cursor
    ADD CONSTRAINT first_wear_cursor_pkey PRIMARY KEY (id);


--
-- Name: friend_messages friend_messages_pkey; Type: CONSTRAINT; Schema: social; Owner: -
--

ALTER TABLE ONLY social.friend_messages
    ADD CONSTRAINT friend_messages_pkey PRIMARY KEY (id);


--
-- Name: friendship_actions friendship_actions_pkey; Type: CONSTRAINT; Schema: social; Owner: -
--

ALTER TABLE ONLY social.friendship_actions
    ADD CONSTRAINT friendship_actions_pkey PRIMARY KEY (id);


--
-- Name: friendship_mutation_limits friendship_mutation_limits_pkey; Type: CONSTRAINT; Schema: social; Owner: -
--

ALTER TABLE ONLY social.friendship_mutation_limits
    ADD CONSTRAINT friendship_mutation_limits_pkey PRIMARY KEY (key);


--
-- Name: friendships friendships_pkey; Type: CONSTRAINT; Schema: social; Owner: -
--

ALTER TABLE ONLY social.friendships
    ADD CONSTRAINT friendships_pkey PRIMARY KEY (id);


--
-- Name: island_admission_fences island_admission_fences_pkey; Type: CONSTRAINT; Schema: social; Owner: -
--

ALTER TABLE ONLY social.island_admission_fences
    ADD CONSTRAINT island_admission_fences_pkey PRIMARY KEY (wallet);


--
-- Name: island_cleanup_dispatches island_cleanup_dispatches_pkey; Type: CONSTRAINT; Schema: social; Owner: -
--

ALTER TABLE ONLY social.island_cleanup_dispatches
    ADD CONSTRAINT island_cleanup_dispatches_pkey PRIMARY KEY (dispatch_id);


--
-- Name: island_cleanup_dispatches island_cleanup_dispatches_wallet_dispatch_id_key; Type: CONSTRAINT; Schema: social; Owner: -
--

ALTER TABLE ONLY social.island_cleanup_dispatches
    ADD CONSTRAINT island_cleanup_dispatches_wallet_dispatch_id_key UNIQUE (wallet, dispatch_id);


--
-- Name: island_takeover_receipts island_takeover_receipts_pkey; Type: CONSTRAINT; Schema: social; Owner: -
--

ALTER TABLE ONLY social.island_takeover_receipts
    ADD CONSTRAINT island_takeover_receipts_pkey PRIMARY KEY (wallet, source_incarnation, takeover_id);


--
-- Name: land_transfer_cursor land_transfer_cursor_pkey; Type: CONSTRAINT; Schema: social; Owner: -
--

ALTER TABLE ONLY social.land_transfer_cursor
    ADD CONSTRAINT land_transfer_cursor_pkey PRIMARY KEY (id);


--
-- Name: livekit_removal_intents livekit_removal_intents_pkey; Type: CONSTRAINT; Schema: social; Owner: -
--

ALTER TABLE ONLY social.livekit_removal_intents
    ADD CONSTRAINT livekit_removal_intents_pkey PRIMARY KEY (root_key, target_key);


--
-- Name: mls_commits mls_commits_pkey; Type: CONSTRAINT; Schema: social; Owner: -
--

ALTER TABLE ONLY social.mls_commits
    ADD CONSTRAINT mls_commits_pkey PRIMARY KEY (group_id, epoch);


--
-- Name: mls_group_members mls_group_members_pkey; Type: CONSTRAINT; Schema: social; Owner: -
--

ALTER TABLE ONLY social.mls_group_members
    ADD CONSTRAINT mls_group_members_pkey PRIMARY KEY (group_id, member);


--
-- Name: mls_groups mls_groups_pkey; Type: CONSTRAINT; Schema: social; Owner: -
--

ALTER TABLE ONLY social.mls_groups
    ADD CONSTRAINT mls_groups_pkey PRIMARY KEY (group_id);


--
-- Name: mls_key_packages mls_key_packages_pkey; Type: CONSTRAINT; Schema: social; Owner: -
--

ALTER TABLE ONLY social.mls_key_packages
    ADD CONSTRAINT mls_key_packages_pkey PRIMARY KEY (id);


--
-- Name: mls_key_packages mls_key_packages_ref_key; Type: CONSTRAINT; Schema: social; Owner: -
--

ALTER TABLE ONLY social.mls_key_packages
    ADD CONSTRAINT mls_key_packages_ref_key UNIQUE (ref_hash);


--
-- Name: mls_message_refs mls_message_refs_pkey; Type: CONSTRAINT; Schema: social; Owner: -
--

ALTER TABLE ONLY social.mls_message_refs
    ADD CONSTRAINT mls_message_refs_pkey PRIMARY KEY (signature_hash);


--
-- Name: notification_deliveries notification_deliveries_pkey; Type: CONSTRAINT; Schema: social; Owner: -
--

ALTER TABLE ONLY social.notification_deliveries
    ADD CONSTRAINT notification_deliveries_pkey PRIMARY KEY (notification_id, channel);


--
-- Name: notification_reader_seen notification_reader_seen_pkey; Type: CONSTRAINT; Schema: social; Owner: -
--

ALTER TABLE ONLY social.notification_reader_seen
    ADD CONSTRAINT notification_reader_seen_pkey PRIMARY KEY (address);


--
-- Name: notifications notifications_pkey; Type: CONSTRAINT; Schema: social; Owner: -
--

ALTER TABLE ONLY social.notifications
    ADD CONSTRAINT notifications_pkey PRIMARY KEY (id);


--
-- Name: player_connections player_connections_pkey; Type: CONSTRAINT; Schema: social; Owner: -
--

ALTER TABLE ONLY social.player_connections
    ADD CONSTRAINT player_connections_pkey PRIMARY KEY (address);


--
-- Name: player_report_evidence player_report_evidence_pkey; Type: CONSTRAINT; Schema: social; Owner: -
--

ALTER TABLE ONLY social.player_report_evidence
    ADD CONSTRAINT player_report_evidence_pkey PRIMARY KEY (report_id, evidence_key);


--
-- Name: player_reports player_reports_pkey; Type: CONSTRAINT; Schema: social; Owner: -
--

ALTER TABLE ONLY social.player_reports
    ADD CONSTRAINT player_reports_pkey PRIMARY KEY (id);


--
-- Name: private_voice_chat_participants private_voice_chat_participants_pkey; Type: CONSTRAINT; Schema: social; Owner: -
--

ALTER TABLE ONLY social.private_voice_chat_participants
    ADD CONSTRAINT private_voice_chat_participants_pkey PRIMARY KEY (call_id, address);


--
-- Name: private_voice_chats private_voice_chats_callee_key; Type: CONSTRAINT; Schema: social; Owner: -
--

ALTER TABLE ONLY social.private_voice_chats
    ADD CONSTRAINT private_voice_chats_callee_key UNIQUE (callee_address);


--
-- Name: private_voice_chats private_voice_chats_caller_key; Type: CONSTRAINT; Schema: social; Owner: -
--

ALTER TABLE ONLY social.private_voice_chats
    ADD CONSTRAINT private_voice_chats_caller_key UNIQUE (caller_address);


--
-- Name: private_voice_chats private_voice_chats_pkey; Type: CONSTRAINT; Schema: social; Owner: -
--

ALTER TABLE ONLY social.private_voice_chats
    ADD CONSTRAINT private_voice_chats_pkey PRIMARY KEY (id);


--
-- Name: profile_images profile_images_pkey; Type: CONSTRAINT; Schema: social; Owner: -
--

ALTER TABLE ONLY social.profile_images
    ADD CONSTRAINT profile_images_pkey PRIMARY KEY (entity_id, kind);


--
-- Name: referral_emails referral_emails_pkey; Type: CONSTRAINT; Schema: social; Owner: -
--

ALTER TABLE ONLY social.referral_emails
    ADD CONSTRAINT referral_emails_pkey PRIMARY KEY (id);


--
-- Name: referral_progress referral_progress_id_key; Type: CONSTRAINT; Schema: social; Owner: -
--

ALTER TABLE ONLY social.referral_progress
    ADD CONSTRAINT referral_progress_id_key UNIQUE (id);


--
-- Name: referral_progress referral_progress_pkey; Type: CONSTRAINT; Schema: social; Owner: -
--

ALTER TABLE ONLY social.referral_progress
    ADD CONSTRAINT referral_progress_pkey PRIMARY KEY (invited_user);


--
-- Name: referral_progress_viewed referral_progress_viewed_pkey; Type: CONSTRAINT; Schema: social; Owner: -
--

ALTER TABLE ONLY social.referral_progress_viewed
    ADD CONSTRAINT referral_progress_viewed_pkey PRIMARY KEY (referrer);


--
-- Name: referral_reward_images referral_reward_images_pkey; Type: CONSTRAINT; Schema: social; Owner: -
--

ALTER TABLE ONLY social.referral_reward_images
    ADD CONSTRAINT referral_reward_images_pkey PRIMARY KEY (id);


--
-- Name: registry_profile_cache registry_profile_cache_pkey; Type: CONSTRAINT; Schema: social; Owner: -
--

ALTER TABLE ONLY social.registry_profile_cache
    ADD CONSTRAINT registry_profile_cache_pkey PRIMARY KEY (profile_key);


--
-- Name: scene_admins scene_admins_pkey; Type: CONSTRAINT; Schema: social; Owner: -
--

ALTER TABLE ONLY social.scene_admins
    ADD CONSTRAINT scene_admins_pkey PRIMARY KEY (place_id, admin_address);


--
-- Name: scene_bans scene_bans_pkey; Type: CONSTRAINT; Schema: social; Owner: -
--

ALTER TABLE ONLY social.scene_bans
    ADD CONSTRAINT scene_bans_pkey PRIMARY KEY (place_id, banned_address);


--
-- Name: scene_stream_access scene_stream_access_pkey; Type: CONSTRAINT; Schema: social; Owner: -
--

ALTER TABLE ONLY social.scene_stream_access
    ADD CONSTRAINT scene_stream_access_pkey PRIMARY KEY (id);


--
-- Name: scene_stream_cleanup scene_stream_cleanup_pkey; Type: CONSTRAINT; Schema: social; Owner: -
--

ALTER TABLE ONLY social.scene_stream_cleanup
    ADD CONSTRAINT scene_stream_cleanup_pkey PRIMARY KEY (access_id);


--
-- Name: subscription_opt_outs subscription_opt_outs_pkey; Type: CONSTRAINT; Schema: social; Owner: -
--

ALTER TABLE ONLY social.subscription_opt_outs
    ADD CONSTRAINT subscription_opt_outs_pkey PRIMARY KEY (address, scope, scope_id);


--
-- Name: subscriptions subscriptions_pkey; Type: CONSTRAINT; Schema: social; Owner: -
--

ALTER TABLE ONLY social.subscriptions
    ADD CONSTRAINT subscriptions_pkey PRIMARY KEY (address);


--
-- Name: translation_cache translation_cache_pkey; Type: CONSTRAINT; Schema: social; Owner: -
--

ALTER TABLE ONLY social.translation_cache
    ADD CONSTRAINT translation_cache_pkey PRIMARY KEY (backend, target_lang, text_sha256);


--
-- Name: unconfirmed_emails unconfirmed_emails_pkey; Type: CONSTRAINT; Schema: social; Owner: -
--

ALTER TABLE ONLY social.unconfirmed_emails
    ADD CONSTRAINT unconfirmed_emails_pkey PRIMARY KEY (address);


--
-- Name: user_achieved_tiers user_achieved_tiers_pkey; Type: CONSTRAINT; Schema: social; Owner: -
--

ALTER TABLE ONLY social.user_achieved_tiers
    ADD CONSTRAINT user_achieved_tiers_pkey PRIMARY KEY (address, badge_id, tier_id);


--
-- Name: user_badge_progress user_badge_progress_pkey; Type: CONSTRAINT; Schema: social; Owner: -
--

ALTER TABLE ONLY social.user_badge_progress
    ADD CONSTRAINT user_badge_progress_pkey PRIMARY KEY (address, badge_id);


--
-- Name: user_bans user_bans_pkey; Type: CONSTRAINT; Schema: social; Owner: -
--

ALTER TABLE ONLY social.user_bans
    ADD CONSTRAINT user_bans_pkey PRIMARY KEY (id);


--
-- Name: user_mutes user_mutes_pkey; Type: CONSTRAINT; Schema: social; Owner: -
--

ALTER TABLE ONLY social.user_mutes
    ADD CONSTRAINT user_mutes_pkey PRIMARY KEY (muter_address, muted_address);


--
-- Name: user_settings user_settings_pkey; Type: CONSTRAINT; Schema: social; Owner: -
--

ALTER TABLE ONLY social.user_settings
    ADD CONSTRAINT user_settings_pkey PRIMARY KEY (address);


--
-- Name: user_warnings user_warnings_pkey; Type: CONSTRAINT; Schema: social; Owner: -
--

ALTER TABLE ONLY social.user_warnings
    ADD CONSTRAINT user_warnings_pkey PRIMARY KEY (id);


--
-- Name: worn_history worn_history_pkey; Type: CONSTRAINT; Schema: social; Owner: -
--

ALTER TABLE ONLY social.worn_history
    ADD CONSTRAINT worn_history_pkey PRIMARY KEY (address, urn);


--
-- Name: events events_pkey; Type: CONSTRAINT; Schema: telemetry; Owner: -
--

ALTER TABLE ONLY telemetry.events
    ADD CONSTRAINT events_pkey PRIMARY KEY (id);


--
-- Name: experiment_group_targets experiment_group_targets_pkey; Type: CONSTRAINT; Schema: telemetry; Owner: -
--

ALTER TABLE ONLY telemetry.experiment_group_targets
    ADD CONSTRAINT experiment_group_targets_pkey PRIMARY KEY (exp_key, group_name);


--
-- Name: experiment_overrides experiment_overrides_pkey; Type: CONSTRAINT; Schema: telemetry; Owner: -
--

ALTER TABLE ONLY telemetry.experiment_overrides
    ADD CONSTRAINT experiment_overrides_pkey PRIMARY KEY (exp_key);


--
-- Name: flag_group_targets flag_group_targets_pkey; Type: CONSTRAINT; Schema: telemetry; Owner: -
--

ALTER TABLE ONLY telemetry.flag_group_targets
    ADD CONSTRAINT flag_group_targets_pkey PRIMARY KEY (flag, group_name);


--
-- Name: flag_groups flag_groups_pkey; Type: CONSTRAINT; Schema: telemetry; Owner: -
--

ALTER TABLE ONLY telemetry.flag_groups
    ADD CONSTRAINT flag_groups_pkey PRIMARY KEY (name);


--
-- Name: flag_overrides flag_overrides_pkey; Type: CONSTRAINT; Schema: telemetry; Owner: -
--

ALTER TABLE ONLY telemetry.flag_overrides
    ADD CONSTRAINT flag_overrides_pkey PRIMARY KEY (flag);


--
-- Name: ingest_settings ingest_settings_pkey; Type: CONSTRAINT; Schema: telemetry; Owner: -
--

ALTER TABLE ONLY telemetry.ingest_settings
    ADD CONSTRAINT ingest_settings_pkey PRIMARY KEY (id);


--
-- Name: issue_merges issue_merges_pkey; Type: CONSTRAINT; Schema: telemetry; Owner: -
--

ALTER TABLE ONLY telemetry.issue_merges
    ADD CONSTRAINT issue_merges_pkey PRIMARY KEY (source_fingerprint);


--
-- Name: issue_state issue_state_pkey; Type: CONSTRAINT; Schema: telemetry; Owner: -
--

ALTER TABLE ONLY telemetry.issue_state
    ADD CONSTRAINT issue_state_pkey PRIMARY KEY (fingerprint);


--
-- Name: product_areas product_areas_pkey; Type: CONSTRAINT; Schema: telemetry; Owner: -
--

ALTER TABLE ONLY telemetry.product_areas
    ADD CONSTRAINT product_areas_pkey PRIMARY KEY (kind, name);


--
-- Name: project_quotas project_quotas_pkey; Type: CONSTRAINT; Schema: telemetry; Owner: -
--

ALTER TABLE ONLY telemetry.project_quotas
    ADD CONSTRAINT project_quotas_pkey PRIMARY KEY (project);


--
-- Name: release_state release_state_pkey; Type: CONSTRAINT; Schema: telemetry; Owner: -
--

ALTER TABLE ONLY telemetry.release_state
    ADD CONSTRAINT release_state_pkey PRIMARY KEY (release);


--
-- Name: blocked_wallets blocked_wallets_pkey; Type: CONSTRAINT; Schema: worlds; Owner: -
--

ALTER TABLE ONLY worlds.blocked_wallets
    ADD CONSTRAINT blocked_wallets_pkey PRIMARY KEY (wallet);


--
-- Name: env_variables env_variables_pkey; Type: CONSTRAINT; Schema: worlds; Owner: -
--

ALTER TABLE ONLY worlds.env_variables
    ADD CONSTRAINT env_variables_pkey PRIMARY KEY (realm, scene_namespace, key);


--
-- Name: federation_peer_status federation_peer_status_pkey; Type: CONSTRAINT; Schema: worlds; Owner: -
--

ALTER TABLE ONLY worlds.federation_peer_status
    ADD CONSTRAINT federation_peer_status_pkey PRIMARY KEY (peer_id);


--
-- Name: federation_worlds federation_worlds_pkey; Type: CONSTRAINT; Schema: worlds; Owner: -
--

ALTER TABLE ONLY worlds.federation_worlds
    ADD CONSTRAINT federation_worlds_pkey PRIMARY KEY (peer_id, world_name);


--
-- Name: forward_outbox forward_outbox_entity_id_key; Type: CONSTRAINT; Schema: worlds; Owner: -
--

ALTER TABLE ONLY worlds.forward_outbox
    ADD CONSTRAINT forward_outbox_entity_id_key UNIQUE (entity_id);


--
-- Name: forward_outbox forward_outbox_pkey; Type: CONSTRAINT; Schema: worlds; Owner: -
--

ALTER TABLE ONLY worlds.forward_outbox
    ADD CONSTRAINT forward_outbox_pkey PRIMARY KEY (id);


--
-- Name: mirror_passes mirror_passes_pkey; Type: CONSTRAINT; Schema: worlds; Owner: -
--

ALTER TABLE ONLY worlds.mirror_passes
    ADD CONSTRAINT mirror_passes_pkey PRIMARY KEY (mirror);


--
-- Name: player_storage player_storage_pkey; Type: CONSTRAINT; Schema: worlds; Owner: -
--

ALTER TABLE ONLY worlds.player_storage
    ADD CONSTRAINT player_storage_pkey PRIMARY KEY (realm, scene_namespace, player_address, key);


--
-- Name: remote_worlds remote_worlds_pkey; Type: CONSTRAINT; Schema: worlds; Owner: -
--

ALTER TABLE ONLY worlds.remote_worlds
    ADD CONSTRAINT remote_worlds_pkey PRIMARY KEY (world_name);


--
-- Name: world_access_communities world_access_communities_pkey; Type: CONSTRAINT; Schema: worlds; Owner: -
--

ALTER TABLE ONLY worlds.world_access_communities
    ADD CONSTRAINT world_access_communities_pkey PRIMARY KEY (world_name, community_id);


--
-- Name: world_access_log world_access_log_pkey; Type: CONSTRAINT; Schema: worlds; Owner: -
--

ALTER TABLE ONLY worlds.world_access_log
    ADD CONSTRAINT world_access_log_pkey PRIMARY KEY (id);


--
-- Name: world_permission_parcels world_permission_parcels_pkey; Type: CONSTRAINT; Schema: worlds; Owner: -
--

ALTER TABLE ONLY worlds.world_permission_parcels
    ADD CONSTRAINT world_permission_parcels_pkey PRIMARY KEY (world_name, permission, address, parcel);


--
-- Name: world_permissions world_permissions_pkey; Type: CONSTRAINT; Schema: worlds; Owner: -
--

ALTER TABLE ONLY worlds.world_permissions
    ADD CONSTRAINT world_permissions_pkey PRIMARY KEY (world_name, permission, address);


--
-- Name: world_scene_identity world_scene_identity_pkey; Type: CONSTRAINT; Schema: worlds; Owner: -
--

ALTER TABLE ONLY worlds.world_scene_identity
    ADD CONSTRAINT world_scene_identity_pkey PRIMARY KEY (identity);


--
-- Name: world_scenes world_scenes_pkey; Type: CONSTRAINT; Schema: worlds; Owner: -
--

ALTER TABLE ONLY worlds.world_scenes
    ADD CONSTRAINT world_scenes_pkey PRIMARY KEY (world_name, entity_id);


--
-- Name: world_storage world_storage_pkey; Type: CONSTRAINT; Schema: worlds; Owner: -
--

ALTER TABLE ONLY worlds.world_storage
    ADD CONSTRAINT world_storage_pkey PRIMARY KEY (realm, scene_namespace, key);


--
-- Name: worlds worlds_pkey; Type: CONSTRAINT; Schema: worlds; Owner: -
--

ALTER TABLE ONLY worlds.worlds
    ADD CONSTRAINT worlds_pkey PRIMARY KEY (name);


--
-- Name: asset_packs_project_idx; Type: INDEX; Schema: builder; Owner: -
--

CREATE INDEX asset_packs_project_idx ON builder.asset_packs USING btree (project_id, created_at, id);


--
-- Name: assets_pack_ordinal_idx; Type: INDEX; Schema: builder; Owner: -
--

CREATE INDEX assets_pack_ordinal_idx ON builder.assets USING btree (asset_pack_id, ordinal);


--
-- Name: collections_owner_idx; Type: INDEX; Schema: builder; Owner: -
--

CREATE INDEX collections_owner_idx ON builder.collections USING btree (eth_address);


--
-- Name: items_collection_created_idx; Type: INDEX; Schema: builder; Owner: -
--

CREATE INDEX items_collection_created_idx ON builder.items USING btree (collection_id, created_at, id);


--
-- Name: items_owner_updated_idx; Type: INDEX; Schema: builder; Owner: -
--

CREATE INDEX items_owner_updated_idx ON builder.items USING btree (eth_address, updated_at DESC);


--
-- Name: projects_owner_updated_idx; Type: INDEX; Schema: builder; Owner: -
--

CREATE INDEX projects_owner_updated_idx ON builder.projects USING btree (eth_address, updated_at DESC, id) WHERE (NOT is_deleted);


--
-- Name: images_image_hash_idx; Type: INDEX; Schema: camera_reel; Owner: -
--

CREATE INDEX images_image_hash_idx ON camera_reel.images USING btree (image_hash);


--
-- Name: images_place_public_created_idx; Type: INDEX; Schema: camera_reel; Owner: -
--

CREATE INDEX images_place_public_created_idx ON camera_reel.images USING btree (place_id, created_at DESC, id DESC) WHERE (is_public AND (review_status <> 'rejected'::text));


--
-- Name: images_ranking_pending_idx; Type: INDEX; Schema: camera_reel; Owner: -
--

CREATE INDEX images_ranking_pending_idx ON camera_reel.images USING btree (id) WHERE (NOT ranking_published);


--
-- Name: images_thumbnail_hash_idx; Type: INDEX; Schema: camera_reel; Owner: -
--

CREATE INDEX images_thumbnail_hash_idx ON camera_reel.images USING btree (thumbnail_hash);


--
-- Name: images_user_created_idx; Type: INDEX; Schema: camera_reel; Owner: -
--

CREATE INDEX images_user_created_idx ON camera_reel.images USING btree (user_address, created_at DESC, id DESC);


--
-- Name: images_wearable_items_idx; Type: INDEX; Schema: camera_reel; Owner: -
--

CREATE INDEX images_wearable_items_idx ON camera_reel.images USING gin (camera_reel.image_wearable_items(metadata)) WHERE (is_public AND (review_status <> 'rejected'::text));


--
-- Name: actions_expires_at_idx; Type: INDEX; Schema: chat; Owner: -
--

CREATE INDEX actions_expires_at_idx ON chat.actions USING btree (expires_at);


--
-- Name: community_activity_object_uidx; Type: INDEX; Schema: chat; Owner: -
--

CREATE UNIQUE INDEX community_activity_object_uidx ON chat.community_activity USING btree (community_id, kind, object);


--
-- Name: messages_channel_seq_idx; Type: INDEX; Schema: chat; Owner: -
--

CREATE INDEX messages_channel_seq_idx ON chat.messages USING btree (community_id, channel, seq);


--
-- Name: messages_id_uidx; Type: INDEX; Schema: chat; Owner: -
--

CREATE UNIQUE INDEX messages_id_uidx ON chat.messages USING btree (id);


--
-- Name: messages_slow_mode_idx; Type: INDEX; Schema: chat; Owner: -
--

CREATE INDEX messages_slow_mode_idx ON chat.messages USING btree (community_id, channel, wallet, created_at DESC);


--
-- Name: replies_id_uidx; Type: INDEX; Schema: chat; Owner: -
--

CREATE UNIQUE INDEX replies_id_uidx ON chat.replies USING btree (id);


--
-- Name: replies_parent_seq_idx; Type: INDEX; Schema: chat; Owner: -
--

CREATE INDEX replies_parent_seq_idx ON chat.replies USING btree (parent_id, seq DESC);


--
-- Name: active_pointers_entity_id_idx; Type: INDEX; Schema: content; Owner: -
--

CREATE INDEX active_pointers_entity_id_idx ON content.active_pointers USING btree (entity_id);


--
-- Name: active_pointers_entity_type_idx; Type: INDEX; Schema: content; Owner: -
--

CREATE INDEX active_pointers_entity_type_idx ON content.active_pointers USING btree (entity_type);


--
-- Name: active_pointers_pointer_ops_idx; Type: INDEX; Schema: content; Owner: -
--

CREATE INDEX active_pointers_pointer_ops_idx ON content.active_pointers USING btree (pointer varchar_pattern_ops);


--
-- Name: content_files_content_hash_index; Type: INDEX; Schema: content; Owner: -
--

CREATE INDEX content_files_content_hash_index ON content.content_files USING btree (content_hash);


--
-- Name: deployer_address_lower_case; Type: INDEX; Schema: content; Owner: -
--

CREATE INDEX deployer_address_lower_case ON content.deployments USING btree (lower(deployer_address) text_pattern_ops);


--
-- Name: deployments_deployer_addr_ts_idx; Type: INDEX; Schema: content; Owner: -
--

CREATE INDEX deployments_deployer_addr_ts_idx ON content.deployments USING btree (deployer_address, entity_timestamp DESC);


--
-- Name: deployments_entity_pointers_index; Type: INDEX; Schema: content; Owner: -
--

CREATE INDEX deployments_entity_pointers_index ON content.deployments USING gin (entity_pointers);


--
-- Name: deployments_entity_timestamp_entity_id_idx; Type: INDEX; Schema: content; Owner: -
--

CREATE INDEX deployments_entity_timestamp_entity_id_idx ON content.deployments USING btree (entity_timestamp DESC, entity_id DESC);


--
-- Name: deployments_entity_type_index; Type: INDEX; Schema: content; Owner: -
--

CREATE INDEX deployments_entity_type_index ON content.deployments USING btree (entity_type);


--
-- Name: deployments_full_snapshots_ix; Type: INDEX; Schema: content; Owner: -
--

CREATE INDEX deployments_full_snapshots_ix ON content.deployments USING btree (deleter_deployment DESC, local_timestamp) INCLUDE (entity_type);


--
-- Name: deployments_local_timestamp_lower_entity_id_idx; Type: INDEX; Schema: content; Owner: -
--

CREATE INDEX deployments_local_timestamp_lower_entity_id_idx ON content.deployments USING btree (local_timestamp DESC, lower(entity_id) DESC);


--
-- Name: deployments_scene_logs_permissions_idx; Type: INDEX; Schema: content; Owner: -
--

CREATE INDEX deployments_scene_logs_permissions_idx ON content.deployments USING gin (worlds.logs_permissions(((entity_metadata -> 'logsPermissions'::text))::text)) WHERE ((entity_type = 'scene'::text) AND (deleter_deployment IS NULL));


--
-- Name: deployments_type_entity_ts_idx; Type: INDEX; Schema: content; Owner: -
--

CREATE INDEX deployments_type_entity_ts_idx ON content.deployments USING btree (entity_type, entity_timestamp DESC, entity_id DESC);


--
-- Name: deployments_type_local_ts_idx; Type: INDEX; Schema: content; Owner: -
--

CREATE INDEX deployments_type_local_ts_idx ON content.deployments USING btree (entity_type, local_timestamp DESC, lower(entity_id) DESC);


--
-- Name: failed_deployments_due_idx; Type: INDEX; Schema: content; Owner: -
--

CREATE INDEX failed_deployments_due_idx ON content.failed_deployments USING btree (next_retry_at, entity_id);


--
-- Name: forward_outbox_due_idx; Type: INDEX; Schema: content; Owner: -
--

CREATE INDEX forward_outbox_due_idx ON content.forward_outbox USING btree (target, next_attempt_at, id) WHERE (status = ANY (ARRAY['queued'::text, 'sending'::text]));


--
-- Name: local_entities_tombstoned_idx; Type: INDEX; Schema: content; Owner: -
--

CREATE INDEX local_entities_tombstoned_idx ON content.local_entities USING btree (tombstoned_at) WHERE (tombstoned_at IS NOT NULL);


--
-- Name: authorizations_address_created_idx; Type: INDEX; Schema: credits; Owner: -
--

CREATE INDEX authorizations_address_created_idx ON credits.authorizations USING btree (address, created_at DESC, id);


--
-- Name: authorizations_address_status_expiry_idx; Type: INDEX; Schema: credits; Owner: -
--

CREATE INDEX authorizations_address_status_expiry_idx ON credits.authorizations USING btree (address, status, expires_at);


--
-- Name: authorizations_expiry_idx; Type: INDEX; Schema: credits; Owner: -
--

CREATE INDEX authorizations_expiry_idx ON credits.authorizations USING btree (expires_at) WHERE (status = ANY (ARRAY['authorized'::text, 'retired'::text]));


--
-- Name: authorizations_idempotency_key_uidx; Type: INDEX; Schema: credits; Owner: -
--

CREATE UNIQUE INDEX authorizations_idempotency_key_uidx ON credits.authorizations USING btree (idempotency_key) WHERE (idempotency_key IS NOT NULL);


--
-- Name: captcha_challenges_expires_idx; Type: INDEX; Schema: credits; Owner: -
--

CREATE INDEX captcha_challenges_expires_idx ON credits.captcha_challenges USING btree (expires_at);


--
-- Name: captcha_challenges_open_idx; Type: INDEX; Schema: credits; Owner: -
--

CREATE INDEX captcha_challenges_open_idx ON credits.captcha_challenges USING btree (address, id DESC) WHERE (consumed_at IS NULL);


--
-- Name: checkouts_address_id_idx; Type: INDEX; Schema: credits; Owner: -
--

CREATE INDEX checkouts_address_id_idx ON credits.checkouts USING btree (address, id DESC);


--
-- Name: fulfillment_outbox_checkout_idx; Type: INDEX; Schema: credits; Owner: -
--

CREATE INDEX fulfillment_outbox_checkout_idx ON credits.fulfillment_outbox USING btree (checkout_id, id);


--
-- Name: fulfillment_outbox_external_ref_idx; Type: INDEX; Schema: credits; Owner: -
--

CREATE INDEX fulfillment_outbox_external_ref_idx ON credits.fulfillment_outbox USING btree (external_ref) WHERE (external_ref IS NOT NULL);


--
-- Name: fulfillment_outbox_pending_idx; Type: INDEX; Schema: credits; Owner: -
--

CREATE INDEX fulfillment_outbox_pending_idx ON credits.fulfillment_outbox USING btree (id) WHERE (status = 'pending'::text);


--
-- Name: ledger_address_id_idx; Type: INDEX; Schema: credits; Owner: -
--

CREATE INDEX ledger_address_id_idx ON credits.ledger USING btree (address, id DESC);


--
-- Name: ledger_tx_ref_idx; Type: INDEX; Schema: credits; Owner: -
--

CREATE INDEX ledger_tx_ref_idx ON credits.ledger USING btree (tx_ref, address) WHERE (tx_ref IS NOT NULL);


--
-- Name: purchases_address_idx; Type: INDEX; Schema: credits; Owner: -
--

CREATE INDEX purchases_address_idx ON credits.purchases USING btree (address, id DESC);


--
-- Name: purchases_checkout_session_uidx; Type: INDEX; Schema: credits; Owner: -
--

CREATE UNIQUE INDEX purchases_checkout_session_uidx ON credits.purchases USING btree (stripe_checkout_session) WHERE (stripe_checkout_session IS NOT NULL);


--
-- Name: purchases_order_id_uidx; Type: INDEX; Schema: credits; Owner: -
--

CREATE UNIQUE INDEX purchases_order_id_uidx ON credits.purchases USING btree (order_id) WHERE (order_id IS NOT NULL);


--
-- Name: purchases_payment_intent_uidx; Type: INDEX; Schema: credits; Owner: -
--

CREATE UNIQUE INDEX purchases_payment_intent_uidx ON credits.purchases USING btree (stripe_payment_intent) WHERE (stripe_payment_intent IS NOT NULL);


--
-- Name: revocations_due_idx; Type: INDEX; Schema: credits; Owner: -
--

CREATE INDEX revocations_due_idx ON credits.revocations USING btree (created_at) WHERE (status = ANY (ARRAY['pending'::text, 'sent'::text]));


--
-- Name: usage_settlements_authorization_idx; Type: INDEX; Schema: credits; Owner: -
--

CREATE INDEX usage_settlements_authorization_idx ON credits.usage_settlements USING btree (authorization_id, block_number, log_index);


--
-- Name: usages_salt_idx; Type: INDEX; Schema: credits; Owner: -
--

CREATE INDEX usages_salt_idx ON credits.usages USING btree (salt);


--
-- Name: price_override_audit_key_idx; Type: INDEX; Schema: economy; Owner: -
--

CREATE INDEX price_override_audit_key_idx ON economy.price_override_audit USING btree (token_id, vs_currency, id DESC);


--
-- Name: price_snapshots_newest_idx; Type: INDEX; Schema: economy; Owner: -
--

CREATE INDEX price_snapshots_newest_idx ON economy.price_snapshots USING btree (taken_at DESC);


--
-- Name: price_snapshots_taken_at_idx; Type: INDEX; Schema: economy; Owner: -
--

CREATE INDEX price_snapshots_taken_at_idx ON economy.price_snapshots USING btree (taken_at DESC) WHERE (source = 'coingecko'::text);


--
-- Name: transactions_user_created_idx; Type: INDEX; Schema: economy; Owner: -
--

CREATE INDEX transactions_user_created_idx ON economy.transactions USING btree (user_address, created_at);


--
-- Name: event_approved_live_idx; Type: INDEX; Schema: explore; Owner: -
--

CREATE INDEX event_approved_live_idx ON explore.event USING btree (approved, next_finish_at);


--
-- Name: event_attendance_local_signer_action_idx; Type: INDEX; Schema: explore; Owner: -
--

CREATE INDEX event_attendance_local_signer_action_idx ON explore.event_attendance_local USING btree (signer, action);


--
-- Name: event_coords_idx; Type: INDEX; Schema: explore; Owner: -
--

CREATE INDEX event_coords_idx ON explore.event USING btree (coordinates_x, coordinates_y);


--
-- Name: event_fetched_at_idx; Type: INDEX; Schema: explore; Owner: -
--

CREATE INDEX event_fetched_at_idx ON explore.event USING btree (fetched_at);


--
-- Name: event_notifications_live_idx; Type: INDEX; Schema: explore; Owner: -
--

CREATE INDEX event_notifications_live_idx ON explore.event USING btree (finish_at, id) WHERE approved;


--
-- Name: idx_seen_nonces_expires; Type: INDEX; Schema: explore; Owner: -
--

CREATE INDEX idx_seen_nonces_expires ON explore.seen_nonces USING btree (expires_at);


--
-- Name: opensea_nft_resolved_at_idx; Type: INDEX; Schema: explore; Owner: -
--

CREATE INDEX opensea_nft_resolved_at_idx ON explore.opensea_nft USING btree (resolved_at);


--
-- Name: place_fetched_at_idx; Type: INDEX; Schema: explore; Owner: -
--

CREATE INDEX place_fetched_at_idx ON explore.place USING btree (fetched_at);


--
-- Name: place_like_score_guarded_place_only_idx; Type: INDEX; Schema: explore; Owner: -
--

CREATE INDEX place_like_score_guarded_place_only_idx ON explore.place USING btree ((
CASE
    WHEN ((jsonb_typeof((raw -> 'like_score'::text)) = ANY (ARRAY['number'::text, 'string'::text])) AND ((raw ->> 'like_score'::text) ~ '^-?(([0-9]{1,255}|[0-9]{100}[0-9]{1,208})(\.([0-9]{1,255}|[0-9]{100}[0-9]{1,223}))?|[0-9]{1,3}(\.[0-9]{1,17})?[eE][+-]?([0-9]{1,2}|[12][0-9]{2}|30[0-5])|[0-9](\.[0-9]{1,17})?[eE][+-]?30[6-7]|1(\.(7[0-9]?|[0-6][0-9]{0,16}))?[eE][+-]?308)$'::text)) THEN ((raw ->> 'like_score'::text))::double precision
    ELSE NULL::double precision
END) DESC NULLS LAST, deployed_at DESC) WHERE ((disabled IS FALSE) AND (world IS FALSE));


--
-- Name: place_like_score_guarded_world_only_idx; Type: INDEX; Schema: explore; Owner: -
--

CREATE INDEX place_like_score_guarded_world_only_idx ON explore.place USING btree ((
CASE
    WHEN ((jsonb_typeof((raw -> 'like_score'::text)) = ANY (ARRAY['number'::text, 'string'::text])) AND ((raw ->> 'like_score'::text) ~ '^-?(([0-9]{1,255}|[0-9]{100}[0-9]{1,208})(\.([0-9]{1,255}|[0-9]{100}[0-9]{1,223}))?|[0-9]{1,3}(\.[0-9]{1,17})?[eE][+-]?([0-9]{1,2}|[12][0-9]{2}|30[0-5])|[0-9](\.[0-9]{1,17})?[eE][+-]?30[6-7]|1(\.(7[0-9]?|[0-6][0-9]{0,16}))?[eE][+-]?308)$'::text)) THEN ((raw ->> 'like_score'::text))::double precision
    ELSE NULL::double precision
END) DESC NULLS LAST, deployed_at DESC) WHERE ((disabled IS FALSE) AND (world IS TRUE));


--
-- Name: place_positions_path_gin; Type: INDEX; Schema: explore; Owner: -
--

CREATE INDEX place_positions_path_gin ON explore.place USING gin (((raw -> 'positions'::text)) jsonb_path_ops);


--
-- Name: place_removal_due_idx; Type: INDEX; Schema: explore; Owner: -
--

CREATE INDEX place_removal_due_idx ON explore.place_removal_pending USING btree (retry_at, event_id);


--
-- Name: place_reports_entity_created_idx; Type: INDEX; Schema: explore; Owner: -
--

CREATE INDEX place_reports_entity_created_idx ON explore.place_reports USING btree (entity_id, created_at DESC, id DESC);


--
-- Name: place_reports_status_created_idx; Type: INDEX; Schema: explore; Owner: -
--

CREATE INDEX place_reports_status_created_idx ON explore.place_reports USING btree (status, created_at DESC, id DESC);


--
-- Name: place_world_local_owner_idx; Type: INDEX; Schema: explore; Owner: -
--

CREATE INDEX place_world_local_owner_idx ON explore.place_world_local USING btree (lower(COALESCE((raw ->> 'owner'::text), creator_address))) WHERE (world IS TRUE);


--
-- Name: place_world_local_positions_path_gin; Type: INDEX; Schema: explore; Owner: -
--

CREATE INDEX place_world_local_positions_path_gin ON explore.place_world_local USING gin (((raw -> 'positions'::text)) jsonb_path_ops);


--
-- Name: place_world_local_world_name_col_idx; Type: INDEX; Schema: explore; Owner: -
--

CREATE INDEX place_world_local_world_name_col_idx ON explore.place_world_local USING btree (lower(world_name));


--
-- Name: place_world_name_col_idx; Type: INDEX; Schema: explore; Owner: -
--

CREATE INDEX place_world_name_col_idx ON explore.place USING btree (lower(world_name));


--
-- Name: place_world_owner_idx; Type: INDEX; Schema: explore; Owner: -
--

CREATE INDEX place_world_owner_idx ON explore.place USING btree (lower(COALESCE((raw ->> 'owner'::text), creator_address))) WHERE (world IS TRUE);


--
-- Name: signed_actions_scope_seq_idx; Type: INDEX; Schema: explore; Owner: -
--

CREATE INDEX signed_actions_scope_seq_idx ON explore.signed_actions USING btree (scope, seq);


--
-- Name: user_favorites_entity_idx; Type: INDEX; Schema: explore; Owner: -
--

CREATE INDEX user_favorites_entity_idx ON explore.user_favorites USING btree (entity_id);


--
-- Name: user_likes_entity_idx; Type: INDEX; Schema: explore; Owner: -
--

CREATE INDEX user_likes_entity_idx ON explore.user_likes USING btree (entity_id);


--
-- Name: world_scene_place_deployment_idx; Type: INDEX; Schema: explore; Owner: -
--

CREATE UNIQUE INDEX world_scene_place_deployment_idx ON explore.world_scene_place USING btree (world_name, entity_id) WHERE active;


--
-- Name: discourse_posts_topic_created_idx; Type: INDEX; Schema: governance; Owner: -
--

CREATE INDEX discourse_posts_topic_created_idx ON governance.discourse_posts USING btree (topic_id, created_at DESC);


--
-- Name: discourse_topics_fetched_idx; Type: INDEX; Schema: governance; Owner: -
--

CREATE INDEX discourse_topics_fetched_idx ON governance.discourse_topics USING btree (fetched_at);


--
-- Name: project_updates_created_at_idx; Type: INDEX; Schema: governance; Owner: -
--

CREATE INDEX project_updates_created_at_idx ON governance.project_updates USING btree (created_at DESC);


--
-- Name: project_updates_project_idx; Type: INDEX; Schema: governance; Owner: -
--

CREATE INDEX project_updates_project_idx ON governance.project_updates USING btree (project_id, created_at);


--
-- Name: proposals_created_at_idx; Type: INDEX; Schema: governance; Owner: -
--

CREATE INDEX proposals_created_at_idx ON governance.proposals USING btree (created_at DESC);


--
-- Name: proposals_fetched_at_idx; Type: INDEX; Schema: governance; Owner: -
--

CREATE INDEX proposals_fetched_at_idx ON governance.proposals USING btree (fetched_at DESC, id);


--
-- Name: proposals_finish_at_idx; Type: INDEX; Schema: governance; Owner: -
--

CREATE INDEX proposals_finish_at_idx ON governance.proposals USING btree (finish_at DESC);


--
-- Name: proposals_status_idx; Type: INDEX; Schema: governance; Owner: -
--

CREATE INDEX proposals_status_idx ON governance.proposals USING btree (status);


--
-- Name: proposals_type_idx; Type: INDEX; Schema: governance; Owner: -
--

CREATE INDEX proposals_type_idx ON governance.proposals USING btree (type);


--
-- Name: snapshot_votes_created_idx; Type: INDEX; Schema: governance; Owner: -
--

CREATE INDEX snapshot_votes_created_idx ON governance.snapshot_votes USING btree (created_at DESC);


--
-- Name: account_right_account_idx; Type: INDEX; Schema: land_authz; Owner: -
--

CREATE INDEX account_right_account_idx ON land_authz.account_right USING btree (account, token_address, kind) WHERE is_approved;


--
-- Name: account_right_operator_idx; Type: INDEX; Schema: land_authz; Owner: -
--

CREATE INDEX account_right_operator_idx ON land_authz.account_right USING btree (operator, kind) WHERE is_approved;


--
-- Name: authz_event_account_idx; Type: INDEX; Schema: land_authz; Owner: -
--

CREATE INDEX authz_event_account_idx ON land_authz.authz_event USING btree (token_address, account, operator, kind, block_number, log_index) WHERE ((account IS NOT NULL) AND (operator IS NOT NULL));


--
-- Name: authz_event_missing_time_idx; Type: INDEX; Schema: land_authz; Owner: -
--

CREATE INDEX authz_event_missing_time_idx ON land_authz.authz_event USING btree (block_number) WHERE (block_time <= 0);


--
-- Name: authz_event_ownership_idx; Type: INDEX; Schema: land_authz; Owner: -
--

CREATE INDEX authz_event_ownership_idx ON land_authz.authz_event USING btree (token_address, token_id, block_number, log_index) WHERE ((token_id IS NOT NULL) AND (kind = ANY (ARRAY['transfer'::text, 'add_land'::text, 'remove_land'::text])));


--
-- Name: authz_event_token_idx; Type: INDEX; Schema: land_authz; Owner: -
--

CREATE INDEX authz_event_token_idx ON land_authz.authz_event USING btree (token_address, token_id, block_number, log_index) WHERE (token_id IS NOT NULL);


--
-- Name: estate_land_estate_idx; Type: INDEX; Schema: land_authz; Owner: -
--

CREATE INDEX estate_land_estate_idx ON land_authz.estate_land USING btree (estate_token_id);


--
-- Name: token_right_operator_idx; Type: INDEX; Schema: land_authz; Owner: -
--

CREATE INDEX token_right_operator_idx ON land_authz.token_right USING btree (operator) WHERE (operator IS NOT NULL);


--
-- Name: token_right_owner_idx; Type: INDEX; Schema: land_authz; Owner: -
--

CREATE INDEX token_right_owner_idx ON land_authz.token_right USING btree (token_address, owner) WHERE (owner IS NOT NULL);


--
-- Name: token_right_update_operator_idx; Type: INDEX; Schema: land_authz; Owner: -
--

CREATE INDEX token_right_update_operator_idx ON land_authz.token_right USING btree (update_operator) WHERE (update_operator IS NOT NULL);


--
-- Name: token_right_xy_idx; Type: INDEX; Schema: land_authz; Owner: -
--

CREATE INDEX token_right_xy_idx ON land_authz.token_right USING btree (x, y) WHERE (x IS NOT NULL);


--
-- Name: admin_audit_actor_idx; Type: INDEX; Schema: ops; Owner: -
--

CREATE INDEX admin_audit_actor_idx ON ops.admin_audit USING btree (actor, at DESC);


--
-- Name: admin_audit_at_idx; Type: INDEX; Schema: ops; Owner: -
--

CREATE INDEX admin_audit_at_idx ON ops.admin_audit USING btree (at DESC);


--
-- Name: admin_audit_domain_action_idx; Type: INDEX; Schema: ops; Owner: -
--

CREATE INDEX admin_audit_domain_action_idx ON ops.admin_audit USING btree (domain, action, at DESC);


--
-- Name: admin_audit_target_idx; Type: INDEX; Schema: ops; Owner: -
--

CREATE INDEX admin_audit_target_idx ON ops.admin_audit USING btree (target, at DESC) WHERE (target IS NOT NULL);


--
-- Name: scene_occupancy_pointer_idx; Type: INDEX; Schema: presence; Owner: -
--

CREATE INDEX scene_occupancy_pointer_idx ON presence.scene_occupancy USING btree (pointer, taken_at DESC);


--
-- Name: snapshots_taken_at_idx; Type: INDEX; Schema: presence; Owner: -
--

CREATE INDEX snapshots_taken_at_idx ON presence.snapshots USING btree (taken_at DESC);


--
-- Name: world_membership_world_idx; Type: INDEX; Schema: presence; Owner: -
--

CREATE INDEX world_membership_world_idx ON presence.world_membership USING btree (world_name, taken_at DESC);


--
-- Name: quest_events_instance_idx; Type: INDEX; Schema: quests; Owner: -
--

CREATE INDEX quest_events_instance_idx ON quests.quest_events USING btree (quest_instance_id, occurred_at, id);


--
-- Name: quest_instances_quest_idx; Type: INDEX; Schema: quests; Owner: -
--

CREATE INDEX quest_instances_quest_idx ON quests.quest_instances USING btree (quest_id, started_at) WHERE (abandoned_at IS NULL);


--
-- Name: quest_instances_user_quest_idx; Type: INDEX; Schema: quests; Owner: -
--

CREATE INDEX quest_instances_user_quest_idx ON quests.quest_instances USING btree (user_address, quest_id) WHERE (abandoned_at IS NULL);


--
-- Name: quests_creator_created_idx; Type: INDEX; Schema: quests; Owner: -
--

CREATE INDEX quests_creator_created_idx ON quests.quests USING btree (creator_address, created_at DESC);


--
-- Name: quests_previous_quest_uidx; Type: INDEX; Schema: quests; Owner: -
--

CREATE UNIQUE INDEX quests_previous_quest_uidx ON quests.quests USING btree (previous_quest_id) WHERE (previous_quest_id IS NOT NULL);


--
-- Name: admin_audit_target_idx; Type: INDEX; Schema: shop; Owner: -
--

CREATE INDEX admin_audit_target_idx ON shop.admin_audit USING btree (target_kind, target_hash, id DESC);


--
-- Name: broker_purchases_escrow_address_idx; Type: INDEX; Schema: shop; Owner: -
--

CREATE INDEX broker_purchases_escrow_address_idx ON shop.broker_purchases USING btree (escrow_address);


--
-- Name: broker_purchases_idempotency_key_uidx; Type: INDEX; Schema: shop; Owner: -
--

CREATE UNIQUE INDEX broker_purchases_idempotency_key_uidx ON shop.broker_purchases USING btree (idempotency_key) WHERE (idempotency_key IS NOT NULL);


--
-- Name: broker_purchases_trade_sig_uidx; Type: INDEX; Schema: shop; Owner: -
--

CREATE UNIQUE INDEX broker_purchases_trade_sig_uidx ON shop.broker_purchases USING btree (trade_hashed_signature) WHERE ((trade_hashed_signature IS NOT NULL) AND (status <> ALL (ARRAY['reverted'::text, 'error'::text])));


--
-- Name: broker_purchases_unsettled_idx; Type: INDEX; Schema: shop; Owner: -
--

CREATE INDEX broker_purchases_unsettled_idx ON shop.broker_purchases USING btree (id) WHERE (status = ANY (ARRAY['sent'::text, 'bought'::text, 'forwarding'::text]));


--
-- Name: collections_approved_name_idx; Type: INDEX; Schema: shop; Owner: -
--

CREATE INDEX collections_approved_name_idx ON shop.collections USING btree (name COLLATE "C") WHERE is_approved;


--
-- Name: collections_creator_idx; Type: INDEX; Schema: shop; Owner: -
--

CREATE INDEX collections_creator_idx ON shop.collections USING btree (creator);


--
-- Name: coupons_expires_idx; Type: INDEX; Schema: shop; Owner: -
--

CREATE INDEX coupons_expires_idx ON shop.coupons USING btree (expires_at);


--
-- Name: coupons_signer_created_idx; Type: INDEX; Schema: shop; Owner: -
--

CREATE INDEX coupons_signer_created_idx ON shop.coupons USING btree (signer, created_at DESC);


--
-- Name: disputes_status_idx; Type: INDEX; Schema: shop; Owner: -
--

CREATE INDEX disputes_status_idx ON shop.disputes USING btree (status, opened_at DESC);


--
-- Name: escrow_actions_escrow_address_idx; Type: INDEX; Schema: shop; Owner: -
--

CREATE INDEX escrow_actions_escrow_address_idx ON shop.escrow_actions USING btree (escrow_address);


--
-- Name: escrow_actions_idempotency_key_uidx; Type: INDEX; Schema: shop; Owner: -
--

CREATE UNIQUE INDEX escrow_actions_idempotency_key_uidx ON shop.escrow_actions USING btree (idempotency_key) WHERE (idempotency_key IS NOT NULL);


--
-- Name: escrow_actions_unsettled_idx; Type: INDEX; Schema: shop; Owner: -
--

CREATE INDEX escrow_actions_unsettled_idx ON shop.escrow_actions USING btree (id) WHERE (status = 'sent'::text);


--
-- Name: executions_actor_idx; Type: INDEX; Schema: shop; Owner: -
--

CREATE INDEX executions_actor_idx ON shop.executions USING btree (actor, created_at DESC);


--
-- Name: executions_due_idx; Type: INDEX; Schema: shop; Owner: -
--

CREATE INDEX executions_due_idx ON shop.executions USING btree (next_check_at) WHERE (status = ANY (ARRAY['submitted'::text, 'mined'::text]));


--
-- Name: fed_actions_kind_seq_idx; Type: INDEX; Schema: shop; Owner: -
--

CREATE INDEX fed_actions_kind_seq_idx ON shop.fed_actions USING btree (kind, seq);


--
-- Name: fed_seen_nonces_expires_idx; Type: INDEX; Schema: shop; Owner: -
--

CREATE INDEX fed_seen_nonces_expires_idx ON shop.fed_seen_nonces USING btree (expires_at);


--
-- Name: idx_trade_assets_beneficiary_created; Type: INDEX; Schema: shop; Owner: -
--

CREATE INDEX idx_trade_assets_beneficiary_created ON shop.trade_assets USING btree (beneficiary, created_at DESC) WHERE (beneficiary IS NOT NULL);


--
-- Name: idx_trade_assets_trade; Type: INDEX; Schema: shop; Owner: -
--

CREATE INDEX idx_trade_assets_trade ON shop.trade_assets USING btree (trade_id);


--
-- Name: idx_trades_created_at; Type: INDEX; Schema: shop; Owner: -
--

CREATE INDEX idx_trades_created_at ON shop.trades USING btree (created_at DESC);


--
-- Name: idx_trades_signer_created; Type: INDEX; Schema: shop; Owner: -
--

CREATE INDEX idx_trades_signer_created ON shop.trades USING btree (signer, created_at DESC);


--
-- Name: item_notify_outbox_due_idx; Type: INDEX; Schema: shop; Owner: -
--

CREATE INDEX item_notify_outbox_due_idx ON shop.item_notify_outbox USING btree (next_attempt_at, id) WHERE (status = 'pending'::text);


--
-- Name: item_notify_outbox_pending_once; Type: INDEX; Schema: shop; Owner: -
--

CREATE UNIQUE INDEX item_notify_outbox_pending_once ON shop.item_notify_outbox USING btree (request_id) WHERE (status = 'pending'::text);


--
-- Name: item_notify_requests_pending_idx; Type: INDEX; Schema: shop; Owner: -
--

CREATE INDEX item_notify_requests_pending_idx ON shop.item_notify_requests USING btree (contract_address, item_id, id) WHERE (status = 'pending'::text);


--
-- Name: items_approved_created_idx; Type: INDEX; Schema: shop; Owner: -
--

CREATE INDEX items_approved_created_idx ON shop.items USING btree (created_at DESC) WHERE search_is_collection_approved;


--
-- Name: items_creator_idx; Type: INDEX; Schema: shop; Owner: -
--

CREATE INDEX items_creator_idx ON shop.items USING btree (creator);


--
-- Name: lists_user_idx; Type: INDEX; Schema: shop; Owner: -
--

CREATE INDEX lists_user_idx ON shop.lists USING btree (user_address);


--
-- Name: mana_transfer_recipient; Type: INDEX; Schema: shop; Owner: -
--

CREATE INDEX mana_transfer_recipient ON shop.mana_transfer USING btree (network, recipient, block_number DESC, log_index DESC);


--
-- Name: mana_transfer_sender; Type: INDEX; Schema: shop; Owner: -
--

CREATE INDEX mana_transfer_sender ON shop.mana_transfer USING btree (network, sender, block_number DESC, log_index DESC);


--
-- Name: moderation_flags_kind_idx; Type: INDEX; Schema: shop; Owner: -
--

CREATE INDEX moderation_flags_kind_idx ON shop.moderation_flags USING btree (target_kind, flagged_at DESC);


--
-- Name: name_transfers_token_id_idx; Type: INDEX; Schema: shop; Owner: -
--

CREATE INDEX name_transfers_token_id_idx ON shop.name_transfers USING btree (registrar, token_id);


--
-- Name: name_transfers_unsettled_idx; Type: INDEX; Schema: shop; Owner: -
--

CREATE INDEX name_transfers_unsettled_idx ON shop.name_transfers USING btree (id) WHERE (status = 'sent'::text);


--
-- Name: nft_event_token_idx; Type: INDEX; Schema: shop; Owner: -
--

CREATE INDEX nft_event_token_idx ON shop.nft_event USING btree (network, contract_address, token_id, block_number, log_index);


--
-- Name: nft_item_idx; Type: INDEX; Schema: shop; Owner: -
--

CREATE INDEX nft_item_idx ON shop.nft USING btree (network, contract_address, item_id) WHERE (item_id IS NOT NULL);


--
-- Name: nft_order_open_idx; Type: INDEX; Schema: shop; Owner: -
--

CREATE INDEX nft_order_open_idx ON shop.nft_order USING btree (network, contract_address, expires_at) WHERE (status = 'open'::text);


--
-- Name: nft_owner_idx; Type: INDEX; Schema: shop; Owner: -
--

CREATE INDEX nft_owner_idx ON shop.nft USING btree (owner) WHERE (owner IS NOT NULL);


--
-- Name: outfits_published_created_idx; Type: INDEX; Schema: shop; Owner: -
--

CREATE INDEX outfits_published_created_idx ON shop.outfits USING btree (created_at DESC) WHERE published;


--
-- Name: picks_item_idx; Type: INDEX; Schema: shop; Owner: -
--

CREATE INDEX picks_item_idx ON shop.picks USING btree (item_id);


--
-- Name: picks_list_id_idx; Type: INDEX; Schema: shop; Owner: -
--

CREATE INDEX picks_list_id_idx ON shop.picks USING btree (list_id, created_at DESC);


--
-- Name: rental_event_asset_idx; Type: INDEX; Schema: shop; Owner: -
--

CREATE INDEX rental_event_asset_idx ON shop.rental_event USING btree (rentals_address, contract_address, token_id, block_number, log_index) WHERE (contract_address IS NOT NULL);


--
-- Name: rental_event_signature_idx; Type: INDEX; Schema: shop; Owner: -
--

CREATE INDEX rental_event_signature_idx ON shop.rental_event USING btree (signature) WHERE (kind = 'rented'::text);


--
-- Name: rental_event_signer_idx; Type: INDEX; Schema: shop; Owner: -
--

CREATE INDEX rental_event_signer_idx ON shop.rental_event USING btree (signer, new_index) WHERE (kind = 'signer_index_updated'::text);


--
-- Name: trade_chain_state_digest_idx; Type: INDEX; Schema: shop; Owner: -
--

CREATE INDEX trade_chain_state_digest_idx ON shop.trade_chain_state USING btree (digest) WHERE (digest IS NOT NULL);


--
-- Name: trade_event_index_idx; Type: INDEX; Schema: shop; Owner: -
--

CREATE INDEX trade_event_index_idx ON shop.trade_event USING btree (network, marketplace_address, kind, caller, block_number DESC, log_index DESC) WHERE (new_index IS NOT NULL);


--
-- Name: trade_event_signature_idx; Type: INDEX; Schema: shop; Owner: -
--

CREATE INDEX trade_event_signature_idx ON shop.trade_event USING btree (hashed_signature, kind) WHERE (hashed_signature IS NOT NULL);


--
-- Name: treasury_drift_checks_checked_idx; Type: INDEX; Schema: shop; Owner: -
--

CREATE INDEX treasury_drift_checks_checked_idx ON shop.treasury_drift_checks USING btree (checked_at DESC);


--
-- Name: treasury_ledger_created_idx; Type: INDEX; Schema: shop; Owner: -
--

CREATE INDEX treasury_ledger_created_idx ON shop.treasury_ledger USING btree (created_at DESC);


--
-- Name: treasury_ledger_refill_window_idx; Type: INDEX; Schema: shop; Owner: -
--

CREATE INDEX treasury_ledger_refill_window_idx ON shop.treasury_ledger USING btree (created_at) WHERE (type = 'refill'::text);


--
-- Name: treasury_ledger_type_reference_uniq; Type: INDEX; Schema: shop; Owner: -
--

CREATE UNIQUE INDEX treasury_ledger_type_reference_uniq ON shop.treasury_ledger USING btree (type, reference) WHERE (reference IS NOT NULL);


--
-- Name: treasury_pending_refill_unresolved_idx; Type: INDEX; Schema: shop; Owner: -
--

CREATE INDEX treasury_pending_refill_unresolved_idx ON shop.treasury_pending_refill USING btree (created_at) WHERE (status = ANY (ARRAY['pending'::text, 'swap_sent'::text, 'swapped'::text, 'transfer_sent'::text, 'transferred'::text]));


--
-- Name: usage_grants_grantee_idx; Type: INDEX; Schema: shop; Owner: -
--

CREATE INDEX usage_grants_grantee_idx ON shop.usage_grants USING btree (grantee_address) WHERE (status = 'active'::text);


--
-- Name: usage_grants_natural_key; Type: INDEX; Schema: shop; Owner: -
--

CREATE UNIQUE INDEX usage_grants_natural_key ON shop.usage_grants USING btree (escrow_ref, urn, token_id) NULLS NOT DISTINCT WHERE (escrow_ref IS NOT NULL);


--
-- Name: usage_grants_urn_idx; Type: INDEX; Schema: shop; Owner: -
--

CREATE INDEX usage_grants_urn_idx ON shop.usage_grants USING btree (urn);


--
-- Name: rental_listings_lessor_idx; Type: INDEX; Schema: signatures; Owner: -
--

CREATE INDEX rental_listings_lessor_idx ON signatures.rental_listings USING btree (lessor);


--
-- Name: rental_listings_tenant_idx; Type: INDEX; Schema: signatures; Owner: -
--

CREATE INDEX rental_listings_tenant_idx ON signatures.rental_listings USING btree (tenant) WHERE (tenant IS NOT NULL);


--
-- Name: rental_periods_rental_idx; Type: INDEX; Schema: signatures; Owner: -
--

CREATE INDEX rental_periods_rental_idx ON signatures.rental_periods USING btree (rental_id);


--
-- Name: rentals_asset_idx; Type: INDEX; Schema: signatures; Owner: -
--

CREATE INDEX rentals_asset_idx ON signatures.rentals USING btree (contract_address, token_id, created_at DESC);


--
-- Name: rentals_created_idx; Type: INDEX; Schema: signatures; Owner: -
--

CREATE INDEX rentals_created_idx ON signatures.rentals USING btree (created_at DESC);


--
-- Name: rentals_open_asset_key; Type: INDEX; Schema: signatures; Owner: -
--

CREATE UNIQUE INDEX rentals_open_asset_key ON signatures.rentals USING btree (contract_address, token_id) WHERE (status = 'open'::signatures.rental_status);


--
-- Name: blocks_blocked_idx; Type: INDEX; Schema: social; Owner: -
--

CREATE INDEX blocks_blocked_idx ON social.blocks USING btree (blocked_address);


--
-- Name: communities_owner_idx; Type: INDEX; Schema: social; Owner: -
--

CREATE INDEX communities_owner_idx ON social.communities USING btree (owner_address);


--
-- Name: communities_ranking_idx; Type: INDEX; Schema: social; Owner: -
--

CREATE INDEX communities_ranking_idx ON social.communities USING btree (editors_choice DESC, ranking_score DESC, name) WHERE (active AND (NOT unlisted) AND (NOT suspended));


--
-- Name: communities_voice_notifications_idx; Type: INDEX; Schema: social; Owner: -
--

CREATE INDEX communities_voice_notifications_idx ON social.communities USING btree (id) WHERE (voice_notification_generation IS NOT NULL);


--
-- Name: community_bans_active_idx; Type: INDEX; Schema: social; Owner: -
--

CREATE INDEX community_bans_active_idx ON social.community_bans USING btree (community_id, banned_at) WHERE active;


--
-- Name: community_compliance_cache_expiry_idx; Type: INDEX; Schema: social; Owner: -
--

CREATE INDEX community_compliance_cache_expiry_idx ON social.community_compliance_cache USING btree (expires_at);


--
-- Name: community_members_member_idx; Type: INDEX; Schema: social; Owner: -
--

CREATE INDEX community_members_member_idx ON social.community_members USING btree (member_address, community_id);


--
-- Name: community_members_owner_uidx; Type: INDEX; Schema: social; Owner: -
--

CREATE UNIQUE INDEX community_members_owner_uidx ON social.community_members USING btree (community_id) WHERE (role = 'owner'::text);


--
-- Name: community_places_community_idx; Type: INDEX; Schema: social; Owner: -
--

CREATE INDEX community_places_community_idx ON social.community_places USING btree (community_id, added_at DESC);


--
-- Name: community_posts_feed_idx; Type: INDEX; Schema: social; Owner: -
--

CREATE INDEX community_posts_feed_idx ON social.community_posts USING btree (community_id, created_at DESC, id DESC);


--
-- Name: community_requests_community_idx; Type: INDEX; Schema: social; Owner: -
--

CREATE INDEX community_requests_community_idx ON social.community_requests USING btree (community_id, status, created_at DESC);


--
-- Name: community_requests_member_idx; Type: INDEX; Schema: social; Owner: -
--

CREATE INDEX community_requests_member_idx ON social.community_requests USING btree (member_address, status, created_at DESC);


--
-- Name: community_requests_pending_uidx; Type: INDEX; Schema: social; Owner: -
--

CREATE UNIQUE INDEX community_requests_pending_uidx ON social.community_requests USING btree (community_id, member_address) WHERE (status = 'pending'::text);


--
-- Name: community_seen_nonces_expires_idx; Type: INDEX; Schema: social; Owner: -
--

CREATE INDEX community_seen_nonces_expires_idx ON social.community_seen_nonces USING btree (expires_at);


--
-- Name: community_signed_actions_content_idx; Type: INDEX; Schema: social; Owner: -
--

CREATE INDEX community_signed_actions_content_idx ON social.community_signed_actions USING btree (content_hash) WHERE (content_hash IS NOT NULL);


--
-- Name: community_signed_actions_create_uidx; Type: INDEX; Schema: social; Owner: -
--

CREATE UNIQUE INDEX community_signed_actions_create_uidx ON social.community_signed_actions USING btree (community_uuid) WHERE (kind = 'create'::text);


--
-- Name: community_signed_actions_kind_seq_idx; Type: INDEX; Schema: social; Owner: -
--

CREATE INDEX community_signed_actions_kind_seq_idx ON social.community_signed_actions USING btree (kind, seq);


--
-- Name: community_signed_actions_member_idx; Type: INDEX; Schema: social; Owner: -
--

CREATE INDEX community_signed_actions_member_idx ON social.community_signed_actions USING btree (community_uuid, target, signed_at, signature_hash) WHERE (role IS NOT NULL);


--
-- Name: community_voice_chat_users_status_idx; Type: INDEX; Schema: social; Owner: -
--

CREATE INDEX community_voice_chat_users_status_idx ON social.community_voice_chat_users USING btree (status, status_updated_at);


--
-- Name: event_notification_intents_pending_idx; Type: INDEX; Schema: social; Owner: -
--

CREATE INDEX event_notification_intents_pending_idx ON social.event_notification_intents USING btree (dispatch_at, id) WHERE (processed_at IS NULL);


--
-- Name: event_notification_occurrence_idx; Type: INDEX; Schema: social; Owner: -
--

CREATE UNIQUE INDEX event_notification_occurrence_idx ON social.event_notification_intents USING btree (event_id, occurrence_start, kind) WHERE (occurrence_start IS NOT NULL);


--
-- Name: event_notification_retention_idx; Type: INDEX; Schema: social; Owner: -
--

CREATE INDEX event_notification_retention_idx ON social.event_notification_intents USING btree (processed_at, id) WHERE (processed_at IS NOT NULL);


--
-- Name: event_payloads_expiry_idx; Type: INDEX; Schema: social; Owner: -
--

CREATE INDEX event_payloads_expiry_idx ON social.event_payloads USING btree (expires_at, id);


--
-- Name: friend_messages_pair_idx; Type: INDEX; Schema: social; Owner: -
--

CREATE INDEX friend_messages_pair_idx ON social.friend_messages USING btree (LEAST(sender_address, recipient_address), GREATEST(sender_address, recipient_address), created_at DESC, id DESC);


--
-- Name: friendship_actions_friendship_time_idx; Type: INDEX; Schema: social; Owner: -
--

CREATE INDEX friendship_actions_friendship_time_idx ON social.friendship_actions USING btree (friendship_id, "timestamp" DESC, id DESC);


--
-- Name: friendship_mutation_limits_expiry_idx; Type: INDEX; Schema: social; Owner: -
--

CREATE INDEX friendship_mutation_limits_expiry_idx ON social.friendship_mutation_limits USING btree (expires_at_ms, key) WHERE (expires_at_ms IS NOT NULL);


--
-- Name: friendships_pending_requested_idx; Type: INDEX; Schema: social; Owner: -
--

CREATE INDEX friendships_pending_requested_idx ON social.friendships USING btree (address_requested, last_action_at DESC, id DESC) WHERE (last_action = 'request'::text);


--
-- Name: friendships_pending_requester_idx; Type: INDEX; Schema: social; Owner: -
--

CREATE INDEX friendships_pending_requester_idx ON social.friendships USING btree (address_requester, last_action_at DESC, id DESC) WHERE (last_action = 'request'::text);


--
-- Name: friendships_requested_idx; Type: INDEX; Schema: social; Owner: -
--

CREATE INDEX friendships_requested_idx ON social.friendships USING btree (address_requested);


--
-- Name: friendships_requester_idx; Type: INDEX; Schema: social; Owner: -
--

CREATE INDEX friendships_requester_idx ON social.friendships USING btree (address_requester);


--
-- Name: friendships_unordered_pair_uidx; Type: INDEX; Schema: social; Owner: -
--

CREATE UNIQUE INDEX friendships_unordered_pair_uidx ON social.friendships USING btree (LEAST(address_requester, address_requested), GREATEST(address_requester, address_requested));


--
-- Name: island_cleanup_dispatches_active_wallet_idx; Type: INDEX; Schema: social; Owner: -
--

CREATE UNIQUE INDEX island_cleanup_dispatches_active_wallet_idx ON social.island_cleanup_dispatches USING btree (wallet) WHERE (state = ANY (ARRAY['dispatched'::text, 'quarantine'::text, 'settled_late'::text]));


--
-- Name: island_cleanup_dispatches_enforcement_idx; Type: INDEX; Schema: social; Owner: -
--

CREATE INDEX island_cleanup_dispatches_enforcement_idx ON social.island_cleanup_dispatches USING btree (intent_root_key, intent_target_key, intent_generation, fence_generation DESC) WHERE (purpose = 'island_enforcement'::text);


--
-- Name: island_cleanup_dispatches_takeover_idx; Type: INDEX; Schema: social; Owner: -
--

CREATE INDEX island_cleanup_dispatches_takeover_idx ON social.island_cleanup_dispatches USING btree (wallet, pulse_source, pulse_takeover_id, fence_generation DESC) WHERE (purpose = 'pulse_takeover'::text);


--
-- Name: island_takeover_receipts_due_idx; Type: INDEX; Schema: social; Owner: -
--

CREATE INDEX island_takeover_receipts_due_idx ON social.island_takeover_receipts USING btree (next_attempt_at, wallet, source_incarnation, takeover_id) WHERE (NOT acknowledged);


--
-- Name: livekit_removal_intents_due_idx; Type: INDEX; Schema: social; Owner: -
--

CREATE INDEX livekit_removal_intents_due_idx ON social.livekit_removal_intents USING btree (next_attempt_at, created_at, root_key, target_key);


--
-- Name: livekit_removal_intents_room_idx; Type: INDEX; Schema: social; Owner: -
--

CREATE INDEX livekit_removal_intents_room_idx ON social.livekit_removal_intents USING btree (room_name, room_sid) WHERE (room_name IS NOT NULL);


--
-- Name: mls_key_packages_owner_unconsumed_idx; Type: INDEX; Schema: social; Owner: -
--

CREATE INDEX mls_key_packages_owner_unconsumed_idx ON social.mls_key_packages USING btree (owner, created_at) WHERE (consumed_at IS NULL);


--
-- Name: mls_message_refs_ciphertext_idx; Type: INDEX; Schema: social; Owner: -
--

CREATE INDEX mls_message_refs_ciphertext_idx ON social.mls_message_refs USING btree (ciphertext_hash);


--
-- Name: mls_message_refs_group_time_idx; Type: INDEX; Schema: social; Owner: -
--

CREATE INDEX mls_message_refs_group_time_idx ON social.mls_message_refs USING btree (group_id, received_at DESC);


--
-- Name: notification_deliveries_due_idx; Type: INDEX; Schema: social; Owner: -
--

CREATE INDEX notification_deliveries_due_idx ON social.notification_deliveries USING btree (next_attempt_at, notification_id) WHERE (state = 'pending'::text);


--
-- Name: notifications_address_time_idx; Type: INDEX; Schema: social; Owner: -
--

CREATE INDEX notifications_address_time_idx ON social.notifications USING btree (address, occurred_at DESC, id DESC);


--
-- Name: notifications_broadcast_uidx; Type: INDEX; Schema: social; Owner: -
--

CREATE UNIQUE INDEX notifications_broadcast_uidx ON social.notifications USING btree (address, broadcast_key) WHERE (broadcast_key IS NOT NULL);


--
-- Name: notifications_unread_idx; Type: INDEX; Schema: social; Owner: -
--

CREATE INDEX notifications_unread_idx ON social.notifications USING btree (address, occurred_at DESC, id DESC) WHERE (NOT read);


--
-- Name: player_report_evidence_created_idx; Type: INDEX; Schema: social; Owner: -
--

CREATE INDEX player_report_evidence_created_idx ON social.player_report_evidence USING btree (created_at);


--
-- Name: player_report_evidence_reporter_idx; Type: INDEX; Schema: social; Owner: -
--

CREATE INDEX player_report_evidence_reporter_idx ON social.player_report_evidence USING btree (reporter_address, created_at);


--
-- Name: player_reports_created_idx; Type: INDEX; Schema: social; Owner: -
--

CREATE INDEX player_reports_created_idx ON social.player_reports USING btree (created_at DESC);


--
-- Name: player_reports_reported_idx; Type: INDEX; Schema: social; Owner: -
--

CREATE INDEX player_reports_reported_idx ON social.player_reports USING btree (reported_address, created_at DESC);


--
-- Name: player_reports_reporter_idx; Type: INDEX; Schema: social; Owner: -
--

CREATE INDEX player_reports_reporter_idx ON social.player_reports USING btree (reporter_address, created_at);


--
-- Name: player_reports_status_idx; Type: INDEX; Schema: social; Owner: -
--

CREATE INDEX player_reports_status_idx ON social.player_reports USING btree (status, created_at DESC);


--
-- Name: private_voice_chat_participants_status_idx; Type: INDEX; Schema: social; Owner: -
--

CREATE INDEX private_voice_chat_participants_status_idx ON social.private_voice_chat_participants USING btree (status, status_updated_at);


--
-- Name: private_voice_chats_created_idx; Type: INDEX; Schema: social; Owner: -
--

CREATE INDEX private_voice_chats_created_idx ON social.private_voice_chats USING btree (created_at);


--
-- Name: referral_emails_referrer_idx; Type: INDEX; Schema: social; Owner: -
--

CREATE INDEX referral_emails_referrer_idx ON social.referral_emails USING btree (referrer, updated_at DESC);


--
-- Name: referral_progress_granted_idx; Type: INDEX; Schema: social; Owner: -
--

CREATE INDEX referral_progress_granted_idx ON social.referral_progress USING btree (referrer) WHERE tier_granted;


--
-- Name: referral_progress_ip_idx; Type: INDEX; Schema: social; Owner: -
--

CREATE INDEX referral_progress_ip_idx ON social.referral_progress USING btree (referrer, invited_user_ip);


--
-- Name: referral_progress_referrer_idx; Type: INDEX; Schema: social; Owner: -
--

CREATE INDEX referral_progress_referrer_idx ON social.referral_progress USING btree (referrer, status);


--
-- Name: referral_reward_images_referrer_idx; Type: INDEX; Schema: social; Owner: -
--

CREATE INDEX referral_reward_images_referrer_idx ON social.referral_reward_images USING btree (referrer);


--
-- Name: registry_profile_cache_expiry_idx; Type: INDEX; Schema: social; Owner: -
--

CREATE INDEX registry_profile_cache_expiry_idx ON social.registry_profile_cache USING btree (expires_at, profile_key);


--
-- Name: scene_bans_place_time_idx; Type: INDEX; Schema: social; Owner: -
--

CREATE INDEX scene_bans_place_time_idx ON social.scene_bans USING btree (place_id, banned_at DESC);


--
-- Name: scene_stream_access_active_ingress_idx; Type: INDEX; Schema: social; Owner: -
--

CREATE INDEX scene_stream_access_active_ingress_idx ON social.scene_stream_access USING btree (ingress_id) WHERE active;


--
-- Name: scene_stream_access_active_place_key; Type: INDEX; Schema: social; Owner: -
--

CREATE UNIQUE INDEX scene_stream_access_active_place_key ON social.scene_stream_access USING btree (place_id) WHERE active;


--
-- Name: scene_stream_access_key_expiry_idx; Type: INDEX; Schema: social; Owner: -
--

CREATE INDEX scene_stream_access_key_expiry_idx ON social.scene_stream_access USING btree (COALESCE(expiration_time, (created_at + '4 days'::interval)), id) WHERE active;


--
-- Name: scene_stream_access_reconcile_idx; Type: INDEX; Schema: social; Owner: -
--

CREATE INDEX scene_stream_access_reconcile_idx ON social.scene_stream_access USING btree (streaming_checked_at NULLS FIRST, id) WHERE (active AND streaming);


--
-- Name: scene_stream_access_stream_start_idx; Type: INDEX; Schema: social; Owner: -
--

CREATE INDEX scene_stream_access_stream_start_idx ON social.scene_stream_access USING btree (streaming_start_time, id) WHERE (active AND streaming);


--
-- Name: scene_stream_cleanup_due_idx; Type: INDEX; Schema: social; Owner: -
--

CREATE INDEX scene_stream_cleanup_due_idx ON social.scene_stream_cleanup USING btree (due_at, access_id);


--
-- Name: subscriptions_email_idx; Type: INDEX; Schema: social; Owner: -
--

CREATE INDEX subscriptions_email_idx ON social.subscriptions USING btree (lower(email)) WHERE (email IS NOT NULL);


--
-- Name: user_bans_open_by_address_idx; Type: INDEX; Schema: social; Owner: -
--

CREATE INDEX user_bans_open_by_address_idx ON social.user_bans USING btree (banned_address, banned_at DESC) WHERE (lifted_at IS NULL);


--
-- Name: user_bans_open_by_device_idx; Type: INDEX; Schema: social; Owner: -
--

CREATE INDEX user_bans_open_by_device_idx ON social.user_bans USING btree (banned_device_id) WHERE ((lifted_at IS NULL) AND (banned_device_id IS NOT NULL));


--
-- Name: user_warnings_address_idx; Type: INDEX; Schema: social; Owner: -
--

CREATE INDEX user_warnings_address_idx ON social.user_warnings USING btree (warned_address, warned_at DESC);


--
-- Name: events_client_error_received_idx; Type: INDEX; Schema: telemetry; Owner: -
--

CREATE INDEX events_client_error_received_idx ON telemetry.events USING btree (received_at) WHERE ((source = 'segment'::text) AND (event_kind = 'track'::text) AND ((body ->> 'event'::text) = 'client_error'::text));


--
-- Name: events_experiments_idx; Type: INDEX; Schema: telemetry; Owner: -
--

CREATE INDEX events_experiments_idx ON telemetry.events USING btree (source, (((body -> 'properties'::text) ->> 'exp_key'::text)), (((body -> 'properties'::text) ->> 'variant'::text)), ((body ->> 'event'::text))) WHERE ((source = 'segment'::text) AND (((body -> 'properties'::text) ->> 'exp_key'::text) IS NOT NULL));


--
-- Name: events_fingerprint_received_idx; Type: INDEX; Schema: telemetry; Owner: -
--

CREATE INDEX events_fingerprint_received_idx ON telemetry.events USING btree (fingerprint, received_at DESC);


--
-- Name: events_level_received_idx; Type: INDEX; Schema: telemetry; Owner: -
--

CREATE INDEX events_level_received_idx ON telemetry.events USING btree (((body ->> 'level'::text)), received_at DESC);


--
-- Name: events_received_at_idx; Type: INDEX; Schema: telemetry; Owner: -
--

CREATE INDEX events_received_at_idx ON telemetry.events USING btree (received_at DESC);


--
-- Name: events_scene_errors_genesis_idx; Type: INDEX; Schema: telemetry; Owner: -
--

CREATE INDEX events_scene_errors_genesis_idx ON telemetry.events USING btree ((((body -> 'properties'::text) ->> 'scene_base'::text)), received_at DESC) WHERE ((source = 'segment'::text) AND ((body ->> 'event'::text) = 'Scene Error'::text));


--
-- Name: events_scene_errors_world_idx; Type: INDEX; Schema: telemetry; Owner: -
--

CREATE INDEX events_scene_errors_world_idx ON telemetry.events USING btree (lower(((body -> 'properties'::text) ->> 'scene_world'::text)), received_at DESC) WHERE ((source = 'segment'::text) AND ((body ->> 'event'::text) = 'Scene Error'::text));


--
-- Name: events_source_kind_received_idx; Type: INDEX; Schema: telemetry; Owner: -
--

CREATE INDEX events_source_kind_received_idx ON telemetry.events USING btree (source, event_kind, received_at DESC);


--
-- Name: issue_merges_canonical_idx; Type: INDEX; Schema: telemetry; Owner: -
--

CREATE INDEX issue_merges_canonical_idx ON telemetry.issue_merges USING btree (canonical_fingerprint);


--
-- Name: env_variables_size_idx; Type: INDEX; Schema: worlds; Owner: -
--

CREATE INDEX env_variables_size_idx ON worlds.env_variables USING btree (realm, scene_namespace) INCLUDE (key, value_size);


--
-- Name: forward_outbox_due_idx; Type: INDEX; Schema: worlds; Owner: -
--

CREATE INDEX forward_outbox_due_idx ON worlds.forward_outbox USING btree (target, next_attempt_at, id) WHERE (status = ANY (ARRAY['queued'::text, 'sending'::text]));


--
-- Name: player_storage_size_idx; Type: INDEX; Schema: worlds; Owner: -
--

CREATE INDEX player_storage_size_idx ON worlds.player_storage USING btree (realm, player_address, scene_namespace) INCLUDE (key, value_size);


--
-- Name: world_access_communities_community_idx; Type: INDEX; Schema: worlds; Owner: -
--

CREATE INDEX world_access_communities_community_idx ON worlds.world_access_communities USING btree (community_id);


--
-- Name: world_access_log_address_idx; Type: INDEX; Schema: worlds; Owner: -
--

CREATE INDEX world_access_log_address_idx ON worlds.world_access_log USING btree (address, created_at DESC, id DESC);


--
-- Name: world_access_log_created_idx; Type: INDEX; Schema: worlds; Owner: -
--

CREATE INDEX world_access_log_created_idx ON worlds.world_access_log USING btree (created_at DESC, id DESC);


--
-- Name: world_access_log_world_idx; Type: INDEX; Schema: worlds; Owner: -
--

CREATE INDEX world_access_log_world_idx ON worlds.world_access_log USING btree (world_name, created_at DESC, id DESC);


--
-- Name: world_scene_identity_world_active_idx; Type: INDEX; Schema: worlds; Owner: -
--

CREATE INDEX world_scene_identity_world_active_idx ON worlds.world_scene_identity USING btree (world_name) WHERE active;


--
-- Name: world_scenes_logs_permissions_idx; Type: INDEX; Schema: worlds; Owner: -
--

CREATE INDEX world_scenes_logs_permissions_idx ON worlds.world_scenes USING gin (worlds.logs_permissions(((entity #> '{metadata,logsPermissions}'::text[]))::text)) WHERE (mirror_source IS NULL);


--
-- Name: world_scenes_world_name_created_idx; Type: INDEX; Schema: worlds; Owner: -
--

CREATE INDEX world_scenes_world_name_created_idx ON worlds.world_scenes USING btree (world_name, created_at DESC);


--
-- Name: world_storage_size_idx; Type: INDEX; Schema: worlds; Owner: -
--

CREATE INDEX world_storage_size_idx ON worlds.world_storage USING btree (realm, scene_namespace) INCLUDE (key, value_size);


--
-- Name: deployments deployment_notify_trigger; Type: TRIGGER; Schema: content; Owner: -
--

CREATE TRIGGER deployment_notify_trigger AFTER INSERT ON content.deployments FOR EACH ROW EXECUTE FUNCTION content.notify_new_deployment();


--
-- Name: place_world_local local_world_removal_capture; Type: TRIGGER; Schema: explore; Owner: -
--

CREATE TRIGGER local_world_removal_capture AFTER INSERT OR DELETE OR UPDATE ON explore.place_world_local FOR EACH ROW EXECUTE FUNCTION explore.capture_place_removal();


--
-- Name: place place_removal_capture; Type: TRIGGER; Schema: explore; Owner: -
--

CREATE TRIGGER place_removal_capture AFTER INSERT OR DELETE OR UPDATE ON explore.place FOR EACH ROW EXECUTE FUNCTION explore.capture_place_removal();


--
-- Name: asset_contents asset_contents_asset_id_fkey; Type: FK CONSTRAINT; Schema: builder; Owner: -
--

ALTER TABLE ONLY builder.asset_contents
    ADD CONSTRAINT asset_contents_asset_id_fkey FOREIGN KEY (asset_id) REFERENCES builder.assets(id) ON DELETE CASCADE;


--
-- Name: asset_contents asset_contents_hash_fkey; Type: FK CONSTRAINT; Schema: builder; Owner: -
--

ALTER TABLE ONLY builder.asset_contents
    ADD CONSTRAINT asset_contents_hash_fkey FOREIGN KEY (hash) REFERENCES builder.uploaded_contents(hash);


--
-- Name: asset_packs asset_packs_project_id_fkey; Type: FK CONSTRAINT; Schema: builder; Owner: -
--

ALTER TABLE ONLY builder.asset_packs
    ADD CONSTRAINT asset_packs_project_id_fkey FOREIGN KEY (project_id) REFERENCES builder.projects(id) ON DELETE CASCADE;


--
-- Name: asset_packs asset_packs_thumbnail_fkey; Type: FK CONSTRAINT; Schema: builder; Owner: -
--

ALTER TABLE ONLY builder.asset_packs
    ADD CONSTRAINT asset_packs_thumbnail_fkey FOREIGN KEY (thumbnail) REFERENCES builder.uploaded_contents(hash);


--
-- Name: assets assets_asset_pack_id_fkey; Type: FK CONSTRAINT; Schema: builder; Owner: -
--

ALTER TABLE ONLY builder.assets
    ADD CONSTRAINT assets_asset_pack_id_fkey FOREIGN KEY (asset_pack_id) REFERENCES builder.asset_packs(id) ON DELETE CASCADE;


--
-- Name: assets assets_thumbnail_fkey; Type: FK CONSTRAINT; Schema: builder; Owner: -
--

ALTER TABLE ONLY builder.assets
    ADD CONSTRAINT assets_thumbnail_fkey FOREIGN KEY (thumbnail) REFERENCES builder.uploaded_contents(hash);


--
-- Name: collection_publications collection_publications_collection_id_fkey; Type: FK CONSTRAINT; Schema: builder; Owner: -
--

ALTER TABLE ONLY builder.collection_publications
    ADD CONSTRAINT collection_publications_collection_id_fkey FOREIGN KEY (collection_id) REFERENCES builder.collections(id) ON DELETE CASCADE;


--
-- Name: item_contents item_contents_hash_fkey; Type: FK CONSTRAINT; Schema: builder; Owner: -
--

ALTER TABLE ONLY builder.item_contents
    ADD CONSTRAINT item_contents_hash_fkey FOREIGN KEY (hash) REFERENCES builder.uploaded_contents(hash);


--
-- Name: item_contents item_contents_item_id_fkey; Type: FK CONSTRAINT; Schema: builder; Owner: -
--

ALTER TABLE ONLY builder.item_contents
    ADD CONSTRAINT item_contents_item_id_fkey FOREIGN KEY (item_id) REFERENCES builder.items(id) ON DELETE CASCADE;


--
-- Name: items items_collection_id_fkey; Type: FK CONSTRAINT; Schema: builder; Owner: -
--

ALTER TABLE ONLY builder.items
    ADD CONSTRAINT items_collection_id_fkey FOREIGN KEY (collection_id) REFERENCES builder.collections(id) ON DELETE CASCADE;


--
-- Name: linked_collection_publications linked_collection_publications_collection_id_fkey; Type: FK CONSTRAINT; Schema: builder; Owner: -
--

ALTER TABLE ONLY builder.linked_collection_publications
    ADD CONSTRAINT linked_collection_publications_collection_id_fkey FOREIGN KEY (collection_id) REFERENCES builder.collections(id) ON DELETE CASCADE;


--
-- Name: project_files project_files_hash_fkey; Type: FK CONSTRAINT; Schema: builder; Owner: -
--

ALTER TABLE ONLY builder.project_files
    ADD CONSTRAINT project_files_hash_fkey FOREIGN KEY (hash) REFERENCES builder.uploaded_contents(hash);


--
-- Name: project_files project_files_project_id_fkey; Type: FK CONSTRAINT; Schema: builder; Owner: -
--

ALTER TABLE ONLY builder.project_files
    ADD CONSTRAINT project_files_project_id_fkey FOREIGN KEY (project_id) REFERENCES builder.projects(id) ON DELETE CASCADE;


--
-- Name: project_manifests project_manifests_project_id_fkey; Type: FK CONSTRAINT; Schema: builder; Owner: -
--

ALTER TABLE ONLY builder.project_manifests
    ADD CONSTRAINT project_manifests_project_id_fkey FOREIGN KEY (project_id) REFERENCES builder.projects(id) ON DELETE CASCADE;


--
-- Name: projects projects_thumbnail_fkey; Type: FK CONSTRAINT; Schema: builder; Owner: -
--

ALTER TABLE ONLY builder.projects
    ADD CONSTRAINT projects_thumbnail_fkey FOREIGN KEY (thumbnail) REFERENCES builder.uploaded_contents(hash);


--
-- Name: messages messages_channel_fkey; Type: FK CONSTRAINT; Schema: chat; Owner: -
--

ALTER TABLE ONLY chat.messages
    ADD CONSTRAINT messages_channel_fkey FOREIGN KEY (community_id, channel) REFERENCES chat.channels(community_id, name) ON DELETE CASCADE;


--
-- Name: pins pins_message_id_fkey; Type: FK CONSTRAINT; Schema: chat; Owner: -
--

ALTER TABLE ONLY chat.pins
    ADD CONSTRAINT pins_message_id_fkey FOREIGN KEY (message_id) REFERENCES chat.messages(id) ON DELETE CASCADE;


--
-- Name: reactions reactions_message_id_fkey; Type: FK CONSTRAINT; Schema: chat; Owner: -
--

ALTER TABLE ONLY chat.reactions
    ADD CONSTRAINT reactions_message_id_fkey FOREIGN KEY (message_id) REFERENCES chat.messages(id) ON DELETE CASCADE;


--
-- Name: replies replies_parent_id_fkey; Type: FK CONSTRAINT; Schema: chat; Owner: -
--

ALTER TABLE ONLY chat.replies
    ADD CONSTRAINT replies_parent_id_fkey FOREIGN KEY (parent_id) REFERENCES chat.messages(id) ON DELETE CASCADE;


--
-- Name: reports reports_message_id_fkey; Type: FK CONSTRAINT; Schema: chat; Owner: -
--

ALTER TABLE ONLY chat.reports
    ADD CONSTRAINT reports_message_id_fkey FOREIGN KEY (message_id) REFERENCES chat.messages(id) ON DELETE CASCADE;


--
-- Name: authorization_lines authorization_lines_authorization_fkey; Type: FK CONSTRAINT; Schema: credits; Owner: -
--

ALTER TABLE ONLY credits.authorization_lines
    ADD CONSTRAINT authorization_lines_authorization_fkey FOREIGN KEY (authorization_id) REFERENCES credits.authorizations(id) ON DELETE CASCADE;


--
-- Name: cart_items cart_items_cart_fkey; Type: FK CONSTRAINT; Schema: credits; Owner: -
--

ALTER TABLE ONLY credits.cart_items
    ADD CONSTRAINT cart_items_cart_fkey FOREIGN KEY (cart_id) REFERENCES credits.carts(id) ON DELETE CASCADE;


--
-- Name: fulfillment_outbox fulfillment_outbox_checkout_fkey; Type: FK CONSTRAINT; Schema: credits; Owner: -
--

ALTER TABLE ONLY credits.fulfillment_outbox
    ADD CONSTRAINT fulfillment_outbox_checkout_fkey FOREIGN KEY (checkout_id) REFERENCES credits.checkouts(id);


--
-- Name: discourse_posts discourse_posts_topic_fkey; Type: FK CONSTRAINT; Schema: governance; Owner: -
--

ALTER TABLE ONLY governance.discourse_posts
    ADD CONSTRAINT discourse_posts_topic_fkey FOREIGN KEY (topic_id) REFERENCES governance.discourse_topics(topic_id) ON DELETE CASCADE;


--
-- Name: hot_scene_snapshots hot_scene_snapshots_snapshot_id_fkey; Type: FK CONSTRAINT; Schema: presence; Owner: -
--

ALTER TABLE ONLY presence.hot_scene_snapshots
    ADD CONSTRAINT hot_scene_snapshots_snapshot_id_fkey FOREIGN KEY (snapshot_id) REFERENCES presence.snapshots(id) ON DELETE CASCADE;


--
-- Name: island_snapshots island_snapshots_snapshot_id_fkey; Type: FK CONSTRAINT; Schema: presence; Owner: -
--

ALTER TABLE ONLY presence.island_snapshots
    ADD CONSTRAINT island_snapshots_snapshot_id_fkey FOREIGN KEY (snapshot_id) REFERENCES presence.snapshots(id) ON DELETE CASCADE;


--
-- Name: peer_snapshots peer_snapshots_snapshot_id_fkey; Type: FK CONSTRAINT; Schema: presence; Owner: -
--

ALTER TABLE ONLY presence.peer_snapshots
    ADD CONSTRAINT peer_snapshots_snapshot_id_fkey FOREIGN KEY (snapshot_id) REFERENCES presence.snapshots(id) ON DELETE CASCADE;


--
-- Name: scene_occupancy scene_occupancy_snapshot_id_fkey; Type: FK CONSTRAINT; Schema: presence; Owner: -
--

ALTER TABLE ONLY presence.scene_occupancy
    ADD CONSTRAINT scene_occupancy_snapshot_id_fkey FOREIGN KEY (snapshot_id) REFERENCES presence.snapshots(id) ON DELETE CASCADE;


--
-- Name: world_membership world_membership_snapshot_id_fkey; Type: FK CONSTRAINT; Schema: presence; Owner: -
--

ALTER TABLE ONLY presence.world_membership
    ADD CONSTRAINT world_membership_snapshot_id_fkey FOREIGN KEY (snapshot_id) REFERENCES presence.snapshots(id) ON DELETE CASCADE;


--
-- Name: quest_events quest_events_instance_fkey; Type: FK CONSTRAINT; Schema: quests; Owner: -
--

ALTER TABLE ONLY quests.quest_events
    ADD CONSTRAINT quest_events_instance_fkey FOREIGN KEY (quest_instance_id) REFERENCES quests.quest_instances(id);


--
-- Name: quest_instances quest_instances_quest_fkey; Type: FK CONSTRAINT; Schema: quests; Owner: -
--

ALTER TABLE ONLY quests.quest_instances
    ADD CONSTRAINT quest_instances_quest_fkey FOREIGN KEY (quest_id) REFERENCES quests.quests(id);


--
-- Name: quest_reward_items quest_reward_items_quest_fkey; Type: FK CONSTRAINT; Schema: quests; Owner: -
--

ALTER TABLE ONLY quests.quest_reward_items
    ADD CONSTRAINT quest_reward_items_quest_fkey FOREIGN KEY (quest_id) REFERENCES quests.quests(id) ON DELETE CASCADE;


--
-- Name: quests quests_previous_fkey; Type: FK CONSTRAINT; Schema: quests; Owner: -
--

ALTER TABLE ONLY quests.quests
    ADD CONSTRAINT quests_previous_fkey FOREIGN KEY (previous_quest_id) REFERENCES quests.quests(id);


--
-- Name: coupon_state coupon_state_coupon_id_fkey; Type: FK CONSTRAINT; Schema: shop; Owner: -
--

ALTER TABLE ONLY shop.coupon_state
    ADD CONSTRAINT coupon_state_coupon_id_fkey FOREIGN KEY (coupon_id) REFERENCES shop.coupons(id) ON DELETE CASCADE;


--
-- Name: item_notify_outbox item_notify_outbox_request_id_fkey; Type: FK CONSTRAINT; Schema: shop; Owner: -
--

ALTER TABLE ONLY shop.item_notify_outbox
    ADD CONSTRAINT item_notify_outbox_request_id_fkey FOREIGN KEY (request_id) REFERENCES shop.item_notify_requests(id) ON DELETE CASCADE;


--
-- Name: list_acl list_acl_list_id_fkey; Type: FK CONSTRAINT; Schema: shop; Owner: -
--

ALTER TABLE ONLY shop.list_acl
    ADD CONSTRAINT list_acl_list_id_fkey FOREIGN KEY (list_id) REFERENCES shop.lists(id) ON DELETE CASCADE;


--
-- Name: picks picks_list_id_fkey; Type: FK CONSTRAINT; Schema: shop; Owner: -
--

ALTER TABLE ONLY shop.picks
    ADD CONSTRAINT picks_list_id_fkey FOREIGN KEY (list_id) REFERENCES shop.lists(id) ON DELETE CASCADE;


--
-- Name: trade_assets_erc20 trade_assets_erc20_asset_id_fkey; Type: FK CONSTRAINT; Schema: shop; Owner: -
--

ALTER TABLE ONLY shop.trade_assets_erc20
    ADD CONSTRAINT trade_assets_erc20_asset_id_fkey FOREIGN KEY (asset_id) REFERENCES shop.trade_assets(id) ON DELETE CASCADE;


--
-- Name: trade_assets_erc721 trade_assets_erc721_asset_id_fkey; Type: FK CONSTRAINT; Schema: shop; Owner: -
--

ALTER TABLE ONLY shop.trade_assets_erc721
    ADD CONSTRAINT trade_assets_erc721_asset_id_fkey FOREIGN KEY (asset_id) REFERENCES shop.trade_assets(id) ON DELETE CASCADE;


--
-- Name: trade_assets_item trade_assets_item_asset_id_fkey; Type: FK CONSTRAINT; Schema: shop; Owner: -
--

ALTER TABLE ONLY shop.trade_assets_item
    ADD CONSTRAINT trade_assets_item_asset_id_fkey FOREIGN KEY (asset_id) REFERENCES shop.trade_assets(id) ON DELETE CASCADE;


--
-- Name: trade_assets trade_assets_trade_id_fkey; Type: FK CONSTRAINT; Schema: shop; Owner: -
--

ALTER TABLE ONLY shop.trade_assets
    ADD CONSTRAINT trade_assets_trade_id_fkey FOREIGN KEY (trade_id) REFERENCES shop.trades(id) ON DELETE CASCADE;


--
-- Name: trade_chain_state trade_chain_state_trade_id_fkey; Type: FK CONSTRAINT; Schema: shop; Owner: -
--

ALTER TABLE ONLY shop.trade_chain_state
    ADD CONSTRAINT trade_chain_state_trade_id_fkey FOREIGN KEY (trade_id) REFERENCES shop.trades(id) ON DELETE CASCADE;


--
-- Name: rental_listings rental_listings_id_fkey; Type: FK CONSTRAINT; Schema: signatures; Owner: -
--

ALTER TABLE ONLY signatures.rental_listings
    ADD CONSTRAINT rental_listings_id_fkey FOREIGN KEY (id) REFERENCES signatures.rentals(id) ON DELETE CASCADE;


--
-- Name: rental_periods rental_periods_rental_id_fkey; Type: FK CONSTRAINT; Schema: signatures; Owner: -
--

ALTER TABLE ONLY signatures.rental_periods
    ADD CONSTRAINT rental_periods_rental_id_fkey FOREIGN KEY (rental_id) REFERENCES signatures.rental_listings(id) ON DELETE CASCADE;


--
-- Name: rentals rentals_contract_address_token_id_fkey; Type: FK CONSTRAINT; Schema: signatures; Owner: -
--

ALTER TABLE ONLY signatures.rentals
    ADD CONSTRAINT rentals_contract_address_token_id_fkey FOREIGN KEY (contract_address, token_id) REFERENCES signatures.rental_assets(contract_address, token_id) ON DELETE CASCADE;


--
-- Name: badge_tiers badge_tiers_badge_id_fkey; Type: FK CONSTRAINT; Schema: social; Owner: -
--

ALTER TABLE ONLY social.badge_tiers
    ADD CONSTRAINT badge_tiers_badge_id_fkey FOREIGN KEY (badge_id) REFERENCES social.badge_definitions(id) ON DELETE CASCADE;


--
-- Name: community_bans community_bans_community_id_fkey; Type: FK CONSTRAINT; Schema: social; Owner: -
--

ALTER TABLE ONLY social.community_bans
    ADD CONSTRAINT community_bans_community_id_fkey FOREIGN KEY (community_id) REFERENCES social.communities(id) ON DELETE CASCADE;


--
-- Name: community_members community_members_community_id_fkey; Type: FK CONSTRAINT; Schema: social; Owner: -
--

ALTER TABLE ONLY social.community_members
    ADD CONSTRAINT community_members_community_id_fkey FOREIGN KEY (community_id) REFERENCES social.communities(id) ON DELETE CASCADE;


--
-- Name: community_mirror community_mirror_community_id_fkey; Type: FK CONSTRAINT; Schema: social; Owner: -
--

ALTER TABLE ONLY social.community_mirror
    ADD CONSTRAINT community_mirror_community_id_fkey FOREIGN KEY (community_id) REFERENCES social.communities(id) ON DELETE CASCADE;


--
-- Name: community_places community_places_community_id_fkey; Type: FK CONSTRAINT; Schema: social; Owner: -
--

ALTER TABLE ONLY social.community_places
    ADD CONSTRAINT community_places_community_id_fkey FOREIGN KEY (community_id) REFERENCES social.communities(id) ON DELETE CASCADE;


--
-- Name: community_post_likes community_post_likes_post_id_fkey; Type: FK CONSTRAINT; Schema: social; Owner: -
--

ALTER TABLE ONLY social.community_post_likes
    ADD CONSTRAINT community_post_likes_post_id_fkey FOREIGN KEY (post_id) REFERENCES social.community_posts(id) ON DELETE CASCADE;


--
-- Name: community_posts community_posts_community_id_fkey; Type: FK CONSTRAINT; Schema: social; Owner: -
--

ALTER TABLE ONLY social.community_posts
    ADD CONSTRAINT community_posts_community_id_fkey FOREIGN KEY (community_id) REFERENCES social.communities(id) ON DELETE CASCADE;


--
-- Name: community_ranking_events community_ranking_events_community_id_fkey; Type: FK CONSTRAINT; Schema: social; Owner: -
--

ALTER TABLE ONLY social.community_ranking_events
    ADD CONSTRAINT community_ranking_events_community_id_fkey FOREIGN KEY (community_id) REFERENCES social.communities(id) ON DELETE CASCADE;


--
-- Name: community_requests community_requests_community_id_fkey; Type: FK CONSTRAINT; Schema: social; Owner: -
--

ALTER TABLE ONLY social.community_requests
    ADD CONSTRAINT community_requests_community_id_fkey FOREIGN KEY (community_id) REFERENCES social.communities(id) ON DELETE CASCADE;


--
-- Name: community_signed_actions community_signed_actions_community_uuid_fkey; Type: FK CONSTRAINT; Schema: social; Owner: -
--

ALTER TABLE ONLY social.community_signed_actions
    ADD CONSTRAINT community_signed_actions_community_uuid_fkey FOREIGN KEY (community_uuid) REFERENCES social.communities(id) ON DELETE CASCADE;


--
-- Name: community_thumbnails community_thumbnails_community_id_fkey; Type: FK CONSTRAINT; Schema: social; Owner: -
--

ALTER TABLE ONLY social.community_thumbnails
    ADD CONSTRAINT community_thumbnails_community_id_fkey FOREIGN KEY (community_id) REFERENCES social.communities(id) ON DELETE CASCADE;


--
-- Name: community_voice_chat_users community_voice_chat_users_community_id_fkey; Type: FK CONSTRAINT; Schema: social; Owner: -
--

ALTER TABLE ONLY social.community_voice_chat_users
    ADD CONSTRAINT community_voice_chat_users_community_id_fkey FOREIGN KEY (community_id) REFERENCES social.communities(id) ON DELETE CASCADE;


--
-- Name: friendship_actions friendship_actions_friendship_id_fkey; Type: FK CONSTRAINT; Schema: social; Owner: -
--

ALTER TABLE ONLY social.friendship_actions
    ADD CONSTRAINT friendship_actions_friendship_id_fkey FOREIGN KEY (friendship_id) REFERENCES social.friendships(id) ON DELETE CASCADE;


--
-- Name: island_cleanup_dispatches island_cleanup_dispatches_wallet_fkey; Type: FK CONSTRAINT; Schema: social; Owner: -
--

ALTER TABLE ONLY social.island_cleanup_dispatches
    ADD CONSTRAINT island_cleanup_dispatches_wallet_fkey FOREIGN KEY (wallet) REFERENCES social.island_admission_fences(wallet);


--
-- Name: island_takeover_receipts island_takeover_receipts_wallet_fkey; Type: FK CONSTRAINT; Schema: social; Owner: -
--

ALTER TABLE ONLY social.island_takeover_receipts
    ADD CONSTRAINT island_takeover_receipts_wallet_fkey FOREIGN KEY (wallet) REFERENCES social.island_admission_fences(wallet);


--
-- Name: island_takeover_receipts island_takeover_receipts_wallet_latest_dispatch_fkey; Type: FK CONSTRAINT; Schema: social; Owner: -
--

ALTER TABLE ONLY social.island_takeover_receipts
    ADD CONSTRAINT island_takeover_receipts_wallet_latest_dispatch_fkey FOREIGN KEY (wallet, latest_dispatch) REFERENCES social.island_cleanup_dispatches(wallet, dispatch_id);


--
-- Name: mls_commits mls_commits_group_fkey; Type: FK CONSTRAINT; Schema: social; Owner: -
--

ALTER TABLE ONLY social.mls_commits
    ADD CONSTRAINT mls_commits_group_fkey FOREIGN KEY (group_id) REFERENCES social.mls_groups(group_id) ON DELETE CASCADE;


--
-- Name: mls_group_members mls_group_members_group_fkey; Type: FK CONSTRAINT; Schema: social; Owner: -
--

ALTER TABLE ONLY social.mls_group_members
    ADD CONSTRAINT mls_group_members_group_fkey FOREIGN KEY (group_id) REFERENCES social.mls_groups(group_id) ON DELETE CASCADE;


--
-- Name: mls_message_refs mls_message_refs_group_fkey; Type: FK CONSTRAINT; Schema: social; Owner: -
--

ALTER TABLE ONLY social.mls_message_refs
    ADD CONSTRAINT mls_message_refs_group_fkey FOREIGN KEY (group_id) REFERENCES social.mls_groups(group_id) ON DELETE CASCADE;


--
-- Name: notification_deliveries notification_deliveries_notification_id_fkey; Type: FK CONSTRAINT; Schema: social; Owner: -
--

ALTER TABLE ONLY social.notification_deliveries
    ADD CONSTRAINT notification_deliveries_notification_id_fkey FOREIGN KEY (notification_id) REFERENCES social.notifications(id) ON DELETE CASCADE;


--
-- Name: private_voice_chat_participants private_voice_chat_participants_call_id_fkey; Type: FK CONSTRAINT; Schema: social; Owner: -
--

ALTER TABLE ONLY social.private_voice_chat_participants
    ADD CONSTRAINT private_voice_chat_participants_call_id_fkey FOREIGN KEY (call_id) REFERENCES social.private_voice_chats(id) ON DELETE CASCADE;


--
-- Name: user_achieved_tiers user_achieved_tiers_badge_id_fkey; Type: FK CONSTRAINT; Schema: social; Owner: -
--

ALTER TABLE ONLY social.user_achieved_tiers
    ADD CONSTRAINT user_achieved_tiers_badge_id_fkey FOREIGN KEY (badge_id) REFERENCES social.badge_definitions(id) ON DELETE CASCADE;


--
-- Name: user_badge_progress user_badge_progress_badge_id_fkey; Type: FK CONSTRAINT; Schema: social; Owner: -
--

ALTER TABLE ONLY social.user_badge_progress
    ADD CONSTRAINT user_badge_progress_badge_id_fkey FOREIGN KEY (badge_id) REFERENCES social.badge_definitions(id) ON DELETE CASCADE;


--
-- Name: experiment_group_targets experiment_group_targets_group_fkey; Type: FK CONSTRAINT; Schema: telemetry; Owner: -
--

ALTER TABLE ONLY telemetry.experiment_group_targets
    ADD CONSTRAINT experiment_group_targets_group_fkey FOREIGN KEY (group_name) REFERENCES telemetry.flag_groups(name) ON DELETE CASCADE;


--
-- Name: flag_group_targets flag_group_targets_group_fkey; Type: FK CONSTRAINT; Schema: telemetry; Owner: -
--

ALTER TABLE ONLY telemetry.flag_group_targets
    ADD CONSTRAINT flag_group_targets_group_fkey FOREIGN KEY (group_name) REFERENCES telemetry.flag_groups(name) ON DELETE CASCADE;


--
-- Name: world_access_communities fk_world_access_communities_world; Type: FK CONSTRAINT; Schema: worlds; Owner: -
--

ALTER TABLE ONLY worlds.world_access_communities
    ADD CONSTRAINT fk_world_access_communities_world FOREIGN KEY (world_name) REFERENCES worlds.worlds(name) ON DELETE CASCADE;


--
-- Name: world_scenes fk_world_name; Type: FK CONSTRAINT; Schema: worlds; Owner: -
--

ALTER TABLE ONLY worlds.world_scenes
    ADD CONSTRAINT fk_world_name FOREIGN KEY (world_name) REFERENCES worlds.worlds(name) ON DELETE CASCADE;


--
-- Name: world_permission_parcels fk_world_permission_parcels_grant; Type: FK CONSTRAINT; Schema: worlds; Owner: -
--

ALTER TABLE ONLY worlds.world_permission_parcels
    ADD CONSTRAINT fk_world_permission_parcels_grant FOREIGN KEY (world_name, permission, address) REFERENCES worlds.world_permissions(world_name, permission, address) ON DELETE CASCADE;


--
-- Name: world_permissions fk_world_permissions_world; Type: FK CONSTRAINT; Schema: worlds; Owner: -
--

ALTER TABLE ONLY worlds.world_permissions
    ADD CONSTRAINT fk_world_permissions_world FOREIGN KEY (world_name) REFERENCES worlds.worlds(name) ON DELETE CASCADE;


--
-- PostgreSQL database dump complete
--


--
-- PostgreSQL database dump
--


-- Dumped from database version 18.6
-- Dumped by pg_dump version 18.6

SET statement_timeout = 0;
SET lock_timeout = 0;
SET idle_in_transaction_session_timeout = 0;
SET client_encoding = 'UTF8';
SET standard_conforming_strings = on;
SELECT pg_catalog.set_config('search_path', '', false);
SET check_function_bodies = false;
SET xmloption = content;
SET client_min_messages = warning;
SET row_security = off;

--
-- Data for Name: schema; Type: TABLE DATA; Schema: meta; Owner: -
--

INSERT INTO meta.schema VALUES (1, 'catalyst', 2, '2026-11-03 20:02:47.969514');


--
-- Data for Name: schema_steps; Type: TABLE DATA; Schema: meta; Owner: -
--

INSERT INTO meta.schema_steps VALUES ('0001_credits_purchases_quantity_source', '98f0f1da05826e9b72db918e1f3b8f00fd0ce1c5a61e5d75688b7321a0d97338', false, '2026-11-03 20:02:48.154956');
INSERT INTO meta.schema_steps VALUES ('0002_chain_cursor_rentals_indexer', '7c626109785f1b39e2e94347efee170350380acdf215b9fa166ec5f69a507b0d', false, '2026-11-03 20:02:48.154956');
INSERT INTO meta.schema_steps VALUES ('0003_shop_lists_name_unique', '9ed6a20beb6aa369441a787546c57f11ea5202bf47769d468fe9b344a9042625', false, '2026-11-03 20:02:48.154956');
INSERT INTO meta.schema_steps VALUES ('0004_credits_protocol', 'b70a360fa4d62b7ff5cb0c3832efc84d9ed2098f194ef3532fd17102d3e41e6b', false, '2026-11-03 20:02:48.154956');
INSERT INTO meta.schema_steps VALUES ('0005_broker_purchases_mode_primary', '0d75b0bea8cf92951981dce52524851ba772760888a605140a8d684e85d3aac6', false, '2026-11-03 20:02:48.154956');
INSERT INTO meta.schema_steps VALUES ('0006_chain_cursor_mana_transfers', '429e020ef0ba46179e059a84a9a129cd1f93164dd446fc6b2d34e63bf4386679', false, '2026-11-03 20:02:48.154956');
INSERT INTO meta.schema_steps VALUES ('0007_mirror_origin', 'a644df7d4a0558f88a389bae9e013b6ddc37b5992e19fe2839974526c724ce7f', false, '2026-11-03 20:02:48.154956');
INSERT INTO meta.schema_steps VALUES ('0008_comms_stream_cleanup', '8a6f913d09d5a479c04f7e7b713659b9c7cfb76050988edd6e7ce907e17ec808', false, '2026-11-03 20:02:48.154956');
INSERT INTO meta.schema_steps VALUES ('0010_world_scene_storage_identity', 'a8f0d6a719e8d44c14920c4a9df60d8524534d387279073b9e7dc5b0b9cf0a8b', false, '2026-11-03 20:02:48.154956');
INSERT INTO meta.schema_steps VALUES ('0011_land_history_timestamps', '9b66321fd0de2b92d380b70045ce9fe9695e3d9e15992d68186b2ddfaf0dd45f', false, '2026-11-03 20:02:48.154956');
INSERT INTO meta.schema_steps VALUES ('0012_land_history_published_readiness', '8629204e1dd053a981d1778325a4277dee1effd556a842e8cf7148ae1f771670', false, '2026-11-03 20:02:48.154956');
INSERT INTO meta.schema_steps VALUES ('0013_island_receipt_retirement', '819ca1841a998a90dcdb474a6b2b62f6f22a3b83db05f5601f9ed789ef4350bf', false, '2026-11-03 20:02:48.154956');
INSERT INTO meta.schema_steps VALUES ('0014_event_permission_grants', '5f02dabf25078f5b86d3a90d0f8dfc26b75c8f0dac3adbafe0c936f619fcb52f', false, '2026-11-03 20:02:48.154956');
INSERT INTO meta.schema_steps VALUES ('0016_report_upload_capabilities', '79776d101d77838db55c5371ea27ac7bd9b601e374a6e7a88af652b8e0340a17', false, '2026-11-03 20:02:48.154956');
INSERT INTO meta.schema_steps VALUES ('0017_event_notification_visibility', '25515f040fb9f9768be04419dbc15d538f17e9952d52149430864beb1bf88b5d', false, '2026-11-03 20:02:48.154956');
INSERT INTO meta.schema_steps VALUES ('0020_social_voice_notifications', '41c5172fe07ee74acad62a9e1fa3ae74e5eaeb51d2a13262146f20034faaef17', false, '2026-11-03 20:02:48.154956');
INSERT INTO meta.schema_steps VALUES ('0021_social_referral_lifecycle', 'ebf8f13fc80bfa29cbf29fffc835b197ad5a239f87912d3b891bdfaa7e850a26', false, '2026-11-03 20:02:48.154956');
INSERT INTO meta.schema_steps VALUES ('0022_social_community_ranking', '22b853d00b7e37503abccb2ba4daac926559d5004499dec9ef3941bb40a70ffe', false, '2026-11-03 20:02:48.154956');


--
-- PostgreSQL database dump complete
--
