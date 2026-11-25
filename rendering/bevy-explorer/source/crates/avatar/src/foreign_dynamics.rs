use bevy::prelude::*;

use common::{
    sets::SceneSets,
    structs::{
        avatar_tilt_quat, AvatarDynamicState, ForeignAvatar, MoveKind, SceneDrivenAnim,
        SceneDrivenAnimationRequest,
    },
    util::QuatNormalizeExt,
};

use crate::animate::{ActiveEmote, ActiveEmoteSource};
use comms::global_crdt::{
    process_transport_updates, ForeignPlayer, PlayerPositionEvent, PlayerSceneAnimEvent,
};
use dcl_component::{transform_and_parent::DclTransformAndParent, SceneEntityId};
use scene_runner::{update_world::mesh_collider::SceneColliderData, ContainingScene};

/// A backward jump beyond this reads as a server restart (the Pulse `server_tick` resets to ~0), so
/// we re-sync to it instead of freezing; smaller backward steps are reordered/duplicate datagrams.
const TIMESTAMP_RESET_SECS: f64 = 60.0;

/// Time constant for dead-reckoning a foreign avatar past its last observation. The extrapolated
/// offset saturates at `velocity * COAST_SECS` instead of growing with the silence, and the
/// reported velocity decays to match, so a peer whose updates stop coasts a plausible distance and
/// settles rather than departing in a straight line forever.
///
/// Sized so ordinary operation is untouched: between 10Hz packets the staleness is a frame or two,
/// where this is within ~1% of plain `velocity * elapsed`. Damping only becomes visible once an
/// update is genuinely late — which is exactly when extrapolating a stale velocity is unsound.
const COAST_SECS: f32 = 1.0;

use bevy_console::ConsoleCommand;
use common::util::AsH160;
use comms::profile::UserProfile;
use console::{AddDebugSection, DebugInput, DebugInputKind, DebugSection, DoAddConsoleCommand};
use serde_json::json;

pub struct PlayerMovementPlugin;

impl Plugin for PlayerMovementPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MovementProfile>()
            .add_console_command::<MovementProfileCommand, _>(movement_profile)
            .add_console_command::<ForeignMovementCommand, _>(foreign_movement)
            .add_debug_section(DebugSection {
                title: "Multiplayer movement",
                query: "/foreign_movement",
                fields: &[
                    ("Remote players", "count"),
                    ("Profile", "name"),
                    ("Wallet", "wallet"),
                    ("Entity", "entity"),
                    ("Profile version", "version"),
                    ("Last target was a teleport", "teleport"),
                    ("Packet timestamp (s)", "timestamp"),
                    ("Time since receipt (s)", "age"),
                    ("Estimated packet interval (s)", "interval"),
                    ("Rendered position (X / Y / Z)", "position"),
                    ("Target position (X / Y / Z)", "target"),
                    ("Target yaw (degrees)", "yaw"),
                    ("Packet velocity (X / Y / Z)", "velocity"),
                    ("Movement kind", "movement"),
                    (
                        "Remote players animated by a room clip",
                        "animation.room_clip",
                    ),
                    (
                        "Remote players animated by the velocity fallback",
                        "animation.fallback",
                    ),
                    (
                        "Remote players with a triggered emote or none yet",
                        "animation.other",
                    ),
                    ("Jump take-off in flight", "jump"),
                ],
                actions: &[("Inspect first remote player", "/movement_profile")],
                inputs: &[DebugInput {
                    label: "Inspect name or wallet",
                    field: "selector",
                    command: "/movement_profile {value}",
                    kind: DebugInputKind::Text,
                }],
            });
        app.add_systems(
            Update,
            (
                update_foreign_user_target_position,
                update_foreign_scene_anim,
                update_foreign_user_actual_position,
            )
                .chain()
                // Apply the rejoin pose before PostLoop visibility can reveal the avatar.
                .after(process_transport_updates)
                // the jump take-off clip restarts on the frame `jump_time` is set, so the
                // animation must see that frame
                .before(SceneSets::PostLoop),
        );
    }
}

#[derive(Resource, Default)]
struct MovementProfile(Option<String>);

/// Select a remote player by name or wallet. Omit the selector to inspect the first peer.
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/movement_profile")]
struct MovementProfileCommand {
    selector: Vec<String>,
}

fn movement_profile(
    mut input: ConsoleCommand<MovementProfileCommand>,
    mut profile: ResMut<MovementProfile>,
) {
    if let Some(Ok(command)) = input.take() {
        profile.0 = (!command.selector.is_empty()).then(|| command.selector.join(" "));
        input.reply_ok("Movement profile selected");
    }
}

/// How many remote players each animation source currently drives.
#[derive(Debug, Default, PartialEq, Eq)]
struct AnimationSourceCounts {
    room_clip: usize,
    fallback: usize,
    other: usize,
}

impl AnimationSourceCounts {
    /// `None` is a player that has no animation state yet; it counts as neither source.
    fn count(sources: impl IntoIterator<Item = Option<ActiveEmoteSource>>) -> Self {
        let mut counts = Self::default();
        for source in sources {
            match source {
                Some(ActiveEmoteSource::SceneMovementAnim) => counts.room_clip += 1,
                Some(ActiveEmoteSource::VelocitySelected) => counts.fallback += 1,
                Some(ActiveEmoteSource::TriggeredEmote) | None => counts.other += 1,
            }
        }
        counts
    }
}

/// Read the accepted network target and current rendered transform of a remote player.
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/foreign_movement")]
struct ForeignMovementCommand;

fn foreign_movement(
    mut input: ConsoleCommand<ForeignMovementCommand>,
    filter: Res<MovementProfile>,
    time: Res<Time>,
    peers: Query<(
        Entity,
        &ForeignPlayer,
        Option<&UserProfile>,
        Option<&Transform>,
        Option<&PlayerTargetPosition>,
        Option<&AvatarDynamicState>,
    )>,
    emotes: Query<Option<&ActiveEmote>, With<ForeignPlayer>>,
) {
    let Some(Ok(_)) = input.take() else { return };
    let selector = filter.0.as_deref();
    let address = selector.and_then(|id| id.as_h160());
    let count = peers.iter().count();
    let animation = AnimationSourceCounts::count(emotes.iter().map(|e| e.map(|e| e.source())));
    let mut report = json!({
        "count": count,
        "selector": selector,
        "animation": {
            "room_clip": animation.room_clip,
            "fallback": animation.fallback,
            "other": animation.other,
        },
    });
    if let Some((entity, player, profile, pose, target, dynamics)) = peers
        .iter()
        .filter(|(_, player, profile, ..)| {
            selector.is_none_or(|id| {
                Some(player.address) == address
                    || profile.is_some_and(|p| p.content.name.eq_ignore_ascii_case(id))
            })
        })
        .min_by_key(|(entity, player, ..)| (player.address, *entity))
    {
        report["entity"] = json!(format!("{entity:?}"));
        report["wallet"] = json!(format!("{:#x}", player.address));
        report["name"] = json!(profile.map(|p| p.content.name.as_str()));
        report["version"] = json!(player.profile_version);
        report["position"] = json!(pose.map(|t| t.translation.to_array()));
        report["jump"] = json!(dynamics.map(|d| d.jump_time > 0.0));
        report["movement"] = json!(dynamics.map(|d| match d.move_kind {
            MoveKind::Idle => "idle",
            MoveKind::Walk => "walk",
            MoveKind::Jog => "jog",
            MoveKind::Run => "run",
            MoveKind::Jump => "jump",
            MoveKind::DoubleJump => "double jump",
            MoveKind::Falling => "falling",
            MoveKind::LongFalling => "long falling",
            MoveKind::Glide => "glide",
            MoveKind::Emote => "emote",
        }));
        report["unavailable"] = json!({
            "": "No movement packet received for this player",
            "name": "Waiting for the player profile",
            "position": "The player has no rendered transform",
            "movement": "The player has no animation state",
            "jump": "The player has no animation state",
        });
        if let Some(target) = target {
            report["teleport"] = json!(target.teleport);
            report["timestamp"] = json!(target.timestamp);
            report["age"] = json!((time.elapsed_secs_f64() - target.time).max(0.0));
            report["interval"] = json!(target.update_freq);
            report["target"] = json!(target.translation.to_array());
            report["velocity"] = json!(target.velocity.map(|v| v.to_array()));
            report["yaw"] = json!(target.rotation.to_euler(EulerRot::YXZ).0.to_degrees());
            report["unavailable"]["timestamp"] = json!("The movement packet has no timestamp");
            report["unavailable"]["velocity"] = json!("The movement packet has no velocity");
        }
    } else {
        report["unavailable"] = json!({"": if count == 0 {
            "No remote players are present"
        } else {
            "No remote player matches this name or wallet"
        }});
    }
    input.reply_ok(report.to_string());
}

#[derive(Component)]
struct PlayerTargetPosition {
    time: f64,
    /// The source clock of the last applied update, seconds (see `PlayerPositionEvent`).
    timestamp: Option<f64>,
    velocity: Option<Vec3>,
    /// Already box-resolved against the quantization grid when the packet landed (see
    /// `update_foreign_user_target_position`); the interpolator treats it as an exact, stable
    /// goalpost.
    translation: Vec3,
    rotation: Quat,
    update_freq: f32,
    grounded: Option<bool>,
    // Jump / DoubleJump / Glide inferred from the rfc4::Movement packet. Applied to the
    // avatar's move_kind so the velocity picker can drive jump_time (Jump) or select the
    // matching emote (DoubleJump / Glide); None resets a previously-applied remote state
    // back to Idle so the picker reclaims.
    remote_move_kind: Option<MoveKind>,
    /// First placement or a discontinuous target, as reported by the movement debug panel.
    teleport: bool,
    /// A discontinuity still needs to reach the rendered pose, even if a newer delta arrived
    /// in the same frame.
    snap_pending: bool,
}

/// Pending scene-driven animation for a foreign player, fed by [`PlayerSceneAnimEvent`] (which rides
/// LiveKit, separate from the Pulse position stream) and applied in sync with the position packet
/// whose motion it belongs to, so the clip starts/stops with the visible avatar. Ordering/dedup is
/// already done in `global_crdt` (by the sender's sequence), so events arrive here in order.
#[derive(Component, Default)]
struct PendingSceneAnim {
    /// The animation awaiting its movement trigger (`None` = clear). Fired in sync with the position
    /// packet whose motion it belongs to: the next packet arms it ([`fire_on_next_movement`]) and the
    /// one after fires it, delaying the clip by ~one packet interval so it lands with the interpolated
    /// (lagged) body rather than ahead of it. As a backstop — a static sender stops sending packets —
    /// it also fires once `update_freq` elapses from receipt. `None` once applied.
    pending: Option<Option<SceneDrivenAnimationRequest>>,
    /// Local time the pending anim was received, for the `update_freq` backstop.
    received_at: f64,
    /// Whether the next movement packet should fire `pending` (vs. merely arm it; see `pending`).
    fire_on_next_movement: bool,
    /// Render-only lean from the latest scene-anim event, composed onto the interpolated yaw each
    /// frame. Tracked continuously (not gated on the apply delay like the clip) since it's a
    /// pose overlay, not a transition. `IDENTITY` = upright.
    tilt: Quat,
}

fn update_foreign_user_target_position(
    mut commands: Commands,
    mut move_events: EventReader<PlayerPositionEvent>,
    mut players: Query<
        (
            Option<&mut PlayerTargetPosition>,
            Option<&mut PendingSceneAnim>,
        ),
        With<ForeignAvatar>,
    >,
) {
    for ev in move_events.read() {
        let dcl_transform = DclTransformAndParent {
            translation: ev.translation,
            rotation: ev.rotation,
            scale: Vec3::ONE,
            parent: SceneEntityId::WORLD_ORIGIN,
        };

        let bevy_trans = dcl_transform.to_bevy_transform();

        if let Ok((maybe_pos, maybe_pending)) = players.get_mut(ev.player) {
            if let Some(mut pos) = maybe_pos {
                // The Pulse server tick is monotonic seconds: accept any strictly-newer stamp — a
                // large forward jump is a legitimate resume (an idle peer whose unchanged state the
                // server stopped republishing, now moving again), not a garbage future stamp. Treat
                // a large backward jump as a server restart (re-sync rather than freeze), and reject
                // small backward steps as reordered/duplicate datagrams.
                // A teleport is an authoritative discontinuity — always valid, bypassing the
                // monotonic-timestamp gate. That gate exists to drop reordered / duplicate deltas;
                // a teleport's server tick may not advance past the delta that landed just before it
                // (e.g. after an out-of-world gap), and dropping it would leave the avatar
                // interpolating across the jump instead of snapping.
                let is_valid = ev.teleport
                    || pos.timestamp.is_none_or(|pts| {
                        let forward = ev.timestamp - pts;
                        let is_forward = forward > 0.0;
                        let is_restart = forward < -TIMESTAMP_RESET_SECS;
                        is_forward || is_restart
                    });
                if is_valid {
                    const LAG_DECAY_SECS: f32 = 1.5;
                    let delta = (ev.time - pos.time) as f32;
                    let update_freq = if ev.teleport {
                        // The gap across a discontinuity does not measure the stream's cadence.
                        pos.update_freq
                    } else {
                        LAG_DECAY_SECS
                            / ((LAG_DECAY_SECS - delta).max(0.0) / pos.update_freq
                                + (LAG_DECAY_SECS / delta).min(1.0))
                    };
                    // Resolve the quantized observation once, here — not per frame in the
                    // interpolator (running the ±box through `v_req = dp/dt` there amplified it into
                    // velocity jerks). Dead-reckon the previous goalpost forward over the inter-packet
                    // interval at the velocity that was in effect across it (the previous packet's),
                    // then constrain that prediction to the ±precision box the new observation
                    // vouches for: while the avatar tracks its velocity the prediction stays inside
                    // the box and rides it smoothly (no re-anchoring onto the quantization grid — that
                    // re-anchoring was the jitter); only genuine drift reaches a box face, and is
                    // corrected by at most a half-step. Zero precision (a precise transport, or
                    // full-state) collapses the box to a point, so `resolved == observed` and the
                    // prior snap-to-observation behaviour stands.
                    let observed = bevy_trans.translation;
                    let predicted = pos.translation + pos.velocity.unwrap_or(ev.velocity) * delta;
                    let resolved =
                        predicted.clamp(observed - ev.precision, observed + ev.precision);
                    *pos = PlayerTargetPosition {
                        time: ev.time,
                        timestamp: Some(ev.timestamp),
                        velocity: Some(ev.velocity),
                        translation: resolved,
                        rotation: bevy_trans.rotation.normalize_or_identity(),
                        update_freq,
                        grounded: ev.grounded,
                        remote_move_kind: ev.remote_move_kind,
                        teleport: ev.teleport,
                        snap_pending: ev.teleport || pos.snap_pending,
                    };
                    // Movement trigger: the first packet after an anim arrives arms it; the next one
                    // fires it, in sync with the motion (see `PendingSceneAnim`).
                    if let Some(mut pending) = maybe_pending {
                        if pending.pending.is_some() {
                            if pending.fire_on_next_movement {
                                let anim = pending.pending.take().unwrap();
                                commands
                                    .entity(ev.player)
                                    .try_insert(SceneDrivenAnim { active: anim });
                            } else {
                                pending.fire_on_next_movement = true;
                            }
                        }
                    }
                }
            } else {
                commands.entity(ev.player).try_insert((
                    PlayerTargetPosition {
                        time: ev.time,
                        timestamp: Some(ev.timestamp),
                        velocity: Some(ev.velocity),
                        translation: bevy_trans.translation,
                        rotation: bevy_trans.rotation,
                        update_freq: 0.01,
                        grounded: ev.grounded,
                        remote_move_kind: ev.remote_move_kind,
                        // First placement: snap from the spawn position rather than travel from it.
                        teleport: true,
                        snap_pending: true,
                    },
                    AvatarDynamicState::default(),
                    PendingSceneAnim::default(),
                ));
            }
        }
    }
}

/// Stash each incoming [`PlayerSceneAnimEvent`] as the player's pending animation, to be applied by
/// the movement trigger (see [`PendingSceneAnim`]). Events arrive in order; if a newer one lands
/// before the previous has fired, the previous is applied first (apply-before-overwrite) so it
/// isn't silently dropped.
fn update_foreign_scene_anim(
    mut commands: Commands,
    mut anim_events: EventReader<PlayerSceneAnimEvent>,
    mut players: Query<&mut PendingSceneAnim>,
    time: Res<Time>,
) {
    let now = time.elapsed_secs_f64();
    for ev in anim_events.read() {
        let tilt = avatar_tilt_quat(ev.tilt.0, ev.tilt.1);
        // The next movement packet arms the clip and the one after fires it (see `PendingSceneAnim`),
        // delaying the anim by ~one packet interval so it lands in sync with the interpolated body.
        if let Ok(mut slot) = players.get_mut(ev.player) {
            if let Some(anim) = slot.pending.take() {
                commands
                    .entity(ev.player)
                    .try_insert(SceneDrivenAnim { active: anim });
            }
            slot.pending = Some(ev.anim.clone());
            slot.received_at = now;
            slot.fire_on_next_movement = false;
            slot.tilt = tilt;
        } else {
            commands.entity(ev.player).try_insert(PendingSceneAnim {
                pending: Some(ev.anim.clone()),
                received_at: now,
                fire_on_next_movement: false,
                tilt,
            });
        }
    }
}

/// Apply the remote-derived move kind to a foreign avatar's dynamic state. This runs every frame
/// on the last packet's target, so it must be idempotent.
///
/// `jump_time` marks the take-off of the jump in flight (`<= 0.0` for none): set once on the
/// rising edge of a Jump and kept while the avatar stays airborne, so the animation starts the
/// take-off clip once and the falling loop follows. A grounded packet that does not announce a
/// jump ends the flight and clears it. DoubleJump / Glide select the matching emote; a Jump, or
/// no state, clears a previously-applied DoubleJump / Glide so the picker reclaims.
fn apply_remote_move_kind(
    dynamic_state: &mut AvatarDynamicState,
    kind: Option<MoveKind>,
    grounded: Option<bool>,
    now: f64,
) {
    match kind {
        Some(MoveKind::Jump) => {
            if dynamic_state.jump_time <= 0.0 {
                dynamic_state.jump_time = now;
            }
        }
        Some(k) => dynamic_state.move_kind = k,
        None => {}
    }
    if matches!(kind, None | Some(MoveKind::Jump))
        && matches!(
            dynamic_state.move_kind,
            MoveKind::DoubleJump | MoveKind::Glide
        )
    {
        dynamic_state.move_kind = MoveKind::Idle;
    }
    if kind != Some(MoveKind::Jump) && grounded == Some(true) {
        dynamic_state.jump_time = -1.0;
    }
}

fn update_foreign_user_actual_position(
    mut commands: Commands,
    mut avatars: Query<(
        Entity,
        &mut PlayerTargetPosition,
        &mut Transform,
        &mut AvatarDynamicState,
        Option<&mut PendingSceneAnim>,
    )>,
    mut scene_datas: Query<(&mut SceneColliderData, &GlobalTransform)>,
    containing_scene: ContainingScene,
    time: Res<Time>,
) {
    for (foreign_ent, mut target, mut actual, mut dynamic_state, maybe_pending) in
        avatars.iter_mut()
    {
        debug!(
            "positioning foreign {foreign_ent:?}, target {}, current {}",
            target.translation, actual.translation
        );

        // A discontinuity places the avatar at the fresh pose for this frame.
        let turn_time;
        if target.snap_pending || (actual.translation - target.translation).length() > 125.0 {
            target.snap_pending = false;
            actual.translation = target.translation;
            dynamic_state.velocity = target.velocity.unwrap_or_default();
            turn_time = 0.0;
        } else if let Some(velocity) = target.velocity {
            let t0 = time.elapsed_secs_f64();
            let t1 = target.time + target.update_freq as f64;

            if t1 < t0 + time.delta_secs_f64() * 2.0 {
                // Past the goalpost: dead-reckon forward from the last observation. `stale` is
                // unbounded while no packet arrives, so the offset is saturated rather than linear
                // (see `COAST_SECS`) — a single missed update used to walk the peer away at full
                // speed indefinitely. `decay` is the derivative of that offset, so the velocity we
                // report stays consistent with the motion we render and the walk/run blend winds
                // down with it instead of animating a sprint on a stationary avatar.
                let stale = (t0 - t1) as f32;
                let decay = (-stale / COAST_SECS).exp();
                actual.translation = target.translation + velocity * (COAST_SECS * (1.0 - decay));
                dynamic_state.velocity = velocity * decay;
                turn_time = 0.0;
            } else {
                // use some extrapolation but slow it down so we don't overcompensate for missed packets
                let dt = (t1 - t0) as f32;
                let dt = if dt < 1.0 { dt } else { dt.sqrt() };

                let p0 = actual.translation;
                let p1 = target.translation;
                let dp = p1 - p0;

                let v_req = dp / dt;
                let v0 = dynamic_state.velocity;
                let v1 = velocity;

                let speed_without_middle = (v0 + v1) * 0.25;
                let req_middle = (v_req - speed_without_middle) * 2.0;
                dynamic_state.velocity +=
                    (req_middle - v0) * (time.delta_secs() / (dt * 0.5)).min(1.0);
                turn_time = dt.max(0.0);
                actual.translation += dynamic_state.velocity * time.delta_secs();
            }
        } else {
            // arrive at target position by time + 0.5
            let walk_time_left = (target.time + 0.5 - time.elapsed_secs_f64()) as f32;
            if walk_time_left <= 0.0 {
                actual.translation = target.translation;
                dynamic_state.velocity = Vec3::ZERO;
            } else {
                let walk_fraction = (time.delta_secs() / walk_time_left).min(1.0);
                let delta = (target.translation - actual.translation) * walk_fraction;
                dynamic_state.velocity = delta / time.delta_secs();
                actual.translation += dynamic_state.velocity * time.delta_secs();
            }
            turn_time = (target.time + 0.2 - time.elapsed_secs_f64()) as f32;
        }

        // Compose the render-only lean onto the (yaw-only) target before interpolating, so the
        // existing rotation lerp carries the tilt as part of the rotation — tilt changes blend
        // smoothly and the lean never decays out of the yaw interpolation. `target.rotation`
        // itself stays yaw-only (that's the value scenes read via the CRDT transform).
        let tilt = maybe_pending
            .as_deref()
            .map(|p| p.tilt)
            .unwrap_or(Quat::IDENTITY);
        let target_rotation = target.rotation * tilt;
        if turn_time <= 0.0 {
            actual.rotation = target_rotation;
        } else {
            let turn_fraction = (time.delta_secs() / turn_time).min(1.0);
            actual.rotation = actual.rotation.lerp(target_rotation, turn_fraction);
        }

        apply_remote_move_kind(
            &mut dynamic_state,
            target.remote_move_kind,
            target.grounded,
            time.elapsed_secs_f64(),
        );

        if let Some(grounded) = target.grounded {
            dynamic_state.ground_height = if grounded { 0.0 } else { 1.0 };
        } else if common::structs::server_mode() {
            // a server trusts the broadcast position verbatim: no visual ground-snap or
            // fall. Scene colliders must not be consulted here — co-tenant scenes
            // physically overlap, so the lookup below would read other rooms' geometry
            // (and pay a raycast per player per scene per frame).
            dynamic_state.ground_height = 0.0;
        } else {
            // update ground height
            dynamic_state.ground_height = actual.translation.y;
            // get containing scene
            containing_scene
                .get(foreign_ent)
                .into_iter()
                .for_each(|scene| {
                    if let Ok((mut collider_data, _scene_transform)) = scene_datas.get_mut(scene) {
                        if let Some(ground_height) =
                            collider_data.get_ground(actual.translation).map(|(h, _)| h)
                        {
                            dynamic_state.ground_height =
                                dynamic_state.ground_height.min(ground_height);
                        }
                    }
                });

            // fall
            if actual.translation.y > target.translation.y && dynamic_state.ground_height > 0.0 {
                let updated_y = target
                    .translation
                    .y
                    .max(actual.translation.y - 15.0 * time.delta_secs())
                    .max(actual.translation.y - dynamic_state.ground_height);

                dynamic_state.ground_height += updated_y - actual.translation.y;
                actual.translation.y = updated_y;
            }
        }

        // Backstop: the movement trigger fires the pending anim in sync with the motion, but a
        // static sender stops sending packets — so if `update_freq` elapses from receipt with no
        // trigger, apply it anyway rather than strand it.
        if let Some(mut pending) = maybe_pending {
            if pending.pending.is_some()
                && time.elapsed_secs_f64() >= pending.received_at + target.update_freq as f64
            {
                let anim = pending.pending.take().unwrap();
                commands
                    .entity(foreign_ent)
                    .try_insert(SceneDrivenAnim { active: anim });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use bevy::time::TimeUpdateStrategy;
    use scene_runner::initialize_scene::{LiveScenes, PortableScenes, ScenePointers};

    use super::*;

    const STEP: Duration = Duration::from_millis(100);

    /// The interpolation system alone, on a clock that advances `STEP` per update. Targets are
    /// written straight onto the avatar, as the packet receiver does.
    fn app_with_avatar() -> (App, Entity) {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .insert_resource(TimeUpdateStrategy::ManualDuration(STEP))
            .init_resource::<ScenePointers>()
            .init_resource::<LiveScenes>()
            .init_resource::<PortableScenes>()
            .add_systems(Update, update_foreign_user_actual_position);
        // a spawned remote avatar starts from the default state, as `update_foreign_user_target_position` inserts it
        let avatar = app
            .world_mut()
            .spawn((Transform::default(), AvatarDynamicState::default()))
            .id();
        // the clock reads zero until its second update; a jump never happens at exactly zero
        app.update();
        (app, avatar)
    }

    /// One packet's worth of target for the avatar, then one frame.
    fn receive(app: &mut App, avatar: Entity, kind: Option<MoveKind>, grounded: bool) {
        let time = app.world().resource::<Time>().elapsed_secs_f64();
        app.world_mut()
            .entity_mut(avatar)
            .insert(PlayerTargetPosition {
                time,
                timestamp: Some(time),
                velocity: Some(Vec3::ZERO),
                translation: Vec3::ZERO,
                rotation: Quat::IDENTITY,
                update_freq: 0.1,
                grounded: Some(grounded),
                remote_move_kind: kind,
                teleport: false,
                snap_pending: false,
            });
        app.update();
    }

    fn state(app: &App, avatar: Entity) -> (f64, MoveKind) {
        let d = app.world().get::<AvatarDynamicState>(avatar).unwrap();
        (d.jump_time, d.move_kind)
    }

    #[test]
    fn reconnect_snapshot_snaps_even_with_a_delta_in_the_same_frame() {
        use dcl_component::transform_and_parent::{DclQuat, DclTranslation};

        let (mut app, avatar) = app_with_avatar();
        app.add_event::<PlayerPositionEvent>().add_systems(
            Update,
            update_foreign_user_target_position.before(update_foreign_user_actual_position),
        );
        app.world_mut().entity_mut(avatar).insert(ForeignAvatar);
        receive(&mut app, avatar, None, true);

        // The old target survived because the player remained in a LiveKit room.
        app.world_mut()
            .resource_mut::<Time<Virtual>>()
            .set_max_delta(Duration::from_secs(300));
        app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
            144_500,
        )));
        app.update();
        app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(1)));
        let now = app.world().resource::<Time>().elapsed_secs_f64();
        for (timestamp, teleport) in [(144.5, true), (144.6, false)] {
            app.world_mut().send_event(PlayerPositionEvent {
                player: avatar,
                time: now,
                timestamp,
                translation: DclTranslation([6.0, 0.0, 0.0]),
                rotation: DclQuat::default(),
                velocity: Vec3::X,
                grounded: Some(true),
                remote_move_kind: None,
                teleport,
                precision: Vec3::ZERO,
            });
        }
        app.update();

        let target = app.world().get::<PlayerTargetPosition>(avatar).unwrap();
        assert_eq!(target.timestamp, Some(144.6));
        assert!(
            (0.09..0.11).contains(&target.update_freq),
            "expected the pre-reconnect cadence, got {}",
            target.update_freq
        );
        let rendered = app.world().get::<Transform>(avatar).unwrap();
        assert_eq!(rendered.translation, Vec3::new(6.0, 0.0, 0.0));
    }

    #[test]
    fn a_remote_jump_sets_the_take_off_time_once_and_keeps_it_while_airborne() {
        let (mut app, avatar) = app_with_avatar();
        receive(&mut app, avatar, None, true);
        assert!(state(&app, avatar).0 < 0.0);

        receive(&mut app, avatar, Some(MoveKind::Jump), false);
        let (take_off, _) = state(&app, avatar);
        assert!(take_off > 0.0);

        // later frames of the same jump, with and without new packets
        for _ in 0..5 {
            app.update();
            receive(&mut app, avatar, Some(MoveKind::Jump), false);
            assert_eq!(state(&app, avatar).0, take_off);
        }
    }

    #[test]
    fn landing_clears_the_take_off_time_and_a_second_jump_sets_it_again() {
        let (mut app, avatar) = app_with_avatar();
        receive(&mut app, avatar, Some(MoveKind::Jump), false);
        let (first, _) = state(&app, avatar);
        assert!(first > 0.0);

        receive(&mut app, avatar, None, true);
        assert!(state(&app, avatar).0 < 0.0);

        receive(&mut app, avatar, Some(MoveKind::Jump), false);
        let (second, _) = state(&app, avatar);
        assert!(second > first);
    }

    #[test]
    fn a_double_jump_and_a_glide_keep_selecting_their_clips_until_the_jump_ends() {
        let (mut app, avatar) = app_with_avatar();
        receive(&mut app, avatar, Some(MoveKind::Jump), false);
        receive(&mut app, avatar, Some(MoveKind::DoubleJump), false);
        assert_eq!(state(&app, avatar).1, MoveKind::DoubleJump);
        receive(&mut app, avatar, Some(MoveKind::Glide), false);
        assert_eq!(state(&app, avatar).1, MoveKind::Glide);

        receive(&mut app, avatar, None, true);
        assert_eq!(state(&app, avatar).1, MoveKind::Idle);
        assert!(state(&app, avatar).0 < 0.0);
    }

    #[test]
    fn a_jump_after_a_double_jump_does_not_replay_the_double_jump_clip() {
        // the landing packet was lost: the next jump follows the double jump directly
        let (mut app, avatar) = app_with_avatar();
        receive(&mut app, avatar, Some(MoveKind::DoubleJump), false);
        receive(&mut app, avatar, Some(MoveKind::Jump), false);
        assert_ne!(state(&app, avatar).1, MoveKind::DoubleJump);
    }

    #[test]
    fn counts_split_room_clips_from_the_fallback() {
        let counts = AnimationSourceCounts::count([
            Some(ActiveEmoteSource::SceneMovementAnim),
            Some(ActiveEmoteSource::VelocitySelected),
            Some(ActiveEmoteSource::VelocitySelected),
            Some(ActiveEmoteSource::TriggeredEmote),
            None,
        ]);
        assert_eq!(
            counts,
            AnimationSourceCounts {
                room_clip: 1,
                fallback: 2,
                other: 2,
            }
        );
    }

    #[test]
    fn no_remote_players_counts_nothing() {
        assert_eq!(
            AnimationSourceCounts::count([]),
            AnimationSourceCounts::default()
        );
    }
}
