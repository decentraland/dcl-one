-- shop.notification_cursor admits `bid_received`: the producer that tells a
-- token's owner of each ERC721BidV2 `BidCreated` the `sales` index keeps
-- (shop.sale_event, Polygon). schemas/30_shop.sql declares it in the CHECK; a
-- database created before it re-declares the constraint here. The row is
-- seeded on the producer's first pass at the index's published block.
ALTER TABLE IF EXISTS shop.notification_cursor DROP CONSTRAINT IF EXISTS notification_cursor_producer_known;
ALTER TABLE IF EXISTS shop.notification_cursor ADD CONSTRAINT notification_cursor_producer_known CHECK (producer IN ('transfer_received', 'royalties_earned', 'rental_started', 'rental_ended', 'sales:traded:ethereum', 'sales:traded:matic', 'sales:order_successful:matic', 'sales:mint:matic', 'sales:bid_accepted:matic', 'royalties:bid_accepted:matic', 'bid_received'));
