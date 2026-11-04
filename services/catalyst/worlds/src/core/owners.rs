//! `worlds-owner-reconcile`: the fleet job that rewrites `worlds.owner` from
//! the name authority (the upstream's 12-hourly update-owner job), so the
//! listings, wallet stats and limits that read the column follow a NAME
//! transfer. Admission never trusts the column while an authority is
//! configured; this keeps the copy current and re-decides the previous
//! holder's place in the world's live rooms. Only worlds with a local scene
//! are asked about, in batches and outside the admission memos. A name the
//! authority cannot answer for, or says nobody holds, keeps its stored owner.

use crate::comms::world_identity::WorldName;
use crate::shared::identity::Address;

use super::CoreState;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ReconcileReport {
    pub checked: usize,
    pub updated: usize,
    pub unresolved: usize,
}

/// One pass over every world with a local scene the authority can name an owner for.
pub async fn reconcile_owners(state: &CoreState) -> Result<ReconcileReport, sqlx::Error> {
    let rows: Vec<(String, Option<String>)> = sqlx::query_as(
        "SELECT w.name, w.owner FROM worlds.worlds w \
         WHERE EXISTS (SELECT 1 FROM worlds.world_scenes s \
                       WHERE s.world_name = w.name AND s.mirror_source IS NULL) \
         ORDER BY w.name",
    )
    .fetch_all(&state.pool)
    .await?;
    let worlds: Vec<(WorldName, Option<Address>)> = rows
        .into_iter()
        .filter_map(|(name, owner)| {
            let world = WorldName::parse(&name).ok()?;
            Some((world, owner.and_then(|o| Address::parse(&o).ok())))
        })
        .collect();
    let names: Vec<WorldName> = worlds.iter().map(|(w, _)| w.clone()).collect();
    let mut holders = state.policy.reconcile_owners(&names).await;
    let mut report = ReconcileReport::default();
    for (world, stored) in worlds {
        let Some(holder) = holders.remove(world.key()) else {
            continue;
        };
        report.checked += 1;
        let holder = match holder {
            Ok(Some(holder)) => holder,
            Ok(None) => continue,
            Err(error) => {
                tracing::warn!(%world, %error, "world owner unresolved; stored owner kept");
                report.unresolved += 1;
                continue;
            }
        };
        if stored.as_ref() == Some(&holder) {
            continue;
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
            continue;
        }
        report.updated += 1;
        tracing::info!(%world, previous = ?stored, owner = %holder, "world owner reconciled");
        state.policy.note_owner(&world, Some(&holder));
        if let Some(previous) = stored {
            state.revoke_unless_admitted(&world, &previous).await;
        }
    }
    Ok(report)
}
