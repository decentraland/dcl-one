//! The web sink: the engine's CSV and log bytes, queued for the page's sender
//! (`deploy/web/engine/perf_stream.js`), which posts them to the harness with the offset
//! protocol. The engine never sends a request itself: a trapped engine (a panic) can no longer
//! run Rust, and the page can still finish the run.
//!
//! Any thread may queue (a worker's log line, the page's own); queueing never blocks, which the
//! page's thread could not survive. The engine's frame drains the queues and hands the text to
//! the page (`src/web.rs`). Built for every target, so the queues are tested on the desktop.

use std::{
    io,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{channel, Receiver, Sender},
        Mutex, OnceLock, PoisonError, TryLockError,
    },
};

use crate::sink::LogSink;

/// The two byte streams a run sends; the names are the paths' last part (`P/csv`, `P/log`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Stream {
    Csv,
    Log,
}

impl Stream {
    pub fn name(self) -> &'static str {
        match self {
            Stream::Csv => "csv",
            Stream::Log => "log",
        }
    }

    fn queue(self) -> Option<&'static Queue> {
        match self {
            Stream::Csv => CSV.get(),
            Stream::Log => LOG.get(),
        }
    }
}

struct Queue {
    sender: Sender<Vec<u8>>,
    receiver: Mutex<Receiver<Vec<u8>>>,
}

impl Queue {
    fn new() -> Self {
        let (sender, receiver) = channel();
        Self {
            sender,
            receiver: Mutex::new(receiver),
        }
    }
}

static LOG: OnceLock<Queue> = OnceLock::new();
static CSV: OnceLock<Queue> = OnceLock::new();

/// The page asked for a stream (`perfStream`): from here the log is queued, and so is the CSV
/// when `csv` (`perfCsv`) is set. Call before the logger starts.
pub fn start(csv: bool) {
    LOG.get_or_init(Queue::new);
    if csv {
        CSV.get_or_init(Queue::new);
    }
}

/// Whether [`start`] was called; the log layer is installed only then.
pub fn started() -> bool {
    LOG.get().is_some()
}

/// The log's queue, for the layer; `None` before [`start`].
pub fn log_sink() -> Option<impl LogSink> {
    LOG.get().map(|queue| QueueSink(queue.sender.clone()))
}

/// The CSV's queue, as a writer; `None` unless [`start`] was called with `csv`.
pub fn csv_writer() -> Option<impl io::Write + Send + Sync> {
    CSV.get().map(|queue| QueueSink(queue.sender.clone()))
}

struct QueueSink(Sender<Vec<u8>>);

impl LogSink for QueueSink {
    fn write(&self, bytes: &[u8]) {
        // the receiver lives as long as the process
        let _ = self.0.send(bytes.to_vec());
    }
}

impl io::Write for QueueSink {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        LogSink::write(&*self, bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// Everything queued on `stream` since the last call, as text, or `None` when nothing is. Every
/// writer queues whole events (or ASCII), so the text is whole UTF-8.
pub fn drain(stream: Stream) -> Option<String> {
    let queue = stream.queue()?;
    let receiver = queue
        .receiver
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    take(&receiver)
}

/// [`drain`] for a thread that must not wait: `None` also when another thread is draining, or this
/// one was. The web's panic hook calls it on the panicking thread, which may have been draining
/// itself, and the page when the engine stopped without handing over what it queued (the page's
/// thread cannot block).
pub fn try_drain(stream: Stream) -> Option<String> {
    let queue = stream.queue()?;
    let receiver = match queue.receiver.try_lock() {
        Ok(receiver) => receiver,
        Err(TryLockError::Poisoned(poisoned)) => poisoned.into_inner(),
        Err(TryLockError::WouldBlock) => return None,
    };
    take(&receiver)
}

fn take(receiver: &Receiver<Vec<u8>>) -> Option<String> {
    let mut bytes = Vec::new();
    while let Ok(chunk) = receiver.try_recv() {
        bytes.extend_from_slice(&chunk);
    }
    (!bytes.is_empty()).then(|| String::from_utf8_lossy(&bytes).into_owned())
}

static ENDED: AtomicBool = AtomicBool::new(false);

/// Records that the page is being told the run is over; true for the first caller only, so the
/// page hears it once, whichever of the engine's exits gets there first.
pub fn mark_ended() -> bool {
    !ENDED.swap(true, Ordering::SeqCst)
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;

    // the queues are process-wide: one test owns them
    #[test]
    fn the_queues_carry_what_was_written_in_order_and_empty_when_drained() {
        assert!(!started());
        assert!(log_sink().is_none());
        assert_eq!(drain(Stream::Log), None);

        start(false);
        assert!(started());
        assert!(csv_writer().is_none(), "no CSV without perfCsv");
        assert_eq!(drain(Stream::Csv), None);

        let log = log_sink().unwrap();
        log.write("first é\n".as_bytes());
        log.write(b"second\n");
        assert_eq!(drain(Stream::Log).as_deref(), Some("first é\nsecond\n"));
        assert_eq!(drain(Stream::Log), None);

        start(true);
        let mut csv = csv_writer().unwrap();
        csv.write_all(b"1,16.5,0\r\n").unwrap();
        csv.flush().unwrap();
        assert_eq!(drain(Stream::Csv).as_deref(), Some("1,16.5,0\r\n"));

        // from another thread, as a worker's log line is
        std::thread::spawn(move || log.write(b"worker\n"))
            .join()
            .unwrap();
        assert_eq!(drain(Stream::Log).as_deref(), Some("worker\n"));

        // what is queued is taken without waiting, by the page after a trap, once
        let log = log_sink().unwrap();
        log.write(b"last line\n");
        csv.write_all(b"2,16.5,0\r\n").unwrap();
        assert_eq!(try_drain(Stream::Log).as_deref(), Some("last line\n"));
        assert_eq!(try_drain(Stream::Csv).as_deref(), Some("2,16.5,0\r\n"));
        assert_eq!(try_drain(Stream::Log), None);

        // a drain in progress is not waited for: the lock is held, so nothing is taken
        log.write(b"held\n");
        let held = LOG.get().unwrap().receiver.lock().unwrap();
        assert_eq!(try_drain(Stream::Log), None);
        drop(held);
        assert_eq!(try_drain(Stream::Log).as_deref(), Some("held\n"));
    }

    #[test]
    fn the_end_is_told_once() {
        assert!(mark_ended());
        assert!(!mark_ended());
        assert!(!mark_ended());
    }
}
