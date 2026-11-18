-- Every upstream opt-out, keyed and normalized as opt_out.extract.sql.
SELECT o.address AS source_address,
       o.scope,
       o.scope_id AS source_scope_id,
       lower(o.address) AS address,
       CASE WHEN o.scope = 'community'
             AND o.scope_id ~ '^\{?[0-9a-fA-F]{8}-?[0-9a-fA-F]{4}-?[0-9a-fA-F]{4}-?[0-9a-fA-F]{4}-?[0-9a-fA-F]{12}\}?$'
            THEN o.scope_id::uuid::text
            ELSE o.scope_id END AS scope_id
FROM notification_opt_outs o
WHERE lower(o.address) ~ '^0x[0-9a-f]{40}$'
