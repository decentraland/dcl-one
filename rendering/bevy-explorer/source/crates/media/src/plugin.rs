use bevy::prelude::*;

pub struct MediaPlugin;

impl Plugin for MediaPlugin {
    #[cfg(all(target_arch = "wasm32", feature = "html"))]
    fn build(&self, app: &mut App) {
        app.add_plugins(crate::html::plugin::HtmlMediaPlugin);
    }

    #[cfg(not(all(target_arch = "wasm32", feature = "html")))]
    fn build(&self, _app: &mut App) {}
}
