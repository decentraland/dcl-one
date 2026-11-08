//! The messages between the main world and the render world about the video players. The main
//! world owns the players; the render world owns what it presents them with. Nothing else is
//! shared between them.

use bevy::prelude::*;
use tokio::sync::mpsc;

use super::ffi::PlayerId;
#[cfg(windows)]
use super::platform;

/// What the main world tells the render world.
pub(super) enum ToRender {
    /// Present the player's frames into `image`.
    Present { id: PlayerId, image: AssetId<Image> },
    /// Stop presenting the player's frames.
    Stop(PlayerId),
    /// Stop presenting the player's frames, then free the player.
    Free(PlayerId),
    /// The decode device of the runtime that just started; it replaces the previous one.
    #[cfg(windows)]
    DecodeDevice(platform::DecodeDevice),
}

/// What the render world tells the main world.
#[derive(Debug, PartialEq)]
pub(super) enum FromRender {
    /// The size of the planes the player's frames come in; sent when it changes.
    Size {
        id: PlayerId,
        width: u32,
        height: u32,
    },
    /// A frame was drawn into the player's image.
    Presented(PlayerId),
    /// The player's frames cannot be presented; the render world has stopped presenting them.
    Failed(PlayerId),
}

/// The main world's end of the link.
pub(super) struct MainLink {
    /// `None` without a render world.
    to_render: Option<mpsc::UnboundedSender<ToRender>>,
    from_render: mpsc::UnboundedReceiver<FromRender>,
}

/// The render world's end of the link.
#[derive(Resource)]
pub(super) struct RenderLink {
    pub from_main: mpsc::UnboundedReceiver<ToRender>,
    pub to_main: mpsc::UnboundedSender<FromRender>,
}

/// Both ends of a link; no render end without a render world.
pub(super) fn link(render_world: bool) -> (MainLink, Option<RenderLink>) {
    let (to_main, from_render) = mpsc::unbounded_channel();
    if !render_world {
        return (
            MainLink {
                to_render: None,
                from_render,
            },
            None,
        );
    }
    let (to_render, from_main) = mpsc::unbounded_channel();
    (
        MainLink {
            to_render: Some(to_render),
            from_render,
        },
        Some(RenderLink { from_main, to_main }),
    )
}

impl MainLink {
    /// Asks the render world to present the player's frames into `image`. Returns whether a
    /// render world took the request.
    pub fn present(&self, id: PlayerId, image: AssetId<Image>) -> bool {
        self.send(ToRender::Present { id, image })
    }

    pub fn stop(&self, id: PlayerId) {
        self.send(ToRender::Stop(id));
    }

    /// Frees the player. A player the render world was asked to present is freed there, once it
    /// no longer reads the player's planes; any other player is freed at once. Returns whether
    /// the free went to the render world.
    pub fn free(&self, id: PlayerId, render_world: bool) -> bool {
        if render_world && self.send(ToRender::Free(id)) {
            return true;
        }
        uuav::uuav_player_free(id);
        false
    }

    #[cfg(windows)]
    pub fn send_decode_device(&self, device: platform::DecodeDevice) {
        self.send(ToRender::DecodeDevice(device));
    }

    pub fn try_recv(&mut self) -> Option<FromRender> {
        self.from_render.try_recv().ok()
    }

    /// Whether the message reached a render world.
    fn send(&self, message: ToRender) -> bool {
        self.to_render
            .as_ref()
            .is_some_and(|to_render| to_render.send(message).is_ok())
    }
}

impl Default for MainLink {
    /// A link without a render world.
    fn default() -> Self {
        link(false).0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn without_a_render_world_nothing_is_presented_and_a_player_is_freed_at_once() {
        let (main, render) = link(false);
        assert!(render.is_none());
        assert!(!main.present(7, AssetId::default()));
        assert!(!main.free(7, true));
    }

    #[test]
    fn the_render_world_frees_a_player_it_presents() {
        let (main, render) = link(true);
        let mut render = render.unwrap();
        assert!(main.present(7, AssetId::default()));
        assert!(main.free(7, true));
        assert!(matches!(
            render.from_main.try_recv(),
            Ok(ToRender::Present { id: 7, .. })
        ));
        assert!(matches!(render.from_main.try_recv(), Ok(ToRender::Free(7))));
    }

    #[test]
    fn a_player_not_presented_is_freed_at_once() {
        let (main, render) = link(true);
        let mut render = render.unwrap();
        assert!(!main.free(9, false));
        assert!(render.from_main.try_recv().is_err());
    }

    #[test]
    fn a_gone_render_world_leaves_the_free_to_the_main_world() {
        let (main, render) = link(true);
        drop(render);
        assert!(!main.free(7, true));
    }

    #[test]
    fn the_render_world_reports_to_the_main_world() {
        let (mut main, render) = link(true);
        let render = render.unwrap();
        assert!(render.to_main.send(FromRender::Presented(7)).is_ok());
        assert_eq!(main.try_recv(), Some(FromRender::Presented(7)));
        assert_eq!(main.try_recv(), None);
    }
}
