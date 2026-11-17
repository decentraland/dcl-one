-- What the CouponManager last reported per coupon. The node refreshes only
-- live coupons, so this snapshot is the only state an expired one gets.
SELECT cs.coupon_id,
       cs.uses,
       cs.cancelled,
       cs.revoked,
       cs.checked_at
FROM marketplace.coupon_state cs
