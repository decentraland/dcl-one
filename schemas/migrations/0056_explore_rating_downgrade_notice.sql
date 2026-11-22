-- Admit the places notices of a creator's laxer rating, which the row keeps
-- out, and of an operator rating a new LAND owner's place released.
ALTER TABLE IF EXISTS explore.moderation_hooks DROP CONSTRAINT IF EXISTS moderation_hooks_kind_check;
ALTER TABLE IF EXISTS explore.moderation_hooks ADD CONSTRAINT moderation_hooks_kind_check CHECK (
    CASE channel
        WHEN 'events' THEN kind IN ('submitted','edited','approved','rejected','deleted','profile_permissions','upcoming')
        WHEN 'places' THEN kind IN ('disabled','rating_upgraded','rating_downgrade_refused','operator_rating_released')
    END);
