//! `POST /exports?format=json|csv`: a scene's world values, then its player
//! values (never env), streamed from Postgres row streams through the
//! serializer, a raw-size cap and gzip (upstream world-storage-service's
//! storage export). Values are the stored jsonb text spliced verbatim, as the
//! list routes serve them. The limits are per process, as upstream's are per
//! pod: a slot is held by the producer, so completion, timeout or cancellation
//! frees it even when a client keeps an unread response open.
//!
//! As upstream, admission starts the scene cooldown and the export timer, and
//! the answer is a 200 before any database work: a connection, snapshot or
//! query failure afterwards cuts the gzip stream (an HTTP body error, never a
//! valid trailer). Both row streams read one read-only repeatable-read
//! snapshot; `PG_COMPONENT_STREAM_QUERY_TIMEOUT` bounds each statement.

use std::collections::HashMap;
use std::io::Write;
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::body::{Body, Bytes};
use axum::extract::{OriginalUri, Request, State};
use axum::http::{HeaderMap, HeaderValue, header};
use axum::response::{IntoResponse, Response};
use flate2::Compression;
use flate2::write::GzEncoder;
use futures::TryStreamExt;
use parking_lot::Mutex;
use serde_json::Value;
use sqlx::{PgConnection, PgPool, Row};
use tokio::sync::{mpsc, oneshot};
use tokio_util::sync::CancellationToken;

use crate::comms::world_identity::SceneNamespace;
use crate::shared::identity::Address;

use super::errors::ApiError;
use super::handlers::validate_key;
use super::{AuthPolicy, SharedState, resolve_authorized_checking_body};

/// Upstream's body cap (256 KiB) plus its envelope slack.
pub const MAX_BODY_BYTES: u64 = 256 * 1024 + 1024;
const MAX_LIST_ENTRIES: usize = 100;
const FLUSH_BYTES: usize = 64 * 1024;

/// `EXPORT_*` and `PG_COMPONENT_STREAM_QUERY_TIMEOUT`, upstream's names and
/// defaults. Any value is accepted, as upstream's: a zero cap refuses every
/// export 429, a zero timeout or size cap aborts every stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExportSettings {
    pub max_concurrent_per_scene: usize,
    pub max_concurrent_per_pod: usize,
    pub scene_cooldown: Duration,
    pub timeout: Duration,
    pub max_size_bytes: u64,
    /// Each export statement's `statement_timeout` (0 disables it, as in PostgreSQL).
    pub stream_query_timeout: Duration,
}

impl Default for ExportSettings {
    fn default() -> Self {
        Self {
            max_concurrent_per_scene: 1,
            max_concurrent_per_pod: 5,
            scene_cooldown: Duration::from_secs(10),
            timeout: Duration::from_secs(120),
            max_size_bytes: 524_288_000,
            stream_query_timeout: Duration::from_millis(120_000),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Json,
    Csv,
}

impl Format {
    fn extension(self) -> &'static str {
        match self {
            Format::Json => "json",
            Format::Csv => "csv",
        }
    }
}

/// `format` exactly once, `json` or `csv`.
pub fn parse_format(query: Option<&str>) -> Result<Format, ApiError> {
    let formats: Vec<_> = url::form_urlencoded::parse(query.unwrap_or("").as_bytes())
        .filter(|(name, _)| name == "format")
        .map(|(_, value)| value)
        .collect();
    match formats.as_slice() {
        [f] if f == "json" => Ok(Format::Json),
        [f] if f == "csv" => Ok(Format::Csv),
        _ => Err(ApiError::bad_request("format must be one of: json, csv")),
    }
}

/// `None` selects everything, an empty list nothing.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Selection {
    pub world_keys: Option<Vec<String>>,
    pub player_addresses: Option<Vec<String>>,
    pub player_keys: Option<Vec<String>>,
}

fn object<'a>(
    value: &'a Value,
    type_message: &str,
    allowed: &[&str],
    fields_message: &str,
) -> Result<&'a serde_json::Map<String, Value>, ApiError> {
    let map = value
        .as_object()
        .ok_or_else(|| ApiError::bad_request(type_message))?;
    if map.keys().any(|k| !allowed.contains(&k.as_str())) {
        return Err(ApiError::bad_request(fields_message));
    }
    Ok(map)
}

fn list(value: Option<&Value>, label: &str) -> Result<Option<Vec<String>>, ApiError> {
    let Some(value) = value else { return Ok(None) };
    let entries = value
        .as_array()
        .and_then(|a| {
            a.iter()
                .map(|e| e.as_str().map(str::to_string))
                .collect::<Option<Vec<_>>>()
        })
        .ok_or_else(|| ApiError::bad_request(format!("{label} must be an array of strings")))?;
    if entries.len() > MAX_LIST_ENTRIES {
        return Err(ApiError::bad_request(format!(
            "{label} must have at most {MAX_LIST_ENTRIES} entries"
        )));
    }
    Ok(Some(entries))
}

/// The optional body `{world:{keys}, players:{addresses, keys}}`; an empty
/// body selects everything.
pub fn parse_selection(body: &[u8]) -> Result<Selection, ApiError> {
    let text = String::from_utf8_lossy(body);
    if text.trim().is_empty() {
        return Ok(Selection::default());
    }
    let parsed: Value = serde_json::from_str(&text)
        .map_err(|_| ApiError::bad_request("Request body must be valid JSON"))?;
    let request = object(
        &parsed,
        "Request body must be a JSON object",
        &["world", "players"],
        "Request body may only contain world and players",
    )?;
    let empty = serde_json::Map::new();
    let world = match request.get("world") {
        Some(w) => object(
            w,
            "world must be an object",
            &["keys"],
            "world may only contain keys",
        )?,
        None => &empty,
    };
    let players = match request.get("players") {
        Some(p) => object(
            p,
            "players must be an object",
            &["addresses", "keys"],
            "players may only contain addresses and keys",
        )?,
        None => &empty,
    };
    let world_keys = list(world.get("keys"), "world.keys")?;
    let player_addresses = list(players.get("addresses"), "players.addresses")?
        .map(|a| a.into_iter().map(|a| a.to_lowercase()).collect::<Vec<_>>());
    let player_keys = list(players.get("keys"), "players.keys")?;
    for key in world_keys.iter().chain(player_keys.iter()).flatten() {
        validate_key(key)?;
        if key.contains('\0') {
            return Err(ApiError::bad_request("Key must not contain NUL characters"));
        }
    }
    if player_addresses
        .iter()
        .flatten()
        .any(|a| Address::parse_untrimmed(a).is_err())
    {
        return Err(ApiError::bad_request("Invalid player address"));
    }
    let none = |l: &Option<Vec<String>>| l.as_ref().is_some_and(Vec::is_empty);
    if none(&world_keys) && (none(&player_addresses) || none(&player_keys)) {
        return Err(ApiError::bad_request(
            "The selection is empty: nothing to export",
        ));
    }
    Ok(Selection {
        world_keys,
        player_addresses,
        player_keys,
    })
}

#[derive(Default)]
struct Counters {
    open_by_scene: HashMap<String, usize>,
    last_start: HashMap<String, Instant>,
    open: usize,
}

/// The per-process caps and cooldown.
pub struct ExportLimits {
    settings: ExportSettings,
    counters: Mutex<Counters>,
}

/// Why an export did not start.
#[derive(Debug, PartialEq, Eq)]
pub enum Refusal {
    SceneBusy,
    Cooldown { retry_after_secs: u64 },
    PodBusy,
}

impl Refusal {
    fn log(&self, ns: &SceneNamespace) {
        let (realm, namespace) = (ns.realm(), ns.scene_namespace());
        match self {
            Refusal::Cooldown { retry_after_secs } => tracing::info!(
                realm,
                namespace,
                retry_after_secs,
                "storage export rejected: scene export cooldown"
            ),
            Refusal::SceneBusy | Refusal::PodBusy => tracing::info!(
                realm,
                namespace,
                scope = if *self == Refusal::SceneBusy {
                    "scene"
                } else {
                    "pod"
                },
                "storage export rejected: concurrent export cap reached"
            ),
        }
    }
}

/// A running export's place in the caps, released on drop.
pub struct Slot {
    limits: Arc<ExportLimits>,
    scene: String,
    /// Admission: the export timer runs from here, as upstream's.
    admitted: tokio::time::Instant,
    #[cfg(test)]
    pub(super) test_hooks: TestHooks,
}

/// Per-export synchronization at completed database boundaries, never global.
#[cfg(test)]
#[derive(Default)]
pub(super) struct TestHooks {
    pub after_world_rows: Option<TestPause>,
    pub before_rollback: Option<TestPause>,
}

#[cfg(test)]
pub(super) struct TestPause {
    pub reached: oneshot::Sender<()>,
    pub resume: oneshot::Receiver<()>,
}

#[cfg(test)]
impl TestPause {
    async fn wait(self) -> Result<(), Abort> {
        self.reached.send(()).map_err(|_| Abort::ClientGone)?;
        self.resume.await.map_err(|_| Abort::ClientGone)
    }
}

impl Drop for Slot {
    fn drop(&mut self) {
        let mut c = self.limits.counters.lock();
        c.open -= 1;
        if let Some(n) = c.open_by_scene.get_mut(&self.scene) {
            *n -= 1;
            if *n == 0 {
                c.open_by_scene.remove(&self.scene);
            }
        }
    }
}

impl ExportLimits {
    pub fn new(settings: ExportSettings) -> Arc<Self> {
        Arc::new(Self {
            settings,
            counters: Mutex::default(),
        })
    }

    pub fn settings(&self) -> ExportSettings {
        self.settings
    }

    /// Upstream's order: the scene cap, the scene's cooldown, the process cap.
    /// Admission starts the cooldown, so a concurrent caller gets the cooldown
    /// 429 and a start that fails later still consumes it.
    pub fn open(self: &Arc<Self>, scene: &str) -> Result<Slot, Refusal> {
        let now = Instant::now();
        let cooldown = self.settings.scene_cooldown;
        let mut c = self.counters.lock();
        if c.open_by_scene.get(scene).copied().unwrap_or(0)
            >= self.settings.max_concurrent_per_scene
        {
            return Err(Refusal::SceneBusy);
        }
        c.last_start
            .retain(|_, at| now.duration_since(*at) < cooldown);
        if let Some(at) = c.last_start.get(scene) {
            let left = cooldown - now.duration_since(*at);
            return Err(Refusal::Cooldown {
                retry_after_secs: left.as_millis().div_ceil(1000) as u64,
            });
        }
        if c.open >= self.settings.max_concurrent_per_pod {
            return Err(Refusal::PodBusy);
        }
        if !cooldown.is_zero() {
            c.last_start.insert(scene.to_string(), now);
        }
        c.open += 1;
        *c.open_by_scene.entry(scene.to_string()).or_default() += 1;
        Ok(Slot {
            limits: self.clone(),
            scene: scene.to_string(),
            admitted: tokio::time::Instant::now(),
            #[cfg(test)]
            test_hooks: TestHooks::default(),
        })
    }
}

fn csv_cell(cell: &str) -> String {
    if cell.contains(['"', ',', '\r', '\n']) {
        format!("\"{}\"", cell.replace('"', "\"\""))
    } else {
        cell.to_string()
    }
}

/// A key a spreadsheet would read as a formula gets a leading `'`.
fn neutralize(cell: &str) -> String {
    if cell.starts_with(['=', '+', '-', '@', '\t', '\r']) {
        format!("'{cell}")
    } else {
        cell.to_string()
    }
}

fn json_string(s: &str) -> String {
    serde_json::to_string(s).expect("strings serialize")
}

fn separator(first: bool) -> &'static str {
    if first { "" } else { "," }
}

impl Format {
    fn head(self) -> &'static str {
        match self {
            Format::Json => "{\"world\":[",
            Format::Csv => "scope,player_address,key,value\r\n",
        }
    }

    fn middle(self) -> &'static str {
        match self {
            Format::Json => "],\"players\":[",
            Format::Csv => "",
        }
    }

    fn tail(self) -> &'static str {
        match self {
            Format::Json => "]}",
            Format::Csv => "",
        }
    }

    fn world_row(self, first: bool, key: &str, value: &str) -> String {
        match self {
            Format::Json => format!(
                "{}{{\"key\":{},\"value\":{value}}}",
                separator(first),
                json_string(key)
            ),
            Format::Csv => format!(
                "world,,{},{}\r\n",
                csv_cell(&neutralize(key)),
                csv_cell(value)
            ),
        }
    }

    fn player_row(self, first: bool, address: &str, key: &str, value: &str) -> String {
        match self {
            Format::Json => format!(
                "{}{{\"address\":{},\"key\":{},\"value\":{value}}}",
                separator(first),
                json_string(address),
                json_string(key)
            ),
            Format::Csv => format!(
                "player,{},{},{}\r\n",
                csv_cell(address),
                csv_cell(&neutralize(key)),
                csv_cell(value)
            ),
        }
    }
}

#[derive(Debug, thiserror::Error)]
enum Abort {
    #[error("the raw export passed EXPORT_MAX_SIZE_BYTES")]
    TooLarge,
    #[error("the export ran past EXPORT_TIMEOUT_SECONDS")]
    Timeout,
    #[error("the client went away")]
    ClientGone,
    #[error("database: {0}")]
    Db(sqlx::Error),
    #[error("gzip: {0}")]
    Io(std::io::Error),
}

/// Counts raw bytes against the cap and gzips them into body chunks.
struct Gzip {
    /// `None` only while a batch compresses on the blocking pool.
    encoder: Option<GzEncoder<Vec<u8>>>,
    pending: Vec<u8>,
    raw: u64,
    max: u64,
    tx: mpsc::Sender<Bytes>,
}

impl Gzip {
    fn new(max: u64, tx: mpsc::Sender<Bytes>) -> Self {
        Self {
            encoder: Some(GzEncoder::new(Vec::new(), Compression::default())),
            pending: Vec::new(),
            raw: 0,
            max,
            tx,
        }
    }

    async fn push(&mut self, text: &str) -> Result<(), Abort> {
        self.raw += text.len() as u64;
        if self.raw > self.max {
            return Err(Abort::TooLarge);
        }
        self.pending.extend_from_slice(text.as_bytes());
        if self.pending.len() >= FLUSH_BYTES {
            self.compress().await?;
        }
        Ok(())
    }

    /// Deflates the pending text off the runtime workers, as Node's zlib
    /// runs on its thread pool, and sends the output once it reaches a chunk.
    async fn compress(&mut self) -> Result<(), Abort> {
        let mut encoder = self
            .encoder
            .take()
            .expect("the encoder is back after each batch");
        let input = std::mem::take(&mut self.pending);
        let mut encoder =
            tokio::task::spawn_blocking(move || encoder.write_all(&input).map(|()| encoder))
                .await
                .map_err(|e| Abort::Io(std::io::Error::other(e)))?
                .map_err(Abort::Io)?;
        let chunk = if encoder.get_ref().len() >= FLUSH_BYTES {
            std::mem::take(encoder.get_mut())
        } else {
            Vec::new()
        };
        self.encoder = Some(encoder);
        send(&self.tx, chunk).await
    }

    async fn finish(mut self) -> Result<(), Abort> {
        self.compress().await?;
        let encoder = self
            .encoder
            .take()
            .expect("the encoder is back after each batch");
        let rest = tokio::task::spawn_blocking(move || encoder.finish())
            .await
            .map_err(|e| Abort::Io(std::io::Error::other(e)))?
            .map_err(Abort::Io)?;
        send(&self.tx, rest).await
    }
}

async fn send(tx: &mpsc::Sender<Bytes>, chunk: Vec<u8>) -> Result<(), Abort> {
    if chunk.is_empty() {
        return Ok(());
    }
    tx.send(Bytes::from(chunk))
        .await
        .map_err(|_| Abort::ClientGone)
}

fn is_none_selected(list: &Option<Vec<String>>) -> bool {
    list.as_ref().is_some_and(Vec::is_empty)
}

/// World rows by key, then player rows by address and key; never env.
async fn produce(
    conn: &mut PgConnection,
    ns: &SceneNamespace,
    selection: &Selection,
    format: Format,
    out: &mut Gzip,
    #[cfg(test)] after_world_rows: Option<TestPause>,
) -> Result<(), Abort> {
    out.push(format.head()).await?;
    if !is_none_selected(&selection.world_keys) {
        let mut rows = sqlx::query(
            "SELECT key, value::text AS value FROM worlds.world_storage
             WHERE realm = $1 AND scene_namespace = $2 AND ($3::text[] IS NULL OR key = ANY($3))
             ORDER BY key",
        )
        .bind(ns.realm())
        .bind(ns.scene_namespace())
        .bind(&selection.world_keys)
        .fetch(&mut *conn);
        let mut first = true;
        while let Some(row) = rows.try_next().await.map_err(Abort::Db)? {
            out.push(&format.world_row(first, row.get("key"), row.get("value")))
                .await?;
            first = false;
        }
    }
    #[cfg(test)]
    if let Some(pause) = after_world_rows {
        pause.wait().await?;
    }
    out.push(format.middle()).await?;
    if !is_none_selected(&selection.player_addresses) && !is_none_selected(&selection.player_keys) {
        let mut rows = sqlx::query(
            "SELECT player_address, key, value::text AS value FROM worlds.player_storage
             WHERE realm = $1 AND scene_namespace = $2
               AND ($3::text[] IS NULL OR player_address = ANY($3))
               AND ($4::text[] IS NULL OR key = ANY($4))
             ORDER BY player_address, key",
        )
        .bind(ns.realm())
        .bind(ns.scene_namespace())
        .bind(&selection.player_addresses)
        .bind(&selection.player_keys)
        .fetch(&mut *conn);
        let mut first = true;
        while let Some(row) = rows.try_next().await.map_err(Abort::Db)? {
            out.push(&format.player_row(
                first,
                row.get("player_address"),
                row.get("key"),
                row.get("value"),
            ))
            .await?;
            first = false;
        }
    }
    out.push(format.tail()).await
}

/// The admitted export's response body. The producer owns the slot and, once
/// acquired, one pooled connection holding a read-only snapshot. The
/// completion channel distinguishes successful EOF from every failure,
/// including a panic. Dropping the body cancels the producer; the export
/// timer, running since admission, bounds acquisition, SQL and stalled clients.
pub(super) fn body(
    pool: PgPool,
    ns: SceneNamespace,
    selection: Selection,
    format: Format,
    #[allow(unused_mut)] mut slot: Slot,
    settings: ExportSettings,
) -> Body {
    // A timeout past what an Instant can hold sets no deadline.
    let deadline = slot.admitted.checked_add(settings.timeout);
    let (tx, rx) = mpsc::channel::<Bytes>(4);
    let (completed, completion) = oneshot::channel();
    let cancel = CancellationToken::new();
    let drop_guard = cancel.clone().drop_guard();
    tokio::spawn(async move {
        let mut out = Gzip::new(settings.max_size_bytes, tx);
        #[cfg(test)]
        let (after_world_rows, before_rollback) = (
            slot.test_hooks.after_world_rows.take(),
            slot.test_hooks.before_rollback.take(),
        );
        let work = async {
            let mut tx_db = pool.begin().await.map_err(Abort::Db)?;
            sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
                .execute(&mut *tx_db)
                .await
                .map_err(Abort::Db)?;
            // SET LOCAL resets on rollback, preserving both pool timeouts.
            // Backpressure can leave the snapshot idle between row streams.
            let millis = settings
                .stream_query_timeout
                .as_millis()
                .min(i32::MAX as u128)
                .to_string();
            sqlx::query(
                "SELECT set_config('statement_timeout', $1, true),
                        set_config('idle_in_transaction_session_timeout', $1, true)",
            )
            .bind(millis)
            .execute(&mut *tx_db)
            .await
            .map_err(Abort::Db)?;
            produce(
                &mut tx_db,
                &ns,
                &selection,
                format,
                &mut out,
                #[cfg(test)]
                after_world_rows,
            )
            .await?;
            #[cfg(test)]
            if let Some(pause) = before_rollback {
                pause.wait().await?;
            }
            let raw = out.raw;
            // A database failure must not follow a valid gzip trailer.
            tx_db.rollback().await.map_err(Abort::Db)?;
            out.finish().await?;
            Ok(raw)
        };
        let bounded = async {
            match deadline {
                // timeout_at polls a ready future before its timer: an
                // already spent budget never starts the work.
                Some(at) if tokio::time::Instant::now() >= at => Err(Abort::Timeout),
                Some(at) => tokio::time::timeout_at(at, work)
                    .await
                    .unwrap_or(Err(Abort::Timeout)),
                None => work.await,
            }
        };
        let outcome = tokio::select! {
            biased;
            _ = cancel.cancelled() => Err(Abort::ClientGone),
            result = bounded => result,
        };
        log_outcome(&ns, format, slot.admitted.elapsed(), &outcome);
        // Completion never blocks behind a full data channel. A producer panic
        // cancels this oneshot, which the reader also reports as a body error.
        drop(slot);
        let _ = completed.send(
            outcome
                .map(|_| ())
                .map_err(|_| std::io::Error::other("storage export aborted")),
        );
    });
    response_body(rx, completion, drop_guard)
}

/// Upstream's outcome lines: finished and cancelled at info, failures at error.
fn log_outcome(
    ns: &SceneNamespace,
    format: Format,
    elapsed: Duration,
    outcome: &Result<u64, Abort>,
) {
    let (realm, namespace, format) = (ns.realm(), ns.scene_namespace(), format.extension());
    let duration_ms = elapsed.as_millis() as u64;
    match outcome {
        Ok(raw_bytes) => tracing::info!(
            realm,
            namespace,
            format,
            raw_bytes,
            duration_ms,
            "storage export finished"
        ),
        Err(Abort::ClientGone) => tracing::info!(
            realm,
            namespace,
            format,
            duration_ms,
            "storage export cancelled by client"
        ),
        Err(Abort::TooLarge) => tracing::error!(
            realm,
            namespace,
            format,
            duration_ms,
            "storage export exceeded the maximum size"
        ),
        Err(Abort::Timeout) => tracing::error!(
            realm,
            namespace,
            format,
            duration_ms,
            "storage export timed out"
        ),
        Err(error) => tracing::error!(
            realm,
            namespace,
            format,
            duration_ms,
            %error,
            "storage export failed"
        ),
    }
}

fn response_body(
    rx: mpsc::Receiver<Bytes>,
    completion: oneshot::Receiver<Result<(), std::io::Error>>,
    drop_guard: tokio_util::sync::DropGuard,
) -> Body {
    Body::from_stream(futures::stream::unfold(
        (rx, Some(completion), drop_guard),
        |(mut rx, mut completion, drop_guard)| async move {
            let done = completion.as_mut()?;
            match rx.recv().await {
                Some(chunk) => Some((Ok(chunk), (rx, completion, drop_guard))),
                None => {
                    let result = done.await.unwrap_or_else(|_| {
                        Err(std::io::Error::other("storage export producer stopped"))
                    });
                    completion = None;
                    result
                        .err()
                        .map(|error| (Err(error), (rx, completion, drop_guard)))
                }
            }
        },
    ))
}

/// Upstream's body cap reads the declared length: absent with no
/// `Transfer-Encoding` is an empty body, unreadable or chunked is a 411.
fn check_declared_length(headers: &HeaderMap) -> Result<(), ApiError> {
    let length = match headers.get(header::CONTENT_LENGTH) {
        Some(v) => v.to_str().ok().and_then(|v| v.parse::<u64>().ok()),
        None if headers.contains_key(header::TRANSFER_ENCODING) => None,
        None => Some(0),
    };
    match length {
        None => Err(ApiError::LengthRequired(
            "Requests with a body must include a valid Content-Length header".to_string(),
        )),
        Some(n) if n > MAX_BODY_BYTES => Err(ApiError::PayloadTooLarge(format!(
            "Request body exceeds the maximum allowed size ({MAX_BODY_BYTES} bytes)"
        ))),
        Some(_) => Ok(()),
    }
}

fn refusal_to_api(refusal: Refusal) -> ApiError {
    match refusal {
        Refusal::SceneBusy => ApiError::TooManyRequests {
            message: "An export of this scene is already in progress; try again shortly".into(),
            retry_after_secs: None,
        },
        Refusal::PodBusy => ApiError::TooManyRequests {
            message: "Too many exports are in progress; try again shortly".into(),
            retry_after_secs: None,
        },
        Refusal::Cooldown { retry_after_secs } => ApiError::TooManyRequests {
            message: format!(
                "This scene was exported moments ago; try again in {retry_after_secs} seconds"
            ),
            retry_after_secs: Some(retry_after_secs),
        },
    }
}

/// Upstream's middleware order: the signed fetch and the scene context, the
/// body cap, then authorization; the handler parses the format and the body.
pub async fn create_export(
    State(state): State<SharedState>,
    req: Request,
) -> Result<Response, ApiError> {
    let (parts, body_in) = req.into_parts();
    let ctx = resolve_authorized_checking_body(
        &state,
        &parts.headers,
        "post",
        &parts.uri,
        parts.extensions.get::<OriginalUri>().map(|o| &o.0),
        AuthPolicy::EXPORT,
        &check_declared_length,
    )
    .await?;
    let format = parse_format(parts.uri.query())?;
    let bytes = axum::body::to_bytes(body_in, MAX_BODY_BYTES as usize)
        .await
        .map_err(|_| {
            ApiError::PayloadTooLarge(format!(
                "Request body exceeds the maximum allowed size ({MAX_BODY_BYTES} bytes)"
            ))
        })?;
    let selection = parse_selection(&bytes)?;
    let scene = format!(
        "{}:{}",
        ctx.namespace.realm(),
        ctx.namespace.scene_namespace()
    );
    let slot = state.exports.open(&scene).map_err(|refusal| {
        refusal.log(&ctx.namespace);
        refusal_to_api(refusal)
    })?;
    tracing::info!(
        realm = ctx.namespace.realm(),
        namespace = ctx.namespace.scene_namespace(),
        format = format.extension(),
        "storage export started"
    );
    let content_type = match format {
        Format::Json => "application/json; charset=utf-8",
        Format::Csv => "text/csv; charset=utf-8",
    };
    let disposition = format!(
        "attachment; filename=\"storage-export.{}\"",
        format.extension()
    );
    Ok((
        [
            (header::CONTENT_TYPE, HeaderValue::from_static(content_type)),
            (header::CONTENT_ENCODING, HeaderValue::from_static("gzip")),
            (
                header::CONTENT_DISPOSITION,
                HeaderValue::from_str(&disposition).expect("ascii"),
            ),
            (header::CACHE_CONTROL, HeaderValue::from_static("no-store")),
        ],
        body(
            state.pool.clone(),
            ctx.namespace,
            selection,
            format,
            slot,
            state.exports.settings(),
        ),
    )
        .into_response())
}

#[cfg(test)]
mod lifecycle_tests {
    use super::*;

    #[test]
    fn admission_starts_the_cooldown_even_when_the_export_fails_later() {
        let limits = ExportLimits::new(ExportSettings {
            max_concurrent_per_scene: 3,
            ..ExportSettings::default()
        });
        let first = limits.open("scene").unwrap();
        // A concurrent caller under the scene cap gets the cooldown, as upstream's.
        match limits.open("scene") {
            Err(Refusal::Cooldown { retry_after_secs }) => {
                assert!((1..=10).contains(&retry_after_secs))
            }
            Err(other) => panic!("expected the cooldown, got {other:?}"),
            Ok(_) => panic!("expected the cooldown"),
        }
        // A start that failed still consumed it.
        drop(first);
        assert!(matches!(
            limits.open("scene"),
            Err(Refusal::Cooldown { .. })
        ));
        assert!(limits.open("other").is_ok());
        limits
            .counters
            .lock()
            .last_start
            .insert("scene".into(), Instant::now() - Duration::from_secs(20));
        assert!(limits.open("scene").is_ok());
    }

    #[test]
    fn disabled_cooldown_allows_concurrent_exports_up_to_the_caps() {
        let limits = ExportLimits::new(ExportSettings {
            max_concurrent_per_scene: 2,
            scene_cooldown: Duration::ZERO,
            ..ExportSettings::default()
        });
        let first = limits.open("scene").unwrap();
        let _second = limits.open("scene").unwrap();
        assert!(matches!(limits.open("scene"), Err(Refusal::SceneBusy)));
        drop(first);
        assert!(limits.open("scene").is_ok());
    }

    #[test]
    fn zero_caps_refuse_every_export_as_upstream() {
        let scene = ExportLimits::new(ExportSettings {
            max_concurrent_per_scene: 0,
            ..ExportSettings::default()
        });
        assert!(matches!(scene.open("scene"), Err(Refusal::SceneBusy)));
        let pod = ExportLimits::new(ExportSettings {
            max_concurrent_per_pod: 0,
            ..ExportSettings::default()
        });
        assert!(matches!(pod.open("scene"), Err(Refusal::PodBusy)));
        // Neither refusal started a cooldown.
        assert!(pod.counters.lock().last_start.is_empty());
    }

    fn genesis_namespace() -> SceneNamespace {
        SceneNamespace::new(
            &crate::comms::world_identity::RealmId::parse("main").unwrap(),
            crate::comms::world_identity::Parcel::parse("0,0").unwrap(),
            None,
        )
        .unwrap()
    }

    /// Nothing listens on port 1: acquisition fails after the headers.
    fn unreachable_pool() -> PgPool {
        sqlx::postgres::PgPoolOptions::new()
            .acquire_timeout(Duration::from_secs(2))
            .connect_lazy("postgres://postgres@127.0.0.1:1/unused")
            .unwrap()
    }

    #[tokio::test]
    async fn a_database_failure_after_admission_cuts_the_stream_and_keeps_the_cooldown() {
        // An unrepresentable timeout sets no deadline; the failure is the connection's.
        for timeout in [Duration::MAX, Duration::ZERO] {
            let settings = ExportSettings {
                timeout,
                ..ExportSettings::default()
            };
            let limits = ExportLimits::new(settings);
            let body = body(
                unreachable_pool(),
                genesis_namespace(),
                Selection::default(),
                Format::Json,
                limits.open("scene").unwrap(),
                settings,
            );
            assert!(
                axum::body::to_bytes(body, 1 << 16).await.is_err(),
                "{timeout:?}"
            );
            assert!(matches!(
                limits.open("scene"),
                Err(Refusal::Cooldown { .. })
            ));
            assert_eq!(limits.counters.lock().open, 0, "the slot was released");
        }
    }

    #[test]
    fn player_addresses_are_lowercased_but_never_trimmed() {
        let ok = parse_selection(
            br#"{"players":{"addresses":["0xABCDEFabcdefABCDEFabcdefABCDEFabcdefABCD"]}}"#,
        )
        .unwrap();
        assert_eq!(
            ok.player_addresses.unwrap(),
            ["0xabcdefabcdefabcdefabcdefabcdefabcdefabcd"]
        );
        for padded in [
            " 0xabcdefabcdefabcdefabcdefabcdefabcdefabcd",
            "0xabcdefabcdefabcdefabcdefabcdefabcdefabcd\n",
        ] {
            let body = serde_json::json!({ "players": { "addresses": [padded] } }).to_string();
            match parse_selection(body.as_bytes()) {
                Err(ApiError::BadRequest(message)) => assert_eq!(message, "Invalid player address"),
                other => panic!("{padded:?}: {other:?}"),
            }
        }
    }

    #[tokio::test]
    async fn closed_data_channel_waits_for_the_producer_verdict() {
        let (tx, rx) = mpsc::channel(1);
        let (done, completion) = oneshot::channel();
        let cancel = CancellationToken::new();
        let body = response_body(rx, completion, cancel.drop_guard());
        tx.send(Bytes::from_static(b"partial gzip")).await.unwrap();
        drop(tx);
        let reader = tokio::spawn(axum::body::to_bytes(body, 1024));
        tokio::task::yield_now().await;
        assert!(!reader.is_finished());
        done.send(Err(std::io::Error::other("failed after channel close")))
            .unwrap();
        assert!(reader.await.unwrap().is_err());
    }

    #[tokio::test]
    async fn producer_panic_is_a_body_error() {
        let (tx, rx) = mpsc::channel(1);
        let (done, completion) = oneshot::channel();
        let producer = tokio::spawn(async move {
            let (_tx, _done) = (tx, done);
            panic!("injected export panic");
        });
        assert!(producer.await.unwrap_err().is_panic());
        let body = response_body(rx, completion, CancellationToken::new().drop_guard());
        assert!(axum::body::to_bytes(body, 1024).await.is_err());
    }

    #[tokio::test]
    async fn dropping_an_unpolled_body_cancels_the_producer() {
        let (_tx, rx) = mpsc::channel(1);
        let (_done, completion) = oneshot::channel();
        let cancel = CancellationToken::new();
        drop(response_body(rx, completion, cancel.clone().drop_guard()));
        assert!(cancel.is_cancelled());
    }
}
