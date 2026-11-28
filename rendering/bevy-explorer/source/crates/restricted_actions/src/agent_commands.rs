use bevy::prelude::*;
use bevy_console::ConsoleCommand;
use common::{
    dynamics,
    rpc::{RpcCall, RpcCallEvent, RpcResultReceiver, RpcResultSender},
    structs::{AvatarDynamicState, PrimaryUser},
};
use console::{reply, DoAddConsoleCommand, PendingConsoleResponses};
use dcl_component::transform_and_parent::DclTranslation;
use scene_runner::{
    initialize_scene::{LiveScenes, PortableKind, PortableScenes},
    renderer_context::RendererSceneContext,
    util::settle,
    vec3_to_parcel,
};
use serde_json::json;

pub struct AgentCommandsPlugin;

impl Plugin for AgentCommandsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MoveWaits>()
            .add_systems(Update, settle_moves)
            .add_preview_console_command::<MovePlayerToCommand, _>(move_player_to_cmd);
        app.add_console_command::<WalkPlayerToCommand, _>(walk_player_to_cmd);
        app.add_console_command::<PlayerPositionCommand, _>(player_position_cmd);
        app.add_console_command::<ListPortablesCommand, _>(list_portables_cmd);
        app.add_console_command::<ConnectedPlayersCommand, _>(connected_players_cmd);
        app.add_console_command::<GetUserDataCommand, _>(get_user_data_cmd);
    }
}

// --- /move_player_to ---

/// How long `--wait` waits after the move's duration when `--timeout-seconds` is not given.
const DEFAULT_WAIT_SECONDS: f32 = 10.0;
/// The speed, in metres a second, under which the player counts as still.
const STILL_SPEED: f32 = 0.3;
/// How long the player must be landed and still to have settled, in seconds: a span of time, not
/// of frames, so it means the same at any frame rate.
const STILL_SECONDS: f32 = 0.25;

/// Move the player to a DCL world-space position, with optional linear interpolation, as a scene's
/// movePlayerTo does. Coordinates are in Decentraland world space (x right, y up, z forward).
/// --camera-target aims the camera at a point and --avatar-target turns the avatar to face one
/// (the camera target without it). --wait replies, as JSON, once the player is landed and still:
/// position,
/// parcel, settled, settleMs, and a warning when --timeout-seconds ran out first
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/move_player_to")]
struct MovePlayerToCommand {
    #[arg(allow_hyphen_values(true))]
    x: f32,
    #[arg(allow_hyphen_values(true))]
    y: f32,
    #[arg(allow_hyphen_values(true))]
    z: f32,
    /// Duration in seconds for linear interpolation. Omit for instant teleport.
    #[arg(allow_hyphen_values(true))]
    duration: Option<f32>,
    /// A world point to aim the camera at, as movePlayerTo's cameraTarget.
    #[arg(long, num_args = 3, value_names = ["X", "Y", "Z"], allow_negative_numbers = true)]
    camera_target: Option<Vec<f32>>,
    /// A world point for the avatar to face, as movePlayerTo's avatarTarget.
    #[arg(long, num_args = 3, value_names = ["X", "Y", "Z"], allow_negative_numbers = true)]
    avatar_target: Option<Vec<f32>>,
    /// Reply once the player has moved, landed and been still for a moment.
    #[arg(long)]
    wait: bool,
    /// How long --wait waits, from the command, in seconds: 10 after the duration by default, at
    /// most 300.
    #[arg(long, requires = "wait")]
    timeout_seconds: Option<f32>,
}

fn move_player_to_cmd(
    mut input: ConsoleCommand<MovePlayerToCommand>,
    mut events: EventWriter<RpcCallEvent>,
    mut pending: ResMut<PendingConsoleResponses>,
    mut waits: ResMut<MoveWaits>,
) {
    let Some(Ok(command)) = input.take() else {
        return;
    };
    let point = |v: &Option<Vec<f32>>| v.as_deref().map(|v| [v[0], v[1], v[2]]);
    let camera_target = point(&command.camera_target);
    let avatar_target = point(&command.avatar_target);
    let position = [command.x, command.y, command.z];
    let finite = position
        .into_iter()
        .chain(camera_target.into_iter().flatten())
        .chain(avatar_target.into_iter().flatten())
        .chain(command.duration)
        .all(f32::is_finite);
    if !finite {
        input.reply_failed("the coordinates and the duration must be finite numbers");
        return;
    }
    let default_wait = DEFAULT_WAIT_SECONDS + command.duration.unwrap_or(0.0).max(0.0);
    let timeout = match settle::wait_timeout(command.wait, command.timeout_seconds, default_wait) {
        Ok(timeout) => timeout,
        Err(e) => {
            input.reply_failed(e);
            return;
        }
    };
    let (response, moved) = RpcResultSender::<bool>::channel();
    events.write(RpcCallEvent::engine(RpcCall::move_player(
        position,
        camera_target,
        avatar_target,
        command.duration,
        Some(response),
    )));
    let responder = input.take_responder();
    if let Some(timeout) = timeout {
        let (reply, rx) = RpcResultSender::channel();
        waits.0.push(MoveWait {
            moved: Some(moved),
            timeout,
            elapsed: 0.0,
            last: None,
            still_for: 0.0,
            landed: false,
            reply,
        });
        pending.push_receiver(rx, |reply| reply, responder);
        return;
    }
    let (x, y, z) = (command.x, command.y, command.z);
    let has_duration = command.duration.is_some();
    pending.push_receiver(
        moved,
        move |success| {
            if success {
                Ok(if has_duration {
                    format!("arrived at ({x}, {y}, {z})")
                } else {
                    format!("moved to ({x}, {y}, {z})")
                })
            } else {
                Err("move cancelled".to_string())
            }
        },
        responder,
    );
}

/// A `/move_player_to --wait`: the move, then the player until it is still.
struct MoveWait {
    /// The engine's answer to the move, until it comes.
    moved: Option<RpcResultReceiver<bool>>,
    timeout: f32,
    elapsed: f32,
    /// Where the player was the frame before, once the move was made.
    last: Option<Vec3>,
    /// How long the player has been landed and still, in seconds.
    still_for: f32,
    /// Whether the player stood on the ground in the last frame seen.
    landed: bool,
    reply: RpcResultSender<Result<String, String>>,
}

/// The `/move_player_to --wait` replies still to come, answered by [`settle_moves`].
#[derive(Resource, Default)]
struct MoveWaits(Vec<MoveWait>);

fn settle_moves(
    mut waits: ResMut<MoveWaits>,
    time: Res<Time>,
    player: Query<(&Transform, Option<&AvatarDynamicState>), With<PrimaryUser>>,
) {
    if waits.0.is_empty() {
        return;
    }
    let dt = time.delta_secs();
    // a player with no dynamics does not fall, so it stands where it is
    let player = player.single().ok().map(|(transform, dynamic)| {
        let landed = dynamic.is_none_or(|dynamic| dynamics::landed(dynamic.ground_height));
        (transform.translation, landed)
    });
    waits.0.retain_mut(|wait| wait.advance(dt, player));
}

impl MoveWait {
    /// One frame of the wait, with where the player is and whether it is landed; `false` once it
    /// has replied.
    fn advance(&mut self, dt: f32, player: Option<(Vec3, bool)>) -> bool {
        self.elapsed += dt;
        let expired = self.elapsed >= self.timeout;
        let position = player.map(|(position, _)| position);
        if let Some(moved) = &mut self.moved {
            match moved.poll_once() {
                Ok(Some(true)) => self.moved = None,
                Ok(Some(false)) | Err(()) => {
                    self.reply.send(Err("move cancelled".to_owned()));
                    return false;
                }
                Ok(None) if expired => return self.finish(position, Some("the move to be made")),
                Ok(None) => return true,
            }
        }
        let Some((position, landed)) = player else {
            self.reply.send(Err("the player was removed".to_owned()));
            return false;
        };
        // a move to a point in the air holds the player there until the movement scene takes it
        // back, so a player off the ground has not settled however still it is; still is a speed,
        // not a distance a frame, and a frame that took no time says nothing
        self.landed = landed;
        if let Some(last) = self.last.filter(|_| dt > 0.0) {
            let still = landed && last.distance(position) / dt < STILL_SPEED;
            self.still_for = if still { self.still_for + dt } else { 0.0 };
        }
        self.last = Some(position);
        if self.still_for >= STILL_SECONDS {
            self.finish(Some(position), None)
        } else if expired {
            let waiting = if self.landed {
                "the player to be still"
            } else {
                "the player to land"
            };
            self.finish(Some(position), Some(waiting))
        } else {
            true
        }
    }

    /// Replies where the player is; `waiting` names what the wait ran out on. Always `false`.
    fn finish(&mut self, position: Option<Vec3>, waiting: Option<&str>) -> bool {
        let mut reply = json!({
            "position": position.map(|p| DclTranslation::from_bevy_translation(p).0.map(reply::round)),
            "parcel": position.map(|p| vec3_to_parcel(p).to_array()),
            "settled": waiting.is_none(),
            "settleMs": (self.elapsed * 1000.0).round(),
        });
        if let Some(waiting) = waiting {
            reply["warning"] = json!(format!(
                "timed out after {} s waiting for {waiting}",
                self.timeout
            ));
        }
        self.reply.send(Ok(reply.to_string()));
        false
    }
}

// --- /walk_player_to ---

/// Walk the player to a DCL world-space position using the movement scene controller.
/// Coordinates are in Decentraland world space (x right, y up, z forward).
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/walk_player_to")]
struct WalkPlayerToCommand {
    #[arg(allow_hyphen_values(true))]
    x: f32,
    #[arg(allow_hyphen_values(true))]
    y: f32,
    #[arg(allow_hyphen_values(true))]
    z: f32,
    /// Timeout in seconds before the walk is cancelled. Omit for no timeout.
    #[arg(allow_hyphen_values(true))]
    timeout: Option<f32>,
}

fn walk_player_to_cmd(
    mut input: ConsoleCommand<WalkPlayerToCommand>,
    mut events: EventWriter<RpcCallEvent>,
    mut pending: ResMut<PendingConsoleResponses>,
) {
    if let Some(Ok(command)) = input.take() {
        let to = DclTranslation([command.x, command.y, command.z]).to_bevy_translation();
        let (response, rx) = RpcResultSender::<bool>::channel();
        events.write(RpcCallEvent::engine(RpcCall::WalkPlayer {
            to,
            stop_threshold: 0.5,
            timeout: command.timeout,
            response,
        }));
        let (x, y, z) = (command.x, command.y, command.z);
        let responder = input.take_responder();
        pending.push_receiver(
            rx,
            move |success| {
                if success {
                    Ok(format!("arrived at ({x}, {y}, {z})"))
                } else {
                    Err("walk failed or timed out".to_string())
                }
            },
            responder,
        );
    }
}

// --- /player_position ---

#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/player_position")]
struct PlayerPositionCommand;

fn player_position_cmd(
    mut input: ConsoleCommand<PlayerPositionCommand>,
    player: Query<&Transform, With<PrimaryUser>>,
) {
    if let Some(Ok(_)) = input.take() {
        match player.single() {
            Ok(transform) => {
                let p = transform.translation;
                input.reply_ok(format!("({}, {}, {})", p.x, p.y, -p.z));
            }
            Err(_) => input.reply_failed("player not found"),
        }
    }
}

// --- /list_portables ---

/// Every entry of `PortableScenes`, startup portables included: a debug listing, unlike the
/// scene-facing `getPortableExperiencesLoaded` and the player-facing `/portables`, which hide
/// them.
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/list_portables")]
struct ListPortablesCommand;

fn list_portables_cmd(
    mut input: ConsoleCommand<ListPortablesCommand>,
    portables: Res<PortableScenes>,
    live_scenes: Res<LiveScenes>,
    contexts: Query<&RendererSceneContext>,
) {
    if let Some(Ok(_)) = input.take() {
        let mut entries = portables
            .iter()
            .map(|(hash, source)| {
                let (name, kind) = match &source.kind {
                    PortableKind::SmartWearable { name, .. } => (name.clone(), "smart wearable"),
                    kind => {
                        let title = live_scenes
                            .scenes
                            .get(hash)
                            .and_then(|ent| contexts.get(*ent).ok())
                            .map(|ctx| ctx.title.clone())
                            .unwrap_or_default();
                        let name = source.ens.clone().unwrap_or(title);
                        let kind = match kind {
                            PortableKind::Startup => "startup",
                            _ => "scene",
                        };
                        (name, kind)
                    }
                };
                format!("{name} ({}, {kind})", source.pid)
            })
            .collect::<Vec<_>>();
        if entries.is_empty() {
            input.reply_ok("no portables running");
        } else {
            entries.sort();
            input.reply_ok(entries.join(", "));
        }
    }
}

// --- /connected_players ---

#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/connected_players")]
struct ConnectedPlayersCommand;

fn connected_players_cmd(
    mut input: ConsoleCommand<ConnectedPlayersCommand>,
    mut events: EventWriter<RpcCallEvent>,
    mut pending: ResMut<PendingConsoleResponses>,
) {
    if let Some(Ok(_)) = input.take() {
        let (response, rx) = RpcResultSender::<Vec<String>>::channel();
        // console command has no scene: resolves to the shared context (client: every
        // player; multi-tenant server: none — its shared context holds no players)
        events.write(RpcCallEvent::engine(RpcCall::GetConnectedPlayers {
            response,
        }));
        let responder = input.take_responder();
        pending.push_receiver(
            rx,
            |players| {
                if players.is_empty() {
                    Ok("no other players connected".to_string())
                } else {
                    Ok(players.join(", "))
                }
            },
            responder,
        );
    }
}

// --- /get_user_data ---

#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/get_user_data")]
struct GetUserDataCommand {
    /// Wallet address to look up. Omit to get your own profile.
    address: Option<String>,
}

fn get_user_data_cmd(
    mut input: ConsoleCommand<GetUserDataCommand>,
    mut events: EventWriter<RpcCallEvent>,
    mut pending: ResMut<PendingConsoleResponses>,
) {
    if let Some(Ok(command)) = input.take() {
        let (response, rx) =
            RpcResultSender::<Result<common::profile::SerializedProfile, ()>>::channel();
        events.write(RpcCallEvent::engine(RpcCall::GetUserData {
            user: command.address.clone(),
            response,
        }));
        let label = command.address.unwrap_or_else(|| "self".to_string());
        let responder = input.take_responder();
        pending.push_receiver(
            rx,
            move |result| match result {
                Ok(profile) => Ok(format!(
                    "{} ({}): v{}, web3={}",
                    profile.name,
                    profile.eth_address,
                    profile.version,
                    profile.has_connected_web3.unwrap_or(false),
                )),
                Err(()) => Err(format!("profile not found for {label}")),
            },
            responder,
        );
    }
}

#[cfg(test)]
mod tests {
    use std::{
        sync::{Arc, Mutex},
        time::Duration,
    };

    use bevy_console::{ConsoleCommandEntered, ConsoleResponder};
    use common::sync::MutexExt;
    use console::ConsolePlugin;
    use serde_json::Value;

    use super::*;

    type Answer = Arc<Mutex<Option<Result<String, String>>>>;

    /// The console with `/move_player_to` alone, a player standing at the origin and a clock a
    /// test turns.
    fn app() -> (App, Entity) {
        let mut app = App::new();
        app.add_plugins(ConsolePlugin {
            add_bevy_console: false,
        })
        .add_event::<RpcCallEvent>()
        .init_resource::<MoveWaits>()
        .init_resource::<Time>()
        .add_systems(Update, (move_player_to_cmd, settle_moves).chain());
        let player = app
            .world_mut()
            .spawn((
                PrimaryUser::default(),
                Transform::default(),
                AvatarDynamicState::default(),
            ))
            .id();
        (app, player)
    }

    /// Frames, a sixtieth of a second each, the player must be landed and still to have settled.
    const STILL_FRAMES: u32 = (STILL_SECONDS * 60.0) as u32;

    /// A sixtieth of a second.
    fn frame(app: &mut App) {
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(Duration::from_secs_f64(1.0 / 60.0));
        app.update();
    }

    /// Sends `/move_player_to <line>` and runs the frame that takes it.
    fn send(app: &mut App, line: &str) -> Answer {
        let answer = Answer::default();
        let responder: ConsoleResponder = {
            let answer = answer.clone();
            Arc::new(move |result| *answer.lock_ignore_poison() = Some(result))
        };
        app.world_mut().send_event(ConsoleCommandEntered {
            command_name: "/move_player_to".to_owned(),
            args: line.split_whitespace().map(str::to_owned).collect(),
            responder: Some(responder),
        });
        frame(app);
        answer
    }

    fn reply(answer: &Answer) -> Option<Result<String, String>> {
        answer.lock_ignore_poison().take()
    }

    /// The reply a wait has given, once the console has polled it (a frame later at most).
    fn json_reply(app: &mut App, answer: &Answer) -> Value {
        frame(app);
        serde_json::from_str(&reply(answer).expect("answered").expect("a reply")).unwrap()
    }

    /// The moves asked of the engine since the last call.
    fn moves(app: &mut App) -> Vec<RpcCall> {
        app.world_mut()
            .resource_mut::<Events<RpcCallEvent>>()
            .drain()
            .map(|event| event.call)
            .collect()
    }

    /// Answers the one move asked for.
    fn answer_move(app: &mut App, moved: bool) {
        let mut moves = moves(app);
        assert_eq!(moves.len(), 1);
        let Some(RpcCall::MovePlayer {
            response: Some(response),
            ..
        }) = moves.pop()
        else {
            panic!("a move with a reply");
        };
        response.send(moved);
    }

    #[test]
    fn a_plain_move_asks_the_engine_and_answers_in_a_sentence_once_it_has_moved() {
        let (mut app, _) = app();
        let answer = send(&mut app, "8 0 -8");
        assert!(reply(&answer).is_none(), "it waits for the engine");
        answer_move(&mut app, true);
        frame(&mut app);
        assert_eq!(reply(&answer), Some(Ok("moved to (8, 0, -8)".to_owned())));

        let answer = send(&mut app, "8 0 -8 2");
        answer_move(&mut app, true);
        frame(&mut app);
        assert_eq!(reply(&answer), Some(Ok("arrived at (8, 0, -8)".to_owned())));

        let answer = send(&mut app, "1.5 0 2");
        answer_move(&mut app, false);
        frame(&mut app);
        assert_eq!(reply(&answer), Some(Err("move cancelled".to_owned())));
    }

    #[test]
    fn the_targets_map_as_the_scene_op_maps_them() {
        let (mut app, _) = app();
        send(
            &mut app,
            "8 -1 -8 2.5 --camera-target 8 9 2 --avatar-target 3 0 -4",
        );
        let Some(RpcCall::MovePlayer {
            to,
            looking_at,
            duration,
            camera_rotation,
            ..
        }) = moves(&mut app).pop()
        else {
            panic!("a move");
        };
        // Decentraland world points, so bevy's z is their -z
        assert_eq!(to, Vec3::new(8.0, -1.0, 8.0));
        assert_eq!(looking_at, Some(Vec3::new(3.0, 0.0, 4.0)));
        assert_eq!(duration, Some(2.5));
        // the camera turns along the line from the new position to its target: 10 up, 10 ahead
        let looks = camera_rotation.expect("an aim") * Vec3::NEG_Z;
        let expected = Vec3::new(0.0, 1.0, -1.0).normalize();
        assert!(looks.distance(expected) < 1e-5, "{looks}");
    }

    #[test]
    fn a_waiting_move_answers_once_the_player_is_still() {
        let (mut app, player) = app();
        let answer = send(&mut app, "8 0 8 --wait");
        answer_move(&mut app, true);
        // the move lands where the engine puts it; the player then falls a little
        app.world_mut()
            .get_mut::<Transform>(player)
            .unwrap()
            .translation = Vec3::new(8.0, 1.0, -8.0);
        frame(&mut app);
        for y in [0.8, 0.5, 0.2] {
            app.world_mut()
                .get_mut::<Transform>(player)
                .unwrap()
                .translation
                .y = y;
            frame(&mut app);
            assert!(reply(&answer).is_none(), "still falling");
        }
        for _ in 0..STILL_FRAMES - 1 {
            frame(&mut app);
            assert!(reply(&answer).is_none(), "not still for long enough");
        }
        frame(&mut app);
        frame(&mut app);
        let report = json_reply(&mut app, &answer);
        assert_eq!(report["position"], serde_json::json!([8.0, 0.2, 8.0]));
        assert_eq!(report["parcel"], serde_json::json!([0, 0]));
        assert_eq!(report["settled"], true);
        assert!(report.get("warning").is_none());
    }

    #[test]
    fn a_player_held_in_the_air_has_not_settled_however_still_it_is() {
        let (mut app, player) = app();
        // a move to a point in the air: the engine holds the player there, still, until the
        // movement scene takes over
        let answer = send(&mut app, "100 10 100 --wait --timeout-seconds 1");
        answer_move(&mut app, true);
        let mut entity = app.world_mut().entity_mut(player);
        entity.get_mut::<Transform>().unwrap().translation = Vec3::new(100.0, 10.0, -100.0);
        entity.get_mut::<AvatarDynamicState>().unwrap().ground_height = 10.0;
        for _ in 0..30 {
            frame(&mut app);
            assert!(reply(&answer).is_none(), "held in the air");
        }
        // it falls and lands; the wait then counts from the landing
        let mut entity = app.world_mut().entity_mut(player);
        entity.get_mut::<Transform>().unwrap().translation.y = 0.0;
        entity.get_mut::<AvatarDynamicState>().unwrap().ground_height = 0.0;
        for _ in 0..STILL_FRAMES {
            frame(&mut app);
            assert!(reply(&answer).is_none(), "landed but not still for long enough");
        }
        frame(&mut app);
        frame(&mut app);
        let report = json_reply(&mut app, &answer);
        assert_eq!(report["settled"], true);
        assert_eq!(report["position"], serde_json::json!([100.0, 0.0, 100.0]));

        // one that never lands says so
        let answer = send(&mut app, "100 10 100 --wait --timeout-seconds 0.5");
        answer_move(&mut app, true);
        app.world_mut()
            .get_mut::<AvatarDynamicState>(player)
            .unwrap()
            .ground_height = 10.0;
        for _ in 0..40 {
            frame(&mut app);
        }
        let report = json_reply(&mut app, &answer);
        assert_eq!(report["settled"], false);
        assert_eq!(
            report["warning"],
            "timed out after 0.5 s waiting for the player to land"
        );
    }

    #[test]
    fn a_waiting_move_that_runs_out_of_time_says_what_it_waited_for() {
        let (mut app, player) = app();
        // the engine never answers: the move is held here, so it is not dropped either
        let answer = send(&mut app, "8 0 8 --wait --timeout-seconds 0.03");
        let unanswered = moves(&mut app);
        frame(&mut app);
        let report = json_reply(&mut app, &answer);
        assert_eq!(report["settled"], false);
        // where the player still is
        assert_eq!(report["position"], serde_json::json!([0.0, 0.0, 0.0]));
        assert_eq!(
            report["warning"],
            "timed out after 0.03 s waiting for the move to be made"
        );
        drop(unanswered);

        // the player keeps moving
        let answer = send(&mut app, "8 0 8 --wait --timeout-seconds 0.1");
        answer_move(&mut app, true);
        for step in 0..8 {
            app.world_mut()
                .get_mut::<Transform>(player)
                .unwrap()
                .translation
                .x = step as f32;
            frame(&mut app);
        }
        let report = json_reply(&mut app, &answer);
        assert_eq!(report["settled"], false);
        assert_eq!(
            report["warning"],
            "timed out after 0.1 s waiting for the player to be still"
        );

        // a move another one replaced fails as the plain one does
        let answer = send(&mut app, "8 0 8 --wait");
        answer_move(&mut app, false);
        frame(&mut app);
        frame(&mut app);
        assert_eq!(reply(&answer), Some(Err("move cancelled".to_owned())));
    }

    #[test]
    fn the_wait_and_its_timeout_are_checked_before_anything_moves() {
        let (mut app, _) = app();
        for line in [
            "8 0 8 --timeout-seconds 3",
            "8 0 8 --wait --timeout-seconds 0",
            "8 0 8 --wait --timeout-seconds 301",
            "8 0 8 2 --timeout-seconds 3",
            "8 0 8 --camera-target 1 2",
            "8 0 NaN",
        ] {
            let answer = send(&mut app, line);
            assert!(
                matches!(reply(&answer), Some(Err(_))),
                "{line} is an argument error"
            );
            assert!(moves(&mut app).is_empty(), "{line}");
        }
        // a default past the flag's cap is not a value the caller gave
        let answer = send(&mut app, "8 0 8 295 --wait");
        assert!(reply(&answer).is_none(), "it waits");
        assert_eq!(moves(&mut app).len(), 1);
    }
}
