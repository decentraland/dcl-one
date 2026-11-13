//! A child `dcl-scene-cli` under test: its piped streams read while it runs,
//! a bounded wait for its exit, and a SIGTERM-first stop.
#![allow(dead_code)]

use std::io::Read;
use std::process::{Child, ExitStatus};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

/// A child's piped stream, drained by a thread so it is readable *while* the
/// child is still running and can never fill its pipe and deadlock.
pub struct Tail {
    seen: Arc<(Mutex<Seen>, Condvar)>,
    reader: Option<std::thread::JoinHandle<()>>,
}

/// What a [`Tail`] has read so far, and whether the stream has ended.
#[derive(Default)]
pub struct Seen {
    pub bytes: Vec<u8>,
    pub ended: bool,
}

impl Tail {
    pub fn spawn(mut src: impl Read + Send + 'static) -> Self {
        let seen = Arc::new((Mutex::new(Seen::default()), Condvar::new()));
        let sink = Arc::clone(&seen);
        let reader = std::thread::spawn(move || {
            let (lock, grew) = &*sink;
            let mut chunk = [0u8; 4096];
            loop {
                let n = src.read(&mut chunk).unwrap_or(0);
                let mut seen = lock.lock().unwrap_or_else(PoisonError::into_inner);
                seen.bytes.extend_from_slice(&chunk[..n]);
                seen.ended = n == 0;
                grew.notify_all();
                if seen.ended {
                    break;
                }
            }
        });
        Tail {
            seen,
            reader: Some(reader),
        }
    }

    pub fn seen(&self) -> MutexGuard<'_, Seen> {
        self.seen.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.seen().bytes).into_owned()
    }

    /// Blocks until the stream says `needle` (true) or ends without it
    /// (false); `guard` only bounds a stream that does neither.
    pub fn wait_for(&self, needle: &str, guard: Duration) -> bool {
        let (lock, grew) = &*self.seen;
        let seen = lock.lock().unwrap_or_else(PoisonError::into_inner);
        let (seen, _) = grew
            .wait_timeout_while(seen, guard, |seen| {
                !seen.ended && !String::from_utf8_lossy(&seen.bytes).contains(needle)
            })
            .unwrap_or_else(PoisonError::into_inner);
        String::from_utf8_lossy(&seen.bytes).contains(needle)
    }

    /// Everything, after the writer end is gone. Only valid once the child has
    /// exited; before that use [`Tail::text`].
    pub fn finish(&mut self) -> Vec<u8> {
        if let Some(r) = self.reader.take() {
            let _ = r.join();
        }
        self.seen().bytes.clone()
    }
}

pub fn wait_bounded(child: &mut Child, limit: Duration) -> Option<ExitStatus> {
    super::wait::until_blocking(limit, || {
        child.try_wait().expect("waiting on the CLI child")
    })
}

/// SIGTERM first, SIGKILL only as a backstop: `start` tears its abgen sidecar
/// down on SIGTERM (`asset_bundles::kill_sidecar_group`), and a SIGKILLed
/// preview server runs no such handler. The sidecar sits in its own process
/// group, so it would be orphaned onto the developer's machine holding a port
/// and a pile of worker threads, once per run of this file.
pub fn terminate(child: &mut Child) -> ExitStatus {
    #[cfg(unix)]
    unsafe {
        libc::kill(child.id() as libc::pid_t, libc::SIGTERM);
    }
    if let Some(status) = wait_bounded(child, Duration::from_secs(15)) {
        return status;
    }
    let _ = child.kill();
    child.wait().expect("reaping the CLI child")
}
