-- Only for the same accepted parent version; stale children cannot overwrite
-- a newer node fold, even when the id is already present.
INSERT INTO signatures.rental_periods (id, rental_id, min_days, max_days, price_per_day)
SELECT s.id, s.rental_id, s.min_days, s.max_days, s.price_per_day
FROM stage s
JOIN signatures.rental_listings l ON l.id = s.rental_id
JOIN signatures.rentals r ON r.id = l.id
 AND r.signature = s.parent_signature
 AND r.updated_at = s.parent_updated_at AT TIME ZONE 'UTC'
ON CONFLICT (id) DO UPDATE
   SET min_days = EXCLUDED.min_days, max_days = EXCLUDED.max_days, price_per_day = EXCLUDED.price_per_day
 WHERE signatures.rental_periods.rental_id = EXCLUDED.rental_id;
