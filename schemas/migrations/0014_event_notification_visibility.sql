-- Event producers preserve email delivery when a recipient opts out of the
-- in-app channel. Existing notifications keep their current visibility.
-- New intent/cursor relations are installed by schema migrate's object pass
-- from schemas/10_explore.sql and schemas/40_social.sql.
ALTER TABLE IF EXISTS social.notifications ADD COLUMN IF NOT EXISTS in_app boolean NOT NULL DEFAULT true;
