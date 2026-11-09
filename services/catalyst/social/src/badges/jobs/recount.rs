//! Badges whose input is a count of rows the node keeps whole, recounted
//! against the stored progress instead of read as a log (the mirrors are
//! rewritten on every walk, so no position orders their changes):
//!
//! - emote creator / wearable designer: the creator's Polygon items in
//!   `shop.items` (upstream counted each `item-published` of the L2
//!   collections subgraph once by item id); a creator is recounted while
//!   their item count exceeds their steps;
//! - open for business, collection half: a Polygon collection by the creator
//!   in `shop.collections` (upstream's `collection-created`);
//! - referral: the referrer's accepted invitations in
//!   `social.referral_progress` (`tier_granted`), the steps set to that count
//!   (upstream's `steps++` per event double counted a redelivery).
//!
//! The mirrors are read once their first complete pass is recorded
//! (`shop.mirror_state`). Each recount's `badge_cursors.position` counts its
//! completed passes; the first pass (`0`) brings the stored progress up to
//! the node's rows without notifying, so a node never announces history.

use super::super::evaluators::{self as ev, BusinessStep};
use super::super::progress::Applied;
use super::{Pass, award, lock_cursor};
use crate::identity::Address;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Recount {
    EmoteCreator,
    WearableDesigner,
    Collections,
    Referrals,
}

impl Recount {
    pub const ALL: [Recount; 4] = [
        Recount::EmoteCreator,
        Recount::WearableDesigner,
        Recount::Collections,
        Recount::Referrals,
    ];

    pub fn source(self) -> &'static str {
        match self {
            Recount::EmoteCreator => "recount:emote_creator",
            Recount::WearableDesigner => "recount:wearable_designer",
            Recount::Collections => "recount:open_for_business",
            Recount::Referrals => "recount:referral",
        }
    }

    fn badge(self) -> &'static str {
        match self {
            Recount::EmoteCreator => ev::EMOTE_CREATOR,
            Recount::WearableDesigner => ev::WEARABLE_DESIGNER,
            Recount::Collections => ev::OPEN_FOR_BUSINESS,
            Recount::Referrals => ev::REFERRAL,
        }
    }

    fn mirror(self) -> Option<&'static str> {
        match self {
            Recount::EmoteCreator | Recount::WearableDesigner => Some("items"),
            Recount::Collections => Some("collections"),
            Recount::Referrals => None,
        }
    }

    /// The wallets whose stored progress is behind their rows: `$1` the
    /// badge, `$2` the batch; (address, count).
    fn candidates_sql(self) -> &'static str {
        match self {
            Recount::EmoteCreator => {
                "SELECT c.address, c.n FROM (SELECT lower(creator) AS address, count(*) AS n \
                   FROM shop.items WHERE network = 'matic' AND item_type LIKE 'emote%' GROUP BY 1) c \
                 LEFT JOIN social.user_badge_progress p ON p.address = c.address AND p.badge_id = $1 \
                 WHERE p.address IS NULL OR (p.completed_at IS NULL AND p.steps_done < c.n) \
                 ORDER BY c.address LIMIT $2"
            }
            Recount::WearableDesigner => {
                "SELECT c.address, c.n FROM (SELECT lower(creator) AS address, count(*) AS n \
                   FROM shop.items WHERE network = 'matic' AND item_type NOT LIKE 'emote%' GROUP BY 1) c \
                 LEFT JOIN social.user_badge_progress p ON p.address = c.address AND p.badge_id = $1 \
                 WHERE p.address IS NULL OR (p.completed_at IS NULL AND p.steps_done < c.n) \
                 ORDER BY c.address LIMIT $2"
            }
            Recount::Collections => {
                "SELECT c.address, 1::bigint FROM (SELECT DISTINCT lower(creator) AS address \
                   FROM shop.collections WHERE network = 'matic') c \
                 LEFT JOIN social.user_badge_progress p ON p.address = c.address AND p.badge_id = $1 \
                 WHERE p.address IS NULL OR (p.completed_at IS NULL \
                   AND (p.progress->'collection_submitted') IS DISTINCT FROM 'true'::jsonb) \
                 ORDER BY c.address LIMIT $2"
            }
            Recount::Referrals => {
                "SELECT c.address, c.n FROM (SELECT lower(referrer) AS address, count(*) AS n \
                   FROM social.referral_progress WHERE tier_granted GROUP BY 1) c \
                 LEFT JOIN social.user_badge_progress p ON p.address = c.address AND p.badge_id = $1 \
                 WHERE p.address IS NULL OR (p.completed_at IS NULL AND p.steps_done <> c.n) \
                 ORDER BY c.address LIMIT $2"
            }
        }
    }
}

pub(super) async fn run(pass: &Pass) -> anyhow::Result<u64> {
    let mut written = 0;
    for recount in Recount::ALL {
        written += run_one(pass, recount).await?;
    }
    Ok(written)
}

/// Batches of one recount until no wallet is behind or a batch writes
/// nothing (a candidate the evaluator cannot move, such as an imported
/// document whose flag is not `true`, stays one until its rows change);
/// returns the wallets written.
pub(crate) async fn run_one(pass: &Pass, recount: Recount) -> anyhow::Result<u64> {
    let mut written = 0;
    for _ in 0..super::MAX_BATCHES_PER_TICK {
        let (n, more) = batch(pass, recount).await?;
        written += n;
        if !more {
            break;
        }
    }
    Ok(written)
}

async fn batch(pass: &Pass, recount: Recount) -> Result<(u64, bool), sqlx::Error> {
    let mut tx = pass.pool.begin().await?;
    if let Some(mirror) = recount.mirror() {
        let ready: bool = sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM shop.mirror_state \
             WHERE mirror = $1 AND network = 'matic' AND last_complete_pass_at IS NOT NULL)",
        )
        .bind(mirror)
        .fetch_one(&mut *tx)
        .await?;
        if !ready {
            tx.rollback().await?;
            return Ok((0, false));
        }
    }
    let awardable: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM social.badge_definitions \
         WHERE id = $1 AND published_at IS NOT NULL AND archived_at IS NULL)",
    )
    .bind(recount.badge())
    .fetch_one(&mut *tx)
    .await?;
    if !awardable {
        tx.rollback().await?;
        return Ok((0, false));
    }
    let passes = match lock_cursor(&mut tx, recount.source()).await? {
        Some((passes, _)) => passes,
        None => {
            super::init_cursor(&mut tx, recount.source(), 0, 0).await?;
            0
        }
    };
    let notify = (passes > 0).then_some(pass.asset_base.as_str());
    let limit = pass.batch();
    let candidates: Vec<(String, i64)> =
        sqlx::query_as(sqlx::AssertSqlSafe(recount.candidates_sql()))
            .bind(recount.badge())
            .bind(limit)
            .fetch_all(&mut *tx)
            .await?;
    let mut written = 0;
    for (raw, count) in &candidates {
        let Ok(address) = Address::parse(raw) else {
            tracing::debug!(
                address = raw,
                source = recount.source(),
                "not a wallet: skipped"
            );
            continue;
        };
        let applied = match recount {
            Recount::EmoteCreator | Recount::WearableDesigner => {
                let emote = recount == Recount::EmoteCreator;
                let items: Vec<(String, i64)> = sqlx::query_as(
                    "SELECT contract_address || '-' || item_id::text, created_at * 1000 \
                     FROM shop.items WHERE network = 'matic' AND lower(creator) = $1 \
                       AND (item_type LIKE 'emote%') = $2 \
                     ORDER BY created_at, contract_address, item_id",
                )
                .bind(address.as_str())
                .bind(emote)
                .fetch_all(&mut *tx)
                .await?;
                let badge = recount.badge();
                award(&mut tx, &address, badge, notify, |p| {
                    ev::creator(p, badge, &items)
                })
                .await?
            }
            Recount::Collections => {
                award(&mut tx, &address, ev::OPEN_FOR_BUSINESS, notify, |p| {
                    ev::open_for_business(p, BusinessStep::Collection)
                })
                .await?
            }
            Recount::Referrals => {
                award(&mut tx, &address, ev::REFERRAL, notify, |p| {
                    ev::referral(p, *count)
                })
                .await?
            }
        };
        if matches!(applied, Applied::Written(_)) {
            written += 1;
        }
    }
    let more = candidates.len() as i64 == limit && written > 0;
    if !more {
        super::advance(&mut tx, recount.source(), passes + 1, 0).await?;
    }
    tx.commit().await?;
    Ok((written, more))
}
