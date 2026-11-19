// TODO
// - support blending animations
// - suport morph targets
use bevy::{
    animation::{graph::AnimationNodeType, AnimationEntityMut, AnimationTarget, RepeatAnimation},
    platform::collections::{HashMap, HashSet},
    prelude::*,
    scene::{InstanceId, SceneSpawner},
};

use common::sets::SceneSets;
use dcl_component::{
    proto_components::sdk::components::{PbAnimationState, PbAnimator},
    SceneComponentId,
};
use dcl_types::interface::ComponentPosition;

use crate::SceneEntity;

use super::{gltf_container::GltfReady, AddCrdtInterfaceExt};

pub struct AnimatorPlugin;

#[cfg(test)]
mod tests;

impl Plugin for AnimatorPlugin {
    fn build(&self, app: &mut App) {
        app.add_crdt_lww_component::<PbAnimator, Animator>(
            SceneComponentId::ANIMATOR,
            ComponentPosition::Any,
        );

        app.add_systems(Update, update_animations.in_set(SceneSets::PostLoop));
    }
}

pub use common::structs::Clips;

#[derive(Component)]
pub struct Animator {
    pb_animator: PbAnimator,
}

impl From<PbAnimator> for Animator {
    fn from(pb_animator: PbAnimator) -> Self {
        Self { pb_animator }
    }
}

// Sample only properties keyed by the default clip. Other bones keep their last pose.
fn sample_default_pose(
    In((player, instance, graph, node)): In<(
        Entity,
        InstanceId,
        Handle<AnimationGraph>,
        AnimationNodeIndex,
    )>,
    scenes: Res<SceneSpawner>,
    graphs: Res<Assets<AnimationGraph>>,
    clips: Res<Assets<AnimationClip>>,
    mut targets: Query<(&AnimationTarget, AnimationEntityMut)>,
) {
    let Some(AnimationNodeType::Clip(handle)) = graphs
        .get(&graph)
        .and_then(|g| g.get(node))
        .map(|n| &n.node_type)
    else {
        return;
    };
    let Some(clip) = clips.get(handle) else {
        return;
    };
    for entity in scenes.iter_instance_entities(instance) {
        let Ok((target, mut properties)) = targets.get_mut(entity) else {
            continue;
        };
        if target.player != player {
            continue;
        }
        let Some(curves) = clip.curves_for_target(target.id) else {
            continue;
        };
        for curve in curves {
            let mut evaluator = curve.0.create_evaluator();
            if let Err(error) = curve
                .0
                .apply(evaluator.as_mut(), 0.0, 1.0, node)
                .and_then(|()| evaluator.commit(properties.reborrow()))
            {
                warn!("Failed to sample default animation on {entity:?}: {error:?}");
            }
        }
    }
}

fn update_animations(
    mut commands: Commands,
    // GltfReady follows the visible instance while a replacement source loads.
    changed: Query<Entity, (With<Clips>, Or<(Changed<Animator>, Changed<GltfReady>)>)>,
    mut removed: RemovedComponents<Animator>,
    mut players: Query<(
        &SceneEntity,
        Option<Ref<Animator>>,
        Ref<GltfReady>,
        &mut AnimationPlayer,
        &AnimationGraphHandle,
        &Clips,
    )>,
) {
    let removed: HashSet<_> = removed.read().collect();
    for ent in changed
        .iter()
        .filter(|e| !removed.contains(e))
        .chain(removed.iter().copied())
    {
        let Ok((scene_ent, maybe_animator, ready, mut player, graph, clips)) = players.get_mut(ent)
        else {
            continue;
        };
        if removed.contains(&ent)
            || maybe_animator
                .as_ref()
                .is_some_and(|a| a.is_added() || ready.is_changed())
        {
            player.stop_all();
            if let Some(default) = clips.default {
                commands.run_system_cached_with(
                    sample_default_pose,
                    (ent, ready.0, graph.0.clone(), default),
                );
            }
            if maybe_animator.is_none() {
                continue;
            }
        }
        debug!(
            "[{ent:?} / {scene_ent:?}] {:?}",
            maybe_animator.as_ref().map(|a| &a.pb_animator)
        );
        let targets: HashMap<AnimationNodeIndex, (f32, PbAnimationState)> = match maybe_animator {
            Some(ref animator) => animator
                .pb_animator
                .states
                .iter()
                .filter_map(|state| {
                    clips
                        .named
                        .get(state.clip.as_str())
                        .map(|(index, duration)| (*index, (*duration, state.clone())))
                })
                .collect(),
            None => clips
                .default
                .map(|clip| (clip, (0.0, PbAnimationState::default())))
                .into_iter()
                .collect(),
        };

        let mut prev_anims: HashSet<_> = player
            .playing_animations()
            .filter_map(|(ix, anim)| (!anim.is_finished()).then_some(*ix))
            .collect();

        for (ix, (duration, state)) in targets.into_iter() {
            let playing = state.playing.unwrap_or(true);
            // clamp: bevy normalizes by the summed weight, so negative weights extrapolate
            let new_weight = if !playing {
                0.0
            } else {
                let weight = state.weight.unwrap_or(1.0);
                if weight.is_nan() {
                    0.0
                } else {
                    weight.clamp(0.0, 1.0)
                }
            };
            let new_speed = if !playing {
                0.0
            } else {
                state.speed.unwrap_or(1.0)
            };

            let active_animation = match (prev_anims.remove(&ix), state.should_reset()) {
                // if shouldReset, we always (re)start
                (_, true) |
                // if not playing we start
                (false, _) => {
                    let anim = player.start(ix);

                    if new_speed < 0.0 {
                        anim.seek_to(duration);
                    }
                    anim
                }
                // otherwise use existing
                (true, false) => player.animation_mut(ix).unwrap(),
            };

            active_animation.set_weight(new_weight);
            active_animation.set_speed(new_speed);
            if state.r#loop.unwrap_or(true) {
                active_animation.repeat();
            } else {
                active_animation.set_repeat(RepeatAnimation::Never);
            }

            // clamp seek time and reset completions
            if duration != 0.0 {
                let seek_time = active_animation.seek_time().clamp(0.0, duration);
                active_animation.replay();
                active_animation.seek_to(seek_time);
            }
        }

        let playing = player.playing_animations().collect::<Vec<_>>();
        debug!("final: {:?}", playing);

        // stop anims that have been removed
        for ix in prev_anims {
            player.stop(ix);
        }
    }
}
