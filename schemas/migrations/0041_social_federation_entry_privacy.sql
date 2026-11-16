-- Whether a federation entry was written while its community was public; the
-- public feeds and post bodies publish only those entries. The constant
-- default adds the column without rewriting the table. Entries logged before
-- this step take their community's privacy at the step: the log kept no
-- earlier record of it.
ALTER TABLE IF EXISTS social.community_signed_actions
    ADD COLUMN IF NOT EXISTS public_at_entry boolean NOT NULL DEFAULT true;
UPDATE social.community_signed_actions a SET public_at_entry = false
  FROM social.communities c
 WHERE c.id = a.community_uuid AND c.private AND a.public_at_entry;
