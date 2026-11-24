ALTER TABLE builder.uploaded_contents DROP CONSTRAINT IF EXISTS uploaded_contents_size_check;
ALTER TABLE builder.uploaded_contents ADD CONSTRAINT uploaded_contents_size_check CHECK (size >= 0 AND size <= 250000000);
