, proceeds AS (
    SELECT * FROM sold
    WHERE ($8::bigint IS NULL OR block_time*1000 >= $8) AND ($9::bigint IS NULL OR block_time*1000 <= $9)
)
