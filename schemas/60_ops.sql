-- Catalyst v2: the operator plane and the remaining services: telemetry (the
-- Sentry/Segment sinks and the issues/flags/experiments console), credits
-- (the Marketplace Credits ledger, Stripe purchases, carts, checkouts and
-- authorizations) and ops (the ONE operator audit).
--
-- Designed from the workload, not copied: the old crates' four databases
-- (`catalyrst` search_path=telemetry, `credits`) collapse into two schemas
-- of `catalyst`. Sources: catalyrst/crates/catalyrst-telemetry/
-- migrations 0001..0011, catalyrst-credits/migrations 0001..0021,
-- the old server's admin_audit (catalyrst-server/src/admin/audit.rs).
--
-- Conventions, as in the other files: wallet addresses are lowercase 0x + 40
-- hex text where this node writes them (CHECKed); every timestamp is
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
-- the telemetry console writes and the operator audit are
-- LOCAL authority behind OPS_WRITES_ENABLED (default false; 503 envelope like
-- src/social/writes.rs).
--
-- Domain agents append ONLY inside the `-- ==== section: ... ====` blocks at
-- the end, after re-reading this file.

CREATE SCHEMA telemetry;
CREATE SCHEMA credits;
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
-- The reverse lookup (every source folded into one canonical): the
-- `fingerprint=` filter of the console's events page, issues aggregate and
-- facet stats (`src/ops/telemetry/dashboard/mod.rs`, `merged_into`); the
-- aggregate's fold itself probes the PK per event row.
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
    -- Packs this checkout bought; credits and amount_cents already hold the
    -- totals, so it moves no money (the reports tell 3 x pack from 1 x pack).
    quantity integer NOT NULL DEFAULT 1,
    -- The surface that opened a Checkout Session (NULL: unknown or another flow);
    -- it also chose the page Stripe returned the buyer to.
    source text,
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
    CONSTRAINT purchases_revoked_bound_check CHECK (revoked_credits >= 0 AND revoked_credits <= credits),
    CONSTRAINT purchases_quantity_check CHECK (quantity >= 1),
    CONSTRAINT purchases_source_check CHECK (source IN ('website', 'client', 'builder'))
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
-- pins a v3 signed trade (re-fetched from shop.trades and checked
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

-- One ephemeral credit (src/shop/credits/{authorize,batch,settlement}.rs;
-- docs/shop/CREDIT-HASH.md, CONTRACTS.md): a settlement group of up to 30
-- lines (`authorization_lines`) pledged as ONE signed credit whose salt is
-- `id`. `usd_cents` is the charged total (each line's quote rounded up to a
-- whole credit), `quoted_cents` the quoted total the signed value is sized
-- from (the oracle rate `oracle_rate` at `oracle_decimals`, the buffer and
-- the cap headroom), `amount_wei` the signed value, `contract` the
-- CreditsManager and `chain_id` its chain, `credit_hash` the contract's
-- identity of the credit (the revocation key; the indexed `_creditId` of
-- CreditUsed), `expires_at` the credit's expiry in the row clock (the
-- signed `expiresAt` is its unix SECONDS). A pledge counts against the
-- balance while it is `authorized` or `retired` (the buyer cancelled it,
-- which is not proof the signed credit became unspendable); the sweep moves
-- both to `released`/`expired` only once the CreditUsed index has read the
-- chain past the expiry (`usage_index_cursor.folded_block_time`), and the
-- settlement pass moves any of them to `consumed` when a CreditUsed log
-- names the salt, however late. `submitted_tx_hash` is the transaction the
-- client reported submitting the credit in: a fact about an attempt, never
-- a verdict (the outcome is `status` and `usage_settlements`).
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
    CONSTRAINT authorizations_pkey PRIMARY KEY (id),
    CONSTRAINT authorizations_address_check CHECK (address ~ '^0x[0-9a-f]{40}$'),
    CONSTRAINT authorizations_status_check CHECK (status IN ('authorized', 'retired', 'released', 'consumed', 'expired')),
    CONSTRAINT authorizations_usd_check CHECK (usd_cents > 0),
    CONSTRAINT authorizations_credit_hash_check CHECK (credit_hash IS NULL OR credit_hash ~ '^0x[0-9a-f]{64}$'),
    CONSTRAINT authorizations_submitted_tx_check CHECK (submitted_tx_hash IS NULL OR submitted_tx_hash ~ '^0x[0-9a-f]{64}$')
);

-- The idempotent reserve (the client's key, one authorization per key).
CREATE UNIQUE INDEX authorizations_idempotency_key_uidx ON credits.authorizations (idempotency_key)
    WHERE idempotency_key IS NOT NULL;
-- The outstanding budget: SUM(usd_cents) WHERE address = $1 AND status IN
-- ('authorized', 'retired'), the wallet's own list and the purchases
-- history (address, newest first).
CREATE INDEX authorizations_address_status_expiry_idx ON credits.authorizations (address, status, expires_at);
-- The expiry sweep (status IN ('authorized', 'retired') AND expires_at <= $bound), run by the fulfilment tick.
CREATE INDEX authorizations_expiry_idx ON credits.authorizations (expires_at) WHERE status IN ('authorized', 'retired');
-- The purchases history: the buyer's groups newest first, keyset on (created_at, id).
CREATE INDEX authorizations_address_created_idx ON credits.authorizations (address, created_at DESC, id);

-- The lines of one credit, in request order: what each line bought (the
-- payment rule's derivation: `sale_kind`/`asset_kind` are what
-- shop::policy derived from the mirrors and the decoded order, never a
-- client flag), what it was quoted and charged. SUM(charged_cents) over a
-- credit's lines equals the credit's usd_cents by construction (the
-- reconciler checks it). Lines share their credit's lifecycle: a group is
-- settled, retired or expired as one.
CREATE TABLE credits.authorization_lines (
    authorization_id text NOT NULL,
    position integer NOT NULL,
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
    CONSTRAINT authorization_lines_pkey PRIMARY KEY (authorization_id, position),
    CONSTRAINT authorization_lines_authorization_fkey FOREIGN KEY (authorization_id)
        REFERENCES credits.authorizations (id) ON DELETE CASCADE,
    CONSTRAINT authorization_lines_quoted_check CHECK (quoted_cents > 0),
    CONSTRAINT authorization_lines_charged_check CHECK (charged_cents >= quoted_cents AND charged_cents % 10 = 0),
    CONSTRAINT authorization_lines_sale_kind_check CHECK (sale_kind IN ('primary_mint', 'primary_name_registration')),
    CONSTRAINT authorization_lines_asset_kind_check CHECK (asset_kind IN ('wearable', 'emote', 'name')),
    CONSTRAINT authorization_lines_asset_contract_check CHECK (asset_contract ~ '^0x[0-9a-f]{40}$'),
    CONSTRAINT authorization_lines_position_check CHECK (position >= 0 AND position < 30)
);

-- One ledger effect per CreditUsed log: the exact-once key of settlement
-- (src/shop/credits/settlement.rs). The first log of a salt settles the
-- purchase (`kind = 'purchase'`: the credit's charged usd_cents, the debit
-- the group was quoted); any later log of the same salt is a remainder the
-- buyer spent directly (`kind = 'remainder'`: `value_wei` at the credit's
-- oracle rate, rounded up to whole credits), charged like the purchase so
-- no signed value is ever free (docs/shop/CONTRACTS.md, "No spendable
-- remainder"). `tx_ref` is the ledger spend's reference
-- (`credit-used:<salt>:<block>:<log>`); a replay finds this row and moves
-- nothing. A debit the balance could not cover has no row and is retried by
-- the reconciler.
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
    settled_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    CONSTRAINT usage_settlements_pkey PRIMARY KEY (chain_id, block_number, log_index),
    CONSTRAINT usage_settlements_address_check CHECK (address ~ '^0x[0-9a-f]{40}$'),
    CONSTRAINT usage_settlements_kind_check CHECK (kind IN ('purchase', 'remainder')),
    CONSTRAINT usage_settlements_debited_check CHECK (debited_credits >= 0)
);

-- A credit's settlements (the purchases history's txHash and settled MANA, the reconciler's per-credit sum).
CREATE INDEX usage_settlements_authorization_idx ON credits.usage_settlements (authorization_id, block_number, log_index);

-- The revocation outbox: a credit hash to `revokeCredits` on the manager
-- (docs/shop/CONTRACTS.md: as soon as a credit's first CreditUsed is
-- indexed, and on a refund), drained by the `credits-revocations` job with
-- the `CREDITS_REVOKER_KEY_FILE` signer when one is configured. The row is
-- written in the transaction that decides the revocation; the broadcast
-- runs outside it, `tx_hash` is stored before `eth_sendRawTransaction`
-- (a kill between the two leaves a pollable row; re-sending revokeCredits
-- for an already revoked hash is a no-op on chain).
CREATE TABLE credits.revocations (
    credit_hash text NOT NULL,
    authorization_id text NOT NULL,
    chain_id bigint NOT NULL,
    contract text NOT NULL,
    reason text NOT NULL,
    status text NOT NULL DEFAULT 'pending',
    tx_hash text,
    attempts integer NOT NULL DEFAULT 0,
    last_error text,
    created_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    updated_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    CONSTRAINT revocations_pkey PRIMARY KEY (credit_hash),
    CONSTRAINT revocations_credit_hash_check CHECK (credit_hash ~ '^0x[0-9a-f]{64}$'),
    CONSTRAINT revocations_contract_check CHECK (contract ~ '^0x[0-9a-f]{40}$'),
    CONSTRAINT revocations_status_check CHECK (status IN ('pending', 'sent', 'confirmed', 'failed')),
    CONSTRAINT revocations_attempts_check CHECK (attempts >= 0)
);

-- The job's due set: unfinished revocations, oldest first.
CREATE INDEX revocations_due_idx ON credits.revocations (created_at) WHERE status IN ('pending', 'sent');

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
        'content', 'telemetry', 'credits', 'rpc', 'registry',
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
-- The CreditUsed usage index (src/credits/usage.rs): the CreditsManager contract's
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

-- ==== section: ops ====
-- (ops agent: a persisted job report or sync-control row if a restart must
-- keep them; both are process state today)
-- ==== end section: ops ====
