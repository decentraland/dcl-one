-- The old events service's web-push subscriptions (ProfileSubscription/model.ts:
-- table profile_subscriptions, primary key endpoint, no timestamps). Nothing
-- sends pushes: the three routes keep the rows and the owner's
-- `notify_by_browser` flag in step for the old clients that still call them.
--
-- Workload: one row per browser a wallet subscribed from (a handful per
-- wallet, low thousands overall); writes are a wallet's own POST/DELETE,
-- reads are that wallet's rows on its own settings and subscription GETs.
-- The endpoint is a browser push URL and unique across wallets; the user
-- index serves every read and the DELETE of a wallet's rows.
CREATE TABLE explore.event_profile_subscriptions (
    endpoint   text PRIMARY KEY,
    "user"     text NOT NULL,
    p256dh     text NOT NULL,
    auth       text NOT NULL,
    created_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
    CONSTRAINT event_profile_subscriptions_user_lowercase CHECK ("user" = lower("user")),
    CONSTRAINT event_profile_subscriptions_endpoint_plain CHECK (length(endpoint) BETWEEN 1 AND 2048)
);
CREATE INDEX idx_event_profile_subscriptions_user ON explore.event_profile_subscriptions ("user");
