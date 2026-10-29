-- credits.purchases gained the pack quantity a checkout bought and the
-- surface that opened the Checkout Session (the Shop checkout, 2026-09-26);
-- both are declared on the table in schema/60_ops.sql, and a database
-- stamped before them takes them here. quantity backfills to 1: every
-- earlier purchase was one pack, and credits/amount_cents hold the totals.
ALTER TABLE credits.purchases ADD COLUMN IF NOT EXISTS quantity integer NOT NULL DEFAULT 1;
ALTER TABLE credits.purchases ADD COLUMN IF NOT EXISTS source text;
ALTER TABLE credits.purchases ADD CONSTRAINT purchases_quantity_check CHECK (quantity >= 1);
ALTER TABLE credits.purchases ADD CONSTRAINT purchases_source_check CHECK (source IN ('website', 'client', 'builder'));
