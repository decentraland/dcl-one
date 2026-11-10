use crate::js::errors::OpError;
use std::{cell::RefCell, rc::Rc, sync::Arc};

use deno_core::{
    anyhow,
    convert::Uint8Array,
    error::AnyError,
    op2,
    url::{form_urlencoded, quirks, Url},
    v8, JsBuffer, OpDecl, OpState,
};
use deno_web::{BlobStoreTrait, InMemoryBlobPart, StartTime};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{blob_store::CappedBlobStore, errors::bail, plain::Plain};
use v8::{ValueDeserializerHelper, ValueSerializerHelper};

// Replacements for registered ops that scene code can reach through the ops table. Each one
// keeps its original's name, async-ness and JS argument count (`with_implementation_from`
// swaps only the function pointers) and returns an error for input the original panics or
// exits on: a panic across the V8 boundary aborts the sidecar and every scene in it.
pub fn override_ops() -> Vec<OpDecl> {
    vec![
        op_panic(),
        op_current_user_call_site(),
        op_leak_tracing_submit(),
        op_leak_tracing_get(),
        op_serialize(),
        op_deserialize(),
        op_eval_context(),
        op_message_port_post_message(),
        op_url_parse(),
        op_url_parse_with_base(),
        op_url_reparse(),
        op_url_get_serialization(),
        op_url_stringify_search_params(),
        op_urlpattern_parse(),
        op_urlpattern_process_match_input(),
        op_blob_create_part(),
        op_blob_clone_part(),
        op_blob_slice_part(),
        op_blob_read_part(),
        op_blob_remove_part(),
        op_blob_create_object_url(),
        op_now(),
        op_time_origin(),
        op_timer_now(),
    ]
}

/// Upstream ops scenes never get. The lock manager keeps one process-wide lock table, so
/// scenes would see and steal each other's locks; BroadcastChannel is our own shim.
pub const DISABLED_OPS: &[&str] = &[
    "op_lock_manager_request",
    "op_lock_manager_await_lock",
    "op_lock_manager_await_steal",
    "op_lock_manager_is_stolen",
    "op_lock_manager_cancel",
    "op_lock_manager_release",
    "op_lock_manager_query",
    "op_broadcast_subscribe",
    "op_broadcast_unsubscribe",
    "op_broadcast_serialize",
    "op_broadcast_deserialize",
    "op_broadcast_free",
    "op_broadcast_send",
    "op_broadcast_recv",
];

#[op2(fast)]
fn op_panic(#[string] _message: String) -> Result<(), OpError> {
    bail!("op_panic is not available to scenes")
}

/// Writes the first user frame's line and column into `ret_buf` and returns its file name.
#[op2]
#[string]
fn op_current_user_call_site(
    scope: &mut v8::PinScope,
    #[buffer] ret_buf: &mut [u8],
) -> Result<String, OpError> {
    if ret_buf.len() < 8 {
        bail!("call site buffer must hold at least 8 bytes");
    }
    let Some(stack_trace) = v8::StackTrace::current_stack_trace(scope, 10) else {
        bail!("no stack trace available");
    };
    let frame_count = stack_trace.get_frame_count();
    for i in 0..frame_count {
        let Some(frame) = stack_trace.get_frame(scope, i) else {
            continue;
        };
        if !frame.is_user_javascript() {
            continue;
        }
        let file_name = frame
            .get_script_name(scope)
            .map(|name| name.to_rust_string_lossy(scope))
            .unwrap_or_default();
        if (file_name.starts_with("ext:") || file_name.starts_with("node:")) && i != frame_count - 1
        {
            continue;
        }
        let line_number = frame.get_line_number() as u32;
        let column_number = frame.get_column() as u32;
        ret_buf[0..4].copy_from_slice(&line_number.to_le_bytes());
        ret_buf[4..8].copy_from_slice(&column_number.to_le_bytes());
        return Ok(file_name);
    }
    bail!("no user call site on the stack")
}

fn unavailable(op: &str) -> OpError {
    anyhow::anyhow!("{op} is not available to scenes").into()
}

// Leak tracing is off; the originals hit `unreachable!` on an unknown kind.
#[op2(fast)]
fn op_leak_tracing_submit(
    #[smi] _kind: u8,
    #[smi] _id: i32,
    #[string] _trace: &str,
) -> Result<(), OpError> {
    Err(unavailable("leak tracing"))
}

#[op2(fast)]
fn op_leak_tracing_get(#[smi] _kind: u8, #[smi] _id: i32) -> Result<(), OpError> {
    Err(unavailable("leak tracing"))
}

// structuredClone (performance.mark/measure) reaches these. The originals unwrap reads of
// the host-object and transfer arrays, which only MessagePort uses; scenes have neither.
fn refuse_clone_extras(
    host_objects: Option<v8::Local<v8::Value>>,
    transferred_array_buffers: Option<v8::Local<v8::Value>>,
) -> Result<(), AnyError> {
    if host_objects.is_some() || transferred_array_buffers.is_some() {
        bail!("structured clone of host objects or transfers is not available to scenes");
    }
    Ok(())
}

struct SceneClone;

impl v8::ValueSerializerImpl for SceneClone {
    fn throw_data_clone_error<'s>(
        &self,
        scope: &mut v8::PinScope<'s, '_>,
        message: v8::Local<'s, v8::String>,
    ) {
        let error = v8::Exception::type_error(scope, message);
        scope.throw_exception(error);
    }
}

impl v8::ValueDeserializerImpl for SceneClone {}

#[op2(reentrant)]
fn op_serialize<'s>(
    scope: &mut v8::PinScope<'s, '_>,
    value: v8::Local<'s, v8::Value>,
    host_objects: Option<v8::Local<'s, v8::Value>>,
    transferred_array_buffers: Option<v8::Local<'s, v8::Value>>,
    _for_storage: bool,
    _error_callback: Option<v8::Local<'s, v8::Value>>,
) -> Result<Uint8Array, OpError> {
    refuse_clone_extras(host_objects, transferred_array_buffers)?;
    let serializer = v8::ValueSerializer::new(scope, Box::new(SceneClone));
    serializer.write_header();
    v8::tc_scope!(let scope, scope);
    let written = serializer.write_value(scope.get_current_context(), value);
    if scope.has_caught() || scope.has_terminated() {
        // the getter's or clone error's exception reaches the caller
        scope.rethrow();
        return Ok(Vec::new().into());
    }
    match written {
        Some(true) => Ok(serializer.release().into()),
        _ => Err(anyhow::anyhow!("Failed to serialize response").into()),
    }
}

#[op2(reentrant)]
fn op_deserialize<'s>(
    scope: &mut v8::PinScope<'s, '_>,
    #[buffer] zero_copy: JsBuffer,
    host_objects: Option<v8::Local<'s, v8::Value>>,
    transferred_array_buffers: Option<v8::Local<'s, v8::Value>>,
    _deserializers: Option<v8::Local<'s, v8::Value>>,
    _for_storage: bool,
) -> Result<v8::Local<'s, v8::Value>, OpError> {
    refuse_clone_extras(host_objects, transferred_array_buffers)?;
    let deserializer = v8::ValueDeserializer::new(scope, Box::new(SceneClone), &zero_copy);
    let context = scope.get_current_context();
    if !deserializer.read_header(context).unwrap_or_default() {
        return Err(anyhow::anyhow!("could not deserialize value").into());
    }
    Ok(deserializer
        .read_value(context)
        .ok_or_else(|| anyhow::anyhow!("could not deserialize value"))?)
}

/// Compiles and runs `source` as a classic script, returning `[result, error]` where `error`
/// is `[thrown, isNativeError, isCompileError]`. The original unwraps reads of the
/// host-defined options array, which only node compatibility passes.
#[op2(reentrant)]
fn op_eval_context<'s>(
    scope: &mut v8::PinScope<'s, '_>,
    source: v8::Local<'s, v8::Value>,
    #[string] specifier: String,
    host_defined_options: Option<v8::Local<'s, v8::Value>>,
) -> Result<v8::Local<'s, v8::Value>, OpError> {
    if host_defined_options.is_some() {
        return Err(anyhow::anyhow!("host-defined options are not available to scenes").into());
    }
    let source = v8::Local::<v8::String>::try_from(source)
        .map_err(|_| super::errors::type_error("Invalid source"))?;
    let specifier = deno_core::resolve_url(&specifier).map_err(|e| anyhow::anyhow!("{e}"))?;
    let name = v8::String::new(scope, specifier.as_str())
        .ok_or_else(|| anyhow::anyhow!("specifier is too long"))?;
    let origin = v8::ScriptOrigin::new(
        scope,
        name.into(),
        0,
        0,
        false,
        0,
        None,
        false,
        false,
        false,
        None,
    );
    let out = v8::Array::new(scope, 2);
    let null: v8::Local<v8::Value> = v8::null(scope).into();
    v8::tc_scope!(let scope, scope);
    let (result, is_compile_error) = match v8::Script::compile(scope, source, Some(&origin)) {
        Some(script) => (script.run(scope), false),
        None => (None, true),
    };
    if let Some(result) = result {
        out.set_index(scope, 0, result);
        out.set_index(scope, 1, null);
        return Ok(out.into());
    }
    if scope.has_terminated() {
        scope.rethrow();
        return Ok(null);
    }
    let thrown = scope.exception().unwrap_or(null);
    let error = v8::Array::new(scope, 3);
    error.set_index(scope, 0, thrown);
    let is_native_error = v8::Boolean::new(scope, thrown.is_native_error());
    error.set_index(scope, 1, is_native_error.into());
    let is_compile_error = v8::Boolean::new(scope, is_compile_error);
    error.set_index(scope, 2, is_compile_error.into());
    out.set_index(scope, 0, null);
    out.set_index(scope, 1, error.into());
    Ok(out.into())
}

// MessageChannel is not installed for scenes, and the original deserializes its message
// with serde_v8 before checking the port.
#[op2(fast)]
fn op_message_port_post_message(
    #[smi] _rid: u32,
    _data: v8::Local<v8::Value>,
) -> Result<(), OpError> {
    Err(unavailable("MessagePort"))
}

// URLPattern and URL.createObjectURL are not installed for scenes, and the originals
// deserialize their arguments with serde_v8, which panics when a getter throws.
#[op2(fast)]
fn op_urlpattern_parse(
    _input: v8::Local<v8::Value>,
    _base_url: v8::Local<v8::Value>,
    _options: v8::Local<v8::Value>,
) -> Result<(), OpError> {
    Err(unavailable("URLPattern"))
}

#[op2(fast)]
fn op_urlpattern_process_match_input(
    _input: v8::Local<v8::Value>,
    _base_url: v8::Local<v8::Value>,
    _buf: v8::Local<v8::Value>,
) -> Result<(), OpError> {
    Err(unavailable("URLPattern"))
}

#[op2(fast)]
fn op_blob_create_object_url(
    _media_type: v8::Local<v8::Value>,
    _part_ids: v8::Local<v8::Value>,
) -> Result<(), OpError> {
    Err(unavailable("URL.createObjectURL"))
}

// The URL ops write eight components into the caller's buffer and leave a serialization
// for `op_url_get_serialization`; the originals index the buffer and take the
// serialization unchecked.
struct UrlSerialization(String);

const URL_PARSE_OK: u32 = 0;
const URL_PARSE_OK_SERIALIZATION: u32 = 1;
const URL_PARSE_ERR: u32 = 2;
const URL_COMPONENTS: usize = 8;
const URL_NO_PORT: u32 = 65536;

fn url_parsed(state: &mut OpState, url: Url, href: &str, buf: &mut [u32], no_port: u32) -> u32 {
    let inner = quirks::internal_components(&url);
    buf[..URL_COMPONENTS].copy_from_slice(&[
        inner.scheme_end,
        inner.username_end,
        inner.host_start,
        inner.host_end,
        inner.port.map_or(no_port, u32::from),
        inner.path_start,
        inner.query_start.unwrap_or(0),
        inner.fragment_start.unwrap_or(0),
    ]);
    let serialization: String = url.into();
    if serialization == href {
        URL_PARSE_OK
    } else {
        state.put(UrlSerialization(serialization));
        URL_PARSE_OK_SERIALIZATION
    }
}

fn url_parse(state: &mut OpState, href: &str, base: Option<&Url>, buf: &mut [u32]) -> u32 {
    if buf.len() < URL_COMPONENTS {
        return URL_PARSE_ERR;
    }
    match Url::options().base_url(base).parse(href) {
        Ok(url) => url_parsed(state, url, href, buf, 0),
        Err(_) => URL_PARSE_ERR,
    }
}

#[op2(fast)]
#[smi]
fn op_url_parse(state: &mut OpState, #[string] href: &str, #[buffer] buf: &mut [u32]) -> u32 {
    url_parse(state, href, None, buf)
}

#[op2(fast)]
#[smi]
fn op_url_parse_with_base(
    state: &mut OpState,
    #[string] href: &str,
    #[string] base_href: &str,
    #[buffer] buf: &mut [u32],
) -> u32 {
    match Url::parse(base_href) {
        Ok(base) => url_parse(state, href, Some(&base), buf),
        Err(_) => URL_PARSE_ERR,
    }
}

#[op2(fast)]
#[smi]
fn op_url_reparse(
    state: &mut OpState,
    #[string] href: String,
    #[smi] setter: u8,
    #[string] setter_value: String,
    #[buffer] buf: &mut [u32],
) -> u32 {
    if buf.len() < URL_COMPONENTS {
        return URL_PARSE_ERR;
    }
    let Ok(mut url) = Url::options().parse(&href) else {
        return URL_PARSE_ERR;
    };
    let value = setter_value.as_str();
    let result = match setter {
        0 => {
            quirks::set_hash(&mut url, value);
            Ok(())
        }
        1 => quirks::set_host(&mut url, value),
        2 => quirks::set_hostname(&mut url, value),
        3 => quirks::set_password(&mut url, value),
        4 => {
            quirks::set_pathname(&mut url, value);
            Ok(())
        }
        5 => quirks::set_port(&mut url, value),
        6 => quirks::set_protocol(&mut url, value),
        7 => {
            quirks::set_search(&mut url, value);
            Ok(())
        }
        8 => quirks::set_username(&mut url, value),
        _ => return URL_PARSE_ERR,
    };
    match result {
        Ok(()) => url_parsed(state, url, &href, buf, URL_NO_PORT),
        Err(()) => URL_PARSE_ERR,
    }
}

#[op2]
#[string]
fn op_url_get_serialization(state: &mut OpState) -> Result<String, OpError> {
    match state.try_take::<UrlSerialization>() {
        Some(serialization) => Ok(serialization.0),
        None => bail!("no URL serialization is pending"),
    }
}

#[op2]
#[string]
fn op_url_stringify_search_params(#[scoped] args: Plain<Vec<(String, String)>>) -> String {
    form_urlencoded::Serializer::new(String::new())
        .extend_pairs(args.0)
        .finish()
}

// Response.blob() reaches these; the originals read ids and options with conversions that
// unwrap when a getter throws.
fn blob_part_id(id: &str) -> Result<Uuid, AnyError> {
    Uuid::parse_str(id).map_err(|_| anyhow::anyhow!("Blob part not found"))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SliceOptions {
    start: usize,
    len: usize,
}

fn capped_store(state: &OpState) -> Result<Arc<CappedBlobStore>, AnyError> {
    state
        .try_borrow::<Arc<CappedBlobStore>>()
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("Blob storage is not available"))
}

#[op2]
#[string]
fn op_blob_create_part(state: &mut OpState, #[buffer] data: JsBuffer) -> Result<String, OpError> {
    let store = capped_store(state)?;
    let id = store.try_insert_with(data.len(), || {
        Arc::new(InMemoryBlobPart::from(data.to_vec()))
    })?;
    Ok(id.to_string())
}

#[derive(Serialize)]
struct ClonedPart {
    uuid: String,
    size: usize,
}

#[op2]
#[serde]
fn op_blob_clone_part(state: &mut OpState, #[string] id: &str) -> Result<ClonedPart, OpError> {
    let store = capped_store(state)?;
    let part = store
        .get_part(&blob_part_id(id)?)
        .ok_or_else(|| anyhow::anyhow!("Blob part not found"))?;
    let size = part.size();
    let uuid = store.try_insert_shared(part)?.to_string();
    Ok(ClonedPart { uuid, size })
}

#[op2]
#[string]
fn op_blob_slice_part(
    state: &mut OpState,
    #[string] id: &str,
    #[scoped] options: Plain<SliceOptions>,
) -> Result<String, OpError> {
    let SliceOptions { start, len } = options.0;
    let store = capped_store(state)?;
    let part = store
        .get_part(&blob_part_id(id)?)
        .ok_or_else(|| anyhow::anyhow!("Blob part not found"))?;
    if start.checked_add(len).is_none_or(|end| end > part.size()) {
        bail!("start + len can not be larger than blob part size");
    }
    // every part in a scene runtime is in memory, so its read is ready at once
    let bytes = deno_core::futures::FutureExt::now_or_never(part.read())
        .ok_or_else(|| anyhow::anyhow!("Blob part is not in memory"))?;
    let id = store.try_insert_with(len, || {
        Arc::new(InMemoryBlobPart::from(bytes[start..start + len].to_vec()))
    })?;
    Ok(id.to_string())
}

#[op2]
async fn op_blob_read_part(
    state: Rc<RefCell<OpState>>,
    #[string] id: String,
) -> Result<Uint8Array, OpError> {
    let id = blob_part_id(&id)?;
    let part = capped_store(&state.borrow())?
        .get_part(&id)
        .ok_or_else(|| anyhow::anyhow!("Blob part not found"))?;
    let bytes = part.read().await;
    Ok(Uint8Array::from(bytes.to_vec()))
}

#[op2(fast)]
fn op_blob_remove_part(state: &mut OpState, #[string] id: &str) -> Result<(), OpError> {
    let id = blob_part_id(id)?;
    capped_store(state)?.remove_part(&id);
    Ok(())
}

// Scenes share a process, so every clock they can read is coarsened against timing side
// channels: these ops, and `Date`, `Temporal.Now` and `Intl.DateTimeFormat` in init.js.
const CLOCK_GRANULARITY: std::time::Duration = std::time::Duration::from_millis(2);
const CLOCK_GRANULARITY_MS: f64 = 2.0;

fn coarsen(time: std::time::Duration) -> std::time::Duration {
    let step = CLOCK_GRANULARITY.as_nanos();
    let nanos = time.as_nanos() / step * step;
    std::time::Duration::new(
        (nanos / 1_000_000_000) as u64,
        (nanos % 1_000_000_000) as u32,
    )
}

fn expose_time(time: std::time::Duration, out: &mut [u8]) {
    if out.len() >= 8 {
        out[0..4].copy_from_slice(&(time.as_secs() as u32).to_ne_bytes());
        out[4..8].copy_from_slice(&time.subsec_nanos().to_ne_bytes());
    }
}

#[op2(fast)]
fn op_now(state: &mut OpState, #[buffer] buf: &mut [u8]) {
    let elapsed = state
        .try_borrow::<StartTime>()
        .map(|start| start.elapsed())
        .unwrap_or_default();
    expose_time(coarsen(elapsed), buf);
}

#[op2(fast)]
fn op_time_origin(state: &mut OpState, #[buffer] buf: &mut [u8]) {
    let monotonic = state
        .try_borrow::<StartTime>()
        .map(|start| start.elapsed())
        .unwrap_or_default();
    let epoch = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .saturating_sub(monotonic);
    expose_time(coarsen(epoch), buf);
}

/// deno_core's timer clock, in milliseconds since the realm was created. Rounded up, so a
/// timer's start is never earlier than the clock the event loop fires it by.
#[op2(fast)]
fn op_timer_now(state: &mut OpState) -> f64 {
    let elapsed = state
        .try_borrow::<TimerOrigin>()
        .map(|origin| origin.0.elapsed().as_secs_f64() * 1000.0)
        .unwrap_or_default();
    (elapsed / CLOCK_GRANULARITY_MS).ceil() * CLOCK_GRANULARITY_MS
}

/// When the realm's timer clock started; see [`mark_timer_origin`].
pub struct TimerOrigin(pub std::time::Instant);

thread_local! {
    static CONTEXT_CREATED: std::cell::Cell<Option<std::time::Instant>> = const { std::cell::Cell::new(None) };
}

/// A global-template middleware: deno_core starts the realm's timer clock just before it
/// builds the global template, so this instant matches it to within microseconds.
pub fn mark_timer_origin<'s, 'i>(
    _scope: &mut v8::PinScope<'s, 'i, ()>,
    template: v8::Local<'s, v8::ObjectTemplate>,
) -> v8::Local<'s, v8::ObjectTemplate> {
    CONTEXT_CREATED.with(|created| created.set(Some(std::time::Instant::now())));
    template
}

/// The instant [`mark_timer_origin`] recorded while this thread created its last runtime.
pub fn take_timer_origin() -> TimerOrigin {
    TimerOrigin(
        CONTEXT_CREATED
            .with(|created| created.take())
            .unwrap_or_else(std::time::Instant::now),
    )
}
