use bevy::{prelude::*, window::PrimaryWindow};
use common::structs::{ClientWindowFocus, RequestClientWindowFocus};

/// Inserts [`ClientWindowFocus`] and keeps it at the native window's or the browser page's focus;
/// answers [`RequestClientWindowFocus`].
pub(crate) struct ClientWindowFocusPlugin;

impl Plugin for ClientWindowFocusPlugin {
    fn build(&self, app: &mut App) {
        // apps without a window plugin get no focus reports and stay `Focused`
        #[cfg(not(target_arch = "wasm32"))]
        app.add_event::<bevy::window::WindowFocused>();
        app.insert_resource(ClientWindowFocus::Focused)
            .add_observer(request_client_window_focus)
            .add_systems(
                First,
                update_client_window_focus_system
                    .before(crate::perf::frame_inputs::collect_frame_inputs_system),
            );
    }
}

/// Sets the primary window's `focused`, which the windowing backend sends to the OS as a focus
/// request.
fn request_client_window_focus(
    _: Trigger<RequestClientWindowFocus>,
    mut window: Query<&mut Window, With<PrimaryWindow>>,
) {
    if let Ok(mut window) = window.single_mut() {
        window.focused = true;
    }
}

/// Copies the OS's focus reports for the primary window into [`ClientWindowFocus`].
#[cfg(not(target_arch = "wasm32"))]
fn update_client_window_focus_system(
    mut window_focused: EventReader<bevy::window::WindowFocused>,
    primary_window: Query<(), With<PrimaryWindow>>,
    mut focus: ResMut<ClientWindowFocus>,
) {
    if let Some(window_focused) = window_focused
        .read()
        .filter(|window_focused| primary_window.contains(window_focused.window))
        .last()
    {
        focus.set_if_neq(ClientWindowFocus::from_window_focused(window_focused));
    }
}

/// Copies the page's focus into [`ClientWindowFocus`].
#[cfg(target_arch = "wasm32")]
fn update_client_window_focus_system(mut focus: ResMut<ClientWindowFocus>) {
    focus.set_if_neq(ClientWindowFocus::from_page_focus(
        crate::web::page_has_focus(),
    ));
}
