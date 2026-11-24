//! How the engine's scene loop (scene_runner `run_scene_loop`) waits for scene replies on the
//! web. A sandbox worker's relay counts each message it forwards to the engine
//! ([`SceneReplies::send`], from `inner::relay`) and wakes the loop if it waits for one
//! ([`SceneReplies::wait`]). A reply then ends the wait as soon as it arrives, not when a timed
//! wait wakes: a browser's timed wait can end a millisecond or more late. The count and its
//! condition variable live in engine memory, which the relays share.
//! Outside the wasm-only module so the host tests cover the wait.

use std::{
    sync::{Condvar, Mutex, PoisonError},
    time::Duration,
};

#[cfg(any(target_arch = "wasm32", test))]
use dcl_types::{SceneResponse, SceneResponseSender};

pub struct SceneReplies {
    count: Mutex<u32>,
    arrived: Condvar,
}

/// The replies relays forwarded to the engine.
pub static REPLIES: SceneReplies = SceneReplies::new();

impl SceneReplies {
    const fn new() -> Self {
        Self {
            count: Mutex::new(0),
            arrived: Condvar::new(),
        }
    }

    /// Relay: forwards `response` to the engine and, once it is sent, counts it and wakes the
    /// loop. False if the engine's channel refused it.
    #[cfg(any(target_arch = "wasm32", test))]
    pub(crate) fn send(&self, sender: &SceneResponseSender, response: SceneResponse) -> bool {
        let sent = sender.try_send(response).is_ok();
        if sent {
            self.replied();
        }
        sent
    }

    // a message reached the engine
    #[cfg(any(target_arch = "wasm32", test))]
    fn replied(&self) {
        let mut count = self.count.lock().unwrap_or_else(PoisonError::into_inner);
        *count = count.wrapping_add(1);
        self.arrived.notify_all();
    }

    /// Engine: the count before a pass drains the replies, for the wait after it.
    pub fn seen(&self) -> u32 {
        *self.count.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Engine: waits until a reply after `seen`, or for `timeout`; returns at once if one came
    /// since. True when the wait ended by its timeout rather than by a reply's wake.
    ///
    /// Reads no clock: std has none on wasm32-unknown-unknown (`Instant::now` panics there), so
    /// not `Condvar::wait_timeout_while`, which reads one for the time left. Each
    /// `Condvar::wait_timeout` is a single futex wait (`Atomics.wait` on the web) with the whole
    /// timeout; a wake without a reply (spurious; on the web only a notify wakes it, and only a
    /// reply notifies) waits the whole timeout again.
    pub fn wait(&self, seen: u32, timeout: Duration) -> bool {
        let mut count = self.count.lock().unwrap_or_else(PoisonError::into_inner);
        while *count == seen {
            let (next, result) = self
                .arrived
                .wait_timeout(count, timeout)
                .unwrap_or_else(PoisonError::into_inner);
            count = next;
            if result.timed_out() {
                return true;
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use std::{sync::Arc, thread, time::Instant};

    use bevy::prelude::Entity;
    use dcl_types::SceneId;

    use super::*;

    // long enough that only a missing wake reaches it
    const LONG: Duration = Duration::from_secs(10);

    #[test]
    fn a_reply_ends_the_wait() {
        let replies = Arc::new(SceneReplies::new());
        let seen = replies.seen();
        let relay = replies.clone();
        let sender = thread::spawn(move || {
            thread::sleep(Duration::from_millis(20));
            relay.replied();
        });
        let start = Instant::now();
        assert!(!replies.wait(seen, LONG));
        assert!(start.elapsed() < LONG);
        sender.join().unwrap();
    }

    #[test]
    fn a_reply_since_the_count_was_read_returns_at_once() {
        let replies = SceneReplies::new();
        let seen = replies.seen();
        replies.replied();
        assert!(!replies.wait(seen, LONG));
        // the next pass reads the count again
        assert_eq!(replies.seen(), seen.wrapping_add(1));
    }

    #[test]
    fn without_a_reply_the_wait_times_out() {
        let replies = SceneReplies::new();
        assert!(replies.wait(replies.seen(), Duration::from_millis(1)));
    }

    /// What the relay forwards (`Relay::forward`, `Relay::end`) and the engine's channel takes ends
    /// the loop's wait; a message the channel refuses changes no count.
    #[test]
    fn a_forwarded_message_ends_the_wait() {
        let (channel, _receiver) = dcl_types::scene_response_channel();
        let sender = SceneResponseSender::new(SceneId(Entity::PLACEHOLDER), channel);
        let replies = Arc::new(SceneReplies::new());
        let seen = replies.seen();
        let relay = replies.clone();
        let forwarded = thread::spawn(move || {
            thread::sleep(Duration::from_millis(20));
            relay.send(&sender, SceneResponse::Error("tick".to_owned()))
        });
        assert!(!replies.wait(seen, LONG));
        assert!(forwarded.join().unwrap());

        let (channel, receiver) = dcl_types::scene_response_channel();
        let sender = SceneResponseSender::new(SceneId(Entity::PLACEHOLDER), channel);
        drop(receiver);
        let seen = replies.seen();
        assert!(!replies.send(&sender, SceneResponse::Error("tick".to_owned())));
        assert_eq!(replies.seen(), seen);
    }
}

// In headless Chrome (the Bevy Explorer CI job "Relay wake in the browser"), on the engine
// worker's terms: the wait blocks the thread, and a relay on another wasm thread, in its own
// worker over the shared memory, ends it. std has no clock on wasm32-unknown-unknown, so a wait
// that reads one panics here, as it would in the engine. The waits' bound is long, so only a
// missing wake reaches it; nothing here measures time.
#[cfg(all(test, target_arch = "wasm32"))]
mod browser {
    use std::sync::atomic::{AtomicI32, Ordering};

    use bevy::web_worker::{spawn_worker, WorkerSpec};
    use js_sys::{Atomics, Promise};
    use wasm_bindgen::{prelude::wasm_bindgen, JsCast, JsValue};
    use wasm_bindgen_futures::JsFuture;
    use wasm_bindgen_test::{wasm_bindgen_test, wasm_bindgen_test_configure};

    use super::*;
    use crate::browser_test::{field, word_view};

    wasm_bindgen_test_configure!(run_in_dedicated_worker);

    // under the test runner's own 20 s limit, so a missing wake fails the assertion
    const LONG: Duration = Duration::from_secs(5);

    // the relay thread's count, and the word it sets once it runs
    static RELAYED: SceneReplies = SceneReplies::new();
    static RELAY_RUNS: AtomicI32 = AtomicI32::new(0);

    /// The relay thread: says it runs, then replies 100 ms later, while the test thread waits.
    #[wasm_bindgen]
    pub fn scene_replies_test_relay() {
        RELAY_RUNS.store(1, Ordering::SeqCst);
        let _ = Atomics::notify(&word_view(&RELAY_RUNS), 0);
        std::thread::sleep(Duration::from_millis(100));
        RELAYED.replied();
    }

    // the glue the test runner serves beside this worker's script, for the relay's worker
    fn glue_url() -> String {
        let href = field(&field(&js_sys::global(), "location"), "href")
            .as_string()
            .unwrap();
        let base = href.split(['?', '#']).next().unwrap();
        format!("{}wasm-bindgen-test.js", &base[..=base.rfind('/').unwrap()])
    }

    #[wasm_bindgen_test]
    async fn a_reply_from_another_thread_ends_the_wait() {
        let seen = RELAYED.seen();
        spawn_worker(
            &glue_url(),
            &WorkerSpec {
                entry: "scene_replies_test_relay".into(),
                args: Vec::new(),
                tag: "relay".into(),
                stack_size: None,
                transfer: Vec::new(),
            },
        )
        .unwrap();
        // the relay's worker starts while this one is free
        let runs =
            Atomics::wait_async_with_timeout(&word_view(&RELAY_RUNS), 0, 0, 5_000.0).unwrap();
        if field(&runs, "async") == JsValue::TRUE {
            let outcome = JsFuture::from(field(&runs, "value").unchecked_into::<Promise>())
                .await
                .unwrap();
            assert_eq!(outcome.as_string().as_deref(), Some("ok"));
        }

        assert!(!RELAYED.wait(seen, LONG), "the reply did not end the wait");
        assert_eq!(RELAYED.seen(), seen.wrapping_add(1));
    }

    #[wasm_bindgen_test]
    fn a_reply_since_the_count_was_read_ends_the_wait_at_once() {
        let replies = SceneReplies::new();
        let seen = replies.seen();
        replies.replied();
        assert!(!replies.wait(seen, LONG));
    }

    #[wasm_bindgen_test]
    fn without_a_reply_the_wait_times_out() {
        let replies = SceneReplies::new();
        assert!(replies.wait(replies.seen(), Duration::from_millis(1)));
    }
}
