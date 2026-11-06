//! Entering and leaving photo mode, its per-frame input, cursor lock and scene UI hiding, and
//! flying the photo camera.

use bevy::{platform::collections::HashSet, prelude::*};
use common::{
    inputs::{CommonInputAction, SystemAction, CAMERA_SET, CAMERA_ZOOM, MOVE_SET, POINTER_SET},
    sets::{PostUpdateSets, SceneSets},
    structs::{CursorLocks, OutOfWorld, PhotoMode, PrimaryCamera, PrimaryUser, PLAYER_CAMERA_FOV},
};
use dcl_component::proto_components::sdk::components::ColliderLayer;
use input_manager::{InputManager, InputPriority, InputType};
use rapier3d_f64::prelude::ColliderBuilder;
use scene_runner::{
    initialize_scene::SuperUserScene,
    update_scene::pointer_lock::update_pointer_lock,
    update_world::{
        mesh_collider::{GlobalGroundCollider, SceneColliderData},
        scene_ui::{SceneUiData, UiLink},
    },
    ContainingScene,
};
use tween::SystemTween;

use crate::photo_camera::{PhotoCamera, PhotoCameraInput, PhotoCameraSettings, PhotoModeSource};

/// The `CursorLocks` entry photo mode holds while its cursor is locked.
const PHOTO_CURSOR_LOCK: &str = "photo";

/// Wheel motion arrives in pixels times the wheel multiplier; one wheel line is 16 of them.
const WHEEL_PIXELS_PER_LINE: f32 = 16.0;
const WHEEL_MULTIPLIER: f32 = 10.0;

/// Asks to open, close or toggle photo mode. Sent by the key, the HUD and the console; applied
/// once per frame by `apply_photo_mode_requests`.
#[derive(Event, Clone, Copy, Debug, PartialEq, Eq)]
pub enum PhotoModeRequest {
    Enable(PhotoModeSource),
    Disable,
    Toggle(PhotoModeSource),
}

/// Photo mode just opened (`true`) or closed (`false`), for analytics and the HUD relay.
#[derive(Event, Clone, Copy, Debug, PartialEq, Eq)]
pub struct PhotoModeChanged {
    pub active: bool,
    pub source: PhotoModeSource,
}

/// A scene UI root hidden for photo mode, holding the display it had.
#[derive(Component)]
struct HiddenForPhotoMode(Display);

pub(crate) fn register(app: &mut App) {
    app.init_resource::<PhotoCameraSettings>()
        .add_event::<PhotoModeRequest>()
        .add_event::<PhotoModeChanged>()
        .add_systems(
            Update,
            (
                read_photo_mode_keys,
                apply_photo_mode_requests,
                hide_scene_ui_in_photo_mode,
            )
                .chain()
                .in_set(SceneSets::Input)
                .after(update_pointer_lock),
        )
        .add_systems(
            PostUpdate,
            move_photo_camera.in_set(PostUpdateSets::CameraUpdate),
        );
}

/// The keys photo mode owns: its toggle outside and inside the mode, and inside it the close,
/// gallery, nametag and HUD keys and the cursor lock toggle.
#[allow(clippy::too_many_arguments)]
fn read_photo_mode_keys(
    mut input_manager: InputManager,
    mut camera: Query<(&PrimaryCamera, Option<&mut PhotoCamera>)>,
    player: Query<Has<OutOfWorld>, With<PrimaryUser>>,
    mut requests: EventWriter<PhotoModeRequest>,
    mut locks: ResMut<CursorLocks>,
    mut nametags: ResMut<avatar::dynamic_nametag::NametagsVisible>,
) {
    let Ok((primary, photo)) = camera.single_mut() else {
        return;
    };
    let out_of_world = player.single().unwrap_or(false);

    let Some(mut photo) = photo else {
        if input_manager.just_down(SystemAction::PhotoMode, InputPriority::Ui) {
            requests.write(PhotoModeRequest::Toggle(PhotoModeSource::Shortcut));
        }
        return;
    };

    if primary.scene_override.is_some() || out_of_world {
        requests.write(PhotoModeRequest::Disable);
        return;
    }

    // the reservation is re-asserted each frame: a HUD surface closing releases the same key
    input_manager
        .priorities()
        .reserve(InputType::All, InputPriority::Ui);

    if input_manager.just_down(SystemAction::PhotoMode, InputPriority::Ui)
        || input_manager.just_down(SystemAction::Cancel, InputPriority::Ui)
        || input_manager.just_down(SystemAction::Gallery, InputPriority::Ui)
    {
        requests.write(PhotoModeRequest::Disable);
        return;
    }
    if input_manager.just_down(SystemAction::HideUi, InputPriority::Ui) {
        photo.hud_hidden = !photo.hud_hidden;
    }
    if input_manager.just_down(SystemAction::HideNames, InputPriority::Ui) {
        nametags.0 = !nametags.0;
    }

    let locked = locks.0.contains(PHOTO_CURSOR_LOCK);
    let browser_released = locked && platform::platform_pointer_is_locked() == Some(false);
    if input_manager.just_down(SystemAction::CameraLock, InputPriority::Ui) || browser_released {
        if locked {
            locks.0.remove(PHOTO_CURSOR_LOCK);
        } else {
            locks.0.insert(PHOTO_CURSOR_LOCK);
            platform::platform_pointer_lock_requested();
        }
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(crate) fn apply_photo_mode_requests(
    mut commands: Commands,
    mut requests: EventReader<PhotoModeRequest>,
    mut changed: EventWriter<PhotoModeChanged>,
    mut camera: Query<(
        Entity,
        &mut Transform,
        &mut Projection,
        &PrimaryCamera,
        Option<&PhotoCamera>,
    )>,
    player: Query<Has<OutOfWorld>, With<PrimaryUser>>,
    settings: Res<PhotoCameraSettings>,
    mut input_manager: InputManager,
    mut locks: ResMut<CursorLocks>,
    mut nametags: ResMut<avatar::dynamic_nametag::NametagsVisible>,
    mut sessions: Local<u32>,
) {
    let Some(request) = requests.read().last().copied() else {
        return;
    };
    let Ok((entity, mut transform, mut projection, primary, photo)) = camera.single_mut() else {
        return;
    };
    let active = photo.is_some();
    let (enable, source) = match request {
        PhotoModeRequest::Enable(source) => (true, source),
        PhotoModeRequest::Disable => (false, PhotoModeSource::Console),
        PhotoModeRequest::Toggle(source) => (!active, source),
    };
    if enable == active {
        return;
    }

    if enable {
        let out_of_world = player.single().unwrap_or(false);
        if primary.scene_override.is_some() || out_of_world {
            return;
        }
        let first_person = primary.distance < 0.05;
        let start = PhotoCamera::start_transform(&transform, first_person, &settings);
        let fov = match &*projection {
            Projection::Perspective(p) => p.fov,
            _ => PLAYER_CAMERA_FOV,
        };
        *sessions += 1;
        let mut photo = PhotoCamera::new(start, fov.to_degrees(), source);
        photo.session = *sessions;
        photo.nametags_visible_before = nametags.0;
        commands.entity(entity).insert((PhotoMode, photo));
        if first_person {
            commands.entity(entity).insert(SystemTween {
                target: start,
                time: settings.enter_from_first_person_secs,
                fov: Some(fov),
            });
        } else {
            *transform = start;
            if let Projection::Perspective(p) = &mut *projection {
                p.fov = fov;
            }
        }
        input_manager
            .priorities()
            .reserve(InputType::All, InputPriority::Ui);
        locks.0.insert(PHOTO_CURSOR_LOCK);
        platform::platform_pointer_lock_requested();
        changed.write(PhotoModeChanged {
            active: true,
            source,
        });
    } else {
        let source = photo.map(|p| p.source).unwrap_or(source);
        // N flips the global setting for the photo only
        if let Some(photo) = photo {
            if nametags.0 != photo.nametags_visible_before {
                nametags.0 = photo.nametags_visible_before;
            }
        }
        let current = *transform;
        // the player camera retargets this tween to its follow position next frame
        commands
            .entity(entity)
            .remove::<(PhotoMode, PhotoCamera)>()
            .insert(SystemTween {
                target: Transform {
                    scale: Vec3::ONE,
                    ..current
                },
                time: settings.exit_secs,
                fov: Some(PLAYER_CAMERA_FOV),
            });
        input_manager
            .priorities()
            .release(InputType::All, InputPriority::Ui);
        locks.0.remove(PHOTO_CURSOR_LOCK);
        changed.write(PhotoModeChanged {
            active: false,
            source,
        });
    }
}

/// Scene UI is hidden for the whole of photo mode; roots that appear meanwhile are hidden too.
#[allow(clippy::type_complexity)]
fn hide_scene_ui_in_photo_mode(
    mut commands: Commands,
    photo: Query<(), (With<PhotoMode>, With<PrimaryCamera>)>,
    roots: Query<&UiLink, (With<SceneUiData>, Without<SuperUserScene>)>,
    mut nodes: Query<(Entity, &mut Node, Option<&HiddenForPhotoMode>)>,
) {
    let active = !photo.is_empty();
    for link in roots.iter() {
        let Ok((entity, mut node, hidden)) = nodes.get_mut(link.ui_entity) else {
            continue;
        };
        match (active, hidden) {
            (true, None) => {
                let display = node.display;
                node.display = Display::None;
                commands.entity(entity).insert(HiddenForPhotoMode(display));
            }
            (false, Some(HiddenForPhotoMode(display))) => {
                node.display = *display;
                commands.entity(entity).remove::<HiddenForPhotoMode>();
            }
            _ => {}
        }
    }
}

/// Reads this frame's input into the photo camera and writes the rendered camera. Waits for the
/// opening transition to land before the follow target takes over.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn move_photo_camera(
    time: Res<Time>,
    settings: Res<PhotoCameraSettings>,
    input_manager: InputManager,
    locks: Res<CursorLocks>,
    mut camera: Query<
        (&mut Transform, &mut Projection, &mut PhotoCamera),
        (
            With<PrimaryCamera>,
            Without<SystemTween>,
            Without<PrimaryUser>,
        ),
    >,
    player: Query<&GlobalTransform, With<PrimaryUser>>,
    containing_scene: ContainingScene,
    mut scene_colliders: Query<&mut SceneColliderData, Without<GlobalGroundCollider>>,
    mut ground: Query<&mut SceneColliderData, With<GlobalGroundCollider>>,
) {
    let Ok((mut transform, mut projection, mut photo)) = camera.single_mut() else {
        return;
    };
    let Ok(player_position) = player.single().map(|gt| gt.translation()) else {
        return;
    };
    if !photo.following {
        photo.attach(&transform);
    }
    let dt = time.delta_secs();
    let cursor_locked = locks.0.contains(PHOTO_CURSOR_LOCK);

    let wheel = input_manager.get_analog(CAMERA_ZOOM, InputPriority::Ui).y;
    let zoom = if cfg!(target_os = "macos") {
        wheel / WHEEL_MULTIPLIER
    } else {
        wheel / (WHEEL_MULTIPLIER * WHEEL_PIXELS_PER_LINE)
    };
    let p = InputPriority::Ui;
    // the arrows are a look set, so its up slot is CameraDown: flip it to read as movement
    let arrows = input_manager.get_analog(CAMERA_SET, p) * Vec2::new(1.0, -1.0);
    let input = PhotoCameraInput {
        translation: (input_manager.get_analog(MOVE_SET, p) + arrows)
            .clamp(Vec2::NEG_ONE, Vec2::ONE),
        panning: axis(
            &input_manager,
            SystemAction::PhotoCameraUp,
            SystemAction::PhotoCameraDown,
        ),
        tilting: axis(
            &input_manager,
            SystemAction::PhotoTiltRight,
            SystemAction::PhotoTiltLeft,
        ),
        running: input_manager.is_down(CommonInputAction::IaModifier, p),
        walking: input_manager.is_down(CommonInputAction::IaWalk, p),
        aim: input_manager.get_analog(POINTER_SET, p),
        mouse_dragging: !cursor_locked && input_manager.is_down(CommonInputAction::IaPointer, p),
        zoom,
        cursor_locked,
    };

    let movement = photo.requested_move(&input, &settings, dt, player_position);
    let desired = photo.target.translation + movement;
    let blocked = movement != Vec3::ZERO && {
        let probe = ColliderBuilder::ball(settings.collider_radius as f64)
            .translation(desired.as_dvec3().into())
            .build();
        let mask = ColliderLayer::ClPhysics as u32;
        let scenes: HashSet<Entity> = containing_scene.get_position(desired);
        let mut hit = false;
        for scene in scenes.iter() {
            if let Ok(mut colliders) = scene_colliders.get_mut(*scene) {
                hit |= !colliders.intersect_collider(&probe, mask).is_empty();
            }
        }
        hit || ground
            .iter_mut()
            .any(|mut colliders| !colliders.intersect_collider(&probe, mask).is_empty())
    };
    if !blocked {
        photo.apply_move(movement);
    }
    photo.steer(&input, &settings, dt);

    *transform = photo.follow(&transform, &settings, dt);
    if let Projection::Perspective(perspective) = &mut *projection {
        let fov = photo.fov.current.to_radians();
        if perspective.fov != fov {
            perspective.fov = fov;
        }
    }
}

fn axis(input_manager: &InputManager, positive: SystemAction, negative: SystemAction) -> f32 {
    let p = InputPriority::Ui;
    (if input_manager.is_down(positive, p) {
        1.0
    } else {
        0.0
    }) - (if input_manager.is_down(negative, p) {
        1.0
    } else {
        0.0
    })
}
