use bevy::prelude::*;
use common::structs::ClientWindowFocus;

/// Inserts [`ClientWindowFocus`] and keeps it at the native window's or the browser page's focus.
pub(crate) struct ClientWindowFocusPlugin;

impl Plugin for ClientWindowFocusPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ClientWindowFocus::Focused).add_systems(
            First,
            update_client_window_focus_system
                .before(crate::perf::frame_inputs::collect_frame_inputs_system),
        );
    }
}

/// Copies the primary window's focus into [`ClientWindowFocus`].
#[cfg(not(target_arch = "wasm32"))]
fn update_client_window_focus_system(
    window: Query<&Window, With<bevy::window::PrimaryWindow>>,
    mut focus: ResMut<ClientWindowFocus>,
) {
    if let Ok(window) = window.single() {
        focus.set_if_neq(ClientWindowFocus::from_window(window));
    }
}

/// Copies the page's focus into [`ClientWindowFocus`].
#[cfg(target_arch = "wasm32")]
fn update_client_window_focus_system(mut focus: ResMut<ClientWindowFocus>) {
    focus.set_if_neq(ClientWindowFocus::from_page_focus(
        crate::web::page_has_focus(),
    ));
}
