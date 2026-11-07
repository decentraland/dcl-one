-- Only for coupons that landed; a newer reading the node took wins.
INSERT INTO shop.coupon_state (coupon_id, uses, cancelled, revoked, checked_at)
SELECT s.coupon_id, s.uses, s.cancelled, s.revoked, s.checked_at AT TIME ZONE 'UTC'
FROM stage s
JOIN shop.coupons c ON c.id = s.coupon_id
ON CONFLICT (coupon_id) DO UPDATE
   SET uses = EXCLUDED.uses, cancelled = EXCLUDED.cancelled,
       revoked = EXCLUDED.revoked, checked_at = EXCLUDED.checked_at
 WHERE shop.coupon_state.checked_at <= EXCLUDED.checked_at;
