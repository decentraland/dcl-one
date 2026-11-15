//! Collections v2 facts the shop indexes and the badge processor share.

/// 2^216: a v2 collection token id is `item << 216 | issued`.
pub const ITEM_SHIFT: &str = "105312291668557186697918027683670432318895095400549111254310977536";

/// How long after a sale the items mirror must complete a pass before a
/// sale whose item it lacks is passed (a pass takes well under it: 50 pages
/// of 1,000 items every 300 s by default).
pub const ITEM_WAIT_SECS: i64 = 3_600;
