-- Imported flags carry this source; flags raised on the node carry another
-- and are never touched here.
-- Upstream unflag deletes its row, so an imported flag missing upstream goes.
DELETE FROM credits.wallet_flags f
WHERE f.source = 'credits-server:multi-account'
  AND NOT EXISTS (SELECT 1 FROM stage s WHERE s.address = f.address);

-- A flag lifted on the node (unflag or unblock) after upstream raised it stays lifted.
INSERT INTO credits.wallet_flags AS f (address, source, ip, linked_addresses, flagged_at)
SELECT s.address, 'credits-server:multi-account', s.ip, s.linked_addresses, s.flagged_at AT TIME ZONE 'UTC'
FROM stage s
WHERE NOT EXISTS (SELECT 1 FROM ops.admin_audit a
                   WHERE a.domain = 'credits' AND a.action IN ('user.unflag', 'user.unblock')
                     AND a.target = s.address AND a.at >= s.flagged_at AT TIME ZONE 'UTC')
ON CONFLICT (address) DO UPDATE SET
    ip = EXCLUDED.ip,
    linked_addresses = EXCLUDED.linked_addresses,
    flagged_at = EXCLUDED.flagged_at
WHERE f.source = EXCLUDED.source
  AND (f.ip, f.linked_addresses, f.flagged_at)
      IS DISTINCT FROM (EXCLUDED.ip, EXCLUDED.linked_addresses, EXCLUDED.flagged_at);
