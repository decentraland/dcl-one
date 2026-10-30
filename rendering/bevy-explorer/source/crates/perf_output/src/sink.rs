//! Where the log's bytes go: the one writer's destinations. [`LogSink`] takes whole events, so a
//! sink never sees half a line or an exception block split by another thread's line.

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Mutex, PoisonError,
};

/// A destination for the log. `write` takes one event's bytes (one entry line, or the three
/// parts of an exception) and must keep them together.
pub trait LogSink: Send + Sync + 'static {
    fn write(&self, bytes: &[u8]);

    /// Whether anything written is kept; the layer builds no line for a sink that keeps nothing.
    fn keeps(&self) -> bool {
        true
    }
}

impl<T: LogSink> LogSink for &'static T {
    fn write(&self, bytes: &[u8]) {
        (*self).write(bytes)
    }

    fn keeps(&self) -> bool {
        (*self).keeps()
    }
}

/// A closure as a sink.
pub struct FnSink<F>(pub F);

impl<F: Fn(&[u8]) + Send + Sync + 'static> LogSink for FnSink<F> {
    fn write(&self, bytes: &[u8]) {
        (self.0)(bytes)
    }
}

/// Desktop: the file at `--log-file`. Each event is one `write` to the file with no buffer in
/// between, so a panic, a crash or a kill leaves everything logged before it, and the exit
/// marker is on disk before the process goes.
#[cfg(not(target_arch = "wasm32"))]
pub struct FileSink(Mutex<std::fs::File>);

#[cfg(not(target_arch = "wasm32"))]
impl FileSink {
    /// Creates the file, or empties it.
    pub fn create(path: &std::path::Path) -> std::io::Result<Self> {
        std::fs::File::create(path).map(|file| Self(Mutex::new(file)))
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl LogSink for FileSink {
    fn write(&self, bytes: &[u8]) {
        use std::io::Write;
        // a failing log must not fail the app, and there is nowhere to report it
        let _ = self
            .0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .write_all(bytes);
    }
}

/// The destination of a process's log, for a layer built before the destination is known. The
/// desktop installs the layer when the logger starts and learns `--log-file` only after parsing
/// the command line; what is logged in between is held here and written first.
pub struct DeferredSink {
    closed: AtomicBool,
    state: Mutex<Deferred>,
}

enum Deferred {
    Pending(Vec<u8>),
    Open(Box<dyn LogSink>),
    Off,
}

impl DeferredSink {
    pub const fn new() -> Self {
        Self {
            closed: AtomicBool::new(false),
            state: Mutex::new(Deferred::Pending(Vec::new())),
        }
    }

    /// The destination is known: it gets what was held, then every later event.
    pub fn open(&self, sink: impl LogSink) {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        if let Deferred::Pending(held) = &*state {
            if !held.is_empty() {
                sink.write(held);
            }
        }
        *state = Deferred::Open(Box::new(sink));
        self.closed.store(false, Ordering::Relaxed);
    }

    /// There is no destination: what was held is dropped, and later events are not kept.
    pub fn close(&self) {
        self.closed.store(true, Ordering::Relaxed);
        *self.state.lock().unwrap_or_else(PoisonError::into_inner) = Deferred::Off;
    }
}

impl Default for DeferredSink {
    fn default() -> Self {
        Self::new()
    }
}

impl LogSink for DeferredSink {
    fn write(&self, bytes: &[u8]) {
        match &mut *self.state.lock().unwrap_or_else(PoisonError::into_inner) {
            Deferred::Pending(held) => held.extend_from_slice(bytes),
            Deferred::Open(sink) => sink.write(bytes),
            Deferred::Off => {}
        }
    }

    fn keeps(&self) -> bool {
        !self.closed.load(Ordering::Relaxed)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;

    fn capture() -> (Arc<Mutex<Vec<u8>>>, impl LogSink) {
        let captured = Arc::new(Mutex::new(Vec::new()));
        let sink = {
            let captured = captured.clone();
            FnSink(move |bytes: &[u8]| captured.lock().unwrap().extend_from_slice(bytes))
        };
        (captured, sink)
    }

    #[test]
    fn deferred_sink_writes_what_it_held_before_what_follows() {
        let deferred = DeferredSink::new();
        let (captured, sink) = capture();
        deferred.write(
            b"early
",
        );
        deferred.open(sink);
        deferred.write(
            b"late
",
        );
        assert_eq!(
            &*captured.lock().unwrap(),
            b"early
late
"
        );
    }

    #[test]
    fn closed_deferred_sink_drops_what_it_held_and_keeps_nothing() {
        let deferred = DeferredSink::new();
        deferred.write(
            b"early
",
        );
        deferred.close();
        deferred.write(
            b"late
",
        );
        let (captured, sink) = capture();
        deferred.open(sink);
        deferred.write(
            b"after
",
        );
        assert_eq!(
            &*captured.lock().unwrap(),
            b"after
"
        );
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn file_sink_leaves_every_event_on_disk_without_a_flush() {
        let dir = std::env::temp_dir().join(format!("perf_output_sink_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("session.log");
        let sink = FileSink::create(&path).unwrap();
        sink.write(
            b"first
",
        );
        sink.write(
            b"second
",
        );
        assert_eq!(
            std::fs::read(&path).unwrap(),
            b"first
second
"
        );
        drop(sink);
        // creating it again empties it
        drop(FileSink::create(&path).unwrap());
        assert_eq!(std::fs::read(&path).unwrap(), b"");
        std::fs::remove_dir_all(dir).unwrap();
    }
}
