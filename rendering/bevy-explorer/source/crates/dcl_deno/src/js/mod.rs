use crate::js::errors::{generic_error, OpError};
use common::sync::MutexExt;
use std::{cell::RefCell, collections::HashMap, rc::Rc, time::Duration};

use base64::{prelude::BASE64_URL_SAFE_NO_PAD, Engine};
use bevy::log::{debug, error, info_span};
use common::structs::GlobalCrdtStateUpdate;
use dcl_js::{
    begin_tick, end_start, engine::crdt_send_to_renderer, init_state, CommunicatedWithRenderer,
    CrdtSendsThisTick, FilteredCrdtStore, KillFlag, SuperUserScene,
};
use dcl_types::{
    interface::{crdt_context::CrdtContext, CrdtComponentInterfaces, CrdtStore},
    RendererResponse, RpcCalls, SceneResourceCounters, SceneResponse, SceneResponseSender,
};
use deno_core::{
    ascii_str, error::AnyError, op2, v8, Extension, JsRuntime, OpDecl, OpState,
    PollEventLoopOptions, RuntimeOptions,
};
use multihash_codetable::MultihashDigest;
use platform::project_directories;
use system_bridge::SystemApi;
use tokio::sync::mpsc::UnboundedReceiver;

use common::structs::SceneJsFile;

#[cfg(feature = "inspect")]
use crate::js::inspector::InspectorServer;
use crate::VM_HANDLES;
#[cfg(feature = "inspect")]
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
#[cfg(not(feature = "inspect"))]
pub struct InspectorServer;

mod array_buffers;
mod blob_slice;
mod clock;
mod errors;
pub mod fetch;
#[cfg(feature = "inspect")]
pub mod inspector;
pub mod local_storage;
pub mod op_wrappers;
mod plain;
mod rejections;
mod sources;
mod structured_clone;
pub mod websocket;

/// The upstream ops scenes get: the ones a script the runtime loads calls. Every other upstream
/// op is disabled, so a call fails. Several of the rest abort the process on scene input
/// (`op_panic`, `op_compile_function`, `op_deserialize`, `op_urlpattern_process_match_input`,
/// the leak-tracing ops), and the lock manager and BroadcastChannel are not part of the scene
/// runtime. An op a new upstream version adds stays disabled until it is listed here.
const UPSTREAM_OPS: &[&str] = &[
    "op_abort_wasm_streaming",
    "op_add_main_module_handler",
    "op_base64_decode",
    "op_base64_encode_from_buffer",
    "op_blob_clone_part",
    "op_blob_create_object_url",
    "op_blob_create_part",
    "op_blob_from_object_url",
    "op_blob_read_part",
    "op_blob_remove_part",
    "op_blob_revoke_object_url",
    "op_blob_slice_part",
    "op_cancel_handle",
    "op_cancel_read",
    "op_close",
    "op_console_css_to_ansi",
    "op_console_format_value",
    "op_console_get_string_width",
    "op_console_inspect",
    "op_console_inspect_args",
    "op_console_parse_css",
    "op_console_parse_css_color",
    "op_console_quote_string",
    "op_console_strip_vt",
    "op_decode",
    "op_defer",
    "op_destructure_error",
    "op_dispatch_exception",
    "op_encode",
    "op_encode_binary_string",
    "op_encoding_decode",
    "op_encoding_decode_single",
    "op_encoding_decode_utf8",
    "op_encoding_decode_utf8_ascii_only",
    "op_encoding_encode_into",
    "op_encoding_encode_into_fallback",
    "op_encoding_new_decoder",
    "op_encoding_normalize_label",
    "op_eval_context",
    "op_event_loop_has_more_work",
    "op_get_ext_import_meta_proto",
    "op_get_extras_binding_object",
    "op_get_promise_details",
    "op_get_proxy_details",
    "op_immediate_check",
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
    "op_is_terminal",
    "op_is_typed_array",
    "op_is_weak_map",
    "op_is_weak_set",
    "op_lazy_load_esm",
    "op_load_ext_script",
    "op_memory_usage",
    "op_message_port_create_entangled",
    "op_message_port_post_message",
    "op_message_port_post_message_raw",
    "op_message_port_recv_message",
    "op_message_port_recv_message_sync",
    "op_op_names",
    "op_pipe",
    "op_print",
    "op_read",
    "op_read_all",
    "op_read_sync",
    "op_readable_stream_resource_allocate",
    "op_readable_stream_resource_allocate_sized",
    "op_readable_stream_resource_await_close",
    "op_readable_stream_resource_close",
    "op_readable_stream_resource_get_sink",
    "op_readable_stream_resource_write_buf",
    "op_readable_stream_resource_write_error",
    "op_readable_stream_resource_write_sync",
    "op_ref_op",
    "op_resources",
    "op_run_microtasks",
    "op_set_captured_bootstrap",
    "op_set_handled_promise_rejection_handler",
    "op_set_promise_hooks",
    "op_set_wasm_streaming_callback",
    "op_shutdown",
    "op_str_byte_length",
    "op_time_origin",
    "op_timer_now",
    "op_timer_schedule",
    "op_timer_track",
    "op_timer_untrack",
    "op_try_close",
    "op_unref_op",
    "op_url_get_serialization",
    "op_url_parse",
    "op_url_parse_search_params",
    "op_url_parse_with_base",
    "op_url_reparse",
    "op_url_stringify_search_params",
    "op_wasm_streaming_feed",
    "op_wasm_streaming_set_url",
    "op_write",
    "op_write_all",
    "op_write_sync",
    "op_write_type_error",
];

/// The runtime's own ops, and the implementations that replace an op of the same name: ours
/// over deno_fetch's, and the overrides of upstream ops.
fn scene_ops(super_user: bool) -> (Vec<OpDecl>, HashMap<&'static str, OpDecl>) {
    let mut ops = vec![op_require(), op_log(), op_error()];

    let op_sets: [Vec<deno_core::OpDecl>; 16] = [
        op_wrappers::engine::ops(),
        op_wrappers::restricted_actions::ops(),
        op_wrappers::runtime::ops(),
        fetch::ops(),
        op_wrappers::portables::ops(),
        op_wrappers::user_identity::ops(),
        op_wrappers::player::ops(),
        op_wrappers::events::ops(),
        op_wrappers::comms::ops(),
        op_wrappers::comms_api::ops(),
        op_wrappers::testing::ops(),
        op_wrappers::ethereum_controller::ops(),
        op_wrappers::adaption_layer_helper::ops(),
        op_wrappers::system_api::ops(super_user),
        websocket::ops(),
        local_storage::ops(),
    ];

    // add plugin registrations
    let mut op_map = HashMap::new();
    for set in op_sets {
        for op in &set {
            // explicitly record the ones we added so we can remove deno_fetch imposters
            op_map.insert(op.name, *op);
        }
        ops.extend(set);
    }

    for op in clock::override_ops()
        .into_iter()
        .chain(structured_clone::override_ops())
        .chain(rejections::override_ops())
        .chain(blob_slice::override_ops())
    {
        op_map.insert(op.name, op);
    }

    (ops, op_map)
}

/// Per-isolate V8 heap cap. Every scene runs in its own isolate on its own thread, so a cap here
/// bounds ONE scene's memory and stops a single runaway/hostile scene from OOM-killing the shared
/// sidecar (and every co-tenant scene with it). The near-limit callback in
/// [`create_runtime_with_limits`] terminates just that isolate instead of aborting the process.
const MAX_SCENE_HEAP_BYTES: usize = 512 * 1024 * 1024;

/// Per-scene ArrayBuffer budget. Their memory is outside the V8 heap, so the heap cap does not
/// bound it; a scene past this budget is terminated like one past its heap cap.
const MAX_SCENE_ARRAY_BUFFER_BYTES: usize = 512 * 1024 * 1024;

/// What one scene's isolate may hold.
#[derive(Clone, Copy)]
struct SceneLimits {
    heap: usize,
    array_buffers: usize,
}

const SCENE_LIMITS: SceneLimits = SceneLimits {
    heap: MAX_SCENE_HEAP_BYTES,
    array_buffers: MAX_SCENE_ARRAY_BUFFER_BYTES,
};

pub fn create_runtime(
    inspect: bool,
    super_user: bool,
    storage_root: &str,
    kill_flag: KillFlag,
) -> (JsRuntime, Option<InspectorServer>) {
    create_runtime_with_limits(inspect, super_user, storage_root, kill_flag, SCENE_LIMITS)
}

fn create_runtime_with_limits(
    inspect: bool,
    super_user: bool,
    storage_root: &str,
    kill_flag: KillFlag,
    limits: SceneLimits,
) -> (JsRuntime, Option<InspectorServer>) {
    let max_heap = limits.heap;
    #[cfg(test)]
    crate::test_platform();

    let [webidl, web, fetch_ext] = web_extensions();

    let storage_digest = multihash_codetable::Code::Sha2_256.digest(storage_root.as_bytes());
    let storage_hash = BASE64_URL_SAFE_NO_PAD.encode(storage_digest.digest());
    let storage_folder = project_directories()
        .unwrap()
        .data_local_dir()
        .join("LocalStorage")
        .join(storage_hash);

    let (ops, op_map) = scene_ops(super_user);
    let own_ops: std::collections::HashSet<&str> = ops.iter().map(|op| op.name).collect();
    let ext = Extension {
        name: "decentraland",
        ops: ops.into(),
        esm_files: sources::own().into(),
        esm_entry_point: Some("ext:BevyExplorer/init.js"),
        middleware_fn: Some(Box::new(move |op: OpDecl| -> OpDecl {
            if let Some(custom_op) = op_map.get(&op.name) {
                debug!("replace: {}", op.name);
                op.with_implementation_from(custom_op)
            } else if own_ops.contains(&op.name) || UPSTREAM_OPS.contains(&op.name) {
                debug!("default: {}", op.name);
                op
            } else {
                debug!("disable: {}", op.name);
                op.disable()
            }
        })),
        ..Default::default()
    };

    // granted on each near-limit trip while the scene is being terminated
    const HEAP_LIMIT_MARGIN: usize = 128 * 1024 * 1024;

    let array_buffers = array_buffers::ArrayBufferBudget::new(limits.array_buffers);

    // create runtime
    #[allow(unused_mut)]
    let mut runtime = JsRuntime::new(RuntimeOptions {
        extensions: vec![webidl, web, fetch_ext, ext],
        inspector: inspect,
        create_params: Some(
            deno_core::v8::CreateParams::default()
                .heap_limits(0, max_heap)
                .array_buffer_allocator(array_buffers::allocator(&array_buffers)),
        ),
        ..Default::default()
    });

    runtime
        .op_state()
        .borrow_mut()
        .put(local_storage::LocalStorageDir(storage_folder));
    runtime.op_state().borrow_mut().put(array_buffers.clone());

    // Past its ArrayBuffer budget the scene ends the same way as past its heap cap.
    {
        let terminate_handle = runtime.v8_isolate().thread_safe_handle();
        let kill_flag = kill_flag.clone();
        let logged = std::sync::atomic::AtomicBool::new(false);
        let limit = limits.array_buffers;
        array_buffers.on_over(move || {
            if !logged.swap(true, std::sync::atomic::Ordering::SeqCst) {
                bevy::prelude::error!(
                    "scene exceeded its {limit}-byte ArrayBuffer budget; terminating the scene isolate"
                );
            }
            kill_flag.kill();
            terminate_handle.terminate_execution();
        });
    }

    // On approaching the cap, terminate this isolate's execution so its JS unwinds and the
    // scene ends cleanly; raise the reported limit so V8 has headroom to run the
    // termination itself instead of hard-aborting the whole sidecar process.
    {
        let terminate_handle = runtime.v8_isolate().thread_safe_handle();
        let logged = std::sync::atomic::AtomicBool::new(false);
        runtime.add_near_heap_limit_callback(move |current, _initial| {
            let first_trip = !logged.swap(true, std::sync::atomic::Ordering::SeqCst);
            if first_trip {
                bevy::prelude::error!(
                    "scene exceeded its {max_heap}-byte heap cap; terminating the scene isolate"
                );
            }
            // termination alone only unwinds the current js: the heap stays rooted by the
            // scene globals and the scene loop keeps ticking through uncaught errors, so it
            // would re-trip this callback forever. the kill flag makes the loop exit, which
            // drops the runtime and actually releases the heap.
            kill_flag.kill();
            terminate_handle.terminate_execution();
            // V8 aborts the process if an allocation in flight still does not fit, and the
            // termination only runs once it does, so every trip grants a margin. The ceiling
            // bounds what a scene that keeps allocating through termination can reach.
            current
                .saturating_add(HEAP_LIMIT_MARGIN)
                .min(max_heap.saturating_mul(2))
                .max(current)
        });
    }

    #[cfg(feature = "inspect")]
    if inspect {
        bevy::prelude::info!(
            "[{}] inspector attached",
            std::thread::current().name().unwrap()
        );
        let server = InspectorServer::new(
            SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 9222),
            "bevy-explorer",
        );
        server.register_inspector("decentraland".to_owned(), &mut runtime, true);
        (runtime, Some(server))
    } else {
        (runtime, None)
    }

    #[cfg(not(feature = "inspect"))]
    if inspect {
        panic!("can't inspect without inspect feature")
    } else {
        (runtime, None)
    }
}

/// deno_webidl, deno_web and the vendored fetch scripts, with every script compiled in.
fn web_extensions() -> [Extension; 3] {
    let mut webidl = deno_webidl::deno_webidl::init();
    // per runtime, so no Blob state is shared between scenes
    let mut web = deno_web::deno_web::init(
        deno_web::BlobStore::default_arc(),
        None,
        false,
        deno_web::InMemoryBroadcastChannel::default(),
    );
    // scenes get none of these classes
    web.objects = Default::default();
    sources::embed_upstream(&mut webidl);
    sources::embed_upstream(&mut web);
    let fetch = Extension {
        name: "deno_fetch",
        lazy_loaded_js_files: sources::deno_fetch().into(),
        ..Default::default()
    };
    [webidl, web, fetch]
}

pub struct StorageRoot(pub String);

// main scene processing thread - constructs an isolate and runs the scene
pub(crate) fn scene_thread(
    initial_crdt_store: CrdtStore,
    scene_context: CrdtContext,
    storage_root: String,
    scene_js: SceneJsFile,
    crdt_component_interfaces: CrdtComponentInterfaces,
    thread_sx: SceneResponseSender,
    thread_rx: UnboundedReceiver<RendererResponse>,
    global_update_receiver: tokio::sync::broadcast::Receiver<GlobalCrdtStateUpdate>,
    inspect: bool,
    super_user: Option<tokio::sync::mpsc::UnboundedSender<SystemApi>>,
    scene_origin: bevy::prelude::Vec3,
) {
    let scene_id = scene_context.scene_id;
    let kill_flag = KillFlag::default();
    // V8 posts delayed tasks onto the tokio runtime that is current when the isolate is
    // created, so it is entered first.
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .enable_io()
        .build()
        .unwrap();
    let _rt_guard = rt.enter();
    let (mut runtime, inspector) = create_runtime(
        inspect,
        super_user.is_some(),
        &storage_root,
        kill_flag.clone(),
    );

    // store handle
    let vm_handle = runtime.v8_isolate().thread_safe_handle();
    let mut guard = VM_HANDLES.lock_ignore_poison();
    guard.insert(scene_id, (vm_handle, kill_flag.clone()));
    drop(guard);

    let state = runtime.op_state();
    init_state(
        &mut *state.borrow_mut(),
        initial_crdt_store,
        scene_context,
        storage_root,
        scene_js,
        crdt_component_interfaces,
        thread_sx,
        thread_rx,
        global_update_receiver,
        super_user,
        scene_origin,
        kill_flag.clone(),
    );

    let span = info_span!("js startup").entered();
    state.borrow_mut().put(span);

    // store kill handle
    state
        .borrow_mut()
        .put(runtime.v8_isolate().thread_safe_handle());

    if inspector.is_some() {
        let _ = state
            .borrow_mut()
            .borrow_mut::<SceneResponseSender>()
            .try_send(SceneResponse::WaitingForInspector);

        runtime
            .inspector()
            .wait_for_session_and_break_on_next_statement();
    }

    // The first interval includes every startup operation counted below.
    let metrics_started = std::time::Instant::now();

    // load module
    let script = rt.block_on(async {
        runtime.execute_script("<loader>", ascii_str!("require (\"~scene.js\")"))
    });

    let script = match script {
        Err(e) => {
            error!("[scene thread {scene_id:?}] script load error: {}", e);
            let _ = state
                .borrow_mut()
                .take::<SceneResponseSender>()
                .try_send(SceneResponse::Error(format!("{e}")));
            return;
        }
        Ok(script) => script,
    };

    debug!(
        "[scene thread {scene_id:?}] post script execute, {} rpc calls",
        state.borrow().borrow::<RpcCalls>().len()
    );

    // send any initial rpc requests
    crdt_send_to_renderer(state.clone(), &[]);
    // the initial send drew on the tick's send allowance; give onStart a full one
    state.borrow_mut().try_take::<CrdtSendsThisTick>();

    let mut last_stats = metrics_started;
    // run startup function
    let run_start = thread_cpu_us();
    let result =
        rt.block_on(async { run_script(&mut runtime, &script, "onStart", |_| Vec::new()).await });
    state
        .borrow_mut()
        .borrow_mut::<SceneResourceCounters>()
        .run_us += thread_cpu_us().saturating_sub(run_start);
    end_start(&mut *state.borrow_mut());

    debug!(
        "[scene thread {scene_id:?}] post startup, {} rpc calls",
        state.borrow().borrow::<RpcCalls>().len()
    );

    if let Err(e) = result {
        // ignore failure to send failure
        error!("[{scene_id:?}] onStart err: {e}");
        let _ = state
            .borrow_mut()
            .take::<SceneResponseSender>()
            .try_send(SceneResponse::Error(format!("{e}")));
        return;
    }

    // lifetime uncaught-error count; only gates the log cap
    let mut reported_errors = 0;
    // consecutive ticks that never reached the renderer interface functions
    let mut silent_ticks = 0;
    loop {
        let dt = begin_tick(&mut *state.borrow_mut());

        // run the onUpdate function
        let run_start = thread_cpu_us();
        let result = rt.block_on(async {
            run_script(&mut runtime, &script, "onUpdate", |scope| {
                vec![v8::Number::new(scope, dt).into()]
            })
            .await
        });
        let cpu_us = thread_cpu_us().saturating_sub(run_start);
        {
            let mut guard = state.borrow_mut();
            let counters = guard.borrow_mut::<SceneResourceCounters>();
            counters.run_us += cpu_us;
            counters.updates_completed += 1;
            counters.update_errors += u64::from(result.is_err());
            counters.record_update_cpu(cpu_us);
        }
        // Only completed work publishes a sample. A hung update leaves a gap.
        // Sample heap and retained payloads together; these walks run at most every 5s.
        if last_stats.elapsed() >= Duration::from_secs(5) {
            let heap = runtime.v8_isolate().get_heap_statistics();
            let mut guard = state.borrow_mut();
            let stores = [
                guard.borrow::<CrdtStore>(),
                &guard.borrow::<FilteredCrdtStore>().0,
            ];
            let store_bytes = stores
                .iter()
                .map(|store| store.retained_data_bytes() as u64)
                .sum();
            let mut entities = std::collections::HashSet::new();
            let mut components = 0u64;
            for store in stores {
                for component in store.lww.values() {
                    entities.extend(component.last_write.keys().copied());
                    components += component.last_write.len() as u64;
                }
                for component in store.go.values() {
                    entities.extend(component.0.keys().copied());
                    components += component.0.len() as u64;
                }
            }
            let array_buffer_bytes = guard
                .try_borrow::<std::sync::Arc<array_buffers::ArrayBufferBudget>>()
                .map_or(0, |budget| budget.used() as u64);
            let outstanding = fetch::outstanding_calls(&guard);
            let open = websocket::open_sockets(&guard);
            let counters = guard.borrow_mut::<SceneResourceCounters>();
            counters.heap_used = heap.used_heap_size() as u64;
            counters.heap_limit = heap.heap_size_limit() as u64;
            counters.external_memory = heap.external_memory() as u64;
            counters.array_buffers = array_buffer_bytes;
            counters.crdt_store_bytes = store_bytes;
            counters.crdt_entities = entities.len() as u64;
            counters.crdt_components = components;
            counters.fetch_in_flight = outstanding;
            counters.ws_open = open;
            counters.sample_seq += 1;
            counters.sample_elapsed_us = metrics_started.elapsed().as_micros() as u64;
            counters.sample_timestamp_ms = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64;
            let snapshot = counters.clone();
            let _ = guard
                .borrow::<SceneResponseSender>()
                .try_send(SceneResponse::Stats(snapshot));
            last_stats = std::time::Instant::now();
        }

        // set cooperatively by the ops (renderer channel closed, policy kill) or
        // externally with terminate_execution (watchdog kill, heap cap); either way
        // exit instead of running another tick, which for a terminated scene would
        // re-enter a deterministic wedge
        if kill_flag.killed() {
            debug!("[{scene_id:?}] scene loop exiting");
            rt.block_on(async move {
                drop(runtime);
            });
            return;
        }

        let err = result.err();
        if let Some(e) = &err {
            reported_errors += 1;
            if reported_errors <= 10 {
                error!("[{scene_id:?}] uncaught error: {e}");
                if reported_errors == 10 {
                    error!("[{scene_id:?}] not logging any further uncaught errors.")
                }
            }
        }

        // we don't exit on uncaught `onUpdate` errors, only on the scene failing to reach the
        // renderer interface functions. a tick that doesn't reach them never blocks on the
        // renderer round-trip, so a scene that stays silent (erroring or not) would spin the
        // loop hot until the renderer's not-responding timeout; cut it off after a few ticks.
        if state
            .borrow_mut()
            .try_take::<CommunicatedWithRenderer>()
            .is_some()
        {
            silent_ticks = 0;
        } else {
            silent_ticks += 1;
        }
        if silent_ticks >= 10 {
            let msg = err.map_or_else(
                || "no renderer interaction for 10 consecutive ticks".to_owned(),
                |e| format!("{e}"),
            );
            error!("[{scene_id:?}] {msg}: shutting down");
            let _ = state
                .borrow_mut()
                .take::<SceneResponseSender>()
                .try_send(SceneResponse::Error(msg));
            rt.block_on(async move {
                drop(runtime);
            });
            return;
        }
    }
}

/// Microseconds of CPU time consumed by the calling thread. The scene loop runs
/// lockstep with the renderer (run_script blocks on the crdt round-trip inside a
/// tick), so wall time would count idle waiting as script time; thread CPU time
/// only advances while JS actually executes. Non-unix targets fall back to wall
/// time.
fn thread_cpu_us() -> u64 {
    #[cfg(unix)]
    {
        let mut ts = libc::timespec {
            tv_sec: 0,
            tv_nsec: 0,
        };
        // SAFETY: ts is a valid, writable timespec
        if unsafe { libc::clock_gettime(libc::CLOCK_THREAD_CPUTIME_ID, &mut ts) } == 0 {
            (ts.tv_sec as u64) * 1_000_000 + (ts.tv_nsec as u64) / 1_000
        } else {
            0
        }
    }
    #[cfg(not(unix))]
    {
        use std::sync::OnceLock;
        static START: OnceLock<std::time::Instant> = OnceLock::new();
        START
            .get_or_init(std::time::Instant::now)
            .elapsed()
            .as_micros() as u64
    }
}

// helper to setup, acquire, run and return results from a script function
async fn run_script(
    runtime: &mut JsRuntime,
    script: &v8::Global<v8::Value>,
    fn_name: &str,
    arg_fn: impl for<'a, 'i> Fn(&mut v8::PinScope<'a, 'i>) -> Vec<v8::Local<'a, v8::Value>>,
) -> Result<(), AnyError> {
    // set up scene i/o
    let promise = {
        deno_core::scope!(scope, runtime);
        let script_this = v8::Local::new(scope, script.clone());
        // get module
        let script = v8::Local::<v8::Object>::try_from(script_this).unwrap();

        // get function
        let target_function =
            v8::String::new_from_utf8(scope, fn_name.as_bytes(), v8::NewStringType::Internalized)
                .unwrap();
        let Some(target_function) = script.get(scope, target_function.into()) else {
            // function not define, is that an error ?
            // debug!("{fn_name} is not defined");
            return Err(AnyError::msg(format!("{fn_name} is not defined")));
        };
        let Ok(target_function) = v8::Local::<v8::Function>::try_from(target_function) else {
            // error!("{fn_name} is not a function");
            return Err(AnyError::msg(format!("{fn_name} is not a function")));
        };

        // get args
        let args = arg_fn(scope);

        // call
        let res = target_function.call(scope, script_this, &args);
        let Some(res) = res else {
            // error!("{fn_name} did not return a promise");
            return Err(AnyError::msg(format!("{fn_name} did not return a promise")));
        };

        drop(args);
        v8::Global::new(scope, res)
    };

    let f = runtime.resolve(promise);

    let result = runtime
        .with_event_loop_promise(f, PollEventLoopOptions::default())
        .await
        .map(|_| ());

    Ok(result?)
}

/// The `~system/*` modules any scene can require.
const SYSTEM_MODULES: &[(&str, &str)] = &[
    (
        "~system/CommunicationsController",
        include_str!("../../../dcl_js/src/modules/CommunicationsController.js"),
    ),
    (
        "~system/CommsApi",
        include_str!("../../../dcl_js/src/modules/CommsApi.js"),
    ),
    (
        "~system/EngineApi",
        include_str!("../../../dcl_js/src/modules/EngineApi.js"),
    ),
    (
        "~system/EnvironmentApi",
        include_str!("../../../dcl_js/src/modules/EnvironmentApi.js"),
    ),
    (
        "~system/EthereumController",
        include_str!("../../../dcl_js/src/modules/EthereumController.js"),
    ),
    (
        "~system/Players",
        include_str!("../../../dcl_js/src/modules/Players.js"),
    ),
    (
        "~system/PortableExperiences",
        include_str!("../../../dcl_js/src/modules/PortableExperiences.js"),
    ),
    (
        "~system/RestrictedActions",
        include_str!("../../../dcl_js/src/modules/RestrictedActions.js"),
    ),
    (
        "~system/Runtime",
        include_str!("../../../dcl_js/src/modules/Runtime.js"),
    ),
    (
        "~system/Scene",
        include_str!("../../../dcl_js/src/modules/Scene.js"),
    ),
    (
        "~system/SignedFetch",
        include_str!("../../../dcl_js/src/modules/SignedFetch.js"),
    ),
    (
        "~system/Testing",
        include_str!("../../../dcl_js/src/modules/Testing.js"),
    ),
    (
        "~system/UserActionModule",
        include_str!("../../../dcl_js/src/modules/UserActionModule.js"),
    ),
    (
        "~system/UserIdentity",
        include_str!("../../../dcl_js/src/modules/UserIdentity.js"),
    ),
    (
        "~system/AdaptationLayerHelper",
        include_str!("../../../dcl_js/src/modules/AdaptationLayerHelper.js"),
    ),
];

// synchronously returns a string containing JS code from the file system
#[op2]
#[string]
fn op_require(
    state: Rc<RefCell<OpState>>,
    #[string] module_spec: String,
) -> Result<String, OpError> {
    debug!("require(\"{module_spec}\")");

    match module_spec.as_str() {
        // user module load
        "~scene.js" => Ok(state.borrow().borrow::<SceneJsFile>().0.as_ref().clone()),
        // system api (only allowed for su scene)
        "~system/BevyExplorerApi" => {
            if state.borrow().try_borrow::<SuperUserScene>().is_some() {
                Ok(include_str!("../../../dcl_js/src/modules/SystemApi.js").to_owned())
            } else {
                Err(generic_error(format!("invalid module request `{module_spec}`")).into())
            }
        }
        // core module load
        _ => SYSTEM_MODULES
            .iter()
            .find(|(name, _)| *name == module_spec)
            .map(|(_, source)| (*source).to_owned())
            .ok_or_else(|| generic_error(format!("invalid module request `{module_spec}`")).into()),
    }
}

#[op2(fast)]
fn op_log(state: Rc<RefCell<OpState>>, #[string] message: String) {
    dcl_js::op_log(state, message);
}

#[op2(fast)]
fn op_error(state: Rc<RefCell<OpState>>, #[string] message: String) {
    dcl_js::op_error(state, message);
}

#[cfg(test)]
mod tests {
    mod system_scene_network {
        include!("system_scene_network_tests.rs");
    }
    mod websocket {
        include!("websocket_tests.rs");
    }
    mod runtime_surface {
        include!("runtime_surface_tests.rs");
    }
    use std::sync::Arc;

    use common::structs::SceneJsFile;
    use dcl_types::{interface::crdt_context::CrdtContext, SceneId};
    use deno_core::{ascii_str, JsRuntime};

    use super::InspectorServer;

    /// `super::create_runtime` inside a tokio runtime, which V8's delayed tasks need.
    fn create_runtime(
        inspect: bool,
        super_user: bool,
        storage_root: &str,
        kill_flag: dcl_js::KillFlag,
    ) -> (JsRuntime, Option<InspectorServer>) {
        static RT: std::sync::LazyLock<tokio::runtime::Runtime> = std::sync::LazyLock::new(|| {
            tokio::runtime::Builder::new_multi_thread()
                .worker_threads(1)
                .enable_time()
                .build()
                .unwrap()
        });
        let _guard = tokio::runtime::Handle::try_current()
            .is_err()
            .then(|| RT.enter());
        super::create_runtime(inspect, super_user, storage_root, kill_flag)
    }

    fn context(is_server: bool) -> CrdtContext {
        CrdtContext::new(
            SceneId(bevy::prelude::Entity::from_raw(0)),
            "hash".to_owned(),
            "title".to_owned(),
            false,
            false,
            is_server,
        )
    }

    /// Boot a scene runtime and evaluate `scene_js` exactly the way a deployed scene bundle
    /// is evaluated (`op_require("~scene.js")` -> `evalContext`). A `throw` in the scene
    /// surfaces as `Err`.
    fn run_scene(is_server: bool, scene_js: &str) -> Result<(), String> {
        run_scene_as(is_server, false, scene_js)
    }

    /// [`run_scene`], optionally as the super-user scene.
    fn run_scene_as(is_server: bool, super_user: bool, scene_js: &str) -> Result<(), String> {
        run_scene_with(is_server, super_user, super::SCENE_LIMITS, scene_js)
    }

    /// [`run_scene_as`] under `limits`.
    fn run_scene_with(
        is_server: bool,
        super_user: bool,
        limits: super::SceneLimits,
        scene_js: &str,
    ) -> Result<(), String> {
        let (mut runtime, _) = super::create_runtime_with_limits(
            false,
            super_user,
            "test-scene-realm",
            Default::default(),
            limits,
        );
        let (host, _inbox) = tokio::sync::mpsc::unbounded_channel();
        {
            let state = runtime.op_state();
            let mut state = state.borrow_mut();
            state.put(context(is_server));
            state.put(SceneJsFile(Arc::new(scene_js.to_owned())));
            if super_user {
                state.put(dcl_js::SuperUserScene(host));
            }
        }
        runtime
            .execute_script("<test>", ascii_str!("require(\"~scene.js\")"))
            .map(|_| ())
            .map_err(|e| e.to_string())
    }

    #[test]
    fn signed_fetch_scene_module_queues_only_rust_computed_body_hashes() {
        let (mut runtime, _) =
            create_runtime(false, false, "test-signed-fetch", Default::default());
        let op_state = runtime.op_state();
        op_state.borrow_mut().put(context(false));
        op_state.borrow_mut().put(dcl_types::RpcCalls::default());
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let _guard = rt.enter();
        runtime.execute_script("<test>", ascii_str!(r#"
            globalThis.fetch = async (url, init) => {
                globalThis.__sent = JSON.stringify({ url, method: init.method, body: init.body, headers: init.headers });
                return { ok: true, status: 200, statusText: 'OK', headers: {}, text: async () => '' };
            };
            globalThis.__done = false;
            const api = require('~system/SignedFetch');
            const forged = 'f'.repeat(64);
            const init = { method: 'POST', body_hash: forged, hashPayload: forged,
                headers: { 'X-Identity-Metadata': JSON.stringify({ hashPayload: forged }) } };
            Promise.all([
                api.getHeaders({url:'https://example.org/absent', init}),
                api.getHeaders({url:'https://example.org/empty', init:{...init, body:''}}),
                api.signedFetch({url:'https://example.org/unicode', init:{...init, body:'pi\u00f1ata\r\n\u{1f98a}'}}),
                api.getHeaders({url:'https://example.org/large', init:{...init, body:'x'.repeat(10 * 1024 * 1024 + 1)}}),
            ]).then(() => { globalThis.__done = true; });
        "#)).unwrap();
        const UNICODE_HASH: &str =
            "867ee32bed3423b5d737ff3d6afa68cf9478857edba007c744c63d507b8a4d30";
        let expected = [
            ("absent", None),
            (
                "empty",
                Some("e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"),
            ),
            ("unicode", Some(UNICODE_HASH)),
            (
                "large",
                Some("e2be3b83c289189dd311afb0db7a79ec8cfbb993ca5267315b437e41edccec98"),
            ),
        ];
        // All four requests must be queued before the event loop polls any pending promises.
        let calls = std::mem::take(op_state.borrow_mut().borrow_mut::<dcl_types::RpcCalls>());
        assert_eq!(calls.len(), expected.len());
        for (call, (path, digest)) in calls.into_iter().zip(expected) {
            let common::rpc::RpcCall::SignRequest(request) = call else {
                panic!("expected a signing request")
            };
            assert_eq!(request.method, "POST");
            assert_eq!(request.uri, format!("https://example.org/{path}"));
            assert_eq!(request.body_hash.as_deref(), digest);
            let metadata = match request.body_hash {
                Some(hash) => serde_json::json!({ "hashPayload": hash }),
                None => serde_json::json!({}),
            };
            request.response.send(Ok(vec![(
                "x-identity-metadata".into(),
                metadata.to_string(),
            )]));
        }
        rt.block_on(runtime.run_event_loop(Default::default()))
            .unwrap();
        let value = runtime
            .execute_script(
                "<test>",
                ascii_str!("JSON.stringify({done: __done, sent: JSON.parse(__sent)})"),
            )
            .unwrap();
        deno_core::scope!(scope, runtime);
        let result: serde_json::Value = serde_json::from_str(
            &deno_core::v8::Local::new(scope, value).to_rust_string_lossy(scope),
        )
        .unwrap();
        assert_eq!(result["done"], true);
        assert_eq!(result["sent"]["body"], "piñata\r\n🦊");
        assert_eq!(
            result["sent"]["headers"],
            serde_json::json!([[
                "x-identity-metadata",
                format!(r#"{{"hashPayload":"{UNICODE_HASH}"}}"#)
            ]])
        );
    }

    /// The scene bundle shares a realm with the runtime, so anything left on the global
    /// object is scene-reachable -- and `Deno.core.ops` is every op with none of the checks
    /// the `~system/*` wrappers apply. This is the property, not an implementation detail.
    #[test]
    fn scene_cannot_reach_the_ops_table_on_the_server() {
        run_scene(
            true,
            r#"
            if (typeof Deno !== "undefined") { throw new Error("`Deno` is reachable"); }
            if (typeof __bootstrap !== "undefined") { throw new Error("`__bootstrap` is reachable"); }
            if (typeof __infra !== "undefined") { throw new Error("`__infra` is reachable"); }
            // the global object itself, reached the indirect way
            const g = Function("return this")();
            if (g.Deno !== undefined) { throw new Error("`Deno` is reachable via globalThis"); }
            if (g.__bootstrap !== undefined) { throw new Error("`__bootstrap` is reachable via globalThis"); }
            "#,
        )
        .unwrap();
    }

    /// A module's `module.exports.x = ...` runs a scene setter on `Object.prototype`, and a
    /// non-strict wrapper would hand that setter its `Deno` argument through `caller`.
    #[test]
    fn a_scene_setter_cannot_read_the_module_wrapper_arguments() {
        run_scene(
            true,
            r#"
            let seen = "the setter did not run";
            Object.defineProperty(Object.prototype, "getRealm", {
                set: function (value) {
                    try {
                        seen = typeof arguments.callee.caller.arguments[5];
                    } catch (e) {
                        seen = "refused";
                    }
                    Object.defineProperty(this, "getRealm", { value, writable: true });
                },
                configurable: true,
            });
            const runtime = require("~system/Runtime");
            if (seen !== "refused") { throw new Error("the wrapper's arguments are reachable: " + seen); }
            if (typeof runtime.getRealm !== "function") { throw new Error("Runtime did not load"); }
            "#,
        )
        .unwrap();
    }

    /// Every module a scene can require loads under the strict wrapper, sealed or not.
    #[test]
    fn every_system_module_loads() {
        let names: Vec<&str> = super::SYSTEM_MODULES
            .iter()
            .map(|(name, _)| *name)
            .collect();
        let scene = format!(
            r#"
            for (const name of {names:?}) {{
                if (Object.keys(require(name)).length === 0) {{
                    throw new Error(name + " exports nothing");
                }}
            }}
            "#
        );
        for server in [true, false] {
            run_scene(server, &scene).unwrap();
        }
    }

    /// The HUD's module is the largest one; it loads only for a super-user scene.
    #[test]
    fn the_super_user_module_loads() {
        let scene = r#"if (Object.keys(require("~system/BevyExplorerApi")).length === 0) { throw new Error("no exports"); }"#;
        run_scene_as(false, true, scene).unwrap();
        assert!(run_scene(false, scene).is_err());
    }

    /// Each module is evaluated under its own name, so a stack trace says where an error is.
    #[test]
    fn a_module_script_is_named_after_the_module() {
        run_scene(
            false,
            r#"
            const scene = new Error("x").stack;
            if (!scene.includes("file://~scene.js/:")) { throw new Error(scene); }
            // a setter the module's `exports.getRealm = ...` runs, from inside that module
            let system = "the setter did not run";
            Object.defineProperty(Object.prototype, "getRealm", {
                set(value) {
                    system = new Error("x").stack;
                    Object.defineProperty(this, "getRealm", { value, writable: true });
                },
                configurable: true,
            });
            require("~system/Runtime");
            delete Object.prototype.getRealm;
            if (!system.includes("file://~system/Runtime:")) { throw new Error(system); }
            "#,
        )
        .unwrap();
    }

    /// ...while the modules the scene is *supposed* to use still get it, via the `Deno`
    /// parameter `require` injects. Required from scene code, i.e. after the seal.
    #[test]
    fn system_modules_still_work_after_sealing() {
        run_scene(
            true,
            r#"
            const engine = require("~system/EngineApi");
            if (typeof engine.crdtSendToRenderer !== "function") {
                throw new Error("EngineApi did not load");
            }
            if (typeof require("~system/Runtime").readFile !== "function") {
                throw new Error("Runtime did not load");
            }
            "#,
        )
        .unwrap();
    }

    /// `@dcl/ecs` installs its own `clearInterval` when the global is missing, and that one only
    /// deletes SDK timers: the interval would keep firing.
    #[test]
    fn clear_interval_stops_an_interval() {
        let (mut runtime, _) =
            create_runtime(false, false, "test-clear-interval", Default::default());
        {
            let state = runtime.op_state();
            let mut state = state.borrow_mut();
            state.put(context(false));
            state.put(SceneJsFile(Arc::new(
                r#"
                globalThis.clearInterval ??= () => {};
                globalThis.__ticks = 0;
                const id = setInterval(() => {
                    if (++globalThis.__ticks === 2) clearInterval(id);
                }, 1);
                "#
                .to_owned(),
            )));
        }
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let _guard = rt.enter();
        runtime
            .execute_script("<test>", ascii_str!("require(\"~scene.js\")"))
            .unwrap();
        rt.block_on(tokio::time::timeout(
            std::time::Duration::from_secs(5),
            runtime.run_event_loop(Default::default()),
        ))
        .expect("the interval was not cleared")
        .unwrap();
        let ticks = runtime
            .execute_script("<test>", ascii_str!("globalThis.__ticks"))
            .unwrap();
        deno_core::scope!(scope, runtime);
        let ticks = deno_core::v8::Local::new(scope, ticks);
        assert_eq!(ticks.uint32_value(scope), Some(2));
    }

    /// The seal is scoped to the authoritative server on purpose: the desktop client runs on
    /// the user's own machine, and narrowing the change keeps preview and every deployed
    /// scene behaving exactly as they do today. Pinned so the scoping is a decision rather
    /// than something that drifts.
    #[test]
    fn client_realm_is_left_alone() {
        run_scene(
            false,
            r#"if (typeof Deno === "undefined") { throw new Error("client behaviour changed"); }"#,
        )
        .unwrap();
    }

    /// A scene server has no player to partition `localStorage` by, and no business writing to
    /// its host's disk: every storage op must raise there.
    #[test]
    fn local_storage_raises_on_a_scene_server() {
        for call in [
            "localStorage.length",
            "localStorage.key(0)",
            "localStorage.getItem('k')",
            "localStorage.setItem('k', 'v')",
            "localStorage.removeItem('k')",
            "localStorage.clear()",
            "Object.keys(localStorage)",
        ] {
            let err = run_scene(true, call).expect_err(call);
            assert!(err.contains("scene server"), "{call}: {err}");
        }
    }

    /// A scene-supplied proxy is dialled instead of the URL host, so it walks straight past
    /// the per-request `assert_public_url` check -- the check only ever sees the URL. On the
    /// authoritative server the option must be refused outright.
    #[test]
    fn server_refuses_a_scene_supplied_proxy() {
        let (mut runtime, _) =
            create_runtime(false, false, "test-custom-client", Default::default());
        runtime.op_state().borrow_mut().put(context(true));
        let err = runtime
            .execute_script(
                "<test>",
                ascii_str!(
                    r#"Deno.core.ops.op_fetch_custom_client({ caCerts: [], proxy: { url: "http://169.254.169.254:80" } })"#
                ),
            )
            .expect_err("proxy must be refused on the authoritative server")
            .to_string();
        assert!(err.contains("proxy"), "unexpected error: {err}");
    }

    /// Same for a scene-supplied trust root: it would make the scene a CA for the server's
    /// own outbound TLS.
    #[test]
    fn server_refuses_scene_supplied_ca_certs() {
        let (mut runtime, _) =
            create_runtime(false, false, "test-custom-client-ca", Default::default());
        runtime.op_state().borrow_mut().put(context(true));
        let err = runtime
            .execute_script(
                "<test>",
                ascii_str!(
                    r#"Deno.core.ops.op_fetch_custom_client({ caCerts: ["-----BEGIN CERTIFICATE-----"] })"#
                ),
            )
            .expect_err("ca_certs must be refused on the authoritative server")
            .to_string();
        assert!(err.contains("root certificates"), "unexpected error: {err}");
    }

    /// The desktop/web client runs on the user's own machine and keeps the old behaviour --
    /// this restriction is server-only.
    #[test]
    fn client_still_allows_custom_clients() {
        let (mut runtime, _) = create_runtime(
            false,
            false,
            "test-custom-client-clientmode",
            Default::default(),
        );
        runtime.op_state().borrow_mut().put(context(false));
        runtime
            .execute_script(
                "<test>",
                ascii_str!(
                    r#"Deno.core.ops.op_fetch_custom_client({ caCerts: [], proxy: { url: "http://localhost:8080" } })"#
                ),
            )
            .expect("client mode must keep working");
    }

    /// A local server on two listeners: `/redirect` answers a 302 to `/target` on the second,
    /// and `/target` a 200 `arrived`. Returns both ports.
    fn redirect_servers() -> [u16; 2] {
        use std::io::{Read, Write};
        use std::net::TcpListener;

        let listeners = [
            TcpListener::bind("127.0.0.1:0").unwrap(),
            TcpListener::bind("127.0.0.1:0").unwrap(),
        ];
        let ports = listeners.each_ref().map(|l| l.local_addr().unwrap().port());
        for listener in listeners {
            let target_port = ports[1];
            std::thread::spawn(move || {
                for stream in listener.incoming() {
                    let mut stream = stream.unwrap();
                    let mut buf = [0u8; 4096];
                    let n = stream.read(&mut buf).unwrap();
                    let path = String::from_utf8_lossy(&buf[..n])
                        .split_whitespace()
                        .nth(1)
                        .unwrap_or("/")
                        .to_owned();
                    let response = if path == "/redirect" {
                        format!("HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1:{target_port}/target\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
                    } else {
                        "HTTP/1.1 200 OK\r\nContent-Length: 7\r\nConnection: close\r\n\r\narrived"
                            .to_owned()
                    };
                    stream.write_all(response.as_bytes()).unwrap();
                }
            });
        }
        ports
    }

    /// Runs `script` in a client or server scene and returns `globalThis.__outcome` once the
    /// event loop settles. Loopback needs `preview`; the fetch permission request that
    /// `op_fetch_send` parks on is granted from a plain thread, as the renderer would
    /// (`RpcResultSender::send` blocks).
    fn fetch_outcome(is_server: bool, script: String) -> String {
        use std::sync::mpsc;

        let (mut runtime, _) =
            create_runtime(false, false, "test-fetch-redirect", Default::default());
        {
            let state = runtime.op_state();
            let mut state = state.borrow_mut();
            state.put(CrdtContext::new(
                SceneId(bevy::prelude::Entity::from_raw(0)),
                "hash".to_owned(),
                "title".to_owned(),
                false,
                true,
                is_server,
            ));
            state.put(dcl_types::SceneResourceCounters::default());
            state.put(dcl_types::RpcCalls::default());
            state.put(Vec::<dcl_types::SceneLogMessage>::default());
            state.put(dcl_types::SceneElapsedTime(0.0));
        }

        let (grant_sx, grant_rx) = mpsc::channel::<common::rpc::RpcResultSender<bool>>();
        std::thread::spawn(move || {
            for sender in grant_rx {
                sender.send(true);
            }
        });

        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(async {
            runtime.execute_script("<test>", script).unwrap();
            let op_state = runtime.op_state();
            let grant_permissions = async {
                loop {
                    tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                    let calls = std::mem::take(
                        &mut **op_state.borrow_mut().borrow_mut::<dcl_types::RpcCalls>(),
                    );
                    for call in calls {
                        if let common::rpc::RpcCall::RequestGenericPermission { response, .. } =
                            call
                        {
                            grant_sx.send(response).unwrap();
                        }
                    }
                }
            };
            tokio::select! {
                r = tokio::time::timeout(
                    std::time::Duration::from_secs(10),
                    runtime.run_event_loop(deno_core::PollEventLoopOptions::default()),
                ) => r.expect("fetch did not settle").unwrap(),
                _ = grant_permissions => unreachable!(),
            }
        });
        let outcome = runtime
            .execute_script("<test>", ascii_str!("String(globalThis.__outcome)"))
            .unwrap();
        deno_core::scope!(scope, runtime);
        deno_core::v8::Local::new(scope, outcome).to_rust_string_lossy(scope)
    }

    /// One scene `fetch` of `/redirect`. Returns `"<status> <body>"`.
    fn fetch_redirect_outcome(is_server: bool) -> String {
        let ports = redirect_servers();
        fetch_outcome(
            is_server,
            format!(
                r#"fetch("http://127.0.0.1:{}/redirect")
                .then(async (r) => `${{r.status}} ${{await r.text()}}`, (e) => `error ${{e}}`)
                .then((s) => globalThis.__outcome = s);"#,
                ports[0]
            ),
        )
    }

    /// The server never auto-follows a redirect: the scene gets the 3xx back. (Following
    /// would also need the `URL` global, which this runtime does not install -- pinned here
    /// so the outcome is the policy, not that accident.)
    #[test]
    fn server_hands_redirects_back_to_the_scene() {
        let outcome = fetch_redirect_outcome(true);
        assert!(outcome.starts_with("302 "), "unexpected outcome: {outcome}");
    }

    /// The client keeps following, like the browser does.
    #[test]
    fn client_follows_redirects() {
        assert_eq!(fetch_redirect_outcome(false), "200 arrived");
    }
}
