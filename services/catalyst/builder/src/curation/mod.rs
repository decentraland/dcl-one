//! The curation rows: a collection's reviews (`builder.collection_curations`),
//! an item's submissions for review (`builder.item_curations`), a
//! collection's review events and a third-party publication's slot cheques.
//! A row's state is the row itself: an item's curation status is its newest
//! item curation's, read where it is needed, never copied onto the item.

/// A boolean over an item aliased `i`: the item was submitted for curation.
/// A third-party item is published once this holds.
pub const ITEM_CURATED: &str =
    "EXISTS (SELECT 1 FROM builder.item_curations curated WHERE curated.item_id = i.id)";

/// `column`, an expression over `k`, of the newest curation of the item
/// `item`, NULL when the item has none. The newest curation is the last
/// created one; every read of an item's curation state goes through this.
pub fn newest_item_curation(item: &str, column: &str) -> String {
    format!(
        "(SELECT {column} FROM builder.item_curations k WHERE k.item_id = {item} \
         ORDER BY k.created_at DESC LIMIT 1)"
    )
}

/// `column`, an expression over `k`, of the newest curation of the first
/// curated item, by item id, of the collection `collection`: the curation
/// builder-server reads for a third-party collection and for an item of it
/// read by itself.
pub fn first_curated_item_curation(collection: &str, column: &str) -> String {
    format!(
        "(SELECT {} FROM builder.items ki WHERE ki.collection_id = {collection} \
         AND EXISTS (SELECT 1 FROM builder.item_curations kc WHERE kc.item_id = ki.id) \
         ORDER BY ki.id LIMIT 1)",
        newest_item_curation("ki.id", column)
    )
}
