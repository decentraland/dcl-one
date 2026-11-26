// TODO
// - suport morph targets
use bevy::{
    animation::{
        graph::AnimationNodeType, ActiveAnimation, AnimationEntityMut, AnimationTarget,
        RepeatAnimation,
    },
    platform::collections::{HashMap, HashSet},
    prelude::*,
    scene::{InstanceId, SceneSpawner},
};

use common::{anim_last_system, sets::SceneSets, structs::NoRenderApp};
use dcl_component::{
    proto_components::sdk::components::{PbAnimationState, PbAnimator},
    DclReader, DclWriter, FromDclReader, SceneComponentId, SceneCrdtTimestamp, SceneEntityId,
};
use dcl_types::{
    crdt::lww::CrdtLWWState,
    interface::{ComponentPosition, CrdtType},
};

use crate::{RendererSceneContext, SceneEntity, SceneLoopSchedule, SceneLoopSets};

use super::{
    gltf_container::GltfReady, process_crdt_lww_updates, AddCrdtInterfaceExt, CrdtStateComponent,
};

pub struct AnimatorPlugin;

mod diagnostics;
#[cfg(test)]
mod tests;

impl Plugin for AnimatorPlugin {
    fn build(&self, app: &mut App) {
        app.add_crdt_lww_component::<PbAnimator, Animator>(
            SceneComponentId::ANIMATOR,
            ComponentPosition::Any,
        );

        app.init_resource::<FinishReports>();
        // On a scene's reply before it is applied, in the pass that receives it, so the scene's
        // next tick starts from what this settles.
        app.world_mut()
            .resource_mut::<SceneLoopSchedule>()
            .schedule
            .add_systems(
                settle_stale_animator_writes
                    .in_set(SceneLoopSets::UpdateWorld)
                    .before(process_crdt_lww_updates::<PbAnimator, Animator>),
            );
        app.add_systems(Update, update_animations.in_set(SceneSets::PostLoop));
        app.add_systems(PostUpdate, report_finished_clips.after(anim_last_system!()));
        if app.world().get_resource::<NoRenderApp>().is_none() {
            app.add_plugins(diagnostics::AnimatorDiagnosticsPlugin);
        }
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

/// How many clips have been reported to their scenes as finished, for the debug panel.
#[derive(Resource, Default)]
struct FinishReports(u64);

/// The model's Animator plays a clip once, so the clip's end is watched for.
#[derive(Component)]
struct PlaysOnce;

/// The clips the scene has been told ended, each with the timestamp of the Animator that told
/// it. A scene write that does not pass a clip's timestamp was made before the scene read that
/// end.
#[derive(Component, Default)]
struct ReportedClips(Vec<(String, SceneCrdtTimestamp)>);

impl ReportedClips {
    fn tell(&mut self, clip: &str, timestamp: SceneCrdtTimestamp) {
        match self.0.iter_mut().find(|(told, _)| told == clip) {
            Some(told) => told.1 = timestamp,
            None => self.0.push((clip.to_owned(), timestamp)),
        }
    }
}

/// A state plays only when the scene sets `playing`.
fn is_playing(state: &PbAnimationState) -> bool {
    state.playing.unwrap_or(false)
}

fn is_looping(state: &PbAnimationState) -> bool {
    state.r#loop.unwrap_or(true)
}

fn plays_once(animator: &PbAnimator, clips: &Clips) -> bool {
    animator.states.iter().any(|state| {
        is_playing(state) && !is_looping(state) && clips.named.contains_key(state.clip.as_str())
    })
}

/// A clip can end while its scene's tick is running. That tick's write still says the clip is
/// playing, and would play it again. Such a write keeps its other changes, and the scene is
/// sent the result, which its next tick starts from.
fn settle_stale_animator_writes(
    mut commands: Commands,
    mut scenes: Query<(
        &mut RendererSceneContext,
        &mut CrdtStateComponent<CrdtLWWState, PbAnimator>,
    )>,
    mut reported: Query<&mut ReportedClips>,
) {
    for (mut context, mut writes) in scenes.iter_mut() {
        let told: Vec<(SceneEntityId, Entity)> = writes
            .last_write
            .keys()
            .filter_map(|id| {
                let ent = context.bevy_entity(*id)?;
                reported.contains(ent).then_some((*id, ent))
            })
            .collect();
        for (id, ent) in told {
            let Ok(mut reported) = reported.get_mut(ent) else {
                continue;
            };
            let Some(write) = writes.last_write.get_mut(&id) else {
                continue;
            };
            // a write that passes a clip's timestamp was made after the scene read that end:
            // playing the clip again is its choice
            reported.0.retain(|(_, told)| write.timestamp <= *told);
            if reported.0.is_empty() {
                commands.entity(ent).try_remove::<ReportedClips>();
                continue;
            }
            if !write.is_some {
                // the report the scene has yet to read would give it the Animator back
                context.clear_crdt(SceneComponentId::ANIMATOR, CrdtType::LWW_ANY, id);
                commands.entity(ent).try_remove::<ReportedClips>();
                continue;
            }
            let Ok(mut animator) = PbAnimator::from_reader(&mut DclReader::new(&write.data)) else {
                continue;
            };
            for state in animator.states.iter_mut() {
                if is_playing(state) && reported.0.iter().any(|(clip, _)| *clip == state.clip) {
                    state.playing = Some(false);
                }
            }
            write.data.clear();
            DclWriter::new(&mut write.data).write(&animator);
            let timestamp =
                context.update_crdt(SceneComponentId::ANIMATOR, CrdtType::LWW_ANY, id, &animator);
            for (_, told) in reported.0.iter_mut() {
                *told = timestamp;
            }
        }
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
                commands.entity(ent).try_remove::<PlaysOnce>();
                continue;
            }
        }
        if maybe_animator
            .as_ref()
            .is_some_and(|animator| plays_once(&animator.pb_animator, clips))
        {
            commands.entity(ent).try_insert(PlaysOnce);
        } else {
            commands.entity(ent).try_remove::<PlaysOnce>();
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
                .map(|clip| {
                    let state = PbAnimationState {
                        playing: Some(true),
                        ..Default::default()
                    };
                    (clip, (0.0, state))
                })
                .into_iter()
                .collect(),
        };

        let mut prev_anims: HashSet<_> = player
            .playing_animations()
            .filter_map(|(ix, anim)| (!anim.is_finished()).then_some(*ix))
            .collect();

        for (ix, (duration, state)) in targets.into_iter() {
            let playing = is_playing(&state);
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

            // shouldReset acts on a playing state only: a stopped one keeps its clock
            let reset = playing && state.should_reset();
            let active_animation = match (prev_anims.remove(&ix), reset) {
                // a playing state with shouldReset (re)starts on every write
                (_, true) |
                // a clip that is new to the player, or has finished, starts
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
            if is_looping(&state) {
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

/// Tells the scene when a clip it asked to play once has reached its end: that state's
/// `playing` becomes false in the scene's Animator. The clip stops contributing, so the model
/// holds the pose it ended on.
fn report_finished_clips(
    mut commands: Commands,
    mut players: Query<
        (
            Entity,
            &SceneEntity,
            &mut Animator,
            &mut AnimationPlayer,
            &Clips,
            Option<&mut ReportedClips>,
        ),
        With<PlaysOnce>,
    >,
    mut scenes: Query<&mut RendererSceneContext>,
    mut reports: ResMut<FinishReports>,
) {
    for (ent, scene_ent, mut animator, mut player, clips, reported) in players.iter_mut() {
        let finished: Vec<(usize, AnimationNodeIndex)> = animator
            .pb_animator
            .states
            .iter()
            .enumerate()
            .filter(|(_, state)| is_playing(state) && !is_looping(state))
            .filter_map(|(state_ix, state)| {
                let (node, _) = clips.named.get(state.clip.as_str())?;
                player
                    .animation(*node)
                    .is_some_and(ActiveAnimation::is_finished)
                    .then_some((state_ix, *node))
            })
            .collect();
        if finished.is_empty() {
            continue;
        }
        let Ok(mut context) = scenes.get_mut(scene_ent.root) else {
            continue;
        };

        // The scene is told below; marking the Animator changed would apply it again and
        // restart its clips.
        let animator = animator.bypass_change_detection();
        for (state_ix, node) in &finished {
            animator.pb_animator.states[*state_ix].playing = Some(false);
            if let Some(active) = player.animation_mut(*node) {
                active.set_weight(0.0);
            }
        }
        let timestamp = context.update_crdt(
            SceneComponentId::ANIMATOR,
            CrdtType::LWW_ANY,
            scene_ent.id,
            &animator.pb_animator,
        );
        reports.0 += finished.len() as u64;

        let ended = finished
            .iter()
            .map(|(state_ix, _)| animator.pb_animator.states[*state_ix].clip.as_str());
        match reported {
            Some(mut reported) => ended.for_each(|clip| reported.tell(clip, timestamp)),
            None => {
                let mut reported = ReportedClips::default();
                ended.for_each(|clip| reported.tell(clip, timestamp));
                commands.entity(ent).try_insert(reported);
            }
        }
        if !plays_once(&animator.pb_animator, clips) {
            commands.entity(ent).try_remove::<PlaysOnce>();
        }
    }
}
