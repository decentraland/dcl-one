//! `worlds-owner-reconcile`: the fleet job that rewrites `worlds.owner` from
//! the name authority (the upstream's 12-hourly update-owner job), so the
//! listings, wallet stats and limits that read the column follow a NAME
//! transfer. Admission never trusts the column while an authority is
//! configured; this keeps the copy current and re-decides the previous
//! holder's place in the world's live rooms. A name the authority cannot
//! answer for, or says nobody holds, keeps its stored owner.

use futures::StreamExt;

use crate::comms::world_identity::WorldName;
use crate::shared::identity::Address;

use super::CoreState;

/// Worlds asked about at once.
const CONCURRENCY: usize = 4;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ReconcileReport {
    pub checked: usize,
    pub updated: usize,
    pub unresolved: usize,
}

enum Outcome {
    Kept,
    Updated,
    Unresolved,
}

/// One pass over every world row the authority can name an owner for.
pub async fn reconcile_owners(state: &CoreState) -> Result<ReconcileReport, sqlx::Error> {
    let rows: Vec<(String, Option<String>)> =
        sqlx::query_as("SELECT name, owner FROM worlds.worlds ORDER BY name")
            .fetch_all(&state.pool)
            .await?;
    let worlds: Vec<(WorldName, Option<Address>)> = rows
        .into_iter()
        .filter_map(|(name, owner)| {
            let world = WorldName::parse(&name).ok()?;
            let live = state.policy.ownable(&world)
                && state.policy.has_live_authority(&world)
                && world.personal_owner().is_none();
            live.then(|| (world, owner.and_then(|o| Address::parse(&o).ok())))
        })
        .collect();
    let mut report = ReconcileReport {
        checked: worlds.len(),
        ..ReconcileReport::default()
    };
    let mut outcomes = futures::stream::iter(worlds)
        .map(|(world, stored)| reconcile_one(state, world, stored))
        .buffer_unordered(CONCURRENCY);
    while let Some(outcome) = outcomes.next().await {
        match outcome? {
            Outcome::Kept => {}
            Outcome::Updated => report.updated += 1,
            Outcome::Unresolved => report.unresolved += 1,
        }
    }
    Ok(report)
}

async fn reconcile_one(
    state: &CoreState,
    world: WorldName,
    stored: Option<Address>,
) -> Result<Outcome, sqlx::Error> {
    let holder = match state.policy.live_name_owner(&world).await {
        Ok(Some(holder)) => holder,
        Ok(None) => return Ok(Outcome::Kept),
        Err(error) => {
            tracing::warn!(%world, %error, "world owner unresolved; stored owner kept");
            return Ok(Outcome::Unresolved);
        }
    };
    if stored.as_ref() == Some(&holder) {
        return Ok(Outcome::Kept);
    }
    let changed = sqlx::query(
        "UPDATE worlds.worlds SET owner = $2 WHERE name = $1 AND owner IS DISTINCT FROM $2",
    )
    .bind(world.key())
    .bind(&holder)
    .execute(&state.pool)
    .await?
    .rows_affected();
    if changed == 0 {
        return Ok(Outcome::Kept);
    }
    tracing::info!(%world, previous = ?stored, owner = %holder, "world owner reconciled");
    if let Some(previous) = stored {
        state.revoke_unless_admitted(&world, &previous).await;
    }
    Ok(Outcome::Updated)
}
