-- Rebuild the supporting claim index for the operator/submission/edit order.
-- All queued intents are retained; the object pass creates the current index.
DROP INDEX IF EXISTS explore.event_moderation_hooks_due_idx;
