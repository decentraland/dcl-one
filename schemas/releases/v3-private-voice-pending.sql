-- Schema release v3: Social private pending/media coexistence and exact-key
-- community voice notification facts. Stop all writers/pollers/admin paths
-- and verify a backup restore. Accept shipped main v2 (no historical 0020,
-- no legacy notification layout) or genuinely prepared v2 (matching real
-- 0020 receipt and complete legacy layout). Never manufacture old columns or
-- receipts. Run psql -v ON_ERROR_STOP=1 -f this-file. Relative includes share
-- canonical CREATE bytes with fresh bootstrap; backfill, retirement, replacement
-- private indexes, UNIQUE drops and final stamp 3 commit atomically.
-- Afterwards run v3 `catalyst schema migrate` for the remaining additive layout.
-- Old binaries refuse stamp 3. No pending/media row or credential is rewritten.
-- Once coexistence is written, rollback is quiesce + forward fix or a reviewed
-- restore: never delete/deduplicate calls to reinstall the former UNIQUEs.
\set ON_ERROR_STOP on
BEGIN;
SET LOCAL lock_timeout = '15s';
LOCK TABLE meta.schema, social.communities, social.private_voice_chats,
    social.private_voice_chat_participants IN ACCESS EXCLUSIVE MODE;
DO $$
DECLARE incomplete_legacy boolean;
BEGIN
    IF (SELECT version FROM meta.schema WHERE id = 1 AND identity = 'catalyst') IS DISTINCT FROM 2 THEN
        RAISE EXCEPTION 'schema release v3 requires catalyst version 2';
    END IF;
    IF to_regclass('meta.schema_steps') IS NULL THEN
        RAISE EXCEPTION 'schema release v3 requires shipped or prepared v2 migration ledger; run the source v2 schema migrate before cutover; no data changed';
    END IF;
    IF EXISTS (SELECT 1 FROM meta.schema_steps WHERE step='0020_social_voice_notifications'
        AND sha256<>'41c5172fe07ee74acad62a9e1fa3ae74e5eaeb51d2a13262146f20034faaef17') THEN
        RAISE EXCEPTION 'schema release v3 historical 0020 digest differs; no data changed';
    END IF;
    IF EXISTS (SELECT 1 FROM meta.schema_steps WHERE step='0020_social_voice_notifications') THEN
        IF (SELECT count(*) FROM pg_attribute WHERE attrelid='social.communities'::regclass AND NOT attisdropped AND attname IN ('voice_notification_scope','voice_notification_generation','voice_notification_expires_at')) <> 3
           OR NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conrelid='social.communities'::regclass AND conname='communities_voice_notification_scope_check' AND convalidated)
           OR to_regclass('social.communities_voice_notifications_idx') IS NULL THEN
            RAISE EXCEPTION 'schema release v3 requires the complete prepared v2 notification layout with its historical 0020 receipt; no data changed';
        END IF;
    ELSIF EXISTS (SELECT 1 FROM pg_attribute WHERE attrelid='social.communities'::regclass AND NOT attisdropped AND attname IN ('voice_notification_scope','voice_notification_generation','voice_notification_expires_at'))
        OR EXISTS (SELECT 1 FROM pg_constraint WHERE conrelid='social.communities'::regclass AND conname='communities_voice_notification_scope_check')
        OR to_regclass('social.communities_voice_notifications_idx') IS NOT NULL THEN
        RAISE EXCEPTION 'schema release v3 found legacy notification layout without its historical 0020 receipt; no data changed';
    END IF;
    IF to_regclass('social.community_voice_notifications') IS NOT NULL THEN
        RAISE EXCEPTION 'schema release v3 destination already exists; inspect retained state before cutover; no data changed';
    END IF;
    -- Both former role constraints must be exactly the reviewed v2 keys.
    -- Refuse extra UNIQUE indexes too: dropping only the known constraints must
    -- not leave hidden role uniqueness in the version-3 layout.
    IF (SELECT count(*)
        FROM (VALUES ('private_voice_chats_caller_key','caller_address'),
                     ('private_voice_chats_callee_key','callee_address')) expected(name, column_name)
        JOIN pg_constraint c ON c.conrelid='social.private_voice_chats'::regclass
            AND c.conname=expected.name AND c.contype='u' AND c.convalidated
        JOIN pg_attribute a ON a.attrelid=c.conrelid AND a.attname=expected.column_name
            AND NOT a.attisdropped AND c.conkey=ARRAY[a.attnum]
        JOIN pg_index i ON i.indexrelid=c.conindid AND i.indrelid=c.conrelid
            AND i.indisunique AND i.indisvalid AND i.indisready
            AND i.indnkeyatts=1 AND i.indnatts=1 AND i.indkey[0]=a.attnum
            AND i.indpred IS NULL AND i.indexprs IS NULL
    ) <> 2 THEN
        RAISE EXCEPTION 'schema release v3 requires the named v2 caller/callee UNIQUE constraints with their expected keys; no data changed';
    END IF;
    IF EXISTS (SELECT 1 FROM pg_index i
        WHERE i.indrelid='social.private_voice_chats'::regclass
          AND i.indisunique AND NOT i.indisprimary
          AND NOT EXISTS (SELECT 1 FROM pg_constraint c WHERE c.conrelid=i.indrelid
              AND c.conindid=i.indexrelid AND c.contype='u'
              AND c.conname IN ('private_voice_chats_caller_key','private_voice_chats_callee_key'))
    ) THEN
        RAISE EXCEPTION 'schema release v3 found unexpected private voice UNIQUE indexes; inspect retained state before cutover; no data changed';
    END IF;
    IF EXISTS (SELECT 1 FROM meta.schema_steps WHERE step='0020_social_voice_notifications') THEN
        EXECUTE $audit$
            SELECT EXISTS (SELECT 1 FROM social.communities
                WHERE (voice_notification_generation IS NULL
                    AND (voice_notification_scope IS NOT NULL OR voice_notification_expires_at IS NOT NULL))
                   OR (voice_notification_generation IS NOT NULL
                    AND (voice_notification_scope IS NULL OR voice_notification_expires_at IS NULL)))
        $audit$ INTO incomplete_legacy;
        IF incomplete_legacy THEN
            RAISE EXCEPTION 'schema release v3 found incomplete legacy notification facts; no data changed';
        END IF;
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
\ir v3/41_social_voice_notifications.sql
-- Cutover boundary: destination created.
DO $transfer$
DECLARE mismatch boolean;
BEGIN
    IF EXISTS (SELECT 1 FROM meta.schema_steps WHERE step='0020_social_voice_notifications') THEN
        -- Dynamic statements are planned only in the genuine prepared branch;
        -- shipped main v2 has no legacy columns and never creates them.
        EXECUTE $copy$
            INSERT INTO social.community_voice_notifications
                (raw_community_id, community_id, notification_scope, generation, expires_at)
            SELECT id::text, id, voice_notification_scope, voice_notification_generation, voice_notification_expires_at
            FROM social.communities WHERE voice_notification_generation IS NOT NULL
        $copy$;
        -- Legacy facts retain canonical spelling, generation and exact TTL,
        -- including expired TTLs. No raw alias can be recovered from UUIDs.
        EXECUTE $verify$
            SELECT EXISTS (
                SELECT id::text, id, voice_notification_scope, voice_notification_generation, voice_notification_expires_at
                FROM social.communities WHERE voice_notification_generation IS NOT NULL
                EXCEPT SELECT raw_community_id, community_id, notification_scope, generation, expires_at
                FROM social.community_voice_notifications
            ) OR EXISTS (
                SELECT raw_community_id, community_id, notification_scope, generation, expires_at
                FROM social.community_voice_notifications
                EXCEPT SELECT id::text, id, voice_notification_scope, voice_notification_generation, voice_notification_expires_at
                FROM social.communities WHERE voice_notification_generation IS NOT NULL
            )
        $verify$ INTO mismatch;
        IF mismatch THEN
            RAISE EXCEPTION 'schema release v3 notification backfill mismatch; no data changed';
        END IF;
    END IF;
END $transfer$;
-- Cutover boundary: legacy facts verified.
DO $retire$
BEGIN
    IF EXISTS (SELECT 1 FROM meta.schema_steps WHERE step='0020_social_voice_notifications') THEN
        DROP INDEX social.communities_voice_notifications_idx;
        ALTER TABLE social.communities DROP CONSTRAINT communities_voice_notification_scope_check;
        ALTER TABLE social.communities DROP COLUMN voice_notification_scope,
            DROP COLUMN voice_notification_generation, DROP COLUMN voice_notification_expires_at;
    END IF;
END $retire$;
-- These sole canonical definitions are shared with fresh bootstrap. Both
-- address access paths exist before the old UNIQUEs and final stamp change.
\ir v3/42_social_private_voice_indexes.sql
-- Cutover boundary: replacement indexes created.
ALTER TABLE social.private_voice_chats DROP CONSTRAINT private_voice_chats_caller_key;
ALTER TABLE social.private_voice_chats DROP CONSTRAINT private_voice_chats_callee_key;
DO $postcondition$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_class
        WHERE oid=to_regclass('social.community_voice_notifications') AND relkind='r') THEN
        RAISE EXCEPTION 'schema release v3 requires the notification destination table before stamp; no data changed';
    END IF;
    IF (SELECT count(*)
        FROM (VALUES ('private_voice_chats_caller_idx','caller_address'),
                     ('private_voice_chats_callee_idx','callee_address')) expected(name, column_name)
        JOIN pg_class c ON c.relnamespace='social'::regnamespace AND c.relname=expected.name
        JOIN pg_index i ON i.indexrelid=c.oid AND i.indrelid='social.private_voice_chats'::regclass
            AND NOT i.indisunique AND i.indisvalid AND i.indisready
            AND i.indnkeyatts=1 AND i.indnatts=1 AND i.indpred IS NULL AND i.indexprs IS NULL
        JOIN pg_attribute a ON a.attrelid=i.indrelid AND a.attname=expected.column_name
            AND NOT a.attisdropped AND i.indkey[0]=a.attnum
        JOIN pg_am am ON am.oid=c.relam AND am.amname='btree'
    ) <> 2 THEN
        RAISE EXCEPTION 'schema release v3 requires both valid replacement private voice indexes before stamp; no data changed';
    END IF;
END $postcondition$;
UPDATE meta.schema SET version = 3, applied_at = timezone('UTC', now()) WHERE id = 1;
COMMIT;
