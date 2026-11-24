use std::{f32::consts::TAU, marker::PhantomData, ops::DerefMut};

use bevy::{
    diagnostic::FrameCount,
    ecs::{
        component::Tick,
        system::{SystemChangeTick, SystemParam},
    },
    math::DVec3,
    platform::collections::{HashMap, HashSet},
    prelude::*,
};
use common::{
    dynamics::{
        PLAYER_COLLIDER_OVERLAP, PLAYER_COLLIDER_RADIUS, PLAYER_GROUND_THRESHOLD,
        PLAYER_LANDED_THRESHOLD,
    },
    inputs::MOVE_SET,
    sets::{PostUpdateSets, SceneLoopSets, SceneSets},
    structs::{
        avatar_tilt_quat, AppConfig, AvatarDynamicState, EngineMovementControl, LocomotionSpeeds,
        PlayerModifiers, PrimaryPlayerRes, PrimaryUser, SceneDrivenAnim,
        SceneDrivenAnimationFeedback, SceneDrivenAnimationRequest,
    },
};
use dcl_component::{
    proto_components::{
        common::Vector3,
        sdk::components::{
            AvatarAnimationState, ColliderLayer, MovementAnimation, PbAvatarLocomotionSettings,
            PbAvatarMovement, PbAvatarMovementInfo, PbPhysicsCombinedForce,
            PbPhysicsCombinedImpulse,
        },
    },
    DclReader, DclWriter, SceneComponentId, SceneEntityId,
};
use dcl_types::interface::{ComponentPosition, CrdtType};
use input_manager::{InputManager, InputPriority};
use ipfs::{
    ipfs_path::{IpfsPath, IpfsType},
    IpfsAssetServer,
};

use scene_runner::{
    initialize_scene::{PortableScenes, PARCEL_SIZE},
    renderer_context::{RedispatchRotation, RendererSceneContext, SceneTickTiming},
    update_world::{
        avatar_modifier_area::InputModifier,
        mesh_collider::{
            ColliderId, PreviousColliderTransform, SceneColliderData, GROUND_COLLISION_MASK,
        },
        AddCrdtInterfaceExt,
    },
    ContainingScene, SceneEntity, SceneLoopSchedule, SceneUpdates, MOVEMENT_CONTROLLER,
};

pub struct AvatarMovementPlugin;

impl Plugin for AvatarMovementPlugin {
    fn build(&self, app: &mut App) {
        app.add_crdt_lww_component::<PbAvatarMovement, AvatarMovement>(
            SceneComponentId::AVATAR_MOVEMENT,
            ComponentPosition::EntityOnly,
        );
        app.add_crdt_lww_component::<PbAvatarLocomotionSettings, AvatarLocomotionSettings>(
            SceneComponentId::AVATAR_LOCOMOTION_SETTINGS,
            ComponentPosition::EntityOnly,
        );
        app.add_crdt_lww_component::<PbPhysicsCombinedImpulse, PhysicsCombinedImpulse>(
            SceneComponentId::PHYSICS_COMBINED_IMPULSE,
            ComponentPosition::EntityOnly,
        );
        app.add_crdt_lww_component::<PbPhysicsCombinedForce, PhysicsCombinedForce>(
            SceneComponentId::PHYSICS_COMBINED_FORCE,
            ComponentPosition::EntityOnly,
        );

        app.init_resource::<AvatarMovementInfo>();
        app.init_resource::<LastMovementStep>();
        app.register_required_components::<PrimaryUser, LocomotionSpeeds>();
        app.register_required_components::<RendererSceneContext, MovementPeriod>();
        app.init_resource::<CollisionPush>();
        app.init_resource::<CentralCollisions>();
        app.init_resource::<SceneDrivenAnimationFeedback>();
        app.init_resource::<LatestFramePick<AvatarMovement>>();

        app.add_systems(Update, broadcast_movement_info.in_set(SceneSets::Init));
        app.world_mut()
            .resource_mut::<SceneLoopSchedule>()
            .add_systems(stage_redispatch_rotation.after(SceneLoopSets::UpdateWorld));

        app.add_systems(
            Update,
            (
                ActivePlayerComponent::<AvatarMovement>::pick_latest_frame_only_by_priority,
                ActivePlayerComponent::<AvatarLocomotionSettings>::pick_by_priority,
                ActivePlayerComponent::<InputModifier>::pick_by_priority,
                update_locomotion_speeds
                    .after(ActivePlayerComponent::<AvatarLocomotionSettings>::pick_by_priority),
                update_priority_scene.after(
                    ActivePlayerComponent::<AvatarMovement>::pick_latest_frame_only_by_priority,
                ),
                update_scene_driven_animation.after(
                    ActivePlayerComponent::<AvatarMovement>::pick_latest_frame_only_by_priority,
                ),
                start_movement_periods,
            )
                .in_set(SceneSets::PostLoop),
        );

        app.add_systems(
            PostUpdate,
            (
                apply_rotation,
                apply_ground_collider_movement,
                resolve_collisions,
                apply_impulses,
                apply_movement,
                record_own_movement,
                record_ground_collider,
                accumulate_movement_periods,
            )
                .chain()
                .in_set(PostUpdateSets::PlayerUpdate),
        );
    }
}

/// The speeds the active locomotion settings give; a speed they leave unset is the player's own
/// (configured speed with area modifiers).
fn locomotion_speeds(
    base: &PrimaryUser,
    settings: Option<&PbAvatarLocomotionSettings>,
) -> LocomotionSpeeds {
    LocomotionSpeeds {
        walk: settings
            .and_then(|s| s.walk_speed)
            .unwrap_or(base.walk_speed),
        jog: settings.and_then(|s| s.jog_speed).unwrap_or(base.jog_speed),
        run: settings.and_then(|s| s.run_speed).unwrap_or(base.run_speed),
    }
}

/// Keeps the player's [`LocomotionSpeeds`] in step with the active locomotion settings.
fn update_locomotion_speeds(
    mut player: Query<(
        &PrimaryUser,
        Option<&PlayerModifiers>,
        Option<&ActivePlayerComponent<AvatarLocomotionSettings>>,
        &mut LocomotionSpeeds,
    )>,
) {
    for (user, modifiers, active, mut speeds) in player.iter_mut() {
        let base = modifiers.map_or_else(|| user.clone(), |m| m.combine(user));
        let settings = active.map(|a| &a.component.0);
        speeds.set_if_neq(locomotion_speeds(&base, settings));
    }
}

fn update_priority_scene(
    player: Query<&ActivePlayerComponent<AvatarMovement>, With<PrimaryUser>>,
    mut updates: ResMut<SceneUpdates>,
) {
    let scene = player
        .single()
        .ok()
        .map(|active| active.scene())
        .filter(|&e| e != Entity::PLACEHOLDER);

    match scene {
        Some(scene) => {
            updates.priority_scenes.insert(MOVEMENT_CONTROLLER, scene);
        }
        None => {
            updates.priority_scenes.remove(MOVEMENT_CONTROLLER);
        }
    }
}

// Resolves the active scene's `MovementAnimation.src` against the scene content map
// (path -> content hash) and writes a ready-to-play request onto the primary player's
// `SceneDrivenAnim` component for the avatar animation system to consume.
fn update_scene_driven_animation(
    mut commands: Commands,
    player: Query<(Entity, &ActivePlayerComponent<AvatarMovement>), With<PrimaryUser>>,
    scenes: Query<&RendererSceneContext>,
    ipfas: IpfsAssetServer,
    mut logged_failures: Local<HashSet<String>>,
) {
    let Ok((primary, active)) = player.single() else {
        return;
    };
    let request = (|| {
        let anim = active.component.animation.as_ref()?;
        let scene_ent = active.scene();
        if scene_ent == Entity::PLACEHOLDER {
            return None;
        }
        let ctx = scenes.get(scene_ent).ok()?;
        let ipfs_path = IpfsPath::new(IpfsType::new_content_file(
            ctx.hash.to_string(),
            anim.src.to_lowercase(),
        ));
        let ipfs_ctx = ipfas.ipfs().context.blocking_read();
        let Some(content_hash) = ipfs_path.hash(&ipfs_ctx) else {
            if logged_failures.insert(anim.src.clone()) {
                warn!(
                    "scene-driven movement animation path not found in scene content map: {}",
                    anim.src
                );
            }
            return None;
        };

        // Resolve each scene-relative audio path to a content hash against the same
        // scene content map. Drop (and warn once per src) any path that doesn't resolve.
        let sounds = anim
            .sounds
            .iter()
            .filter_map(|sound_src| {
                let sound_path = IpfsPath::new(IpfsType::new_content_file(
                    ctx.hash.to_string(),
                    sound_src.to_lowercase(),
                ));
                match sound_path.hash(&ipfs_ctx) {
                    Some(h) => Some(h),
                    None => {
                        if logged_failures.insert(sound_src.clone()) {
                            warn!(
                                "scene-driven movement sound path not found in scene content map: {sound_src}"
                            );
                        }
                        None
                    }
                }
            })
            .collect();

        // The `-false` suffix is a fixed part of the scene-emote URN format here;
        // loop behavior is carried separately in SceneDrivenAnimationRequest.r#loop.
        let urn = format!(
            "urn:decentraland:off-chain:scene-emote:{}-{}-false",
            ctx.hash, content_hash
        );
        Some(SceneDrivenAnimationRequest {
            src: anim.src.clone(),
            urn,
            scene_hash: ctx.hash.to_string(),
            content_hash,
            r#loop: anim.r#loop,
            speed: anim.speed,
            idle: anim.idle,
            transition_seconds: anim.transition_seconds.unwrap_or(0.2),
            seek: anim.playback_time,
            sounds,
        })
    })();

    commands
        .entity(primary)
        .try_insert(SceneDrivenAnim { active: request });
}

#[derive(Component, Clone, Debug)]
pub struct AvatarMovement {
    pub velocity: Vec3,
    pub orientation: f32,
    pub ground_direction: Vec3,
    /// set for one frame when a walk_target ends: true = reached target, false = failed
    pub walk_success: Option<bool>,
    /// scene-driven movement animation request; if absent, engine falls back to velocity-based selection
    pub animation: Option<MovementAnimation>,
    /// render-only lean (degrees) composed on top of `orientation`; does not affect physics
    /// or the yaw broadcast to other clients. 0 = upright.
    pub tilt_pitch: f32,
    pub tilt_roll: f32,
}

impl Default for AvatarMovement {
    fn default() -> Self {
        Self {
            velocity: Vec3::ZERO,
            orientation: 0.0,
            ground_direction: Vec3::NEG_Y,
            walk_success: None,
            animation: None,
            tilt_pitch: 0.0,
            tilt_roll: 0.0,
        }
    }
}

#[derive(Component, Clone, Debug)]
pub struct AvatarLocomotionSettings(pub(crate) PbAvatarLocomotionSettings);

impl From<PbAvatarLocomotionSettings> for AvatarLocomotionSettings {
    fn from(value: PbAvatarLocomotionSettings) -> Self {
        Self(value)
    }
}

impl FromConfig for AvatarLocomotionSettings {
    fn from_config(config: &AppConfig) -> Self {
        Self(PbAvatarLocomotionSettings {
            walk_speed: Some(config.player_settings.walk_speed),
            jog_speed: Some(config.player_settings.jog_speed),
            run_speed: Some(config.player_settings.run_speed),
            jump_height: Some(config.player_settings.jump_height),
            run_jump_height: Some(config.player_settings.run_jump_height),
            // unset, so that the movement scene keeps its own landing stun. 0 removes the stun
            hard_landing_cooldown: None,
            double_jump_height: None,
            gliding_speed: None,
            gliding_falling_speed: None,
        })
    }
}

impl From<PbAvatarMovement> for AvatarMovement {
    fn from(value: PbAvatarMovement) -> Self {
        Self {
            velocity: value.velocity.unwrap_or_default().world_vec_to_vec3(),
            orientation: value.orientation,
            ground_direction: value
                .ground_direction
                .as_ref()
                .map(Vector3::world_vec_to_vec3)
                .map(Vec3::normalize_or_zero)
                .unwrap_or(Vec3::NEG_Y),
            walk_success: value.walk_success,
            animation: value.animation,
            tilt_pitch: value.tilt_pitch.unwrap_or_default(),
            tilt_roll: value.tilt_roll.unwrap_or_default(),
        }
    }
}

// generic wrapper component that tracks the active provider of singleton player components
// e.g. AvatarMovement, by picking from the highest priority scene or falling back to default
// if none is found.
#[derive(Component, Clone)]
pub struct ActivePlayerComponent<C: Component> {
    scene: Entity,
    entity: Entity,
    scene_last_update: u32,
    scene_start_tick: u32,
    scene_is_portable: bool,
    /// Engine dispatch time (`RendererSceneContext::reply_sent`) of the scene tick
    /// that produced `component`. Used to reject AvatarMovement that was initiated
    /// before a `movePlayerTo`-imposed facing (and so read a stale transform).
    initiated_at: f64,
    pub component: C,
}

impl<C: Component> ActivePlayerComponent<C> {
    pub fn scene(&self) -> Entity {
        self.scene
    }
}

/// The velocities of the last completed movement step, in SDK world axes. The broadcast to the
/// scenes clears [`AvatarMovementInfo`] before `Update` systems and console commands can read it.
#[derive(Resource, Default, Clone, Copy, Debug)]
pub struct LastMovementStep {
    pub requested_velocity: Option<Vector3>,
    /// After collisions. Zero when the step did not move.
    pub actual_velocity: Option<Vector3>,
    /// From SDK forces and impulses.
    pub external_velocity: Option<Vector3>,
}

impl<C: Component + FromConfig> FromConfig for ActivePlayerComponent<C> {
    fn from_config(config: &AppConfig) -> Self {
        Self {
            scene: Entity::PLACEHOLDER,
            entity: Entity::PLACEHOLDER,
            scene_last_update: 0,
            scene_start_tick: 0,
            scene_is_portable: true,
            initiated_at: 0.0,
            component: C::from_config(config),
        }
    }
}

pub trait FromConfig {
    fn from_config(config: &AppConfig) -> Self;
}

impl<T: Default> FromConfig for T {
    fn from_config(_: &AppConfig) -> Self {
        Self::default()
    }
}

#[derive(Component)]
pub struct PhysicsCombinedForce(pub PbPhysicsCombinedForce);

impl From<PbPhysicsCombinedForce> for PhysicsCombinedForce {
    fn from(value: PbPhysicsCombinedForce) -> Self {
        Self(value)
    }
}

#[derive(Component)]
pub struct PhysicsCombinedImpulse(pub PbPhysicsCombinedImpulse);

impl From<PbPhysicsCombinedImpulse> for PhysicsCombinedImpulse {
    fn from(value: PbPhysicsCombinedImpulse) -> Self {
        Self(value)
    }
}

#[derive(Resource, Default)]
pub struct AvatarMovementInfo(pub PbAvatarMovementInfo);

// What was applied to the player since a scene was last sent an update. A scene can miss
// engine frames (its reply lands after the frame's scene-loop deadline), so each scene is
// told about the whole period since it last looked rather than the last frame only.
#[derive(Component, Default)]
pub(crate) struct MovementPeriod {
    time: f64,
    // requested and actual velocity are the latest frame's, not a total: scenes use them to
    // tell whether their own output is what was applied
    requested_velocity: Option<Vector3>,
    actual_velocity: Option<Vector3>,
    external_velocity: Option<Vector3>,
    // how far collision resolution pushed the player, and over how much time. Reported as the
    // mean push velocity, not a total: the engine holds a scene's velocity over the frames the
    // scene misses, so a collider pushing at a steady speed pushes again each frame and a total
    // would report a multiple of that speed
    collision_push: Vec3,
    collision_push_time: f64,
}

// How far this frame's collision resolution pushed the player out of colliders.
#[derive(Resource, Default)]
struct CollisionPush(Vec3);

impl<C: Component + Clone + FromConfig> ActivePlayerComponent<C> {
    // pick from available of any write-time, based on priority
    fn pick_by_priority(
        mut commands: Commands,
        q: Query<(Entity, Ref<C>, &SceneEntity)>,
        mut removed_components: RemovedComponents<C>,
        scenes: Query<&RendererSceneContext>,
        containing_scenes: ContainingScene,
        mut player: Query<&mut ActivePlayerComponent<C>, With<PrimaryUser>>,
        player_res: Res<PrimaryPlayerRes>,
        config: Res<AppConfig>,
    ) {
        let containing_scenes = containing_scenes.get(player_res.0);

        let Ok(mut current_choice) = player.single_mut() else {
            commands
                .entity(player_res.0)
                .try_insert(ActivePlayerComponent::<C>::from_config(&config));
            return;
        };

        // clear current choice if we left the scene or the component was removed
        let current_choice_valid = containing_scenes.contains(&current_choice.scene)
            && !removed_components
                .read()
                .any(|e| e == current_choice.entity);

        if !current_choice_valid {
            *current_choice = FromConfig::from_config(&config);
        }

        // find best choice: parcel first, then portables by most-recently spawned
        for (entity, update, scene_ent) in q.iter().filter(|(_, _, scene_ent)| {
            scene_ent.id == SceneEntityId::PLAYER && containing_scenes.contains(&scene_ent.root)
        }) {
            // skip unchanged
            if current_choice.entity == entity && !update.is_changed() {
                continue;
            }

            let Ok(ctx) = scenes.get(scene_ent.root) else {
                continue;
            };

            // prioritise parcel scenes
            if !current_choice.scene_is_portable && ctx.is_portable {
                continue;
            }

            // prioritise newer portables
            if ctx.is_portable && ctx.start_tick < current_choice.scene_start_tick {
                continue;
            }

            *current_choice = ActivePlayerComponent {
                scene: scene_ent.root,
                entity,
                scene_last_update: ctx.last_update_frame,
                scene_start_tick: ctx.start_tick,
                scene_is_portable: ctx.is_portable,
                initiated_at: ctx.reply_sent,
                component: update.clone(),
            };

            debug!("{} chose {}", std::any::type_name::<C>(), ctx.title);
        }
    }

    // pick based on priority, only from scenes that updated the component
    // in the last available scene-tick response
    fn pick_latest_frame_only_by_priority(
        mut commands: Commands,
        q: Query<(Entity, &C, &SceneEntity), Changed<C>>,
        scenes: Query<&RendererSceneContext>,
        containing_scenes: ContainingScene,
        mut player: Query<&mut ActivePlayerComponent<C>, With<PrimaryUser>>,
        player_res: Res<PrimaryPlayerRes>,
        config: Res<AppConfig>,
        ticks: SystemChangeTick,
        mut picked: ResMut<LatestFramePick<C>>,
    ) {
        picked.tick = ticks.this_run();
        let Ok(mut current_choice) = player.single_mut() else {
            commands
                .entity(player_res.0)
                .try_insert(ActivePlayerComponent::<C>::from_config(&config));
            return;
        };

        choose_latest_frame(
            &mut current_choice,
            q.iter(),
            &scenes,
            &containing_scenes.get(player_res.0),
            &config,
        );
    }
}

/// The choice of [`ActivePlayerComponent::pick_latest_frame_only_by_priority`], shared with
/// [`NextMovementPick`]. `changed` holds the components changed since the last pick.
fn choose_latest_frame<'a, C: Component + Clone + FromConfig>(
    current_choice: &mut impl DerefMut<Target = ActivePlayerComponent<C>>,
    changed: impl Iterator<Item = (Entity, &'a C, &'a SceneEntity)>,
    scenes: &Query<&RendererSceneContext>,
    containing_scenes: &HashSet<Entity>,
    config: &AppConfig,
) {
    // clear current choice if we left the scene or it has updated
    let current_choice_valid = containing_scenes.contains(&current_choice.scene)
        && scenes
            .get(current_choice.scene)
            .is_ok_and(|ctx| ctx.last_update_frame == current_choice.scene_last_update);

    if !current_choice_valid {
        **current_choice = FromConfig::from_config(config);
    }

    // find best choice: parcel first, then portables by most-recently spawned
    for (entity, update, scene_ent) in changed.filter(|(_, _, scene_ent)| {
        scene_ent.id == SceneEntityId::PLAYER && containing_scenes.contains(&scene_ent.root)
    }) {
        // prioritise parcel scenes
        if !current_choice.scene_is_portable {
            continue;
        }

        let Ok(ctx) = scenes.get(scene_ent.root) else {
            continue;
        };

        // prioritise newer portables
        if ctx.is_portable && ctx.start_tick <= current_choice.scene_start_tick {
            continue;
        }

        **current_choice = ActivePlayerComponent {
            scene: scene_ent.root,
            entity,
            scene_last_update: ctx.last_update_frame,
            scene_start_tick: ctx.start_tick,
            scene_is_portable: ctx.is_portable,
            initiated_at: ctx.reply_sent,
            component: update.clone(),
        };

        debug!("{} chose {}", std::any::type_name::<C>(), ctx.title);
    }
}

pub fn apply_impulses(
    impulses: Query<(&PhysicsCombinedImpulse, &SceneEntity), Changed<PhysicsCombinedImpulse>>,
    forces: Query<(Ref<PhysicsCombinedForce>, &SceneEntity)>,
    player: Res<PrimaryPlayerRes>,
    containing_scenes: ContainingScene,
    mut info: ResMut<AvatarMovementInfo>,
    time: Res<Time>,
) {
    let containing_scenes = containing_scenes.get(player.0);

    for (impulse, entity) in impulses {
        if !containing_scenes.contains(&entity.root) {
            continue;
        }

        info.0.external_velocity = Some(
            info.0.external_velocity.unwrap_or_default() + impulse.0.vector.unwrap_or_default(),
        );
    }

    for (force, entity) in forces {
        if !containing_scenes.contains(&entity.root) {
            continue;
        }

        info.0.external_velocity = Some(
            info.0.external_velocity.unwrap_or_default()
                + force.0.vector.unwrap_or_default() * time.delta_secs(),
        );
    }
}

// Whether scene-driven movement should be suppressed this frame: either an explicit
// suppression is held (e.g. movePlayerTo interpolation), OR the active AvatarMovement
// was initiated before the most recent movePlayerTo-imposed facing — such a tick read a
// pre-teleport transform, so applying its orientation would clobber the imposed facing.
// Strict `>` (here as `<=` to suppress) treats a same-frame dispatch as stale, since
// `reply_sent` is captured before the restricted-action runs and shares the frame's timestamp.
fn movement_suppressed(movement_control: &EngineMovementControl, initiated_at: f64) -> bool {
    !movement_control.suppress_avatar_physics.is_empty()
        || initiated_at <= movement_control.accept_movement_after
}

impl AvatarMovement {
    /// Yaw is authoritative (broadcast to other clients, used by physics); tilt is a render-only
    /// lean composed on top. `to_euler(YXZ).0` still recovers the yaw.
    fn rotation(&self) -> Quat {
        Quat::from_rotation_y(self.orientation / 360.0 * TAU)
            * avatar_tilt_quat(self.tilt_pitch, self.tilt_roll)
    }
}

// Movement pipeline step 1 (see the PBAvatarMovement proto comment): set the avatar
// orientation from the scene-driven movement info. Kept separate from velocity
// application (`apply_movement`) and ordered ahead of `apply_ground_collider_movement`
// so a rotating ground platform's rotation delta composes onto this orientation rather
// than being overwritten by it.
pub fn apply_rotation(
    mut player: Query<(&mut Transform, &ActivePlayerComponent<AvatarMovement>), With<PrimaryUser>>,
    movement_control: Res<EngineMovementControl>,
) {
    let Ok((mut transform, movement)) = player.single_mut() else {
        return;
    };

    if !movement_suppressed(&movement_control, movement.initiated_at) {
        transform.rotation = movement.component.rotation();
    }
}

/// The change tick of the last run of [`ActivePlayerComponent::pick_latest_frame_only_by_priority`]
/// for `C`. Its next run sees only the `C` changed after this tick.
#[derive(Resource)]
struct LatestFramePick<C> {
    tick: Tick,
    component: PhantomData<C>,
}

impl<C> Default for LatestFramePick<C> {
    fn default() -> Self {
        Self {
            tick: Tick::default(),
            component: PhantomData,
        }
    }
}

/// Predicts, during the scene loop, the [`AvatarMovement`] that
/// [`ActivePlayerComponent::pick_latest_frame_only_by_priority`] will choose after the loop.
#[derive(SystemParam)]
struct NextMovementPick<'w, 's> {
    movements: Query<'w, 's, (Entity, Ref<'static, AvatarMovement>, &'static SceneEntity)>,
    picked: Res<'w, LatestFramePick<AvatarMovement>>,
    ticks: SystemChangeTick,
    scenes: Query<'w, 's, &'static RendererSceneContext>,
    containing_scenes: ContainingScene<'w, 's>,
    player: Res<'w, PrimaryPlayerRes>,
    config: Res<'w, AppConfig>,
}

impl NextMovementPick<'_, '_> {
    fn choose(
        &self,
        current: &ActivePlayerComponent<AvatarMovement>,
    ) -> ActivePlayerComponent<AvatarMovement> {
        let mut choice = current.clone();
        let changed = self
            .movements
            .iter()
            .filter(|(_, movement, _)| {
                movement
                    .last_changed()
                    .is_newer_than(self.picked.tick, self.ticks.this_run())
            })
            .map(|(entity, movement, scene_entity)| (entity, movement.into_inner(), scene_entity));
        choose_latest_frame(
            &mut &mut choice,
            changed,
            &self.scenes,
            &self.containing_scenes.get(self.player.0),
            &self.config,
        );
        choice
    }
}

/// Stages on the movement scene the rotation that [`apply_rotation`] will set this frame, for the
/// re-dispatch that answers a late reply landing in this frame. Stages once per frame, after the
/// loop pass that applies the late reply, because the next pass sends the re-dispatch. Stages
/// nothing when [`movement_suppressed`] holds, so the re-dispatch gets the player's rotation. A
/// staged rotation stays only on the scene that drives the player, in its late reply's frame.
fn stage_redispatch_rotation(
    mut commands: Commands,
    player: Query<&ActivePlayerComponent<AvatarMovement>, With<PrimaryUser>>,
    tick_timings: Query<&SceneTickTiming>,
    staged: Query<Entity, With<RedispatchRotation>>,
    next_pick: NextMovementPick,
    movement_control: Res<EngineMovementControl>,
    frame: Res<FrameCount>,
    mut staged_in: Local<Option<u32>>,
) {
    let late = player.single().ok().filter(|active| {
        tick_timings
            .get(active.scene())
            .is_ok_and(|timing| timing.late_reply_in(frame.0))
    });
    for scene in &staged {
        if late.is_none_or(|active| active.scene() != scene) {
            commands.entity(scene).try_remove::<RedispatchRotation>();
        }
    }
    let Some(active) = late else {
        return;
    };
    if staged_in.replace(frame.0) == Some(frame.0) {
        return;
    }
    let choice = next_pick.choose(active);
    let mut scene = commands.entity(active.scene());
    if movement_suppressed(&movement_control, choice.initiated_at) {
        scene.try_remove::<RedispatchRotation>();
    } else {
        scene.try_insert(RedispatchRotation(choice.component.rotation()));
    }
}

pub(crate) fn apply_movement(
    mut player: Query<
        (
            &mut Transform,
            &mut AvatarDynamicState,
            &ActivePlayerComponent<AvatarMovement>,
        ),
        With<PrimaryUser>,
    >,
    mut scenes: Query<(Entity, &mut SceneColliderData)>,
    time_res: Res<Time>,
    mut info: ResMut<AvatarMovementInfo>,
    mut last_step: ResMut<LastMovementStep>,
    movement_control: Res<EngineMovementControl>,
    central: Res<CentralCollisions>,
) {
    let Ok((mut transform, mut dynamic_state, movement)) = player.single_mut() else {
        return;
    };
    let requested = Some(movement.component.velocity)
        .filter(|velocity| velocity.is_finite())
        .unwrap_or(Vec3::ZERO);

    info.0.step_time = time_res.delta_secs();

    // Orientation was already set this frame by `apply_rotation` (pipeline step 1);
    // here we only apply velocity. The same suppression gate still governs velocity.
    let suppress = movement_suppressed(&movement_control, movement.initiated_at);

    if suppress || requested == Vec3::ZERO {
        dynamic_state.velocity = Vec3::ZERO;
        dynamic_state.ground_height = ground_height(&mut scenes, transform.translation);
        end_jump_on_landing(&mut dynamic_state);
        *last_step = LastMovementStep {
            requested_velocity: Some(Vector3::world_vec_from_vec3(&requested)),
            actual_velocity: Some(Vector3::world_vec_from_vec3(&Vec3::ZERO)),
            external_velocity: info.0.external_velocity,
        };
        return;
    };

    let disabled = &central.0;

    if !disabled.is_empty() {
        warn!("move disabling {} colliders", disabled.len());
    }

    let mut position = transform.translation.as_dvec3();
    let mut time = time_res.delta_secs_f64();
    let mut velocity = requested.as_dvec3();
    let mut steps = 0;

    // deliberately penetrating (placed here by movePlayerTo): don't move deeper into
    // whatever we're inside, but otherwise let the scene walk us out
    if movement_control.deliberate_penetration {
        let (eject_min, eject_max) = movement_control.penetration_bounds;
        let floor = eject_min.cmpgt(DVec3::ZERO);
        let cap = eject_max.cmplt(DVec3::ZERO);
        velocity = DVec3::select(floor, velocity.max(DVec3::ZERO), velocity);
        velocity = DVec3::select(cap, velocity.min(DVec3::ZERO), velocity);
    }

    while steps < 60 && time > 1e-10 {
        steps += 1;
        let mut step_time = time;
        let mut contact_normal = DVec3::ZERO;
        if movement_control.suppress_clipping.is_empty() {
            for (e, mut collider_data) in scenes.iter_mut() {
                if let Some(hit) = collider_data.cast_avatar_nearest_in_scene(
                    position,
                    velocity,
                    step_time,
                    ColliderLayer::ClPhysics as u32 | GROUND_COLLISION_MASK,
                    movement_control.deliberate_penetration,
                    false,
                    disabled
                        .get(&e)
                        .map(|d| d.iter().collect())
                        .unwrap_or_default(),
                    -PLAYER_COLLIDER_OVERLAP,
                ) {
                    step_time = hit.toi as f64;
                    contact_normal = hit.normal.as_dvec3();
                }
            }
        }

        position += velocity * step_time + contact_normal * PLAYER_COLLIDER_OVERLAP as f64;
        velocity = velocity - (velocity.dot(contact_normal).min(0.0) * contact_normal);
        time -= step_time;
    }

    debug!(
        "move {:.7} + {:.7} = {:.7} ({steps} iterations)",
        transform.translation, requested, position
    );

    info.0.requested_velocity = Some(Vector3::world_vec_from_vec3(&requested));
    info.0.actual_velocity = Some(Vector3::world_vec_from_vec3(
        &actual_velocity(
            transform.translation.as_dvec3(),
            position,
            time_res.delta_secs_f64(),
        )
        .as_vec3(),
    ));
    *last_step = LastMovementStep {
        requested_velocity: info.0.requested_velocity,
        actual_velocity: info.0.actual_velocity,
        external_velocity: info.0.external_velocity,
    };

    let position = position.as_vec3();
    let velocity = velocity.as_vec3();

    transform.translation = position.with_y(position.y.max(0.0));

    // for now we hack in the old dynamic state values for animations
    let rose = dynamic_state.velocity.y > 0.0;
    dynamic_state.velocity = velocity;
    dynamic_state.ground_height = ground_height(&mut scenes, transform.translation);
    start_jump_on_take_off(&mut dynamic_state, rose, time_res.elapsed_secs_f64());
    end_jump_on_landing(&mut dynamic_state);
}

/// Zero for a step with no elapsed time, such as the first frame.
fn actual_velocity(from: DVec3, to: DVec3, delta: f64) -> DVec3 {
    if delta > 0.0 {
        (to - from) / delta
    } else {
        DVec3::ZERO
    }
}

/// The height of the feet above the ground the player stands on, in every scene that can ground it.
fn ground_height(scenes: &mut Query<(Entity, &mut SceneColliderData)>, feet: Vec3) -> f32 {
    scenes
        .iter_mut()
        .fold(feet.y, |ground, (_, mut collider_data)| {
            if !collider_data.grounds_avatar(feet.as_dvec3()) {
                return ground;
            }
            ground.min(
                collider_data
                    .get_standing_ground(feet)
                    .map(|(height, _)| height)
                    .unwrap_or(f32::INFINITY),
            )
        })
}

/// Record the take-off time when the player rises off the ground, or starts rising again in the
/// air (an air jump): off the ground is past [`PLAYER_LANDED_THRESHOLD`], where
/// [`end_jump_on_landing`] stops. `rose` is whether the player rose on the previous frame. The
/// rise, not its speed, marks the jump, so any jump height the movement scene uses counts.
fn start_jump_on_take_off(dynamic_state: &mut AvatarDynamicState, rose: bool, now: f64) {
    let rising = dynamic_state.velocity.y > 0.0;
    let airborne = dynamic_state.ground_height >= PLAYER_LANDED_THRESHOLD;
    if rising && airborne && (dynamic_state.jump_time <= 0.0 || !rose) {
        dynamic_state.jump_time = now;
    }
}

/// Forget the take-off time once the player is back on the ground and no longer rising, so that
/// `jump_time` marks a jump in flight (`<= 0.0` for none): the animation's take-off window and the
/// `jump_count` this client broadcasts both read it.
fn end_jump_on_landing(dynamic_state: &mut AvatarDynamicState) {
    if dynamic_state.ground_height < PLAYER_LANDED_THRESHOLD && dynamic_state.velocity.y <= 0.0 {
        dynamic_state.jump_time = -1.0;
    }
}

// Below this an impulse or force sum is no push: a zero `PhysicsCombinedForce` still sets
// `external_velocity`.
const NO_EXTERNAL_VELOCITY: f32 = 1e-3;

// The player's own walking, for analytics: the controller's displacement after collisions, so
// lifts, platforms and walls add nothing. Only a startup portable counts (launch-time, console or
// server-hosted, which includes the default controller); a parcel scene's override, a smart
// wearable or scene-spawned portable, a suppression, an impulse or force this frame, or no
// movement input held leaves none.
fn record_own_movement(
    mut player: Query<
        (
            &mut AvatarDynamicState,
            &ActivePlayerComponent<AvatarMovement>,
        ),
        With<PrimaryUser>,
    >,
    scenes: Query<&RendererSceneContext>,
    portables: Res<PortableScenes>,
    info: Res<AvatarMovementInfo>,
    movement_control: Res<EngineMovementControl>,
    input_manager: InputManager,
) {
    let Ok((mut dynamic_state, movement)) = player.single_mut() else {
        return;
    };
    let startup_controller = scenes
        .get(movement.scene())
        .ok()
        .filter(|ctx| ctx.is_portable)
        .and_then(|ctx| portables.get(&ctx.hash))
        .is_some_and(|source| source.is_startup());
    let pushed = info
        .0
        .external_velocity
        .is_some_and(|v| v.world_vec_to_vec3().length() > NO_EXTERNAL_VELOCITY);
    // a zero request skips `apply_movement`'s solve, leaving `actual_velocity` stale
    let own = startup_controller
        && movement.component.velocity != Vec3::ZERO
        && !movement_suppressed(&movement_control, movement.initiated_at)
        && !pushed
        && input_manager.get_analog(MOVE_SET, InputPriority::Scene) != Vec2::ZERO;
    let velocity = info
        .0
        .actual_velocity
        .filter(|_| own)
        .map_or(Vec3::ZERO, |v| v.world_vec_to_vec3());
    // `step_time` is this frame's delta, the one `actual_velocity` was divided by
    dynamic_state.own_displacement = velocity * info.0.step_time;
}

// (scene entity, collider id) of collider player is standing on
#[derive(Component, Default)]
pub struct GroundCollider(pub Option<(Entity, ColliderId, GlobalTransform)>);

fn record_ground_collider(
    mut player: Query<(
        Entity,
        &Transform,
        &ActivePlayerComponent<AvatarMovement>,
        &mut GroundCollider,
    )>,
    containing_scenes: ContainingScene,
    mut scenes: Query<&mut SceneColliderData>,
) {
    let Ok((player_ent, transform, movement, mut ground)) = player.single_mut() else {
        return;
    };

    ground.0 = None;

    if movement.component.ground_direction == Vec3::ZERO {
        return;
    }

    let mut best_height = PLAYER_GROUND_THRESHOLD;

    for scene in containing_scenes.get_area(player_ent, PLAYER_COLLIDER_RADIUS) {
        let Ok(mut collider_data) = scenes.get_mut(scene) else {
            continue;
        };
        if !collider_data.grounds_avatar(transform.translation.as_dvec3()) {
            continue;
        }

        if let Some((height, collider_id)) =
            collider_data.get_standing_ground(transform.translation)
        {
            if height < best_height {
                if let Some(entity) = collider_data.get_collider_entity(&collider_id) {
                    best_height = height;
                    ground.0 = Some((entity, collider_id.clone(), Default::default()));
                }
            }
        }
    }
}

fn apply_ground_collider_movement(
    ground_transforms: Query<(&GlobalTransform, &PreviousColliderTransform)>,
    mut player: Query<(&mut Transform, &GroundCollider), With<PrimaryUser>>,
    frame: Res<FrameCount>,
    // mut info: ResMut<AvatarMovementInfo>,
    // time: Res<Time>,
    movement_control: Res<EngineMovementControl>,
) {
    if !movement_control.suppress_avatar_physics.is_empty() {
        return;
    }

    let Ok((mut transform, GroundCollider(Some((ground_entity, _, _))))) = player.single_mut()
    else {
        return;
    };

    let Ok((
        new_global_transform,
        PreviousColliderTransform {
            prev_transform,
            updated,
        },
    )) = ground_transforms.get(*ground_entity)
    else {
        return;
    };

    if *updated == frame.0 {
        // update rotation
        let rotation_change = new_global_transform.to_scale_rotation_translation().1
            * prev_transform.to_scale_rotation_translation().1.inverse();
        // clamp to x/z plane to avoid twisting around
        let new_facing =
            ((rotation_change * Vec3::from(transform.forward())) * (Vec3::X + Vec3::Z)).normalize();
        transform.look_to(new_facing, Vec3::Y);

        // calculate updated translation
        let player_global_transform = GlobalTransform::from(*transform);
        let relative_position = player_global_transform.reparented_to(prev_transform);
        let new_transform = new_global_transform.mul_transform(relative_position);
        let new_translation = new_transform.translation();

        debug!(
            "ground collider {} + ? = {}",
            transform.translation, new_translation
        );

        if (new_translation - transform.translation).length() < 5.0 {
            // don't add ground collider movement to external_velocity, else we bounce/slide off everything
            transform.translation = new_translation;
        } else {
            debug!("skipped");
        }
    }
}

// below this required correction a deliberate penetration is considered resolved
const DELIBERATE_PENETRATION_CLEAR: f64 = 1e-3;
// consecutive resolved frames before the flag clears: a gltf reload (e.g. a scene
// changing a chair's collision mask when the player sits) drops its colliders for a
// frame before the replacements are registered
const DELIBERATE_PENETRATION_CLEAR_FRAMES: u32 = 2;

// (scene entity -> collider ids) intersecting the avatar's central segment, at the
// post-depenetration position. Computed once in `resolve_collisions`; `apply_movement`
// excludes them from the sweep since they can't be resolved by pushing out.
#[derive(Resource, Default)]
pub struct CentralCollisions(HashMap<Entity, HashSet<ColliderId>>);

fn resolve_collisions(
    mut player: Query<(&mut Transform, &ActivePlayerComponent<AvatarMovement>), With<PrimaryUser>>,
    mut scenes: Query<(Entity, &mut SceneColliderData)>,
    mut push: ResMut<CollisionPush>,
    time: Res<Time>,
    mut movement_control: ResMut<EngineMovementControl>,
    mut central: ResMut<CentralCollisions>,
    mut resolved_frames: Local<u32>,
) {
    if !movement_control.suppress_clipping.is_empty()
        || !movement_control.suppress_avatar_physics.is_empty()
    {
        return;
    }

    let Ok((mut transform, movement)) = player.single_mut() else {
        return;
    };

    let mut constraint_min = DVec3::NEG_INFINITY;
    let mut constraint_max = DVec3::INFINITY;

    let mut prev = DVec3::INFINITY;
    let mut current_offset = DVec3::ZERO;
    let mut iteration = 0;
    while (prev - current_offset).length() > PLAYER_COLLIDER_OVERLAP as f64 * 0.01 && iteration < 60
    {
        prev = current_offset;

        let position = transform.translation.as_dvec3() + current_offset;
        for (_, mut collider_data) in scenes.iter_mut() {
            if !collider_data.reaches_avatar(position) {
                continue;
            }
            // Note: collisions that intersect the avatar central segment are automatically excluded here
            let (scene_min, scene_max) = collider_data.avatar_constraints(position);

            constraint_min = constraint_min.max(scene_min + current_offset);
            constraint_max = constraint_max.min(scene_max + current_offset);
        }

        // vertical: satisfy floor over ceiling
        current_offset.y = current_offset.y.min(constraint_max.y).max(constraint_min.y);

        // x/z: average if squashed
        if constraint_min.x > constraint_max.x {
            current_offset.x = (constraint_min.x + constraint_max.x) * 0.5;
        } else {
            current_offset.x = current_offset.x.clamp(constraint_min.x, constraint_max.x);
        }

        if constraint_min.z > constraint_max.z {
            current_offset.z = (constraint_min.z + constraint_max.z) * 0.5;
        } else {
            current_offset.z = current_offset.z.clamp(constraint_min.z, constraint_max.z);
        }

        iteration += 1;
    }

    if (constraint_min, constraint_max) != (DVec3::NEG_INFINITY, DVec3::INFINITY) {
        debug!(
            "constraining {:.7} to ({:.7}, {:.7}) -> {:.7} ({iteration} iterations)",
            transform.translation,
            constraint_min,
            constraint_max,
            transform.translation + current_offset.as_vec3()
        );
    }

    let resolved = current_offset.length() < DELIBERATE_PENETRATION_CLEAR;

    if movement_control.deliberate_penetration {
        movement_control.penetration_bounds = (constraint_min, constraint_max);
        // vertical only, and only once the scene starts moving us: a floor sink
        // self-heals on the first step, a chair seat pops us up when we stand
        let v = movement.component.velocity;
        let moving = v.x != 0.0 || v.z != 0.0 || v.y > 0.0;
        if moving {
            transform.translation.y += current_offset.y as f32;
        }
        debug!(
            "deliberate penetration: offset {current_offset:.4} moving {moving} bounds ({constraint_min:.4}, {constraint_max:.4})"
        );
    } else {
        let current_offset = current_offset.as_vec3();

        if current_offset != Vec3::ZERO {
            push.0 = current_offset;
            debug!(
                "depenetration external velocity {:.4}",
                current_offset / time.delta_secs()
            );

            transform.translation += current_offset;
        }
    }

    central.0 = scenes
        .iter_mut()
        .filter(|(_, collider_data)| collider_data.reaches_avatar(transform.translation.as_dvec3()))
        .flat_map(|(scene, mut collider_data)| {
            let results = collider_data.avatar_central_collisions(transform.translation.as_dvec3());
            if results.is_empty() {
                None
            } else {
                Some((scene, results))
            }
        })
        .collect();

    // the solve ignores colliders that intersect the central segment, so a deeply
    // embedded collider looks resolved until we start walking out of it
    if movement_control.deliberate_penetration && resolved && central.0.is_empty() {
        *resolved_frames += 1;
        if *resolved_frames >= DELIBERATE_PENETRATION_CLEAR_FRAMES {
            debug!("deliberate penetration cleared");
            movement_control.deliberate_penetration = false;
        }
    } else {
        *resolved_frames = 0;
    }
}

pub(crate) fn broadcast_movement_info(
    mut info: ResMut<AvatarMovementInfo>,
    active_components: Query<
        (
            Option<&ActivePlayerComponent<AvatarLocomotionSettings>>,
            Option<&ActivePlayerComponent<InputModifier>>,
        ),
        With<PrimaryUser>,
    >,
    feedback: Res<SceneDrivenAnimationFeedback>,
    mut scenes: Query<(&mut RendererSceneContext, &MovementPeriod)>,
    time: Res<Time>,
    mut buf: Local<Vec<u8>>,
) {
    let (maybe_locomotion, maybe_modifier) = active_components.single().unwrap_or_default();

    info.0.active_avatar_locomotion_settings = maybe_locomotion.map(|l| l.component.0.clone());
    info.0.active_input_modifier = maybe_modifier.and_then(|l| l.component.0.clone());
    info.0.active_animation_state = feedback.state.as_ref().map(|s| AvatarAnimationState {
        src: s.src.clone(),
        r#loop: s.r#loop,
        speed: s.speed,
        idle: s.idle,
        playback_time: s.playback_time,
        duration: s.duration,
        loop_count: s.loop_count,
    });

    // the velocity a scene writes this tick is applied over this frame's delta
    info.0.step_time = time.delta_secs();

    debug!("broadcast {:?}", info.0);

    for (mut context, period) in scenes.iter_mut() {
        let mut scene_info = info.0.clone();
        scene_info.previous_step_time = period.time as f32;
        scene_info.requested_velocity = period.requested_velocity;
        scene_info.actual_velocity = period.actual_velocity;
        scene_info.external_velocity = if period.collision_push_time > 0.0 {
            let push = period.collision_push / period.collision_push_time as f32;
            Some(period.external_velocity.unwrap_or_default() + Vector3::world_vec_from_vec3(&push))
        } else {
            period.external_velocity
        };
        // walk_target is a world-space position, scenes read it relative to their origin
        if let Some(target) = scene_info.walk_target.as_mut() {
            target.x -= context.base.x as f32 * PARCEL_SIZE;
            target.z -= context.base.y as f32 * PARCEL_SIZE;
        }

        buf.clear();
        DclWriter::new(&mut buf).write(&scene_info);
        context.crdt_store.force_update(
            SceneComponentId::AVATAR_MOVEMENT_INFO,
            CrdtType::LWW_ANY,
            SceneEntityId::PLAYER,
            Some(&mut DclReader::new(&buf)),
        );
    }

    info.0 = PbAvatarMovementInfo {
        step_time: time.delta_secs(),
        previous_step_time: 0.0,
        requested_velocity: None,
        actual_velocity: None,
        external_velocity: None,
        active_avatar_locomotion_settings: None,
        active_input_modifier: None,
        walk_target: None,
        walk_threshold: None,
        active_animation_state: None,
    }
}

// A scene that was sent an update this frame has seen everything applied so far, so its
// period starts again.
fn start_movement_periods(
    mut scenes: Query<(&RendererSceneContext, &mut MovementPeriod)>,
    time: Res<Time>,
) {
    let now = time.elapsed_secs_f64();
    for (context, mut period) in scenes.iter_mut() {
        if context.last_sent == now {
            *period = default();
        }
    }
}

fn accumulate_movement_periods(
    mut periods: Query<&mut MovementPeriod>,
    info: Res<AvatarMovementInfo>,
    mut push: ResMut<CollisionPush>,
    time: Res<Time>,
) {
    let dt = time.delta_secs_f64();
    let push = std::mem::take(&mut push.0);
    for mut period in periods.iter_mut() {
        period.time += dt;
        period.requested_velocity = info.0.requested_velocity;
        period.actual_velocity = info.0.actual_velocity;
        if let Some(external) = info.0.external_velocity.as_ref() {
            period.external_velocity =
                Some(period.external_velocity.unwrap_or_default() + *external);
        }
        if push != Vec3::ZERO {
            period.collision_push += push;
            period.collision_push_time += dt;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::time::{TimePlugin, TimeUpdateStrategy};
    use common::structs::EngineMovementControl;
    use scene_runner::update_world::mesh_collider::ColliderId;

    fn set_velocity(app: &mut App, player: Entity, velocity: Vec3) {
        app.world_mut()
            .get_mut::<ActivePlayerComponent<AvatarMovement>>(player)
            .unwrap()
            .component
            .velocity = velocity;
    }

    /// `apply_movement` over a floor with its top at y 0, with the player still at `start`. The
    /// steps last 20 ms, except the first, which has no elapsed time.
    fn movement_app(start: Vec3) -> (App, Entity) {
        let mut app = App::new();
        app.add_plugins(TimePlugin)
            .insert_resource(TimeUpdateStrategy::ManualDuration(
                std::time::Duration::from_millis(20),
            ))
            .init_resource::<AvatarMovementInfo>()
            .init_resource::<LastMovementStep>()
            .init_resource::<CentralCollisions>()
            .insert_resource(EngineMovementControl {
                accept_movement_after: -1.0,
                ..default()
            })
            .add_systems(PostUpdate, apply_movement);
        let mut colliders = SceneColliderData::default();
        let id = ColliderId::new(SceneEntityId::ROOT, None, 0);
        colliders.set_wall(
            &id,
            Vec3::new(start.x, -0.5, start.z),
            Vec3::new(50.0, 0.5, 50.0),
            Quat::IDENTITY,
        );
        app.world_mut().spawn(colliders);
        let movement = ActivePlayerComponent::<AvatarMovement>::from_config(&default());
        let player = app
            .world_mut()
            .spawn((
                PrimaryUser::default(),
                movement,
                AvatarDynamicState::default(),
                Transform::from_translation(start),
            ))
            .id();
        (app, player)
    }

    fn add_scene_resources(app: &mut App) {
        use scene_runner::initialize_scene::{LiveScenes, PortableScenes, ScenePointers};

        app.init_resource::<LiveScenes>()
            .init_resource::<PortableScenes>()
            .init_resource::<ScenePointers>();
    }

    /// `Some(tick)` makes a portable scene that started at that tick. `None` makes a parcel scene.
    fn scene_context(scene: Entity, hash: &str, portable: Option<u32>) -> RendererSceneContext {
        use dcl_types::SceneId;

        RendererSceneContext::new(
            SceneId::DUMMY,
            hash.to_owned(),
            "storage_root".to_owned(),
            portable.is_some(),
            portable.unwrap_or_default(),
            hash.to_owned(),
            IVec2::ZERO,
            HashSet::from_iter([IVec2::ZERO]),
            vec![],
            default(),
            vec![],
            scene,
            0.0,
            false,
            "sdk_version",
            false,
            false,
        )
    }

    fn place_scene(
        app: &mut App,
        scene: Entity,
        hash: &str,
        parcels: impl IntoIterator<Item = IVec2>,
    ) {
        use scene_runner::initialize_scene::{LiveScenes, PointerResult, ScenePointers};

        let world = app.world_mut();
        world
            .entity_mut(scene)
            .insert(scene_context(scene, hash, None));
        world
            .resource_mut::<LiveScenes>()
            .scenes
            .insert(hash.to_owned(), scene);
        let mut pointers = world.resource_mut::<ScenePointers>();
        for parcel in parcels {
            pointers.insert(
                parcel,
                PointerResult::Exists {
                    hash: hash.to_owned(),
                    urn: None,
                },
            );
        }
    }

    fn run_for(app: &mut App, player: Entity, velocity: Vec3, frames: usize) -> Vec3 {
        set_velocity(app, player, velocity);
        for _ in 0..frames {
            app.update();
        }
        app.world().get::<Transform>(player).unwrap().translation
    }

    #[test]
    fn a_scene_velocity_that_is_not_finite_moves_nothing() {
        for velocity in [
            Vec3::NAN,
            Vec3::new(f32::INFINITY, 0.0, 0.0),
            Vec3::new(0.0, 0.0, f32::NEG_INFINITY),
        ] {
            let start = Vec3::new(8.0, 0.0, -8.0);
            let (mut app, player) = movement_app(start);
            app.update();
            let end = run_for(&mut app, player, velocity, 3);
            assert_eq!(end, start, "{velocity}");
        }
    }

    #[test]
    fn without_a_scene_setting_the_hard_landing_cooldown_reaches_scenes_unset() {
        let settings = AvatarLocomotionSettings::from_config(&default()).0;
        assert_eq!(settings.hard_landing_cooldown, None);
        assert!(settings.walk_speed.is_some() && settings.jump_height.is_some());
    }

    fn yaw(rotation: Quat) -> f32 {
        rotation.to_euler(EulerRot::YXZ).0.to_degrees()
    }

    /// `Update` stands for a pass of the scene loop. The player stands in the middle of parcel 0,0.
    fn rotation_app(accept_movement_after: f64) -> (App, Entity) {
        let mut app = App::new();
        app.insert_resource(EngineMovementControl {
            accept_movement_after,
            ..default()
        })
        .init_resource::<AppConfig>()
        .init_resource::<LatestFramePick<AvatarMovement>>()
        .insert_resource(FrameCount(0))
        .add_systems(Update, stage_redispatch_rotation)
        .add_systems(
            PostUpdate,
            (
                ActivePlayerComponent::<AvatarMovement>::pick_latest_frame_only_by_priority,
                apply_rotation,
            )
                .chain(),
        );
        add_scene_resources(&mut app);
        let player = app
            .world_mut()
            .spawn((
                PrimaryUser::default(),
                ActivePlayerComponent::<AvatarMovement>::from_config(&default()),
                Transform::default(),
                GlobalTransform::from_translation(Vec3::new(8.0, 0.0, -8.0)),
            ))
            .id();
        app.insert_resource(PrimaryPlayerRes(player));
        (app, player)
    }

    /// Returns the scene and its player entity, which holds the scene's `AvatarMovement`. `None`
    /// makes the scene of parcel 0,0.
    fn add_scene(app: &mut App, hash: &str, portable: Option<u32>) -> (Entity, Entity) {
        use dcl_types::SceneId;
        use scene_runner::initialize_scene::{LiveScenes, PortableScenes, PortableSource};

        let scene = app.world_mut().spawn_empty().id();
        let scene_player = app
            .world_mut()
            .spawn(SceneEntity {
                root: scene,
                scene_id: SceneId::DUMMY,
                id: SceneEntityId::PLAYER,
            })
            .id();
        if portable.is_some() {
            let world = app.world_mut();
            world
                .entity_mut(scene)
                .insert(scene_context(scene, hash, portable));
            world
                .resource_mut::<LiveScenes>()
                .scenes
                .insert(hash.to_owned(), scene);
            world.resource_mut::<PortableScenes>().insert(
                hash.to_owned(),
                PortableSource::new(hash.to_owned(), None, None, false),
            );
        } else {
            place_scene(app, scene, hash, [IVec2::ZERO]);
        }
        app.world_mut()
            .get_mut::<RendererSceneContext>(scene)
            .unwrap()
            .associate_bevy_entity(SceneEntityId::PLAYER, scene_player);
        (scene, scene_player)
    }

    /// `None` sets no movement. The engine clock reads 1 s at frame 0 and advances 1 s a frame.
    fn land_reply(
        app: &mut App,
        (scene, scene_player): (Entity, Entity),
        sent: u32,
        frame: u32,
        orientation: Option<f32>,
    ) {
        use scene_runner::renderer_context::{ReplyTiming, SentTiming};

        let world = app.world_mut();
        if let Some(orientation) = orientation {
            world.entity_mut(scene_player).insert(AvatarMovement {
                orientation,
                ..default()
            });
        }
        let at = web_time::Instant::now();
        let sent = SentTiming { at, frame: sent };
        world.get_mut::<SceneTickTiming>(scene).unwrap().reply =
            Some(ReplyTiming { sent, at, frame });
        let mut context = world.get_mut::<RendererSceneContext>(scene).unwrap();
        context.reply_sent = f64::from(sent.frame) + 1.0;
        context.last_update_frame = frame;
    }

    /// The staged rotation in the frame of a late reply, else the player's rotation.
    fn rotation_to_send(world: &World, scene: Entity, player: Entity, frame: u32) -> Quat {
        world
            .get::<RedispatchRotation>(scene)
            .filter(|_| {
                world
                    .get::<SceneTickTiming>(scene)
                    .unwrap()
                    .late_reply_in(frame)
            })
            .map_or(world.get::<Transform>(player).unwrap().rotation, |staged| {
                staged.0
            })
    }

    /// Returns the yaw that a dispatch of `scene` in `frame` is sent, and the player's yaw after
    /// the frame.
    fn run_frame(app: &mut App, frame: u32, scene: Entity, player: Entity) -> (f32, f32) {
        app.insert_resource(FrameCount(frame));
        app.world_mut().run_schedule(Update);
        let sent = rotation_to_send(app.world(), scene, player, frame);
        app.world_mut().run_schedule(PostUpdate);
        let applied = app.world().get::<Transform>(player).unwrap().rotation;
        (yaw(sent), yaw(applied))
    }

    /// Each reply of the movement scene lands a frame late and is re-dispatched in that frame.
    /// Each of the first `turning` ticks turns 10 degrees toward 90 from the yaw it is sent. Later
    /// ticks hold that yaw. `imposed` starts the player at that yaw, and every reply predates it.
    /// Returns, for each frame, the yaw sent and the player's yaw after the frame.
    fn late_replies(turning: u32, frames: u32, imposed: Option<f32>) -> Vec<(f32, f32)> {
        let (mut app, player) = rotation_app(if imposed.is_some() { f64::MAX } else { 0.0 });
        let scene = add_scene(&mut app, "movement", Some(1));
        let start = imposed.unwrap_or_default();
        app.world_mut()
            .get_mut::<Transform>(player)
            .unwrap()
            .rotation = Quat::from_rotation_y(start.to_radians());
        // the scene has been driving the player
        app.world_mut().entity_mut(scene.1).insert(AvatarMovement {
            orientation: start,
            ..default()
        });

        let mut in_flight = None;
        let mut frames_out = Vec::new();
        for frame in 0..frames {
            if let Some(orientation) = in_flight.take() {
                land_reply(&mut app, scene, frame - 1, frame, Some(orientation));
            }
            let (sent, applied) = run_frame(&mut app, frame, scene.0, player);
            in_flight = Some(if frame < turning {
                sent + (90.0 - sent).clamp(-10.0, 10.0)
            } else {
                sent
            });
            frames_out.push((sent, applied));
        }
        frames_out
    }

    /// A parcel scene takes over at 70 degrees, then something else turns the player to 40.
    #[test]
    fn a_scene_no_longer_driving_the_player_is_not_sent_its_staged_rotation() {
        let (mut app, player) = rotation_app(0.0);
        let movement = add_scene(&mut app, "movement", Some(1));
        let parcel = add_scene(&mut app, "parcel", None);
        land_reply(&mut app, movement, 0, 0, Some(10.0));
        run_frame(&mut app, 0, movement.0, player);

        land_reply(&mut app, movement, 0, 1, Some(30.0));
        land_reply(&mut app, parcel, 1, 1, Some(70.0));
        let (staged, _) = run_frame(&mut app, 1, movement.0, player);
        assert!((staged - 70.0).abs() < 1e-3, "staged {staged}");
        assert_eq!(
            app.world()
                .get::<ActivePlayerComponent<AvatarMovement>>(player)
                .unwrap()
                .scene(),
            parcel.0
        );
        app.world_mut()
            .get_mut::<Transform>(player)
            .unwrap()
            .rotation = Quat::from_rotation_y(40f32.to_radians());

        land_reply(&mut app, movement, 1, 2, None);
        let (sent, _) = run_frame(&mut app, 2, movement.0, player);
        assert!((sent - 40.0).abs() < 1e-3, "sent {sent}");
    }

    #[test]
    fn a_redispatch_is_sent_the_rotation_of_a_scene_taking_over_in_its_frame() {
        let (mut app, player) = rotation_app(0.0);
        let movement = add_scene(&mut app, "movement", Some(1));
        let parcel = add_scene(&mut app, "parcel", None);
        land_reply(&mut app, movement, 0, 0, Some(10.0));
        run_frame(&mut app, 0, movement.0, player);

        land_reply(&mut app, movement, 0, 1, Some(30.0));
        land_reply(&mut app, parcel, 1, 1, Some(70.0));
        let (sent, applied) = run_frame(&mut app, 1, movement.0, player);
        assert!((applied - 70.0).abs() < 1e-3, "applied {applied}");
        assert!(
            (sent - applied).abs() < 1e-3,
            "sent {sent}, applied {applied}"
        );
    }

    /// The turn to 40 degrees stands for a rotating platform.
    #[test]
    fn a_late_reply_without_movement_redispatches_the_players_rotation() {
        let (mut app, player) = rotation_app(0.0);
        let movement = add_scene(&mut app, "movement", Some(1));
        land_reply(&mut app, movement, 0, 0, Some(10.0));
        run_frame(&mut app, 0, movement.0, player);
        app.world_mut()
            .get_mut::<Transform>(player)
            .unwrap()
            .rotation = Quat::from_rotation_y(40f32.to_radians());

        land_reply(&mut app, movement, 0, 1, None);
        let (sent, applied) = run_frame(&mut app, 1, movement.0, player);
        assert!((applied - 40.0).abs() < 1e-3, "applied {applied}");
        assert!(
            (sent - applied).abs() < 1e-3,
            "sent {sent}, applied {applied}"
        );
    }

    #[test]
    fn a_redispatch_turns_on_from_the_late_reply_it_answers() {
        let frames = late_replies(3, 12, None);
        let yaws: Vec<f32> = frames.iter().map(|(_, applied)| *applied).collect();
        assert!(
            yaws.windows(2).all(|pair| pair[1] >= pair[0] - 1e-3),
            "turned back: {yaws:?}"
        );
        // three ticks with input turn 30 degrees
        assert!(
            yaws[4..].iter().all(|yaw| (yaw - 30.0).abs() < 1e-3),
            "settled at {yaws:?}"
        );
    }

    /// As [`late_replies`], but the reply to tick `k` lands `schedule[k % schedule.len()]` frames
    /// after its dispatch. Returns the (sent, replied) yaw of each tick, and the player's yaw
    /// after each frame.
    fn scheduled_replies(
        schedule: &[u32],
        turning: usize,
        frames: u32,
    ) -> (Vec<(f32, f32)>, Vec<f32>) {
        let (mut app, player) = rotation_app(0.0);
        let scene = add_scene(&mut app, "movement", Some(1));
        // the scene has been driving the player
        app.world_mut()
            .entity_mut(scene.1)
            .insert(AvatarMovement::default());

        // (dispatch frame, landing frame, orientation) of the tick in flight
        let mut in_flight: Option<(u32, u32, f32)> = None;
        let mut ticks = Vec::new();
        let mut applied = Vec::new();
        for frame in 0..frames {
            app.insert_resource(FrameCount(frame));
            if let Some((sent, _, orientation)) =
                in_flight.take_if(|&mut (_, lands, _)| lands == frame)
            {
                land_reply(&mut app, scene, sent, frame, Some(orientation));
            }
            app.world_mut().run_schedule(Update);
            if in_flight.is_none() {
                let sent = yaw(rotation_to_send(app.world(), scene.0, player, frame));
                let orientation = if ticks.len() < turning {
                    sent + (90.0 - sent).clamp(-10.0, 10.0)
                } else {
                    sent
                };
                let lands = frame + schedule[ticks.len() % schedule.len()];
                ticks.push((sent, orientation));
                if lands == frame {
                    land_reply(&mut app, scene, frame, frame, Some(orientation));
                } else {
                    in_flight = Some((frame, lands, orientation));
                }
            }
            app.world_mut().run_schedule(PostUpdate);
            applied.push(yaw(app.world().get::<Transform>(player).unwrap().rotation));
        }
        (ticks, applied)
    }

    #[test]
    fn a_redispatch_turns_on_from_a_reply_two_frames_late_or_between_in_frame_ones() {
        for schedule in [&[2][..], &[0, 1], &[1, 2]] {
            let (ticks, applied) = scheduled_replies(schedule, 3, 16);
            // every tick is sent the yaw the one before it replied
            assert!(
                ticks
                    .windows(2)
                    .all(|pair| (pair[1].0 - pair[0].1).abs() < 1e-3),
                "{schedule:?}: (sent, replied) {ticks:?}"
            );
            assert!(
                applied.windows(2).all(|pair| pair[1] >= pair[0] - 1e-3),
                "{schedule:?}: turned back: {applied:?}"
            );
            // three ticks with input turn 30 degrees
            let last = applied.last().unwrap();
            assert!(
                (last - 30.0).abs() < 1e-3,
                "{schedule:?}: settled at {applied:?}"
            );
        }
    }

    #[test]
    fn a_redispatch_is_sent_an_imposed_facing_its_late_reply_does_not_override() {
        let frames = late_replies(3, 6, Some(45.0));
        assert!(
            frames
                .iter()
                .all(|(sent, applied)| (sent - 45.0).abs() < 1e-3 && (applied - 45.0).abs() < 1e-3),
            "{frames:?}"
        );
    }

    #[test]
    fn unset_locomotion_speeds_use_the_players_own() {
        let base = PrimaryUser {
            walk_speed: 3.0,
            jog_speed: 7.0,
            run_speed: 9.0,
            ..Default::default()
        };
        assert_eq!(
            locomotion_speeds(&base, None),
            LocomotionSpeeds {
                walk: 3.0,
                jog: 7.0,
                run: 9.0
            }
        );
        let settings = PbAvatarLocomotionSettings {
            walk_speed: Some(1.0),
            run_speed: Some(20.0),
            ..Default::default()
        };
        assert_eq!(
            locomotion_speeds(&base, Some(&settings)),
            LocomotionSpeeds {
                walk: 1.0,
                jog: 7.0,
                run: 20.0
            }
        );
    }
}

#[cfg(test)]
mod scene_reach_tests;

#[cfg(test)]
mod jump_tests {
    use super::*;

    fn state(ground_height: f32, vertical_velocity: f32) -> AvatarDynamicState {
        AvatarDynamicState {
            velocity: Vec3::new(0.0, vertical_velocity, 0.0),
            ground_height,
            jump_time: 5.0,
            ..default()
        }
    }

    #[test]
    fn landing_ends_the_jump_in_flight() {
        let mut landed = state(0.0, 0.0);
        end_jump_on_landing(&mut landed);
        assert!(landed.jump_time <= 0.0);
    }

    #[test]
    fn the_take_off_frame_and_the_flight_keep_the_jump() {
        // still within the grounded height on the frame the jump starts, but rising
        let mut taking_off = state(0.1, 11.0);
        end_jump_on_landing(&mut taking_off);
        assert_eq!(taking_off.jump_time, 5.0);

        let mut airborne = state(1.5, -3.0);
        end_jump_on_landing(&mut airborne);
        assert_eq!(airborne.jump_time, 5.0);
    }

    /// The take-off time a jump of `height` records, rising from the ground at 60 frames a second
    /// as the movement scene launches it, and checks that the rise keeps it.
    fn take_off_of_a_jump(height: f32) -> f64 {
        // the movement scene's deceleration of a jump's rise (`JUMP_RISE_DECEL`)
        const RISE_DECEL: f32 = 40.0;
        let dt = 1.0 / 60.0;
        let mut dynamic_state = AvatarDynamicState::default();
        let mut vertical_velocity = (2.0 * height * RISE_DECEL).sqrt();
        let mut take_off = None;
        for frame in 1..120 {
            if vertical_velocity <= 0.0 {
                break;
            }
            let rose = dynamic_state.velocity.y > 0.0;
            dynamic_state.velocity.y = vertical_velocity;
            dynamic_state.ground_height += vertical_velocity * dt;
            start_jump_on_take_off(&mut dynamic_state, rose, f64::from(frame) * f64::from(dt));
            end_jump_on_landing(&mut dynamic_state);
            if dynamic_state.jump_time > 0.0 {
                assert_eq!(
                    *take_off.get_or_insert(dynamic_state.jump_time),
                    dynamic_state.jump_time
                );
            }
            vertical_velocity -= RISE_DECEL * dt;
        }
        take_off.expect("the jump records its take-off")
    }

    #[test]
    fn a_jump_records_its_take_off_whatever_its_height() {
        // the default jump, and a lower one a scene's locomotion settings may ask for
        assert!(take_off_of_a_jump(1.0) < 0.1);
        assert!(take_off_of_a_jump(0.5) < 0.1);
    }

    #[test]
    fn rising_on_the_ground_is_no_jump() {
        // following the ground up a slope or a step
        let mut walking = state(0.0, 3.0);
        walking.jump_time = -1.0;
        start_jump_on_take_off(&mut walking, true, 9.0);
        assert!(walking.jump_time <= 0.0);
    }

    #[test]
    fn an_air_jump_records_a_new_take_off() {
        // the air jump holds the height, then launches
        let mut hovering = state(1.5, 0.0);
        start_jump_on_take_off(&mut hovering, false, 6.0);
        assert_eq!(hovering.jump_time, 5.0);

        let mut launched = state(1.5, 8.0);
        start_jump_on_take_off(&mut launched, false, 6.2);
        assert_eq!(launched.jump_time, 6.2);

        let mut rising = state(2.0, 6.0);
        rising.jump_time = 6.2;
        start_jump_on_take_off(&mut rising, true, 6.3);
        assert_eq!(rising.jump_time, 6.2);
    }
}
