//! The telemetry ingest seam: the row a sink queues and the operations a
//! domain's event route needs from the ops plane's ingest. `ops::telemetry`
//! implements [`Ingest`]; a domain holds it as `Arc<dyn Ingest>` and never
//! names ops.

use axum::response::Response;
use serde_json::Value;

/// One event on its way to `ops.telemetry_events`.
pub struct Row {
    pub source: &'static str,
    pub project: String,
    pub kind: String,
    pub body: Value,
    pub invalid_reason: Option<String>,
}

pub trait Ingest: Send + Sync {
    /// The operator's ingest switch (`TELEMETRY_INGEST_ENABLED`).
    fn ingest_enabled(&self) -> bool;
    /// The answer a route gives while ingest is switched off.
    fn ingest_disabled_response(&self) -> Response;
    /// Takes `decoded_bytes` from the global byte ceiling.
    fn admit_bytes(&self, decoded_bytes: usize) -> bool;
    /// How many of `count` events the project's daily quota admits.
    fn admit_n(&self, project: &str, count: usize) -> usize;
    /// Queues the rows; returns the writer's `push_all` count.
    fn push_all(&self, rows: Vec<Row>) -> usize;
}
