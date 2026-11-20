use std::time::Duration;

use bevy::{
    prelude::*,
    window::{PrimaryWindow, RawHandleWrapper},
    winit::DisplayHandleWrapper,
};
use winit::{event_loop::OwnedDisplayHandle, raw_window_handle::RawDisplayHandle};

/// How long the exit waits for a clipboard read or write that is in progress.
const CLOSE_WAIT: Duration = Duration::from_secs(1);

/// The session clipboard of a window on Wayland, for the engine and for the HUD page.
pub(crate) struct WaylandClipboardPlugin;

impl Plugin for WaylandClipboardPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, register_wayland_clipboard);
        #[cfg(feature = "react-hud-cef")]
        app.add_systems(
            Update,
            (hud::start_paste, hud::deliver_paste)
                .chain()
                .run_if(resource_exists::<WaylandClipboard>),
        );
    }
}

/// Present while the clipboard uses the primary window's Wayland display. It holds winit's handle
/// of that display, which keeps the connection open, and its drop ends the use: bevy's runner
/// clears the world when winit's event loop exits.
#[derive(Resource)]
struct WaylandClipboard {
    /// `None` only in tests, which have no display.
    display: Option<OwnedDisplayHandle>,
}

impl Drop for WaylandClipboard {
    fn drop(&mut self) {
        if !copypwasmta::clear_wayland_display(CLOSE_WAIT) {
            // A read that the selection's owner never answers still runs, and its worker uses
            // the display: the connection stays open for the rest of the process.
            warn!("[wayland-clipboard] a clipboard read did not end: its display stays open");
            std::mem::forget(self.display.take());
        }
    }
}

/// Gives the clipboard the primary window's Wayland display when winit put the window on Wayland,
/// so chat, text inputs and scene copies use that session's clipboard. A window on X11 registers
/// nothing, and the clipboard stays on X11.
fn register_wayland_clipboard(
    mut commands: Commands,
    window: Query<&RawHandleWrapper, (With<PrimaryWindow>, Added<RawHandleWrapper>)>,
    // absent in a run with no winit event loop
    display: Option<Res<DisplayHandleWrapper>>,
) {
    let (Ok(handle), Some(display)) = (window.single(), display) else {
        return;
    };
    if let RawDisplayHandle::Wayland(handle) = handle.get_display_handle() {
        // SAFETY: the pointer is the display of winit's Wayland connection. `WaylandClipboard`
        // holds winit's handle of that connection, which keeps the display open, and lets it go
        // only after `clear_wayland_display` said that nothing uses the display.
        if unsafe { copypwasmta::set_wayland_display(handle.display.as_ptr()) } {
            commands.insert_resource(WaylandClipboard {
                display: Some(display.0.clone()),
            });
            // only a window on Wayland hears the HUD page's clipboard messages
            #[cfg(feature = "react-hud-cef")]
            commands.add_observer(hud::on_page_message);
        }
    }
}

/// The HUD page's copies and pastes. Its webview draws offscreen, and on Wayland a client with no
/// focused surface can neither set nor read the session's clipboard: the engine does both for it.
/// On X11 the webview's own clipboard works, and nothing here runs.
#[cfg(feature = "react-hud-cef")]
mod hud {
    use std::time::Duration;

    use bevy::{
        platform::{collections::HashSet, time::Instant},
        prelude::*,
        tasks::IoTaskPool,
        winit::RawWinitWindowEvent,
    };
    use cef_offscreen::prelude::HostEmitEvent;
    use common::rpc::{RpcResultReceiver, RpcResultSender};
    use copypwasmta::{ClipboardContext, ClipboardProvider};
    use winit::{
        event::WindowEvent,
        keyboard::{KeyCode, PhysicalKey},
    };

    use crate::react_hud_cef::PageEnvelope;

    /// A text that the clipboard gives later than this after the key is not typed: the user has
    /// moved on, and it would land in another field.
    const PASTE_TIME_LIMIT: Duration = Duration::from_secs(2);

    /// On the webview whose page asked, on its current load, to copy and paste through the engine.
    #[derive(Component, Default)]
    pub(super) struct ClipboardThroughEngine {
        /// The paste under way: when its key was pressed, and the clipboard's text to come. One
        /// at a time.
        paste: Option<(Instant, RpcResultReceiver<String>)>,
    }

    /// page -> engine. Only the page's main frame is heard (`cef_offscreen` drops the rest). The
    /// page can ask for the route and can copy. It cannot ask for the clipboard's text: only
    /// [`start_paste`] reads it, for a key of the window.
    pub(super) fn on_page_message(
        trigger: Trigger<PageEnvelope>,
        routed: Query<(), With<ClipboardThroughEngine>>,
        mut commands: Commands,
    ) {
        let env = &trigger.event().0;
        if env.get("to").and_then(|t| t.as_str()) != Some("engine") {
            return;
        }
        let page = trigger.target();
        match env.get("kind").and_then(|k| k.as_str()) {
            // Sent once by each load of the page: a paste of the load before it is dropped.
            Some("engineClipboard") => {
                commands
                    .entity(page)
                    .insert(ClipboardThroughEngine::default());
                commands.trigger_targets(
                    HostEmitEvent {
                        id: "engineClipboard".to_string(),
                        payload: String::new(),
                    },
                    page,
                );
            }
            Some("copyToClipboard") if routed.contains(page) => {
                let Some(text) = env.get("text").and_then(|t| t.as_str()) else {
                    return;
                };
                let text = text.to_owned();
                IoTaskPool::get()
                    .spawn(async move {
                        let copied = match ClipboardContext::new() {
                            Ok(mut ctx) => ctx.set_contents(text).await.map_err(|e| e.to_string()),
                            Err(e) => Err(e.to_string()),
                        };
                        if let Err(e) = copied {
                            warn!("[wayland-clipboard] could not copy the HUD's text: {e}");
                        }
                    })
                    .detach();
            }
            _ => {}
        }
    }

    /// Whether these keys, each with whether it went down, hold a paste in their order. Blink's
    /// paste keys on Linux are Ctrl+V, with Shift too (paste as plain text), and Shift+Insert.
    /// `held` keeps the modifier keys that are down from one call to the next. `ButtonInput` does
    /// not serve here: it holds the state at the end of a frame, and a chord inside one slow
    /// frame is gone from it.
    fn holds_paste(
        events: impl IntoIterator<Item = (KeyCode, bool)>,
        held: &mut HashSet<KeyCode>,
    ) -> bool {
        use KeyCode::{
            AltLeft, AltRight, ControlLeft, ControlRight, Insert, KeyV, ShiftLeft, ShiftRight,
        };
        let mut paste = false;
        for (key, pressed) in events {
            match key {
                ControlLeft | ControlRight | ShiftLeft | ShiftRight | AltLeft | AltRight => {
                    if pressed {
                        held.insert(key);
                    } else {
                        held.remove(&key);
                    }
                }
                KeyV | Insert if pressed => {
                    let down = |left, right| held.contains(&left) || held.contains(&right);
                    let ctrl = down(ControlLeft, ControlRight);
                    let shift = down(ShiftLeft, ShiftRight);
                    paste |=
                        !down(AltLeft, AltRight) && if key == KeyV { ctrl } else { shift && !ctrl };
                }
                _ => {}
            }
        }
        paste
    }

    /// Reads the clipboard for the page when the user presses a paste key. It reads the keys of
    /// winit's window, not bevy's key events: the console writes those too (`/press_key`), and
    /// the page can run a console command, so a key event is not proof of the user's key. The
    /// webview gets the same key (`cef_offscreen` forwards every key), and the page's bridge
    /// stops Blink's own paste, which has no clipboard to read here.
    pub(super) fn start_paste(
        mut window_events: EventReader<RawWinitWindowEvent>,
        mut held: Local<HashSet<KeyCode>>,
        mut pages: Query<&mut ClipboardThroughEngine>,
    ) {
        let mut pasted = false;
        for event in window_events.read() {
            match &event.event {
                // key releases can go missing with the focus
                WindowEvent::Focused(false) => held.clear(),
                WindowEvent::KeyboardInput { event: key, .. } => {
                    if let PhysicalKey::Code(code) = key.physical_key {
                        pasted |= holds_paste([(code, key.state.is_pressed())], &mut held);
                    }
                }
                _ => {}
            }
        }
        if !pasted {
            return;
        }
        for mut page in &mut pages {
            if page.paste.is_some() {
                debug!("[wayland-clipboard] a paste is under way: this one is dropped");
                continue;
            }
            let (sx, rx) = RpcResultSender::channel();
            IoTaskPool::get()
                .spawn(async move {
                    let mut ctx = match ClipboardContext::new() {
                        Ok(ctx) => ctx,
                        Err(e) => {
                            warn!("[wayland-clipboard] could not read the clipboard: {e}");
                            return;
                        }
                    };
                    match ctx.get_contents().await {
                        Ok(text) => sx.send(text),
                        // an empty clipboard is an error, and so is a read still in progress
                        Err(e) => debug!("[wayland-clipboard] nothing to paste: {e}"),
                    }
                })
                .detach();
            page.paste = Some((Instant::now(), rx));
        }
    }

    /// Sends the text of a paste to the page's main frame, where the bridge types it into the
    /// field that asked. A paste with no text at the time limit ends there: the selection's
    /// owner can take any time to answer, and the next paste key starts again.
    pub(super) fn deliver_paste(
        mut pages: Query<(Entity, &mut ClipboardThroughEngine)>,
        mut commands: Commands,
    ) {
        for (entity, mut page) in &mut pages {
            let Some((started, text)) = &mut page.paste else {
                continue;
            };
            let in_time = started.elapsed() <= PASTE_TIME_LIMIT;
            let text = match text.poll_once() {
                Ok(None) if in_time => continue,
                Ok(Some(text)) if in_time => Some(text),
                Ok(_) => {
                    debug!("[wayland-clipboard] the clipboard gave no text in time to paste");
                    None
                }
                Err(()) => None,
            };
            page.paste = None;
            if let Some(payload) = text {
                commands.trigger_targets(
                    HostEmitEvent {
                        id: "paste".to_string(),
                        payload,
                    },
                    entity,
                );
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        use bevy::input::{
            keyboard::{Key, KeyboardInput},
            ButtonState,
        };
        use KeyCode::{AltLeft, ControlLeft, ControlRight, Insert, KeyC, KeyV, ShiftLeft};

        const PRESSED: bool = true;
        const RELEASED: bool = false;

        fn paste(events: &[(KeyCode, bool)]) -> bool {
            holds_paste(events.iter().copied(), &mut HashSet::default())
        }

        #[derive(Resource, Default)]
        struct Answers(Vec<String>);

        /// The plugin as the app has it, on a window that is on Wayland or not, with one webview.
        fn app(on_wayland: bool) -> (App, Entity) {
            let mut app = App::new();
            app.add_plugins(super::super::WaylandClipboardPlugin)
                .add_event::<RawWinitWindowEvent>()
                .init_resource::<Answers>()
                .add_observer(
                    |answer: Trigger<HostEmitEvent>, mut answers: ResMut<Answers>| {
                        answers.0.push(answer.event().id.clone());
                    },
                );
            if on_wayland {
                // what `register_wayland_clipboard` adds for a window on Wayland
                app.insert_resource(super::super::WaylandClipboard { display: None })
                    .add_observer(on_page_message);
            }
            let page = app.world_mut().spawn_empty().id();
            (app, page)
        }

        fn page_asks(app: &mut App, page: Entity) {
            for kind in ["engineClipboard", "paste"] {
                app.world_mut().trigger_targets(
                    PageEnvelope(serde_json::json!({ "to": "engine", "kind": kind })),
                    page,
                );
            }
            app.update();
        }

        #[test]
        fn a_window_that_is_not_on_wayland_hears_no_clipboard_message() {
            let (mut app, page) = app(false);
            page_asks(&mut app, page);
            assert!(app.world().resource::<Answers>().0.is_empty());
            assert!(app.world().get::<ClipboardThroughEngine>(page).is_none());
        }

        #[test]
        fn a_page_gets_the_route_and_never_the_text_by_asking() {
            let (mut app, page) = app(true);
            page_asks(&mut app, page);
            assert_eq!(app.world().resource::<Answers>().0, ["engineClipboard"]);
            let routed = app.world().get::<ClipboardThroughEngine>(page).unwrap();
            assert!(routed.paste.is_none());
        }

        #[test]
        fn a_key_that_the_console_plays_starts_no_paste() {
            let (mut app, page) = app(true);
            app.add_event::<KeyboardInput>();
            app.world_mut()
                .entity_mut(page)
                .insert(ClipboardThroughEngine::default());
            for (key_code, logical_key) in [
                (bevy::input::keyboard::KeyCode::ControlLeft, Key::Control),
                (
                    bevy::input::keyboard::KeyCode::KeyV,
                    Key::Character("v".into()),
                ),
            ] {
                app.world_mut().send_event(KeyboardInput {
                    key_code,
                    logical_key,
                    state: ButtonState::Pressed,
                    text: None,
                    repeat: false,
                    window: page,
                });
            }
            app.update();
            let routed = app.world().get::<ClipboardThroughEngine>(page).unwrap();
            assert!(routed.paste.is_none());
        }

        #[test]
        fn a_paste_with_no_text_at_the_time_limit_ends() {
            let (mut app, page) = app(true);
            // the read is still in progress: its sender lives
            let (_read, text) = RpcResultSender::<String>::channel();
            let started = Instant::now().checked_sub(PASTE_TIME_LIMIT * 2).unwrap();
            app.world_mut()
                .entity_mut(page)
                .insert(ClipboardThroughEngine {
                    paste: Some((started, text)),
                });
            app.update();
            let routed = app.world().get::<ClipboardThroughEngine>(page).unwrap();
            assert!(routed.paste.is_none());
            assert!(app.world().resource::<Answers>().0.is_empty());
        }

        #[test]
        fn a_paste_is_ctrl_v_or_shift_insert() {
            assert!(paste(&[(ControlLeft, PRESSED), (KeyV, PRESSED)]));
            assert!(paste(&[
                (ControlRight, PRESSED),
                (ShiftLeft, PRESSED),
                (KeyV, PRESSED)
            ]));
            assert!(paste(&[(ShiftLeft, PRESSED), (Insert, PRESSED)]));

            assert!(!paste(&[(KeyV, PRESSED)]));
            assert!(!paste(&[(Insert, PRESSED)]));
            assert!(!paste(&[(ControlLeft, PRESSED), (KeyC, PRESSED)]));
            assert!(!paste(&[(ControlLeft, PRESSED), (KeyV, RELEASED)]));
            assert!(!paste(&[
                (ControlLeft, PRESSED),
                (AltLeft, PRESSED),
                (KeyV, PRESSED)
            ]));
            assert!(!paste(&[
                (ControlLeft, PRESSED),
                (ShiftLeft, PRESSED),
                (Insert, PRESSED)
            ]));
        }

        #[test]
        fn a_chord_inside_one_frame_is_a_paste_and_the_keys_keep_their_order() {
            assert!(paste(&[
                (ControlLeft, PRESSED),
                (KeyV, PRESSED),
                (KeyV, RELEASED),
                (ControlLeft, RELEASED),
            ]));
            assert!(!paste(&[
                (ControlLeft, PRESSED),
                (ControlLeft, RELEASED),
                (KeyV, PRESSED)
            ]));
            assert!(!paste(&[(KeyV, PRESSED), (ControlLeft, PRESSED)]));
        }

        #[test]
        fn a_modifier_stays_down_from_one_frame_to_the_next() {
            let mut held = HashSet::default();
            assert!(!holds_paste([(ControlLeft, PRESSED)], &mut held));
            assert!(holds_paste([(KeyV, PRESSED)], &mut held));
            assert!(!holds_paste([(ControlLeft, RELEASED)], &mut held));
            assert!(!holds_paste([(KeyV, PRESSED)], &mut held));
        }
    }
}
