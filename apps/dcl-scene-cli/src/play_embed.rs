//! The packaged web explorer, carried inside this binary and served at
//! `/play/` on the preview's own origin. build.rs embeds the tree
//! `PLAY_EMBED_DIR` names (play_embed_select.rs decides which files), so
//! `--talk` can open a player that is same-origin with the project API: the
//! HUD's writes then ride the capability token on 127.0.0.1 and no third
//! origin has to be let through the gate. Without `PLAY_EMBED_DIR` the table
//! is empty, `/play/` answers 404 saying so and `--talk` uses the public
//! player.
//!
//! Precompressed files (`.br`) are stored as they are and served with
//! `Content-Encoding: br` to a client that accepts it, inflated once and kept
//! for one that does not (Chrome offers br on https and loopback only, so a
//! LAN address over plain http asks for identity). Text-like files are
//! gzipped at build time and inflated on the fly for a client without gzip.
//!
//! `index.html` is served with one script spliced into its head
//! ([`PlayConfig`]): that this is the embedded copy, and the site whose
//! services the HUD reads. Every `/play/*` response is cross-origin isolated
//! (COOP/COEP, which SharedArrayBuffer and the wasm threads need) and carries
//! an enforced CSP that names the page's inline scripts by hash
//! ([`csp_for`]); the scene worker alone may `eval`, since running a scene is
//! what it is for.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use axum::{
    Router,
    body::{Body, Bytes},
    extract::{Path as AxPath, RawQuery, Request},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Redirect, Response},
    routing::get,
};

#[path = "play_embed_select.rs"]
pub mod select;

/// One embedded file. `raw_len` is the uncompressed size of a gzip-stored
/// file and the stored size of a `.br` one (nothing decodes brotli at build
/// time); `etag` a content hash build.rs computed, quoted on the wire with
/// the coding it went out in (see [`etag_for`]).
pub struct File {
    pub path: &'static str,
    pub bytes: &'static [u8],
    pub encoding: Encoding,
    pub raw_len: usize,
    pub etag: &'static str,
}

/// How `bytes` are stored, which is how they go on the wire when the client
/// accepts that coding.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Encoding {
    Identity,
    Gzip,
    Br,
}

include!(concat!(env!("OUT_DIR"), "/play_embed_data.rs"));

/// The policy every `/play/*` response carries, enforced. The page's inline
/// scripts are named by hash (build.rs computes them from the embedded
/// index.html, [`config_script`] adds its own), so no `'unsafe-inline'`;
/// `'wasm-unsafe-eval'` is what instantiating the engine needs. The one
/// exception is [`csp_for`]'s: the scene worker script gets `'unsafe-eval'`,
/// since a dedicated worker fetched by URL takes the policy of its own
/// response and running scene code is that worker's purpose.
pub const CSP_HEADER: &str = "content-security-policy";

/// One policy out of [`CSP_DIRECTIVES`] (engine/web/csp-policy.mts by
/// way of the docroot's csp.json, the list the nginx front renders too):
/// `{site}` is the configured site — frames may come from this origin and
/// the site alone (the HUD's marketplace frame lives there), not any https
/// origin, since the page is cross-origin isolated and a frame from
/// elsewhere would be an overlay a hostile scene could point at — and
/// `script-src` takes the consumer's grants: `extra` (the scene worker's
/// `'unsafe-eval'`) and the page's inline-script hashes.
fn policy(site: &str, extra: &str, hashes: &[&str]) -> String {
    CSP_DIRECTIVES
        .iter()
        .map(|(name, value)| {
            let mut value = value.replace("{site}", site).trim().to_string();
            if *name == "script-src" {
                if !extra.is_empty() {
                    value.push(' ');
                    value.push_str(extra);
                }
                for hash in hashes {
                    value.push_str(" '");
                    value.push_str(hash);
                    value.push('\'');
                }
            }
            format!("{name} {value}")
        })
        .collect::<Vec<_>>()
        .join("; ")
}

/// The policy for one served path: the scene worker's, or the page's.
pub fn csp_for(path: &str) -> &'static str {
    let p = prepared();
    if path.rsplit('/').next() == Some(SCENE_WORKER) {
        &p.worker_csp
    } else {
        &p.csp
    }
}

/// [`csp_for`] as the header value it goes out as, built once.
fn csp_header_for(path: &str) -> &'static HeaderValue {
    let p = prepared();
    if path.rsplit('/').next() == Some(SCENE_WORKER) {
        &p.worker_csp_header
    } else {
        &p.csp_header
    }
}

fn document_csp(config_hash: &str, site: &str) -> String {
    let hashes: Vec<&str> = INLINE_SCRIPT_HASHES
        .iter()
        .copied()
        .chain([config_hash])
        .collect();
    policy(site, "", &hashes)
}

fn worker_csp(site: &str) -> String {
    policy(site, SCENE_WORKER_SCRIPT_SRC, &[])
}

/// What the page is told before its first script runs, spliced into the
/// embedded `index.html` once per process ([`configure`]): that it is the
/// embedded copy (so it probes no dev server, and the HUD resolves places,
/// events, badges, the camera reel and the marketplace against
/// `__SITE_BASE__` instead of the page's own origin, where this server has
/// no such routes — ui3's data/catalyst/client.ts owns the service paths),
/// and which site that is. The realm stays this origin.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlayConfig {
    /// An http(s) origin, [`DEFAULT_SITE`] by default; anything else falls
    /// back to the default ([`site_origin`]).
    pub site: String,
}

/// The operator's site, fixed at build time from `DCL_SCENE_CLI_SITE` (an
/// http(s) origin such as `https://example.org`). A build that names none
/// has no default site: the embedded HUD's services then resolve against
/// the page's own origin, and `--talk`, the account sign-in and the Creator
/// Hub link need their own URL (`--player`, `DCL_SCENE_CLI_WEB_EXPLORER`,
/// `DCL_SCENE_CLI_TARGET_SERVER`, `DCL_SCENE_CLI_CREATOR_HUB_URL`).
pub const DEFAULT_SITE: Option<&str> = option_env!("DCL_SCENE_CLI_SITE");

impl Default for PlayConfig {
    fn default() -> Self {
        Self {
            site: DEFAULT_SITE.unwrap_or_default().to_string(),
        }
    }
}

/// [`DEFAULT_SITE`] as an origin, when the build names a valid one.
pub fn default_site() -> Option<String> {
    DEFAULT_SITE.and_then(origin_of)
}

/// The operator's player page, `<site>/play`, when the build names a site.
pub fn site_player() -> Option<String> {
    default_site().map(|site| format!("{site}/play"))
}

/// `site` as the http(s) origin it names, [`default_site`] when it is not
/// one, else empty: the value goes into a header and a script, so it is an
/// origin (scheme, host, port; no path, no quote, no `<`) or nothing.
pub fn site_origin(site: &str) -> String {
    origin_of(site).or_else(default_site).unwrap_or_default()
}

fn origin_of(site: &str) -> Option<String> {
    // The url crate lets `"` and `;` through as host characters; a `;`
    // would end a CSP directive, so the host is held to a hostname's
    // alphabet (an IPv6 literal's brackets and colons included).
    let plain_host = |h: &str| {
        h.bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b':' | b'[' | b']'))
    };
    url::Url::parse(site.trim())
        .ok()
        .filter(|u| matches!(u.scheme(), "http" | "https"))
        .filter(|u| u.host_str().is_some_and(plain_host))
        .map(|u| u.origin().ascii_serialization())
}

/// `s` as a JavaScript string literal that cannot close the `<script>` it
/// sits in: JSON quoting, with `<` written as `\u003c`.
fn js_string(s: &str) -> String {
    serde_json::to_string(s)
        .expect("a string")
        .replace('<', "\\u003c")
}

/// The head script for one configuration; its CSP hash goes in the policy.
/// The zod line takes the place of `'unsafe-eval'`: zod's object fast path
/// compiles validators with `new Function` after probing `Function("")`
/// once, and under this policy the probe is refused and logged; set before
/// any module runs (the same line catalyrst/sites puts in its head), zod
/// takes the interpreted path outright, here and nowhere else.
pub fn config_script(config: &PlayConfig) -> String {
    format!(
        "window.__DCL_PLAY_EMBEDDED__=true;window.__SITE_BASE__={};\
         globalThis.__zod_globalConfig={{...globalThis.__zod_globalConfig,jitless:true}};",
        js_string(&site_origin(&config.site)),
    )
}

static CONFIG: OnceLock<PlayConfig> = OnceLock::new();
static PREPARED: OnceLock<Prepared> = OnceLock::new();

/// Sets this process's [`PlayConfig`], before the first `/play/*` request;
/// the default applies when nothing was set. Once the page has been
/// prepared or a config set, a later call is refused and hands its argument
/// back as the Err.
pub fn configure(config: PlayConfig) -> Result<(), PlayConfig> {
    CONFIG.set(config)
}

/// The page as served — the embedded index.html with the config script in
/// its head — in both codings, its ETag, and the two policies (as text for
/// the tests, as header values for the wire).
struct Prepared {
    html: Bytes,
    gz: Bytes,
    etag: String,
    csp: String,
    worker_csp: String,
    csp_header: HeaderValue,
    worker_csp_header: HeaderValue,
}

fn prepared() -> &'static Prepared {
    PREPARED.get_or_init(|| {
        let config = CONFIG.get_or_init(PlayConfig::default);
        let site = site_origin(&config.site);
        let script = config_script(config);
        let raw = find("index.html")
            .map(|file| unpacked(file).unwrap_or_else(|why| panic!("index.html: {why}")))
            .unwrap_or_default();
        let html = splice_head(&String::from_utf8_lossy(&raw), &script);
        let etag = {
            let mut h: u64 = 0xcbf29ce484222325;
            fnv1a64(&mut h, b"index.html");
            fnv1a64(&mut h, html.as_bytes());
            format!("{h:016x}")
        };
        let gz = {
            use std::io::Write;
            let mut e = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::best());
            e.write_all(html.as_bytes()).expect("gzip");
            e.finish().expect("gzip")
        };
        let csp = document_csp(&select::csp_sha256(&script), &site);
        let worker_csp = worker_csp(&site);
        let header = |policy: &str| HeaderValue::from_str(policy).expect("an ASCII policy");
        Prepared {
            html: Bytes::from(html),
            gz: Bytes::from(gz),
            etag,
            csp_header: header(&csp),
            worker_csp_header: header(&worker_csp),
            csp,
            worker_csp,
        }
    })
}

/// `script` as the first thing in `<head>` (attributes and case allowed),
/// where a dev build puts its build id too; first thing in `<html>` when the
/// page has no head; at the very start when it has neither. Never ahead of
/// a doctype, which would put the page in quirks mode.
fn splice_head(html: &str, script: &str) -> String {
    let tag = format!("<script>{script}</script>");
    match open_tag_end(html, "head").or_else(|| open_tag_end(html, "html")) {
        Some(end) => format!("{}{tag}{}", &html[..end], &html[end..]),
        None => format!("{tag}{html}"),
    }
}

/// The index just past the first `<name …>` start tag, whatever its case
/// and attributes; `<header>` is not `<head>`.
fn open_tag_end(html: &str, name: &str) -> Option<usize> {
    let lower = html.to_ascii_lowercase();
    let needle = format!("<{name}");
    let mut from = 0;
    while let Some(rel) = lower[from..].find(&needle) {
        let after = from + rel + needle.len();
        match lower[after..].chars().next() {
            Some('>') => return Some(after + 1),
            Some(c) if c.is_ascii_whitespace() || c == '/' => {
                return lower[after..].find('>').map(|g| after + g + 1);
            }
            _ => from = after,
        }
    }
    None
}

fn fnv1a64(h: &mut u64, bytes: &[u8]) {
    for b in bytes {
        *h ^= u64::from(*b);
        *h = h.wrapping_mul(0x100000001b3);
    }
}

const PERMISSIONS: &str = "microphone=(self), camera=(self), clipboard-write=(self), autoplay=(self), \
    fullscreen=(self), pointer-lock=(self), xr-spatial-tracking=(self), geolocation=(), payment=(), \
    usb=(), serial=(), bluetooth=()";

/// On every `/play/*` response, the 404 and the redirect included; the CSP
/// rides beside them ([`csp_for`]).
pub const ISOLATION_HEADERS: &[(&str, &str)] = &[
    ("cross-origin-opener-policy", "same-origin"),
    ("cross-origin-embedder-policy", "credentialless"),
    ("cross-origin-resource-policy", "same-origin"),
    ("permissions-policy", PERMISSIONS),
];

pub const NOT_EMBEDDED: &str = "this dcl-scene-cli build carries no web explorer (built without PLAY_EMBED_DIR): \
    use a hosted player (--player or DCL_SCENE_CLI_WEB_EXPLORER) or rebuild with PLAY_EMBED_DIR=<packaged web dir>\n";

pub fn present() -> bool {
    !FILES.is_empty()
}

/// What the table adds to the binary, as stored (gzip and br sizes).
pub fn embedded_bytes() -> usize {
    FILES.iter().map(|f| f.bytes.len()).sum()
}

/// The engine revision the embedded player was built from, out of its
/// provenance.json.
pub fn engine_rev() -> Option<String> {
    serde_json::from_str::<serde_json::Value>(PROVENANCE)
        .ok()?
        .get("rev")?
        .as_str()
        .map(str::to_string)
}

pub fn find(path: &str) -> Option<&'static File> {
    FILES.iter().find(|f| f.path == path)
}

/// `http://127.0.0.1:{port}/play/`: the player `--talk` prefers, when the
/// binary carries one. Loopback and not a LAN address on purpose: WebGPU and
/// the microphone need a secure context, which plain http has only there.
pub fn local_player(port: u16) -> Option<String> {
    present().then(|| format!("http://127.0.0.1:{port}/play/"))
}

pub fn routes() -> Router {
    Router::new()
        .route("/play", get(play_root))
        .route("/play/", get(index))
        .route("/play/{*path}", get(file))
        .layer(middleware::from_fn(isolate))
}

async fn isolate(req: Request, next: Next) -> Response {
    let csp = csp_header_for(req.uri().path());
    let mut response = next.run(req).await;
    for (name, value) in ISOLATION_HEADERS {
        response
            .headers_mut()
            .insert(*name, HeaderValue::from_static(value));
    }
    response.headers_mut().insert(CSP_HEADER, csp.clone());
    response
}

async fn play_root(RawQuery(query): RawQuery) -> Response {
    if !present() {
        return not_embedded();
    }
    let target = match query {
        Some(q) => format!("/play/?{q}"),
        None => "/play/".to_string(),
    };
    Redirect::temporary(&target).into_response()
}

async fn index(headers: HeaderMap) -> Response {
    serve_async("index.html".to_string(), headers).await
}

async fn file(AxPath(path): AxPath<String>, headers: HeaderMap) -> Response {
    serve_async(path, headers).await
}

/// [`serve`], off the runtime's threads when it is about to inflate a
/// brotli file for a client without br (the wasm inflates to over 100 MB,
/// once per blob): that decode would otherwise stall a worker the
/// data-layer socket and the hot-reload share.
async fn serve_async(path: String, headers: HeaderMap) -> Response {
    let accept = headers
        .get(header::ACCEPT_ENCODING)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    let heavy = find(&path)
        .is_some_and(|f| f.encoding == Encoding::Br && !accepts(accept, "br") && !inflated(f));
    if !heavy {
        return serve(&path, &headers);
    }
    match tokio::task::spawn_blocking(move || serve(&path, &headers)).await {
        Ok(response) => response,
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("inflating failed: {e}\n"),
        )
            .into_response(),
    }
}

fn not_embedded() -> Response {
    (StatusCode::NOT_FOUND, NOT_EMBEDDED).into_response()
}

fn serve(path: &str, headers: &HeaderMap) -> Response {
    if !present() {
        return not_embedded();
    }
    let accept = headers
        .get(header::ACCEPT_ENCODING)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    if path == "index.html" {
        return serve_index(headers, accept);
    }
    let Some(file) = find(path) else {
        return (
            StatusCode::NOT_FOUND,
            format!("the embedded web explorer has no {path}\n"),
        )
            .into_response();
    };
    let coding = coding_for(file, accept);
    let etag = etag_for(file, coding);
    if matches_etag(headers, &etag) {
        return not_modified(&etag, cache_control(file.path));
    }
    respond(file, accept)
}

fn matches_etag(headers: &HeaderMap, etag: &str) -> bool {
    headers
        .get(header::IF_NONE_MATCH)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|given| given.split(',').any(|tag| tag.trim() == etag))
}

fn not_modified(etag: &str, cache: &'static str) -> Response {
    Response::builder()
        .status(StatusCode::NOT_MODIFIED)
        .header(header::ETAG, etag)
        .header(header::CACHE_CONTROL, cache)
        .header(header::VARY, "Accept-Encoding")
        .body(Body::empty())
        .expect("static headers")
}

/// The page ([`Prepared`]): gzip to a client that takes it, identity
/// otherwise, one ETag per representation, always revalidated.
fn serve_index(headers: &HeaderMap, accept: &str) -> Response {
    let p = prepared();
    let gzip = accepts(accept, "gzip");
    let etag = if gzip {
        format!("\"{}-gz\"", p.etag)
    } else {
        format!("\"{}\"", p.etag)
    };
    if matches_etag(headers, &etag) {
        return not_modified(&etag, cache_control("index.html"));
    }
    let builder = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, mime_for("index.html"))
        .header(header::CACHE_CONTROL, cache_control("index.html"))
        .header(header::ETAG, etag)
        .header(header::VARY, "Accept-Encoding")
        .header(header::X_CONTENT_TYPE_OPTIONS, "nosniff");
    let (builder, body) = if gzip {
        (
            builder.header(header::CONTENT_ENCODING, "gzip"),
            Body::from(p.gz.clone()),
        )
    } else {
        (builder, Body::from(p.html.clone()))
    };
    builder.body(body).expect("static headers")
}

/// The coding a file goes out in for this `Accept-Encoding`: as stored when
/// the client takes it, identity otherwise (a gzip file inflated on the fly,
/// a brotli one inflated once and kept, see [`unpacked`]).
pub fn coding_for(file: &File, accept_encoding: &str) -> Encoding {
    match file.encoding {
        Encoding::Identity => Encoding::Identity,
        Encoding::Br if accepts(accept_encoding, "br") => Encoding::Br,
        Encoding::Gzip if accepts(accept_encoding, "gzip") => Encoding::Gzip,
        Encoding::Br | Encoding::Gzip => Encoding::Identity,
    }
}

/// The strong ETag of one representation: RFC 9110 §8.8.3 wants the gzip
/// and identity bodies of a file to validate apart, so a cache holding the
/// identity body is never told the gzip one is what it has.
pub fn etag_for(file: &File, coding: Encoding) -> String {
    match coding {
        Encoding::Identity => format!("\"{}\"", file.etag),
        Encoding::Gzip => format!("\"{}-gz\"", file.etag),
        Encoding::Br => format!("\"{}-br\"", file.etag),
    }
}

/// The response for one embedded file given the client's `Accept-Encoding`.
pub fn respond(file: &File, accept_encoding: &str) -> Response {
    let coding = coding_for(file, accept_encoding);
    let builder = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, mime_for(file.path))
        .header(header::CACHE_CONTROL, cache_control(file.path))
        .header(header::ETAG, etag_for(file, coding))
        .header(header::VARY, "Accept-Encoding")
        .header(header::X_CONTENT_TYPE_OPTIONS, "nosniff");
    let (builder, body) = match coding {
        Encoding::Identity if file.encoding != Encoding::Identity => match unpacked(file) {
            Ok(raw) => (builder, Body::from(raw)),
            Err(why) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("{}: {why}\n", file.path),
                )
                    .into_response();
            }
        },
        Encoding::Identity => (builder, Body::from(file.bytes)),
        Encoding::Br => (
            builder.header(header::CONTENT_ENCODING, "br"),
            Body::from(file.bytes),
        ),
        Encoding::Gzip => (
            builder.header(header::CONTENT_ENCODING, "gzip"),
            Body::from(file.bytes),
        ),
    };
    builder.body(body).expect("static headers")
}

/// The inflated brotli files, one slot per stored blob (the wasm is one
/// blob under two served paths), filled once: a second request for a blob
/// being inflated waits on the same slot instead of decoding again.
type Inflated = Arc<OnceLock<Result<Bytes, String>>>;
static INFLATED: OnceLock<Mutex<HashMap<(usize, usize), Inflated>>> = OnceLock::new();

fn inflated_slot(file: &File) -> Inflated {
    INFLATED
        .get_or_init(Default::default)
        .lock()
        .expect("no poison")
        .entry((file.bytes.as_ptr() as usize, file.bytes.len()))
        .or_default()
        .clone()
}

/// Whether a brotli file's inflated bytes are already kept.
fn inflated(file: &File) -> bool {
    inflated_slot(file).get().is_some()
}

/// The uncompressed bytes of a stored file. A gzip file inflates on each
/// call (small: the text-like files); a brotli file inflates once per blob
/// and is kept ([`INFLATED`]) — the wasm, over 100 MB inflated, and the
/// asset bundles, for the client without br. A stored file that is not what
/// its coding says is an error, not a body.
fn unpacked(file: &File) -> Result<Bytes, String> {
    use std::io::Read;
    match file.encoding {
        Encoding::Identity => Ok(Bytes::from_static(file.bytes)),
        Encoding::Gzip => {
            let mut out = Vec::with_capacity(file.raw_len);
            flate2::read::GzDecoder::new(file.bytes)
                .read_to_end(&mut out)
                .map_err(|e| format!("did not inflate: {e}"))?;
            Ok(Bytes::from(out))
        }
        Encoding::Br => inflated_slot(file)
            .get_or_init(|| {
                // raw_len is the stored size for a .br file; brotli runs
                // near 5:1 on the wasm, so this is about the inflated size
                // and the buffer is trimmed to what came out.
                let mut out = Vec::with_capacity(file.raw_len.saturating_mul(5));
                brotli_decompressor::Decompressor::new(file.bytes, 1 << 16)
                    .read_to_end(&mut out)
                    .map_err(|e| format!("did not inflate (brotli): {e}"))?;
                out.shrink_to_fit();
                Ok(Bytes::from(out))
            })
            .clone(),
    }
}

/// Whether an `Accept-Encoding` value takes `coding`: named with a non-zero
/// q, or covered by a `*` that is.
pub fn accepts(accept_encoding: &str, coding: &str) -> bool {
    let mut wildcard = None;
    for item in accept_encoding.split(',') {
        let mut parts = item.split(';');
        let name = parts.next().unwrap_or_default().trim().to_ascii_lowercase();
        let q = parts
            .find_map(|p| p.trim().strip_prefix("q="))
            .and_then(|q| q.trim().parse::<f32>().ok())
            .unwrap_or(1.0);
        if name == coding {
            return q > 0.0;
        }
        if name == "*" {
            wildcard = Some(q > 0.0);
        }
    }
    wildcard.unwrap_or(false)
}

/// `index.html` is what a new build changes, so it is always revalidated;
/// hashed chunks never change under their name; the rest revalidates on the
/// ETag, a 304 for the 13 MB wasm.
pub fn cache_control(path: &str) -> &'static str {
    if path != "index.html" && immutable(path) {
        "public, max-age=31536000, immutable"
    } else {
        "no-cache"
    }
}

/// A content-hashed name: a boot payload under `v/<hash>/`, the ui3
/// overlay's chunks and assets, or a file whose stem ends in a build hash
/// (`-5ncZRl8O`, `-53217a45365bb5fa`).
fn immutable(path: &str) -> bool {
    if path.starts_with("ui3-overlay/chunks/") || path.starts_with("ui3-overlay/assets/") {
        return true;
    }
    if let Some(rest) = path.strip_prefix("v/")
        && let Some((hash, _)) = rest.split_once('/')
        && !hash.is_empty()
        && hash.bytes().all(|b| b.is_ascii_hexdigit())
    {
        return true;
    }
    let name = path.rsplit('/').next().unwrap_or(path);
    let stem = name.split_once('.').map(|(s, _)| s).unwrap_or(name);
    let Some((_, tag)) = stem.rsplit_once('-') else {
        return false;
    };
    let hex16 = tag.len() >= 16 && tag.bytes().all(|b| b.is_ascii_hexdigit());
    let vite8 = tag.len() == 8
        && tag
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
        && tag.bytes().any(|b| b.is_ascii_uppercase())
        && tag.bytes().any(|b| b.is_ascii_digit());
    hex16 || vite8
}

fn mime_for(path: &str) -> &'static str {
    let ext = path
        .rsplit('/')
        .next()
        .and_then(|name| name.rsplit_once('.'))
        .map(|(_, ext)| ext.to_ascii_lowercase())
        .unwrap_or_default();
    match ext.as_str() {
        "html" => "text/html; charset=utf-8",
        "js" | "mjs" => "application/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" | "map" => "application/json; charset=utf-8",
        "webmanifest" => "application/manifest+json",
        "wasm" => "application/wasm",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "gif" => "image/gif",
        "avif" => "image/avif",
        "ico" => "image/x-icon",
        "ktx2" => "image/ktx2",
        "ttf" => "font/ttf",
        "otf" => "font/otf",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "glb" => "model/gltf-binary",
        "gltf" => "model/gltf+json",
        "wav" => "audio/wav",
        "mp3" => "audio/mpeg",
        "ogg" => "audio/ogg",
        "mp4" => "video/mp4",
        "webm" => "video/webm",
        "txt" | "md" | "wgsl" | "vert" | "frag" | "glsl" => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    /// The tree as deploy/web lies on disk: previous builds beside the
    /// current one, `.d.ts` next to the wasm, twins of every `.br`, and the
    /// content-addressed `v/<hash>/` payloads of this build next to a
    /// stale one.
    #[test]
    fn selection_drops_previous_builds_typings_twins_and_strays() {
        let boot = select::boot_prefixes(
            r#"<script>window.__DCL_BOOT__ = {"wasm":"v/2b6436930d8c33b3/pkg/webgpu_build_bg.wasm","bundle":"v/72d02f6d5f0a705f/assets_bundle.bin"};</script>"#,
        );
        assert_eq!(boot, ["v/2b6436930d8c33b3/", "v/72d02f6d5f0a705f/"]);
        assert!(select::boot_prefixes("<script>var v = \"v/\";</script>").is_empty());
        let picked = select::select(
            &strings(&[
                "index.html",
                "main.js",
                "assets_bundle.bin",
                "assets_bundle.bin.br",
                "assets_bundle.bin.br.prev-20260817",
                "assets_bundle.bin.prev-20260731",
                "provenance.json",
                "provenance.prev-20260731.json",
                "shader_compiling_transparent2.png",
                "logo.svg",
                "serve.json",
                "pkg/webgpu_build_bg.wasm",
                "pkg/webgpu_build_bg.wasm.br",
                "pkg/webgpu_build_bg.wasm.d.ts",
                "pkg/webgpu_build.d.ts",
                "pkg/webgpu_build.js",
                "pkg.prev-20260830/webgpu_build.js",
                "vendor/hls-1.6.16.min.js",
                "vendor/hls-1.6.16.min.js.br",
                "vendor/README.md",
                "fonts/NotoColorEmoji.subset.ttf",
                "fonts/NotoColorEmoji.subset.ttf.br",
                "ui3-overlay/chunks/bevy-CNJbS4Ep.js",
                "ui3-overlay/.DS_Store",
                "v/72d02f6d5f0a705f/assets_bundle.bin",
                "v/72d02f6d5f0a705f/assets_bundle.bin.br",
                "v/2b6436930d8c33b3/pkg/webgpu_build_bg.wasm",
                "v/2b6436930d8c33b3/pkg/webgpu_build_bg.wasm.br",
                "v/0000000000000000/assets_bundle.bin.br",
                "draco/draco_decoder.wasm",
                "favicon/favicon.svg",
                "modules/Runtime.js",
                "assets/textures/x.png",
            ]),
            &boot,
        );
        let served: Vec<&str> = picked.iter().map(|s| s.served.as_str()).collect();
        assert_eq!(
            served,
            [
                "assets/textures/x.png",
                "assets_bundle.bin",
                "draco/draco_decoder.wasm",
                "favicon/favicon.svg",
                "fonts/NotoColorEmoji.subset.ttf",
                "index.html",
                "main.js",
                "modules/Runtime.js",
                "pkg/webgpu_build.js",
                "pkg/webgpu_build_bg.wasm",
                "provenance.json",
                "shader_compiling_transparent2.png",
                "ui3-overlay/chunks/bevy-CNJbS4Ep.js",
                "v/2b6436930d8c33b3/pkg/webgpu_build_bg.wasm",
                "v/72d02f6d5f0a705f/assets_bundle.bin",
                "vendor/README.md",
                "vendor/hls-1.6.16.min.js",
            ]
        );
        let by_served = |p: &str| picked.iter().find(|s| s.served == p).unwrap().clone();
        assert_eq!(
            by_served("pkg/webgpu_build_bg.wasm"),
            select::Selected {
                served: "pkg/webgpu_build_bg.wasm".into(),
                source: "pkg/webgpu_build_bg.wasm.br".into(),
                br: true
            },
            "the .br twin stands in for the wasm"
        );
        assert!(by_served("assets_bundle.bin").br);
        assert!(by_served("vendor/hls-1.6.16.min.js").br);
        assert!(!by_served("main.js").br);
        assert!(select::compressible("main.js"));
        assert!(select::compressible("pkg/webgpu_build.js"));
        assert!(!select::compressible("favicon/favicon.png"));
        assert!(!select::compressible("ui3-overlay/fonts/Inter-Bold.woff2"));
    }

    /// The page text of an embedded file, whatever coding it is stored in.
    fn text(file: &File) -> String {
        String::from_utf8(unpacked(file).expect("stored as its coding says").to_vec())
            .expect("utf-8")
    }

    /// Whatever PLAY_EMBED_DIR held at build time, the table obeys the
    /// rules, the boot payloads the page names are there, and a payload
    /// that is a symlink to its root twin shares the twin's bytes.
    #[test]
    fn the_embedded_table_obeys_the_selection_rules() {
        let boot = find("index.html")
            .map(|index| select::boot_prefixes(&text(index)))
            .unwrap_or_default();
        let mut seen = std::collections::HashSet::new();
        for file in FILES {
            assert!(seen.insert(file.path), "{} twice", file.path);
            assert!(!file.path.contains(".prev-"), "{}", file.path);
            assert!(!file.path.ends_with(".d.ts"), "{}", file.path);
            assert!(
                !file.path.ends_with(".br"),
                "{} is served under its own name",
                file.path
            );
            assert!(
                select::wanted(file.path, &boot)
                    || select::wanted(&format!("{}.br", file.path), &boot),
                "{}",
                file.path
            );
            if let Some(rel) = file.path.strip_prefix("v/") {
                let twin = rel.split_once('/').map(|(_, twin)| twin).unwrap_or(rel);
                if let Some(root) = find(twin).filter(|root| root.bytes == file.bytes) {
                    assert!(
                        std::ptr::eq(root.bytes, file.bytes),
                        "{} carries a second copy of {twin}",
                        file.path
                    );
                }
            }
            assert_eq!(file.etag.len(), 16, "{}", file.path);
            if file.encoding == Encoding::Identity {
                assert_eq!(file.bytes.len(), file.raw_len, "{}", file.path);
            }
        }
        for prefix in &boot {
            assert!(
                FILES.iter().any(|f| f.path.starts_with(prefix.as_str())),
                "the page boots from {prefix} and nothing is embedded under it"
            );
        }
        if present() {
            assert!(find("index.html").is_some());
            assert!(
                engine_rev().is_some(),
                "provenance.json names the engine rev"
            );
            assert!(
                find("pkg/webgpu_build_bg.wasm").is_some_and(|f| f.encoding == Encoding::Br),
                "the wasm rides as brotli"
            );
            assert_eq!(
                local_player(8000).as_deref(),
                Some("http://127.0.0.1:8000/play/")
            );
        } else {
            assert_eq!(local_player(8000), None);
            assert_eq!(engine_rev(), None);
        }
    }

    /// A relative PLAY_EMBED_DIR comes back absolute, since the generated
    /// include_bytes! paths resolve against OUT_DIR; a missing dir or one
    /// without index.html is one message naming the variable.
    #[test]
    fn the_embed_root_is_canonical_and_holds_an_index() {
        let dir = std::env::temp_dir().join(format!("play-embed-root-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let missing = select::embed_root(dir.to_str().unwrap()).unwrap_err();
        assert!(missing.contains("PLAY_EMBED_DIR=") && missing.contains("no index.html"));
        std::fs::write(dir.join("index.html"), "<!doctype html>").unwrap();
        let cwd = std::env::current_dir().unwrap();
        let ups = "../".repeat(cwd.components().count() - 1);
        let relative = format!("{ups}{}", dir.strip_prefix("/").unwrap().display());
        let root = select::embed_root(&relative).unwrap();
        assert!(root.is_absolute(), "{}", root.display());
        assert_eq!(root, std::fs::canonicalize(&dir).unwrap());
        assert_eq!(
            select::embed_root(dir.to_str().unwrap()).unwrap(),
            std::fs::canonicalize(&dir).unwrap()
        );
        std::fs::remove_dir_all(&dir).unwrap();
        let gone = select::embed_root(dir.to_str().unwrap()).unwrap_err();
        assert!(gone.contains("not a readable directory"), "{gone}");
    }

    #[test]
    fn accept_encoding_is_read_with_its_q_values() {
        assert!(accepts("gzip, deflate, br, zstd", "br"));
        assert!(accepts("br;q=0.8, gzip", "br"));
        assert!(!accepts("gzip, br;q=0", "br"));
        assert!(!accepts("gzip", "br"));
        assert!(accepts("*", "br"));
        assert!(!accepts("*;q=0", "br"));
        assert!(!accepts("", "gzip"));
        assert!(accepts("GZIP", "gzip"));
    }

    fn body_and_headers(resp: Response) -> (StatusCode, HeaderMap, Vec<u8>) {
        let (parts, body) = resp.into_parts();
        let bytes = futures::executor::block_on(axum::body::to_bytes(body, usize::MAX)).unwrap();
        (parts.status, parts.headers, bytes.to_vec())
    }

    /// `node -e 'zlib.brotliCompressSync("not really brotli")'`.
    const BROTLI_SAMPLE: &[u8] = b"\x0b\x08\x80not really brotli\x03";

    #[test]
    fn brotli_files_go_as_is_or_inflated_for_a_client_without_br() {
        let file = File {
            path: "pkg/webgpu_build_bg.wasm",
            bytes: BROTLI_SAMPLE,
            encoding: Encoding::Br,
            raw_len: 17,
            etag: "0123456789abcdef",
        };
        let (status, headers, body) = body_and_headers(respond(&file, "gzip, deflate, br"));
        assert_eq!(status, StatusCode::OK);
        assert_eq!(headers["content-encoding"], "br");
        assert_eq!(headers["content-type"], "application/wasm");
        assert_eq!(headers["etag"], "\"0123456789abcdef-br\"");
        assert_eq!(headers["cache-control"], "no-cache");
        assert_eq!(body, BROTLI_SAMPLE);
        for accept in ["gzip", ""] {
            let (status, headers, body) = body_and_headers(respond(&file, accept));
            assert_eq!(status, StatusCode::OK, "{accept:?}");
            assert!(!headers.contains_key("content-encoding"));
            assert_eq!(headers["etag"], "\"0123456789abcdef\"");
            assert_eq!(headers["vary"], "Accept-Encoding");
            assert_eq!(body, b"not really brotli");
        }
        let corrupt = File {
            path: "pkg/x_bg.wasm",
            bytes: b"not really brotli",
            encoding: Encoding::Br,
            raw_len: 17,
            etag: "fedcba9876543202",
        };
        let (status, _, body) = body_and_headers(respond(&corrupt, "gzip"));
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert!(String::from_utf8(body).unwrap().contains("brotli"));
    }

    #[test]
    fn inline_scripts_are_hashed_the_way_csp_wants() {
        let html = "<head><script>a()</script>\n<script src=\"x.js\"></script>\n\
            <script type=\"module\">\nb()\n</script><scripts></scripts><script>c</script>";
        assert_eq!(select::inline_script_bodies(html), ["a()", "\nb()\n", "c"]);
        assert!(select::inline_script_bodies("<script src=a.js></script>").is_empty());
        assert!(select::inline_script_bodies("<script>never closed").is_empty());
        assert_eq!(
            select::csp_sha256(""),
            "sha256-47DEQpj8HBSa+/TImW+5JCeuQeRkm5NMpJWZG3hSuFU="
        );
        assert_eq!(
            select::csp_sha256("alert(1)"),
            "sha256-bhHHL3z2vDgxUt0W3dWQOrprscmda2Y5pLsLg4GF+pI="
        );
    }

    /// The head script names the embedded copy and the site's services,
    /// with the realm left to the page's own origin; the site is quoted, so
    /// a hostile value cannot leave the string.
    #[test]
    fn config_script_names_the_site_and_its_services() {
        let script = config_script(&PlayConfig::default());
        assert!(
            script.starts_with("window.__DCL_PLAY_EMBEDDED__=true;"),
            "{script}"
        );
        let default = format!("window.__SITE_BASE__={};", js_string(&site_origin("")));
        assert!(script.contains(&default), "{script}");
        assert!(
            script.ends_with(
                "globalThis.__zod_globalConfig={...globalThis.__zod_globalConfig,jitless:true};"
            ),
            "{script}"
        );
        assert!(
            !script.contains("__SERVICE_BASES__"),
            "ui3 derives the services from the site"
        );
        assert!(!script.contains("__CATALYST_BASE__"));
        let own = config_script(&PlayConfig {
            site: "https://example.org:8443/play/".to_string(),
        });
        assert!(
            own.contains("__SITE_BASE__=\"https://example.org:8443\";"),
            "an origin, no path: {own}"
        );
        // Not an origin: the default, not the string. A quote or a tag can
        // never reach the script; even if one did, `<` is written \u003c.
        for bad in [
            "https://x\";alert(1);",
            "</script><script>alert(1)",
            "ftp://x",
            "",
        ] {
            let out = config_script(&PlayConfig {
                site: bad.to_string(),
            });
            assert!(out.contains(&default), "{bad:?} -> {out}");
        }
        assert_eq!(js_string("</script>"), "\"\\u003c/script>\"");
        assert_eq!(
            site_origin("http://localhost:8000/x?y"),
            "http://localhost:8000"
        );
        assert_eq!(
            splice_head("<!doctype html><html><head><meta x></head>", "s()"),
            "<!doctype html><html><head><script>s()</script><meta x></head>"
        );
        assert_eq!(
            splice_head(
                "<!DOCTYPE html><HTML><HEAD lang=\"en\"\n data-x=\">\"><meta></HEAD>",
                "s()"
            ),
            "<!DOCTYPE html><HTML><HEAD lang=\"en\"\n data-x=\"><script>s()</script>\"><meta></HEAD>",
            "an attributed head still takes the script first (a `>` inside an attribute is the template's problem)"
        );
        assert_eq!(
            splice_head(
                "<!doctype html><html lang=en><body><header></header></body>",
                "s()"
            ),
            "<!doctype html><html lang=en><script>s()</script><body><header></header></body>",
            "<header> is not <head>; no head means first in <html>, never ahead of the doctype"
        );
        assert_eq!(splice_head("<p>", "s()"), "<script>s()</script><p>");
    }

    /// The page's policy names every inline script by hash and no
    /// `'unsafe-eval'`; the scene worker's has it; nothing has
    /// `'unsafe-inline'` for scripts.
    #[test]
    fn csp_is_enforced_with_hashes_and_the_scene_worker_may_eval() {
        assert_eq!(CSP_HEADER, "content-security-policy");
        let page = csp_for("index.html");
        assert_eq!(page, csp_for("main.js"));
        assert_eq!(page, csp_for("ui3-overlay/chunks/x.js"));
        assert_eq!(page, csp_for("sandbox_worker.js.map"));
        assert_eq!(page, csp_for("/play/not_sandbox_worker.js"));
        let worker = csp_for("sandbox_worker.js");
        assert_eq!(
            worker,
            csp_for("/play/sandbox_worker.js"),
            "the middleware passes the request path"
        );
        assert_eq!(worker, csp_for("v/0123abcd/sandbox_worker.js"));
        assert_ne!(page, worker);
        assert_eq!(
            csp_header_for("/play/sandbox_worker.js").to_str().unwrap(),
            worker
        );
        assert_eq!(csp_header_for("/play/main.js").to_str().unwrap(), page);
        assert!(!page.contains(" 'unsafe-eval'"), "{page}");
        assert!(
            worker.contains("script-src 'self' 'wasm-unsafe-eval' blob: 'unsafe-eval';"),
            "{worker}"
        );
        assert!(
            !page.contains("'unsafe-inline'; script")
                && !page.contains("script-src 'self' 'unsafe-inline'"),
            "{page}"
        );
        // The list is engine/web/csp-policy.mts (by way of the docroot's
        // csp.json, or build.rs's copy of it for a build without one); the
        // same literal is pinned in web/scripts/test-csp-policy.mts, so an
        // edit to one side fails a test on the other.
        assert_eq!(
            policy("https://example.org", "", &[]),
            "default-src 'self'; script-src 'self' 'wasm-unsafe-eval' blob:; worker-src 'self' blob:; \
             frame-src 'self' https://example.org; frame-ancestors 'self'; base-uri 'none'; object-src 'none'; \
             img-src * data: blob:; media-src * data: blob:; font-src 'self' data:; \
             style-src 'self' 'unsafe-inline'; connect-src * https: wss: data: blob:"
        );
        assert!(policy("", "", &[]).contains("; frame-src 'self'; frame-ancestors"));
        assert_eq!(SCENE_WORKER, "sandbox_worker.js");
        let frames = format!(
            "; worker-src 'self' blob:; frame-src {}; frame-ancestors 'self'; base-uri 'none'; object-src 'none';",
            format!(
                "'self' {}",
                site_origin(&CONFIG.get_or_init(PlayConfig::default).site)
            )
            .trim()
        );
        for csp in [page, worker] {
            assert!(
                csp.starts_with("default-src 'self'; script-src 'self' 'wasm-unsafe-eval' blob:"),
                "{csp}"
            );
            assert!(csp.contains(&frames), "{csp}");
            assert!(
                !csp.contains(" https:;"),
                "no frame from any https origin: {csp}"
            );
            assert!(
                csp.contains(
                    "style-src 'self' 'unsafe-inline'; connect-src * https: wss: data: blob:"
                ),
                "{csp}"
            );
        }
        let own = select::csp_sha256(&config_script(CONFIG.get_or_init(PlayConfig::default)));
        assert!(page.contains(&format!(" '{own}'")), "{page}");
        for hash in INLINE_SCRIPT_HASHES {
            assert!(page.contains(&format!(" '{hash}'")), "{hash} in {page}");
        }
        if present() {
            assert!(
                !INLINE_SCRIPT_HASHES.is_empty(),
                "the page has inline scripts"
            );
            let raw = unpacked(find("index.html").unwrap()).unwrap();
            let raw = String::from_utf8_lossy(&raw);
            let bodies = select::inline_script_bodies(&raw);
            assert_eq!(bodies.len(), INLINE_SCRIPT_HASHES.len());
            let served = String::from_utf8_lossy(&prepared().html).into_owned();
            assert!(
                served.contains("<head><script>window.__DCL_PLAY_EMBEDDED__=true;"),
                "the config script leads the head"
            );
            assert_eq!(
                select::inline_script_bodies(&served).len(),
                bodies.len() + 1
            );
            assert!(
                !has_inline_handler(&served),
                "no inline handlers: {}",
                served.len()
            );
        }
    }

    /// An `onclick="…"` or `onload='…'` attribute anywhere in the markup —
    /// what an enforced CSP without 'unsafe-hashes' refuses.
    fn has_inline_handler(html: &str) -> bool {
        let bytes = html.as_bytes();
        let mut i = 0;
        while let Some(rel) = html[i..].find(" on") {
            let start = i + rel + 3;
            let name = bytes[start..]
                .iter()
                .take_while(|b| b.is_ascii_alphabetic())
                .count();
            if name > 0 && bytes.get(start + name) == Some(&b'=') {
                return true;
            }
            i = start;
        }
        false
    }

    #[test]
    fn inline_handler_scan_reads_attributes_not_words() {
        assert!(has_inline_handler("<a href=# onclick=\"x()\">"));
        assert!(has_inline_handler("<body onload='go()'>"));
        assert!(!has_inline_handler(
            "<p>click on the button</p><a data-on=\"x\">"
        ));
        assert!(!has_inline_handler(
            "<script>var onx = 1; only one = 2</script>"
        ));
    }

    /// One ETag per representation (RFC 9110 §8.8.3): the gzip body and
    /// the inflated one validate apart, a 304 says which coding it stands
    /// for, and a brotli file goes out inflated under its identity tag to a
    /// client that names the br tag but cannot take br.
    #[tokio::test]
    async fn etags_name_the_coding_and_conditional_answers_vary() {
        let raw = b"export const x = 1;\n".repeat(50);
        let packed = {
            use std::io::Write;
            let mut e = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::best());
            e.write_all(&raw).unwrap();
            e.finish().unwrap()
        };
        let packed: &'static [u8] = Box::leak(packed.into_boxed_slice());
        let gz = File {
            path: "ui3-overlay/chunks/x-CNJbS4Ep.js",
            bytes: packed,
            encoding: Encoding::Gzip,
            raw_len: raw.len(),
            etag: "00000000deadbeef",
        };
        let (_, gzipped, _) = body_and_headers(respond(&gz, "gzip"));
        let (_, plain, _) = body_and_headers(respond(&gz, ""));
        assert_eq!(gzipped["etag"], "\"00000000deadbeef-gz\"");
        assert_eq!(plain["etag"], "\"00000000deadbeef\"");
        assert_eq!(etag_for(&gz, Encoding::Br), "\"00000000deadbeef-br\"");
        assert_eq!(coding_for(&gz, "br"), Encoding::Identity);
        let br = File {
            path: "pkg/x_bg.wasm",
            bytes: b"br",
            encoding: Encoding::Br,
            raw_len: 2,
            etag: "0123456789abcd01",
        };
        assert_eq!(coding_for(&br, "gzip"), Encoding::Identity);
        assert_eq!(coding_for(&br, "br"), Encoding::Br);
        let raw_file = File {
            path: "favicon/f.png",
            bytes: b"png",
            encoding: Encoding::Identity,
            raw_len: 3,
            etag: "fedcba9876543201",
        };
        assert_eq!(coding_for(&raw_file, ""), Encoding::Identity);
        assert_eq!(
            etag_for(&raw_file, Encoding::Identity),
            "\"fedcba9876543201\""
        );
        if !present() {
            return;
        }
        assert_eq!(
            find("index.html").unwrap().encoding,
            Encoding::Gzip,
            "index.html is gzipped"
        );
        let identity_tag = format!("\"{}\"", prepared().etag);
        let gzip_tag = format!("\"{}-gz\"", prepared().etag);
        let cached_identity = served(
            "/play/index.html",
            &[
                ("if-none-match", &identity_tag),
                ("accept-encoding", "gzip"),
            ],
        )
        .await;
        assert_eq!(
            cached_identity.status(),
            StatusCode::OK,
            "the identity tag does not validate the gzip representation"
        );
        assert_eq!(cached_identity.headers()["etag"], gzip_tag);
        let cached_gzip = served(
            "/play/index.html",
            &[("if-none-match", &gzip_tag), ("accept-encoding", "gzip")],
        )
        .await;
        assert_eq!(cached_gzip.status(), StatusCode::NOT_MODIFIED);
        assert_eq!(cached_gzip.headers()["etag"], gzip_tag);
        assert_eq!(cached_gzip.headers()["vary"], "Accept-Encoding");
        let cached_plain = served("/play/index.html", &[("if-none-match", &identity_tag)]).await;
        assert_eq!(cached_plain.status(), StatusCode::NOT_MODIFIED);
        assert_eq!(cached_plain.headers()["etag"], identity_tag);
        let wasm = find("pkg/webgpu_build_bg.wasm").unwrap();
        let inflated = served(
            "/play/pkg/webgpu_build_bg.wasm",
            &[
                ("if-none-match", &etag_for(wasm, Encoding::Br)),
                ("accept-encoding", "gzip"),
            ],
        )
        .await;
        assert_eq!(inflated.status(), StatusCode::OK);
        assert_eq!(inflated.headers()["vary"], "Accept-Encoding");
        assert_eq!(
            inflated.headers()["etag"],
            etag_for(wasm, Encoding::Identity)
        );
        assert!(!inflated.headers().contains_key("content-encoding"));
        let cached = served(
            "/play/pkg/webgpu_build_bg.wasm",
            &[
                ("if-none-match", &etag_for(wasm, Encoding::Identity)),
                ("accept-encoding", "gzip"),
            ],
        )
        .await;
        assert_eq!(cached.status(), StatusCode::NOT_MODIFIED);
    }

    #[test]
    fn gzipped_files_inflate_for_a_client_without_gzip() {
        let raw = b"console.log('hello from the embedded player')".repeat(20);
        let packed = {
            use std::io::Write;
            let mut e = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::best());
            e.write_all(&raw).unwrap();
            e.finish().unwrap()
        };
        let packed: &'static [u8] = Box::leak(packed.into_boxed_slice());
        let file = File {
            path: "ui3-overlay/chunks/bevy-CNJbS4Ep.js",
            bytes: packed,
            encoding: Encoding::Gzip,
            raw_len: raw.len(),
            etag: "fedcba9876543210",
        };
        let (status, headers, body) = body_and_headers(respond(&file, "gzip, br"));
        assert_eq!(status, StatusCode::OK);
        assert_eq!(headers["content-encoding"], "gzip");
        assert_eq!(
            headers["cache-control"],
            "public, max-age=31536000, immutable"
        );
        assert_eq!(
            headers["content-type"],
            "application/javascript; charset=utf-8"
        );
        assert_eq!(body, packed);
        let (status, headers, body) = body_and_headers(respond(&file, ""));
        assert_eq!(status, StatusCode::OK);
        assert!(!headers.contains_key("content-encoding"));
        assert_eq!(body, raw);
    }

    #[test]
    fn cache_control_pins_hashed_chunks_and_revalidates_the_rest() {
        assert_eq!(cache_control("index.html"), "no-cache");
        assert_eq!(cache_control("engine.js"), "no-cache");
        assert_eq!(cache_control("pkg/webgpu_build_bg.wasm"), "no-cache");
        assert_eq!(cache_control("engine-worker-shims.js"), "no-cache");
        assert_eq!(
            cache_control("vendor/livekit-client-2.20.0.umd.min.js"),
            "no-cache"
        );
        for hashed in [
            "ui3-overlay/chunks/AppShell-CmosbINx.js",
            "ui3-overlay/assets/AuthScreenImg-860-DcU4-nNW.webp",
            "ui3-overlay/template-bundles/comms-53217a45365bb5fa",
            "chunk-5ncZRl8O.js",
            "v/2b6436930d8c33b3/pkg/webgpu_build_bg.wasm",
            "v/72d02f6d5f0a705f/assets_bundle.bin",
        ] {
            assert_eq!(
                cache_control(hashed),
                "public, max-age=31536000, immutable",
                "{hashed}"
            );
        }
    }

    async fn served(path: &str, extra: &[(&str, &str)]) -> reqwest::Response {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, routes()).await.unwrap();
        });
        let mut req = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .unwrap()
            .get(format!("http://{addr}{path}"));
        for (k, v) in extra {
            req = req.header(*k, *v);
        }
        req.send().await.unwrap()
    }

    fn assert_isolated(headers: &reqwest::header::HeaderMap) {
        for (name, value) in ISOLATION_HEADERS {
            assert_eq!(headers[*name], *value, "{name}");
        }
        assert_eq!(headers[CSP_HEADER].to_str().unwrap(), csp_for("index.html"));
        assert!(!headers.contains_key("content-security-policy-report-only"));
    }

    /// The isolation headers ride every answer, the page and a chunk alike,
    /// and the 404 of a build without a player.
    #[tokio::test]
    async fn every_play_response_is_isolated() {
        let root = served("/play/", &[("accept-encoding", "gzip, br")]).await;
        assert_isolated(root.headers());
        if !present() {
            assert_eq!(root.status(), StatusCode::NOT_FOUND);
            assert_eq!(root.text().await.unwrap(), NOT_EMBEDDED);
            let redirect = served("/play", &[]).await;
            assert_eq!(redirect.status(), StatusCode::NOT_FOUND);
            assert_isolated(redirect.headers());
            return;
        }
        assert_eq!(root.status(), StatusCode::OK);
        assert_eq!(root.headers()["cache-control"], "no-cache");
        assert_eq!(root.headers()["content-encoding"], "gzip");
        let gzip_tag = format!("\"{}-gz\"", prepared().etag);
        assert_eq!(root.headers()["etag"], gzip_tag);
        let again = served(
            "/play/index.html",
            &[
                ("if-none-match", &gzip_tag),
                ("accept-encoding", "gzip, br"),
            ],
        )
        .await;
        assert_eq!(again.status(), StatusCode::NOT_MODIFIED);
        assert_isolated(again.headers());
        let plain = served("/play/", &[]).await;
        assert_eq!(plain.status(), StatusCode::OK);
        assert!(!plain.headers().contains_key("content-encoding"));
        let page = plain.text().await.unwrap();
        assert!(
            page.contains("<head><script>window.__DCL_PLAY_EMBEDDED__=true;"),
            "the config script leads the head"
        );
        assert!(
            page.contains(&format!(
                "window.__SITE_BASE__={};globalThis.__zod_globalConfig=",
                js_string(&site_origin(""))
            )),
            "{}",
            &page[..400]
        );
        let worker = served("/play/sandbox_worker.js", &[]).await;
        assert_eq!(worker.status(), StatusCode::OK);
        assert_eq!(
            worker.headers()[CSP_HEADER].to_str().unwrap(),
            csp_for("sandbox_worker.js")
        );
        assert!(
            worker.headers()[CSP_HEADER]
                .to_str()
                .unwrap()
                .contains("'unsafe-eval'")
        );

        let chunk = FILES
            .iter()
            .find(|f| f.path.starts_with("ui3-overlay/chunks/") && f.encoding == Encoding::Gzip)
            .expect("the overlay ships gzipped chunks");
        let resp = served(&format!("/play/{}", chunk.path), &[]).await;
        assert_eq!(resp.status(), StatusCode::OK);
        assert_isolated(resp.headers());
        assert_eq!(
            resp.headers()["cache-control"],
            "public, max-age=31536000, immutable"
        );
        assert!(
            !resp.headers().contains_key("content-encoding"),
            "inflated for a client without gzip"
        );
        assert_eq!(resp.bytes().await.unwrap().len(), chunk.raw_len);

        let wasm = served(
            "/play/pkg/webgpu_build_bg.wasm",
            &[("accept-encoding", "gzip")],
        )
        .await;
        assert_eq!(
            wasm.status(),
            StatusCode::OK,
            "inflated for a client without br"
        );
        assert_isolated(wasm.headers());
        assert!(!wasm.headers().contains_key("content-encoding"));
        let stored = find("pkg/webgpu_build_bg.wasm").unwrap().raw_len;
        let body = wasm.bytes().await.unwrap();
        assert!(
            body.len() > stored,
            "inflated past its stored {stored} bytes"
        );
        assert_eq!(&body[..4], b"\0asm");
        let wasm = served(
            "/play/pkg/webgpu_build_bg.wasm",
            &[("accept-encoding", "br")],
        )
        .await;
        assert_eq!(wasm.status(), StatusCode::OK);
        assert_eq!(wasm.headers()["content-encoding"], "br");

        let redirect = served("/play?preview=true&realm=x", &[]).await;
        assert_eq!(redirect.status(), StatusCode::TEMPORARY_REDIRECT);
        assert_eq!(
            redirect.headers()["location"],
            "/play/?preview=true&realm=x"
        );
        assert_isolated(redirect.headers());
        let missing = served("/play/nope.js", &[]).await;
        assert_eq!(missing.status(), StatusCode::NOT_FOUND);
        assert_isolated(missing.headers());
    }
}
