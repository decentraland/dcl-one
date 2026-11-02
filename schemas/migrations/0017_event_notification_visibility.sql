-- Event producers preserve email delivery when a recipient opts out of the
-- in-app channel. Existing notifications keep their current visibility.
-- New intent relation is installed by schema migrate's object pass
-- from schemas/40_social.sql; the live-event index comes from 10_explore.sql.
ALTER TABLE IF EXISTS social.notifications ADD COLUMN IF NOT EXISTS in_app boolean NOT NULL DEFAULT true;
