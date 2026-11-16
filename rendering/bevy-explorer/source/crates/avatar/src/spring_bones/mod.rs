//! Secondary motion for the extra bones of wearables: hair, tails, capes.
//!
//! [`chains`] decides which bones swing and how they group, and [`sim`] is the solver. An avatar's
//! chains are found when it is built and kept on it as [`SpringChains`], so a rebuilt avatar
//! starts with fresh ones and a removed avatar takes its own with it.

use bevy::{
    ecs::system::ParamSet, platform::collections::HashMap, prelude::*,
    transform::helper::TransformHelper,
};
use collectibles::spring_bones::{SpringBoneModel, SpringBoneParams};
use common::{
    sets::PostUpdateSets,
    structs::{AppConfig, PlayerTeleported, PrimaryCamera, PrimaryUser},
};

pub mod chains;
mod diagnostics;
pub mod sim;

use chains::{plan_chains, SkinJoint};
use sim::{AnchorPose, ChainState, FramePlan, JointConfig, Stepper, TIP_LENGTH};

use crate::{avatar_texture::BoothAvatar, AvatarProcessed};

/// Most avatars simulated at once. The rest keep their spring bones at rest.
pub const MAX_SIMULATED_AVATARS: usize = 10;

/// How far up a wearable's own bones a branch is followed to the body bone it hangs off.
pub const MAX_BRANCH_DEPTH: usize = 64;

pub struct SpringBonesPlugin;

impl Plugin for SpringBonesPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(diagnostics::SpringBonesDiagnosticsPlugin)
            .init_resource::<SpringStepper>()
            .add_systems(
                PostUpdate,
                simulate_spring_bones.in_set(PostUpdateSets::SpringBones),
            );
    }
}

/// The spring chains of a built avatar, on the entity that holds its model.
#[derive(Component, Debug, Default)]
pub struct SpringChains(pub Vec<SpringChain>);

#[derive(Debug)]
pub struct SpringChain {
    /// the body bone the chain hangs off
    pub anchor: Entity,
    /// from the anchor outwards
    pub joints: Vec<SpringJoint>,
}

#[derive(Debug)]
pub struct SpringJoint {
    pub entity: Entity,
    pub params: SpringBoneParams,
    /// the bone's local rotation as the wearable was authored, which it returns to
    pub rest_rotation: Quat,
}

/// The fixed steps of this frame, shared by every chain.
#[derive(Resource, Default)]
pub struct SpringStepper(Stepper);

/// On an avatar whose chains are being simulated. Removed, with the bones back at rest, when the
/// avatar is no longer among the simulated ones.
#[derive(Component, Default)]
pub struct SpringSimulation {
    /// one for each of the avatar's chains; `None` where a chain could not be measured
    chains: Vec<Option<SimulatedChain>>,
    /// false until the chains have been set at rest where the avatar now stands
    snapped: bool,
}

impl SpringSimulation {
    pub fn simulated_chains(&self) -> usize {
        self.chains.iter().flatten().count()
    }
}

struct SimulatedChain {
    configs: Vec<JointConfig>,
    /// what each joint's local rotation is written against
    parents: Vec<JointParent>,
    rest_anchor_scale: Vec3,
    state: ChainState,
}

#[derive(Clone, Copy)]
enum JointParent {
    Anchor,
    /// a joint of the same avatar: chain, then joint
    Joint(usize, usize),
    /// a node that is not simulated
    Other(Entity),
}

fn anchor_pose(
    helper: &TransformHelper,
    anchor: Entity,
    rest_scale: Option<Vec3>,
) -> Option<AnchorPose> {
    let (scale, rotation, translation) = helper
        .compute_global_transform(anchor)
        .ok()?
        .to_scale_rotation_translation();
    Some(AnchorPose {
        translation,
        rotation,
        scale,
        scale_factor: sim::scale_factor(scale, rest_scale.unwrap_or(scale)),
    })
}

/// Measures an avatar's chains as they now stand, at rest.
fn measure(
    chains: &SpringChains,
    helper: &TransformHelper,
    nodes: &Query<(&Transform, &ChildOf)>,
) -> Vec<Option<SimulatedChain>> {
    let place = |entity: Entity| {
        chains.0.iter().enumerate().find_map(|(chain_ix, chain)| {
            let joint_ix = chain
                .joints
                .iter()
                .position(|joint| joint.entity == entity)?;
            Some((chain_ix, joint_ix))
        })
    };

    chains
        .0
        .iter()
        .map(|chain| {
            let anchor = anchor_pose(helper, chain.anchor, None)?;
            let mut configs = Vec::with_capacity(chain.joints.len());
            let mut parents = Vec::with_capacity(chain.joints.len());
            let mut poses = Vec::with_capacity(chain.joints.len());

            for (ix, joint) in chain.joints.iter().enumerate() {
                let (transform, parent) = nodes.get(joint.entity).ok()?;
                let (_, rotation, translation) = helper
                    .compute_global_transform(joint.entity)
                    .ok()?
                    .to_scale_rotation_translation();
                poses.push((translation, rotation));

                parents.push(if parent.parent() == chain.anchor {
                    JointParent::Anchor
                } else if let Some((chain_ix, joint_ix)) = place(parent.parent()) {
                    JointParent::Joint(chain_ix, joint_ix)
                } else {
                    JointParent::Other(parent.parent())
                });

                let (bone_axis, length) = match chain.joints.get(ix + 1) {
                    Some(next) => {
                        let (next_transform, _) = nodes.get(next.entity).ok()?;
                        let next_scale = helper
                            .compute_global_transform(next.entity)
                            .ok()?
                            .to_scale_rotation_translation()
                            .0;
                        JointConfig::axis_and_length(next_transform.translation, next_scale)
                    }
                    None => (Vec3::Y, TIP_LENGTH),
                };

                // authored against a left-handed world: this one's z runs the other way
                let direction = joint.params.gravity_dir;
                let gravity =
                    Vec3::new(direction.x, direction.y, -direction.z) * joint.params.gravity_power;

                configs.push(JointConfig {
                    stiffness: joint.params.stiffness,
                    drag: joint.params.drag,
                    gravity,
                    rest_rotation: joint.rest_rotation,
                    bone_axis,
                    length,
                    local_translation: transform.translation,
                    local_scale: transform.scale,
                });
            }

            Some(SimulatedChain {
                configs,
                parents,
                rest_anchor_scale: anchor.scale,
                state: ChainState::snapped(anchor, poses),
            })
        })
        .collect()
}

/// Picks the avatars to simulate, steps their chains and writes the bones' rotations.
///
/// The player and preview avatars come first, then other avatars by distance from the camera,
/// leaving out those behind it and those hidden. An avatar that drops out gets its bones back at
/// rest. Bone poses are computed here from local transforms: the stored globals predate this
/// frame's movement and IK.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn simulate_spring_bones(
    mut commands: Commands,
    config: Res<AppConfig>,
    time: Res<Time>,
    mut stepper: ResMut<SpringStepper>,
    mut teleports: EventReader<PlayerTeleported>,
    mut avatars: Query<
        (
            Entity,
            &SpringChains,
            &ChildOf,
            &Visibility,
            Option<&mut SpringSimulation>,
        ),
        With<AvatarProcessed>,
    >,
    owners: Query<(&GlobalTransform, Has<PrimaryUser>, Has<BoothAvatar>)>,
    camera: Query<&GlobalTransform, With<PrimaryCamera>>,
    // poses are read for every avatar before any rotation is written
    mut transforms: ParamSet<(
        (TransformHelper, Query<(&Transform, &ChildOf)>),
        Query<&mut Transform>,
    )>,
    mut ranked: Local<Vec<(f32, Entity)>>,
    mut writes: Local<Vec<(Entity, Quat)>>,
) {
    let teleported = teleports.read().count() > 0;
    let camera = camera.single().ok();

    ranked.clear();
    if config.graphics.spring_bones {
        for (entity, chains, owner, visibility, _) in avatars.iter() {
            if chains.0.is_empty() || *visibility == Visibility::Hidden {
                continue;
            }
            let Ok((position, is_player, is_preview)) = owners.get(owner.parent()) else {
                continue;
            };
            if is_player || is_preview {
                ranked.push((0.0, entity));
            } else if let Some(camera) = camera {
                let offset = position.translation() - camera.translation();
                if camera.forward().dot(offset) >= 0.0 {
                    ranked.push((offset.length_squared(), entity));
                }
            }
        }
        ranked.sort_unstable_by(|a, b| a.0.total_cmp(&b.0));
        ranked.truncate(MAX_SIMULATED_AVATARS);
    }

    let plan = stepper.0.advance(time.delta_secs());
    writes.clear();

    for (entity, chains, owner, _, simulation) in avatars.iter_mut() {
        let selected = ranked.iter().any(|(_, ranked)| *ranked == entity);

        let Some(mut simulation) = simulation else {
            if selected {
                // starts on the next frame, from where the avatar then stands
                commands
                    .entity(entity)
                    .try_insert(SpringSimulation::default());
            }
            continue;
        };

        if !selected {
            for joint in chains.0.iter().flat_map(|chain| &chain.joints) {
                writes.push((joint.entity, joint.rest_rotation));
            }
            commands.entity(entity).try_remove::<SpringSimulation>();
            continue;
        }

        let (helper, nodes) = transforms.p0();

        if plan == FramePlan::Skip
            || (teleported && owners.get(owner.parent()).is_ok_and(|owner| owner.1))
        {
            // hold the pose, and start again at rest in place once frames flow
            simulation.snapped = false;
            continue;
        }

        if !simulation.snapped {
            simulation.chains = measure(chains, &helper, &nodes);
            simulation.snapped = true;
        } else {
            for (chain, simulated) in chains.0.iter().zip(&mut simulation.chains) {
                let Some(simulated) = simulated else {
                    continue;
                };
                match anchor_pose(&helper, chain.anchor, Some(simulated.rest_anchor_scale)) {
                    Some(anchor) => simulated.state.set_anchor(anchor),
                    None => continue,
                }
            }
        }

        let mut blend = None;
        for simulated in simulation.chains.iter_mut().flatten() {
            blend = simulated.state.simulate(&simulated.configs, &plan);
        }
        let Some(blend) = blend else {
            continue;
        };

        for (chain_ix, chain) in chains.0.iter().enumerate() {
            let Some(simulated) = &simulation.chains[chain_ix] else {
                continue;
            };
            for (joint_ix, joint) in chain.joints.iter().enumerate() {
                let parent_rotation = match simulated.parents[joint_ix] {
                    JointParent::Anchor => {
                        anchor_pose(&helper, chain.anchor, None).map(|anchor| anchor.rotation)
                    }
                    JointParent::Joint(chain_ix, joint_ix) => simulation
                        .chains
                        .get(chain_ix)
                        .and_then(Option::as_ref)
                        .map(|chain| chain.state.joints[joint_ix].blended(blend)),
                    JointParent::Other(parent) => helper
                        .compute_global_transform(parent)
                        .ok()
                        .map(|parent| parent.to_scale_rotation_translation().1),
                };
                let Some(parent_rotation) = parent_rotation else {
                    continue;
                };
                let rotation = simulated.state.joints[joint_ix].blended(blend);
                writes.push((
                    joint.entity,
                    (parent_rotation.inverse() * rotation).normalize(),
                ));
            }
        }
    }

    let mut locals = transforms.p1();
    for (entity, rotation) in writes.drain(..) {
        if let Ok(mut transform) = locals.get_mut(entity) {
            transform.rotation = rotation;
        }
    }
}

/// The chains of one wearable, from its skin's joints as loaded. `body_bone` gives the body's
/// bone for a wearable joint that names one.
pub fn wearable_chains(
    joints: &[Entity],
    nodes: &Query<(&Name, &ChildOf)>,
    transforms: &Query<&Transform>,
    body_bone: impl Fn(Entity) -> Option<Entity>,
    params: Option<&SpringBoneModel>,
) -> Vec<SpringChain> {
    let mut places = HashMap::new();
    for (ix, joint) in joints.iter().enumerate() {
        places.entry(*joint).or_insert(ix);
    }

    let skin: Vec<SkinJoint> = joints
        .iter()
        .map(|joint| match nodes.get(*joint) {
            Ok((name, parent)) => SkinJoint {
                name: name.as_str(),
                parent: places.get(&parent.parent()).copied(),
                avatar_bone: name.to_lowercase().starts_with("avatar_"),
            },
            // a joint with no name or parent is no bone of the avatar and hangs off nothing
            Err(_) => SkinJoint {
                name: "",
                parent: None,
                avatar_bone: false,
            },
        })
        .collect();

    plan_chains(&skin, params)
        .into_iter()
        .filter_map(|chain| {
            if chain.dropped > 0 {
                warn!(
                    "spring chain from {:?} is too long: its last {} bones stay at rest",
                    skin[chain.joints[0].0].name, chain.dropped
                );
            }
            Some(SpringChain {
                anchor: body_bone(joints[chain.anchor])?,
                joints: chain
                    .joints
                    .into_iter()
                    .map(|(ix, params)| SpringJoint {
                        entity: joints[ix],
                        params,
                        rest_rotation: transforms
                            .get(joints[ix])
                            .map_or(Quat::IDENTITY, |transform| transform.rotation),
                    })
                    .collect(),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;

    #[derive(Resource)]
    struct Skin {
        joints: Vec<Entity>,
        body_head: Entity,
        wearable_head: Entity,
    }

    #[derive(Resource, Default)]
    struct Planned(Vec<SpringChain>);

    fn plan(
        skin: Res<Skin>,
        nodes: Query<(&Name, &ChildOf)>,
        transforms: Query<&Transform>,
        mut planned: ResMut<Planned>,
    ) {
        planned.0 = wearable_chains(
            &skin.joints,
            &nodes,
            &transforms,
            |joint| (joint == skin.wearable_head).then_some(skin.body_head),
            None,
        );
    }

    #[test]
    fn chains_hang_off_the_body_bone_and_remember_each_rest_rotation() {
        let mut world = World::new();
        let body_head = world.spawn(Name::new("Avatar_Head")).id();
        let armature = world.spawn(Name::new("Armature")).id();
        let wearable_head = world
            .spawn((Name::new("Avatar_Head"), ChildOf(armature)))
            .id();
        let rest = Quat::from_rotation_x(0.3);
        let hair_01 = world
            .spawn((
                Name::new("Hair_01"),
                ChildOf(wearable_head),
                Transform::from_rotation(rest),
            ))
            .id();
        let hair_02 = world
            .spawn((Name::new("Hair_02"), ChildOf(hair_01), Transform::IDENTITY))
            .id();
        world.insert_resource(Skin {
            joints: vec![wearable_head, hair_01, hair_02],
            body_head,
            wearable_head,
        });
        world.init_resource::<Planned>();

        world.run_system_once(plan).unwrap();

        let planned = &world.resource::<Planned>().0;
        assert_eq!(planned.len(), 1);
        assert_eq!(planned[0].anchor, body_head);
        let entities: Vec<_> = planned[0].joints.iter().map(|j| j.entity).collect();
        assert_eq!(entities, [hair_01, hair_02]);
        assert_eq!(planned[0].joints[0].rest_rotation, rest);
        assert_eq!(planned[0].joints[0].params, chains::DEFAULT_PARAMS);
    }

    #[test]
    fn a_chain_off_a_bone_the_body_lacks_is_left_out() {
        let mut world = World::new();
        let armature = world.spawn(Name::new("Armature")).id();
        let wearable_tail = world
            .spawn((Name::new("Avatar_Tail"), ChildOf(armature)))
            .id();
        let tail_01 = world
            .spawn((Name::new("Tail_01"), ChildOf(wearable_tail)))
            .id();
        world.insert_resource(Skin {
            joints: vec![wearable_tail, tail_01],
            body_head: armature,
            wearable_head: armature,
        });
        world.init_resource::<Planned>();

        world.run_system_once(plan).unwrap();

        assert!(world.resource::<Planned>().0.is_empty());
    }

    mod simulation {
        use super::*;
        use std::time::Duration;

        const FRAME: f32 = sim::FIXED_STEP;

        struct Avatar {
            root: Entity,
            model: Entity,
            joints: [Entity; 2],
        }

        fn app() -> App {
            let mut app = App::new();
            let mut config = AppConfig::default();
            config.graphics.spring_bones = true;
            app.insert_resource(config)
                .init_resource::<Time>()
                .init_resource::<SpringStepper>()
                .add_event::<PlayerTeleported>()
                .add_systems(Update, simulate_spring_bones);
            // at the origin, looking down -z
            app.world_mut()
                .spawn((PrimaryCamera::default(), GlobalTransform::IDENTITY));
            app
        }

        fn spawn_model(world: &mut World, root: Entity) -> (Entity, [Entity; 2]) {
            let model = world
                .spawn((
                    ChildOf(root),
                    Transform::IDENTITY,
                    AvatarProcessed,
                    Visibility::Inherited,
                ))
                .id();
            let anchor = world
                .spawn((ChildOf(model), Transform::from_xyz(0.0, 1.0, 0.0)))
                .id();
            let first = world
                .spawn((ChildOf(anchor), Transform::from_xyz(0.0, 0.2, 0.0)))
                .id();
            let second = world
                .spawn((ChildOf(first), Transform::from_xyz(0.0, 0.2, 0.0)))
                .id();
            let joint = |entity| SpringJoint {
                entity,
                params: chains::DEFAULT_PARAMS,
                rest_rotation: Quat::IDENTITY,
            };
            world
                .entity_mut(model)
                .insert(SpringChains(vec![SpringChain {
                    anchor,
                    joints: vec![joint(first), joint(second)],
                }]));
            (model, [first, second])
        }

        fn spawn_avatar(app: &mut App, position: Vec3) -> Avatar {
            let world = app.world_mut();
            let root = world
                .spawn((
                    Transform::from_translation(position),
                    GlobalTransform::from_translation(position),
                ))
                .id();
            let (model, joints) = spawn_model(world, root);
            Avatar {
                root,
                model,
                joints,
            }
        }

        fn tick(app: &mut App, dt: f32) {
            app.world_mut()
                .resource_mut::<Time>()
                .advance_by(Duration::from_secs_f32(dt));
            app.update();
        }

        fn move_to(app: &mut App, avatar: &Avatar, position: Vec3) {
            app.world_mut().entity_mut(avatar.root).insert((
                Transform::from_translation(position),
                GlobalTransform::from_translation(position),
            ));
        }

        fn simulated(app: &App, avatar: &Avatar) -> bool {
            app.world().get::<SpringSimulation>(avatar.model).is_some()
        }

        /// How far the avatar's first spring bone is turned from rest, in radians.
        fn swing(app: &App, avatar: &Avatar) -> f32 {
            app.world()
                .get::<Transform>(avatar.joints[0])
                .unwrap()
                .rotation
                .angle_between(Quat::IDENTITY)
        }

        /// Starts the avatar's simulation, then walks it sideways until its bones swing.
        fn swing_avatar(app: &mut App, avatar: &Avatar, from: Vec3) {
            tick(app, FRAME);
            tick(app, FRAME);
            for step in 1..=6 {
                move_to(app, avatar, from + Vec3::X * 0.1 * step as f32);
                tick(app, FRAME);
            }
            assert!(swing(app, avatar) > 0.2, "{}", swing(app, avatar));
        }

        #[test]
        fn the_nearest_avatars_are_simulated_up_to_the_limit() {
            let mut app = app();
            let avatars: Vec<_> = (1..=MAX_SIMULATED_AVATARS + 2)
                .map(|ix| spawn_avatar(&mut app, Vec3::NEG_Z * ix as f32))
                .collect();

            tick(&mut app, FRAME);

            for (ix, avatar) in avatars.iter().enumerate() {
                assert_eq!(simulated(&app, avatar), ix < MAX_SIMULATED_AVATARS, "{ix}");
            }
        }

        #[test]
        fn avatars_behind_the_camera_or_hidden_are_not_simulated() {
            let mut app = app();
            let behind = spawn_avatar(&mut app, Vec3::Z);
            let hidden = spawn_avatar(&mut app, Vec3::NEG_Z * 2.0);
            let ahead = spawn_avatar(&mut app, Vec3::NEG_Z * 5.0);
            app.world_mut()
                .entity_mut(hidden.model)
                .insert(Visibility::Hidden);

            tick(&mut app, FRAME);

            assert!(!simulated(&app, &behind));
            assert!(!simulated(&app, &hidden));
            assert!(simulated(&app, &ahead));
        }

        #[test]
        fn the_player_and_previews_come_first_wherever_they_stand() {
            let mut app = app();
            let others: Vec<_> = (1..=MAX_SIMULATED_AVATARS)
                .map(|ix| spawn_avatar(&mut app, Vec3::NEG_Z * ix as f32))
                .collect();
            let player = spawn_avatar(&mut app, Vec3::Z * 50.0);
            app.world_mut()
                .entity_mut(player.root)
                .insert(PrimaryUser::default());
            let preview = spawn_avatar(&mut app, Vec3::Z * 900.0);
            app.world_mut().entity_mut(preview.root).insert(BoothAvatar);

            tick(&mut app, FRAME);

            assert!(simulated(&app, &player));
            assert!(simulated(&app, &preview));
            // they take two of the places
            let simulated_others = others.iter().filter(|a| simulated(&app, a)).count();
            assert_eq!(simulated_others, MAX_SIMULATED_AVATARS - 2);
            assert!(!simulated(&app, others.last().unwrap()));
        }

        #[test]
        fn turning_the_setting_off_puts_the_bones_back_at_rest() {
            let mut app = app();
            let from = Vec3::NEG_Z * 3.0;
            let avatar = spawn_avatar(&mut app, from);
            swing_avatar(&mut app, &avatar, from);

            app.world_mut()
                .resource_mut::<AppConfig>()
                .graphics
                .spring_bones = false;
            tick(&mut app, FRAME);

            assert!(!simulated(&app, &avatar));
            assert_eq!(swing(&app, &avatar), 0.0);
        }

        #[test]
        fn an_avatar_that_drops_out_gets_its_bones_back_at_rest() {
            let mut app = app();
            let from = Vec3::NEG_Z * 3.0;
            let avatar = spawn_avatar(&mut app, from);
            swing_avatar(&mut app, &avatar, from);

            move_to(&mut app, &avatar, Vec3::Z * 3.0);
            tick(&mut app, FRAME);

            assert!(!simulated(&app, &avatar));
            assert_eq!(swing(&app, &avatar), 0.0);
        }

        #[test]
        fn a_frame_too_long_holds_the_pose_and_the_next_starts_from_rest_in_place() {
            let mut app = app();
            let from = Vec3::NEG_Z * 3.0;
            let avatar = spawn_avatar(&mut app, from);
            swing_avatar(&mut app, &avatar, from);
            let held = swing(&app, &avatar);

            move_to(&mut app, &avatar, from + Vec3::X * 40.0);
            tick(&mut app, 0.5);
            assert_eq!(swing(&app, &avatar), held);

            // no stretch back towards where it was
            tick(&mut app, FRAME);
            tick(&mut app, FRAME);
            assert!(swing(&app, &avatar) < 1e-3, "{}", swing(&app, &avatar));
        }

        #[test]
        fn a_teleported_player_arrives_with_its_bones_at_rest() {
            let arrive = |announce: bool| {
                let mut app = app();
                let from = Vec3::NEG_Z * 3.0;
                let player = spawn_avatar(&mut app, from);
                app.world_mut()
                    .entity_mut(player.root)
                    .insert(PrimaryUser::default());
                tick(&mut app, FRAME);
                tick(&mut app, FRAME);
                tick(&mut app, FRAME);

                let position = from + Vec3::X * 300.0;
                move_to(&mut app, &player, position);
                if announce {
                    app.world_mut().send_event(PlayerTeleported { position });
                }
                let mut widest: f32 = 0.0;
                for _ in 0..4 {
                    tick(&mut app, FRAME);
                    widest = widest.max(swing(&app, &player));
                }
                widest
            };

            assert!(arrive(true) < 1e-3, "{}", arrive(true));
            // the same move unannounced drags the bones behind
            assert!(arrive(false) > 0.5);
        }

        #[test]
        fn a_rebuilt_or_removed_avatar_mid_swing_leaves_nothing_behind() {
            let mut app = app();
            let from = Vec3::NEG_Z * 3.0;
            let avatar = spawn_avatar(&mut app, from);
            swing_avatar(&mut app, &avatar, from);

            // a look change replaces the model: the new one starts its own simulation at rest
            app.world_mut().entity_mut(avatar.model).despawn();
            let (model, joints) = spawn_model(app.world_mut(), avatar.root);
            let rebuilt = Avatar {
                root: avatar.root,
                model,
                joints,
            };
            tick(&mut app, FRAME);
            assert!(simulated(&app, &rebuilt));
            assert_eq!(swing(&app, &rebuilt), 0.0);
            tick(&mut app, FRAME);
            tick(&mut app, FRAME);
            assert!(swing(&app, &rebuilt) < 1e-3);

            app.world_mut().entity_mut(avatar.root).despawn();
            tick(&mut app, FRAME);
            assert_eq!(
                app.world_mut()
                    .query::<&SpringSimulation>()
                    .iter(app.world())
                    .count(),
                0
            );
        }
    }
}
