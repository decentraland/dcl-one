//! The `worlds-owner-reconcile` job (upstream's `update-owner` job): rewrites
//! `worlds.owner` of the DCL-NAME worlds published here from the name
//! authority, so a NAME transfer reaches the stored owner (listings,
//! notifications) without a republish. Admission reads the live owner
//! (`Policy::current_owner`) and does not wait for it.
//!
//! Left alone: personal worlds (owned by rule), mirrored worlds (the mirror
//! never writes `owner`; their NAME holder is the upstream's), ENS worlds (no
//! ENS reader yet) and every name the subgraph does not list or could not
//! answer for (the stored owner is kept).

use sqlx::PgPool;

use crate::chain_index::graph::{NAMES_PER_QUERY, name_owners};
use crate::comms::world_identity::WorldName;
use crate::policy::Policy;
use crate::shared::identity::Address;

pub const JOB: &str = "worlds-owner-reconcile";

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Summary {
    pub checked: usize,
    pub updated: usize,
    pub unresolved: usize,
}

/// One pass; a no-op without a names reader.
pub async fn reconcile(pool: &PgPool, policy: &Policy) -> anyhow::Result<Summary> {
    let Some(market) = policy.names_reader() else {
        return Ok(Summary::default());
    };
    let rows: Vec<(String, Option<String>)> = sqlx::query_as(
        "SELECT name, owner FROM worlds.worlds \
         WHERE mirror_source IS NULL AND name LIKE '%.dcl.eth' ORDER BY name",
    )
    .fetch_all(pool)
    .await?;
    let worlds: Vec<(WorldName, Option<String>)> = rows
        .into_iter()
        .filter_map(|(name, owner)| Some((WorldName::parse(&name).ok()?, owner)))
        .filter(|(w, _)| w.is_dcl_world_name() && w.personal_owner().is_none())
        .collect();
    let mut summary = Summary::default();
    for chunk in worlds.chunks(NAMES_PER_QUERY) {
        let labels: Vec<String> = chunk.iter().map(|(w, _)| w.label().to_string()).collect();
        summary.checked += chunk.len();
        let holders = match name_owners(market, &labels).await {
            Ok(holders) => holders,
            Err(e) => {
                tracing::warn!(error = %e, worlds = chunk.len(), "world owners not resolved; stored owners kept");
                summary.unresolved += chunk.len();
                continue;
            }
        };
        let mut changes = Vec::new();
        for (world, stored) in chunk {
            let Some(holder) = holders
                .get(world.label())
                .and_then(|h| Address::parse(h).ok())
            else {
                tracing::warn!(world = %world, owner = ?stored, "world owner not resolved; stored owner kept");
                summary.unresolved += 1;
                continue;
            };
            if stored.as_deref() != Some(holder.as_str()) {
                changes.push(Change {
                    name: world.key().to_string(),
                    owner: holder.into_string(),
                    previous: stored.clone(),
                });
            }
        }
        summary.updated += write(pool, &changes).await? as usize;
    }
    Ok(summary)
}

/// One stored-owner rewrite: `owner` replaces `previous`.
#[derive(Debug, Clone)]
pub(crate) struct Change {
    pub name: String,
    pub owner: String,
    pub previous: Option<String>,
}

/// Applies `changes` where the row still holds `previous`, so a publish
/// after the read wins; the rows rewritten.
pub(crate) async fn write(pool: &PgPool, changes: &[Change]) -> sqlx::Result<u64> {
    if changes.is_empty() {
        return Ok(0);
    }
    let names: Vec<&str> = changes.iter().map(|c| c.name.as_str()).collect();
    let owners: Vec<&str> = changes.iter().map(|c| c.owner.as_str()).collect();
    let previous: Vec<Option<&str>> = changes.iter().map(|c| c.previous.as_deref()).collect();
    let updated = sqlx::query(
        "UPDATE worlds.worlds w SET owner = u.owner, updated_at = timezone('UTC', now()) \
         FROM unnest($1::text[], $2::text[], $3::text[]) AS u(name, owner, previous) \
         WHERE w.name = u.name AND w.mirror_source IS NULL \
           AND w.owner IS NOT DISTINCT FROM u.previous",
    )
    .bind(&names)
    .bind(&owners)
    .bind(&previous)
    .execute(pool)
    .await?;
    Ok(updated.rows_affected())
}
