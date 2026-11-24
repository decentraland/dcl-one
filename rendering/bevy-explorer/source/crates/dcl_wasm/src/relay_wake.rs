//! How a sandbox worker's relay (`inner::relay`) wakes on the engine's send instead of its 1 ms
//! poll. The worker waits on a word in engine memory between pumps when the scene loop mode wakes
//! relays (scene_loop_mode `relay_wake`) and the engine wakes its scene (the scene's
//! `dcl_types::SceneWake`, which the engine sets each frame): the engine bumps the word and
//! notifies when it sends the scene something, through the waker the relay registers on its
//! engine channels. The worker reads the word's address (`relay_wake_ptr`) and only loads and
//! waits on it; the engine alone writes it. An Arc keeps the word alive while a channel still
//! holds the waker, after the relay is dropped.
//! The notify is passed in, so the host tests check the send, the bump and the notify without a
//! browser; on the web it is [`engine_notify`]. Outside the wasm-only module for those tests.

use std::{
    sync::{
        atomic::{AtomicBool, AtomicI32, Ordering},
        Arc,
    },
    task::{Context, Poll, Wake},
};

use tokio::sync::mpsc;

use crate::scene_loop_mode::SceneLoopMode;

/// Wakes whoever waits on the word.
pub type Notify = Box<dyn Fn(&AtomicI32) + Send + Sync>;

pub struct RelayWake {
    seq: AtomicI32,
    // whether the worker waits on the word, as the relay last decided (`update_waits`); a waker
    // left on a channel after that turned off bumps the word without a notify
    waits: AtomicBool,
    notify: Notify,
}

impl RelayWake {
    pub fn new(notify: Notify) -> Arc<Self> {
        Arc::new(Self {
            seq: AtomicI32::new(0),
            waits: AtomicBool::new(false),
            notify,
        })
    }

    /// The word the worker waits on.
    pub fn word(&self) -> &AtomicI32 {
        &self.seq
    }

    pub fn waits(&self) -> bool {
        self.waits.load(Ordering::SeqCst)
    }

    /// Decides, before a pump, whether the worker waits for the engine's send: in a mode that
    /// wakes relays, for a scene the engine wakes (`wakes_scene`).
    pub fn update_waits(&self, mode: SceneLoopMode, wakes_scene: bool) -> bool {
        let waits = mode.relay_wake() && wakes_scene;
        self.waits.store(waits, Ordering::SeqCst);
        waits
    }
}

impl Wake for RelayWake {
    fn wake(self: Arc<Self>) {
        self.wake_by_ref();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        self.seq.fetch_add(1, Ordering::SeqCst);
        if self.waits() {
            (self.notify)(&self.seq);
        }
    }
}

/// The next message on an engine channel. When the worker waits for the engine's send, an empty
/// channel registers the waker in `cx` before it waits: anything sent from then on bumps the word.
pub fn next<T>(rx: &mut mpsc::UnboundedReceiver<T>, waits: bool, cx: &mut Context) -> Option<T> {
    match rx.try_recv() {
        Ok(message) => Some(message),
        Err(mpsc::error::TryRecvError::Empty) if waits => match rx.poll_recv(cx) {
            Poll::Ready(message) => message,
            Poll::Pending => None,
        },
        Err(_) => None,
    }
}

#[cfg(target_arch = "wasm32")]
mod web {
    use std::{
        cell::{Cell, RefCell},
        sync::atomic::AtomicI32,
    };

    use wasm_bindgen::JsCast;

    thread_local! {
        // set on a relay's thread: it has no glue, so the notify never reaches JS from there (and
        // the relay is running anyway, so it needs none)
        static RELAY_THREAD: Cell<bool> = const { Cell::new(false) };
        // engine threads: a view over the engine memory for the notify, renewed when it grows
        static WAKE_VIEW: RefCell<Option<js_sys::Int32Array>> = const { RefCell::new(None) };
    }

    /// Marks this thread as a relay's. False if the thread is being torn down.
    pub fn mark_relay_thread() -> bool {
        RELAY_THREAD.try_with(|relay| relay.set(true)).is_ok()
    }

    /// The notify on the web, from an engine thread (it has the glue): `Atomics.notify` on the
    /// word in engine memory. Nothing on a relay's thread.
    pub fn engine_notify(word: &AtomicI32) {
        if RELAY_THREAD.try_with(Cell::get).unwrap_or(true) {
            return;
        }
        let index = (word as *const AtomicI32 as usize / 4) as u32;
        let _ = WAKE_VIEW.try_with(|view| {
            let mut view = view.borrow_mut();
            if view.as_ref().is_none_or(|v| index >= v.length()) {
                let memory = wasm_bindgen::memory().unchecked_into::<js_sys::WebAssembly::Memory>();
                *view = Some(js_sys::Int32Array::new(&memory.buffer()));
            }
            if let Some(view) = view.as_ref() {
                let _ = js_sys::Atomics::notify(view, index);
            }
        });
    }
}

#[cfg(target_arch = "wasm32")]
pub use web::{engine_notify, mark_relay_thread};

#[cfg(test)]
mod tests {
    use std::{sync::atomic::AtomicUsize, task::Waker, thread};

    use super::*;

    // a relay's wake with a notify that counts, and its waker
    fn counted() -> (Arc<RelayWake>, Arc<AtomicUsize>, Waker) {
        let notified = Arc::new(AtomicUsize::new(0));
        let count = notified.clone();
        let wake = RelayWake::new(Box::new(move |_| {
            count.fetch_add(1, Ordering::SeqCst);
        }));
        let waker = Waker::from(wake.clone());
        (wake, notified, waker)
    }

    // the engine sends from its own thread
    fn engine_send(tx: mpsc::UnboundedSender<u32>, message: u32) {
        thread::spawn(move || tx.send(message).unwrap())
            .join()
            .unwrap();
    }

    #[test]
    fn the_engine_send_bumps_the_word_and_notifies_a_woken_scene() {
        let (wake, notified, waker) = counted();
        let mut cx = Context::from_waker(&waker);
        assert!(wake.update_waits(SceneLoopMode::Both, true));
        let (tx, mut rx) = mpsc::unbounded_channel();

        // the pump finds the channel empty and registers the waker
        assert_eq!(next(&mut rx, wake.waits(), &mut cx), None);
        assert_eq!(wake.word().load(Ordering::SeqCst), 0);

        engine_send(tx.clone(), 7);
        assert_eq!(wake.word().load(Ordering::SeqCst), 1);
        assert_eq!(notified.load(Ordering::SeqCst), 1);
        assert_eq!(next(&mut rx, wake.waits(), &mut cx), Some(7));

        // each empty pump registers again, for the next send
        assert_eq!(next(&mut rx, wake.waits(), &mut cx), None);
        engine_send(tx, 8);
        assert_eq!(wake.word().load(Ordering::SeqCst), 2);
        assert_eq!(notified.load(Ordering::SeqCst), 2);
        assert_eq!(next(&mut rx, wake.waits(), &mut cx), Some(8));
    }

    #[test]
    fn a_scene_the_engine_does_not_wake_registers_no_waker() {
        let (wake, notified, waker) = counted();
        let mut cx = Context::from_waker(&waker);
        assert!(!wake.update_waits(SceneLoopMode::Both, false));
        let (tx, mut rx) = mpsc::unbounded_channel();

        assert_eq!(next(&mut rx, wake.waits(), &mut cx), None);
        engine_send(tx, 7);
        assert_eq!(wake.word().load(Ordering::SeqCst), 0);
        assert_eq!(notified.load(Ordering::SeqCst), 0);
        // the poll still delivers it
        assert_eq!(next(&mut rx, wake.waits(), &mut cx), Some(7));
    }

    #[test]
    fn modes_without_the_relay_wake_keep_the_poll() {
        for mode in [SceneLoopMode::Legacy, SceneLoopMode::Deadline] {
            let (wake, notified, waker) = counted();
            let mut cx = Context::from_waker(&waker);
            assert!(!wake.update_waits(mode, true), "{mode:?}");
            let (tx, mut rx) = mpsc::unbounded_channel();

            assert_eq!(next(&mut rx, wake.waits(), &mut cx), None);
            engine_send(tx, 7);
            assert_eq!(wake.word().load(Ordering::SeqCst), 0, "{mode:?}");
            assert_eq!(notified.load(Ordering::SeqCst), 0, "{mode:?}");
            assert_eq!(next(&mut rx, wake.waits(), &mut cx), Some(7));
        }
    }

    #[test]
    fn a_waker_left_after_the_wait_turns_off_bumps_without_a_notify() {
        let (wake, notified, waker) = counted();
        let mut cx = Context::from_waker(&waker);
        assert!(wake.update_waits(SceneLoopMode::Both, true));
        let (tx, mut rx) = mpsc::unbounded_channel();
        assert_eq!(next(&mut rx, wake.waits(), &mut cx), None);

        // the player leaves the scene before the engine sends
        assert!(!wake.update_waits(SceneLoopMode::Both, false));
        engine_send(tx, 7);
        assert_eq!(wake.word().load(Ordering::SeqCst), 1);
        assert_eq!(notified.load(Ordering::SeqCst), 0);
    }

    /// The engine's per-scene flag, as the relay reads it before each pump: set from the engine's
    /// thread, it turns the wait on and off from the next pump; the mode decides first.
    #[test]
    fn the_relay_follows_the_engines_flag_for_its_scene() {
        let (wake, _, _) = counted();
        let flag = dcl_types::SceneWake::default();
        let engine = flag.clone();
        assert!(!wake.update_waits(SceneLoopMode::Both, flag.get()));
        thread::spawn(move || engine.set(true)).join().unwrap();
        assert!(wake.update_waits(SceneLoopMode::Both, flag.get()));
        assert!(!wake.update_waits(SceneLoopMode::Legacy, flag.get()));
        let engine = flag.clone();
        thread::spawn(move || engine.set(false)).join().unwrap();
        assert!(!wake.update_waits(SceneLoopMode::Both, flag.get()));
    }
}

// In headless Chrome (the Bevy Explorer CI job "Relay wake in the browser"), as a worker waits:
// the notify on the engine's send ends the worker's `Atomics.waitAsync` with "ok". A wait that
// expects the notify has a long timeout, so only a missing notify reaches it; the one for a scene
// the engine does not wake expects its timeout. Nothing here measures time.
#[cfg(all(test, target_arch = "wasm32"))]
mod browser {
    use std::task::Waker;

    use js_sys::{Atomics, Promise};
    use wasm_bindgen::{JsCast, JsValue};
    use wasm_bindgen_futures::JsFuture;
    use wasm_bindgen_test::{wasm_bindgen_test, wasm_bindgen_test_configure};

    use super::*;
    use crate::browser_test::{field, word_view};

    wasm_bindgen_test_configure!(run_in_dedicated_worker);

    // under the test runner's own 20 s limit, so a missing notify fails the assertion
    const WAIT_MS: f64 = 5_000.0;

    // a relay step as the worker takes it: decide from the engine's flag for the scene, load the
    // word, pump (when it waits, the empty channel registers the waker)
    fn step(
        wake: &Arc<RelayWake>,
        flag: &dcl_types::SceneWake,
        rx: &mut mpsc::UnboundedReceiver<u32>,
    ) -> i32 {
        wake.update_waits(SceneLoopMode::Both, flag.get());
        let seen = wake.word().load(Ordering::SeqCst);
        let waker = Waker::from(wake.clone());
        let mut cx = Context::from_waker(&waker);
        assert_eq!(next(rx, wake.waits(), &mut cx), None);
        seen
    }

    // the engine wakes the scene
    fn woken() -> dcl_types::SceneWake {
        let flag = dcl_types::SceneWake::default();
        flag.set(true);
        flag
    }

    #[wasm_bindgen_test]
    async fn the_engine_send_ends_the_workers_wait_with_ok() {
        let wake = RelayWake::new(Box::new(engine_notify));
        let (tx, mut rx) = mpsc::unbounded_channel();
        let seen = step(&wake, &woken(), &mut rx);

        let wait =
            Atomics::wait_async_with_timeout(&word_view(wake.word()), 0, seen, WAIT_MS).unwrap();
        assert_eq!(field(&wait, "async"), JsValue::TRUE);
        tx.send(7).unwrap();
        let outcome = JsFuture::from(field(&wait, "value").unchecked_into::<Promise>())
            .await
            .unwrap();
        assert_eq!(outcome.as_string().as_deref(), Some("ok"));
        assert_eq!(
            next(&mut rx, false, &mut Context::from_waker(Waker::noop())),
            Some(7)
        );
    }

    #[wasm_bindgen_test]
    fn a_send_between_the_load_and_the_wait_is_not_missed() {
        let wake = RelayWake::new(Box::new(engine_notify));
        let (tx, mut rx) = mpsc::unbounded_channel();
        let seen = step(&wake, &woken(), &mut rx);

        tx.send(7).unwrap();
        let wait =
            Atomics::wait_async_with_timeout(&word_view(wake.word()), 0, seen, WAIT_MS).unwrap();
        assert_eq!(field(&wait, "async"), JsValue::FALSE);
        assert_eq!(
            field(&wait, "value").as_string().as_deref(),
            Some("not-equal")
        );
    }

    /// A scene the engine does not wake registers no waker: its send leaves the word as it was,
    /// so a wait on it can only time out, and the relay finds the message on its poll.
    #[wasm_bindgen_test]
    async fn a_scene_the_engine_does_not_wake_is_not_notified() {
        let wake = RelayWake::new(Box::new(engine_notify));
        let (tx, mut rx) = mpsc::unbounded_channel();
        let seen = step(&wake, &dcl_types::SceneWake::default(), &mut rx);
        assert!(!wake.waits());

        tx.send(7).unwrap();
        let wait =
            Atomics::wait_async_with_timeout(&word_view(wake.word()), 0, seen, 50.0).unwrap();
        assert_eq!(field(&wait, "async"), JsValue::TRUE);
        let outcome = JsFuture::from(field(&wait, "value").unchecked_into::<Promise>())
            .await
            .unwrap();
        assert_eq!(outcome.as_string().as_deref(), Some("timed-out"));
        assert_eq!(
            next(&mut rx, false, &mut Context::from_waker(Waker::noop())),
            Some(7)
        );
    }
}
