//! What `/scene_logs` reads: a scene's lines from the scene's own log ring
//! (`RendererSceneContext::logs`) and, on request, the client's warnings and errors from the ring
//! here, fed by [`LogRingLayer`]. Every line of either kind takes its `seq` from one process
//! counter ([`next_seq`]), so a reader passes the `latestSeq` of its last read as `since` and gets
//! only newer lines; a reloaded scene's lines take later seqs. `dropped` counts the lines after
//! its cursor that a ring evicted before it read them, and the client lines the layer could not
//! record ([`LogRingLayer`]).

use std::{
    collections::VecDeque,
    fmt::Write,
    sync::{
        atomic::{AtomicU64, Ordering},
        LazyLock, Mutex,
    },
};

use bevy::log::{
    tracing::{Event, Level, Subscriber},
    tracing_subscriber::{layer::Context, Layer},
};
use perf_output::{layer::Fields, scrub};

/// Client lines the ring keeps.
const CAPACITY: usize = 1000;

/// Bytes of a client line the ring keeps; the rest is cut.
const MAX_LINE_BYTES: usize = 4096;

/// The target a scene's own lines are echoed to stdout under (`RendererSceneContext::log` names
/// it on each echo); the layer leaves them out, since the scene's ring holds them.
pub const SCENE_ECHO_TARGET: &str = "scene_runner::renderer_context";

static SEQ: AtomicU64 = AtomicU64::new(0);

/// Client lines the layer could not record because the ring was being read or written.
static SKIPPED: AtomicU64 = AtomicU64::new(0);

static CLIENT: LazyLock<Mutex<ClientRing>> = LazyLock::new(Mutex::default);

/// The seq of a new line, of a scene or of the client: one more than the last.
pub fn next_seq() -> u64 {
    SEQ.fetch_add(1, Ordering::Relaxed) + 1
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

impl Severity {
    pub fn parse(name: &str) -> Option<Self> {
        Some(match name {
            "trace" => Self::Trace,
            "debug" => Self::Debug,
            "info" => Self::Info,
            "warn" => Self::Warn,
            "error" => Self::Error,
            _ => return None,
        })
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Trace => "trace",
            Self::Debug => "debug",
            Self::Info => "info",
            Self::Warn => "warn",
            Self::Error => "error",
        }
    }

    fn of(level: &Level) -> Self {
        match *level {
            Level::TRACE => Self::Trace,
            Level::DEBUG => Self::Debug,
            Level::INFO => Self::Info,
            Level::WARN => Self::Warn,
            _ => Self::Error,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Source {
    Scene,
    /// the client, by the event's tracing target
    Client(String),
}

#[derive(Clone, Debug)]
pub struct LogLine {
    pub seq: u64,
    pub severity: Severity,
    pub source: Source,
    /// a scene line's scene time, in seconds
    pub time: Option<f64>,
    /// a scene line's level as the scene runtime names it (`Log`, `SceneError`, `SystemError`)
    pub label: &'static str,
    pub text: String,
}

/// The lines a scene's ring holds, oldest first, and how many older ones it evicted.
#[derive(Debug, Default)]
pub struct SceneLines {
    pub evicted: u64,
    pub lines: Vec<LogLine>,
}

/// What a read selects.
#[derive(Clone, Debug)]
pub struct LogQuery {
    /// the client's lines too
    pub client: bool,
    /// only lines after this seq
    pub since: Option<u64>,
    /// the least severity read
    pub min: Severity,
    /// at most this many lines, the newest
    pub count: Option<usize>,
}

/// A read: the lines, oldest first, and what the header reports.
#[derive(Debug)]
pub struct LogPage {
    /// the newest seq given out, to any source; 0 before the first line
    pub latest_seq: u64,
    /// lines after `since` that the rings read evicted, and client lines the layer could not
    /// record. Exact without `since`; with it, the most it can be: a ring counts its evictions,
    /// not which of them came after a seq.
    pub dropped: u64,
    /// lines the query selected that `count` left out (the oldest)
    pub omitted: usize,
    pub client: bool,
    pub lines: Vec<LogLine>,
}

#[derive(Default)]
struct ClientRing {
    lines: VecDeque<LogLine>,
    evicted: u64,
}

/// What a read takes from the client ring while it holds the lock.
#[derive(Default)]
struct ClientRead {
    evicted: u64,
    /// the seq of the oldest line the ring holds
    oldest: Option<u64>,
    /// lines the layer could not record
    skipped: u64,
    /// the lines after the query's `since` of at least its severity, oldest first
    lines: Vec<LogLine>,
}

/// Cuts `text` to [`MAX_LINE_BYTES`] on a character boundary; whether it cut anything.
fn cut(text: &mut String) -> bool {
    if text.len() <= MAX_LINE_BYTES {
        return false;
    }
    let mut end = MAX_LINE_BYTES;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    text.truncate(end);
    true
}

/// A client line's text as the ring keeps it: cut to [`MAX_LINE_BYTES`], then with every
/// credential replaced. The cut comes first, so the scrub never reads more than that of a text
/// a scene may have chosen (a failed asset's `src`); a text that was cut ends in `…`.
fn client_text(mut text: String) -> String {
    let mut was_cut = cut(&mut text);
    let mut text = scrub::credentials(&text).into_owned();
    // a replacement can be longer than what it replaces
    was_cut |= cut(&mut text);
    if was_cut {
        text.push('…');
    }
    text
}

impl ClientRing {
    /// Adds a line; `text` is already [`client_text`].
    fn push(&mut self, severity: Severity, target: String, text: String) {
        if self.lines.len() == CAPACITY {
            self.lines.pop_front();
            self.evicted += 1;
        }
        // the seq is taken under the lock, so the ring stays in seq order
        self.lines.push_back(LogLine {
            seq: next_seq(),
            severity,
            source: Source::Client(target),
            time: None,
            label: "",
            text,
        });
    }

    /// What `query` reads of the ring. The ring is in seq order, so the lines after `since` are
    /// its tail, and only those are copied.
    fn read(&self, query: &LogQuery) -> ClientRead {
        let since = query.since.unwrap_or(0);
        let start = self.lines.partition_point(|line| line.seq <= since);
        ClientRead {
            evicted: self.evicted,
            oldest: self.lines.front().map(|line| line.seq),
            skipped: 0,
            lines: self
                .lines
                .range(start..)
                .filter(|line| line.severity >= query.min)
                .cloned()
                .collect(),
        }
    }
}

/// The most lines after `since` a ring can have evicted: no more than it evicted in all, and no
/// more than the seqs between `since` and the oldest line it still holds.
fn dropped_after(since: u64, evicted: u64, oldest: Option<u64>, latest: u64) -> u64 {
    evicted.min(
        oldest
            .unwrap_or(latest + 1)
            .saturating_sub(since.saturating_add(1)),
    )
}

fn page(
    scene: SceneLines,
    client: Option<ClientRead>,
    latest_seq: u64,
    query: &LogQuery,
) -> LogPage {
    let since = query.since.unwrap_or(0);
    let mut dropped = dropped_after(
        since,
        scene.evicted,
        scene.lines.first().map(|line| line.seq),
        latest_seq,
    );
    let mut lines = scene.lines;
    if let Some(client) = client {
        dropped += dropped_after(since, client.evicted, client.oldest, latest_seq);
        // a skipped line took a seq, so no more of them came after `since` than seqs did
        dropped += client.skipped.min(latest_seq.saturating_sub(since));
        lines.extend(client.lines);
        lines.sort_by_key(|line| line.seq);
    }
    lines.retain(|line| line.seq > since && line.severity >= query.min);
    let omitted = lines.len() - query.count.unwrap_or(usize::MAX).min(lines.len());
    let lines = lines.split_off(omitted);
    LogPage {
        latest_seq,
        dropped,
        omitted,
        client: query.client,
        lines,
    }
}

/// Reads a scene's lines and, with `query.client`, the client's.
pub fn read(scene: SceneLines, query: &LogQuery) -> LogPage {
    // the lock is held for the copy of the lines the query selects alone: the merge and the sort
    // run after it, so a thread that logs meanwhile waits (or, on the web, skips) as briefly as
    // can be
    let (client, latest_seq) = {
        let ring = CLIENT
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        // read under the lock: no line the ring records can take a seq past it meanwhile
        let latest_seq = SEQ.load(Ordering::Relaxed);
        let client = query.client.then(|| ClientRead {
            skipped: SKIPPED.load(Ordering::Relaxed),
            ..ring.read(query)
        });
        (client, latest_seq)
    };
    page(scene, client, latest_seq, query)
}

impl LogPage {
    /// The reply: a header line `latestSeq=N dropped=N source=scene|scene+client` (and
    /// ` omitted=N` when `count` cut lines), then one line per entry. `plain` keeps the line shape
    /// `/scene_logs` had before the cursor (`<seq> [<time>] <Level>: <text>`); otherwise a line is
    /// `<seq> [scene]|[client:<target>] <severity> <text>`. A line's text continues on lines
    /// indented four spaces, so no text can pass for an entry or a header.
    pub fn render(&self, plain: bool) -> String {
        let mut out = format!(
            "latestSeq={} dropped={} source={}",
            self.latest_seq,
            self.dropped,
            if self.client { "scene+client" } else { "scene" }
        );
        if self.omitted > 0 {
            let _ = write!(out, " omitted={}", self.omitted);
        }
        for line in &self.lines {
            let text = line.text.trim_end().replace('\n', "\n    ");
            let _ = match (&line.source, plain) {
                (Source::Scene, true) => write!(
                    out,
                    "\n{} [{:.2}] {}: {text}",
                    line.seq,
                    line.time.unwrap_or_default(),
                    line.label
                ),
                (Source::Scene, false) => {
                    write!(
                        out,
                        "\n{} [scene] {} {text}",
                        line.seq,
                        line.severity.name()
                    )
                }
                (Source::Client(target), _) => write!(
                    out,
                    "\n{} [client:{target}] {} {text}",
                    line.seq,
                    line.severity.name()
                ),
            };
        }
        out
    }
}

/// The tracing layer that records the client's warnings and errors in the client ring.
///
/// It never waits on the web. There every thread logs through this layer, the page's thread
/// among them, and the page's thread cannot block: a wait on a held lock traps it. So a line that
/// arrives while the ring is held is not recorded; it takes a seq and is counted, and a read
/// reports it in `dropped`. A native thread waits instead: the ring is held only to push a line
/// or to copy the lines of a read.
pub struct LogRingLayer;

/// The client ring to record a line in, or `None` where this thread may not wait for it.
fn ring_to_write() -> Option<std::sync::MutexGuard<'static, ClientRing>> {
    use std::sync::TryLockError;
    if cfg!(target_arch = "wasm32") {
        match CLIENT.try_lock() {
            Ok(ring) => Some(ring),
            Err(TryLockError::Poisoned(poisoned)) => Some(poisoned.into_inner()),
            Err(TryLockError::WouldBlock) => None,
        }
    } else {
        Some(
            CLIENT
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner()),
        )
    }
}

/// Counts a line the layer could not record. It takes a seq all the same, so a read's cursor
/// bounds how many of these came after it.
fn skip() {
    next_seq();
    SKIPPED.fetch_add(1, Ordering::Relaxed);
}

impl<S: Subscriber> Layer<S> for LogRingLayer {
    fn on_event(&self, event: &Event<'_>, _: Context<'_, S>) {
        let metadata = event.metadata();
        // tracing orders levels by verbosity: WARN is "greater" than ERROR
        if *metadata.level() > Level::WARN || metadata.target() == SCENE_ECHO_TARGET {
            return;
        }
        let mut fields = Fields::default();
        event.record(&mut fields);
        let target = fields.log_target().unwrap_or(metadata.target()).to_owned();
        let text = client_text(fields.message());
        match ring_to_write() {
            Some(mut ring) => ring.push(Severity::of(metadata.level()), target, text),
            None => skip(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scene_line(seq: u64, severity: Severity, label: &'static str, text: &str) -> LogLine {
        LogLine {
            seq,
            severity,
            source: Source::Scene,
            time: Some(seq as f64),
            label,
            text: text.to_owned(),
        }
    }

    fn scene() -> SceneLines {
        SceneLines {
            evicted: 0,
            lines: vec![
                scene_line(1, Severity::Info, "Log", "a"),
                scene_line(3, Severity::Error, "SceneError", "b\nc"),
            ],
        }
    }

    fn client() -> ClientRing {
        ClientRing {
            lines: [LogLine {
                seq: 2,
                severity: Severity::Warn,
                source: Source::Client("comms".to_owned()),
                time: None,
                label: "",
                text: "w".to_owned(),
            }]
            .into(),
            evicted: 0,
        }
    }

    fn query() -> LogQuery {
        LogQuery {
            client: false,
            since: None,
            min: Severity::Trace,
            count: None,
        }
    }

    fn seqs(page: &LogPage) -> Vec<u64> {
        page.lines.iter().map(|line| line.seq).collect()
    }

    #[test]
    fn a_read_merges_the_client_in_seq_order_on_request() {
        let alone = page(scene(), None, 4, &query());
        assert_eq!((alone.latest_seq, seqs(&alone)), (4, vec![1, 3]));
        let both = LogQuery {
            client: true,
            ..query()
        };
        let client = client().read(&both);
        assert_eq!(seqs(&page(scene(), Some(client), 4, &both)), [1, 2, 3]);
    }

    #[test]
    fn since_severity_and_count_narrow_a_read() {
        let client = client();
        let read = |query: LogQuery| page(scene(), Some(client.read(&query)), 4, &query);
        let after = read(LogQuery {
            since: Some(1),
            ..query()
        });
        assert_eq!(seqs(&after), [2, 3]);
        let errors = read(LogQuery {
            min: Severity::Error,
            ..query()
        });
        assert_eq!(seqs(&errors), [3]);
        let last = read(LogQuery {
            count: Some(1),
            ..query()
        });
        assert_eq!((last.omitted, seqs(&last)), (2, vec![3]));
    }

    #[test]
    fn dropped_counts_what_a_ring_evicted_after_the_cursor() {
        // the scene's ring evicted 5 lines and holds seqs 9 and 10
        let evicted = || SceneLines {
            evicted: 5,
            lines: vec![
                scene_line(9, Severity::Info, "Log", "y"),
                scene_line(10, Severity::Info, "Log", "z"),
            ],
        };
        let dropped = |since| {
            let query = LogQuery { since, ..query() };
            page(evicted(), None, 10, &query).dropped
        };
        assert_eq!(dropped(None), 5);
        // seqs 7 and 8 are all that can be missing after 6
        assert_eq!(dropped(Some(6)), 2);
        assert_eq!(dropped(Some(8)), 0);
        assert_eq!(dropped(Some(10)), 0);
        // a ring that evicted nothing dropped nothing, whatever the cursor
        assert_eq!(page(scene(), None, 4, &query()).dropped, 0);
        // the last seq there is has nothing after it
        assert_eq!(dropped(Some(u64::MAX)), 0);
    }

    #[test]
    fn dropped_counts_the_client_lines_the_layer_skipped() {
        let dropped = |since, skipped| {
            let query = LogQuery {
                client: true,
                since,
                ..query()
            };
            let client = ClientRead {
                skipped,
                ..client().read(&query)
            };
            page(scene(), Some(client), 10, &query).dropped
        };
        assert_eq!(dropped(None, 3), 3);
        // only seqs 9 and 10 came after 8, so no more than two skipped lines did
        assert_eq!(dropped(Some(8), 3), 2);
        assert_eq!(dropped(Some(10), 3), 0);
        assert_eq!(dropped(None, 0), 0);
    }

    #[test]
    fn a_skipped_line_takes_a_seq_and_is_counted() {
        let (seq, skipped) = (SEQ.load(Ordering::Relaxed), SKIPPED.load(Ordering::Relaxed));
        skip();
        // other tests give out seqs meanwhile, so each is at least one further
        assert!(SEQ.load(Ordering::Relaxed) > seq);
        assert!(SKIPPED.load(Ordering::Relaxed) > skipped);
    }

    #[test]
    fn a_read_copies_only_the_client_lines_after_the_cursor() {
        let mut ring = client();
        ring.lines.push_back(LogLine {
            seq: 5,
            severity: Severity::Error,
            source: Source::Client("comms".to_owned()),
            time: None,
            label: "",
            text: "e".to_owned(),
        });
        let read = |since, min| {
            let read = ring.read(&LogQuery {
                since,
                min,
                ..query()
            });
            assert_eq!(read.oldest, Some(2));
            read.lines.iter().map(|line| line.seq).collect::<Vec<_>>()
        };
        assert_eq!(read(None, Severity::Trace), [2, 5]);
        assert_eq!(read(Some(2), Severity::Trace), [5]);
        assert_eq!(read(None, Severity::Error), [5]);
        assert_eq!(read(Some(5), Severity::Trace), Vec::<u64>::new());
    }

    #[test]
    fn a_long_text_is_cut_before_its_credentials_are_replaced() {
        // far past the cut: the scrub must not read it
        let long = client_text("signature ".repeat(100_000));
        assert!(long.len() <= MAX_LINE_BYTES + '…'.len_utf8() && long.ends_with('…'));
        let multibyte = client_text("é".repeat(MAX_LINE_BYTES));
        assert!(multibyte.len() <= MAX_LINE_BYTES + '…'.len_utf8() && multibyte.ends_with('…'));
        assert_eq!(client_text("short".to_owned()), "short");
    }

    #[test]
    fn the_client_ring_counts_evictions_and_stays_in_seq_order() {
        let mut ring = ClientRing::default();
        for _ in 0..CAPACITY + 2 {
            ring.push(Severity::Warn, "t".to_owned(), "w".to_owned());
        }
        assert_eq!((ring.lines.len(), ring.evicted), (CAPACITY, 2));
        let seqs: Vec<u64> = ring.lines.iter().map(|line| line.seq).collect();
        assert!(seqs.windows(2).all(|pair| pair[0] < pair[1]));
    }

    #[test]
    fn a_page_renders_a_header_and_indents_continuations() {
        let both = LogQuery {
            client: true,
            ..query()
        };
        assert_eq!(
            page(scene(), Some(client().read(&both)), 4, &both).render(false),
            "latestSeq=4 dropped=0 source=scene+client\n1 [scene] info a\n2 [client:comms] warn \
             w\n3 [scene] error b\n    c"
        );
        assert_eq!(
            page(scene(), None, 4, &query()).render(true),
            "latestSeq=4 dropped=0 source=scene\n1 [1.00] Log: a\n3 [3.00] SceneError: b\n    c"
        );
    }
}
