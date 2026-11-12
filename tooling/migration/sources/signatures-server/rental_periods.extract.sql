-- The offered periods of each listing; immutable once posted.
SELECT p.id,
       p.rental_id,
       p.min_days,
       p.max_days,
       p.price_per_day,
       r.signature AS parent_signature,
       r.updated_at AT TIME ZONE 'UTC' AS parent_updated_at
FROM periods p
JOIN rentals r ON r.id = p.rental_id
