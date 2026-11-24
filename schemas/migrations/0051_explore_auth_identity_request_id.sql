-- The deep-link request UUID a parked identity is bound to; its pickup must
-- present the verifier it was derived from. Existing rows stay unbound.
ALTER TABLE IF EXISTS explore_api.auth_identities
    ADD COLUMN IF NOT EXISTS auth_request_id text;
