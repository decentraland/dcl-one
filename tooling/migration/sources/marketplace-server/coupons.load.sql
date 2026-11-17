-- Upstream ids kept. A coupon whose signature or state key the node already
-- holds under another id is skipped; an id held with another signature is
-- left alone.
INSERT INTO shop.coupons
    (id, network, chain_id, signer, signature, hashed_signature, state_key, coupon_manager,
     coupon_address, checks, discount_type, discount_ppm, root, collections,
     effective_since, expires_at, created_at)
SELECT s.id, s.network, s.chain_id, s.signer, s.signature, s.hashed_signature, s.state_key,
       s.coupon_manager, s.coupon_address, s.checks, s.discount_type, s.discount_ppm, s.root,
       s.collections, s.effective_since AT TIME ZONE 'UTC', s.expires_at AT TIME ZONE 'UTC',
       s.created_at AT TIME ZONE 'UTC'
FROM stage s
WHERE NOT EXISTS (
    SELECT 1 FROM shop.coupons c
    WHERE (c.signature = s.signature OR c.hashed_signature = s.hashed_signature OR c.state_key = s.state_key)
      AND c.id <> s.id)
ON CONFLICT (id) DO UPDATE
   SET network = EXCLUDED.network, chain_id = EXCLUDED.chain_id, signer = EXCLUDED.signer,
       state_key = EXCLUDED.state_key, coupon_manager = EXCLUDED.coupon_manager,
       coupon_address = EXCLUDED.coupon_address, checks = EXCLUDED.checks,
       discount_type = EXCLUDED.discount_type, discount_ppm = EXCLUDED.discount_ppm,
       root = EXCLUDED.root, collections = EXCLUDED.collections,
       effective_since = EXCLUDED.effective_since, expires_at = EXCLUDED.expires_at,
       created_at = EXCLUDED.created_at
 WHERE shop.coupons.hashed_signature = EXCLUDED.hashed_signature;
