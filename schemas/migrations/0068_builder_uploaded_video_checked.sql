ALTER TABLE builder.uploaded_contents ADD COLUMN IF NOT EXISTS video boolean NOT NULL DEFAULT false;
