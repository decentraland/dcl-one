use std::marker::PhantomData;

use bevy::{ecs::system::SystemParam, prelude::*, window::PrimaryWindow};
use common::{
    inputs::{Action, CommonInputAction, SystemAction, POINTER_SET},
    structs::{
        ActiveDialog, AppConfig, CameraOverride, ClientWindowFocus, CursorLocks, PhotoMode,
        PrimaryCamera,
    },
};
use input_manager::{InputManager, InputPriority};
use platform::{platform_pointer_is_locked, platform_pointer_lock_requested};

use super::{
    left_drag_pan::{self, Frame, LeftDragPan, Reason},
    pointer_results::{PointerActionTarget, UiPointerTarget, UiPointerTargetValue},
};
use crate::{
    initialize_scene::SuperUserScene, renderer_context::RendererSceneContext,
    update_world::AddCrdtInterfaceExt, HudFocus, InteractableArea, SceneSets,
};
use dcl_component::{
    proto_components::{
        common::Vector3,
        sdk::components::{PbPointerLock, PbPrimaryPointerInfo, PointerType},
    },
    SceneComponentId, SceneEntityId,
};
use dcl_types::interface::{ComponentPosition, CrdtType};

pub struct PointerLockPlugin;

impl Plugin for PointerLockPlugin {
    fn build(&self, app: &mut App) {
        app.add_crdt_lww_component::<PbPointerLock, PointerLock>(
            SceneComponentId::POINTER_LOCK,
            ComponentPosition::EntityOnly,
        );
        app.add_systems(Update, update_pointer_lock.in_set(SceneSets::Input));
    }
}

#[derive(Component)]
pub struct CumulativePointerDelta {
    pub delta: Vec2,
    pub since: f64,
}

#[derive(Component)]
pub struct PointerLock(PbPointerLock);

impl From<PbPointerLock> for PointerLock {
    fn from(value: PbPointerLock) -> Self {
        Self(value)
    }
}

#[derive(SystemParam)]
pub struct CameraInteractionState<'w, 's> {
    input_manager: InputManager<'w, 's>,
    state: Local<'s, (ClickState, f64)>,
    time: Res<'w, Time>,
    _p: PhantomData<&'s ()>,
}

#[derive(Default, Clone, Copy, PartialEq, Eq, Debug)]
pub enum ClickState {
    #[default]
    None,
    Clicked,
    Held,
    Released,
}

impl CameraInteractionState<'_, '_> {
    pub fn update(&mut self, action: Action) -> ClickState {
        match self.state.0 {
            ClickState::None | ClickState::Released => {
                if self.input_manager.just_down(action, InputPriority::None) {
                    *self.state = (ClickState::Held, self.time.elapsed_secs_f64());
                } else {
                    self.state.0 = ClickState::None;
                }
            }
            ClickState::Held => {
                if self.input_manager.just_up(action) {
                    if self.time.elapsed_secs_f64() - self.state.1 > 0.25 {
                        self.state.0 = ClickState::Released;
                    } else {
                        self.state.0 = ClickState::Clicked;
                    }
                }
            }
            ClickState::Clicked => self.state.0 = ClickState::Released,
        }

        self.state.0
    }
}

/// What the left-button pan reads besides the input. The pan and the pointer results it reads are
/// absent in a headless app (see `LeftDragPanPlugin`), which then never pans; nor does it track
/// window focus.
#[derive(SystemParam)]
pub struct LeftDragCamera<'w, 's> {
    pan: Option<ResMut<'w, LeftDragPan>>,
    ui_target: Option<Res<'w, UiPointerTarget>>,
    action_target: Option<Res<'w, PointerActionTarget>>,
    camera: Query<'w, 's, (&'static PrimaryCamera, Has<PhotoMode>)>,
    focus: Option<Res<'w, ClientWindowFocus>>,
}

impl LeftDragCamera<'_, '_> {
    fn is_panning(&self) -> bool {
        self.pan.as_ref().is_some_and(|pan| pan.is_panning())
    }

    fn interrupt(&mut self, reason: Reason) {
        if let Some(pan) = self.pan.as_mut() {
            pan.interrupt(reason);
        }
    }

    /// Advances the pan one frame; returns whether it holds the camera lock.
    fn tick(
        &mut self,
        input: &InputManager,
        window: &Window,
        enabled: bool,
        dialog_open: bool,
        camera_held_elsewhere: bool,
    ) -> bool {
        if self.pan.is_none() {
            return false;
        }
        let pointer = Action::Scene(CommonInputAction::IaPointer);
        let press = input.just_down(pointer, InputPriority::Scene);
        let panning = self.is_panning();
        let (primary, photo) = self
            .camera
            .single()
            .map_or((None, false), |(c, p)| (Some(c), p));

        let blocked = if dialog_open {
            Some(Reason::Dialog)
        } else if camera_held_elsewhere {
            Some(Reason::CameraLocked)
        } else if primary
            .is_some_and(|c| matches!(c.scene_override, Some(CameraOverride::Cinematic(_))))
        {
            Some(Reason::Cinematic)
        } else if photo {
            Some(Reason::PhotoMode)
        } else if self
            .action_target
            .as_ref()
            .is_some_and(|t| t.holds_pointer_drag())
        {
            Some(Reason::SceneDrag)
        } else if self.focus.as_deref() == Some(&ClientWindowFocus::NoFocus) {
            Some(Reason::NoFocus)
        } else if panning && platform_pointer_is_locked() == Some(false) {
            Some(Reason::LockLost)
        } else {
            None
        };

        let frame = Frame {
            press,
            held: input.is_button_down(pointer, InputPriority::Scene),
            motion: input.get_analog(POINTER_SET, InputPriority::Scroll),
            enabled,
            press_refusal: if press {
                self.press_refusal(window)
            } else {
                None
            },
            blocked,
        };
        self.pan.as_mut().is_some_and(|pan| pan.tick(&frame))
    }

    fn press_refusal(&self, window: &Window) -> Option<Reason> {
        // settled in `PreUpdate`, so it is this frame's, unlike the resolved `PointerTarget`
        if self
            .ui_target
            .as_ref()
            .is_some_and(|t| matches!(t.0, UiPointerTargetValue::Primary(..)))
        {
            return Some(Reason::OverUi);
        }
        let Some(cursor) = window.cursor_position() else {
            return Some(Reason::NoCursor);
        };
        let margin = left_drag_pan::EDGE_MARGIN;
        let inside = cursor.x >= margin
            && cursor.y >= margin
            && cursor.x <= window.width() - margin
            && cursor.y <= window.height() - margin;
        (!inside).then_some(Reason::NearEdge)
    }
}

pub fn update_pointer_lock(
    mut commands: Commands,
    mut scenes: Query<(
        Entity,
        &mut RendererSceneContext,
        Option<&mut CumulativePointerDelta>,
        Has<SuperUserScene>,
    )>,
    changed_pointer_locks: Query<&PointerLock, Changed<PointerLock>>,
    window: Query<&Window, With<PrimaryWindow>>,
    camera: Query<(&Camera, &GlobalTransform), With<PrimaryCamera>>,
    mut prev_coords: Local<(Option<Vec2>, Option<Vec2>)>,
    mut locks: ResMut<CursorLocks>,
    config: Res<AppConfig>,
    mut mb_state: CameraInteractionState,
    active_dialog: Option<Res<ActiveDialog>>,
    mut toggle: Local<bool>,
    mut dialog_was_open: Local<bool>,
    interactable_area: Res<InteractableArea>,
    hud_focus: Res<HudFocus>,
    mut left_drag: LeftDragCamera,
) {
    let Ok(window) = window.single() else {
        return;
    };
    let Ok((camera, camera_position)) = camera.single() else {
        return;
    };

    let (constrained_coordinates, unconstrained_coordinates) = if locks.0.contains("pointer") {
        *prev_coords
    } else {
        let real_window_size = Vec2::new(window.width(), window.height());
        let [left, top, right, bottom] = if config.constrain_scene_ui {
            interactable_area.get_or_default(real_window_size.x, real_window_size.y)
        } else {
            Vec4::ZERO
        }
        .to_array();

        let constrained_window_size = real_window_size - Vec2::new(right + left, bottom + top);
        if window.cursor_options.grab_mode == bevy::window::CursorGrabMode::Locked {
            // if pointer locked, just middle
            (
                Some(constrained_window_size / 2.0),
                Some(real_window_size / 2.0),
            )
        } else {
            let window_origin = Vec2::new(left, top);
            (
                window
                    .cursor_position()
                    .map(|cp| (cp - window_origin).clamp(Vec2::ZERO, constrained_window_size)),
                window.cursor_position(),
            )
        }
    };
    *prev_coords = (constrained_coordinates, unconstrained_coordinates);

    // a dialog or HUD surface frees the cursor while it is up, and the toggle is kept so the lock
    // comes back after. while it is up the toggle can be cleared but not set
    let dialog_open = active_dialog.is_some_and(|ad| ad.in_use()) || hud_focus.0;

    // Handle mouse input
    let mut state = mb_state.update(Action::System(SystemAction::CameraLock));
    let input_manager = &mb_state.input_manager;
    if state == ClickState::None
        && (input_manager.just_down(SystemAction::Cancel, InputPriority::None)
            || (!dialog_open && platform_pointer_is_locked() == Some(false)))
        && *toggle
    {
        // override
        state = ClickState::Released;
        *toggle = false;
    }

    if state == ClickState::Clicked {
        *toggle = !*toggle && !dialog_open;
        if *toggle {
            platform_pointer_lock_requested();
        }
    }

    if *dialog_was_open && !dialog_open && *toggle {
        platform_pointer_lock_requested();
    }
    *dialog_was_open = dialog_open;

    let was_panning = left_drag.is_panning();
    let panning = left_drag.tick(
        input_manager,
        window,
        config.left_drag_camera,
        dialog_open,
        *toggle,
    );
    if panning && !was_panning {
        platform_pointer_lock_requested();
    }

    let mut camera_locked = !dialog_open && (state == ClickState::Held || *toggle || panning);

    for changed_lock in changed_pointer_locks.iter() {
        if dialog_open {
            if !changed_lock.0.is_pointer_locked {
                *toggle = false;
            }
            continue;
        }
        info!("lock updated by scene");
        left_drag.interrupt(Reason::SceneLock);
        *toggle = changed_lock.0.is_pointer_locked;
        if *toggle {
            platform_pointer_lock_requested();
        }
        camera_locked = *toggle;
    }

    if camera_locked {
        locks.0.insert("camera");
    } else {
        locks.0.remove("camera");
    }

    let pointer_lock = PbPointerLock {
        is_pointer_locked: camera_locked,
    };

    let frame_delta = input_manager.get_analog(POINTER_SET, InputPriority::Scene);

    let ray = unconstrained_coordinates
        .and_then(|coords| camera.viewport_to_world(camera_position, coords).ok())
        .map(|ray| Vector3::world_vec_from_vec3(&ray.direction));

    for (entity, mut context, maybe_pointer_delta, is_super) in scenes.iter_mut() {
        let mut screen_delta = Vec2::ZERO;

        if let Some(mut pointer_delta) = maybe_pointer_delta {
            if context.last_sent == pointer_delta.since {
                pointer_delta.delta += frame_delta;
            } else {
                pointer_delta.delta = frame_delta;
                pointer_delta.since = context.last_sent;
            };
            screen_delta += pointer_delta.delta;
        } else {
            commands.entity(entity).try_insert(CumulativePointerDelta {
                delta: frame_delta,
                since: context.last_sent,
            });
        }

        let pointer_info = PbPrimaryPointerInfo {
            pointer_type: Some(PointerType::PotMouse as i32),
            screen_coordinates: if is_super {
                unconstrained_coordinates
            } else {
                constrained_coordinates
            }
            .map(Into::into),
            screen_delta: Some(screen_delta.into()),
            world_ray_direction: ray,
        };

        context.update_crdt_if_different(
            SceneComponentId::PRIMARY_POINTER_INFO,
            CrdtType::LWW_ENT,
            SceneEntityId::ROOT,
            &pointer_info,
        );

        context.update_crdt_if_different(
            SceneComponentId::POINTER_LOCK,
            CrdtType::LWW_ENT,
            SceneEntityId::CAMERA,
            &pointer_lock,
        );
    }
}

#[cfg(test)]
mod tests {
    use bevy::{math::FloatOrd, platform::collections::HashSet};
    use common::{
        inputs::{InputIdentifier, InputMap},
        structs::PointerTargetType,
    };
    use dcl_types::SceneId;
    use input_manager::{CumulativeAxisData, InputPriorities};
    use prost::Message;
    use scene_material::SceneBounds;

    use super::*;
    use crate::update_scene::{
        left_drag_pan::Phase,
        pointer_results::{ActionCandidateMode, PointerTargetInfo, PressCapture},
    };
    use dcl_component::proto_components::sdk::components::common::InputAction;

    /// The pointer moves one pixel to the right a frame while this key is down.
    const MOTION_KEY: KeyCode = KeyCode::KeyL;

    struct Rig {
        app: App,
        scene: Entity,
    }

    impl Rig {
        fn new(setting_on: bool) -> Self {
            let mut app = App::new();
            let mut map = InputMap::default();
            map.inputs.insert(
                Action::System(SystemAction::PointerRight),
                vec![InputIdentifier::Key(MOTION_KEY)],
            );
            app.insert_resource(map)
                .init_resource::<Time>()
                .init_resource::<ButtonInput<MouseButton>>()
                .init_resource::<ButtonInput<KeyCode>>()
                .init_resource::<CumulativeAxisData>()
                .init_resource::<InputPriorities>()
                .init_resource::<CursorLocks>()
                .insert_resource(AppConfig {
                    left_drag_camera: setting_on,
                    ..Default::default()
                })
                .init_resource::<InteractableArea>()
                .init_resource::<HudFocus>()
                .init_resource::<LeftDragPan>()
                .init_resource::<UiPointerTarget>()
                .init_resource::<PointerActionTarget>()
                .add_systems(Update, update_pointer_lock);
            let mut window = Window::default();
            window.set_cursor_position(Some(Vec2::new(400.0, 300.0)));
            app.world_mut().spawn((window, PrimaryWindow));
            app.world_mut().spawn((
                Camera::default(),
                GlobalTransform::default(),
                PrimaryCamera::default(),
            ));
            let scene = app
                .world_mut()
                .spawn(RendererSceneContext::new(
                    SceneId::DUMMY,
                    "hash".to_owned(),
                    "storage_root".to_owned(),
                    false,
                    0,
                    "title".to_owned(),
                    IVec2::ZERO,
                    HashSet::from_iter([IVec2::ZERO]),
                    vec![],
                    SceneBounds::default(),
                    vec![],
                    Entity::PLACEHOLDER,
                    0.,
                    false,
                    "sdk7",
                    false,
                    false,
                ))
                .id();
            Self { app, scene }
        }

        /// One frame with the edges the held state implies, then the edges cleared as bevy does.
        fn frame(&mut self) {
            self.app.update();
            self.app
                .world_mut()
                .resource_mut::<ButtonInput<MouseButton>>()
                .clear();
            self.app
                .world_mut()
                .resource_mut::<ButtonInput<KeyCode>>()
                .clear();
        }

        fn press(&mut self, button: MouseButton) {
            self.app
                .world_mut()
                .resource_mut::<ButtonInput<MouseButton>>()
                .press(button);
        }

        fn release(&mut self, button: MouseButton) {
            self.app
                .world_mut()
                .resource_mut::<ButtonInput<MouseButton>>()
                .release(button);
        }

        fn move_pointer(&mut self, moving: bool) {
            let mut keys = self.app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
            if moving {
                keys.press(MOTION_KEY);
            } else {
                keys.release(MOTION_KEY);
            }
        }

        /// Presses left and drags for three frames: the pan has started when it will.
        fn drag(&mut self) {
            self.press(MouseButton::Left);
            self.frame();
            self.move_pointer(true);
            self.frame();
            self.frame();
        }

        fn camera_locked(&self) -> bool {
            self.app
                .world()
                .resource::<CursorLocks>()
                .0
                .contains("camera")
        }

        fn crdt_lock(&self) -> bool {
            let context = self.app.world().get::<RendererSceneContext>(self.scene);
            let entry = context
                .unwrap()
                .crdt_store
                .lww
                .get(&SceneComponentId::POINTER_LOCK)
                .and_then(|state| state.last_write.get(&SceneEntityId::CAMERA))
                .expect("the lock is always written");
            PbPointerLock::decode(entry.data.as_slice())
                .unwrap()
                .is_pointer_locked
        }

        fn phase(&self) -> Phase {
            self.app.world().resource::<LeftDragPan>().phase()
        }

        fn set_scene_lock(&mut self, locked: bool) {
            self.app.world_mut().spawn(PointerLock(PbPointerLock {
                is_pointer_locked: locked,
            }));
        }
    }

    #[test]
    fn setting_off_left_drag_changes_nothing() {
        let mut rig = Rig::new(false);
        rig.drag();
        assert!(!rig.camera_locked() && !rig.crdt_lock());
        assert_eq!(rig.phase(), Phase::Idle);
        rig.release(MouseButton::Left);
        rig.frame();
        assert!(!rig.camera_locked());
    }

    #[test]
    fn a_drag_holds_the_camera_lock_until_release() {
        let mut rig = Rig::new(true);
        rig.drag();
        assert!(rig.camera_locked() && rig.crdt_lock());
        rig.release(MouseButton::Left);
        rig.frame();
        assert!(!rig.camera_locked() && !rig.crdt_lock());
        assert_eq!(rig.phase(), Phase::Idle);
    }

    #[test]
    fn a_click_without_motion_never_locks() {
        let mut rig = Rig::new(true);
        rig.press(MouseButton::Left);
        rig.frame();
        assert!(!rig.camera_locked());
        rig.frame();
        rig.release(MouseButton::Left);
        rig.frame();
        assert!(!rig.camera_locked() && !rig.crdt_lock());
    }

    #[test]
    fn a_locked_cursor_does_not_pan() {
        let mut rig = Rig::new(true);
        rig.press(MouseButton::Right);
        rig.frame();
        rig.release(MouseButton::Right);
        rig.frame();
        assert!(rig.camera_locked(), "a right click toggles the lock on");
        rig.drag();
        assert_eq!(rig.phase(), Phase::Spent);
        rig.release(MouseButton::Left);
        rig.frame();
        assert!(rig.camera_locked(), "the toggle is still held");
    }

    #[test]
    fn the_right_button_and_the_pan_both_hold_the_lock() {
        let mut rig = Rig::new(true);
        rig.drag();
        rig.press(MouseButton::Right);
        rig.frame();
        // held longer than a click, so releasing it does not toggle the lock
        rig.app
            .world_mut()
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_millis(300));
        rig.release(MouseButton::Left);
        rig.frame();
        assert!(rig.camera_locked(), "the right button still holds it");
        rig.release(MouseButton::Right);
        rig.frame();
        rig.frame();
        assert!(!rig.camera_locked());
    }

    #[test]
    fn a_scene_lock_mid_drag_outlives_the_release() {
        let mut rig = Rig::new(true);
        rig.drag();
        rig.set_scene_lock(true);
        rig.frame();
        assert_eq!(rig.phase(), Phase::Spent);
        rig.release(MouseButton::Left);
        rig.frame();
        assert!(rig.camera_locked() && rig.crdt_lock());
    }

    #[test]
    fn a_scene_unlock_mid_drag_ends_the_pan() {
        let mut rig = Rig::new(true);
        rig.drag();
        rig.set_scene_lock(false);
        rig.frame();
        assert!(!rig.camera_locked());
        rig.frame();
        assert!(!rig.camera_locked(), "the button still down does nothing");
        assert_eq!(rig.phase(), Phase::Spent);
    }

    #[test]
    fn a_dialog_mid_drag_ends_the_pan_for_good() {
        let mut rig = Rig::new(true);
        rig.drag();
        rig.app.world_mut().resource_mut::<HudFocus>().0 = true;
        rig.frame();
        assert!(!rig.camera_locked());
        rig.app.world_mut().resource_mut::<HudFocus>().0 = false;
        rig.frame();
        rig.frame();
        assert!(
            !rig.camera_locked(),
            "closing it with the button down does not resume"
        );
        rig.release(MouseButton::Left);
        rig.frame();
        assert_eq!(rig.phase(), Phase::Idle);
    }

    #[test]
    fn a_scene_drag_wins_over_the_pan() {
        let mut rig = Rig::new(true);
        rig.press(MouseButton::Left);
        rig.frame();
        let info = PointerTargetInfo {
            container: Entity::PLACEHOLDER,
            mesh_name: None,
            distance: FloatOrd(1.0),
            camera_distance: FloatOrd(1.0),
            in_scene: true,
            position: None,
            normal: None,
            face: None,
            ty: PointerTargetType::World,
        };
        rig.app
            .world_mut()
            .resource_mut::<PointerActionTarget>()
            .capture_for_test(
                InputAction::IaPointer,
                PressCapture {
                    info,
                    mode: ActionCandidateMode::Cursor,
                    lock: false,
                },
            );
        rig.move_pointer(true);
        rig.frame();
        rig.frame();
        assert!(!rig.camera_locked());
        assert_eq!(rig.phase(), Phase::Spent);
    }

    #[test]
    fn a_press_near_the_edge_never_pans() {
        let mut rig = Rig::new(true);
        let mut window = rig
            .app
            .world_mut()
            .query_filtered::<&mut Window, With<PrimaryWindow>>();
        window
            .single_mut(rig.app.world_mut())
            .unwrap()
            .set_cursor_position(Some(Vec2::new(4.0, 300.0)));
        rig.drag();
        assert!(!rig.camera_locked());
        assert_eq!(rig.phase(), Phase::Spent);
    }

    #[test]
    fn a_press_without_a_cursor_position_never_pans() {
        let mut rig = Rig::new(true);
        let mut window = rig
            .app
            .world_mut()
            .query_filtered::<&mut Window, With<PrimaryWindow>>();
        window
            .single_mut(rig.app.world_mut())
            .unwrap()
            .set_cursor_position(None);
        rig.drag();
        assert!(!rig.camera_locked());
        assert_eq!(rig.phase(), Phase::Spent);
        assert_eq!(
            rig.app.world().resource::<LeftDragPan>().last_reason(),
            Some(Reason::NoCursor)
        );
    }

    #[test]
    fn a_headless_app_without_the_pan_never_pans() {
        let mut rig = Rig::new(true);
        rig.app.world_mut().remove_resource::<LeftDragPan>();
        rig.drag();
        assert!(!rig.camera_locked() && !rig.crdt_lock());
    }

    #[test]
    fn a_missing_camera_leaves_the_pan_idle() {
        let mut rig = Rig::new(true);
        let cameras: Vec<Entity> = rig
            .app
            .world_mut()
            .query_filtered::<Entity, With<PrimaryCamera>>()
            .iter(rig.app.world())
            .collect();
        for camera in cameras {
            rig.app.world_mut().despawn(camera);
        }
        rig.drag();
        assert_eq!(rig.phase(), Phase::Idle);
        assert!(!rig.camera_locked());
    }
}
