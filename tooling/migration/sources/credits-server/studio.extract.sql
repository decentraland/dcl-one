-- Every studio (credits-server's 1786700000000_studios), times from epoch milliseconds.
SELECT s.id, s.name, s.status, s.created_by,
       to_timestamp(s.created_at / 1000.0) AS created_at,
       to_timestamp(s.updated_at / 1000.0) AS updated_at
FROM studios s
