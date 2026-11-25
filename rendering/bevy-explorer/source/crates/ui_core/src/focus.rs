use bevy::{platform::collections::HashSet, prelude::*};
use input_manager::{InputManager, InputPriority, InputType};

use crate::ui_actions::{UiActionPriority, UiFocusActionSet};

use super::ui_actions::UiActionSet;
use common::{
    inputs::{Action, CommonInputAction, SystemAction},
    sets::SceneSets,
};

#[derive(Component)]
pub struct Focus;

#[derive(Component)]
pub(super) struct PopupFocus;

#[derive(Component)]
pub struct Focusable(pub Option<Entity>);

pub struct FocusPlugin;

impl Plugin for FocusPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            PreUpdate,
            (defocus, focus)
                .chain()
                .in_set(SceneSets::UiActions)
                .after(UiActionSet)
                .before(UiFocusActionSet),
        )
        .add_systems(Last, release_unused_focus_reservations);
    }
}

fn defocus(
    mut commands: Commands,
    focus_elements: Query<(Entity, Ref<Focus>, Option<&PopupFocus>)>,
    mut removed: RemovedComponents<Focus>,
    mut removed_popups: RemovedComponents<PopupFocus>,
    mut input_manager: InputManager,
    mut we_defocussed: Local<HashSet<Entity>>,
) {
    let any_removed = removed.read().any(|e| !we_defocussed.contains(&e));
    let popup_removed = removed_popups.read().count() != 0;

    let refocussed = input_manager.just_down(CommonInputAction::IaPointer, InputPriority::Focus)
        || input_manager.just_down(SystemAction::Cancel, InputPriority::CancelFocus)
        || focus_elements
            .iter()
            .any(|(_, focus, _)| focus.is_changed())
        || any_removed;

    removed.clear();
    we_defocussed.clear();

    if refocussed {
        let closing_popup =
            popup_removed || focus_elements.iter().any(|(_, _, popup)| popup.is_some());
        let mut any_still_focussed = false;
        for (entity, ref_focus, _) in focus_elements.iter() {
            if !ref_focus.is_changed() {
                commands.entity(entity).remove::<Focus>();
                debug!("defocus {:?}", entity);
                we_defocussed.insert(entity);
            } else {
                debug!("still focussed {entity}");
                any_still_focussed = true;
            }
        }

        if any_still_focussed {
            debug!("still focus");
            input_manager.priorities().reserve(
                InputType::Action(Action::System(SystemAction::Cancel)),
                InputPriority::CancelFocus,
            );
            input_manager.priorities().reserve(
                InputType::Action(Action::Scene(CommonInputAction::IaAny)),
                InputPriority::Focus,
            );
        } else if !closing_popup {
            input_manager.priorities().release(
                InputType::Action(Action::System(SystemAction::Cancel)),
                InputPriority::CancelFocus,
            );
            input_manager.priorities().release(
                InputType::Action(Action::Scene(CommonInputAction::IaAny)),
                InputPriority::Focus,
            );
        }
    }
}

fn release_unused_focus_reservations(
    focus_elements: Query<(), With<Focus>>,
    mut input_manager: InputManager,
) {
    // A closing click belongs to the popup through scene input dispatch in this frame.
    if focus_elements.is_empty() {
        input_manager.priorities().release(
            InputType::Action(Action::System(SystemAction::Cancel)),
            InputPriority::CancelFocus,
        );
        input_manager.priorities().release(
            InputType::Action(Action::Scene(CommonInputAction::IaAny)),
            InputPriority::Focus,
        );
    }
}

fn focus(
    mut commands: Commands,
    focused_elements: Query<(Entity, &Interaction, Option<&UiActionPriority>, &Focusable)>,
    input_manager: InputManager,
) {
    // just_down can only succeed if the raw binding was just pressed; skip the per-entity
    // scan entirely otherwise
    if !input_manager.just_down_any_priority(CommonInputAction::IaPointer) {
        return;
    }

    for (entity, interaction, maybe_priority, focusable) in focused_elements.iter() {
        if interaction != &Interaction::None
            && input_manager.just_down(
                CommonInputAction::IaPointer,
                maybe_priority.copied().unwrap_or_default().0,
            )
        {
            commands
                .entity(focusable.0.unwrap_or(entity))
                .try_insert(Focus);
            debug!("focus {:?}", entity);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use common::inputs::InputMap;
    use input_manager::{CumulativeAxisData, InputPriorities};

    #[derive(Resource, Default)]
    struct SceneClicks(Vec<bool>);

    fn app() -> App {
        let mut app = App::new();
        app.init_resource::<InputMap>()
            .init_resource::<ButtonInput<MouseButton>>()
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<CumulativeAxisData>()
            .init_resource::<InputPriorities>()
            .init_resource::<SceneClicks>()
            .add_plugins(FocusPlugin)
            .add_systems(
                Update,
                |input: InputManager, mut clicks: ResMut<SceneClicks>| {
                    clicks
                        .0
                        .push(input.just_down(CommonInputAction::IaPointer, InputPriority::Scene));
                },
            );
        app
    }

    fn click(app: &mut App) {
        let mut mouse = app.world_mut().resource_mut::<ButtonInput<MouseButton>>();
        mouse.clear();
        mouse.press(MouseButton::Left);
        mouse.release(MouseButton::Left);
        app.update();
    }

    #[test]
    fn click_that_closes_focus_does_not_reach_scene() {
        let mut app = app();
        let popup = app.world_mut().spawn((Focus, PopupFocus)).id();
        app.update();
        click(&mut app);
        assert!(!app.world().resource::<SceneClicks>().0[1]);
        assert!(app.world().get::<Focus>(popup).is_none());
        click(&mut app);
        assert!(app.world().resource::<SceneClicks>().0[2]);
    }

    #[test]
    fn despawning_popup_does_not_release_its_closing_click_to_scene() {
        let mut app = app();
        let popup = app.world_mut().spawn((Focus, PopupFocus)).id();
        app.update();
        app.world_mut().despawn(popup);
        click(&mut app);
        assert!(!app.world().resource::<SceneClicks>().0[1]);
        click(&mut app);
        assert!(app.world().resource::<SceneClicks>().0[2]);
    }

    #[test]
    fn new_focus_keeps_scene_input_reserved() {
        let mut app = app();
        let first = app.world_mut().spawn((Focus, PopupFocus)).id();
        app.update();
        app.world_mut().despawn(first);
        app.world_mut().spawn((Focus, PopupFocus));
        click(&mut app);
        click(&mut app);
        assert_eq!(
            app.world().resource::<SceneClicks>().0,
            [false, false, false]
        );
    }

    #[test]
    fn leaving_field_focus_allows_the_scene_activate_click() {
        let mut app = app();
        let field = app.world_mut().spawn(Focus).id();
        app.update();
        click(&mut app);
        assert!(app.world().resource::<SceneClicks>().0[1]);
        assert!(app.world().get::<Focus>(field).is_none());
    }
}
