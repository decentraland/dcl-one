\set ON_ERROR_STOP on
-- Retires the event hook tables of the earlier layouts. Run it once, with
-- the administrative role, after every replica runs a binary with the
-- moderation outbox and `catalyst schema migrate` recorded step
-- 0029_explore_parity; it refuses to run earlier.
--   explore.event_moderation_hooks: the events-only outbox. Its pending rows
--     move into explore.moderation_hooks first, as the job's adopt_legacy_rows
--     moves them; delivered and dead-letter rows are not kept.
--   explore.event_moderation_hook_activation: its activation instant; the
--     outbox keeps the earlier of the two.
--   explore.event_upcoming_hooks: the earlier upcoming-notice queue; the
--     reminder job queues that notice in the outbox and nothing reads it.
-- The drop is not reversible: rollback needs the matching backup.
--   psql "$DATABASE_URL" -X -f schemas/releases/explore-retired-hook-tables.sql
BEGIN;
DO $$
DECLARE
    moved bigint := 0;
BEGIN
    IF to_regclass('explore.moderation_hooks') IS NULL
       OR NOT EXISTS (SELECT 1 FROM meta.schema_steps WHERE step = '0029_explore_parity') THEN
        RAISE EXCEPTION 'run catalyst schema migrate to the explore parity layout first';
    END IF;
    IF to_regclass('explore.event_moderation_hook_activation') IS NOT NULL THEN
        INSERT INTO explore.moderation_hook_activation (activated_at)
        SELECT activated_at FROM explore.event_moderation_hook_activation WHERE singleton
        ON CONFLICT (singleton) DO UPDATE
            SET activated_at = LEAST(explore.moderation_hook_activation.activated_at, EXCLUDED.activated_at);
    END IF;
    IF to_regclass('explore.event_moderation_hooks') IS NOT NULL THEN
        WITH pending AS (
            DELETE FROM explore.event_moderation_hooks h
             WHERE h.completed_at IS NULL AND h.attempts < 6
            RETURNING h.id, COALESCE(h.subject, h.event_id) AS subject, h.kind, h.event,
                      h.creator_address, h.edit_bucket, h.created_at, h.next_attempt_at, h.attempts)
        INSERT INTO explore.moderation_hooks
            (id, channel, subject, kind, payload, creator_address, edit_bucket, created_at, next_attempt_at, attempts)
        SELECT id, 'events', subject, kind, event, creator_address, edit_bucket, created_at, next_attempt_at, attempts
          FROM pending WHERE subject IS NOT NULL
        ON CONFLICT DO NOTHING;
        GET DIAGNOSTICS moved = ROW_COUNT;
        IF moved > 0 THEN
            INSERT INTO explore.moderation_hook_activation (activated_at)
            SELECT min(created_at) FROM explore.moderation_hooks HAVING min(created_at) IS NOT NULL
            ON CONFLICT (singleton) DO UPDATE
                SET activated_at = LEAST(explore.moderation_hook_activation.activated_at, EXCLUDED.activated_at);
        END IF;
        RAISE NOTICE 'moved % pending notice(s) into explore.moderation_hooks', moved;
    END IF;
END $$;
DROP TABLE IF EXISTS explore.event_upcoming_hooks;
DROP TABLE IF EXISTS explore.event_moderation_hooks;
DROP TABLE IF EXISTS explore.event_moderation_hook_activation;
COMMIT;
