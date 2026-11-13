//! The one write path of a wallet's badge progress: the evaluators, the
//! backfills and the console grant all call [`apply_progress`] inside their
//! own transaction (the cursor advance or the audit row commits with it).
//!
//! In one transaction it takes the badge row `FOR SHARE` only when the badge
//! is awardable (published and not archived, upstream's
//! `grantProgressIfAwardable`), so an archive waits for an in-flight award or
//! the award reads it archived and writes nothing; serializes writers of the
//! same (address, badge) on a transaction advisory lock; hands the caller's
//! closure the stored progress document; then writes the document and
//! `steps_done`, the tiers the new steps reach (all of them, or as many as
//! the caller's [`TierGrant`] allows), completion once every tier is
//! achieved, and one `badge_granted` notification per newly achieved tier
//! (or per completion of a single badge) unless the caller suppresses them.
//! Achieved tiers and completion are never taken back.

use std::collections::HashSet;

use chrono::NaiveDateTime;
use serde_json::{Map, Value};
use sqlx::PgConnection;

use super::store::{badge_granted_notification, resolve_assets};
use crate::identity::Address;

/// The progress document of one (address, badge) as the closure edits it.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Progress {
    /// Upstream's `progress.steps`, stored as `steps_done` too: the counter
    /// tier targets (or a single badge's `criteria_steps`) compare against.
    pub steps: i32,
    /// The rest of upstream's document (`profiles_visited`,
    /// `transactions_emotes_purchase`, `store_completed`, `intervals`, ...),
    /// without `steps`. Stored verbatim.
    pub state: Map<String, Value>,
    /// The badge is already complete (read only: completion is never taken
    /// back). Upstream's evaluators leave a completed badge untouched.
    pub completed: bool,
}

impl Progress {
    fn from_document(steps: i32, document: Value) -> Self {
        let mut state = match document {
            Value::Object(map) => map,
            _ => Map::new(),
        };
        state.remove("steps");
        Self {
            steps,
            state,
            completed: false,
        }
    }

    /// Upstream's document: `state` plus `steps`.
    pub fn document(&self) -> Value {
        let mut doc = self.state.clone();
        doc.insert("steps".into(), Value::from(self.steps));
        Value::Object(doc)
    }
}

/// How one write is recorded.
#[derive(Debug, Clone, Copy)]
pub struct ApplyOptions<'a> {
    /// The instant of every row the write stamps (tiers, completion,
    /// notifications, `updated_at`): the event's time, or now.
    pub at: NaiveDateTime,
    /// `Some(asset base)` writes the `badge_granted` notifications with the
    /// tier image resolved against it ([`super::BadgesStore::asset_base_url`]);
    /// `None` writes none (backfills and imports never notify).
    pub notify: Option<&'a str>,
    /// The console actor of a grant; `None` for an evaluator. A grant writes
    /// even when nothing else changed, so the row records who granted it.
    pub granted_by: Option<&'a str>,
}

/// How many reached tiers one write achieves, lowest first.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TierGrant {
    /// Every reached tier (backfills, grants, `traveler.ts`'s `filter`).
    Reached,
    /// At most one: upstream's observers `find` one tier per event.
    OnePerWrite,
    /// At most one per step gained: a write standing for several upstream
    /// events of one step each (a recount).
    OnePerStep,
}

/// What [`apply_progress`] did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Applied {
    /// Unknown, unpublished or archived badge: nothing written.
    NotAwardable,
    /// The closure left the document as it was and no target was newly
    /// reached: nothing written.
    Unchanged,
    Written(ProgressChange),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgressChange {
    pub steps_before: i32,
    pub steps: i32,
    /// Tier ids achieved by this write, in tier order (empty for a single badge).
    pub new_tiers: Vec<String>,
    /// This write completed the badge (its last tier, or a single badge's target).
    pub completed: bool,
    /// `badge_granted` rows inserted (a repeated key inserts nothing).
    pub notified: u64,
}

struct Tier {
    tier_id: String,
    tier_name: String,
    steps: i32,
    assets: Value,
}

/// Applies `update` to the progress of `address` on `badge_id` inside the
/// caller's transaction (`conn` is `&mut *tx`); see the module docs. The
/// caller commits, so its own rows (a cursor, an audit row) land with it.
///
/// ```ignore
/// let mut tx = pool.begin().await?;
/// let applied = apply_progress(&mut tx, &address, "social_butterfly",
///     &ApplyOptions { at, notify: Some(store.asset_base_url()), granted_by: None },
///     |p| {
///         let seen = p.state.entry("profiles_visited").or_insert(json!([]));
///         // ... dedup, push, then:
///         p.steps = seen.as_array().map_or(0, |a| a.len() as i32);
///     }).await?;
/// tx.commit().await?;
/// ```
pub async fn apply_progress<F>(
    conn: &mut PgConnection,
    address: &Address,
    badge_id: &str,
    options: &ApplyOptions<'_>,
    update: F,
) -> Result<Applied, sqlx::Error>
where
    F: FnOnce(&mut Progress),
{
    apply_progress_granting(conn, address, badge_id, options, TierGrant::Reached, update).await
}

/// [`apply_progress`] achieving the reached tiers `grant` allows.
pub async fn apply_progress_granting<F>(
    conn: &mut PgConnection,
    address: &Address,
    badge_id: &str,
    options: &ApplyOptions<'_>,
    grant: TierGrant,
    update: F,
) -> Result<Applied, sqlx::Error>
where
    F: FnOnce(&mut Progress),
{
    let badge: Option<(bool, Option<i32>, String, Value)> = sqlx::query_as(
        "SELECT is_tier, criteria_steps, name, assets FROM social.badge_definitions \
         WHERE id = $1 AND published_at IS NOT NULL AND archived_at IS NULL \
         FOR SHARE",
    )
    .bind(badge_id)
    .fetch_optional(&mut *conn)
    .await?;
    let Some((is_tier, criteria_steps, badge_name, badge_assets)) = badge else {
        return Ok(Applied::NotAwardable);
    };
    let tiers: Vec<Tier> = if is_tier {
        sqlx::query_as::<_, (String, String, i32, Value)>(
            "SELECT tier_id, tier_name, criteria_steps, assets FROM social.badge_tiers \
             WHERE badge_id = $1 ORDER BY ordinal",
        )
        .bind(badge_id)
        .fetch_all(&mut *conn)
        .await?
        .into_iter()
        .map(|(tier_id, tier_name, steps, assets)| Tier {
            tier_id,
            tier_name,
            steps,
            assets,
        })
        .collect()
    } else {
        Vec::new()
    };
    if is_tier && tiers.is_empty() {
        return Ok(Applied::NotAwardable);
    }

    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
        .bind(format!("badge-progress:{}:{badge_id}", address.as_str()))
        .execute(&mut *conn)
        .await?;
    let stored: Option<(i32, Value, Option<NaiveDateTime>)> = sqlx::query_as(
        "SELECT steps_done, progress, completed_at FROM social.user_badge_progress \
         WHERE address = $1 AND badge_id = $2 FOR UPDATE",
    )
    .bind(address)
    .bind(badge_id)
    .fetch_optional(&mut *conn)
    .await?;
    let achieved: HashSet<String> = sqlx::query_scalar(
        "SELECT tier_id FROM social.user_achieved_tiers WHERE address = $1 AND badge_id = $2",
    )
    .bind(address)
    .bind(badge_id)
    .fetch_all(&mut *conn)
    .await?
    .into_iter()
    .collect();

    let (steps_before, document, completed_at) =
        stored.unwrap_or((0, Value::Object(Map::new()), None));
    let before = Progress {
        completed: completed_at.is_some(),
        ..Progress::from_document(steps_before, document)
    };
    let mut progress = before.clone();
    update(&mut progress);

    let cap = match grant {
        TierGrant::Reached => usize::MAX,
        TierGrant::OnePerWrite => 1,
        TierGrant::OnePerStep => {
            usize::try_from(progress.steps.saturating_sub(steps_before)).unwrap_or(0)
        }
    };
    let new_tiers: Vec<&Tier> = tiers
        .iter()
        .filter(|t| t.steps <= progress.steps && !achieved.contains(&t.tier_id))
        .take(cap)
        .collect();
    let held = |t: &Tier| {
        achieved.contains(&t.tier_id) || new_tiers.iter().any(|n| n.tier_id == t.tier_id)
    };
    let complete_now = completed_at.is_none()
        && if is_tier {
            tiers.iter().all(held)
        } else {
            progress.steps >= criteria_steps.unwrap_or(1)
        };
    if progress == before && new_tiers.is_empty() && !complete_now && options.granted_by.is_none() {
        return Ok(Applied::Unchanged);
    }

    let at = options.at;
    let last_tier = tiers
        .iter()
        .rev()
        .find(|t| held(t))
        .map(|t| t.tier_id.clone());
    for tier in &new_tiers {
        sqlx::query(
            "INSERT INTO social.user_achieved_tiers \
               (address, badge_id, tier_id, completed_at, granted_by, granted_at) \
             VALUES ($1, $2, $3, $4, $5, CASE WHEN $5::text IS NULL THEN NULL ELSE $4 END) \
             ON CONFLICT (address, badge_id, tier_id) DO NOTHING",
        )
        .bind(address)
        .bind(badge_id)
        .bind(&tier.tier_id)
        .bind(at)
        .bind(options.granted_by)
        .execute(&mut *conn)
        .await?;
    }
    sqlx::query(
        "INSERT INTO social.user_badge_progress \
           (address, badge_id, steps_done, progress, completed_at, last_completed_tier_id, \
            granted_by, updated_at) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8) \
         ON CONFLICT (address, badge_id) DO UPDATE SET \
           steps_done = EXCLUDED.steps_done, \
           progress = EXCLUDED.progress, \
           completed_at = COALESCE(social.user_badge_progress.completed_at, EXCLUDED.completed_at), \
           last_completed_tier_id = COALESCE(EXCLUDED.last_completed_tier_id, \
                                             social.user_badge_progress.last_completed_tier_id), \
           granted_by = COALESCE(EXCLUDED.granted_by, social.user_badge_progress.granted_by), \
           updated_at = EXCLUDED.updated_at",
    )
    .bind(address)
    .bind(badge_id)
    .bind(progress.steps)
    .bind(progress.document())
    .bind(if complete_now { Some(at) } else { completed_at })
    .bind(last_tier)
    .bind(options.granted_by)
    .bind(at)
    .execute(&mut *conn)
    .await?;

    let mut notified = 0;
    if let Some(base) = options.notify {
        let notices: Vec<_> = if is_tier {
            new_tiers
                .iter()
                .map(|t| {
                    badge_granted_notification(
                        address,
                        badge_id,
                        Some(&t.tier_id),
                        &format!("{badge_name} {}", t.tier_name),
                        &resolve_assets(&t.assets, base),
                        at,
                    )
                })
                .collect()
        } else if complete_now {
            vec![badge_granted_notification(
                address,
                badge_id,
                None,
                &badge_name,
                &resolve_assets(&badge_assets, base),
                at,
            )]
        } else {
            Vec::new()
        };
        if !notices.is_empty() {
            notified = crate::notifications::insert_notifications(&mut *conn, &notices).await?;
        }
    }

    Ok(Applied::Written(ProgressChange {
        steps_before,
        steps: progress.steps,
        new_tiers: new_tiers.iter().map(|t| t.tier_id.clone()).collect(),
        completed: complete_now,
        notified,
    }))
}

#[cfg(test)]
mod tests {
    use super::Progress;
    use serde_json::json;

    #[test]
    fn the_document_keeps_upstream_shape_with_steps_from_the_counter() {
        let p = Progress::from_document(3, json!({"steps": 1, "profiles_visited": ["a"]}));
        assert_eq!(p.steps, 3);
        assert_eq!(p.state.get("steps"), None);
        assert_eq!(p.document(), json!({"steps": 3, "profiles_visited": ["a"]}));
        assert_eq!(
            Progress::from_document(0, json!(null)).document(),
            json!({"steps": 0})
        );
    }
}
