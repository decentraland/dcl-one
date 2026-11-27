//! Entering and leaving photo mode, its per-frame input, cursor lock and scene UI hiding, and
//! flying the photo camera. The HUD opens and closes the mode and declares it as focus, which
//! reserves input above scenes; the keys read here are the mode's own.

use bevy::{
    ecs::system::SystemParam, platform::collections::HashSet, prelude::*,
    render::view::ViewUserValue,
};
use common::{
    inputs::{CommonInputAction, SystemAction, CAMERA_SET, CAMERA_ZOOM, MOVE_SET, POINTER_SET},
    sets::{PostUpdateSets, SceneSets},
    structs::{
        AppConfig, CameraView, CursorLocks, OutOfWorld, PhotoMode, PrimaryCamera, PrimaryUser,
        PLAYER_CAMERA_FOV,
    },
};
use dcl_component::proto_components::sdk::components::ColliderLayer;
use input_manager::{InputManager, InputPriority};
use scene_runner::{
    initialize_scene::SuperUserScene,
    update_scene::pointer_lock::update_pointer_lock,
    update_world::{
        mesh_collider::{GlobalGroundCollider, SceneColliderData},
        scene_ui::{SceneUiData, UiLink},
    },
    ContainingScene,
};
use tween::{SystemTween, SystemTweenData};

use crate::photo_camera::{
    Look, PhotoCamera, PhotoCameraInput, PhotoCameraSettings, PhotoModeSource, PhotoPose,
};

/// The `CursorLocks` entry photo mode holds while its cursor is locked.
const PHOTO_CURSOR_LOCK: &str = "photo";

/// Wheel motion arrives in pixels times the wheel multiplier; one wheel line is 16 of them.
const WHEEL_PIXELS_PER_LINE: f32 = 16.0;
const WHEEL_MULTIPLIER: f32 = 10.0;

/// Asks to open or close photo mode. Sent by the HUD through the console; applied once per
/// frame by `apply_photo_mode_requests`.
#[derive(Event, Clone, Copy, Debug, PartialEq, Eq)]
pub enum PhotoModeRequest {
    Enable(PhotoModeSource),
    Disable,
}

/// Photo mode just opened (`true`) or closed (`false`), for analytics and the HUD relay.
#[derive(Event, Clone, Copy, Debug, PartialEq, Eq)]
pub struct PhotoModeChanged {
    pub active: bool,
    pub source: PhotoModeSource,
}

/// What keeps photo mode closed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PhotoModeBlock {
    /// A scene's `CameraModeArea` or `VirtualCamera` holds the camera.
    SceneCamera,
    /// The player is out of the world, and the camera looks at it from above.
    OutOfWorld,
}

/// Why photo mode cannot open, and closes if it is open: the one rule for it. A request to
/// enable it is dropped while this holds, so a command that opens it asks here first.
pub fn photo_mode_block(camera: &PrimaryCamera, out_of_world: bool) -> Option<PhotoModeBlock> {
    if camera.scene_override.is_some() {
        Some(PhotoModeBlock::SceneCamera)
    } else if out_of_world {
        Some(PhotoModeBlock::OutOfWorld)
    } else {
        None
    }
}

/// Puts the open photo camera in a pose at once, for a console command (`/photo_mode --position`):
/// the follow target, the rendered camera and its projection in the frame it is asked, so the
/// pose is there for the next frame's render and nothing eases toward it.
#[derive(SystemParam)]
pub struct PhotoCameraPlacer<'w, 's> {
    commands: Commands<'w, 's>,
    camera: Query<
        'w,
        's,
        (
            Entity,
            &'static mut Transform,
            Option<&'static mut Projection>,
            &'static mut PhotoCamera,
            Has<SystemTween>,
        ),
        With<PrimaryCamera>,
    >,
    player: Query<'w, 's, &'static GlobalTransform, With<PrimaryUser>>,
    settings: Res<'w, PhotoCameraSettings>,
}

impl PhotoCameraPlacer<'_, '_> {
    /// Places the photo camera and returns where it is now: the pose asked for, or the nearest
    /// point within reach of the player. `None` while photo mode is not open.
    pub fn place(&mut self, pose: &PhotoPose) -> Option<Transform> {
        let (entity, mut transform, projection, mut photo, tweening) =
            self.camera.single_mut().ok()?;
        if tweening {
            // the opening transition from first person would carry the camera to where the mode
            // opened; the pose lands instead
            self.commands
                .entity(entity)
                .remove::<(SystemTween, SystemTweenData)>();
        }
        let player_position = self.player.single().ok().map(GlobalTransform::translation);
        *transform = photo.place(pose, &self.settings, player_position);
        if let Some(mut projection) = projection {
            if let Projection::Perspective(perspective) = &mut *projection {
                perspective.fov = photo.fov.current.to_radians();
            }
        }
        Some(*transform)
    }
}

/// A scene UI root hidden for photo mode, holding the display it had.
#[derive(Component)]
struct HiddenForPhotoMode(Display);

pub(crate) struct PhotoModePlugin;

impl Plugin for PhotoModePlugin {
    fn build(&self, app: &mut App) {
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
}

/// The keys read inside photo mode: the nametag and HUD keys and the cursor lock toggle. A scene
/// taking the camera, or the player leaving the world, closes the mode.
fn read_photo_mode_keys(
    input_manager: InputManager,
    mut camera: Query<(&PrimaryCamera, &mut PhotoCamera)>,
    player: Query<Has<OutOfWorld>, With<PrimaryUser>>,
    mut requests: EventWriter<PhotoModeRequest>,
    mut locks: ResMut<CursorLocks>,
    mut nametags: ResMut<avatar::dynamic_nametag::NametagsVisible>,
) {
    let Ok((primary, mut photo)) = camera.single_mut() else {
        return;
    };
    if photo_mode_block(primary, player.single().unwrap_or(false)).is_some() {
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
    };
    if enable == active {
        return;
    }

    if enable {
        if photo_mode_block(primary, player.single().unwrap_or(false)).is_some() {
            return;
        }
        let first_person = primary.view() == CameraView::FirstPerson;
        let start = PhotoCamera::start_transform(&transform, first_person, &settings);
        let fov = match &*projection {
            Projection::Perspective(p) => p.fov,
            _ => PLAYER_CAMERA_FOV,
        };
        *sessions += 1;
        let mut photo = PhotoCamera::new(start, fov.to_degrees(), source);
        photo.session = *sessions;
        photo.nametags_visible_before = nametags.0;
        // the camera-to-player dither would show in the photos; the player camera restores it
        commands
            .entity(entity)
            .insert((PhotoMode, photo, ViewUserValue(Vec4::ZERO)));
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
        locks.0.remove(PHOTO_CURSOR_LOCK);
        changed.write(PhotoModeChanged {
            active: false,
            source,
        });
    }
}

/// Scene UI is hidden for the whole of photo mode; roots that appear, or are restyled for a new
/// HUD inset, meanwhile are hidden too.
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
            (true, Some(_)) => {
                if node.display != Display::None {
                    let display = node.display;
                    node.display = Display::None;
                    commands.entity(entity).insert(HiddenForPhotoMode(display));
                }
            }
            (false, Some(HiddenForPhotoMode(display))) => {
                node.display = *display;
                commands.entity(entity).remove::<HiddenForPhotoMode>();
            }
            (false, None) => {}
        }
    }
}

/// Reads this frame's input into the photo camera and writes the rendered camera. Waits for the
/// opening transition to land before the follow target takes over.
fn move_photo_camera(
    time: Res<Time>,
    settings: Res<PhotoCameraSettings>,
    config: Res<AppConfig>,
    input_manager: InputManager,
    locks: Res<CursorLocks>,
    mut camera: Query<
        (
            &mut Transform,
            &mut Projection,
            &mut PhotoCamera,
            &PrimaryCamera,
        ),
        (Without<SystemTween>, Without<PrimaryUser>),
    >,
    player: Query<&GlobalTransform, With<PrimaryUser>>,
    containing_scene: ContainingScene,
    mut scene_colliders: Query<&mut SceneColliderData, Without<GlobalGroundCollider>>,
    mut ground: Query<&mut SceneColliderData, With<GlobalGroundCollider>>,
) {
    let Ok((mut transform, mut projection, mut photo, primary)) = camera.single_mut() else {
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

    let p = InputPriority::Ui;
    // a held zoom button and the wheel share the zoom actions: the wheel is read when none is held
    let zoom_held = button(&input_manager, SystemAction::CameraZoomIn)
        - button(&input_manager, SystemAction::CameraZoomOut);
    let zoom_lines = if zoom_held == 0.0 {
        input_manager.get_analog(CAMERA_ZOOM, p).y / (WHEEL_MULTIPLIER * WHEEL_PIXELS_PER_LINE)
    } else {
        0.0
    };
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
        zoom_lines,
        zoom_held,
        cursor_locked,
    };
    let look = Look {
        sensitivity: primary.sensitivity,
        smoothing: config.camera_smoothing,
    };

    let movement = photo.requested_move(&input, &settings, dt, player_position);
    let length = movement.length();
    // a step at run speed is longer than the camera's sphere, so the whole move is swept
    let blocked = length > 0.0 && {
        let start = photo.target.translation;
        let origin = start.as_dvec3();
        let direction = (movement / length).as_dvec3();
        let radius = settings.collider_radius as f64;
        let mask = ColliderLayer::ClPhysics as u32;
        let hits = |colliders: &mut SceneColliderData| {
            colliders
                .cast_ball_nearest(origin, direction, length as f64, radius, mask)
                .is_some()
        };
        let mut scenes: HashSet<Entity> = containing_scene.get_position(start);
        scenes.extend(containing_scene.get_position(start + movement));
        scenes.iter().any(|scene| {
            scene_colliders
                .get_mut(*scene)
                .is_ok_and(|mut colliders| hits(&mut colliders))
        }) || ground.iter_mut().any(|mut colliders| hits(&mut colliders))
    };
    if !blocked {
        photo.apply_move(movement);
    }
    photo.steer(&input, &settings, &look, dt);

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

fn button(input_manager: &InputManager, action: SystemAction) -> f32 {
    if input_manager.is_button_down(action, InputPriority::Ui) {
        1.0
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use common::structs::CameraZoomLevel;

    use super::*;

    /// A pose a console command asks for, placed in the frame it is asked.
    #[derive(Resource, Default)]
    struct Asked(Option<PhotoPose>);

    #[derive(Resource, Default)]
    struct Placed(Option<Transform>);

    fn place(mut asked: ResMut<Asked>, mut placed: ResMut<Placed>, mut placer: PhotoCameraPlacer) {
        if let Some(pose) = asked.0.take() {
            placed.0 = placer.place(&pose);
        }
    }

    /// Photo mode's opening and closing with the engine's own camera transition, a first-person
    /// camera at eye height and a player at the origin.
    fn app() -> (App, Entity, Entity) {
        let mut app = App::new();
        app.add_event::<PhotoModeRequest>()
            .add_event::<PhotoModeChanged>()
            .init_resource::<PhotoCameraSettings>()
            .init_resource::<CursorLocks>()
            .insert_resource(avatar::dynamic_nametag::NametagsVisible(true))
            .init_resource::<Time>()
            .init_resource::<Asked>()
            .init_resource::<Placed>()
            .add_systems(Update, (place, apply_photo_mode_requests).chain())
            .add_systems(PostUpdate, tween::update_system_tween);
        let camera = app
            .world_mut()
            .spawn((
                Transform::from_xyz(0.0, 1.8, 0.0),
                Projection::Perspective(PerspectiveProjection::default()),
                PrimaryCamera {
                    level: CameraZoomLevel::FirstPerson,
                    ..Default::default()
                },
            ))
            .id();
        let player = app
            .world_mut()
            .spawn((PrimaryUser::default(), GlobalTransform::IDENTITY))
            .id();
        (app, camera, player)
    }

    /// A quarter of a second.
    fn frame(app: &mut App) {
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(Duration::from_secs_f64(0.25));
        app.update();
    }

    fn ask(app: &mut App, pose: PhotoPose) -> Option<Transform> {
        app.world_mut().resource_mut::<Asked>().0 = Some(pose);
        frame(app);
        app.world_mut().resource_mut::<Placed>().0.take()
    }

    #[test]
    fn a_scene_camera_or_a_player_out_of_the_world_keeps_photo_mode_closed() {
        let camera = PrimaryCamera::default();
        assert_eq!(photo_mode_block(&camera, false), None);
        assert_eq!(
            photo_mode_block(&camera, true),
            Some(PhotoModeBlock::OutOfWorld)
        );

        // a request to open it is dropped meanwhile
        let (mut app, camera, player) = app();
        app.world_mut()
            .entity_mut(player)
            .insert(OutOfWorld::default());
        app.world_mut()
            .send_event(PhotoModeRequest::Enable(PhotoModeSource::Console));
        frame(&mut app);
        assert!(app.world().get::<PhotoCamera>(camera).is_none());
        // and there is no camera to place
        assert_eq!(ask(&mut app, PhotoPose::default()), None);
    }

    #[test]
    fn a_pose_placed_while_photo_mode_opens_from_first_person_drops_the_opening_transition() {
        let (mut app, camera, _) = app();
        app.world_mut()
            .send_event(PhotoModeRequest::Enable(PhotoModeSource::Console));
        frame(&mut app);
        // opening from first person eases the camera out to behind the eye
        assert!(app.world().get::<PhotoCamera>(camera).is_some());
        assert!(app.world().get::<SystemTween>(camera).is_some());
        assert!(app.world().get::<SystemTweenData>(camera).is_some());

        let position = Vec3::new(10.0, 5.0, -10.0);
        let point = Vec3::new(10.0, 1.0, -20.0);
        let pose = PhotoPose {
            position: Some(position),
            look_at: Some(point),
            fov: Some(40.0),
        };
        let placed = ask(&mut app, pose).expect("photo mode is open");
        assert_eq!(placed.translation, position);
        // the transition is gone, with its bookkeeping, and the pose is on the rendered camera
        assert!(app.world().get::<SystemTween>(camera).is_none());
        assert!(app.world().get::<SystemTweenData>(camera).is_none());
        let rendered = |app: &App| *app.world().get::<Transform>(camera).unwrap();
        assert_eq!(rendered(&app), placed);
        let off = placed
            .forward()
            .angle_between(point - position)
            .to_degrees();
        assert!(off < 0.01, "{off}");
        let Some(Projection::Perspective(perspective)) = app.world().get::<Projection>(camera)
        else {
            panic!("a perspective camera");
        };
        assert!((perspective.fov - 40f32.to_radians()).abs() < 1e-5);
        let photo = app.world().get::<PhotoCamera>(camera).unwrap();
        assert_eq!(photo.target, placed);
        assert!(photo.following);

        // nothing carries it off afterwards
        for _ in 0..8 {
            frame(&mut app);
        }
        assert_eq!(rendered(&app), placed);
    }
}
