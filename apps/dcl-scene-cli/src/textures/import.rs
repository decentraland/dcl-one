//! `start`'s background texture import, like a game engine's asset import:
//! it runs at startup and after every watch batch, whether or not the project
//! enables optimized textures, so the cache is warm when it is turned on or a
//! deploy needs it. The preview never waits for it.
//!
//! One thread per project runs the import ([`Scheduler`]): a request while a
//! run is in progress queues exactly one more run. A run that finds its
//! inputs unchanged does nothing and says nothing. Otherwise it encodes what
//! the cache lacks on all cores but one, then swaps the finished overlay in
//! at once: until then the preview serves the previous overlay, never part of
//! the new one. No reload is sent: the status ([`ImportStatus`]) goes out
//! instead, and the client decides when to reload.

use super::{
    Encoder, Entry, PREVIEW_MANIFEST, Progress, RUN_LOCK, Run, STAGE, Settings, optimize,
    prune_models, registry, settings, stamps_of, swap, write_manifest,
};
use crate::ux;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, OnceLock, PoisonError};
use std::time::{Duration, Instant};

/// RUNNING updates go out at most this often.
pub const PROGRESS_EVERY: Duration = Duration::from_millis(500);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImportState {
    Running,
    Ready,
    Failed,
}

/// One status update: the `TextureOptimization` message without its scene.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ImportStatus {
    pub state: ImportState,
    /// Distinct textures processed in this run: encoded, cached or kept.
    pub done: u32,
    pub total: u32,
    /// Kept original because they could not be optimized.
    pub failed: u32,
    /// Source bytes of the converted images, and their KTX2 bytes.
    pub bytes_before: u64,
    pub bytes_after: u64,
    /// Whether the project serves and deploys the optimized files.
    pub enabled: bool,
}

fn count(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

/// Runs a job on its own thread whenever requested, one run at a time: a
/// request during a run queues one more run, however many arrive.
pub struct Scheduler {
    shared: Arc<Shared>,
}

struct Shared {
    state: Mutex<SchedState>,
    wake: Condvar,
}

#[derive(Default)]
struct SchedState {
    pending: bool,
    running: bool,
    closed: bool,
    runs: u64,
}

impl Shared {
    fn lock(&self) -> MutexGuard<'_, SchedState> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl Scheduler {
    pub fn spawn(name: &str, mut job: impl FnMut() + Send + 'static) -> std::io::Result<Self> {
        let shared = Arc::new(Shared {
            state: Mutex::new(SchedState::default()),
            wake: Condvar::new(),
        });
        let worker = shared.clone();
        std::thread::Builder::new()
            .name(name.to_string())
            .spawn(move || {
                loop {
                    {
                        let mut st = worker.lock();
                        while !st.pending && !st.closed {
                            st = worker.wake.wait(st).unwrap_or_else(PoisonError::into_inner);
                        }
                        if st.closed {
                            return;
                        }
                        st.pending = false;
                        st.running = true;
                    }
                    // a panicking run must not end the thread: later requests still run
                    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(&mut job));
                    let mut st = worker.lock();
                    st.running = false;
                    st.runs += 1;
                    worker.wake.notify_all();
                }
            })?;
        Ok(Scheduler { shared })
    }

    pub fn request(&self) {
        self.shared.lock().pending = true;
        self.shared.wake.notify_all();
    }

    /// Blocks until no run is queued or in progress; the runs so far.
    #[cfg(test)]
    pub fn wait_idle(&self) -> u64 {
        let mut st = self.shared.lock();
        while st.pending || st.running {
            st = self
                .shared
                .wake
                .wait(st)
                .unwrap_or_else(PoisonError::into_inner);
        }
        st.runs
    }
}

impl Drop for Scheduler {
    fn drop(&mut self) {
        self.shared.lock().closed = true;
        self.shared.wake.notify_all();
    }
}

/// Lets an update through when `every` has passed since the last one let
/// through; the first always passes.
pub struct Throttle {
    every: Duration,
    last: Mutex<Option<Instant>>,
}

impl Throttle {
    pub fn new(every: Duration) -> Self {
        Throttle {
            every,
            last: Mutex::new(None),
        }
    }

    pub fn ready_at(&self, now: Instant) -> bool {
        let mut last = self.last.lock().unwrap_or_else(PoisonError::into_inner);
        match *last {
            Some(at) if now.saturating_duration_since(at) < self.every => false,
            _ => {
                *last = Some(now);
                true
            }
        }
    }
}

struct Importer {
    scheduler: Scheduler,
    /// The last update sent, for a client that connects later.
    last: Arc<Mutex<Option<ImportStatus>>>,
}

fn importers() -> MutexGuard<'static, HashMap<PathBuf, Importer>> {
    static IMPORTERS: OnceLock<Mutex<HashMap<PathBuf, Importer>>> = OnceLock::new();
    IMPORTERS
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
}

/// Starts `root`'s background import and queues its first run. `emit`
/// receives every status update (from the import thread); it is how the
/// preview tells its clients.
pub fn start(
    root: &Path,
    emit: impl Fn(&ImportStatus) + Send + Sync + 'static,
) -> std::io::Result<()> {
    let last: Arc<Mutex<Option<ImportStatus>>> = Arc::default();
    let job = {
        let root = root.to_path_buf();
        let last = last.clone();
        let mut first = true;
        move || {
            let send = |status: ImportStatus| {
                *last.lock().unwrap_or_else(PoisonError::into_inner) = Some(status);
                emit(&status);
            };
            let workers = background_workers();
            let encoder = || super::basisu::WasmBasisu::shared().map(|e| e as &dyn Encoder);
            let ran = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                import_once(&root, &encoder, workers, &send, first)
            }));
            if ran.is_err() {
                ux::note_absent(
                    "the texture import stopped unexpectedly; the preview keeps the last optimized files",
                );
                send(ImportStatus {
                    state: ImportState::Failed,
                    done: 0,
                    total: 0,
                    failed: 0,
                    bytes_before: 0,
                    bytes_after: 0,
                    enabled: settings::load(&root).optimize_textures,
                });
            }
            first = false;
        }
    };
    let scheduler = Scheduler::spawn("texture-import", job)?;
    scheduler.request();
    importers().insert(root.to_path_buf(), Importer { scheduler, last });
    Ok(())
}

/// Queues a run of `root`'s background import; nothing when `start` runs
/// none (a plain `deploy`).
pub fn request(root: &Path) {
    if let Some(importer) = importers().get(root) {
        importer.scheduler.request();
    }
}

/// The last update `root`'s import sent: what a client connecting later is
/// told first.
pub fn last_status(root: &Path) -> Option<ImportStatus> {
    let importers = importers();
    let last = importers.get(root)?.last.clone();
    drop(importers);
    *last.lock().unwrap_or_else(PoisonError::into_inner)
}

/// All cores but one: the preview, the bundler and the explorer keep one.
pub fn background_workers() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get().saturating_sub(1).max(1))
        .unwrap_or(1)
}

/// One background run; `true` when it changed what clients see. Inputs
/// unchanged since the last run (files, setting): nothing is done or sent.
/// Otherwise RUNNING goes out while textures are encoded (throttled), then
/// READY once the new overlay is in place — when it differs from the last,
/// or when RUNNING went out — or FAILED.
pub(crate) fn import_once<'e>(
    root: &Path,
    encoder: &dyn Fn() -> anyhow::Result<&'e dyn Encoder>,
    workers: usize,
    send: &(dyn Fn(ImportStatus) + Sync),
    verbose: bool,
) -> bool {
    let _turn = RUN_LOCK.blocking_lock();
    let project = settings::load(root);
    let enabled = project.optimize_textures;
    let settings = Settings::for_project(project);
    let failed = |e: anyhow::Error| {
        if enabled {
            ux::report_watch(
                &e.context("optimizing textures; the preview keeps the last optimized files"),
            );
        } else {
            tracing::warn!("background texture import failed: {e:#}");
        }
        send(ImportStatus {
            state: ImportState::Failed,
            done: 0,
            total: 0,
            failed: 0,
            bytes_before: 0,
            bytes_after: 0,
            enabled,
        });
        false
    };
    let rels = match crate::deploy::collect_publishable_files(root) {
        Ok(rels) => rels,
        Err(e) => return failed(e),
    };
    let stamps = stamps_of(root, &rels);
    let unchanged = registry().get(root).is_some_and(|e| {
        e.enabled == enabled
            && e.settings == settings
            && e.stamps.as_ref() == Some(&stamps)
            && e.overlay
                .as_ref()
                .is_some_and(|o| o.sources.values().all(|staged| staged.is_file()))
    });
    if unchanged {
        return false;
    }
    let encoder = match encoder() {
        Ok(encoder) => encoder,
        Err(e) => return failed(e),
    };
    let throttle = Throttle::new(PROGRESS_EVERY);
    let running = std::sync::atomic::AtomicBool::new(false);
    let progress = |p: Progress| {
        if throttle.ready_at(Instant::now()) {
            running.store(true, std::sync::atomic::Ordering::Relaxed);
            send(ImportStatus {
                state: ImportState::Running,
                done: count(p.done),
                total: count(p.total),
                failed: count(p.failed),
                bytes_before: 0,
                bytes_after: 0,
                enabled,
            });
        }
    };
    let outcome = optimize(
        root,
        &rels,
        settings,
        encoder,
        &|rel| crate::deploy::tree_payload_path(root, rel),
        &Run {
            workers,
            progress: &progress,
        },
    );
    let outcome = match outcome {
        Ok(outcome) => outcome,
        Err(e) => return failed(e),
    };
    let overlay = Arc::new(outcome.overlay);
    let changed = swap(
        root,
        Entry {
            enabled,
            settings,
            overlay: Some(overlay.clone()),
            stamps: Some(stamps),
        },
    );
    prune_models(&root.join(STAGE), &overlay);
    if let Err(e) = write_manifest(root, PREVIEW_MANIFEST, &outcome.manifest) {
        tracing::warn!("{e:#}");
    }
    let summary = &outcome.summary;
    if changed && enabled {
        summary.report(!verbose, PREVIEW_MANIFEST);
        if !summary.ignored.is_empty() {
            ux::note(format!(
                "{} original image(s) left out: only models used them (--keep-source-textures keeps them)",
                summary.ignored.len()
            ));
        }
    }
    if changed || running.load(std::sync::atomic::Ordering::Relaxed) {
        send(ImportStatus {
            state: ImportState::Ready,
            done: count(summary.total),
            total: count(summary.total),
            failed: count(summary.failed),
            bytes_before: summary.converted_bytes,
            bytes_after: summary.ktx2_bytes,
            enabled,
        });
    }
    changed
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    #[test]
    fn requests_during_a_run_coalesce_into_one_more_run() {
        let (started_tx, started_rx) = mpsc::channel::<()>();
        let (gate_tx, gate_rx) = mpsc::channel::<()>();
        let s = Scheduler::spawn("coalesce-test", move || {
            started_tx.send(()).unwrap();
            gate_rx.recv().unwrap();
        })
        .unwrap();
        s.request();
        started_rx.recv().unwrap();
        // the first run is in progress: three requests queue one run
        s.request();
        s.request();
        s.request();
        gate_tx.send(()).unwrap();
        started_rx.recv().unwrap();
        gate_tx.send(()).unwrap();
        assert_eq!(s.wait_idle(), 2);
        assert!(started_rx.try_recv().is_err(), "no third run");

        // an idle scheduler runs a request once
        s.request();
        started_rx.recv().unwrap();
        gate_tx.send(()).unwrap();
        assert_eq!(s.wait_idle(), 3);
    }

    #[test]
    fn a_panicking_run_does_not_stop_later_runs() {
        let (tx, rx) = mpsc::channel::<u32>();
        let mut n = 0;
        let s = Scheduler::spawn("panic-test", move || {
            n += 1;
            tx.send(n).unwrap();
            if n == 1 {
                panic!("first run fails");
            }
        })
        .unwrap();
        s.request();
        assert_eq!(rx.recv().unwrap(), 1);
        assert_eq!(s.wait_idle(), 1);
        s.request();
        assert_eq!(rx.recv().unwrap(), 2);
    }

    #[test]
    fn the_throttle_lets_one_update_through_per_period() {
        let t = Throttle::new(Duration::from_millis(500));
        let at = Instant::now();
        assert!(t.ready_at(at));
        assert!(!t.ready_at(at + Duration::from_millis(100)));
        assert!(!t.ready_at(at + Duration::from_millis(499)));
        assert!(t.ready_at(at + Duration::from_millis(500)));
        assert!(!t.ready_at(at + Duration::from_millis(900)));
        assert!(t.ready_at(at + Duration::from_millis(1000)));
    }

    #[test]
    fn background_workers_leave_a_core() {
        let cores = std::thread::available_parallelism().map_or(1, |n| n.get());
        assert_eq!(background_workers(), cores.saturating_sub(1).max(1));
    }

    #[test]
    fn requesting_an_unknown_project_is_a_no_op() {
        let root = Path::new("/nowhere/importing");
        request(root);
        assert_eq!(last_status(root), None);
    }
}
