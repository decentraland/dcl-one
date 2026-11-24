ALTER TABLE builder.collections DROP CONSTRAINT IF EXISTS collections_contract_address_lowercase_check;
ALTER TABLE builder.collections ADD CONSTRAINT collections_contract_address_lowercase_check CHECK (contract_address = lower(contract_address));
