//! The ONE writer of `telemetry.events`: a bounded queue coalesced into one
//! `INSERT ... SELECT unnest(...)` per [`CHUNK_ROWS`] or [`FLUSH_EVERY`],
//! whichever comes first; a chunk the database refuses is retried row by
//! row so one bad body loses only itself. The queue is the last bound of
//! the sink: a full queue **drops** the row and counts it (the old writer
//! degraded to a direct insert, which turned the bound into a pool of
//! per-request statements under exactly the flood it was meant to hold).
//! The drain loop is the body of the `telemetry-ingest-writer` job
//! ([`Writer::run`]), so `/admin/api/jobs` shows its state and a panic or
//! exit is a failed job, not a silent switch to dropping.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use serde_json::Value;
use sqlx::PgPool;
use tokio::sync::{mpsc, oneshot};
use tokio::time::Instant;
use tokio_util::sync::CancellationToken;

use crate::jobs::JobContext;

pub const CHUNK_ROWS: usize = 256;
pub const FLUSH_EVERY: Duration = Duration::from_millis(200);
pub const QUEUE_CAP: usize = 10_000;

pub use crate::shared::telemetry::Row;

enum Msg {
    Row(Row),
    Flush(oneshot::Sender<()>),
}

/// The handle the sinks push through; cheap to share behind the state.
pub struct WriteBuffer {
    tx: mpsc::Sender<Msg>,
    /// Rows the full queue (or a stopped writer) dropped since boot.
    pub dropped: Arc<AtomicU64>,
    /// Rows the database refused since boot (a body the STORED fingerprint
    /// cannot index, a CHECK): the row-by-row retry's losses.
    pub refused: Arc<AtomicU64>,
    /// Rows written since boot.
    pub written: Arc<AtomicU64>,
}

/// The drain loop's end of the queue; runs once as the writer job.
pub struct Writer {
    pool: PgPool,
    rx: mpsc::Receiver<Msg>,
    refused: Arc<AtomicU64>,
    written: Arc<AtomicU64>,
}

impl WriteBuffer {
    pub fn new(pool: PgPool) -> (Self, Writer) {
        let (tx, rx) = mpsc::channel(QUEUE_CAP);
        let refused = Arc::new(AtomicU64::new(0));
        let written = Arc::new(AtomicU64::new(0));
        let writer = Writer {
            pool,
            rx,
            refused: refused.clone(),
            written: written.clone(),
        };
        (
            Self {
                tx,
                dropped: Arc::new(AtomicU64::new(0)),
                refused,
                written,
            },
            writer,
        )
    }

    /// Queues one row; `false` when the queue is full or the writer is gone
    /// (the row is dropped and counted, never written from the request).
    pub fn push(&self, row: Row) -> bool {
        match self.tx.try_send(Msg::Row(row)) {
            Ok(()) => true,
            Err(_) => {
                self.dropped.fetch_add(1, Ordering::Relaxed);
                metrics::counter!("telemetry_ingest_dropped_total", "reason" => "queue_full")
                    .increment(1);
                false
            }
        }
    }

    /// Queues every row it can; returns how many were dropped.
    pub fn push_all(&self, rows: Vec<Row>) -> usize {
        let mut dropped = 0;
        for row in rows {
            if !self.push(row) {
                dropped += 1;
            }
        }
        dropped
    }

    /// Resolves once every row queued before the call has been written (at
    /// once when the writer is gone).
    pub async fn flush(&self) {
        let (tx, rx) = oneshot::channel();
        if self.tx.send(Msg::Flush(tx)).await.is_ok() {
            let _ = rx.await;
        }
    }
}

impl Writer {
    /// The `telemetry-ingest-writer` job body: coalesces the queue into
    /// bounded inserts until `cancel` fires, then closes the queue, writes
    /// what is left and returns. `ctx` (the job's trace) reports the running
    /// totals; `None` for a detached writer in tests.
    pub async fn run(
        mut self,
        cancel: CancellationToken,
        ctx: Option<JobContext>,
    ) -> anyhow::Result<()> {
        let mut rows: Vec<Row> = Vec::with_capacity(CHUNK_ROWS);
        let mut flushes: Vec<oneshot::Sender<()>> = Vec::new();
        let mut open = true;
        let mut closed = false;
        while open {
            // Once cancelled the queue is closed: `recv` then yields what is
            // buffered and `None` after, whether or not the sender lives on.
            if cancel.is_cancelled() && !closed {
                self.rx.close();
                closed = true;
            }
            let first = if closed {
                self.rx.recv().await
            } else {
                tokio::select! {
                    msg = self.rx.recv() => msg,
                    _ = cancel.cancelled() => continue,
                }
            };
            let Some(mut msg) = first else {
                break;
            };
            let deadline = Instant::now() + FLUSH_EVERY;
            loop {
                match msg {
                    Msg::Row(row) => rows.push(row),
                    Msg::Flush(done) => flushes.push(done),
                }
                if !flushes.is_empty() || rows.len() >= CHUNK_ROWS {
                    break;
                }
                match tokio::time::timeout_at(deadline, self.rx.recv()).await {
                    Ok(Some(next)) => msg = next,
                    Ok(None) => {
                        open = false;
                        break;
                    }
                    Err(_) => break,
                }
            }
            let refused = insert(&self.pool, &rows).await;
            self.refused.fetch_add(refused, Ordering::Relaxed);
            self.written
                .fetch_add(rows.len() as u64 - refused, Ordering::Relaxed);
            rows.clear();
            for done in flushes.drain(..) {
                let _ = done.send(());
            }
            if let Some(ctx) = &ctx {
                ctx.progress(format!(
                    "written {} refused {}",
                    self.written.load(Ordering::Relaxed),
                    self.refused.load(Ordering::Relaxed)
                ));
            }
        }
        Ok(())
    }
}

/// One unnest INSERT per [`CHUNK_ROWS`]; a chunk the database rejects
/// retries row by row so a single bad body only loses itself. Returns the
/// number of rows refused; a refused chunk is one `warn!` line naming the
/// first error and the count, so a flood of refusals is visible in the log
/// (and in `telemetry_rows_refused_total`) rather than a `debug!` per row.
pub async fn insert(pool: &PgPool, rows: &[Row]) -> u64 {
    let mut refused = 0u64;
    for chunk in rows.chunks(CHUNK_ROWS) {
        if chunk.len() > 1
            && !matches!(
                insert_chunk(pool, chunk).await,
                Err(sqlx::Error::Database(_))
            )
        {
            continue;
        }
        let mut first_error: Option<(String, String, String)> = None;
        let mut chunk_refused = 0u64;
        for row in chunk {
            if let Err(e) = insert_chunk(pool, std::slice::from_ref(row)).await {
                chunk_refused += 1;
                if first_error.is_none() {
                    first_error = Some((e.to_string(), row.project.clone(), row.kind.clone()));
                }
            }
        }
        if let Some((error, project, kind)) = first_error {
            metrics::counter!("telemetry_rows_refused_total").increment(chunk_refused);
            tracing::warn!(refused = chunk_refused, error = %error, project = %project, kind = %kind, "telemetry rows refused by the database");
        }
        refused += chunk_refused;
    }
    refused
}

async fn insert_chunk(pool: &PgPool, rows: &[Row]) -> Result<(), sqlx::Error> {
    let sources: Vec<&str> = rows.iter().map(|r| r.source).collect();
    let projects: Vec<&str> = rows.iter().map(|r| r.project.as_str()).collect();
    let kinds: Vec<&str> = rows.iter().map(|r| r.kind.as_str()).collect();
    let bodies: Vec<&Value> = rows.iter().map(|r| &r.body).collect();
    let reasons: Vec<Option<&str>> = rows.iter().map(|r| r.invalid_reason.as_deref()).collect();
    sqlx::query(
        "INSERT INTO telemetry.events (source, project, event_kind, body, invalid_reason) \
         SELECT * FROM unnest($1::text[], $2::text[], $3::text[], $4::jsonb[], $5::text[])",
    )
    .bind(&sources)
    .bind(&projects)
    .bind(&kinds)
    .bind(&bodies)
    .bind(&reasons)
    .execute(pool)
    .await
    .map(|_| ())
}
