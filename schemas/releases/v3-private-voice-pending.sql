-- Schema release v3: Social private pending/media coexistence and exact-key
-- community voice notification facts. Stop all writers/pollers/admin paths,
-- verify a backup restore, and prepare the version-2 database with a compatible
-- v2 binary (c1fab3991 includes historical step 0020). Its real schema migrate
-- must record 0020 before this release retires the old notification columns.
-- Run using psql -v ON_ERROR_STOP=1 -f this-file; the relative include shares
-- the canonical CREATE TABLE bytes with fresh bootstrap. Table creation,
-- exact backfill, retirement, UNIQUE drops and final stamp 3 are one transaction.
-- Afterwards run v3 `catalyst schema migrate` for canonical secondary indexes;
-- verify both private address indexes before traffic resumes. Old binaries
-- refuse the version-3 stamp. No pending/media row or credential is rewritten.
-- Once coexistence is written, rollback is quiesce + forward fix or a reviewed
-- restore: never delete/deduplicate calls to reinstall the former UNIQUEs.
BEGIN;
SET LOCAL lock_timeout = '15s';
LOCK TABLE meta.schema, social.communities, social.private_voice_chats,
    social.private_voice_chat_participants IN ACCESS EXCLUSIVE MODE;
DO $$
BEGIN
    IF (SELECT version FROM meta.schema WHERE id = 1 AND identity = 'catalyst') IS DISTINCT FROM 2 THEN
        RAISE EXCEPTION 'schema release v3 requires catalyst version 2';
    END IF;
    IF to_regclass('meta.schema_steps') IS NULL THEN
        RAISE EXCEPTION 'schema release v3 requires prepared v2 with historical 0020 recorded; run compatible v2 schema migrate before cutover; no data changed';
    END IF;
    IF NOT EXISTS (SELECT 1 FROM meta.schema_steps WHERE step='0020_social_voice_notifications' AND sha256='41c5172fe07ee74acad62a9e1fa3ae74e5eaeb51d2a13262146f20034faaef17') THEN
        RAISE EXCEPTION 'schema release v3 requires unchanged historical 0020 recorded by compatible v2 schema migrate; no data changed';
    END IF;
    IF (SELECT count(*) FROM pg_attribute WHERE attrelid='social.communities'::regclass AND NOT attisdropped AND attname IN ('voice_notification_scope','voice_notification_generation','voice_notification_expires_at')) <> 3
       OR NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conrelid='social.communities'::regclass AND conname='communities_voice_notification_scope_check' AND convalidated) THEN
        RAISE EXCEPTION 'schema release v3 requires the prepared v2 notification layout; no data changed';
    END IF;
    IF to_regclass('social.community_voice_notifications') IS NOT NULL THEN
        RAISE EXCEPTION 'schema release v3 destination already exists; inspect retained state before cutover; no data changed';
    END IF;
    -- A normalized pending wallet appears in only one role of one pending
    -- intent. Audit both roles together; the former independent UNIQUEs did
    -- not exclude a caller also appearing as another call''s callee.
    IF EXISTS (
        SELECT 1 FROM social.private_voice_chats c
        WHERE NOT EXISTS (SELECT 1 FROM social.private_voice_chat_participants p WHERE p.call_id = c.id)
          AND (c.caller_address <> lower(c.caller_address) OR c.callee_address <> lower(c.callee_address))
    ) THEN
        RAISE EXCEPTION 'schema release v3 found non-normalized pending wallets; resolve retained state before cutover; no data changed';
    END IF;
    IF EXISTS (
        SELECT address FROM (
            SELECT lower(caller_address) AS address FROM social.private_voice_chats c
            WHERE NOT EXISTS (SELECT 1 FROM social.private_voice_chat_participants p WHERE p.call_id = c.id)
            UNION ALL
            SELECT lower(callee_address) AS address FROM social.private_voice_chats c
            WHERE NOT EXISTS (SELECT 1 FROM social.private_voice_chat_participants p WHERE p.call_id = c.id)
        ) pending_parties
        GROUP BY address HAVING count(*) > 1
    ) THEN
        RAISE EXCEPTION 'schema release v3 found overlapping pending wallets; resolve retained state before cutover; no data changed';
    END IF;
END $$;
-- This is the sole canonical table definition; do not duplicate its DDL here.
\ir ../41_social_voice_notifications.sql
INSERT INTO social.community_voice_notifications
    (raw_community_id, community_id, notification_scope, generation, expires_at)
SELECT id::text, id, voice_notification_scope, voice_notification_generation, voice_notification_expires_at
FROM social.communities WHERE voice_notification_generation IS NOT NULL;
-- Only canonical spelling survives in the legacy facts. The original raw key
-- cannot be recovered; neither expired TTLs nor generations are renewed.
DO $$
BEGIN
    IF EXISTS (
        SELECT id::text, id, voice_notification_scope, voice_notification_generation, voice_notification_expires_at
        FROM social.communities WHERE voice_notification_generation IS NOT NULL
        EXCEPT SELECT raw_community_id, community_id, notification_scope, generation, expires_at
        FROM social.community_voice_notifications
    ) OR EXISTS (
        SELECT raw_community_id, community_id, notification_scope, generation, expires_at
        FROM social.community_voice_notifications
        EXCEPT SELECT id::text, id, voice_notification_scope, voice_notification_generation, voice_notification_expires_at
        FROM social.communities WHERE voice_notification_generation IS NOT NULL
    ) THEN
        RAISE EXCEPTION 'schema release v3 notification backfill mismatch; no data changed';
    END IF;
END $$;
DROP INDEX social.communities_voice_notifications_idx;
ALTER TABLE social.communities DROP CONSTRAINT communities_voice_notification_scope_check;
ALTER TABLE social.communities DROP COLUMN voice_notification_scope,
    DROP COLUMN voice_notification_generation, DROP COLUMN voice_notification_expires_at;
ALTER TABLE social.private_voice_chats DROP CONSTRAINT IF EXISTS private_voice_chats_caller_key;
ALTER TABLE social.private_voice_chats DROP CONSTRAINT IF EXISTS private_voice_chats_callee_key;
UPDATE meta.schema SET version = 3, applied_at = timezone('UTC', now()) WHERE id = 1;
COMMIT;
