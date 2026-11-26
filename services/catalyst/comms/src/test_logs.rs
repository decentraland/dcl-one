//! Capture of this thread's `tracing` output for tests.
//!
//! `tracing` caches a callsite's interest when the callsite is first hit. While one dispatcher
//! alone is live, a first hit on another test's thread asks only that thread's subscriber (none)
//! and caches "never", hiding the event from every capture. A [`Capture`] keeps a second live
//! dispatcher beside its subscriber, so every first hit asks all of them.

use parking_lot::Mutex;
use std::sync::Arc;

/// A capture in force: this thread's subscriber, and a second live dispatcher beside it.
pub(crate) struct Capture {
    _default: tracing::subscriber::DefaultGuard,
    _second: tracing::Dispatch,
}

/// Installs `subscriber` as this thread's default until the returned capture drops.
pub(crate) fn capture(subscriber: impl tracing::Subscriber + Send + Sync + 'static) -> Capture {
    // Registered before the subscriber, so the subscriber's registration already sees two.
    let second = tracing::Dispatch::new(tracing::subscriber::NoSubscriber::default());
    Capture {
        _default: tracing::subscriber::set_default(subscriber),
        _second: second,
    }
}

/// This thread's formatted log lines, at or above a level, captured until the capture drops.
#[derive(Clone, Default)]
pub(crate) struct LogBuffer(Arc<Mutex<Vec<u8>>>);

impl std::io::Write for LogBuffer {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.lock().extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl LogBuffer {
    pub(crate) fn capture(level: tracing::Level) -> (Self, Capture) {
        let buffer = Self::default();
        let writer = buffer.clone();
        let capture = capture(
            tracing_subscriber::fmt()
                .with_max_level(level)
                .without_time()
                .with_ansi(false)
                .with_writer(move || writer.clone())
                .finish(),
        );
        (buffer, capture)
    }

    pub(crate) fn text(&self) -> String {
        String::from_utf8(self.0.lock().clone()).unwrap()
    }
}

mod tests {
    use super::LogBuffer;

    fn warn_once() {
        tracing::warn!("first hit elsewhere");
    }

    #[test]
    fn a_callsite_first_hit_on_another_thread_still_reaches_the_capture() {
        let (logs, _capture) = LogBuffer::capture(tracing::Level::WARN);
        std::thread::spawn(warn_once).join().unwrap();
        warn_once();
        assert!(
            logs.text().contains("first hit elsewhere"),
            "{}",
            logs.text()
        );
    }
}
