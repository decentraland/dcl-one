-- A per-token leg cleared on chain (an UpdateOperator or Approval naming the
-- zero address, the latest event of its leg) keeps the zero address, as the
-- land-manager subgraph stores it; the fold used to store NULL. Rewrite the
-- rows folded before, once; a later Transfer still clears the leg to NULL.
UPDATE land_authz.token_right tr SET update_operator = '0x0000000000000000000000000000000000000000'
    FROM (SELECT DISTINCT ON (e.token_address, e.token_id) e.token_address, e.token_id, e.kind, e.operator
          FROM land_authz.authz_event e
          WHERE e.token_id IS NOT NULL AND e.kind IN ('update_operator', 'transfer')
          ORDER BY e.token_address, e.token_id, e.block_number DESC, e.log_index DESC) l
    WHERE l.token_address = tr.token_address AND l.token_id = tr.token_id
      AND l.kind = 'update_operator' AND l.operator = '0x0000000000000000000000000000000000000000'
      AND tr.update_operator IS NULL;
UPDATE land_authz.token_right tr SET operator = '0x0000000000000000000000000000000000000000'
    FROM (SELECT DISTINCT ON (e.token_address, e.token_id) e.token_address, e.token_id, e.kind, e.operator
          FROM land_authz.authz_event e
          WHERE e.token_id IS NOT NULL AND e.kind IN ('approval', 'transfer')
          ORDER BY e.token_address, e.token_id, e.block_number DESC, e.log_index DESC) l
    WHERE l.token_address = tr.token_address AND l.token_id = tr.token_id
      AND l.kind = 'approval' AND l.operator = '0x0000000000000000000000000000000000000000'
      AND tr.operator IS NULL;
