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
        op_add(),
        op_add_async(),
        op_current_user_call_site(),
        op_leak_tracing_submit(),
        op_leak_tracing_get(),
        op_serialize(),
        op_deserialize(),
        op_structured_clone(),
        op_drain_pending_rejections(),
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
        op_print(),
        op_cancel_handle(),
        op_encoding_new_decoder(),
        op_encoding_decode(),
        op_encoding_decode_single(),
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
    // CompressionStream is not installed for scenes, and decompression output is unbounded
    "op_compression_new",
    "op_compression_write",
    "op_compression_finish",
    // unwraps reads of its parameter and option arrays
    "op_compile_function",
    // the scene's handler runs while deno_core holds borrows the handler can take again
    "op_set_handled_promise_rejection_handler",
    // nothing the runtime loads calls these, and some grow native state per call
    "op_add_main_module_handler",
    "op_set_promise_hooks",
    "op_set_format_exception_callback",
    "op_leak_tracing_enable",
    "op_leak_tracing_get_all",
    "op_import_sync",
    "op_import_sync_with_source",
    "op_event_loop_has_more_work",
    "op_resources",
    "op_memory_usage",
    "op_op_names",
    "op_is_terminal",
    "op_read_sync",
    "op_write_sync",
    "op_write",
    "op_write_type_error",
    "op_error_async",
    "op_error_async_deferred",
    // the console inspector; scenes log through init.js
    "op_console_css_to_ansi",
    "op_console_format_value",
    "op_console_get_string_width",
    "op_console_inspect",
    "op_console_inspect_args",
    "op_console_parse_css",
    "op_console_parse_css_color",
    "op_console_quote_string",
    "op_console_strip_vt",
    "op_preview_entries",
    "op_get_proxy_details",
    "op_get_promise_details",
    "op_get_non_index_property_names",
    "op_get_constructor_name",
    // MessageChannel, wasm streaming, object URLs and geometry are not installed for scenes
    "op_message_port_create_entangled",
    "op_message_port_post_message_raw",
    "op_message_port_recv_message",
    "op_message_port_recv_message_sync",
    "op_set_wasm_streaming_callback",
    "op_abort_wasm_streaming",
    "op_wasm_streaming_feed",
    "op_wasm_streaming_set_url",
    "op_blob_revoke_object_url",
    "op_geometry_get_enable_css_parser_features",
    "op_geometry_matrix_set_matrix_value",
    "op_geometry_matrix_to_string",
];

/// Upstream ops scenes keep as they are, each reviewed against scene input at the locked
/// deno_core, deno_web and deno_webidl. An upstream op on none of these lists fails
/// `every_upstream_op_is_reviewed`, so an upgrade cannot add one unseen.
#[cfg(test)]
pub const REVIEWED_UPSTREAM_OPS: &[&str] = &[
    // the core's own JS: init, the event loop, rejections and exceptions
    "op_get_extras_binding_object",
    "op_get_ext_import_meta_proto",
    "op_set_captured_bootstrap",
    "op_lazy_load_esm",
    "op_load_ext_script",
    "op_dispatch_exception",
    "op_destructure_error",
    "op_run_microtasks",
    "op_immediate_check",
    // these and op_timer_track grow a set per call; only a client scene reaches them raw
    "op_ref_op",
    "op_unref_op",
    // timers
    "op_timer_schedule",
    "op_timer_track",
    "op_timer_untrack",
    "op_defer",
    // resources: streams, fetch bodies, websockets; a wrong rid is a table miss
    "op_close",
    "op_try_close",
    "op_read",
    "op_write_all",
    "op_shutdown",
    "op_cancel_read",
    "op_pipe",
    // pure conversions and type checks
    "op_void_sync",
    "op_void_async",
    "op_void_async_deferred",
    "op_encode",
    "op_decode",
    "op_encode_binary_string",
    "op_str_byte_length",
    "op_is_any_array_buffer",
    "op_is_arguments_object",
    "op_is_array_buffer",
    "op_is_array_buffer_view",
    "op_is_async_function",
    "op_is_big_int_object",
    "op_is_boolean_object",
    "op_is_boxed_primitive",
    "op_is_data_view",
    "op_is_date",
    "op_is_generator_function",
    "op_is_generator_object",
    "op_is_map",
    "op_is_map_iterator",
    "op_is_module_namespace_object",
    "op_is_native_error",
    "op_is_number_object",
    "op_is_promise",
    "op_is_proxy",
    "op_is_reg_exp",
    "op_is_set",
    "op_is_set_iterator",
    "op_is_shared_array_buffer",
    "op_is_string_object",
    "op_is_symbol_object",
    "op_is_typed_array",
    "op_is_weak_map",
    "op_is_weak_set",
    "op_base64_atob",
    "op_base64_btoa",
    "op_base64_decode",
    "op_base64_decode_into",
    "op_base64_encode_from_buffer",
    "op_base64url_decode",
    "op_base64url_decode_into",
    "op_base64url_encode_from_buffer",
    "op_encoding_decode_utf8",
    "op_encoding_decode_utf8_ascii_only",
    "op_encoding_encode_into",
    "op_encoding_encode_into_fallback",
    "op_encoding_normalize_label",
    "op_url_parse_search_params",
    // looks up the runtime's own object URLs, of which there are none
    "op_blob_from_object_url",
];

#[op2(fast)]
fn op_panic(#[string] _message: String) -> Result<(), OpError> {
    bail!("op_panic is not available to scenes")
}

// the originals overflow-panic in debug builds
#[op2(fast)]
fn op_add(a: i32, b: i32) -> i32 {
    a.wrapping_add(b)
}

#[allow(clippy::unused_async)]
#[op2]
async fn op_add_async(a: i32, b: i32) -> i32 {
    a.wrapping_add(b)
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
        return Err(super::errors::type_error(
            "structured clone of host objects or transfers is not available to scenes",
        ));
    }
    Ok(())
}

struct SceneClone;

/// Throws a TypeError, which the structured clone script turns into a DataCloneError; the
/// defaults throw plain Errors.
fn refuse_clone(scope: &mut v8::PinScope, what: &str) {
    if let Some(message) = v8::String::new(scope, &format!("#<{what}> could not be cloned.")) {
        let error = v8::Exception::type_error(scope, message);
        scope.throw_exception(error);
    }
}

impl v8::ValueSerializerImpl for SceneClone {
    fn throw_data_clone_error<'s>(
        &self,
        scope: &mut v8::PinScope<'s, '_>,
        message: v8::Local<'s, v8::String>,
    ) {
        let error = v8::Exception::type_error(scope, message);
        scope.throw_exception(error);
    }

    fn write_host_object<'s>(
        &self,
        scope: &mut v8::PinScope<'s, '_>,
        _object: v8::Local<'s, v8::Object>,
        _value_serializer: &dyn v8::ValueSerializerHelper,
    ) -> Option<bool> {
        refuse_clone(scope, "Object");
        None
    }

    fn get_shared_array_buffer_id<'s>(
        &self,
        scope: &mut v8::PinScope<'s, '_>,
        _shared_array_buffer: v8::Local<'s, v8::SharedArrayBuffer>,
    ) -> Option<u32> {
        refuse_clone(scope, "SharedArrayBuffer");
        None
    }

    fn get_wasm_module_transfer_id(
        &self,
        scope: &mut v8::PinScope<'_, '_>,
        _module: v8::Local<v8::WasmModuleObject>,
    ) -> Option<u32> {
        refuse_clone(scope, "WebAssembly.Module");
        None
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
        _ => Err(super::errors::type_error("Failed to serialize response").into()),
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

// `core.structuredClone` (performance.mark/measure `detail`, stream tees). The original
// honours the host-object brand on any object and panics on a type it has no deserializer for.
#[op2(reentrant)]
fn op_structured_clone<'s>(
    scope: &mut v8::PinScope<'s, '_>,
    value: v8::Local<'s, v8::Value>,
    _deserializers: Option<v8::Local<'s, v8::Object>>,
) -> Result<v8::Local<'s, v8::Value>, OpError> {
    let serializer = v8::ValueSerializer::new(scope, Box::new(SceneClone));
    serializer.write_header();
    let written = {
        v8::tc_scope!(let scope, scope);
        let written = serializer.write_value(scope.get_current_context(), value);
        if scope.has_caught() || scope.has_terminated() {
            scope.rethrow();
            return Ok(v8::undefined(scope).into());
        }
        written
    };
    if written != Some(true) {
        return Err(super::errors::type_error("Failed to serialize response").into());
    }
    let bytes = serializer.release();
    let deserializer = v8::ValueDeserializer::new(scope, Box::new(SceneClone), &bytes);
    let context = scope.get_current_context();
    if !deserializer.read_header(context).unwrap_or_default() {
        return Err(super::errors::type_error("could not deserialize value").into());
    }
    Ok(deserializer
        .read_value(context)
        .ok_or_else(|| super::errors::type_error("could not deserialize value"))?)
}

/// The original fills a new holey array while it holds the rejection queue's borrow, so an
/// index setter on `Array.prototype` runs scene code there, and a rejection inside it takes the
/// borrow again and aborts the process. Returning nothing leaves every rejection queued for the
/// event loop's own dispatch, which takes the queue first and passes it as arguments.
#[op2]
fn op_drain_pending_rejections<'s>(scope: &mut v8::PinScope<'s, '_>) -> v8::Local<'s, v8::Value> {
    v8::undefined(scope).into()
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
    let null: v8::Local<v8::Value> = v8::null(scope).into();
    v8::tc_scope!(let scope, scope);
    let (result, is_compile_error) = match v8::Script::compile(scope, source, Some(&origin)) {
        Some(script) => (script.run(scope), false),
        None => (None, true),
    };
    // arrays built from their elements, so no setter scene code put on Array.prototype runs
    if let Some(result) = result {
        return Ok(v8::Array::new_with_elements(scope, &[result, null]).into());
    }
    if scope.has_terminated() {
        scope.rethrow();
        return Ok(null);
    }
    let thrown = scope.exception().unwrap_or(null);
    let is_native_error = v8::Boolean::new(scope, thrown.is_native_error()).into();
    let is_compile_error = v8::Boolean::new(scope, is_compile_error).into();
    let error = v8::Array::new_with_elements(scope, &[thrown, is_native_error, is_compile_error]);
    Ok(v8::Array::new_with_elements(scope, &[null, error.into()]).into())
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

/// Parts are freed only after V8 collects their Blobs, and V8 does not see their bytes: report
/// them, and when the store is full collect now, so the parts the scene dropped are freed by
/// its next turn.
fn account_blob_memory<T>(
    scope: &mut v8::PinScope,
    store: &CappedBlobStore,
    inserted: Result<T, AnyError>,
) -> Result<T, AnyError> {
    scope.adjust_amount_of_external_allocated_memory(store.take_external_change());
    if inserted.is_err() {
        scope.low_memory_notification();
    }
    inserted
}

fn capped_store(state: &OpState) -> Result<Arc<CappedBlobStore>, AnyError> {
    state
        .try_borrow::<Arc<CappedBlobStore>>()
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("Blob storage is not available"))
}

#[op2]
#[string]
fn op_blob_create_part(
    scope: &mut v8::PinScope,
    state: &mut OpState,
    #[buffer] data: JsBuffer,
) -> Result<String, OpError> {
    let store = capped_store(state)?;
    let id = store.try_insert_with(data.len(), || {
        Arc::new(InMemoryBlobPart::from(data.to_vec()))
    });
    let id = account_blob_memory(scope, &store, id)?;
    Ok(id.to_string())
}

#[derive(Serialize)]
struct ClonedPart {
    uuid: String,
    size: usize,
}

#[op2]
#[serde]
fn op_blob_clone_part(
    scope: &mut v8::PinScope,
    state: &mut OpState,
    #[string] id: &str,
) -> Result<ClonedPart, OpError> {
    let store = capped_store(state)?;
    let (uuid, size) = store
        .insert_clone(&blob_part_id(id)?)
        .ok_or_else(|| anyhow::anyhow!("Blob part not found"))?;
    account_blob_memory(scope, &store, Ok(()))?;
    Ok(ClonedPart {
        uuid: uuid.to_string(),
        size,
    })
}

#[op2]
#[string]
fn op_blob_slice_part(
    scope: &mut v8::PinScope,
    state: &mut OpState,
    #[string] id: &str,
    #[scoped] options: Plain<SliceOptions>,
) -> Result<String, OpError> {
    let SliceOptions { start, len } = options.0;
    let store = capped_store(state)?;
    let id = store.insert_slice(&blob_part_id(id)?, start, len);
    Ok(account_blob_memory(scope, &store, id)?.to_string())
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

// not fast, so the freed bytes reach V8's external memory count now
#[op2(nofast)]
fn op_blob_remove_part(
    scope: &mut v8::PinScope,
    state: &mut OpState,
    #[string] id: &str,
) -> Result<(), OpError> {
    let id = blob_part_id(id)?;
    let store = capped_store(state)?;
    store.remove_part(&id);
    scope.adjust_amount_of_external_allocated_memory(store.take_external_change());
    Ok(())
}

/// Writes to the sidecar's stdout and unwraps its flush; scene output goes to the log instead.
#[op2(fast)]
fn op_print(#[string] msg: &str, is_err: bool) {
    const MAX_PRINT_CHARS: usize = 4096;
    let end = msg
        .char_indices()
        .nth(MAX_PRINT_CHARS)
        .map_or(msg.len(), |(at, _)| at);
    if is_err {
        bevy::log::warn!("{}", &msg[..end]);
    } else {
        bevy::log::info!("{}", &msg[..end]);
    }
}

// A resource per call, which only fetch closes; bounded by the table size.
const MAX_RESOURCES: usize = 4096;

#[op2(fast)]
#[smi]
fn op_cancel_handle(state: &mut OpState) -> Result<u32, OpError> {
    if state.resource_table.names().count() >= MAX_RESOURCES {
        bail!("too many open resources");
    }
    Ok(state.resource_table.add(deno_core::CancelHandle::new()))
}

// The originals allocate twice the input in UTF-16 outside every cap.
const MAX_DECODE_BYTES: usize = 32 * 1024 * 1024;

struct SceneTextDecoder {
    decoder: RefCell<encoding_rs::Decoder>,
    fatal: bool,
}

// SAFETY: holds no V8 handles, so there is nothing to trace
unsafe impl deno_core::GarbageCollected for SceneTextDecoder {
    fn trace(&self, _visitor: &mut v8::cppgc::Visitor) {}

    fn get_name(&self) -> &'static std::ffi::CStr {
        c"TextDecoderResource"
    }
}

fn text_decoder(label: &str, ignore_bom: bool) -> Result<encoding_rs::Decoder, AnyError> {
    let encoding = encoding_rs::Encoding::for_label(label.as_bytes()).ok_or_else(|| {
        deno_error::JsErrorBox::range_error(format!(
            "The encoding label provided ('{label}') is invalid."
        ))
    })?;
    Ok(if ignore_bom {
        encoding.new_decoder_without_bom_handling()
    } else {
        encoding.new_decoder_with_bom_removal()
    })
}

fn decode<'s>(
    scope: &mut v8::PinScope<'s, '_>,
    decoder: &mut encoding_rs::Decoder,
    data: &[u8],
    fatal: bool,
    last: bool,
) -> Result<v8::Local<'s, v8::String>, AnyError> {
    if data.len() > MAX_DECODE_BYTES {
        return Err(deno_error::JsErrorBox::range_error("Value too large to decode").into());
    }
    let max = decoder
        .max_utf16_buffer_length(data.len())
        .ok_or_else(|| deno_error::JsErrorBox::range_error("Value too large to decode"))?;
    let mut output = vec![0u16; max];
    let written = if fatal {
        match decoder.decode_to_utf16_without_replacement(data, &mut output, last) {
            (encoding_rs::DecoderResult::InputEmpty, _, written) => written,
            (encoding_rs::DecoderResult::Malformed(..), ..) => {
                return Err(
                    deno_error::JsErrorBox::type_error("The encoded data is not valid").into(),
                )
            }
            (encoding_rs::DecoderResult::OutputFull, ..) => {
                return Err(deno_error::JsErrorBox::range_error("Provided buffer too small").into())
            }
        }
    } else {
        match decoder.decode_to_utf16(data, &mut output, last) {
            (encoding_rs::CoderResult::InputEmpty, _, written, _) => written,
            (encoding_rs::CoderResult::OutputFull, ..) => {
                return Err(deno_error::JsErrorBox::range_error("Provided buffer too small").into())
            }
        }
    };
    v8::String::new_from_two_byte(scope, &output[..written], v8::NewStringType::Normal)
        .ok_or_else(|| deno_error::JsErrorBox::type_error("buffer exceeds maximum length").into())
}

#[op2]
#[cppgc]
fn op_encoding_new_decoder(
    #[string] label: &str,
    fatal: bool,
    ignore_bom: bool,
) -> Result<SceneTextDecoder, OpError> {
    Ok(SceneTextDecoder {
        decoder: RefCell::new(text_decoder(label, ignore_bom)?),
        fatal,
    })
}

#[op2]
fn op_encoding_decode<'s>(
    scope: &mut v8::PinScope<'s, '_>,
    #[anybuffer] data: &[u8],
    #[cppgc] resource: &SceneTextDecoder,
    stream: bool,
) -> Result<v8::Local<'s, v8::String>, OpError> {
    let mut decoder = resource
        .decoder
        .try_borrow_mut()
        .map_err(|_| anyhow::anyhow!("TextDecoder is in use"))?;
    Ok(decode(scope, &mut decoder, data, resource.fatal, !stream)?)
}

#[op2]
fn op_encoding_decode_single<'s>(
    scope: &mut v8::PinScope<'s, '_>,
    #[anybuffer] data: &[u8],
    #[string] label: String,
    fatal: bool,
    ignore_bom: bool,
) -> Result<v8::Local<'s, v8::String>, OpError> {
    let mut decoder = text_decoder(&label, ignore_bom)?;
    Ok(decode(scope, &mut decoder, data, fatal, true)?)
}

// Scenes share a process, so the clocks they read are coarsened against timing side channels:
// these ops, and `Date`, `Temporal.Now`, `Intl.DateTimeFormat` and `File` in init.js, which
// repeats the 2 ms. Only the authoritative server holds this: a client
// scene keeps `Deno` and its primordials, which read the native clock.
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
