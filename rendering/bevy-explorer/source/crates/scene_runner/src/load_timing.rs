//! How long scenes take to become ready after they spawn, for load-time comparisons between
//! builds. Logged under the `scene_timing` target: one line per scene when it reaches its
//! fifth tick, and one per arrival when the player's scene and every scene in its first ring
//! are ready. Filter the log with `scene_timing=info`. [`FirstRingTiming::ready`] also keys the
//! first ring's readiness to the player's [`Arrivals`], for `/scene_loading_state`.

use bevy::prelude::*;
use common::structs::{Arrivals, OutOfWorld, PrimaryUser};
use serde_json::{json, Value};
use web_time::Instant;

use crate::{
    initialize_scene::{SceneHash, SceneLoading},
    renderer_context::{RendererSceneContext, FROZEN_BLOCK},
    vec3_to_parcel, ContainingScene,
};

/// The target every line here is logged under.
pub const LOG_TARGET: &str = "scene_timing";
/// How the arrival line starts, before its milliseconds.
pub const FIRST_RING_READY: &str = "player's scene and first ring ready";

/// When the lifecycle spawned the scene.
#[derive(Component, Clone, Copy, Debug)]
pub struct SpawnedAt(pub Instant);

/// The scene's readiness has been logged.
#[derive(Component, Debug)]
pub struct ReadyLogged;

/// One arrival: the player's scene spawned while the player stood in it or waited for it behind
/// the loading screen, and the time until it and its first ring of neighbours are ready has not
/// been reported yet.
#[derive(Resource, Default, Debug)]
pub struct FirstRingTiming {
    pub arrival: Option<(Entity, Instant)>,
    pub reported: bool,
    /// The latest arrival whose first ring around the player has been ready since.
    pub(crate) ready: Option<FirstRingReady>,
}

/// The first ring was ready at `at`, in world after arrival number `arrival` ([`Arrivals`]): the
/// first frame of that arrival when it was already loaded, else the frame it completed.
#[derive(Clone, Copy, Debug)]
pub(crate) struct FirstRingReady {
    pub(crate) arrival: u64,
    pub(crate) at: Instant,
}

/// Chebyshev distance in parcels from `parcel` to the nearest of `parcels`; `u16::MAX` with
/// none.
pub fn ring_of(parcel: IVec2, parcels: impl IntoIterator<Item = IVec2>) -> u16 {
    parcels
        .into_iter()
        .map(|p| (p - parcel).abs().max_element().min(u16::MAX as i32) as u16)
        .min()
        .unwrap_or(u16::MAX)
}

/// The scene has loaded as far as it ever will: five ticks (when `animate_ready_scene` raises
/// it), or a terminal state.
pub fn scene_is_ready(context: &RendererSceneContext) -> bool {
    context.tick_number >= 5 || context.broken() || context.blocked.contains(FROZEN_BLOCK)
}

pub fn report_scene_load_times(
    mut commands: Commands,
    player: Query<(Entity, &GlobalTransform, Has<OutOfWorld>), With<PrimaryUser>>,
    arrivals: Option<Res<Arrivals>>,
    containing_scene: ContainingScene,
    scenes: Query<(
        Entity,
        &SceneHash,
        &SpawnedAt,
        Option<&RendererSceneContext>,
        Option<&SceneLoading>,
        Has<ReadyLogged>,
    )>,
    mut timing: ResMut<FirstRingTiming>,
) {
    let Ok((player, player_transform, out_of_world)) = player.single() else {
        return;
    };
    let player_parcel = vec3_to_parcel(player_transform.translation());
    let now = Instant::now();

    // a new current scene is an arrival worth timing only while it is still loading; behind the
    // loading screen (spawn, teleport) the player's provisional parcel is the destination, which
    // is still loading there
    if let Some(current) = containing_scene.get_parcel_oow(player) {
        if timing.arrival.is_none_or(|(scene, _)| scene != current) {
            if let Ok((_, _, spawned, context, _, _)) = scenes.get(current) {
                let loading = context.is_none_or(|context| !scene_is_ready(context));
                timing.arrival = loading.then_some((current, spawned.0));
                timing.reported = !loading;
            }
        }
    }

    let mut first_ring_complete = true;
    for (entity, hash, spawned, context, loading, logged) in &scenes {
        let failed = matches!(loading, Some(SceneLoading::Failed));
        let ready = failed || context.is_some_and(scene_is_ready);
        let ring = context.map(|context| ring_of(player_parcel, context.parcels.iter().copied()));
        if ready && !logged {
            info!(
                target: LOG_TARGET,
                "scene ready {} ms after spawn: {} ring {} tick {}{}",
                now.duration_since(spawned.0).as_millis(),
                hash.0,
                ring.map(|r| r.to_string()).unwrap_or_else(|| "?".to_owned()),
                context.map(|c| c.tick_number).unwrap_or(0),
                if failed { " (failed)" } else { "" }
            );
            commands.entity(entity).try_insert(ReadyLogged);
        }
        // a scene without a context yet has an unknown ring: it holds the report until known
        let near = ring.is_none_or(|ring| ring <= 1);
        if near && !ready {
            first_ring_complete = false;
        }
    }

    // without the counter (an app without `ArrivalsPlugin`) there is no arrival to key it to
    let arrivals = arrivals.map_or(0, |arrivals| arrivals.0);
    if first_ring_complete
        && !out_of_world
        && timing.ready.map_or(0, |ready| ready.arrival) < arrivals
    {
        timing.ready = Some(FirstRingReady {
            arrival: arrivals,
            at: now,
        });
    }

    if let Some((scene, arrival)) = timing.arrival {
        // a scene despawned before it was ready (a realm change during the landing) ends the
        // arrival unreported
        if scenes.get(scene).is_err() {
            timing.arrival = None;
        } else if !timing.reported && first_ring_complete {
            info!(
                target: LOG_TARGET,
                "{FIRST_RING_READY} {} ms after the scene spawned",
                now.duration_since(arrival).as_millis()
            );
            timing.reported = true;
        }
    }
}

/// `/scene_loading_state`'s `firstRing`: the latest arrival whose first ring has been ready, and
/// the milliseconds since; `null` before one.
pub(crate) fn first_ring_json(timing: &FirstRingTiming, now: Instant) -> Value {
    json!(timing.ready.map(|ready| json!({
        "arrival": ready.arrival,
        "sinceReadyMs": now.duration_since(ready.at).as_millis() as u64,
    })))
}

#[cfg(test)]
mod tests;
