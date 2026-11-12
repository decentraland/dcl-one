//! Waiting, in tests, for work that runs off the main thread.

use std::{
    fmt::Debug,
    time::{Duration, Instant},
};

use bevy::prelude::*;

/// How long a test waits for its assets before it fails.
const DEADLINE: Duration = Duration::from_secs(30);

/// Updates `app` until `pending` lists nothing, and panics naming what `pending` still listed
/// once `DEADLINE` has passed.
///
/// Assets decode on the IO task pool, which every test in the process shares, so how many frames
/// a load takes depends on what the other tests are doing: the wait is measured on the clock, and
/// the thread sleeps between frames so the pool gets to run.
pub(crate) fn update_until<T: Debug>(app: &mut App, mut pending: impl FnMut(&mut App) -> Vec<T>) {
    let deadline = Instant::now() + DEADLINE;
    loop {
        app.update();
        let pending = pending(app);
        if pending.is_empty() {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "still pending after {DEADLINE:?}: {pending:?}"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
}
