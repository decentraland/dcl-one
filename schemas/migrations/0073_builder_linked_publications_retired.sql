UPDATE builder.collections SET publication_pending = false
WHERE third_party_id IS NOT NULL AND publication_pending;
