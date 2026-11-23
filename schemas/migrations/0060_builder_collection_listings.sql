ALTER TABLE builder.collections ADD COLUMN IF NOT EXISTS linked_contract_address text, ADD COLUMN IF NOT EXISTS linked_contract_network text;
ALTER TABLE builder.collections ADD CONSTRAINT collections_linked_contract_check CHECK (linked_contract_address IS NULL OR linked_contract_address ~ '^0x[0-9a-f]{40}$');
ALTER TABLE builder.collections ADD CONSTRAINT collections_linked_network_check CHECK (linked_contract_network IS NULL OR linked_contract_network IN ('mainnet', 'sepolia', 'matic', 'amoy', 'base_mainnet', 'base_sepolia', 'monad_mainnet', 'monad_testnet', 'ape_mainnet', 'ape_caldera'));
ALTER TABLE builder.collections ADD CONSTRAINT collections_linked_contract_key UNIQUE (third_party_id, linked_contract_address, linked_contract_network);
