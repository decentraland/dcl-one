//! Activity of ordinary engine transfers. IPFS and scene transfers have their own scheduling.

#[cfg(not(target_arch = "wasm32"))]
mod native {
    use std::{
        collections::HashMap,
        sync::{LazyLock, Mutex, MutexGuard},
        time::{Duration, Instant},
    };

    const PRESSURE_THRESHOLD: usize = 15;
    const NO_COMPLETIONS: Duration = Duration::from_secs(5);

    #[derive(Debug, serde::Serialize)]
    #[serde(rename_all = "camelCase")]
    pub struct CoreHttpSnapshot {
        pub active: usize,
        pub high_water: usize,
        pub oldest_active_ms: u64,
        pub suspected_starvation: bool,
    }

    #[derive(Default)]
    pub(super) struct Pressure {
        starts: HashMap<u64, Instant>,
        next_id: u64,
        high_water: usize,
        no_completion_since: Option<Instant>,
    }

    impl Pressure {
        pub(super) fn begin(&mut self, now: Instant) -> u64 {
            let id = self.next_id;
            self.next_id = self.next_id.wrapping_add(1);
            self.starts.insert(id, now);
            let active = self.starts.len();
            self.high_water = self.high_water.max(active);
            if active == PRESSURE_THRESHOLD {
                self.no_completion_since = Some(now);
            }
            id
        }

        pub(super) fn end(&mut self, id: u64, now: Instant) {
            self.starts.remove(&id);
            self.no_completion_since = (self.starts.len() >= PRESSURE_THRESHOLD).then_some(now);
        }

        pub(super) fn snapshot(&self, now: Instant) -> CoreHttpSnapshot {
            CoreHttpSnapshot {
                active: self.starts.len(),
                high_water: self.high_water,
                oldest_active_ms: self
                    .starts
                    .values()
                    .min()
                    .map_or(0, |oldest| now.duration_since(*oldest).as_millis() as u64),
                suspected_starvation: self
                    .no_completion_since
                    .is_some_and(|since| now.duration_since(since) >= NO_COMPLETIONS),
            }
        }
    }

    fn pressure() -> MutexGuard<'static, Pressure> {
        static PRESSURE: LazyLock<Mutex<Pressure>> = LazyLock::new(Mutex::default);
        PRESSURE.lock().unwrap_or_else(|err| err.into_inner())
    }

    /// Process-wide activity since startup, sampled against the monotonic clock.
    pub fn core_http_snapshot() -> CoreHttpSnapshot {
        pressure().snapshot(Instant::now())
    }

    pub(super) struct Activity(u64);

    impl Activity {
        pub(super) fn begin() -> Self {
            Self(pressure().begin(Instant::now()))
        }
    }

    impl Drop for Activity {
        fn drop(&mut self) {
            pressure().end(self.0, Instant::now());
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
use native::Activity;
#[cfg(not(target_arch = "wasm32"))]
pub use native::{core_http_snapshot, CoreHttpSnapshot};

#[cfg(target_arch = "wasm32")]
mod browser {
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::{marker::PhantomData, rc::Rc};
    use wasm_bindgen::prelude::*;

    #[wasm_bindgen(raw_module = "../core_http.js")]
    extern "C" {
        #[wasm_bindgen(catch, js_name = beginCoreHttp)]
        fn begin_core_http(id: u32) -> Result<(), JsValue>;
        #[wasm_bindgen(catch, js_name = endCoreHttp)]
        fn end_core_http(id: u32) -> Result<(), JsValue>;
    }

    // Begin and end report through the current worker's JS module.
    pub(super) struct Activity(u32, PhantomData<Rc<()>>);

    impl Activity {
        pub(super) fn begin() -> Self {
            static NEXT_ID: AtomicU32 = AtomicU32::new(0);
            let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
            let _ = begin_core_http(id);
            Self(id, PhantomData)
        }
    }

    impl Drop for Activity {
        fn drop(&mut self) {
            let _ = end_core_http(self.0);
        }
    }
}

#[cfg(target_arch = "wasm32")]
use browser::Activity;

/// Observe one Explorer-owned transfer from dispatch until the body is read or abandoned.
/// Dropping the guard on error or cancellation records completion. Never hold it across retries.
#[must_use = "keep the guard alive for the duration of the transfer"]
pub fn begin() -> impl Drop {
    Activity::begin()
}

#[cfg(all(test, not(target_arch = "wasm32")))]
#[path = "http_tests.rs"]
mod tests;
