-- Schema release v3: pending private-call uniqueness is enforced by the
-- normalized, ordered advisory-lock start path rather than caller/callee
-- UNIQUEs that also forbid retained media history. No row or column changes.
-- Stop every writer, verify a backup restore, then run with ON_ERROR_STOP on
-- a database stamped version 2. Run `catalyst schema migrate` using the v3
-- binary afterwards to create the canonical nonunique address indexes; verify
-- both indexes before traffic resumes. Old binaries refuse the version-3 stamp.
-- Once coexistence is written, rollback is quiesce + forward fix or a reviewed
-- restore: never delete/deduplicate calls to reinstall the former UNIQUEs.
BEGIN;
SET LOCAL lock_timeout = '15s';
LOCK TABLE meta.schema, social.private_voice_chats,
    social.private_voice_chat_participants IN ACCESS EXCLUSIVE MODE;
DO $$
BEGIN
    IF (SELECT version FROM meta.schema WHERE id = 1 AND identity = 'catalyst') IS DISTINCT FROM 2 THEN
        RAISE EXCEPTION 'schema release v3 requires catalyst version 2';
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
ALTER TABLE social.private_voice_chats DROP CONSTRAINT IF EXISTS private_voice_chats_caller_key;
ALTER TABLE social.private_voice_chats DROP CONSTRAINT IF EXISTS private_voice_chats_callee_key;
UPDATE meta.schema SET version = 3, applied_at = timezone('UTC', now()) WHERE id = 1;
COMMIT;
