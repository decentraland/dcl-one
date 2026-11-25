ALTER TABLE credits.authorization_lines DROP CONSTRAINT IF EXISTS authorization_lines_sale_kind_check;
ALTER TABLE credits.authorization_lines ADD CONSTRAINT authorization_lines_sale_kind_check CHECK (sale_kind IN ('primary_mint', 'primary_name_registration', 'publication_fee'));
ALTER TABLE credits.authorization_lines DROP CONSTRAINT IF EXISTS authorization_lines_asset_kind_check;
ALTER TABLE credits.authorization_lines ADD CONSTRAINT authorization_lines_asset_kind_check CHECK (asset_kind IN ('wearable', 'emote', 'name', 'collection'));
