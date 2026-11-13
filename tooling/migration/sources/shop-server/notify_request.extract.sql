-- `updated_at` moves on every status change, so it is the watermark.
SELECT r.id,
       r.address,
       r.email,
       r.contract_address,
       r.item_id,
       r.chain_id,
       r.status,
       r.created_at,
       r.updated_at,
       r.notified_at
FROM item_notify_requests r
