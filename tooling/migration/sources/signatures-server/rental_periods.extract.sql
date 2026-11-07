-- The offered periods of each listing; immutable once posted.
SELECT p.id,
       p.rental_id,
       p.min_days,
       p.max_days,
       p.price_per_day
FROM periods p
