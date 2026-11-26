//! Collections v2 facts the shop indexes and the badge processor share.

/// 2^216: a v2 collection token id is `item << 216 | issued`.
pub const ITEM_SHIFT: &str = "105312291668557186697918027683670432318895095400549111254310977536";

/// How long after a sale the items mirror must complete a pass before a
/// sale whose item it lacks is passed (a pass takes well under it: 50 pages
/// of 1,000 items every 300 s by default).
pub const ITEM_WAIT_SECS: i64 = 3_600;

/// How old a sale may grow while it waits for its item under the mirrors:
/// past it the sale is passed even when the mirror is off or stalled, so a
/// cursor never freezes on a stalled mirror. On a network the collections
/// index owns there is no such bound (`ItemWait::decide`).
pub const ITEM_HOLD_MAX_SECS: i64 = 86_400;

/// Whether the items mirror's last complete pass (`items_pass`, unix
/// seconds) ran [`ITEM_WAIT_SECS`] after a sale at `block_time`: an item it
/// still lacks then is not one it will ever hold.
pub fn item_pass_after(items_pass: Option<i64>, block_time: i64) -> bool {
    items_pass.is_some_and(|done| done >= block_time.saturating_add(ITEM_WAIT_SECS))
}

/// Whether a sale at `block_time` whose item the mirror lacks still waits at
/// `now`: only while the items mirror runs, the sale is younger than
/// [`ITEM_HOLD_MAX_SECS`] and no complete pass ran [`ITEM_WAIT_SECS`] after
/// it ([`item_pass_after`]).
pub fn item_awaited(
    mirror_enabled: bool,
    items_pass: Option<i64>,
    block_time: i64,
    now: i64,
) -> bool {
    mirror_enabled
        && now.saturating_sub(block_time) < ITEM_HOLD_MAX_SECS
        && !item_pass_after(items_pass, block_time)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_sale_waits_only_while_the_mirror_may_still_bring_its_item() {
        let t = 1_700_000_000;
        assert!(item_awaited(true, None, t, t + 60));
        assert!(item_awaited(true, Some(t + ITEM_WAIT_SECS - 1), t, t + 60));
        assert!(
            !item_awaited(true, Some(t + ITEM_WAIT_SECS), t, t + 60),
            "a late complete pass"
        );
        assert!(!item_awaited(false, None, t, t + 60), "the mirror is off");
        assert!(
            !item_awaited(true, None, t, t + ITEM_HOLD_MAX_SECS),
            "a stalled mirror"
        );
    }
}
