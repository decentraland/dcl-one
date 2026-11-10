use std::{cell::RefCell, rc::Rc, sync::Arc};

use deno_core::{
    anyhow, error::AnyError, futures::FutureExt, op2, url::form_urlencoded, v8, JsBuffer, OpDecl,
    OpState, ToJsBuffer,
};
use deno_web::{BlobStore, InMemoryBlobPart, TimersPermission};
use serde::Deserialize;
use uuid::Uuid;

use super::plain::Plain;
use v8::{ValueDeserializerHelper, ValueSerializerHelper};

// Replacements for registered ops that scene code can reach through the ops table. Each one
// keeps its original's name, async-ness and JS argument count (`with_implementation_from`
// swaps only the function pointers) and returns an error for input the original panics or
// exits on: a panic across the V8 boundary aborts the sidecar and every scene in it.
pub fn override_ops() -> Vec<OpDecl> {
    let mut ops = vec![
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
        op_blob_slice_part(),
        op_blob_read_part(),
        op_blob_remove_part(),
        op_blob_create_object_url(),
        op_encoding_encode_into(),
        op_now(),
        // deno_net: registered because deno_fetch's JS imports its TLS module, never used.
        op_net_listen_tcp(),
        op_net_listen_udp(),
        op_node_unstable_net_listen_udp(),
        op_set_nodelay(),
        op_set_keepalive(),
        op_tls_key_null(),
        op_tls_key_static(),
        op_tls_key_static_from_file(),
        op_tls_cert_resolver_create(),
        op_tls_cert_resolver_resolve(),
        op_tls_cert_resolver_resolve_error(),
        op_tls_start(),
        op_net_listen_tls(),
        op_net_listen_unix(),
        op_net_listen_unixpacket(),
        op_node_unstable_net_listen_unixpacket(),
        op_net_accept_tcp(),
        op_net_connect_tcp(),
        op_net_recv_udp(),
        op_net_send_udp(),
        op_net_join_multi_v4_udp(),
        op_net_join_multi_v6_udp(),
        op_net_leave_multi_v4_udp(),
        op_net_leave_multi_v6_udp(),
        op_net_set_multi_loopback_udp(),
        op_net_set_multi_ttl_udp(),
        op_dns_resolve(),
        op_tls_cert_resolver_poll(),
        op_net_connect_tls(),
        op_net_accept_tls(),
        op_tls_handshake(),
    ];
    // elsewhere deno_net's own unix stubs are sync and already refuse
    #[cfg(unix)]
    ops.extend([
        op_net_accept_unix(),
        op_net_connect_unix(),
        op_net_recv_unixpacket(),
        op_net_send_unixpacket(),
    ]);
    ops
}

#[op2(fast)]
fn op_panic(#[string] _message: String) -> Result<(), AnyError> {
    anyhow::bail!("op_panic is not available to scenes")
}

/// Writes the first user frame's line and column into `ret_buf` and returns its file name.
#[op2]
#[string]
fn op_current_user_call_site(
    scope: &mut v8::HandleScope,
    #[buffer] ret_buf: &mut [u8],
) -> Result<String, AnyError> {
    if ret_buf.len() < 8 {
        anyhow::bail!("call site buffer must hold at least 8 bytes");
    }
    let Some(stack_trace) = v8::StackTrace::current_stack_trace(scope, 10) else {
        anyhow::bail!("no stack trace available");
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
    anyhow::bail!("no user call site on the stack")
}

fn unavailable(op: &str) -> AnyError {
    anyhow::anyhow!("{op} is not available to scenes")
}

// Leak tracing is off; the originals hit `unreachable!` on an unknown kind.
#[op2(fast)]
fn op_leak_tracing_submit(
    #[smi] _kind: u8,
    #[smi] _id: i32,
    #[string] _trace: &str,
) -> Result<(), AnyError> {
    Err(unavailable("leak tracing"))
}

#[op2(fast)]
fn op_leak_tracing_get(#[smi] _kind: u8, #[smi] _id: i32) -> Result<(), AnyError> {
    Err(unavailable("leak tracing"))
}

// structuredClone (performance.mark/measure) reaches these. The originals unwrap reads of
// the host-object and transfer arrays, which only MessagePort uses; scenes have neither.
fn refuse_clone_extras(
    host_objects: v8::Local<v8::Value>,
    transferred_array_buffers: v8::Local<v8::Value>,
) -> Result<(), AnyError> {
    if !host_objects.is_null_or_undefined() || !transferred_array_buffers.is_null_or_undefined() {
        anyhow::bail!("structured clone of host objects or transfers is not available to scenes");
    }
    Ok(())
}

struct SceneClone;

impl v8::ValueSerializerImpl for SceneClone {
    fn throw_data_clone_error<'s>(
        &self,
        scope: &mut v8::HandleScope<'s>,
        message: v8::Local<'s, v8::String>,
    ) {
        let error = v8::Exception::type_error(scope, message);
        scope.throw_exception(error);
    }
}

impl v8::ValueDeserializerImpl for SceneClone {}

#[op2(reentrant)]
#[buffer]
fn op_serialize(
    scope: &mut v8::HandleScope,
    value: v8::Local<v8::Value>,
    host_objects: v8::Local<v8::Value>,
    transferred_array_buffers: v8::Local<v8::Value>,
    _for_storage: v8::Local<v8::Value>,
    _error_callback: v8::Local<v8::Value>,
) -> Result<Vec<u8>, AnyError> {
    refuse_clone_extras(host_objects, transferred_array_buffers)?;
    let serializer = v8::ValueSerializer::new(scope, Box::new(SceneClone));
    serializer.write_header();
    let scope = &mut v8::TryCatch::new(scope);
    let written = serializer.write_value(scope.get_current_context(), value);
    if scope.has_caught() || scope.has_terminated() {
        // the getter's or clone error's exception reaches the caller
        scope.rethrow();
        return Ok(Vec::new());
    }
    match written {
        Some(true) => Ok(serializer.release()),
        _ => anyhow::bail!("Failed to serialize response"),
    }
}

#[op2]
fn op_deserialize<'a>(
    scope: &mut v8::HandleScope<'a>,
    #[buffer] zero_copy: JsBuffer,
    host_objects: v8::Local<'a, v8::Value>,
    transferred_array_buffers: v8::Local<'a, v8::Value>,
    _for_storage: v8::Local<'a, v8::Value>,
) -> Result<v8::Local<'a, v8::Value>, AnyError> {
    refuse_clone_extras(host_objects, transferred_array_buffers)?;
    let deserializer = v8::ValueDeserializer::new(scope, Box::new(SceneClone), &zero_copy);
    let context = scope.get_current_context();
    if !deserializer.read_header(context).unwrap_or_default() {
        anyhow::bail!("could not deserialize value");
    }
    deserializer
        .read_value(context)
        .ok_or_else(|| anyhow::anyhow!("could not deserialize value"))
}

/// Compiles and runs `source` as a classic script, returning `[result, error]` where `error`
/// is `[thrown, isNativeError, isCompileError]`. The original unwraps reads of the
/// host-defined options array, which only node compatibility passes.
#[op2(reentrant)]
fn op_eval_context<'a>(
    scope: &mut v8::HandleScope<'a>,
    source: v8::Local<'a, v8::Value>,
    #[string] specifier: String,
    host_defined_options: v8::Local<'a, v8::Value>,
) -> Result<v8::Local<'a, v8::Value>, AnyError> {
    if !host_defined_options.is_null_or_undefined() {
        anyhow::bail!("host-defined options are not available to scenes");
    }
    let source = v8::Local::<v8::String>::try_from(source)
        .map_err(|_| deno_core::error::type_error("Invalid source"))?;
    let specifier = deno_core::resolve_url(&specifier)?;
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
    let scope = &mut v8::TryCatch::new(scope);
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
) -> Result<(), AnyError> {
    Err(unavailable("MessagePort"))
}

// URLPattern and URL.createObjectURL are not installed for scenes, and the originals
// deserialize their arguments with serde_v8, which panics when a getter throws.
#[op2(fast)]
fn op_urlpattern_parse(
    _input: v8::Local<v8::Value>,
    _base_url: v8::Local<v8::Value>,
    _options: v8::Local<v8::Value>,
) -> Result<(), AnyError> {
    Err(unavailable("URLPattern"))
}

#[op2(fast)]
fn op_urlpattern_process_match_input(
    _input: v8::Local<v8::Value>,
    _base_url: v8::Local<v8::Value>,
) -> Result<(), AnyError> {
    Err(unavailable("URLPattern"))
}

#[op2(fast)]
fn op_blob_create_object_url(
    _media_type: v8::Local<v8::Value>,
    _part_ids: v8::Local<v8::Value>,
) -> Result<(), AnyError> {
    Err(unavailable("URL.createObjectURL"))
}

/// Set while deno_url holds a serialization for `op_url_get_serialization`, which panics
/// when there is none.
struct UrlSerializationPending;

const URL_PARSE_OK_SERIALIZATION: u32 = 1;
const URL_PARSE_ERR: u32 = 2;
// the parse ops write eight components into the caller's buffer
const URL_COMPONENTS: usize = 8;

fn track_url_serialization(state: &mut OpState, status: u32) -> u32 {
    if status == URL_PARSE_OK_SERIALIZATION {
        state.put(UrlSerializationPending);
    }
    status
}

#[op2(fast)]
#[smi]
fn op_url_parse(state: &mut OpState, #[string] href: &str, #[buffer] buf: &mut [u32]) -> u32 {
    if buf.len() < URL_COMPONENTS {
        return URL_PARSE_ERR;
    }
    let status = deno_url::op_url_parse__raw_fn(state, href, buf);
    track_url_serialization(state, status)
}

#[op2(fast)]
#[smi]
fn op_url_parse_with_base(
    state: &mut OpState,
    #[string] href: &str,
    #[string] base_href: &str,
    #[buffer] buf: &mut [u32],
) -> u32 {
    if buf.len() < URL_COMPONENTS {
        return URL_PARSE_ERR;
    }
    let status = deno_url::op_url_parse_with_base__raw_fn(state, href, base_href, buf);
    track_url_serialization(state, status)
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
    let status = deno_url::op_url_reparse__raw_fn(state, href, setter, setter_value, buf);
    track_url_serialization(state, status)
}

#[op2]
#[string]
fn op_url_get_serialization(state: &mut OpState) -> Result<String, AnyError> {
    if state.try_take::<UrlSerializationPending>().is_none() {
        anyhow::bail!("no URL serialization is pending");
    }
    Ok(deno_url::op_url_get_serialization__raw_fn(state))
}

#[op2]
#[string]
fn op_url_stringify_search_params(#[from_v8] args: Plain<Vec<(String, String)>>) -> String {
    form_urlencoded::Serializer::new(String::new())
        .extend_pairs(args.0)
        .finish()
}

// Response.blob() reaches these; the originals deserialize ids with serde_v8.
fn blob_part_id(id: &str) -> Result<Uuid, AnyError> {
    Uuid::parse_str(id).map_err(|_| anyhow::anyhow!("Blob part not found"))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SliceOptions {
    start: usize,
    len: usize,
}

#[op2]
#[string]
fn op_blob_slice_part(
    state: &mut OpState,
    #[string] id: &str,
    #[from_v8] options: Plain<SliceOptions>,
) -> Result<String, AnyError> {
    let SliceOptions { start, len } = options.0;
    let blob_store = state.borrow::<Arc<BlobStore>>();
    let part = blob_store
        .get_part(&blob_part_id(id)?)
        .ok_or_else(|| anyhow::anyhow!("Blob part not found"))?;
    if start.checked_add(len).is_none_or(|end| end > part.size()) {
        anyhow::bail!("start + len can not be larger than blob part size");
    }
    // every part in a scene runtime is in memory, so its read is ready at once
    let bytes = part
        .read()
        .now_or_never()
        .ok_or_else(|| anyhow::anyhow!("Blob part is not in memory"))??;
    let sliced = InMemoryBlobPart::from(bytes[start..start + len].to_vec());
    Ok(blob_store.insert_part(Arc::new(sliced)).to_string())
}

#[op2(async)]
#[serde]
async fn op_blob_read_part(
    state: Rc<RefCell<OpState>>,
    #[string] id: String,
) -> Result<ToJsBuffer, AnyError> {
    let id = blob_part_id(&id)?;
    let part = state
        .borrow()
        .borrow::<Arc<BlobStore>>()
        .get_part(&id)
        .ok_or_else(|| anyhow::anyhow!("Blob part not found"))?;
    let bytes = part.read().await?;
    Ok(ToJsBuffer::from(bytes.to_vec()))
}

#[op2(fast)]
fn op_blob_remove_part(state: &mut OpState, #[string] id: &str) -> Result<(), AnyError> {
    let id = blob_part_id(id)?;
    state.borrow::<Arc<BlobStore>>().remove_part(&id);
    Ok(())
}

/// TextEncoder.encodeInto: writes UTF-16 units read and bytes written into `out_buf`.
#[op2(nofast)]
fn op_encoding_encode_into(
    scope: &mut v8::HandleScope,
    input: v8::Local<v8::Value>,
    #[buffer] buffer: &mut [u8],
    #[buffer] out_buf: &mut [u32],
) -> Result<(), AnyError> {
    if out_buf.len() < 2 {
        anyhow::bail!("encodeInto result buffer must hold two values");
    }
    let s = v8::Local::<v8::String>::try_from(input)?;
    let mut nchars = 0;
    out_buf[1] = s.write_utf8(
        scope,
        buffer,
        Some(&mut nchars),
        v8::WriteOptions::NO_NULL_TERMINATION | v8::WriteOptions::REPLACE_INVALID_UTF8,
    ) as u32;
    out_buf[0] = nchars as u32;
    Ok(())
}

/// performance.now: writes whole seconds and nanoseconds as two native-endian u32s. The
/// original writes them through the buffer's pointer, which a scene can misalign.
#[op2(fast)]
fn op_now(state: &mut OpState, #[buffer] buf: &mut [u8]) {
    if buf.len() < 8 {
        return;
    }
    let elapsed = state.borrow::<std::time::Instant>().elapsed();
    let mut nanos = elapsed.subsec_nanos();
    if !state.borrow_mut::<super::fetch::TP>().allow_hrtime() {
        nanos -= nanos % 2_000_000;
    }
    buf[..4].copy_from_slice(&(elapsed.as_secs() as u32).to_ne_bytes());
    buf[4..8].copy_from_slice(&nanos.to_ne_bytes());
}

fn refused() -> AnyError {
    anyhow::anyhow!("raw socket access is not available to scenes")
}

macro_rules! refused_sync_op {
    ($name:ident($($arg:ident),*)) => {
        #[op2(fast)]
        fn $name($($arg: v8::Local<v8::Value>),*) -> Result<(), AnyError> {
            $(let _ = $arg;)*
            Err(refused())
        }
    };
}

macro_rules! refused_async_op {
    ($name:ident($($arg:ident),*)) => {
        #[op2(async)]
        async fn $name($(#[global] $arg: v8::Global<v8::Value>),*) -> Result<(), AnyError> {
            $(drop($arg);)*
            Err(refused())
        }
    };
}

refused_sync_op!(op_net_listen_tcp(addr, reuse_port, load_balanced));
refused_sync_op!(op_net_listen_udp(addr, reuse_address, loopback));
refused_sync_op!(op_node_unstable_net_listen_udp(
    addr,
    reuse_address,
    loopback
));
refused_sync_op!(op_set_nodelay(rid, nodelay));
refused_sync_op!(op_set_keepalive(rid, keepalive));
refused_sync_op!(op_tls_key_null());
refused_sync_op!(op_tls_key_static(cert, key));
refused_sync_op!(op_tls_key_static_from_file(api, cert_file, key_file));
refused_sync_op!(op_tls_cert_resolver_create());
refused_sync_op!(op_tls_cert_resolver_resolve(lookup, sni, key));
refused_sync_op!(op_tls_cert_resolver_resolve_error(lookup, sni, error));
refused_sync_op!(op_tls_start(args));
refused_sync_op!(op_net_listen_tls(addr, args, keys));
refused_sync_op!(op_net_listen_unix(path, api_name));
refused_sync_op!(op_net_listen_unixpacket(path));
refused_sync_op!(op_node_unstable_net_listen_unixpacket(path));

refused_async_op!(op_net_accept_tcp(rid));
refused_async_op!(op_net_connect_tcp(addr));
refused_async_op!(op_net_recv_udp(rid, buf));
refused_async_op!(op_net_send_udp(rid, addr, buf));
refused_async_op!(op_net_join_multi_v4_udp(rid, address, multi_interface));
refused_async_op!(op_net_join_multi_v6_udp(rid, address, multi_interface));
refused_async_op!(op_net_leave_multi_v4_udp(rid, address, multi_interface));
refused_async_op!(op_net_leave_multi_v6_udp(rid, address, multi_interface));
refused_async_op!(op_net_set_multi_loopback_udp(
    rid,
    is_v4_membership,
    loopback
));
refused_async_op!(op_net_set_multi_ttl_udp(rid, ttl));
refused_async_op!(op_dns_resolve(args));
refused_async_op!(op_tls_cert_resolver_poll(lookup));
refused_async_op!(op_net_connect_tls(addr, args, key_pair));
refused_async_op!(op_net_accept_tls(rid));
refused_async_op!(op_tls_handshake(rid));
#[cfg(unix)]
refused_async_op!(op_net_accept_unix(rid));
#[cfg(unix)]
refused_async_op!(op_net_connect_unix(path));
#[cfg(unix)]
refused_async_op!(op_net_recv_unixpacket(rid, buf));
#[cfg(unix)]
refused_async_op!(op_net_send_unixpacket(rid, path, buf));
