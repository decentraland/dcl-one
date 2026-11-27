//! The console surface and debug-panel section of photo mode.

use bevy::{prelude::*, tasks::Task};
use bevy_console::ConsoleCommand;
use common::{
    rpc::RpcResultSender,
    structs::{CursorLocks, OutOfWorld, PrimaryCamera, PrimaryUser},
    util::{TaskCompat, TaskExt},
};
use console::{
    reply::{self, refusal},
    AddDebugSection, DebugInput, DebugInputKind, DebugSection, DoAddConsoleCommand,
    PendingConsoleResponses,
};
use dcl_component::transform_and_parent::DclTranslation;
use ipfs::IpfsAssetServer;
use serde_json::json;
use wallet::Wallet;

use crate::{
    capture::{
        capture_blocker, phase, EncodingPhoto, PendingPhoto, PhotoStats, TakePhotoRequest,
        UploadingPhoto,
    },
    mode::{photo_mode_block, PhotoCameraPlacer, PhotoModeBlock, PhotoModeRequest},
    photo_camera::{PhotoCamera, PhotoCameraSettings, PhotoModeSource, PhotoPose},
    storage::{delete_photo, CameraReelStorage},
};

/// How long a placed photo camera may take to land when `--timeout-seconds` is not given.
const DEFAULT_LANDING_SECONDS: f32 = 5.0;
const MAX_LANDING_SECONDS: f32 = 300.0;
/// How far the camera may land from `--position` and count as there, in metres.
const LANDED_METRES: f32 = 0.1;

/// Photo deletions asked of the console, by photo id.
#[derive(Resource, Default)]
struct PhotoDeletes(Vec<(String, Task<Result<(), String>>)>);

pub(crate) struct PhotoModeDiagnosticsPlugin;

impl Plugin for PhotoModeDiagnosticsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PhotoDeletes>()
            .init_resource::<PhotoLanding>()
            // it waits for photo mode to be open, so it needs no place among photo mode's systems
            .add_systems(Update, land_photo_pose)
            .add_console_command::<PhotoModeCommand, _>(photo_mode_command)
            .add_console_command::<TakePhotoCommand, _>(take_photo_command)
            .add_console_command::<DeletePhotoCommand, _>(delete_photo_command)
            .add_systems(Update, finish_deletes)
            .add_debug_section(DebugSection {
                title: "Photo mode",
                query: "/photo_mode",
                fields: &[
                    ("Position (X / Y / Z)", "position"),
                    ("Yaw (degrees)", "yaw"),
                    ("Pitch (degrees)", "pitch"),
                    ("Roll (degrees)", "roll"),
                    ("Vertical FOV (degrees)", "fov"),
                    ("Cursor locked", "cursorLocked"),
                    ("Camera HUD hidden", "hudHidden"),
                    ("Opened by", "source"),
                    ("Photos kept (current / max)", "storage"),
                    ("Photo in flight", "phase"),
                    ("Captures this session", "captures"),
                    ("Uploads this session", "uploaded"),
                    ("Failures this session", "failed"),
                    ("Last photo id", "lastPhotoId"),
                    ("Last error", "lastError"),
                ],
                actions: &[("Take photo", "/take_photo")],
                inputs: &[DebugInput {
                    label: "Photo mode",
                    field: "active",
                    command: "/photo_mode {value}",
                    kind: DebugInputKind::Toggle,
                }],
            });
    }
}

/// Read photo mode, or open (`on`/`true`) or close (`off`/`false`) it. `source` names what
/// opened it for analytics: `button` (the HUD), `shortcut` or `console` (the default).
/// --position, --look-at and --fov place the photo camera, opening photo mode when it is closed,
/// and reply once it is there, with placed and settled; --timeout-seconds bounds that
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/photo_mode")]
struct PhotoModeCommand {
    state: Option<String>,
    source: Option<String>,
    /// Where to put the photo camera: a Decentraland world point within reach of the player.
    #[arg(long, num_args = 3, value_names = ["X", "Y", "Z"], allow_negative_numbers = true)]
    position: Option<Vec<f32>>,
    /// A Decentraland world point to aim the photo camera at.
    #[arg(long, num_args = 3, value_names = ["X", "Y", "Z"], allow_negative_numbers = true)]
    look_at: Option<Vec<f32>>,
    /// The photo camera's vertical field of view, in degrees.
    #[arg(long, allow_negative_numbers = true)]
    fov: Option<f32>,
    /// How long the placed camera may take to land, from the command, in seconds: 5 by default,
    /// at most 300.
    #[arg(long)]
    timeout_seconds: Option<f32>,
}

/// A placed photo camera waiting for photo mode to open: [`land_photo_pose`] places it the frame
/// it is open.
struct Landing {
    pose: PhotoPose,
    timeout: f32,
    elapsed: f32,
    /// What the reply's `requested` says: `on` when this command opened photo mode.
    requested: Option<bool>,
    reply: RpcResultSender<Result<String, String>>,
}

/// The pose being placed, one at a time.
#[derive(Resource, Default)]
struct PhotoLanding(Option<Landing>);

/// The pose flags, checked: the pose (its points in bevy's world) and how long it may take to
/// land. `None` when no pose flag is given.
fn pose_of(
    args: &PhotoModeCommand,
    settings: &PhotoCameraSettings,
) -> Result<Option<(PhotoPose, f32)>, String> {
    if args.position.is_none() && args.look_at.is_none() && args.fov.is_none() {
        return match args.timeout_seconds {
            Some(_) => Err(
                "--timeout-seconds bounds the landing of a placed camera; give \
                            --position, --look-at or --fov"
                    .to_owned(),
            ),
            None => Ok(None),
        };
    }
    let timeout = args.timeout_seconds.unwrap_or(DEFAULT_LANDING_SECONDS);
    if !timeout.is_finite() || timeout <= 0.0 || timeout > MAX_LANDING_SECONDS {
        return Err(format!(
            "--timeout-seconds takes more than 0 and at most {MAX_LANDING_SECONDS}; got {timeout}"
        ));
    }
    let point = |v: &Option<Vec<f32>>| v.as_deref().map(|v| [v[0], v[1], v[2]]);
    let (position, look_at) = (point(&args.position), point(&args.look_at));
    if !position
        .into_iter()
        .chain(look_at)
        .flatten()
        .all(f32::is_finite)
    {
        return Err("the coordinates must be finite numbers".to_owned());
    }
    if let Some(fov) = args.fov {
        let (min, max) = (settings.min_fov, settings.max_fov);
        if !fov.is_finite() || !(min..=max).contains(&fov) {
            return Err(format!(
                "--fov takes degrees from {min} to {max}; got {fov}"
            ));
        }
    }
    let bevy = |point: [f32; 3]| DclTranslation(point).to_bevy_translation();
    let pose = PhotoPose {
        position: position.map(bevy),
        look_at: look_at.map(bevy),
        fov: args.fov,
    };
    Ok(Some((pose, timeout)))
}

/// Take a photo in photo mode. `source` as for `/photo_mode`.
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/take_photo")]
struct TakePhotoCommand {
    source: Option<String>,
}

/// Delete one of the player's photos from the camera reel; the outcome lands in `lastError`.
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/delete_photo")]
struct DeletePhotoCommand {
    id: String,
}

fn parse_source(source: Option<&str>) -> Result<PhotoModeSource, String> {
    match source.map(str::to_ascii_lowercase).as_deref() {
        None | Some("console") => Ok(PhotoModeSource::Console),
        Some("button") => Ok(PhotoModeSource::Button),
        Some("shortcut") => Ok(PhotoModeSource::Shortcut),
        Some(other) => Err(format!(
            "expected button, shortcut or console as the source, got {other}"
        )),
    }
}

fn take_photo_command(
    mut cmd: ConsoleCommand<TakePhotoCommand>,
    mut requests: EventWriter<TakePhotoRequest>,
    camera: Query<(
        &Transform,
        &PrimaryCamera,
        Option<&PhotoCamera>,
        Has<PendingPhoto>,
        Has<EncodingPhoto>,
        Has<UploadingPhoto>,
    )>,
    locks: Res<CursorLocks>,
    storage: Res<CameraReelStorage>,
    stats: Res<PhotoStats>,
    time: Res<Time>,
) {
    let Some(Ok(args)) = cmd.take() else {
        return;
    };
    let source = match parse_source(args.source.as_deref()) {
        Ok(source) => source,
        Err(message) => {
            cmd.reply_failed(message);
            return;
        }
    };
    let Ok((transform, primary, photo, pending, encoding, uploading)) = camera.single() else {
        cmd.reply_failed("no primary camera");
        return;
    };
    let busy = pending || encoding || uploading;
    if let Some(reason) = capture_blocker(
        photo.is_some(),
        busy,
        &storage,
        stats.effect_until,
        time.elapsed_secs_f64(),
    ) {
        cmd.reply_failed(format!("photo not taken: {reason}"));
        return;
    }
    requests.write(TakePhotoRequest { source });
    cmd.reply_ok(
        state_json(
            transform,
            primary,
            photo,
            &locks,
            &storage,
            &stats,
            phase(pending, encoding, uploading),
            None,
        )
        .to_string(),
    );
}

#[allow(clippy::too_many_arguments)]
fn photo_mode_command(
    mut cmd: ConsoleCommand<PhotoModeCommand>,
    mut requests: EventWriter<PhotoModeRequest>,
    mut landing: ResMut<PhotoLanding>,
    mut pending: ResMut<PendingConsoleResponses>,
    settings: Res<PhotoCameraSettings>,
    player: Query<(&GlobalTransform, Has<OutOfWorld>), With<PrimaryUser>>,
    camera: Query<(
        &Transform,
        &PrimaryCamera,
        Option<&PhotoCamera>,
        Has<PendingPhoto>,
        Has<EncodingPhoto>,
        Has<UploadingPhoto>,
    )>,
    locks: Res<CursorLocks>,
    storage: Res<CameraReelStorage>,
    stats: Res<PhotoStats>,
) {
    let Some(Ok(args)) = cmd.take() else {
        return;
    };
    let source = match parse_source(args.source.as_deref()) {
        Ok(source) => source,
        Err(message) => {
            cmd.reply_failed(message);
            return;
        }
    };
    let requested = match args
        .state
        .as_deref()
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        None => None,
        Some("on" | "true") => Some(true),
        Some("off" | "false") => Some(false),
        Some(other) => {
            cmd.reply_failed(format!("expected on or off, got {other}"));
            return;
        }
    };
    let pose = match pose_of(&args, &settings) {
        Ok(pose) => pose,
        Err(message) => {
            cmd.reply_failed(message);
            return;
        }
    };
    let Ok((transform, primary, photo, in_flight, encoding, uploading)) = camera.single() else {
        cmd.reply_failed("no primary camera");
        return;
    };
    if let Some((pose, timeout)) = pose {
        let player = player.single().ok();
        let refused = place_refusal(
            &pose,
            requested,
            landing.0.is_some(),
            primary,
            player,
            &settings,
        );
        if let Some(refused) = refused {
            cmd.reply_failed(refused);
            return;
        }
        let opens = photo.is_none();
        if opens {
            requests.write(PhotoModeRequest::Enable(source));
        }
        let (reply, rx) = RpcResultSender::channel();
        landing.0 = Some(Landing {
            pose,
            timeout,
            elapsed: 0.0,
            requested: if opens { Some(true) } else { requested },
            reply,
        });
        pending.push_receiver(rx, |reply| reply, cmd.take_responder());
        return;
    }
    if let Some(enable) = requested {
        requests.write(if enable {
            PhotoModeRequest::Enable(source)
        } else {
            PhotoModeRequest::Disable
        });
    }
    cmd.reply_ok(
        state_json(
            transform,
            primary,
            photo,
            &locks,
            &storage,
            &stats,
            phase(in_flight, encoding, uploading),
            requested,
        )
        .to_string(),
    );
}

/// Why a pose cannot be placed, checked before anything is asked: `off`, a pose still landing,
/// what keeps photo mode closed, or a position out of the camera's reach.
fn place_refusal(
    pose: &PhotoPose,
    requested: Option<bool>,
    landing: bool,
    primary: &PrimaryCamera,
    player: Option<(&GlobalTransform, bool)>,
    settings: &PhotoCameraSettings,
) -> Option<String> {
    if requested == Some(false) {
        return Some(
            "--position, --look-at and --fov place the open photo camera; not with off".to_owned(),
        );
    }
    if landing {
        return Some(refusal(
            "a photo camera pose is still landing; wait for its reply",
            Some(json!({ "holder": "photo_mode --position" })),
        ));
    }
    match photo_mode_block(
        primary,
        player.is_some_and(|(_, out_of_world)| out_of_world),
    ) {
        Some(PhotoModeBlock::SceneCamera) => {
            return Some(refusal(
                "a scene holds the camera; photo mode does not open",
                Some(json!({ "holder": "scene" })),
            ))
        }
        Some(PhotoModeBlock::OutOfWorld) => {
            return Some(refusal(
                "the player is out of the world; photo mode does not open",
                Some(json!({ "player": "outOfWorld" })),
            ))
        }
        None => {}
    }
    let (Some(position), Some((player, _))) = (pose.position, player) else {
        return None;
    };
    let distance = position.distance(player.translation());
    let reach = settings.max_distance_from_player;
    (distance > reach).then(|| {
        refusal(
            format!(
                "the photo camera stays within {reach} m of the player: --position is \
                 {distance:.1} m from it"
            ),
            Some(json!({
                "limit": "reach",
                "maxDistance": reach,
                "distance": reply::round(distance),
            })),
        )
    })
}

/// Places the pose `/photo_mode` asked for the frame photo mode is open, and replies the state
/// with where the camera landed (`placed`, Decentraland world coordinates) and `settled`. One
/// that photo mode does not open for in time, or at all, replies `settled: false` with a
/// `warning`.
#[allow(clippy::too_many_arguments)]
fn land_photo_pose(
    mut landing: ResMut<PhotoLanding>,
    time: Res<Time>,
    mut cameras: ParamSet<(
        PhotoCameraPlacer,
        Query<(
            &Transform,
            &PrimaryCamera,
            Option<&PhotoCamera>,
            Has<PendingPhoto>,
            Has<EncodingPhoto>,
            Has<UploadingPhoto>,
        )>,
    )>,
    player: Query<Has<OutOfWorld>, With<PrimaryUser>>,
    locks: Res<CursorLocks>,
    storage: Res<CameraReelStorage>,
    stats: Res<PhotoStats>,
    settings: Res<PhotoCameraSettings>,
) {
    let Some(current) = landing.0.as_mut() else {
        return;
    };
    current.elapsed += time.delta_secs();
    let placed = cameras.p0().place(&current.pose);
    let mut warning = None;
    if placed.is_none() {
        let out_of_world = player.single().unwrap_or(false);
        let blocked = cameras
            .p1()
            .single()
            .ok()
            .and_then(|(_, primary, ..)| photo_mode_block(primary, out_of_world));
        warning = match blocked {
            Some(PhotoModeBlock::SceneCamera) => {
                Some("a scene took the camera; photo mode did not open".to_owned())
            }
            Some(PhotoModeBlock::OutOfWorld) => {
                Some("the player left the world; photo mode did not open".to_owned())
            }
            None if current.elapsed >= current.timeout => Some(format!(
                "timed out after {} s waiting for photo mode to open",
                current.timeout
            )),
            None => return,
        };
    }
    let Some(done) = landing.0.take() else {
        return;
    };
    let cameras = cameras.p1();
    let Ok((transform, primary, photo, in_flight, encoding, uploading)) = cameras.single() else {
        done.reply.send(Err("no primary camera".to_owned()));
        return;
    };
    let off = match (placed, done.pose.position) {
        (Some(placed), Some(asked)) => placed.translation.distance(asked),
        _ => 0.0,
    };
    if placed.is_some() && off > LANDED_METRES {
        // the player moved off since the command: the camera is held within reach of them
        warning = Some(format!(
            "the camera landed {off:.1} m from --position: it stays within {} m of the player",
            settings.max_distance_from_player
        ));
    }
    let mut state = state_json(
        transform,
        primary,
        photo,
        &locks,
        &storage,
        &stats,
        phase(in_flight, encoding, uploading),
        done.requested,
    );
    state["placed"] = json!(placed.map(|placed| {
        DclTranslation::from_bevy_translation(placed.translation)
            .0
            .map(reply::round)
    }));
    state["settled"] = json!(warning.is_none());
    state["settleMs"] = json!((done.elapsed * 1000.0).round());
    if let Some(warning) = warning {
        state["warning"] = json!(warning);
    }
    done.reply.send(Ok(state.to_string()));
}

fn delete_photo_command(
    mut cmd: ConsoleCommand<DeletePhotoCommand>,
    mut deletes: ResMut<PhotoDeletes>,
    wallet: Res<Wallet>,
    ipfs: IpfsAssetServer,
) {
    let Some(Ok(args)) = cmd.take() else {
        return;
    };
    if wallet.address().is_none() {
        cmd.reply_failed("not signed in");
        return;
    }
    let task = bevy::tasks::IoTaskPool::get().spawn_compat(delete_photo(
        ipfs.ipfs().client(),
        wallet.clone(),
        args.id.clone(),
    ));
    deletes.0.push((args.id.clone(), task));
    cmd.reply_ok(json!({ "deleting": args.id }).to_string());
}

/// A finished deletion frees a storage slot; a failed one is the last error.
fn finish_deletes(
    mut deletes: ResMut<PhotoDeletes>,
    mut storage: ResMut<CameraReelStorage>,
    mut stats: ResMut<PhotoStats>,
) {
    deletes.0.retain_mut(|(id, task)| {
        let Some(result) = task.complete() else {
            return true;
        };
        match result {
            Ok(()) => {
                info!("photo deleted: {id}");
                storage.current = storage.current.saturating_sub(1);
            }
            Err(message) => {
                warn!("photo {id} not deleted: {message}");
                stats.last_error = Some(format!("delete {id}: {message}"));
            }
        }
        false
    });
}

pub(crate) fn state_json(
    transform: &Transform,
    primary: &PrimaryCamera,
    photo: Option<&PhotoCamera>,
    locks: &CursorLocks,
    storage: &CameraReelStorage,
    stats: &PhotoStats,
    phase: &str,
    requested: Option<bool>,
) -> serde_json::Value {
    let mut state = json!({
        "active": photo.is_some(),
        "requested": requested,
        "position": [transform.translation.x, transform.translation.y, transform.translation.z],
        "storage": [storage.current, storage.max],
        "storageLoaded": storage.loaded,
        "phase": phase,
        "captures": stats.captures,
        "uploaded": stats.uploaded,
        "failed": stats.failed,
        "lastPhotoId": stats.last_photo_id,
        "lastError": stats.last_error,
    });
    match photo {
        Some(photo) => {
            state["yaw"] = json!(photo.yaw);
            state["pitch"] = json!(photo.pitch);
            state["roll"] = json!(photo.roll);
            state["fov"] = json!(photo.fov.current);
            state["cursorLocked"] = json!(locks.0.contains("photo"));
            state["hudHidden"] = json!(photo.hud_hidden);
            state["source"] = json!(photo.source);
        }
        None => {
            let reason = if primary.scene_override.is_some() {
                "a scene controls the camera"
            } else {
                "photo mode is off"
            };
            state["unavailable"] = json!({ "": reason });
        }
    }
    state
}

#[cfg(test)]
mod tests {
    use std::{
        sync::{mpsc, Arc},
        time::Duration,
    };

    use bevy_console::ConsoleCommandEntered;
    use common::structs::{CameraOverride, CameraZoomLevel};
    use console::ConsolePlugin;
    use serde_json::Value;

    use super::*;
    use crate::mode::{apply_photo_mode_requests, PhotoModeChanged};

    /// `/photo_mode`, photo mode's opening and the landing, in that order each frame, with a
    /// third-person camera and a player at the origin.
    fn app() -> (App, Entity) {
        let mut app = App::new();
        app.add_plugins(ConsolePlugin {
            add_bevy_console: false,
        })
        .add_event::<PhotoModeRequest>()
        .add_event::<PhotoModeChanged>()
        .init_resource::<PhotoCameraSettings>()
        .init_resource::<PhotoLanding>()
        .init_resource::<CursorLocks>()
        .init_resource::<CameraReelStorage>()
        .init_resource::<PhotoStats>()
        .insert_resource(avatar::dynamic_nametag::NametagsVisible(true))
        .init_resource::<Time>()
        .add_systems(
            Update,
            (
                photo_mode_command,
                apply_photo_mode_requests,
                land_photo_pose,
            )
                .chain(),
        );
        let camera = app
            .world_mut()
            .spawn((
                Transform::from_xyz(0.0, 3.0, 4.0),
                Projection::Perspective(PerspectiveProjection::default()),
                PrimaryCamera::default(),
            ))
            .id();
        app.world_mut()
            .spawn((PrimaryUser::default(), GlobalTransform::IDENTITY));
        (app, camera)
    }

    fn frame(app: &mut App) {
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(Duration::from_secs_f64(1.0 / 60.0));
        app.update();
    }

    /// Sends `/photo_mode <line>` and returns its reply, once given within a few frames.
    fn photo_mode(app: &mut App, line: &str) -> Result<String, String> {
        let (tx, rx) = mpsc::channel();
        app.world_mut().send_event(ConsoleCommandEntered {
            command_name: "/photo_mode".into(),
            args: line.split_whitespace().map(str::to_owned).collect(),
            responder: Some(Arc::new(move |result| drop(tx.send(result)))),
        });
        for _ in 0..3 {
            frame(app);
            if let Ok(reply) = rx.try_recv() {
                return reply;
            }
        }
        panic!("no reply to /photo_mode {line}");
    }

    fn json_of(reply: &str) -> Value {
        serde_json::from_str(reply).unwrap()
    }

    #[test]
    fn a_pose_opens_photo_mode_and_answers_once_the_camera_is_there() {
        let (mut app, camera) = app();
        let state =
            json_of(&photo_mode(&mut app, "--position 8 4 8 --look-at 8 1 16 --fov 40").unwrap());
        assert_eq!(state["active"], true);
        assert_eq!(state["requested"], true, "this command opened it");
        assert_eq!(state["placed"], serde_json::json!([8.0, 4.0, 8.0]));
        assert_eq!(state["settled"], true);
        assert_eq!(state["fov"], 40.0);
        assert!(state.get("warning").is_none());
        let transform = app.world().get::<Transform>(camera).unwrap();
        assert_eq!(transform.translation, Vec3::new(8.0, 4.0, -8.0));

        // a second pose in the open mode lands as well, and the plain read keeps its reply
        let state = json_of(&photo_mode(&mut app, "--fov 30").unwrap());
        assert_eq!(state["requested"], Value::Null);
        assert_eq!(state["fov"], 30.0);
        let state = json_of(&photo_mode(&mut app, "").unwrap());
        assert!(state.get("placed").is_none() && state.get("settled").is_none());
    }

    #[test]
    fn a_pose_photo_mode_cannot_take_is_refused_before_anything_opens() {
        let (mut app, camera) = app();
        for line in [
            "--timeout-seconds 3",
            "off --position 1 2 3",
            "--fov 500",
            "--position 1 2",
        ] {
            assert!(photo_mode(&mut app, line).is_err(), "{line}");
        }
        let refused = json_of(&photo_mode(&mut app, "--position 100 0 100").unwrap_err());
        assert_eq!(refused["cause"]["limit"], "reach");

        app.world_mut()
            .get_mut::<PrimaryCamera>(camera)
            .unwrap()
            .scene_override = Some(CameraOverride::Level(CameraZoomLevel::FirstPerson));
        let refused = json_of(&photo_mode(&mut app, "--position 1 2 3").unwrap_err());
        assert_eq!(refused["cause"]["holder"], "scene");
        assert!(app.world().get::<PhotoCamera>(camera).is_none());
    }
}
