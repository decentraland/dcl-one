//! The server the page under test talks to: the web app's files, and under `/__perf/<token>` the
//! stream the page sends its run on. It listens on loopback only.

use std::convert::Infallible;
use std::io;
use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use bytes::Bytes;
use futures_util::stream;
use http_body_util::{combinators::UnsyncBoxBody, BodyExt, Full, Limited, StreamBody};
use hyper::body::{Body as _, Frame, Incoming};
use hyper::header::{
    HeaderMap, HeaderValue, ACCEPT_RANGES, CACHE_CONTROL, CONTENT_LENGTH, CONTENT_RANGE,
    CONTENT_TYPE, HOST, RANGE,
};
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Method, Request, Response, StatusCode};
use hyper_util::rt::{TokioIo, TokioTimer};
use tokio::io::{AsyncReadExt, AsyncSeekExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Notify;
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;

use crate::access::Access;
use crate::app_source::is_device_name;
use crate::byte_stream::{AppendError, Appended, ByteStream};
use crate::limits::{Limits, BODY_IDLE, REQUEST_HEADERS};
use crate::out;

type Body = UnsyncBoxBody<Bytes, io::Error>;

/// How long the server gives open connections to finish when the run is over.
const GRACE: Duration = Duration::from_secs(3);

/// The page that finishes the profile reset (see `profile` and the benchmark guideline): the
/// folders a page can leave code in are already gone from disk; this clears what only a page can,
/// in the origin every build shares. Local storage keeps one key, the local guest's root key, so
/// every run is the same account.
const RESET_PAGE: &str = include_str!("reset.html");

/// What the page says in `end`: its exit code and, from a page that reports them, the bytes it
/// sent on each stream and whether it gave one up (a page that does not leaves them out).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EndReport {
    pub exit_code: i32,
    pub csv_bytes: Option<u64>,
    pub log_bytes: Option<u64>,
    pub failed: bool,
}

/// `end`, once: the first report is the run's.
#[derive(Default)]
pub struct EndSignal {
    report: Mutex<Option<EndReport>>,
    arrived: Notify,
}

impl EndSignal {
    fn set(&self, report: EndReport) {
        let mut slot = self.report.lock().expect("end report lock");
        if slot.is_none() {
            *slot = Some(report);
            self.arrived.notify_waiters();
        }
    }

    pub fn get(&self) -> Option<EndReport> {
        self.report.lock().expect("end report lock").clone()
    }

    pub async fn wait(&self) {
        loop {
            let notified = self.arrived.notified();
            tokio::pin!(notified);
            // Registered before the check, so an `end` in between is not missed.
            notified.as_mut().enable();
            if self.get().is_some() {
                return;
            }
            notified.await;
        }
    }
}

/// Requests for a file the web app does not have. The page under test decides how many it makes,
/// so the first few are logged and the rest only counted.
#[derive(Default)]
pub struct Missing(AtomicU32);

/// What a miss adds to the job log.
#[derive(Debug, PartialEq, Eq)]
enum Notice {
    Path,
    Cutoff,
    Nothing,
}

impl Missing {
    /// Misses whose path is logged.
    const LOGGED: u32 = 5;

    fn notice(before: u32) -> Notice {
        match before {
            n if n < Self::LOGGED => Notice::Path,
            Self::LOGGED => Notice::Cutoff,
            _ => Notice::Nothing,
        }
    }

    fn record(&self, path: &str) {
        match Self::notice(self.0.fetch_add(1, Ordering::Relaxed)) {
            Notice::Path => out::log(format!("404 {}", out::safe(path))),
            Notice::Cutoff => out::log("more 404s follow; they are counted, not logged"),
            Notice::Nothing => {}
        }
    }

    pub fn total(&self) -> u32 {
        self.0.load(Ordering::Relaxed)
    }
}

/// Everything a request handler reaches.
pub struct Shared {
    pub access: Access,
    pub csv: ByteStream,
    pub log: ByteStream,
    pub end: EndSignal,
    pub missing: Missing,
    pub root: PathBuf,
    pub limits: Limits,
}

/// The loopback sockets, bound but not yet serving: the run's token and the `Host` names it
/// accepts depend on the port that was bound.
pub struct Listeners {
    v4: TcpListener,
    v6: Option<TcpListener>,
}

/// Binds `127.0.0.1` and `::1` on `port` (0: any free port). A machine without IPv6 loopback
/// serves the first only.
pub async fn bind(port: u16) -> io::Result<Listeners> {
    let v4 = TcpListener::bind((Ipv4Addr::LOCALHOST, port)).await?;
    let bound = v4.local_addr()?.port();
    let v6 = match TcpListener::bind((Ipv6Addr::LOCALHOST, bound)).await {
        Ok(listener) => Some(listener),
        Err(error) if error.kind() == io::ErrorKind::AddrInUse && port != 0 => return Err(error),
        Err(_) => None,
    };
    Ok(Listeners { v4, v6 })
}

impl Listeners {
    pub fn port(&self) -> u16 {
        self.v4.local_addr().map(|a| a.port()).unwrap_or(0)
    }

    pub fn addresses(&self) -> Vec<SocketAddr> {
        [Some(&self.v4), self.v6.as_ref()]
            .into_iter()
            .flatten()
            .filter_map(|listener| listener.local_addr().ok())
            .collect()
    }

    pub fn serve(self, shared: Arc<Shared>) -> Server {
        let shutdown = CancellationToken::new();
        let tasks = [Some(self.v4), self.v6]
            .into_iter()
            .flatten()
            .map(|listener| tokio::spawn(accept(listener, shared.clone(), shutdown.clone())))
            .collect();
        Server { shutdown, tasks }
    }
}

pub struct Server {
    shutdown: CancellationToken,
    tasks: Vec<tokio::task::JoinHandle<()>>,
}

impl Server {
    /// Stops accepting and gives the open connections a few seconds.
    pub async fn stop(self) {
        self.shutdown.cancel();
        for task in self.tasks {
            let _ = task.await;
        }
    }
}

async fn accept(listener: TcpListener, shared: Arc<Shared>, shutdown: CancellationToken) {
    let mut connections = JoinSet::new();
    loop {
        tokio::select! {
            _ = shutdown.cancelled() => break,
            accepted = listener.accept() => match accepted {
                // Loopback sockets only ever see loopback peers; check anyway.
                Ok((stream, peer)) if peer.ip().is_loopback() => {
                    connections.spawn(connection(stream, shared.clone(), shutdown.clone()));
                }
                Ok(_) => {}
                // Out of file handles, say: wait for some to close.
                Err(_) => tokio::time::sleep(Duration::from_millis(20)).await,
            },
            Some(_) = connections.join_next(), if !connections.is_empty() => {}
        }
    }
    let _ = tokio::time::timeout(GRACE, async {
        while connections.join_next().await.is_some() {}
    })
    .await;
    connections.abort_all();
}

async fn connection(stream: TcpStream, shared: Arc<Shared>, shutdown: CancellationToken) {
    let service = service_fn(move |request| {
        let shared = shared.clone();
        async move { Ok::<_, Infallible>(handle(&shared, request).await) }
    });
    let served = http1::Builder::new()
        .timer(TokioTimer::new())
        .header_read_timeout(REQUEST_HEADERS)
        .max_buf_size(64 * 1024)
        .serve_connection(TokioIo::new(stream), service);
    tokio::pin!(served);

    let mut stopping = false;
    loop {
        tokio::select! {
            _ = served.as_mut() => return,
            _ = shutdown.cancelled(), if !stopping => {
                stopping = true;
                served.as_mut().graceful_shutdown();
            }
        }
    }
}

async fn handle(shared: &Shared, request: Request<Incoming>) -> Response<Body> {
    let mut response = route(shared, request).await;

    // The cross-origin isolation headers the web app needs on every response. no-store keeps
    // Chrome's HTTP cache (and its code cache) from handing one build's files to the next build
    // served on this origin.
    let headers = response.headers_mut();
    for (name, value) in [
        ("cross-origin-opener-policy", "same-origin"),
        ("cross-origin-embedder-policy", "require-corp"),
        ("cross-origin-resource-policy", "cross-origin"),
        // No other site frames a page of this origin (a page on another origin could otherwise
        // start an unattended session in a frame). The web app is itself a shell page that
        // embeds its app page from this origin, so the origin's own pages may frame each other:
        // `'self'`, not `'none'`.
        ("content-security-policy", "frame-ancestors 'self'"),
        ("x-frame-options", "SAMEORIGIN"),
    ] {
        headers.insert(name, HeaderValue::from_static(value));
    }
    headers.insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

async fn route(shared: &Shared, request: Request<Incoming>) -> Response<Body> {
    if !shared.access.host_allowed(single_host(request.headers())) {
        shared.access.refuse();
        return status(StatusCode::MISDIRECTED_REQUEST);
    }

    let path = request.uri().path().to_string();
    if path.starts_with("/__perf/") {
        perf(shared, request, &path).await
    } else {
        static_file(shared, &request, &path).await
    }
}

/// The `Host` header, when there is exactly one.
fn single_host(headers: &HeaderMap) -> Option<&str> {
    let mut hosts = headers.get_all(HOST).iter();
    match (hosts.next(), hosts.next()) {
        (Some(host), None) => host.to_str().ok(),
        _ => None,
    }
}

async fn perf(shared: &Shared, request: Request<Incoming>, path: &str) -> Response<Body> {
    // /__perf/<token>/<what>: a request without this run's token is answered as if nothing were
    // here.
    let parts: Vec<&str> = path["/__perf/".len()..].split('/').collect();
    if parts.len() != 2 || !shared.access.token_matches(parts[0]) {
        shared.access.refuse();
        return status(StatusCode::NOT_FOUND);
    }

    let kind = parts[1];
    let (head, mut body) = request.into_parts();
    let method = head.method;

    if kind == "reset" && method == Method::GET {
        return html(RESET_PAGE);
    }

    if kind == "reset-done" && method == Method::POST {
        return match read_control_body(&head.headers, body, shared.limits.control_bytes).await {
            Ok(summary) => {
                shared.access.report_reset();
                out::log(format!(
                    "profile reset page ran: {}",
                    out::safe(&String::from_utf8_lossy(&summary))
                ));
                status(StatusCode::NO_CONTENT)
            }
            Err(response) => *response,
        };
    }

    if !matches!(kind, "csv" | "log" | "end") {
        return status(StatusCode::NOT_FOUND);
    }

    if method != Method::POST {
        return status(StatusCode::METHOD_NOT_ALLOWED);
    }

    if kind == "end" {
        // A body of `text/plain` is what a cross-site form or no-cors fetch can send without
        // asking; `application/json` makes the browser ask first (and the server answers no
        // preflight).
        if !is_json(&head.headers) {
            return reply(
                StatusCode::UNSUPPORTED_MEDIA_TYPE,
                "`end` takes Content-Type: application/json",
            );
        }

        let payload =
            match read_control_body(&head.headers, body, shared.limits.control_bytes).await {
                Ok(payload) => payload,
                Err(response) => return *response,
            };

        let Some(report) = parse_end(&payload) else {
            return reply(
                StatusCode::BAD_REQUEST,
                "`end` takes {\"exitCode\": n} and may add \"csvBytes\": n, \"logBytes\": n, \"failed\": bool",
            );
        };

        // Nothing is accepted after `end`.
        shared.csv.close().await;
        shared.log.close().await;
        shared.end.set(report);
        return status(StatusCode::NO_CONTENT);
    }

    let stream = if kind == "csv" {
        &shared.csv
    } else {
        &shared.log
    };

    let offset = match head
        .headers
        .get_all("x-perf-offset")
        .iter()
        .collect::<Vec<_>>()[..]
    {
        [value] => value.to_str().ok().and_then(parse_whole_number),
        _ => None,
    };
    let Some(offset) = offset else {
        return reply(
            StatusCode::BAD_REQUEST,
            "X-Perf-Offset must be the byte offset of the body's first byte",
        );
    };

    // The length is known up front (a fetch with a byte body sends it), so the limits apply
    // before a byte is read and the body goes to the file as it arrives.
    let Some(length) = body.size_hint().exact() else {
        return reply(
            StatusCode::LENGTH_REQUIRED,
            "the body needs a Content-Length",
        );
    };

    if length > shared.limits.chunk_bytes {
        return reply(
            StatusCode::PAYLOAD_TOO_LARGE,
            &format!(
                "a request holds at most {} bytes",
                shared.limits.chunk_bytes
            ),
        );
    }

    let appended = stream.append(offset, &mut body, length).await;
    let stored = stream.length().await;

    let mut response = match appended {
        Ok(Appended::Stored) => status(StatusCode::NO_CONTENT),
        Ok(Appended::Gap) => {
            out::log(format!(
                "{kind}: refused offset {offset}, only {stored} B stored (gap)"
            ));
            reply(
                StatusCode::CONFLICT,
                &format!("gap: {stored} bytes stored, the request starts at {offset}"),
            )
        }
        Ok(Appended::Mismatch) => {
            out::log(format!(
                "{kind}: refused offset {offset}, the bytes differ from those stored"
            ));
            reply(
                StatusCode::CONFLICT,
                "the bytes overlapping what is stored differ from it",
            )
        }
        Ok(Appended::TooLarge) => {
            out::log(format!(
                "{kind}: refused {length} B at {offset}, a file holds at most {} B",
                shared.limits.file_bytes
            ));
            reply(
                StatusCode::PAYLOAD_TOO_LARGE,
                &format!("the file holds at most {} bytes", shared.limits.file_bytes),
            )
        }
        Ok(Appended::Ended) => reply(StatusCode::CONFLICT, "the run has ended"),
        Err(AppendError::Body(_)) => {
            reply(StatusCode::BAD_REQUEST, "the body did not arrive whole")
        }
        Err(AppendError::Idle) => reply(StatusCode::REQUEST_TIMEOUT, "the body stopped arriving"),
        Err(AppendError::Disk(error)) => {
            out::log(format!(
                "{kind}: cannot store the bytes ({})",
                out::safe(&error.to_string())
            ));
            reply(
                StatusCode::INTERNAL_SERVER_ERROR,
                "the bytes could not be stored",
            )
        }
    };
    response.headers_mut().insert(
        "x-perf-length",
        HeaderValue::from_str(&stored.to_string()).expect("digits are a header value"),
    );
    response
}

/// An unsigned whole number of digits only: no sign, no white space.
fn parse_whole_number(text: &str) -> Option<u64> {
    if text.is_empty() || !text.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    text.parse::<i64>().ok().map(|n| n as u64)
}

/// A byte total in `end`, absent from a page that does not report it: `Ok(None)` for absent or
/// null, an error for anything but a whole number of at least 0.
fn byte_count(
    root: &serde_json::Map<String, serde_json::Value>,
    name: &str,
) -> Result<Option<u64>, ()> {
    match root.get(name) {
        None | Some(serde_json::Value::Null) => Ok(None),
        Some(value) => match value.as_i64() {
            Some(bytes) if bytes >= 0 => Ok(Some(bytes as u64)),
            _ => Err(()),
        },
    }
}

pub fn parse_end(payload: &[u8]) -> Option<EndReport> {
    let json: serde_json::Value = serde_json::from_slice(payload).ok()?;
    let root = json.as_object()?;
    let exit_code = i32::try_from(root.get("exitCode")?.as_i64()?).ok()?;
    let failed = match root.get("failed") {
        None | Some(serde_json::Value::Null) => false,
        Some(serde_json::Value::Bool(failed)) => *failed,
        Some(_) => return None,
    };
    Some(EndReport {
        exit_code,
        csv_bytes: byte_count(root, "csvBytes").ok()?,
        log_bytes: byte_count(root, "logBytes").ok()?,
        failed,
    })
}

fn is_json(headers: &HeaderMap) -> bool {
    headers
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(';').next())
        .is_some_and(|media| media.trim().eq_ignore_ascii_case("application/json"))
}

/// The body of `end` or a reset route: a few bytes, held in memory. The response to send when it
/// is too long, with or without a declared length, or does not arrive.
async fn read_control_body(
    headers: &HeaderMap,
    body: Incoming,
    limit: u64,
) -> Result<Bytes, Box<Response<Body>>> {
    let too_long = || {
        Box::new(reply(
            StatusCode::PAYLOAD_TOO_LARGE,
            &format!("this route takes at most {limit} bytes"),
        ))
    };

    let declared = headers
        .get(CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok());
    if declared.is_some_and(|declared| declared > limit) {
        return Err(too_long());
    }

    let collected =
        tokio::time::timeout(BODY_IDLE, Limited::new(body, limit as usize).collect()).await;
    match collected {
        Ok(Ok(collected)) => Ok(collected.to_bytes()),
        Ok(Err(error)) if error.is::<http_body_util::LengthLimitError>() => Err(too_long()),
        Ok(Err(_)) => Err(Box::new(reply(
            StatusCode::BAD_REQUEST,
            "the body did not arrive whole",
        ))),
        Err(_) => Err(Box::new(reply(
            StatusCode::REQUEST_TIMEOUT,
            "the body stopped arriving",
        ))),
    }
}

fn status(code: StatusCode) -> Response<Body> {
    let mut response = Response::new(empty());
    *response.status_mut() = code;
    response
}

fn reply(code: StatusCode, text: &str) -> Response<Body> {
    let mut response = Response::new(full(Bytes::from(text.to_string())));
    *response.status_mut() = code;
    response.headers_mut().insert(
        CONTENT_TYPE,
        HeaderValue::from_static("text/plain; charset=utf-8"),
    );
    response
}

fn html(text: &'static str) -> Response<Body> {
    let mut response = Response::new(full(Bytes::from_static(text.as_bytes())));
    response.headers_mut().insert(
        CONTENT_TYPE,
        HeaderValue::from_static("text/html; charset=utf-8"),
    );
    response
}

fn full(bytes: Bytes) -> Body {
    Full::new(bytes)
        .map_err(|never| match never {})
        .boxed_unsync()
}

fn empty() -> Body {
    full(Bytes::new())
}

// ---------------------------------------------------------------------------------------------
// The web app's files.

/// What `GET` of a file answers: the app's files from the root, nothing else on disk.
async fn static_file(shared: &Shared, request: &Request<Incoming>, path: &str) -> Response<Body> {
    let head_only = match *request.method() {
        Method::GET => false,
        Method::HEAD => true,
        _ => return status(StatusCode::METHOD_NOT_ALLOWED),
    };

    let segments = match file_segments(path) {
        Ok(segments) => segments,
        Err(code) => return status(code),
    };
    let mut full_path = shared.root.clone();
    full_path.extend(&segments);

    let Some((file, length)) = open_file(&full_path).await else {
        shared.missing.record(path);
        return status(StatusCode::NOT_FOUND);
    };

    let mut response = Response::new(empty());
    let headers = response.headers_mut();
    headers.insert(ACCEPT_RANGES, HeaderValue::from_static("bytes"));
    headers.insert(
        CONTENT_TYPE,
        HeaderValue::from_str(&content_type(&full_path)).expect("content types are ASCII"),
    );

    let range = request
        .headers()
        .get(RANGE)
        .and_then(|value| value.to_str().ok())
        .map(|value| parse_range(value, length))
        .unwrap_or(RangeRequest::Whole);

    let (start, count) = match range {
        RangeRequest::Whole => (0, length),
        RangeRequest::Partial { start, end } => {
            *response.status_mut() = StatusCode::PARTIAL_CONTENT;
            response.headers_mut().insert(
                CONTENT_RANGE,
                HeaderValue::from_str(&format!("bytes {start}-{end}/{length}"))
                    .expect("digits are a header value"),
            );
            (start, end - start + 1)
        }
        RangeRequest::Unsatisfiable => {
            *response.status_mut() = StatusCode::RANGE_NOT_SATISFIABLE;
            response.headers_mut().insert(
                CONTENT_RANGE,
                HeaderValue::from_str(&format!("bytes */{length}"))
                    .expect("digits are a header value"),
            );
            return response;
        }
    };

    response.headers_mut().insert(
        CONTENT_LENGTH,
        HeaderValue::from_str(&count.to_string()).expect("digits are a header value"),
    );
    if !head_only {
        *response.body_mut() = stream_file(file, start, count);
    }
    response
}

/// The path segments of a request under the root, or the refusal: percent-decoded, no `..`,
/// backslash, drive or stream colon, control character or device name, so no name leaves the
/// root or reaches a device. A path that ends in a slash, or is empty, names `index.html`.
fn file_segments(path: &str) -> Result<Vec<String>, StatusCode> {
    let decoded = urlencoding::decode(path).map_err(|_| StatusCode::BAD_REQUEST)?;
    if decoded.contains('\\') || decoded.contains(':') || decoded.chars().any(char::is_control) {
        return Err(StatusCode::BAD_REQUEST);
    }

    let mut segments = Vec::new();
    for segment in decoded.split('/') {
        match segment {
            "" | "." => {}
            ".." => return Err(StatusCode::BAD_REQUEST),
            name if is_device_name(name) => return Err(StatusCode::NOT_FOUND),
            name => segments.push(name.to_string()),
        }
    }
    if decoded.ends_with('/') || segments.is_empty() {
        segments.push("index.html".to_string());
    }
    Ok(segments)
}

async fn open_file(path: &Path) -> Option<(tokio::fs::File, u64)> {
    let metadata = tokio::fs::metadata(path).await.ok()?;
    if !metadata.is_file() {
        return None;
    }
    let file = tokio::fs::File::open(path).await.ok()?;
    Some((file, metadata.len()))
}

/// `count` bytes of the file from `start`, a piece at a time.
fn stream_file(file: tokio::fs::File, start: u64, count: u64) -> Body {
    const PIECE: u64 = 64 * 1024;

    let pieces = stream::unfold(
        (file, start, count, false),
        |(mut file, start, remaining, sought)| async move {
            if remaining == 0 {
                return None;
            }
            if !sought {
                if let Err(error) = file.seek(io::SeekFrom::Start(start)).await {
                    return Some((Err(error), (file, start, 0, true)));
                }
            }
            let mut buffer = vec![0u8; remaining.min(PIECE) as usize];
            match file.read(&mut buffer).await {
                // The file is shorter than it was when its length was announced.
                Ok(0) => Some((
                    Err(io::ErrorKind::UnexpectedEof.into()),
                    (file, start, 0, true),
                )),
                Ok(read) => {
                    buffer.truncate(read);
                    Some((
                        Ok(Frame::data(Bytes::from(buffer))),
                        (file, start, remaining - read as u64, true),
                    ))
                }
                Err(error) => Some((Err(error), (file, start, 0, true))),
            }
        },
    );
    StreamBody::new(pieces).boxed_unsync()
}

#[derive(Debug, PartialEq, Eq)]
enum RangeRequest {
    Whole,
    Partial { start: u64, end: u64 },
    Unsatisfiable,
}

/// A `Range: bytes=...` header against a file of `length` bytes. One range is honoured; several,
/// or a header that is not a byte range, get the whole file.
fn parse_range(header: &str, length: u64) -> RangeRequest {
    let Some(spec) = header
        .trim()
        .split_once('=')
        .filter(|(unit, _)| unit.trim().eq_ignore_ascii_case("bytes"))
        .map(|(_, spec)| spec.trim())
    else {
        return RangeRequest::Whole;
    };
    if spec.contains(',') {
        return RangeRequest::Whole;
    }
    let Some((first, last)) = spec.split_once('-') else {
        return RangeRequest::Whole;
    };
    let (first, last) = (first.trim(), last.trim());

    match (first.parse::<u64>(), last.parse::<u64>()) {
        (Ok(start), Ok(end)) if end >= start => {
            if start >= length {
                RangeRequest::Unsatisfiable
            } else {
                RangeRequest::Partial {
                    start,
                    end: end.min(length - 1),
                }
            }
        }
        (Ok(start), _) if last.is_empty() => {
            if start >= length {
                RangeRequest::Unsatisfiable
            } else {
                RangeRequest::Partial {
                    start,
                    end: length - 1,
                }
            }
        }
        (_, Ok(suffix)) if first.is_empty() => {
            if suffix == 0 || length == 0 {
                RangeRequest::Unsatisfiable
            } else {
                RangeRequest::Partial {
                    start: length.saturating_sub(suffix),
                    end: length - 1,
                }
            }
        }
        _ => RangeRequest::Whole,
    }
}

/// The type a file is served as: the web's own (`.js` and `.wasm` need theirs for the page to
/// run), text with its charset.
fn content_type(path: &Path) -> String {
    let extension = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let known = match extension.as_str() {
        "html" | "htm" => "text/html",
        "js" | "mjs" => "text/javascript",
        "css" => "text/css",
        "json" | "map" => "application/json",
        "webmanifest" => "application/manifest+json",
        "wasm" => "application/wasm",
        "txt" | "wat" | "md" => "text/plain",
        "xml" => "application/xml",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "avif" => "image/avif",
        "ico" => "image/x-icon",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "ttf" => "font/ttf",
        "otf" => "font/otf",
        "mp3" => "audio/mpeg",
        "ogg" | "oga" => "audio/ogg",
        "wav" => "audio/wav",
        "mp4" => "video/mp4",
        "webm" => "video/webm",
        "glb" => "model/gltf-binary",
        "gltf" => "model/gltf+json",
        "pdf" => "application/pdf",
        "zip" => "application/zip",
        _ => "application/octet-stream",
    };
    if known.starts_with("text/") || known.ends_with("json") || known.ends_with("+xml") {
        format!("{known}; charset=utf-8")
    } else {
        known.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offsets_are_digits_only() {
        assert_eq!(parse_whole_number("0"), Some(0));
        assert_eq!(parse_whole_number("123"), Some(123));
        for bad in [
            "",
            "-1",
            "+1",
            " 1",
            "1 ",
            "1.5",
            "1,2",
            "9223372036854775808",
            "٣",
        ] {
            assert_eq!(parse_whole_number(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn end_takes_an_exit_code_and_optional_totals() {
        let report =
            parse_end(br#"{"exitCode": 101, "csvBytes": 8, "logBytes": 6, "failed": true}"#);
        assert_eq!(
            report,
            Some(EndReport {
                exit_code: 101,
                csv_bytes: Some(8),
                log_bytes: Some(6),
                failed: true
            })
        );
        assert_eq!(
            parse_end(br#"{"exitCode": 0}"#),
            Some(EndReport {
                exit_code: 0,
                csv_bytes: None,
                log_bytes: None,
                failed: false
            })
        );
        assert_eq!(
            parse_end(br#"{"exitCode": 0, "csvBytes": null, "failed": null}"#),
            Some(EndReport {
                exit_code: 0,
                csv_bytes: None,
                log_bytes: None,
                failed: false
            })
        );
    }

    #[test]
    fn end_refuses_what_is_not_the_shape() {
        for bad in [
            &br"{}"[..],
            br#"{"exitCode": "0"}"#,
            br#"{"exitCode": 1.5}"#,
            br#"{"exitCode": 3000000000}"#,
            br#"{"exitCode": 0, "csvBytes": "8"}"#,
            br#"{"exitCode": 0, "logBytes": -1}"#,
            br#"{"exitCode": 0, "csvBytes": 1.5}"#,
            br#"{"exitCode": 0, "failed": "no"}"#,
            br#"[0]"#,
            br#"0"#,
            b"",
            b"not json",
        ] {
            assert_eq!(parse_end(bad), None, "{}", String::from_utf8_lossy(bad));
        }
    }

    #[test]
    fn only_the_first_missing_files_are_logged() {
        let notices: Vec<Notice> = (0..8).map(Missing::notice).collect();
        assert_eq!(
            notices,
            [
                Notice::Path,
                Notice::Path,
                Notice::Path,
                Notice::Path,
                Notice::Path,
                Notice::Cutoff,
                Notice::Nothing,
                Notice::Nothing,
            ]
        );
        assert_eq!(Missing::notice(u32::MAX), Notice::Nothing);

        let missing = Missing::default();
        for _ in 0..20 {
            missing.record("/missing.js");
        }
        assert_eq!(missing.total(), 20);
    }

    #[test]
    fn json_content_type_may_carry_parameters() {
        let with = |value: &'static str| {
            let mut headers = HeaderMap::new();
            headers.insert(CONTENT_TYPE, HeaderValue::from_static(value));
            is_json(&headers)
        };
        assert!(with("application/json"));
        assert!(with("Application/JSON; charset=utf-8"));
        assert!(!with("text/plain"));
        assert!(!with("application/jsonp"));
        assert!(!is_json(&HeaderMap::new()));
    }

    #[test]
    fn file_paths_stay_under_the_root() {
        assert_eq!(file_segments("/").unwrap(), ["index.html"]);
        assert_eq!(file_segments("/app/").unwrap(), ["app", "index.html"]);
        assert_eq!(file_segments("/a/./b//c.js").unwrap(), ["a", "b", "c.js"]);
        assert_eq!(file_segments("/a%20b.js").unwrap(), ["a b.js"]);
        for refused in [
            "/../x",
            "/a/../../x",
            "/%2e%2e/x",
            "/%2E%2E%2Fx",
            "/a%5c..%5cx",
            "/a\\x",
            "/c:/x",
            "/c%3a/x",
            "/index.html:evil",
            "/%00",
            "/%ff",
            "//..//x",
        ] {
            assert_eq!(
                file_segments(refused),
                Err(StatusCode::BAD_REQUEST),
                "{refused}"
            );
        }
        for device in ["/nul", "/dir/CON.txt", "/com1"] {
            assert_eq!(
                file_segments(device),
                Err(StatusCode::NOT_FOUND),
                "{device}"
            );
        }
    }

    #[test]
    fn ranges() {
        use RangeRequest::*;
        assert_eq!(parse_range("bytes=0-3", 10), Partial { start: 0, end: 3 });
        assert_eq!(parse_range("bytes=4-", 10), Partial { start: 4, end: 9 });
        assert_eq!(parse_range("bytes=-3", 10), Partial { start: 7, end: 9 });
        assert_eq!(parse_range("bytes=-30", 10), Partial { start: 0, end: 9 });
        assert_eq!(parse_range("bytes=5-500", 10), Partial { start: 5, end: 9 });
        assert_eq!(parse_range("bytes=10-12", 10), Unsatisfiable);
        assert_eq!(parse_range("bytes=-0", 10), Unsatisfiable);
        assert_eq!(parse_range("bytes=0-1", 0), Unsatisfiable);
        assert_eq!(parse_range("bytes=0-1,4-5", 10), Whole);
        assert_eq!(parse_range("bytes=3-1", 10), Whole);
        assert_eq!(parse_range("items=0-1", 10), Whole);
        assert_eq!(parse_range("bytes=a-b", 10), Whole);
    }

    #[test]
    fn the_web_needs_these_types() {
        let of = |name: &str| content_type(Path::new(name));
        assert_eq!(of("a/index.html"), "text/html; charset=utf-8");
        assert_eq!(of("engine.js"), "text/javascript; charset=utf-8");
        assert_eq!(of("engine.MJS"), "text/javascript; charset=utf-8");
        assert_eq!(of("engine_bg.wasm"), "application/wasm");
        assert_eq!(of("build-info.json"), "application/json; charset=utf-8");
        assert_eq!(of("noextension"), "application/octet-stream");
    }
}
