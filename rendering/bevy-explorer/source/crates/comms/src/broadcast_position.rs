use bevy::prelude::*;

use common::{
    dynamics::PLAYER_LANDED_THRESHOLD,
    movement_clip::ClipMotion,
    structs::{
        AvatarDynamicState, HeadSync, LocomotionSpeeds, MoveKind, PointAtSync, PrimaryUser,
        SceneDrivenAnim,
    },
};
use dcl_component::{
    proto_components::kernel::comms::rfc4,
    transform_and_parent::{DclQuat, DclTranslation},
};
use wallet::Wallet;

use crate::{
    anim_announce::{Announcement, ClipAnnouncer},
    broadcast, broadcast_to,
    movement_animation::{wire_animation, JumpCount},
    pulse::movement_pacing::{Change, MovementPacer},
    BroadcastTarget,
};

use super::Transport;

pub struct BroadcastPositionPlugin;

impl Plugin for BroadcastPositionPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, broadcast_position);
    }
}

// Re-send the anim hash pair at least this often so late joiners pick up the active clip.
const ANIM_URN_KEEPALIVE: f64 = 1.0;
// A head-yaw or head-pitch change beyond this threshold (in degrees) breaks the idle
// keepalive and gets the next packet sent at the next regular slot (see `MovementPacer`).
// Three degrees exceeds one 7-bit Pulse angle step (360 / 127 degrees). A smaller
// turn can still cross a wire step. It travels with the next movement or keepalive packet.
const HEAD_SYNC_EPSILON_DEG: f32 = 3.0;

#[derive(Default)]
struct LastAnim {
    // Decides which sends carry the hash pair or the clear (change, repeats, keepalive). It also
    // remembers the clip as of the previous send, so clip transitions are detected through it.
    announcer: ClipAnnouncer,
    // Latched seek from the scene. The scene publishes `seek` for a single frame;
    // we hold it here until a broadcast goes out so we don't miss it between the
    // regular (`REGULAR_SPACING_SECONDS`) and idle keepalive broadcast intervals.
    pending_seek: Option<f32>,
    // Latched sound content hashes from the scene. Accumulated across frames so
    // single-frame sound triggers still make it out on the next broadcast; drained
    // on send.
    pending_sounds: Vec<String>,
    // Previous frame's sound list, used to avoid re-latching the same list multiple
    // times when the scene holds it across frames between broadcasts.
    last_seen_sounds: Vec<String>,
    // Monotonic counter stamped onto each outbound SDA so the (unreliable) receiver can
    // order datagrams and drop reordered/duplicate ones. Carries no timing.
    sequence: u32,
    jump_count: JumpCount,
}

fn broadcast_position(
    player: Query<
        (
            &GlobalTransform,
            &AvatarDynamicState,
            Option<&SceneDrivenAnim>,
            Option<&HeadSync>,
            Option<&PointAtSync>,
            Option<&LocomotionSpeeds>,
        ),
        With<PrimaryUser>,
    >,
    transports: Query<&Transport>,
    mut last_position: Local<(Vec3, Quat, Vec3)>,
    mut last_head_sync: Local<HeadSync>,
    mut last_point_at: Local<PointAtSync>,
    mut pacer: Local<MovementPacer>,
    mut last_index: Local<u32>,
    mut last_anim: Local<LastAnim>,
    time: Res<Time>,
    wallet: Res<Wallet>,
) {
    // An authoritative server has no avatar to announce — hammurabi's reportPosition is
    // a no-op. Suppress so clients never see a ghost server player.
    if common::structs::server_mode() {
        return;
    }
    let Ok((player, dynamics, scene_anim, head_sync, point_at, speeds)) = player.single() else {
        return;
    };
    let head_sync = head_sync.copied().unwrap_or_default();
    let point_at = point_at.copied().unwrap_or_default();
    let time = time.elapsed_secs_f64();

    // Latch any single-frame seek from the scene so we still send it even if the
    // broadcast cadence skipped the frame it was published on.
    if let Some(seek) = scene_anim
        .and_then(|s| s.active.as_ref())
        .and_then(|a| a.seek)
    {
        last_anim.pending_seek = Some(seek);
    }

    // Latch new sound triggers from the scene. Scene owns the lifecycle: a new list
    // (including a one-frame write that clears next frame) gets accumulated once per
    // transition. Holding the same list across frames doesn't re-latch.
    let current_sounds: &[String] = scene_anim
        .and_then(|s| s.active.as_ref())
        .map(|a| a.sounds.as_slice())
        .unwrap_or(&[]);
    if current_sounds != last_anim.last_seen_sounds.as_slice() {
        last_anim
            .pending_sounds
            .extend(current_sounds.iter().cloned());
        last_anim.last_seen_sounds = current_sounds.to_vec();
    }

    // the jump count updates every frame, before the send pacing, so that a jump clip shorter than
    // the broadcast interval still counts
    let grounded = dynamics.ground_height < PLAYER_LANDED_THRESHOLD;
    let motion = match scene_anim.and_then(|s| s.active.as_ref()) {
        Some(anim) => ClipMotion::of_clip(&anim.src, anim.speed),
        None => ClipMotion::of_move_kind(dynamics.move_kind),
    };
    let jump_count = last_anim
        .jump_count
        .update(grounded, motion, dynamics.jump_time > 0.0);

    let (_, rotation, translation) = player.to_scale_rotation_translation();
    // An anim change (e.g. jump → idle on landing) must go out promptly: when the
    // player comes to rest, velocity/position stop changing, and without this
    // bypass the transition would wait on the idle keepalive (`IDLE_KEEPALIVE_SECONDS`).
    let current_hashes = scene_anim
        .and_then(|s| s.active.as_ref())
        .map(|a| (a.scene_hash.clone(), a.content_hash.clone()));
    let anim_changed = last_anim.announcer.changed(current_hashes.as_ref());
    // Toggling either enabled flag, or a yaw/pitch delta past the threshold, breaks the
    // keepalive and goes out at the next regular slot. Sub-threshold drift just rides the
    // 1Hz keepalive.
    let head_changed = head_sync.yaw_enabled != last_head_sync.yaw_enabled
        || head_sync.pitch_enabled != last_head_sync.pitch_enabled
        || (head_sync.yaw_deg - last_head_sync.yaw_deg).abs() > HEAD_SYNC_EPSILON_DEG
        || (head_sync.pitch_deg - last_head_sync.pitch_deg).abs() > HEAD_SYNC_EPSILON_DEG;
    // Point-at toggling or any non-trivial coord change breaks the keepalive.
    // No degree-style threshold: target coords can move continuously when the
    // user drags, and the receiver visually integrates whatever it gets.
    let point_at_changed = point_at.is_pointing != last_point_at.is_pointing
        || (point_at.is_pointing
            && (point_at.target_world - last_point_at.target_world).length_squared() > 0.01);
    // Pending seeks and the repeats of a clip change are ordinary movement, not edges: they ride
    // the next regular slot. The latch above already holds the most recent seek.
    let velocity_change = (dynamics.velocity - last_position.2).length();
    let moved = (translation - last_position.0).length_squared() >= 0.01
        || rotation != last_position.1
        || velocity_change >= 0.1
        || last_anim.pending_seek.is_some()
        || last_anim.announcer.repeat_pending()
        || head_changed
        || point_at_changed;
    // A new clip, or a velocity change no ordinary acceleration makes (a jump impulse, a landing,
    // an abrupt start, stop or turn), goes out on the frame it happens; the rest waits for its
    // slot. The flags a packet carries are computed for the frame that is sent, so they travel
    // with the edge.
    let change = Change::since_last_send(anim_changed, velocity_change, moved);
    if !pacer.should_send(time, change) {
        return;
    }

    let dcl_position = DclTranslation::from_bevy_translation(translation);

    // Scene-driven animation carrier. The hash pair is sent on transition (new clip, or
    // an empty `scene_hash` to clear a prior active anim), repeated on the next sends so one
    // lost datagram doesn't leave a stale clip, and re-sent every ANIM_URN_KEEPALIVE seconds
    // while active so late joiners pick it up (see `ClipAnnouncer`). The other
    // fields ride along whenever an animation is active. `playback_time` is only sent
    // when the scene explicitly requested a seek this frame. The nested message itself
    // is only attached to the packet when there's something anim-related to say —
    // that way a receiver that's not interested (or that never sees us transition out
    // of an inactive state) costs no bytes.
    let active_anim = scene_anim.and_then(|s| s.active.as_ref());
    let (scene_hash, content_hash) =
        match last_anim
            .announcer
            .announce(time, current_hashes.as_ref(), ANIM_URN_KEEPALIVE)
        {
            Announcement::Clip((scene, content)) => (Some(scene.clone()), Some(content.clone())),
            // Transition active → none: signal clear with an empty scene_hash.
            Announcement::Clear => (Some(String::new()), None),
            Announcement::Nothing => (None, None),
        };

    let speed = active_anim.map(|a| a.speed);
    let transition_seconds = active_anim.map(|a| a.transition_seconds);
    let r#loop = active_anim.map(|a| a.r#loop);
    let idle = active_anim.map(|a| a.idle);
    // Render-only lean, carried so remotes can bank a tilted avatar. Rides along with an
    // active anim, like speed/loop; receivers compose it on top of the yaw in rotation_y.
    // Derived from the same composed rotation we extract yaw from above — `apply_rotation`
    // baked the lean in as `from_euler(YXZ, yaw, pitch, roll)`, so `.1`/`.2` recover it.
    let (tilt_pitch, tilt_roll) = match active_anim {
        Some(_) => {
            let (_, pitch, roll) = rotation.to_euler(bevy::math::EulerRot::YXZ);
            (Some(pitch.to_degrees()), Some(roll.to_degrees()))
        }
        None => (None, None),
    };
    // The latch above already mirrors the freshest `seek` seen since the last
    // broadcast. Only send if there's an active anim to apply it to.
    let playback_time = active_anim.and(last_anim.pending_seek.take());
    // Drain pending sounds on every broadcast. Sounds depend on `scene_hash` to
    // identify their host scene, so only ship them while an anim is active.
    let sound_content_hashes = if active_anim.is_some() {
        std::mem::take(&mut last_anim.pending_sounds)
    } else {
        last_anim.pending_sounds.clear();
        Vec::new()
    };

    // Attach the nested message only when there's anim state to communicate — that is,
    // when we have an active animation (ride-along, keepalive, or transition-in) or
    // when we're transitioning out (scene_hash == Some("")). Otherwise leave it None
    // so the field isn't serialized at all. The monotonic `sequence` is for ordering only;
    // the receiver derives apply timing from our movement stream's server tick instead.
    let scene_driven_animation = if active_anim.is_some() || scene_hash.is_some() {
        last_anim.sequence = last_anim.sequence.wrapping_add(1);
        Some(
            dcl_component::proto_components::kernel::comms::rfc4::SceneDrivenAnimation {
                scene_hash,
                content_hash,
                speed,
                playback_time,
                transition_seconds,
                r#loop,
                sound_content_hashes,
                origin_address: wallet.address().map(|a| format!("{a:#x}")),
                idle,
                tilt_pitch,
                tilt_roll,
                sequence: Some(last_anim.sequence),
            },
        )
    } else {
        None
    };

    let animation = wire_animation(
        dynamics.velocity,
        &speeds.copied().unwrap_or_default(),
        grounded,
        motion,
        jump_count,
    );

    let movement_uncompressed = dcl_component::proto_components::kernel::comms::rfc4::Movement {
        timestamp: time as f32,
        position_x: dcl_position.0[0],
        position_y: dcl_position.0[1],
        position_z: dcl_position.0[2],
        // Yaw in [0, 360). Pulse quantizes rotation_y over [0, 360] on the delta tier, so a negative
        // angle clamps to 0 there and freezes remote rotation. The receiver maps it back with
        // `from_rotation_y(-rotation_y)`.
        rotation_y: (-rotation.to_euler(bevy::math::EulerRot::YXZ).0.to_degrees())
            .rem_euclid(360.0),
        // Raw velocity, which Pulse quantizes server-side.
        // z is negated to match the receiver's `-velocity_z` DCL-space flip.
        velocity_x: dynamics.velocity.x,
        velocity_y: dynamics.velocity.y,
        velocity_z: -dynamics.velocity.z,
        slide_blend_value: 0.0,
        is_long_jump: false,
        is_emoting: dynamics.move_kind == MoveKind::Emote,
        // Interpolation hint other clients read; this sender never teleports through it.
        is_instant: false,
        head_ik_yaw_enabled: head_sync.yaw_enabled,
        head_ik_pitch_enabled: head_sync.pitch_enabled,
        head_yaw: head_sync.yaw_deg,
        head_pitch: head_sync.pitch_deg,
        point_at_x: point_at.target_world.x,
        point_at_y: point_at.target_world.y,
        point_at_z: point_at.target_world.z,
        is_pointing_at: point_at.is_pointing,
        // Scene-driven animation no longer rides the movement — it goes out as its own packet below.
        scene_driven_animation: None,
        // Receive-side only (the Pulse decoder stamps it); never set on the send path, so it stays
        // out of the serialized packet.
        position_precision: None,
        ..animation
    };

    debug!("sending movement: {movement_uncompressed:?}");

    // Movement rides each rfc4 packet to exactly its consumer: `Movement` to Pulse alone (its routing
    // transport bridges it into a `PlayerStateInput`), and `Position` to Archipelago, which clusters
    // islands from it. Every realm is a Pulse realm, so no byte transport carries avatar state — a
    // peer that can't be reached over Pulse can't be reached at all.
    let dcl_rotation = DclQuat::from_bevy_quat(rotation);
    let position_packet = rfc4::Packet {
        message: Some(rfc4::packet::Message::Position(rfc4::Position {
            index: *last_index,
            position_x: dcl_position.0[0],
            position_y: dcl_position.0[1],
            position_z: dcl_position.0[2],
            rotation_x: dcl_rotation.0[0],
            rotation_y: dcl_rotation.0[1],
            rotation_z: dcl_rotation.0[2],
            rotation_w: dcl_rotation.0[3],
        })),
        protocol_version: 100,
    };
    broadcast(
        transports.iter(),
        BroadcastTarget::PULSE,
        true,
        movement_uncompressed,
    );
    broadcast_to(
        transports.iter(),
        BroadcastTarget::ARCHIPELAGO,
        true,
        &position_packet,
    );
    *last_index = last_index.wrapping_add(1);

    // Scene-driven animation rides its own packet (unreliable, like the old movement) to the avatar
    // renderers — LiveKit and the websocket dev server — only when there's anim state to send. Not
    // Archipelago (island assignment, no rendering) nor Pulse (no animation conversion). Carries a
    // monotonic sequence (for ordering); the receiver aligns it to our Pulse positions using its own
    // last-received movement tick from us.
    if let Some(anim) = scene_driven_animation {
        let packet = rfc4::Packet {
            message: Some(rfc4::packet::Message::SceneDrivenAnimation(anim)),
            protocol_version: 100,
        };
        broadcast_to(
            transports.iter(),
            BroadcastTarget::WEBSOCKET | BroadcastTarget::LIVEKIT,
            true,
            &packet,
        );
    }

    *last_position = (translation, rotation, dynamics.velocity);
    *last_head_sync = head_sync;
    *last_point_at = point_at;
    pacer.record_send(time);
}
