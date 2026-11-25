-- Creator-signed coupons. Hex and addresses lowercased as the node's insert
-- path stores them; `checks`, `root` and `collections` copied as stored.
SELECT c.id,
       c.network,
       c.chain_id,
       lower(c.signer) AS signer,
       lower(c.signature) AS signature,
       lower(c.hashed_signature) AS hashed_signature,
       lower(c.state_key) AS state_key,
       lower(c.coupon_manager) AS coupon_manager,
       lower(c.coupon_address) AS coupon_address,
       c.checks,
       c.discount_type,
       c.discount_ppm,
       c.root,
       c.collections,
       c.effective_since,
       c.expires_at,
       c.created_at
FROM marketplace.coupons c
