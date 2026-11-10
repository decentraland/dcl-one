//! How long scenes take to become ready after they spawn, for load-time comparisons between
//! builds. Logged under the `scene_timing` target: one line per scene when it reaches its
//! fifth tick, and one per arrival when the player's scene and every scene in its first ring
//! are ready. Filter the log with `scene_timing=info`.

use bevy::prelude::*;
use common::structs::PrimaryUser;
use web_time::Instant;

use crate::{
    initialize_scene::{SceneHash, SceneLoading},
    renderer_context::{RendererSceneContext, FROZEN_BLOCK},
    vec3_to_parcel, ContainingScene,
};

/// When the lifecycle spawned the scene.
#[derive(Component, Clone, Copy, Debug)]
pub struct SpawnedAt(pub Instant);

/// The scene's readiness has been logged.
#[derive(Component, Debug)]
pub struct ReadyLogged;

/// One arrival: the player's scene spawned while the player stood in it, and the time until it
/// and its first ring of neighbours are ready has not been reported yet.
#[derive(Resource, Default, Debug)]
pub struct FirstRingTiming {
    pub arrival: Option<(Entity, Instant)>,
    pub reported: bool,
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

#[allow(clippy::type_complexity)]
pub fn report_scene_load_times(
    mut commands: Commands,
    player: Query<(Entity, &GlobalTransform), With<PrimaryUser>>,
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
    let Ok((player, player_transform)) = player.single() else {
        return;
    };
    let player_parcel = vec3_to_parcel(player_transform.translation());
    let now = Instant::now();

    // a new current scene is an arrival worth timing only while it is still loading
    if let Some(current) = containing_scene.get_parcel(player) {
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
                target: "scene_timing",
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

    if let Some((_, arrival)) = timing.arrival {
        if !timing.reported && first_ring_complete {
            info!(
                target: "scene_timing",
                "player's scene and first ring ready {} ms after the scene spawned",
                now.duration_since(arrival).as_millis()
            );
            timing.reported = true;
        }
    }
}

#[cfg(test)]
mod tests;
