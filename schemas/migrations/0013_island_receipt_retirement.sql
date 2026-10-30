-- Local disposal of exact positively confirmed obsolete completion obligations.
-- The wallet floor and immutable attempt remain authoritative; no remote ack.
ALTER TABLE social.island_takeover_receipts ADD COLUMN IF NOT EXISTS retired_by_source text;
ALTER TABLE social.island_takeover_receipts ADD CONSTRAINT island_takeover_receipts_retirement_check CHECK (retired_by_source IS NULL OR
    (retired_by_source <> '' AND retired_by_source <> source_incarnation AND NOT acknowledged AND latest_dispatch IS NOT NULL));
