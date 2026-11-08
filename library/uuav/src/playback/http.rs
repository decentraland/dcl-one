//! http(s) input for the demuxer.
//!
//! FFmpeg is built without its https, tls and crypto protocols; its http
//! protocol stays linked as the rtsp demuxer's tunnelling transport and
//! serves no url of its own. The helper's own client (ureq over rustls,
//! roots from [`crate::trust`]) serves every
//! `http:` and `https:` url through custom-I/O contexts: the top-level
//! stream is opened by [`IoHooks::open_top_level`] before
//! `avformat_open_input`, which then treats the context as caller-owned;
//! what the HLS demuxer opens afterwards (playlists, keys, segments,
//! `crypto+` AES-128 segments) arrives through the `io_open`/`io_close2`
//! hooks the format context carries. Urls of any other scheme (`file:`,
//! `data:`) go to FFmpeg's own opener, under the protocol whitelist.
//!
//! Positions are resource offsets, as FFmpeg's http protocol reports them:
//! a byte-range segment starts at its `offset` and ends at `end_offset`.

use anyhow::{Context as _, Result, anyhow, bail, ensure};
use ffmpeg_sys_next as ff;
use parking_lot::Mutex;
use std::ffi::{CStr, CString};
use std::fmt;
use std::io::Read;
use std::os::raw::{c_char, c_int, c_void};
use std::ptr;
use std::sync::Arc;
use ureq::ResponseExt as _;

use super::util::ReadOnlyCancelToken;
use crate::ffutil::StreamingProtocol;
use crate::trust;

/// Transfer unit between FFmpeg's buffer and the response body.
const BUFFER_SIZE: c_int = 64 * 1024;
/// A forward seek up to this far reads and drops bytes on the open
/// connection instead of paying a request round trip.
const DISCARD_LIMIT: u64 = 1024 * 1024;

// avio.h, log.h and opt.h values; `constants_match_the_bindings` pins them
const AVSEEK_SIZE: c_int = 0x10000;
const AVSEEK_FORCE: c_int = 0x20000;
const AVIO_SEEKABLE_NORMAL: c_int = 1;
const AVIO_FLAG_WRITE: c_int = 2;
const LOG_ERROR: c_int = 16;
const LOG_WARNING: c_int = 24;
/// `AV_OPT_FLAG_EXPORT | AV_OPT_FLAG_READONLY`
const OPT_FLAG_EXPORT_READONLY: c_int = 192;

type OpenFn = unsafe extern "C" fn(
    *mut ff::AVFormatContext,
    *mut *mut ff::AVIOContext,
    *const c_char,
    c_int,
    *mut *mut ff::AVDictionary,
) -> c_int;
type CloseFn = unsafe extern "C" fn(*mut ff::AVFormatContext, *mut ff::AVIOContext) -> c_int;

/// Error carrying the code the FFmpeg callbacks return.
#[derive(Debug)]
struct Failure {
    code: c_int,
    message: String,
}

impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for Failure {}

fn fail(errno: c_int, message: impl Into<String>) -> anyhow::Error {
    Failure {
        code: ff::AVERROR(errno),
        message: message.into(),
    }
    .into()
}

/// Code for an error raised below; anything unlabelled is an I/O error.
fn averror(error: &anyhow::Error) -> c_int {
    error
        .downcast_ref::<Failure>()
        .map_or(ff::AVERROR(libc::EIO), |f| f.code)
}

/// Logs through FFmpeg, so the line reaches the host's log callback.
fn log(level: c_int, message: &str) {
    if let Ok(line) = CString::new(message) {
        unsafe {
            ff::av_log(
                ptr::null_mut(),
                level,
                c"uuav http: %s\n".as_ptr(),
                line.as_ptr(),
            );
        }
    }
}

/// Returns the error's code, logging it unless it is a cancellation or a
/// routine refusal (an unknown length, an unsupported seek).
fn report(error: &anyhow::Error) -> c_int {
    let code = averror(error);
    if code != ff::AVERROR_EXIT && code != ff::AVERROR(libc::ENOSYS) {
        log(LOG_ERROR, &format!("{error:#}"));
    }
    code
}

/// An url this module serves, as the demuxer spelled it.
struct HttpTarget<'a> {
    url: &'a str,
    /// Lower-case scheme: the whitelist entry it needs.
    scheme: String,
    /// `crypto+<url>`: an AES-128-CBC segment, key and iv among the options.
    aes: bool,
}

impl<'a> HttpTarget<'a> {
    fn parse(spelled: &'a str) -> Option<Self> {
        let (url, aes) = spelled
            .strip_prefix("crypto+")
            .map_or((spelled, false), |inner| (inner, true));
        let (scheme, rest) = url.split_once(':')?;
        if !rest.starts_with("//") {
            return None;
        }
        let scheme = scheme.to_ascii_lowercase();
        (scheme == "http" || scheme == "https").then_some(Self { url, scheme, aes })
    }
}

/// What FFmpeg calls for every url a demuxer opens or closes; lives in
/// `AVFormatContext.opaque` for as long as the context does.
pub(super) struct IoHooks {
    /// The cancel hook holds a weak reference: releasing the hooks drops
    /// the connection pool, cancelling shuts it down while they live.
    agent: Arc<ureq::Agent>,
    cancel: ReadOnlyCancelToken,
    whitelist: Vec<String>,
    default_open: OpenFn,
    default_close: CloseFn,
    /// Contexts this module created and still owns, by address.
    contexts: Mutex<Vec<usize>>,
}

impl IoHooks {
    /// Installs the hooks on a freshly allocated context, keeping FFmpeg's
    /// own opener for the schemes this module does not serve. Cancelling
    /// shuts the client down, which interrupts a blocked request. The
    /// returned pointer is also `fmt.opaque`; [`IoHooks::release`] it once
    /// the context is closed.
    pub(super) unsafe fn install(
        fmt: *mut ff::AVFormatContext,
        cancel: ReadOnlyCancelToken,
        whitelist: &StreamingProtocol,
    ) -> Result<*mut Self> {
        let (Some(default_open), Some(default_close)) =
            (unsafe { (*fmt).io_open }, unsafe { (*fmt).io_close2 })
        else {
            bail!("format context has no default io_open/io_close2");
        };
        let agent = Arc::new(trust::agent());
        let doomed = Arc::downgrade(&agent);
        cancel.on_cancel(move || {
            if let Some(agent) = doomed.upgrade() {
                ureq::Agent::clone(&agent).shutdown();
            }
        });
        let hooks = Box::into_raw(Box::new(Self {
            agent,
            cancel,
            whitelist: whitelist.names().map(str::to_owned).collect(),
            default_open,
            default_close,
            contexts: Mutex::new(Vec::new()),
        }));
        unsafe {
            (*fmt).opaque = hooks.cast();
            (*fmt).io_open = Some(io_open);
            (*fmt).io_close2 = Some(io_close2);
        }
        Ok(hooks)
    }

    /// Frees the hooks; nothing may call into them afterwards.
    pub(super) unsafe fn release(hooks: *mut Self) {
        if !hooks.is_null() {
            drop(unsafe { Box::from_raw(hooks) });
        }
    }

    /// Opens the stream when `url` is http(s): the context to hand
    /// `avformat_open_input` as `pb`. `None` leaves the url to FFmpeg.
    pub(super) fn open_top_level(&self, url: &str) -> Result<Option<*mut ff::AVIOContext>> {
        let Some(target) = HttpTarget::parse(url) else {
            return Ok(None);
        };
        ensure!(!target.aes, "a crypto+ url is not a stream");
        self.open(&target, 0, None, None).map(Some)
    }

    /// Frees a context this module created; null or foreign ones are left
    /// alone.
    pub(super) unsafe fn close(&self, ctx: *mut ff::AVIOContext) {
        if !ctx.is_null() && self.disown(ctx) {
            unsafe { free_context(ctx) };
        }
    }

    fn open(
        &self,
        target: &HttpTarget,
        offset: u64,
        end: Option<u64>,
        aes: Option<([u8; 16], [u8; 16])>,
    ) -> Result<*mut ff::AVIOContext> {
        if !self.whitelist.contains(&target.scheme) {
            return Err(fail(
                libc::EINVAL,
                format!(
                    "protocol '{}' not on whitelist '{}'",
                    target.scheme,
                    self.whitelist.join(",")
                ),
            ));
        }
        let http = HttpSource::open(
            ureq::Agent::clone(&self.agent),
            self.cancel.clone(),
            target.url,
            offset,
            end,
        )?;
        let body = match aes {
            Some((key, iv)) => Body::Aes(Box::new(AesSource::new(http, key, iv))),
            None => Body::Plain(Box::new(http)),
        };
        let ctx = new_context(Source::new(body)?)?;
        self.contexts.lock().push(ctx.expose_provenance());
        Ok(ctx)
    }

    fn disown(&self, ctx: *mut ff::AVIOContext) -> bool {
        let mut contexts = self.contexts.lock();
        let Some(index) = contexts.iter().position(|&c| c == ctx.addr()) else {
            return false;
        };
        contexts.swap_remove(index);
        true
    }
}

impl Drop for IoHooks {
    /// Frees the contexts the demuxer left open; `release` runs after
    /// `avformat_close_input`, so nothing reads them any more.
    fn drop(&mut self) {
        let left = std::mem::take(self.contexts.get_mut());
        if !left.is_empty() {
            log(
                LOG_WARNING,
                &format!("{} context(s) left open by the demuxer", left.len()),
            );
        }
        for addr in left {
            unsafe { free_context(ptr::with_exposed_provenance_mut(addr)) };
        }
    }
}

unsafe extern "C" fn io_open(
    s: *mut ff::AVFormatContext,
    pb: *mut *mut ff::AVIOContext,
    url: *const c_char,
    flags: c_int,
    options: *mut *mut ff::AVDictionary,
) -> c_int {
    let Some(hooks) = (unsafe { (*s).opaque.cast::<IoHooks>().as_ref() }) else {
        return ff::AVERROR(libc::EINVAL);
    };
    let spelled = unsafe { CStr::from_ptr(url) }.to_string_lossy();
    let Some(target) = HttpTarget::parse(&spelled) else {
        return unsafe { (hooks.default_open)(s, pb, url, flags, options) };
    };
    match unsafe { open_requested(hooks, &target, flags, options) } {
        Ok(ctx) => {
            unsafe { *pb = ctx };
            0
        }
        Err(error) => report(&error),
    }
}

/// `offset`/`end_offset` bound a byte-range segment, `key`/`iv` (hex)
/// decrypt an AES-128 one; the HLS demuxer sets them.
unsafe fn open_requested(
    hooks: &IoHooks,
    target: &HttpTarget,
    flags: c_int,
    options: *mut *mut ff::AVDictionary,
) -> Result<*mut ff::AVIOContext> {
    if flags & AVIO_FLAG_WRITE != 0 {
        return Err(fail(
            libc::EINVAL,
            format!("{} opened for writing", target.url),
        ));
    }
    let option = |name: &CStr| unsafe { dict_value(options, name) };
    let offset = option(c"offset")
        .as_deref()
        .map(str::parse::<u64>)
        .transpose()
        .context("offset option")?
        .unwrap_or(0);
    let end = option(c"end_offset")
        .as_deref()
        .map(str::parse::<u64>)
        .transpose()
        .context("end_offset option")?
        .filter(|end| *end > offset);
    let aes = if target.aes {
        let key = option(c"key").ok_or_else(|| fail(libc::EINVAL, "crypto+ url without a key"))?;
        let iv = option(c"iv").ok_or_else(|| fail(libc::EINVAL, "crypto+ url without an iv"))?;
        Some((hex16(&key).context("key")?, hex16(&iv).context("iv")?))
    } else {
        None
    };
    hooks.open(target, offset, end, aes)
}

unsafe extern "C" fn io_close2(s: *mut ff::AVFormatContext, pb: *mut ff::AVIOContext) -> c_int {
    if pb.is_null() {
        return 0;
    }
    let Some(hooks) = (unsafe { (*s).opaque.cast::<IoHooks>().as_ref() }) else {
        return unsafe { ff::avio_close(pb) };
    };
    if hooks.disown(pb) {
        unsafe { free_context(pb) };
        0
    } else {
        unsafe { (hooks.default_close)(s, pb) }
    }
}

unsafe fn dict_value(options: *mut *mut ff::AVDictionary, name: &CStr) -> Option<String> {
    if options.is_null() || unsafe { *options }.is_null() {
        return None;
    }
    let entry = unsafe { ff::av_dict_get(*options, name.as_ptr(), ptr::null(), 0) };
    if entry.is_null() {
        return None;
    }
    Some(
        unsafe { CStr::from_ptr((*entry).value) }
            .to_string_lossy()
            .into_owned(),
    )
}

fn hex16(text: &str) -> Result<[u8; 16]> {
    let digits = text.as_bytes();
    ensure!(
        digits.len() == 32,
        "expected 32 hex digits, got {}",
        digits.len()
    );
    let mut out = [0u8; 16];
    for (byte, pair) in out.iter_mut().zip(digits.as_chunks::<2>().0) {
        *byte = u8::from_str_radix(std::str::from_utf8(pair)?, 16)?;
    }
    Ok(out)
}

/// The custom context's `opaque`. It starts with an `AVClass` pointer so
/// that `av_opt_get(ctx, "location", AV_OPT_SEARCH_CHILDREN)`, how the
/// HLS demuxer learns where a playlist was redirected to before resolving
/// the segment urls, finds [`SOURCE_OPTIONS`] here.
#[repr(C)]
struct Source {
    class: *const ff::AVClass,
    /// The heap text of `location_text`, which stays put when `Source` moves.
    location: *const c_char,
    location_text: CString,
    body: Body,
}

enum Body {
    Plain(Box<HttpSource>),
    Aes(Box<AesSource>),
}

impl Source {
    fn new(body: Body) -> Result<Self> {
        let served_from = match &body {
            Body::Plain(http) => &http.location,
            Body::Aes(aes) => &aes.inner.location,
        };
        let location_text = CString::new(served_from.as_str()).context("location")?;
        Ok(Self {
            class: &raw const SOURCE_CLASS.0,
            location: location_text.as_ptr(),
            location_text,
            body,
        })
    }

    fn read(&mut self, out: &mut [u8]) -> Result<usize> {
        match &mut self.body {
            Body::Plain(http) => http.read(out),
            Body::Aes(aes) => aes.read(out),
        }
    }

    fn seek(&mut self, offset: i64, whence: c_int) -> Result<u64> {
        match &mut self.body {
            Body::Plain(http) => http.seek(offset, whence),
            Body::Aes(aes) => aes.seek(offset, whence),
        }
    }

    fn seekable(&self) -> bool {
        match &self.body {
            Body::Plain(http) => http.seekable(),
            Body::Aes(_) => false,
        }
    }
}

struct SyncClass(ff::AVClass);
// SAFETY: immutable; FFmpeg only reads it
unsafe impl Sync for SyncClass {}

struct SyncOptions([ff::AVOption; 2]);
// SAFETY: immutable; FFmpeg only reads it
unsafe impl Sync for SyncOptions {}

const OPTION_TERMINATOR: ff::AVOption = ff::AVOption {
    name: ptr::null(),
    help: ptr::null(),
    offset: 0,
    type_: ff::AVOptionType::AV_OPT_TYPE_FLAGS,
    default_val: ff::AVOption__bindgen_ty_1 { i64_: 0 },
    min: 0.0,
    max: 0.0,
    flags: 0,
    unit: ptr::null(),
};

#[allow(clippy::cast_possible_wrap)] // a struct offset
static SOURCE_OPTIONS: SyncOptions = SyncOptions([
    ff::AVOption {
        name: c"location".as_ptr(),
        help: c"url the stream was served from".as_ptr(),
        offset: std::mem::offset_of!(Source, location) as c_int,
        type_: ff::AVOptionType::AV_OPT_TYPE_STRING,
        flags: OPT_FLAG_EXPORT_READONLY,
        ..OPTION_TERMINATOR
    },
    OPTION_TERMINATOR,
]);

type ChildNext = Option<unsafe extern "C" fn(*mut c_void, *mut c_void) -> *mut c_void>;

const fn class(
    name: &'static CStr,
    options: *const ff::AVOption,
    child_next: ChildNext,
) -> ff::AVClass {
    ff::AVClass {
        class_name: name.as_ptr(),
        item_name: Some(ff::av_default_item_name),
        option: options,
        version: LIBAVUTIL_VERSION_INT,
        log_level_offset_offset: 0,
        parent_log_context_offset: 0,
        category: ff::AVClassCategory::AV_CLASS_CATEGORY_NA,
        get_category: None,
        query_ranges: None,
        child_next,
        child_class_iterate: None,
        state_flags_offset: 0,
    }
}

/// `(major << 16) | (minor << 8) | micro`, as `AV_VERSION_INT` spells it.
#[allow(clippy::cast_possible_wrap)]
const LIBAVUTIL_VERSION_INT: c_int = ((ff::LIBAVUTIL_VERSION_MAJOR << 16)
    | (ff::LIBAVUTIL_VERSION_MINOR << 8)
    | ff::LIBAVUTIL_VERSION_MICRO) as c_int;

static SOURCE_CLASS: SyncClass = SyncClass(class(c"uuav-http", SOURCE_OPTIONS.0.as_ptr(), None));

/// Class of the custom `AVIOContext`: no options of its own, the [`Source`]
/// as its one child.
static CONTEXT_CLASS: SyncClass =
    SyncClass(class(c"AVIOContext", ptr::null(), Some(context_child_next)));

unsafe extern "C" fn context_child_next(ctx: *mut c_void, prev: *mut c_void) -> *mut c_void {
    if prev.is_null() {
        unsafe { (*ctx.cast::<ff::AVIOContext>()).opaque }
    } else {
        ptr::null_mut()
    }
}

fn new_context(source: Source) -> Result<*mut ff::AVIOContext> {
    let seekable = source.seekable();
    let buffer = unsafe { ff::av_malloc(BUFFER_SIZE as usize) }.cast::<u8>();
    ensure!(!buffer.is_null(), "av_malloc failed");
    let opaque = Box::into_raw(Box::new(source));
    let ctx = unsafe {
        ff::avio_alloc_context(
            buffer,
            BUFFER_SIZE,
            0,
            opaque.cast(),
            Some(read_packet),
            None,
            Some(seek_packet),
        )
    };
    if ctx.is_null() {
        unsafe {
            drop(Box::from_raw(opaque));
            ff::av_free(buffer.cast());
        }
        bail!("avio_alloc_context failed");
    }
    unsafe {
        (*ctx).av_class = &raw const CONTEXT_CLASS.0;
        (*ctx).seekable = if seekable { AVIO_SEEKABLE_NORMAL } else { 0 };
    }
    Ok(ctx)
}

/// Frees a context [`new_context`] made, buffer and source included.
unsafe fn free_context(ctx: *mut ff::AVIOContext) {
    let mut ctx = ctx;
    unsafe {
        let source = (*ctx).opaque.cast::<Source>();
        (*ctx).opaque = ptr::null_mut();
        if !source.is_null() {
            drop(Box::from_raw(source));
        }
        // FFmpeg may have swapped the buffer for a larger one by now
        ff::av_free((*ctx).buffer.cast());
        (*ctx).buffer = ptr::null_mut();
        ff::avio_context_free(&raw mut ctx);
    }
}

unsafe extern "C" fn read_packet(opaque: *mut c_void, buf: *mut u8, size: c_int) -> c_int {
    let (Some(source), Ok(size)) = (
        unsafe { opaque.cast::<Source>().as_mut() },
        usize::try_from(size),
    ) else {
        return ff::AVERROR(libc::EINVAL);
    };
    if size == 0 {
        return 0;
    }
    let out = unsafe { std::slice::from_raw_parts_mut(buf, size) };
    match source.read(out) {
        Ok(0) => ff::AVERROR_EOF,
        Ok(n) => c_int::try_from(n).unwrap_or(c_int::MAX),
        Err(error) => report(&error),
    }
}

unsafe extern "C" fn seek_packet(opaque: *mut c_void, offset: i64, whence: c_int) -> i64 {
    let Some(source) = (unsafe { opaque.cast::<Source>().as_mut() }) else {
        return i64::from(ff::AVERROR(libc::EINVAL));
    };
    match source.seek(offset, whence) {
        Ok(pos) => i64::try_from(pos).unwrap_or(i64::MAX),
        Err(error) => i64::from(report(&error)),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Ranges {
    Unknown,
    Honoured,
    /// The server answered a `Range` request with the whole resource.
    Ignored,
}

struct HttpSource {
    agent: ureq::Agent,
    cancel: ReadOnlyCancelToken,
    url: String,
    /// Url of the first response, after redirects.
    location: String,
    /// Resource offset of the next byte `read` returns.
    pos: u64,
    /// End of the window the demuxer asked for (a byte-range segment).
    end: Option<u64>,
    /// Resource length, once a response stated it.
    length: Option<u64>,
    ranges: Ranges,
    /// Open response, positioned at `pos`.
    body: Option<ureq::BodyReader<'static>>,
    /// Offset a failed read was already retried at: one reconnect per stall.
    retried_at: Option<u64>,
}

impl HttpSource {
    /// Connects eagerly, so that a refused or missing resource fails the
    /// open rather than the first read.
    fn open(
        agent: ureq::Agent,
        cancel: ReadOnlyCancelToken,
        url: &str,
        offset: u64,
        end: Option<u64>,
    ) -> Result<Self> {
        let mut source = Self {
            agent,
            cancel,
            url: url.to_owned(),
            location: url.to_owned(),
            pos: offset,
            end,
            length: None,
            ranges: Ranges::Unknown,
            body: None,
            retried_at: None,
        };
        source.connect()?;
        Ok(source)
    }

    /// As FFmpeg's http protocol decides it: the server advertised byte
    /// ranges (`Accept-Ranges: bytes`) or served one (a 206). A length alone
    /// is not enough; the demuxer then reads in order, and a backward seek
    /// still tries a Range request.
    fn seekable(&self) -> bool {
        self.ranges == Ranges::Honoured
    }

    /// Where reads stop: the requested window's end, else the resource end.
    fn limit(&self) -> Option<u64> {
        match (self.end, self.length) {
            (Some(end), Some(length)) => Some(end.min(length)),
            (end, length) => end.or(length),
        }
    }

    fn check_cancel(&self) -> Result<()> {
        if self.cancel.is_cancelled() {
            return Err(Failure {
                code: ff::AVERROR_EXIT,
                message: "cancelled".to_owned(),
            }
            .into());
        }
        Ok(())
    }

    /// Requests the bytes from `pos` on and leaves the body open there. A
    /// window that starts past the end leaves no body: reads see EOF.
    fn connect(&mut self) -> Result<()> {
        self.body = None;
        self.check_cancel()?;
        let at = self.pos;
        let ranged = (at > 0 || self.end.is_some()) && self.ranges != Ranges::Ignored;
        let mut request = self
            .agent
            .get(&self.url)
            .header("Accept-Encoding", "identity");
        if ranged {
            let range = self.end.map_or_else(
                || format!("bytes={at}-"),
                |end| format!("bytes={at}-{}", end.saturating_sub(1)),
            );
            request = request.header("Range", range);
        }
        let response = match request.call() {
            Ok(response) => response,
            Err(error) => {
                self.check_cancel()?;
                return Err(fail(libc::EIO, format!("GET {}: {error}", self.url)));
            }
        };
        let status = response.status().as_u16();
        let field = |name: &str| {
            response
                .headers()
                .get(name)
                .and_then(|value| value.to_str().ok())
                .map(|value| value.trim().to_owned())
        };
        let content_length = field("content-length").and_then(|v| v.parse::<u64>().ok());
        let content_range = field("content-range");
        let accept_ranges = field("accept-ranges");
        if self.location == self.url {
            self.location = response.get_uri().to_string();
        }
        match status {
            200 => {
                if ranged {
                    self.ranges = Ranges::Ignored;
                } else if let Some(accept) = accept_ranges {
                    if accept.eq_ignore_ascii_case("none") {
                        self.ranges = Ranges::Ignored;
                    } else if accept.eq_ignore_ascii_case("bytes") {
                        self.ranges = Ranges::Honoured;
                    }
                }
                self.length = content_length;
                let mut reader = response.into_body().into_reader();
                if at > 0 {
                    if at > DISCARD_LIMIT {
                        return Err(fail(
                            libc::ESPIPE,
                            format!(
                                "{} ignores Range requests; offset {at} is out of reach",
                                self.url
                            ),
                        ));
                    }
                    skip(&mut reader, at)
                        .with_context(|| format!("GET {}: skipping to {at}", self.url))?;
                }
                self.body = Some(reader);
            }
            206 => {
                self.ranges = Ranges::Honoured;
                let (start, total) =
                    parse_content_range(content_range.as_deref().unwrap_or_default())?;
                if start != Some(at) {
                    return Err(fail(
                        libc::EIO,
                        format!(
                            "GET {}: range starts at {start:?}, asked for {at}",
                            self.url
                        ),
                    ));
                }
                if total.is_some() {
                    self.length = total;
                }
                self.body = Some(response.into_body().into_reader());
            }
            416 => {
                let total = content_range
                    .as_deref()
                    .and_then(|value| parse_content_range(value).ok())
                    .and_then(|(_, total)| total);
                self.length = Some(total.unwrap_or(at));
            }
            other => {
                return Err(fail(
                    status_errno(other),
                    format!("GET {}: HTTP {other}", self.url),
                ));
            }
        }
        Ok(())
    }

    /// Reads into `out`; `Ok(0)` is the end of the window.
    fn read(&mut self, out: &mut [u8]) -> Result<usize> {
        loop {
            self.check_cancel()?;
            let room = match self.limit() {
                Some(limit) if self.pos >= limit => return Ok(0),
                Some(limit) => usize::try_from(limit.saturating_sub(self.pos))
                    .map_or(out.len(), |left| left.min(out.len())),
                None => out.len(),
            };
            let buf = head_mut(out, room);
            if buf.is_empty() {
                return Ok(0);
            }
            if self.body.is_none() {
                self.connect()?;
            }
            let Some(body) = self.body.as_mut() else {
                return Ok(0);
            };
            match body.read(buf) {
                Ok(0) => {
                    self.body = None;
                    match self.limit() {
                        Some(limit) if self.pos < limit => {
                            self.retry(&format!("stream ends at {} of {limit}", self.pos))?;
                        }
                        Some(_) => return Ok(0),
                        None => {
                            self.length = Some(self.pos);
                            return Ok(0);
                        }
                    }
                }
                Ok(n) => {
                    self.pos = self
                        .pos
                        .saturating_add(u64::try_from(n).unwrap_or(u64::MAX));
                    self.retried_at = None;
                    return Ok(n);
                }
                Err(error) => {
                    self.body = None;
                    self.check_cancel()?;
                    self.retry(&format!("read failed: {error}"))?;
                }
            }
        }
    }

    /// One reconnect per offset; a second failure at the same place is
    /// final.
    fn retry(&mut self, why: &str) -> Result<()> {
        if self.retried_at == Some(self.pos) {
            return Err(fail(
                libc::EIO,
                format!("GET {}: {why}, giving up at {}", self.url, self.pos),
            ));
        }
        log(
            LOG_WARNING,
            &format!("GET {}: {why}, reconnecting at {}", self.url, self.pos),
        );
        self.retried_at = Some(self.pos);
        Ok(())
    }

    fn seek(&mut self, offset: i64, whence: c_int) -> Result<u64> {
        self.check_cancel()?;
        if whence & AVSEEK_SIZE != 0 {
            return self
                .length
                .ok_or_else(|| fail(libc::ENOSYS, "length unknown"));
        }
        let base = match whence & !AVSEEK_FORCE {
            libc::SEEK_SET => 0,
            libc::SEEK_CUR => self.pos,
            libc::SEEK_END => self
                .length
                .ok_or_else(|| fail(libc::ENOSYS, "length unknown"))?,
            _ => return Err(fail(libc::EINVAL, format!("whence {whence}"))),
        };
        let target = i64::try_from(base)
            .ok()
            .and_then(|base| base.checked_add(offset))
            .and_then(|target| u64::try_from(target).ok())
            .ok_or_else(|| fail(libc::EINVAL, format!("seek to {base}{offset:+}")))?;
        if target == self.pos {
            return Ok(target);
        }
        if self.body.is_some()
            && target > self.pos
            && target.saturating_sub(self.pos) <= DISCARD_LIMIT
            && self.skip_to(target).is_ok()
        {
            return Ok(self.pos);
        }
        let before = self.pos;
        self.pos = target;
        if let Err(error) = self.connect() {
            self.pos = before;
            return Err(error);
        }
        Ok(target)
    }

    /// Reads and drops the bytes up to `target` on the open body.
    fn skip_to(&mut self, target: u64) -> Result<()> {
        let mut scratch = [0u8; 16 * 1024];
        while self.pos < target {
            let want = usize::try_from(target.saturating_sub(self.pos))
                .map_or(scratch.len(), |left| left.min(scratch.len()));
            if self.read(head_mut(&mut scratch, want))? == 0 {
                bail!("stream ends before {target}");
            }
        }
        Ok(())
    }
}

fn head_mut(buf: &mut [u8], n: usize) -> &mut [u8] {
    let n = n.min(buf.len());
    buf.split_at_mut(n).0
}

fn skip(reader: &mut impl Read, n: u64) -> Result<()> {
    let skipped = std::io::copy(&mut reader.by_ref().take(n), &mut std::io::sink())?;
    ensure!(skipped == n, "stream ends while skipping");
    Ok(())
}

/// `bytes <start>-<end>/<total>` or `bytes */<total>`; `*` is unknown.
fn parse_content_range(value: &str) -> Result<(Option<u64>, Option<u64>)> {
    let spec = value
        .trim()
        .strip_prefix("bytes")
        .ok_or_else(|| anyhow!("Content-Range unit: {value}"))?
        .trim_start();
    let (range, total) = spec
        .split_once('/')
        .ok_or_else(|| anyhow!("Content-Range: {value}"))?;
    let start = if range == "*" {
        None
    } else {
        let first = range.split_once('-').map_or(range, |(first, _)| first);
        Some(
            first
                .parse::<u64>()
                .with_context(|| format!("Content-Range start: {value}"))?,
        )
    };
    let total = total.trim();
    let total = if total == "*" {
        None
    } else {
        Some(
            total
                .parse::<u64>()
                .with_context(|| format!("Content-Range total: {value}"))?,
        )
    };
    Ok((start, total))
}

const fn status_errno(status: u16) -> c_int {
    match status {
        404 | 410 => libc::ENOENT,
        401 | 403 => libc::EACCES,
        _ => libc::EIO,
    }
}

/// Where decrypted bytes go: the caller's buffer while it has room, then
/// `spill`, which the next read serves first.
struct Sink<'a> {
    out: &'a mut [u8],
    filled: usize,
    spill: &'a mut Vec<u8>,
}

impl Sink<'_> {
    fn put(&mut self, plain: &[u8]) {
        let direct = self.out.len().saturating_sub(self.filled).min(plain.len());
        let (now, later) = plain.split_at(direct);
        let end = self.filled.saturating_add(direct);
        if let Some(dst) = self.out.get_mut(self.filled..end) {
            dst.copy_from_slice(now);
        }
        self.filled = end;
        self.spill.extend_from_slice(later);
    }
}

/// AES-128-CBC with PKCS#7 padding, decrypted as the ciphertext streams in.
struct Cbc {
    cipher: aes::Aes128,
    prev: [u8; 16],
    /// Last decrypted block, held back until the next one proves it is not
    /// the padded final block.
    held: Option<[u8; 16]>,
    /// Ciphertext bytes short of a block.
    tail: Vec<u8>,
}

impl Cbc {
    fn new(key: [u8; 16], iv: [u8; 16]) -> Self {
        use aes::cipher::KeyInit as _;
        Self {
            cipher: aes::Aes128::new(&key.into()),
            prev: iv,
            held: None,
            tail: Vec::with_capacity(16),
        }
    }

    fn feed(&mut self, mut input: &[u8], sink: &mut Sink<'_>) {
        if !self.tail.is_empty() {
            let need = 16usize.saturating_sub(self.tail.len()).min(input.len());
            let (head, rest) = input.split_at(need);
            self.tail.extend_from_slice(head);
            input = rest;
            match <[u8; 16]>::try_from(self.tail.as_slice()) {
                Ok(block) => {
                    self.block(block, sink);
                    self.tail.clear();
                }
                Err(_) => return,
            }
        }
        let (blocks, rest) = input.as_chunks::<16>();
        for block in blocks {
            self.block(*block, sink);
        }
        self.tail.extend_from_slice(rest);
    }

    fn block(&mut self, cipher_text: [u8; 16], sink: &mut Sink<'_>) {
        use aes::cipher::BlockDecrypt as _;
        let mut plain = aes::Block::from(cipher_text);
        self.cipher.decrypt_block(&mut plain);
        let mut plain: [u8; 16] = plain.into();
        for (p, c) in plain.iter_mut().zip(self.prev) {
            *p ^= c;
        }
        self.prev = cipher_text;
        if let Some(ready) = self.held.replace(plain) {
            sink.put(&ready);
        }
    }

    /// Emits the final block without its padding.
    fn finish(&mut self, sink: &mut Sink<'_>) -> Result<()> {
        ensure!(
            self.tail.is_empty(),
            "ciphertext is not a whole number of blocks"
        );
        let last = self
            .held
            .take()
            .ok_or_else(|| anyhow!("empty ciphertext"))?;
        let pad = usize::from(last.last().copied().unwrap_or(0));
        ensure!(
            (1..=16).contains(&pad) && last.iter().rev().take(pad).all(|b| usize::from(*b) == pad),
            "bad PKCS#7 padding"
        );
        sink.put(last.get(..16usize.saturating_sub(pad)).unwrap_or_default());
        Ok(())
    }
}

/// Ciphertext read from the body per call.
const AES_CHUNK: usize = 16 * 1024;

/// An AES-128 segment, decrypted as FFmpeg reads it.
struct AesSource {
    inner: HttpSource,
    cbc: Cbc,
    /// Ciphertext scratch, allocated once.
    cipher_text: Box<[u8]>,
    /// Plaintext the caller's buffer had no room for, served first.
    spill: Vec<u8>,
    taken: usize,
    done: bool,
}

impl AesSource {
    fn new(inner: HttpSource, key: [u8; 16], iv: [u8; 16]) -> Self {
        Self {
            inner,
            cbc: Cbc::new(key, iv),
            cipher_text: vec![0; AES_CHUNK].into_boxed_slice(),
            spill: Vec::new(),
            taken: 0,
            done: false,
        }
    }

    /// Decrypts straight into `out`; only what does not fit is kept.
    fn read(&mut self, out: &mut [u8]) -> Result<usize> {
        let pending = self.spill.len().saturating_sub(self.taken);
        if pending > 0 {
            let n = pending.min(out.len());
            let end = self.taken.saturating_add(n);
            if let (Some(dst), Some(src)) = (out.get_mut(..n), self.spill.get(self.taken..end)) {
                dst.copy_from_slice(src);
            }
            self.taken = end;
            if self.taken >= self.spill.len() {
                self.spill.clear();
                self.taken = 0;
            }
            return Ok(n);
        }
        while !self.done {
            let n = self.inner.read(&mut self.cipher_text)?;
            let mut sink = Sink {
                out: &mut *out,
                filled: 0,
                spill: &mut self.spill,
            };
            if n == 0 {
                self.cbc.finish(&mut sink)?;
                self.done = true;
            } else {
                self.cbc
                    .feed(self.cipher_text.get(..n).unwrap_or_default(), &mut sink);
            }
            if sink.filled > 0 {
                return Ok(sink.filled);
            }
        }
        Ok(0)
    }

    /// `AVSEEK_SIZE` reports the ciphertext length, 1 to 16 bytes above the
    /// plaintext (the padding is only known at the end), as FFmpeg's own
    /// crypto protocol does; the HLS demuxer uses it to size the buffer for
    /// an encrypted init section, where an upper bound is what it needs.
    /// Positions are not seekable: segments are read in order.
    fn seek(&self, _offset: i64, whence: c_int) -> Result<u64> {
        if whence & AVSEEK_SIZE != 0 {
            return self
                .inner
                .length
                .ok_or_else(|| fail(libc::ENOSYS, "length unknown"));
        }
        Err(fail(libc::ENOSYS, "encrypted segments are read in order"))
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects
)]
mod tests {
    use super::*;
    use crate::playback::input::Input;
    use crate::playback::util::CancelToken;
    use std::collections::HashMap;
    use std::fmt::Write as _;
    use std::io::{BufRead, BufReader, Write};
    use std::net::{TcpListener, TcpStream};
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::time::{Duration, Instant};

    #[derive(Clone, Copy)]
    enum Mode {
        Ranges,
        /// Advertises no ranges and answers every request with the whole
        /// resource.
        NoRanges,
        /// Closes the first body after this many bytes.
        TruncateOnce(usize),
        /// Sends the head and the first byte, then holds the connection.
        Stall,
    }

    struct Server {
        base: String,
        requests: Arc<AtomicUsize>,
    }

    impl Server {
        fn url(&self, path: &str) -> String {
            format!("{}{path}", self.base)
        }
    }

    fn serve(files: Vec<(&'static str, Vec<u8>)>, mode: Mode) -> Server {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let requests = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&requests);
        let files: Arc<HashMap<&str, Vec<u8>>> = Arc::new(files.into_iter().collect());
        let truncated = Arc::new(AtomicBool::new(false));
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                counter.fetch_add(1, Ordering::SeqCst);
                let files = Arc::clone(&files);
                let truncated = Arc::clone(&truncated);
                std::thread::spawn(move || {
                    let _ = respond(stream, &files, mode, &truncated);
                });
            }
        });
        Server { base, requests }
    }

    fn respond(
        mut stream: TcpStream,
        files: &HashMap<&str, Vec<u8>>,
        mode: Mode,
        truncated: &AtomicBool,
    ) -> std::io::Result<()> {
        let mut reader = BufReader::new(stream.try_clone()?);
        let mut line = String::new();
        reader.read_line(&mut line)?;
        let path = line.split_whitespace().nth(1).unwrap_or("/").to_owned();
        let mut range = None;
        loop {
            line.clear();
            if reader.read_line(&mut line)? == 0 || line.trim().is_empty() {
                break;
            }
            // ureq writes header names in lower case
            if let Some(spec) = line.to_ascii_lowercase().strip_prefix("range: bytes=") {
                let spec = spec.trim();
                let (first, last) = spec.split_once('-').unwrap_or((spec, ""));
                range = Some((first.parse::<u64>().unwrap_or(0), last.parse::<u64>().ok()));
            }
        }
        let Some(data) = files.get(path.as_str()) else {
            return write!(
                stream,
                "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
            );
        };
        let total = data.len() as u64;
        let (status, start, end) = match (range, mode) {
            (Some((first, _)), Mode::Ranges | Mode::TruncateOnce(_)) if first >= total => {
                return write!(
                    stream,
                    "HTTP/1.1 416 Range Not Satisfiable\r\nContent-Range: bytes */{total}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                );
            }
            (Some((first, last)), Mode::Ranges | Mode::TruncateOnce(_)) => {
                (206, first, last.map_or(total, |last| (last + 1).min(total)))
            }
            _ => (200, 0, total),
        };
        let body = &data[start as usize..end as usize];
        let mut head = format!(
            "HTTP/1.1 {status} {}\r\nContent-Length: {}\r\nConnection: close\r\n",
            if status == 206 {
                "Partial Content"
            } else {
                "OK"
            },
            body.len()
        );
        if !matches!(mode, Mode::NoRanges) {
            head += "Accept-Ranges: bytes\r\n";
        }
        if status == 206 {
            let _ = write!(head, "Content-Range: bytes {start}-{}/{total}\r\n", end - 1);
        }
        head += "\r\n";
        stream.write_all(head.as_bytes())?;
        match mode {
            Mode::TruncateOnce(keep) if !truncated.swap(true, Ordering::SeqCst) => {
                stream.write_all(&body[..keep.min(body.len())])
            }
            Mode::Stall => {
                stream.write_all(&body[..1])?;
                // holds the connection until the client drops it
                let mut sink = [0u8; 1];
                let _ = stream.read(&mut sink);
                Ok(())
            }
            _ => stream.write_all(body),
        }
    }

    fn pattern(len: usize) -> Vec<u8> {
        (0..len).map(|i| (i % 251) as u8).collect()
    }

    fn cancel() -> ReadOnlyCancelToken {
        CancelToken::new().into()
    }

    fn open(server: &Server, path: &str) -> Result<HttpSource> {
        HttpSource::open(trust::agent(), cancel(), &server.url(path), 0, None)
    }

    fn read_all(source: &mut HttpSource) -> Result<Vec<u8>> {
        let mut out = Vec::new();
        let mut buf = [0u8; 7919];
        loop {
            let n = source.read(&mut buf)?;
            if n == 0 {
                return Ok(out);
            }
            out.extend_from_slice(&buf[..n]);
        }
    }

    fn read_exact(source: &mut HttpSource, len: usize) -> Result<Vec<u8>> {
        let mut out = vec![0u8; len];
        let mut filled = 0;
        while filled < len {
            let n = source.read(&mut out[filled..])?;
            ensure!(n > 0, "short read at {filled}");
            filled += n;
        }
        Ok(out)
    }

    #[test]
    fn constants_match_the_bindings() {
        assert_eq!(i64::from(ff::AVSEEK_SIZE), i64::from(AVSEEK_SIZE));
        assert_eq!(i64::from(ff::AVSEEK_FORCE), i64::from(AVSEEK_FORCE));
        assert_eq!(
            i64::from(ff::AVIO_SEEKABLE_NORMAL),
            i64::from(AVIO_SEEKABLE_NORMAL)
        );
        assert_eq!(i64::from(ff::AVIO_FLAG_WRITE), i64::from(AVIO_FLAG_WRITE));
        assert_eq!(i64::from(ff::AV_LOG_ERROR), i64::from(LOG_ERROR));
        assert_eq!(i64::from(ff::AV_LOG_WARNING), i64::from(LOG_WARNING));
        let export_readonly =
            i64::from(ff::AV_OPT_FLAG_EXPORT) | i64::from(ff::AV_OPT_FLAG_READONLY);
        assert_eq!(export_readonly, i64::from(OPT_FLAG_EXPORT_READONLY));
    }

    #[test]
    fn content_range_forms() {
        assert_eq!(
            parse_content_range("bytes 100-199/1000").unwrap(),
            (Some(100), Some(1000))
        );
        assert_eq!(parse_content_range("bytes 5-9/*").unwrap(), (Some(5), None));
        assert_eq!(
            parse_content_range("bytes */321").unwrap(),
            (None, Some(321))
        );
        assert!(parse_content_range("items 1-2/3").is_err());
    }

    #[test]
    fn targets() {
        let plain = HttpTarget::parse("HTTPS://host/a.m3u8").unwrap();
        assert_eq!(
            (plain.url, plain.scheme.as_str(), plain.aes),
            ("HTTPS://host/a.m3u8", "https", false)
        );
        let aes = HttpTarget::parse("crypto+http://host/seg.ts").unwrap();
        assert_eq!(
            (aes.url, aes.scheme.as_str(), aes.aes),
            ("http://host/seg.ts", "http", true)
        );
        assert!(HttpTarget::parse("file:///tmp/x").is_none());
        assert!(HttpTarget::parse("data:video/mp4;base64,AAAA").is_none());
        assert!(HttpTarget::parse("crypto:/relative/seg.ts").is_none());
    }

    fn encrypt_cbc(key: [u8; 16], iv: [u8; 16], plain: &[u8]) -> Vec<u8> {
        use aes::cipher::{BlockEncrypt as _, KeyInit as _};
        let cipher = aes::Aes128::new(&key.into());
        let pad = 16 - plain.len() % 16;
        let mut padded = plain.to_vec();
        padded.extend(std::iter::repeat_n(pad as u8, pad));
        let mut prev = iv;
        let mut out = Vec::with_capacity(padded.len());
        for chunk in padded.as_chunks::<16>().0 {
            let mut block = [0u8; 16];
            for (b, (c, p)) in block.iter_mut().zip(chunk.iter().zip(prev)) {
                *b = c ^ p;
            }
            let mut block = aes::Block::from(block);
            cipher.encrypt_block(&mut block);
            prev = block.into();
            out.extend_from_slice(&prev);
        }
        out
    }

    const KEY: [u8; 16] = *b"0123456789abcdef";
    const IV: [u8; 16] = [7; 16];

    #[test]
    fn sink_fills_the_buffer_then_spills() {
        let mut out = [0u8; 20];
        let mut spill = Vec::new();
        let mut sink = Sink {
            out: &mut out,
            filled: 0,
            spill: &mut spill,
        };
        sink.put(&[1; 16]);
        sink.put(&[2; 16]);
        let filled = sink.filled;
        assert_eq!(filled, 20);
        assert_eq!(out[..16], [1; 16]);
        assert_eq!(out[16..], [2; 4]);
        assert_eq!(spill, [2; 12]);
    }

    #[test]
    fn cbc_streams_in_odd_chunks_and_strips_the_padding() {
        for len in [0usize, 1, 15, 16, 17, 1000] {
            let plain = pattern(len);
            let cipher_text = encrypt_cbc(KEY, IV, &plain);
            let mut cbc = Cbc::new(KEY, IV);
            let mut out = Vec::new();
            let mut none = [0u8; 0];
            let mut sink = Sink {
                out: &mut none,
                filled: 0,
                spill: &mut out,
            };
            for chunk in cipher_text.chunks(7) {
                cbc.feed(chunk, &mut sink);
            }
            cbc.finish(&mut sink).unwrap();
            assert_eq!(out, plain, "len {len}");
        }
    }

    #[test]
    fn cbc_rejects_bad_padding_and_ragged_input() {
        let mut cipher_text = encrypt_cbc(KEY, IV, &pattern(40));
        let mut cbc = Cbc::new(KEY, IV);
        let mut out = Vec::new();
        let mut none = [0u8; 0];
        let mut sink = Sink {
            out: &mut none,
            filled: 0,
            spill: &mut out,
        };
        cbc.feed(&cipher_text, &mut sink);
        cbc.feed(&[1, 2, 3], &mut sink);
        assert!(
            cbc.finish(&mut sink)
                .unwrap_err()
                .to_string()
                .contains("whole number")
        );

        let last = cipher_text.len() - 1;
        cipher_text[last] ^= 0xff;
        let mut cbc = Cbc::new(KEY, IV);
        let mut out = Vec::new();
        let mut sink = Sink {
            out: &mut none,
            filled: 0,
            spill: &mut out,
        };
        cbc.feed(&cipher_text, &mut sink);
        assert!(
            cbc.finish(&mut sink)
                .unwrap_err()
                .to_string()
                .contains("padding")
        );
    }

    #[test]
    fn aes_source_decrypts_across_odd_reads() -> Result<()> {
        let plain = pattern(100_003);
        let server = serve(vec![("/seg", encrypt_cbc(KEY, IV, &plain))], Mode::Ranges);
        for size in [1usize, 15, 16, 17, 4_099, 65_536] {
            let mut source = AesSource::new(open(&server, "/seg")?, KEY, IV);
            assert_eq!(source.seek(0, AVSEEK_SIZE)?, 100_016);
            let mut out = Vec::new();
            let mut buf = vec![0u8; size];
            loop {
                let n = source.read(&mut buf)?;
                if n == 0 {
                    break;
                }
                out.extend_from_slice(&buf[..n]);
            }
            assert_eq!(out, plain, "read size {size}");
        }
        Ok(())
    }

    #[test]
    fn reads_the_whole_resource_in_order() -> Result<()> {
        let data = pattern(200_000);
        let server = serve(vec![("/a.bin", data.clone())], Mode::Ranges);
        let mut source = open(&server, "/a.bin")?;
        assert_eq!(source.length, Some(200_000));
        assert!(source.seekable());
        assert_eq!(read_all(&mut source)?, data);
        assert_eq!(server.requests.load(Ordering::SeqCst), 1);
        Ok(())
    }

    #[test]
    fn seeks_with_ranges_and_skips_short_forward_seeks() -> Result<()> {
        let data = pattern(3_000_000);
        let server = serve(vec![("/a.bin", data.clone())], Mode::Ranges);
        let mut source = open(&server, "/a.bin")?;
        assert_eq!(source.seek(0, AVSEEK_SIZE)?, 3_000_000);

        // far forward: a Range request
        assert_eq!(source.seek(2_000_000, libc::SEEK_SET)?, 2_000_000);
        assert_eq!(read_exact(&mut source, 10)?, data[2_000_000..2_000_010]);
        assert_eq!(server.requests.load(Ordering::SeqCst), 2);

        // forward, within the discard window: no new request
        assert_eq!(source.seek(4_000, libc::SEEK_CUR)?, 2_004_010);
        assert_eq!(read_exact(&mut source, 10)?, data[2_004_010..2_004_020]);
        assert_eq!(server.requests.load(Ordering::SeqCst), 2);

        // backward: a Range request
        assert_eq!(source.seek(1_000, libc::SEEK_SET)?, 1_000);
        assert_eq!(read_exact(&mut source, 10)?, data[1_000..1_010]);
        assert_eq!(server.requests.load(Ordering::SeqCst), 3);

        assert_eq!(source.seek(-16, libc::SEEK_END)?, 2_999_984);
        assert_eq!(read_all(&mut source)?, data[2_999_984..]);
        assert_eq!(server.requests.load(Ordering::SeqCst), 4);

        // past the end: a 416, read sees EOF
        assert_eq!(source.seek(4_000_000, libc::SEEK_SET)?, 4_000_000);
        assert_eq!(source.read(&mut [0u8; 8])?, 0);
        assert_eq!(server.requests.load(Ordering::SeqCst), 5);
        assert!(source.seek(1, 99).is_err());
        Ok(())
    }

    #[test]
    fn servers_without_ranges_are_read_from_the_start() -> Result<()> {
        let data = pattern(1_200_000);
        let server = serve(vec![("/a.bin", data.clone())], Mode::NoRanges);
        let mut source = open(&server, "/a.bin")?;
        assert_eq!(source.length, Some(1_200_000));
        assert!(
            !source.seekable(),
            "a length alone does not make a resource seekable"
        );
        // forward, within the discard window: read and dropped
        assert_eq!(source.seek(20_000, libc::SEEK_SET)?, 20_000);
        assert_eq!(read_exact(&mut source, 8)?, data[20_000..20_008]);
        // a backward seek asks for a range, gets the whole resource and skips
        assert_eq!(source.seek(100, libc::SEEK_SET)?, 100);
        assert_eq!(source.ranges, Ranges::Ignored);
        assert!(!source.seekable());
        assert_eq!(read_exact(&mut source, 8)?, data[100..108]);
        // beyond the discard window the seek fails loudly and the position stays
        let error = source.seek(1_100_000, libc::SEEK_SET).unwrap_err();
        assert_eq!(averror(&error), ff::AVERROR(libc::ESPIPE));
        assert!(error.to_string().contains("out of reach"), "{error}");
        assert_eq!(read_exact(&mut source, 8)?, data[108..116]);
        Ok(())
    }

    #[test]
    fn a_truncated_body_is_resumed_once() -> Result<()> {
        let data = pattern(120_000);
        let server = serve(vec![("/a.bin", data.clone())], Mode::TruncateOnce(30_000));
        let mut source = open(&server, "/a.bin")?;
        assert_eq!(read_all(&mut source)?, data);
        assert_eq!(server.requests.load(Ordering::SeqCst), 2);
        Ok(())
    }

    #[test]
    fn missing_resources_fail_the_open() -> Result<()> {
        let server = serve(vec![], Mode::Ranges);
        let Err(error) = open(&server, "/nope.bin") else {
            bail!("a 404 must fail the open");
        };
        assert_eq!(averror(&error), ff::AVERROR(libc::ENOENT));
        assert!(error.to_string().contains("HTTP 404"), "{error}");
        Ok(())
    }

    #[test]
    fn cancelling_interrupts_a_stalled_read() -> Result<()> {
        let server = serve(vec![("/a.bin", pattern(1000))], Mode::Stall);
        let token = CancelToken::new();
        let reader: ReadOnlyCancelToken = token.clone().into();
        let agent = trust::agent();
        let doomed = agent.clone();
        reader.on_cancel(move || doomed.shutdown());
        let mut source = HttpSource::open(agent, reader, &server.url("/a.bin"), 0, None)?;
        let started = Instant::now();
        let (first_done, first_seen) = std::sync::mpsc::channel();
        let blocked = std::thread::spawn(move || {
            let mut buf = [0u8; 64];
            let first = source.read(&mut buf).map_err(|e| averror(&e));
            let _ = first_done.send(());
            let second = source.read(&mut buf).map_err(|e| averror(&e));
            (first, second)
        });
        first_seen.recv()?;
        token.cancel();
        let (first, second) = blocked.join().map_err(|_| anyhow!("reader panicked"))?;
        assert_eq!(first, Ok(1));
        assert_eq!(second, Err(ff::AVERROR_EXIT));
        assert!(started.elapsed() < Duration::from_secs(10));
        Ok(())
    }

    /// 1 s of 8 kHz mono 16-bit silence with its RIFF header.
    fn wav() -> Vec<u8> {
        let samples = 8_000u32;
        let data_len = samples * 2;
        let mut out = Vec::with_capacity(44 + data_len as usize);
        out.extend_from_slice(b"RIFF");
        out.extend_from_slice(&(36 + data_len).to_le_bytes());
        out.extend_from_slice(b"WAVEfmt ");
        out.extend_from_slice(&16u32.to_le_bytes());
        out.extend_from_slice(&1u16.to_le_bytes());
        out.extend_from_slice(&1u16.to_le_bytes());
        out.extend_from_slice(&8_000u32.to_le_bytes());
        out.extend_from_slice(&16_000u32.to_le_bytes());
        out.extend_from_slice(&2u16.to_le_bytes());
        out.extend_from_slice(&16u16.to_le_bytes());
        out.extend_from_slice(b"data");
        out.extend_from_slice(&data_len.to_le_bytes());
        out.resize(44 + data_len as usize, 0);
        out
    }

    fn whitelist(list: &str) -> Result<StreamingProtocol> {
        let list = CString::new(list)?;
        unsafe { StreamingProtocol::new(list.as_ptr()) }
    }

    #[test]
    fn whitelist_gates_http_before_any_request() -> Result<()> {
        let server = serve(vec![("/a.wav", wav())], Mode::Ranges);
        let error = match Input::open(cancel(), &server.url("/a.wav"), &whitelist("file,https")?) {
            Ok(_) => bail!("http must be refused"),
            Err(error) => error.to_string(),
        };
        assert!(error.contains("whitelist"), "{error}");
        assert_eq!(server.requests.load(Ordering::SeqCst), 0);
        Ok(())
    }

    #[test]
    fn demuxes_a_stream_served_over_http() -> Result<()> {
        let server = serve(vec![("/a.wav", wav())], Mode::Ranges);
        let input = Input::open(cancel(), &server.url("/a.wav"), &whitelist("http")?)?;
        assert_eq!(input.nb_streams(), 1);
        let duration = input.duration().ok_or_else(|| anyhow!("no duration"))?;
        assert!((duration - 1.0).abs() < 0.01, "{duration}");
        Ok(())
    }

    #[test]
    fn release_frees_what_the_demuxer_left_open() -> Result<()> {
        let server = serve(vec![("/a.wav", wav())], Mode::Ranges);
        let fmt = unsafe { ff::avformat_alloc_context() };
        ensure!(!fmt.is_null(), "avformat_alloc_context failed");
        let hooks = unsafe { IoHooks::install(fmt, cancel(), &whitelist("http")?) }?;
        let opened = unsafe { (*hooks).open_top_level(&server.url("/a.wav")) }?;
        ensure!(opened.is_some(), "http must be served by the hooks");
        unsafe {
            IoHooks::release(hooks);
            ff::avformat_free_context(fmt);
        }
        Ok(())
    }

    #[test]
    fn a_closed_input_survives_a_later_cancel() -> Result<()> {
        let server = serve(vec![("/a.wav", wav())], Mode::Ranges);
        let token = CancelToken::new();
        let input = Input::open(
            token.clone().into(),
            &server.url("/a.wav"),
            &whitelist("http")?,
        )?;
        drop(input);
        token.cancel();
        Ok(())
    }

    #[test]
    fn demuxes_an_aes_encrypted_hls_playlist() -> Result<()> {
        let segment = encrypt_cbc(KEY, IV, &wav());
        let iv_hex = IV.iter().fold(String::new(), |mut hex, b| {
            let _ = write!(hex, "{b:02x}");
            hex
        });
        let playlist = format!(
            "#EXTM3U\n#EXT-X-VERSION:3\n#EXT-X-TARGETDURATION:1\n#EXT-X-MEDIA-SEQUENCE:0\n\
             #EXT-X-KEY:METHOD=AES-128,URI=\"key.bin\",IV=0x{iv_hex}\n#EXTINF:1.0,\nseg0.wav\n#EXT-X-ENDLIST\n"
        );
        let server = serve(
            vec![
                ("/live.m3u8", playlist.into_bytes()),
                ("/key.bin", KEY.to_vec()),
                ("/seg0.wav", segment),
            ],
            Mode::Ranges,
        );
        let input = Input::open(cancel(), &server.url("/live.m3u8"), &whitelist("http")?)?;
        assert_eq!(input.nb_streams(), 1);
        assert!(
            server.requests.load(Ordering::SeqCst) >= 3,
            "playlist, key and segment"
        );
        Ok(())
    }
}
