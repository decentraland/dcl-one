-- Catalyst v2: the creator plane (the old catalyrst-create bundle): `builder`,
-- the pre-publish draft store behind builder-api.decentraland.org (creator
-- drafts, uploads, curation state, collection publication state,
-- newsletter opt-ins; no scenes), and
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
-- Designed from the workload, not copied. Sources:
-- catalyrst/crates/catalyrst-builder/migrations 0001..0007 and
-- catalyrst/crates/catalyrst-camera-reel/migrations 20260609..20260619.
-- Not carried: `uploaded_contents.bytes` (a 20 MB bytea per upload, kept in
-- Postgres for want of a store; v2 stores the bytes in the ONE blob store by
-- hash and keeps the acceptance record here), the camera-reel `url` /
-- `thumbnail_url` columns (a copy of the API_URL setting; the wire URL
-- renders from the hash), the camera-reel per-user `(user_address,
-- is_public)` and review-status indexes and the builder `curation_status`
-- index (no statement reads them).
--
-- Conventions: wallet addresses are lowercase 0x + 40 hex text; every
-- timestamp column is `timestamp without time zone` holding UTC (the pool
-- pins TimeZone=UTC; the old migrations declared timestamptz); the current
-- time is a bind parameter wherever it decides anything (an index
-- predicate, a due-work filter, a cursor, withheld_at) and the
-- updated_at/built_at stamps are timezone('UTC', now()) in the statement;
-- every index names its query. No FK crosses a schema:
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

CREATE TABLE builder.collections (
    id uuid NOT NULL,
    name text NOT NULL,
    eth_address text NOT NULL,
    salt text,
    contract_address text,
    urn_suffix text,
    third_party_id text,
    publication_pending boolean NOT NULL DEFAULT false,
    created_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    updated_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    linked_contract_address text,
    linked_contract_network text,
    lock timestamp without time zone,
    forum_link text,
    forum_id integer,
    forum_claimed_at timestamp without time zone,
    CONSTRAINT collections_pkey PRIMARY KEY (id),
    CONSTRAINT collections_contract_address_key UNIQUE (contract_address),
    CONSTRAINT collections_contract_address_lowercase_check CHECK (contract_address = lower(contract_address)),
    CONSTRAINT collections_eth_address_lowercase_check CHECK (eth_address = lower(eth_address)),
    CONSTRAINT collections_name_check CHECK (name = btrim(name) AND name <> ''),
    CONSTRAINT collections_salt_len_check CHECK (salt IS NULL OR char_length(salt) <= 128),
    CONSTRAINT collections_third_party_urn_check
        CHECK (third_party_id IS NULL
               OR (urn_suffix IS NOT NULL AND left(urn_suffix, char_length(third_party_id) + 1) = third_party_id || ':')),
    CONSTRAINT collections_linked_contract_check
        CHECK (linked_contract_address IS NULL OR linked_contract_address ~ '^0x[0-9a-f]{40}$'),
    CONSTRAINT collections_linked_network_check
        CHECK (linked_contract_network IS NULL OR linked_contract_network IN ('mainnet', 'sepolia', 'matic', 'amoy',
               'base_mainnet', 'base_sepolia', 'monad_mainnet', 'monad_testnet', 'ape_mainnet', 'ape_caldera')),
    CONSTRAINT collections_linked_contract_key UNIQUE (third_party_id, linked_contract_address, linked_contract_network),
    CONSTRAINT collections_forum_link_check
        CHECK (forum_link IS NULL OR (char_length(forum_link) <= 2048 AND forum_link ~ '^https?://[^[:space:]]+$')),
    CONSTRAINT collections_forum_id_check CHECK (forum_id IS NULL OR (forum_id > 0 AND forum_link IS NOT NULL)),
    CONSTRAINT collections_forum_claim_check CHECK (forum_claimed_at IS NULL OR forum_link IS NULL)
);

CREATE FUNCTION builder.collections_linked_contract_kept() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    IF (OLD.linked_contract_address IS NOT NULL AND NEW.linked_contract_address IS NULL)
       OR (OLD.linked_contract_network IS NOT NULL AND NEW.linked_contract_network IS NULL) THEN
        RAISE EXCEPTION 'Columns linked_contract_address, linked_contract_network cannot be set to null if they were previously not null';
    END IF;
    RETURN NEW;
END;
$$;

CREATE TRIGGER collections_linked_contract_kept BEFORE UPDATE ON builder.collections
    FOR EACH ROW EXECUTE FUNCTION builder.collections_linked_contract_kept();

-- "My collections" (GET /v1/drafts/collections: eth_address = $1 AND (not
-- published OR third_party_id IS NOT NULL) ORDER BY updated_at DESC), a
-- per-owner set of tens of rows.
CREATE INDEX collections_owner_idx ON builder.collections (eth_address);

CREATE INDEX collections_standard_name_idx ON builder.collections (lower(name)) WHERE third_party_id IS NULL;

CREATE INDEX collections_third_party_urn_idx ON builder.collections (urn_suffix) WHERE third_party_id IS NOT NULL;

CREATE TABLE builder.virtual_third_parties (
    id text NOT NULL,
    managers text[] NOT NULL,
    raw_metadata text NOT NULL,
    is_programmatic boolean NOT NULL DEFAULT false,
    created_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    updated_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    CONSTRAINT virtual_third_parties_pkey PRIMARY KEY (id),
    CONSTRAINT virtual_third_parties_id_len_check CHECK (char_length(id) <= 512),
    CONSTRAINT virtual_third_parties_managers_lowercase_check CHECK (managers::text = lower(managers::text))
);

-- ── items ──────────────────────────────────────────────────────────────────

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
    data json NOT NULL DEFAULT '{}'::json,
    metrics json,
    utility text,
    mappings jsonb,
    urn_suffix text,
    blockchain_item_id text,
    is_published boolean NOT NULL DEFAULT false,
    in_catalyst boolean NOT NULL DEFAULT false,
    total_supply bigint NOT NULL DEFAULT 0,
    local_content_hash text,
    content_hash text,
    catalyst_content_hash text,
    created_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    updated_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    video_named_by text,
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
    CONSTRAINT items_data_object_check CHECK (json_typeof(data) = 'object'),
    CONSTRAINT items_metrics_object_check CHECK (metrics IS NULL OR json_typeof(metrics) = 'object'),
    CONSTRAINT items_published_identity_check
        CHECK (NOT is_published OR (urn_suffix IS NOT NULL AND blockchain_item_id IS NOT NULL)),
    CONSTRAINT items_total_supply_check CHECK (total_supply >= 0),
    CONSTRAINT items_video_named_by_check
        CHECK (video_named_by IS NULL OR (video_named_by <> '' AND video_named_by = lower(video_named_by)))
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

CREATE INDEX items_minted_collection_idx ON builder.items (collection_id) WHERE blockchain_item_id IS NOT NULL;

CREATE INDEX items_created_idx ON builder.items (created_at, id);

CREATE INDEX items_video_idx ON builder.items (video, video_named_by) WHERE video IS NOT NULL;

CREATE INDEX items_urn_idx ON builder.items (urn_suffix) WHERE urn_suffix IS NOT NULL;

-- ── uploads ────────────────────────────────────────────────────────────────

CREATE TABLE builder.uploaded_contents (
    hash text NOT NULL,
    size bigint NOT NULL,
    uploaded_by text NOT NULL,
    created_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    video boolean NOT NULL DEFAULT false,
    CONSTRAINT uploaded_contents_pkey PRIMARY KEY (hash, uploaded_by),
    CONSTRAINT uploaded_contents_size_check CHECK (size >= 0 AND size <= 250000000),
    CONSTRAINT uploaded_contents_uploaded_by_lowercase_check CHECK (uploaded_by = lower(uploaded_by))
);

CREATE INDEX uploaded_contents_uploaded_by_idx ON builder.uploaded_contents (uploaded_by) INCLUDE (size);

CREATE TABLE builder.item_contents (
    item_id uuid NOT NULL,
    file text NOT NULL,
    hash text NOT NULL,
    position integer NOT NULL DEFAULT 0,
    named_by text NOT NULL DEFAULT '',
    CONSTRAINT item_contents_pkey PRIMARY KEY (item_id, file),
    CONSTRAINT item_contents_item_id_fkey FOREIGN KEY (item_id)
        REFERENCES builder.items (id) ON DELETE CASCADE,
    CONSTRAINT item_contents_file_check
        CHECK (file <> '' AND char_length(file) <= 256 AND position('\' in file) = 0),
    CONSTRAINT item_contents_named_by_check CHECK (named_by <> '' AND named_by = lower(named_by))
);

CREATE INDEX item_contents_hash_idx ON builder.item_contents (hash);

CREATE INDEX item_contents_hash_named_by_idx ON builder.item_contents (hash, named_by);

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
-- the ONE store named by `image_hash`/`thumbnail_hash`. The wire URLs use
-- UUID-prefixed basenames when retained, and hashes for older rows. `metadata` is the
-- client document (`Metadata`: userName, userAddress, dateTime, realm,
-- scene, visiblePeople, placeId) returned as a unit; `place_id` is its
-- placeId projected verbatim for the place galleries: a catalogue ID or an
-- explore.world_scene_place identity (no FK: different lifecycles). `user_address` is
-- the verified signer (the write path refuses a metadata.userAddress that is
-- not the signer; the CHECK holds the pair together). `review_status` is the
-- moderator's: `rejected` hides the row from public galleries and metadata
-- reads, and withholds unreferenced blobs on the protected byte routes.
CREATE TABLE camera_reel.images (
    id uuid NOT NULL,
    user_address text NOT NULL,
    image_hash text NOT NULL,
    thumbnail_hash text NOT NULL,
    -- The client's basename renders UUID-prefixed image and thumbnail aliases.
    -- NULL preserves the hash URLs of rows created without a filename.
    original_filename text,
    -- Both source objects use the accepted client MIME, independently of encoding.
    -- NULL leaves older rows on their byte-sniffing fallback.
    image_content_type text CONSTRAINT images_image_content_type_check
        CHECK (image_content_type IN ('image/png', 'image/jpeg')),
    is_public boolean NOT NULL DEFAULT false,
    review_status text NOT NULL DEFAULT 'ok',
    -- Old rows have no new ranking event. New uploads set false, then publish
    -- after both blobs exist; a bounded repair pass finishes interrupted work.
    ranking_published boolean NOT NULL DEFAULT true,
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

-- GET /api/files/{hash}: a blob is served only while a non-rejected row
-- names it as image or thumbnail (image_hash = $1 OR thumbnail_hash = $1,
-- bitmap OR of the two).
CREATE INDEX images_image_hash_idx ON camera_reel.images (image_hash);
CREATE INDEX images_thumbnail_hash_idx ON camera_reel.images (thumbnail_hash);
-- Only pending publications enter this repair keyset; an empty due set does
-- not scan the complete retained photo gallery once per minute.
CREATE INDEX images_ranking_pending_idx ON camera_reel.images (id) WHERE NOT ranking_published;

-- ══ Domain-agent sections: append inside the matching block only ═══════════

-- ==== section: builder ====

-- A creator's acceptance of the publication terms (POST
-- /v1/collections/{id}/tos), kept on the node and forwarded to the
-- warehouse: `body` is the forward's body, the email address included.
-- Written by that route and its forward only; no other statement reads it
-- and no route answers it. The forward is at least once: an inline attempt
-- after the insert, then the leased `builder-terms-forward` pass over the
-- due rows (`next_attempt_at`), at most six attempts with doubling backoff;
-- `forwarded_at` marks a delivered row. No foreign key: a deleted
-- collection keeps the acceptances made for it.
CREATE TABLE builder.terms_acceptances (
    id uuid NOT NULL,
    collection_id uuid NOT NULL,
    signer text NOT NULL,
    event text NOT NULL,
    body jsonb NOT NULL,
    created_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    attempts integer NOT NULL DEFAULT 0,
    next_attempt_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    forwarded_at timestamp without time zone,
    CONSTRAINT terms_acceptances_pkey PRIMARY KEY (id),
    CONSTRAINT terms_acceptances_signer_lowercase_check CHECK (signer = lower(signer)),
    CONSTRAINT terms_acceptances_event_check
        CHECK (event IN ('publish_collection_tos', 'publish_third_party_items_tos')),
    CONSTRAINT terms_acceptances_body_object_check CHECK (jsonb_typeof(body) = 'object'),
    CONSTRAINT terms_acceptances_attempts_check CHECK (attempts BETWEEN 0 AND 6)
);

-- The forward pass's due set: undelivered rows with attempts left, oldest
-- due first.
CREATE INDEX terms_acceptances_due_idx ON builder.terms_acceptances (next_attempt_at)
    WHERE forwarded_at IS NULL AND attempts < 6;

CREATE INDEX terms_acceptances_collection_signer_event_idx
    ON builder.terms_acceptances (collection_id, signer, event);

CREATE TABLE builder.collection_curations (
    id uuid NOT NULL,
    collection_id uuid NOT NULL,
    status text NOT NULL,
    assignee text,
    reviewed_by text,
    rejection_reasons text[],
    rejection_message text,
    created_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    updated_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    CONSTRAINT collection_curations_pkey PRIMARY KEY (id),
    CONSTRAINT collection_curations_collection_id_fkey FOREIGN KEY (collection_id)
        REFERENCES builder.collections (id) ON DELETE CASCADE,
    CONSTRAINT collection_curations_collection_created_key UNIQUE (collection_id, created_at),
    CONSTRAINT collection_curations_status_check CHECK (status IN ('pending', 'approved', 'rejected')),
    CONSTRAINT collection_curations_assignee_check CHECK (assignee IS NULL OR assignee ~ '^0x[0-9a-f]{40}$'),
    CONSTRAINT collection_curations_reviewed_by_check
        CHECK (reviewed_by IS NULL OR reviewed_by = 'validator' OR reviewed_by ~ '^0x[0-9a-f]{40}$'),
    CONSTRAINT collection_curations_rejection_reasons_check
        CHECK (rejection_reasons IS NULL OR (cardinality(rejection_reasons) >= 1
               AND rejection_reasons <@ ARRAY['clipping', 'thumbnail', 'category_hides', 'rigging', 'triangle_count',
                   'emote', 'file_size', 'textures_materials', 'reversed_faces', 'smart_wearable_files',
                   'content_policy_ip', 'other']::text[])),
    CONSTRAINT collection_curations_rejection_message_check
        CHECK (rejection_message IS NULL OR char_length(rejection_message) BETWEEN 1 AND 3000),
    CONSTRAINT collection_curations_updated_check CHECK (updated_at >= created_at)
);

CREATE TABLE builder.item_curations (
    id uuid NOT NULL,
    item_id uuid NOT NULL,
    status text NOT NULL,
    content_hash text NOT NULL,
    is_mapping_complete boolean,
    created_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    updated_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    CONSTRAINT item_curations_pkey PRIMARY KEY (id),
    CONSTRAINT item_curations_item_id_fkey FOREIGN KEY (item_id)
        REFERENCES builder.items (id) ON DELETE CASCADE,
    CONSTRAINT item_curations_item_created_key UNIQUE (item_id, created_at),
    CONSTRAINT item_curations_status_check CHECK (status IN ('pending', 'approved', 'rejected')),
    CONSTRAINT item_curations_content_hash_check CHECK (content_hash ~ '^[0-9a-f]{64}$'),
    CONSTRAINT item_curations_updated_check CHECK (updated_at >= created_at)
);

CREATE TABLE builder.collection_events (
    id uuid NOT NULL,
    collection_id uuid NOT NULL,
    type text NOT NULL,
    actor text NOT NULL,
    actor_address text,
    payload jsonb NOT NULL DEFAULT '{}'::jsonb,
    created_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    CONSTRAINT collection_events_pkey PRIMARY KEY (id),
    CONSTRAINT collection_events_collection_id_fkey FOREIGN KEY (collection_id)
        REFERENCES builder.collections (id) ON DELETE CASCADE,
    CONSTRAINT collection_events_type_check
        CHECK (type IN ('collection.published', 'review.human_required', 'review.automated_started',
               'review.automated_passed', 'review.automated_rejected', 'review.automated_error',
               'review.appeal_requested', 'review.assigned', 'review.approved', 'review.rejected',
               'changes.submitted', 'review.comment')),
    CONSTRAINT collection_events_actor_check CHECK (actor IN ('creator', 'curator', 'validator', 'system')),
    CONSTRAINT collection_events_actor_address_check
        CHECK (actor_address IS NULL OR actor_address ~ '^0x[0-9a-f]{40}$'),
    CONSTRAINT collection_events_payload_object_check CHECK (jsonb_typeof(payload) = 'object')
);

CREATE INDEX collection_events_collection_created_idx
    ON builder.collection_events (collection_id, created_at DESC, id DESC);

CREATE INDEX collection_events_automated_started_idx ON builder.collection_events (created_at)
    WHERE type = 'review.automated_started';

CREATE INDEX collection_events_creator_text_idx ON builder.collection_events (actor_address, created_at)
    WHERE type IN ('review.comment', 'review.appeal_requested');

CREATE UNIQUE INDEX collection_events_verdict_validation_idx
    ON builder.collection_events (collection_id, type, (payload->>'validationId'), (COALESCE(payload->>'reason', '')))
    WHERE type IN ('review.automated_passed', 'review.automated_rejected', 'review.automated_error',
                   'review.human_required');

CREATE TABLE builder.slot_usage_cheques (
    id uuid NOT NULL,
    collection_id uuid,
    third_party_id text NOT NULL,
    signature text NOT NULL,
    salt text NOT NULL,
    qty integer NOT NULL,
    created_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    updated_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    CONSTRAINT slot_usage_cheques_pkey PRIMARY KEY (id),
    CONSTRAINT slot_usage_cheques_collection_id_fkey FOREIGN KEY (collection_id)
        REFERENCES builder.collections (id) ON DELETE SET NULL,
    CONSTRAINT slot_usage_cheques_third_party_salt_key UNIQUE (third_party_id, salt),
    CONSTRAINT slot_usage_cheques_third_party_id_check CHECK (char_length(third_party_id) <= 512),
    CONSTRAINT slot_usage_cheques_salt_check CHECK (salt ~ '^0x[0-9a-f]{64}$'),
    CONSTRAINT slot_usage_cheques_signature_check CHECK (signature ~ '^0x[0-9a-f]{130}$'),
    CONSTRAINT slot_usage_cheques_qty_check CHECK (qty > 0)
);

CREATE INDEX slot_usage_cheques_collection_created_idx ON builder.slot_usage_cheques (collection_id, created_at);

CREATE TABLE builder.validation_requests (
    id uuid NOT NULL,
    collection_id uuid NOT NULL,
    trigger text NOT NULL,
    item_ids uuid[] NOT NULL,
    created_at timestamp without time zone NOT NULL,
    claims smallint NOT NULL DEFAULT 0,
    claim_id uuid,
    claimed_at timestamp without time zone,
    lease_until timestamp without time zone,
    content_hashes jsonb,
    settled_at timestamp without time zone,
    outcome text,
    CONSTRAINT validation_requests_pkey PRIMARY KEY (id),
    CONSTRAINT validation_requests_collection_id_fkey FOREIGN KEY (collection_id)
        REFERENCES builder.collections (id) ON DELETE CASCADE,
    CONSTRAINT validation_requests_trigger_check CHECK (trigger IN ('publish', 'changes', 'retry')),
    CONSTRAINT validation_requests_item_ids_check CHECK (cardinality(item_ids) BETWEEN 1 AND 50),
    CONSTRAINT validation_requests_claims_check CHECK (claims BETWEEN 0 AND 3),
    CONSTRAINT validation_requests_claim_check
        CHECK ((claim_id IS NULL) = (claimed_at IS NULL) AND (claimed_at IS NULL) = (lease_until IS NULL)
               AND (claim_id IS NULL) = (claims = 0)),
    CONSTRAINT validation_requests_content_hashes_object_check
        CHECK (content_hashes IS NULL OR jsonb_typeof(content_hashes) = 'object'),
    CONSTRAINT validation_requests_settled_check CHECK ((settled_at IS NULL) = (outcome IS NULL)),
    CONSTRAINT validation_requests_outcome_check
        CHECK (outcome IN ('passed', 'rejected', 'error', 'superseded', 'abandoned', 'expired'))
);

CREATE UNIQUE INDEX validation_requests_open_key ON builder.validation_requests (collection_id)
    WHERE settled_at IS NULL;

CREATE INDEX validation_requests_open_created_idx ON builder.validation_requests (created_at, id)
    WHERE settled_at IS NULL;

CREATE INDEX validation_requests_collection_created_idx
    ON builder.validation_requests (collection_id, created_at);

CREATE INDEX validation_requests_settled_idx ON builder.validation_requests (settled_at)
    WHERE settled_at IS NOT NULL;

CREATE TABLE builder.validator_seen_signatures (
    signature text NOT NULL,
    expires_at timestamp without time zone NOT NULL,
    CONSTRAINT validator_seen_signatures_pkey PRIMARY KEY (signature),
    CONSTRAINT validator_seen_signatures_signature_check CHECK (signature ~ '^[0-9a-f]{64}$')
);

CREATE INDEX validator_seen_signatures_expires_idx ON builder.validator_seen_signatures (expires_at);
-- ==== end section: builder ====

-- ==== section: camera_reel ====
-- World-name galleries (`name.dcl.eth`) resolve catalogue IDs and active scene
-- identities bound to their current deployment, independently of listing
-- visibility. See DATABASE.md, "Camera reel World gallery lookup", for the
-- shared reads, eligibility rules and in-process cache; no cache table.
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
-- and are not checked. `image_id` is the
-- row whose verdict wrote it, kept as a fact (no FK: a deleted row is gone).
CREATE TABLE camera_reel.withheld_blobs (
    hash text NOT NULL,
    image_id uuid NOT NULL,
    reason text NOT NULL,
    withheld_at timestamp without time zone NOT NULL,
    CONSTRAINT withheld_blobs_pkey PRIMARY KEY (hash),
    CONSTRAINT withheld_blobs_reason_check CHECK (reason IN ('rejected', 'deleted'))
);

-- The collection items worn in a photo, as `<contract>-<itemId>` lowercase
-- (the marketplace's item identity: the per-NFT token id of a
-- collections-v2 URN is dropped, so every copy of an item matches the one
-- item a shopper looks at). Derived from `metadata.visiblePeople[*]
-- .wearables[*]`; a document without those arrays (an older shape, a photo
-- of an empty scene) yields an empty array rather than failing the index
-- build. IMMUTABLE so it can be indexed; rebuild rule = the expression.
CREATE FUNCTION camera_reel.image_wearable_items(metadata jsonb) RETURNS text[]
LANGUAGE sql IMMUTABLE PARALLEL SAFE AS $$
  SELECT coalesce(
           array_agg(DISTINCT lower(split_part(wearable, ':', 5) || '-' || split_part(wearable, ':', 6))),
           ARRAY[]::text[])
  FROM jsonb_array_elements(CASE WHEN jsonb_typeof(metadata -> 'visiblePeople') = 'array'
                                 THEN metadata -> 'visiblePeople' ELSE '[]'::jsonb END) AS person,
       jsonb_array_elements_text(CASE WHEN jsonb_typeof(person -> 'wearables') = 'array'
                                      THEN person -> 'wearables' ELSE '[]'::jsonb END) AS wearable
  WHERE wearable LIKE 'urn:decentraland:%:collections-v2:%'
$$;

-- GET /api/wearables/{item}/images: public photos of people wearing an item
-- (image_wearable_items(metadata) && ARRAY[$1] AND is_public AND
-- review_status <> 'rejected' ORDER BY created_at DESC, id DESC), which only
-- ever reads public, non-rejected rows, so the predicate is the index's.
CREATE INDEX images_wearable_items_idx ON camera_reel.images
    USING gin (camera_reel.image_wearable_items(metadata))
    WHERE is_public AND review_status <> 'rejected';

-- The photo events camera-reel-service published to SNS, recorded for
-- in-process consumers instead (append-only outbox; camera_reel::events).
-- Workload: one insert per finalised upload (`photo-taken`, in the
-- transaction that sets images.ranking_published) and one per visibility
-- change (`photo-privacy-changed`, in the statement that changes it), so
-- writes follow the upload rate. Reads: each consumer pages `id > $cursor
-- ORDER BY id LIMIT n` (the primary key) and keeps its cursor in its own
-- schema; the outbox holds no consumer state and rows are never updated.
-- Retention: the fleet-wide `camera-photo-events-retention` job deletes rows
-- older than 30 days in bounded batches by `created_at` (the second index),
-- so the table holds about 30 days of events. `payload` is the upstream
-- event body `{type, subType, key, timestamp, metadata}`; `photo_id` names
-- an images row as a fact (no FK: the photo may be deleted later).
CREATE TABLE camera_reel.photo_events (
    id bigint GENERATED ALWAYS AS IDENTITY,
    kind text NOT NULL,
    photo_id uuid NOT NULL,
    user_address text NOT NULL,
    payload jsonb NOT NULL,
    created_at timestamp with time zone NOT NULL DEFAULT now(),
    CONSTRAINT photo_events_pkey PRIMARY KEY (id),
    CONSTRAINT photo_events_kind_check CHECK (kind IN ('photo-taken', 'photo-privacy-changed')),
    CONSTRAINT photo_events_user_address_lowercase_check CHECK (user_address = lower(user_address)),
    CONSTRAINT photo_events_payload_object_check CHECK (jsonb_typeof(payload) = 'object')
);
CREATE INDEX photo_events_created_at_idx ON camera_reel.photo_events (created_at);

-- The photos `catalyst camera-reel import` copied from camera-reel-service's
-- database: one row per imported `images` row, by the same id, written in
-- the transaction that inserts it. A fact about the copy, not about the
-- photo, so it is its own relation rather than a column every gallery read
-- carries. Readers: the import's re-runs (which source rows it refreshes:
-- only imported ones, never a photo uploaded here) and its `--prune` (which
-- local rows a source delete may remove: only imported ones), both by
-- primary key. Rows: at most one per photo, written once at import and
-- never updated; the cascade removes it with its photo (an owner's or
-- moderator's delete, or a prune). Once the old service is retired nothing
-- reads it.
CREATE TABLE camera_reel.imported_images (
    id uuid NOT NULL,
    imported_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    CONSTRAINT imported_images_pkey PRIMARY KEY (id),
    CONSTRAINT imported_images_id_fkey FOREIGN KEY (id)
        REFERENCES camera_reel.images (id) ON DELETE CASCADE
);
-- ==== end section: camera_reel ====
