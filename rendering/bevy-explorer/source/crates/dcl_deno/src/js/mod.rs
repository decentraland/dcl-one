use crate::js::errors::{generic_error, OpError};
use std::{cell::RefCell, collections::HashMap, rc::Rc, time::Duration};

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
    anyhow::anyhow, ascii_str, error::AnyError, op2, v8, Extension, JsRuntime, OpDecl, OpState,
    PollEventLoopOptions, RuntimeOptions,
};
use system_bridge::SystemApi;
use tokio::{sync::mpsc::UnboundedReceiver, time::timeout};

use common::structs::SceneJsFile;

#[cfg(feature = "inspect")]
use crate::js::inspector::InspectorServer;
use crate::VM_HANDLES;
#[cfg(feature = "inspect")]
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
#[cfg(not(feature = "inspect"))]
pub struct InspectorServer;

mod errors;
pub mod fetch;
mod hardened_ops;
#[cfg(feature = "inspect")]
pub mod inspector;
pub mod op_wrappers;
mod plain;
mod sources;
mod stream_resource;
pub mod websocket;

pub fn create_runtime(
    inspect: bool,
    super_user: bool,
    kill_flag: KillFlag,
) -> (JsRuntime, Option<InspectorServer>) {
    #[cfg(test)]
    crate::test_platform();

    let [webidl, web, fetch_ext] = web_extensions();

    let mut ops = vec![op_require(), op_log(), op_error()];

    let op_sets: [Vec<deno_core::OpDecl>; 15] = [
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
    ];

    // add plugin registrations
    let mut op_map = HashMap::new();
    for set in op_sets {
        for op in &set {
            op_map.insert(op.name, *op);
        }
        ops.extend(set);
    }

    let override_sets: [Vec<deno_core::OpDecl>; 2] = [
        hardened_ops::override_ops(),
        stream_resource::override_ops(),
    ];

    let mut overrides = HashMap::new();
    for set in override_sets {
        for op in set {
            op_map.insert(op.name, op);
            overrides.insert(op.name, op);
        }
    }
    // the middleware keeps the original's name and async-ness, so check each override fits
    let pending_overrides = Rc::new(RefCell::new(overrides));
    let pending_disabled = Rc::new(RefCell::new(
        hardened_ops::DISABLED_OPS
            .iter()
            .copied()
            .collect::<std::collections::HashSet<_>>(),
    ));
    let override_problems = Rc::new(RefCell::new(Vec::new()));

    let ext = Extension {
        name: "decentraland",
        ops: ops.into(),
        esm_files: sources::own().into(),
        esm_entry_point: Some("ext:BevyExplorer/init.js"),
        middleware_fn: Some(Box::new({
            let pending_overrides = pending_overrides.clone();
            let pending_disabled = pending_disabled.clone();
            let override_problems = override_problems.clone();
            move |op: OpDecl| -> OpDecl {
                if pending_disabled.borrow_mut().remove(op.name) {
                    debug!("disable: {}", op.name);
                    return op.disable();
                }
                if let Some(custom_op) = pending_overrides.borrow_mut().remove(op.name) {
                    // `arg_count` also counts state and scope parameters, so only async-ness compares
                    if custom_op.is_async != op.is_async {
                        override_problems.borrow_mut().push(format!(
                            "override {} differs from the original in async-ness",
                            op.name
                        ));
                    }
                }
                if let Some(custom_op) = op_map.get(&op.name) {
                    debug!("replace: {}", op.name);
                    op.with_implementation_from(custom_op)
                } else {
                    debug!("default: {}", op.name);
                    op
                }
            }
        })),
        ..Default::default()
    };

    // Per-isolate V8 heap cap. Every scene runs in its own isolate on its own thread, so
    // a cap here bounds ONE scene's memory and stops a single runaway/hostile scene from
    // OOM-killing the shared sidecar (and every co-tenant scene with it). The near-limit
    // callback below terminates just that isolate instead of aborting the process.
    const MAX_SCENE_HEAP_BYTES: usize = 512 * 1024 * 1024;

    // create runtime
    #[allow(unused_mut)]
    let mut runtime = JsRuntime::new(RuntimeOptions {
        extensions: vec![webidl, web, fetch_ext, ext],
        inspector: inspect,
        create_params: Some(
            deno_core::v8::CreateParams::default().heap_limits(0, MAX_SCENE_HEAP_BYTES),
        ),
        ..Default::default()
    });

    {
        let mut problems = override_problems.take();
        problems.extend(
            pending_overrides
                .borrow()
                .keys()
                .map(|name| format!("override {name} replaces no registered op")),
        );
        problems.extend(
            pending_disabled
                .borrow()
                .iter()
                .map(|name| format!("disabled op {name} is not registered")),
        );
        for problem in &problems {
            error!("{problem}");
        }
        runtime
            .op_state()
            .borrow_mut()
            .put(OpOverrideProblems(problems));
    }

    // On approaching the cap, terminate this isolate's execution so its JS unwinds and the
    // scene ends cleanly; raise the reported limit so V8 has headroom to run the
    // termination itself instead of hard-aborting the whole sidecar process.
    {
        let terminate_handle = runtime.v8_isolate().thread_safe_handle();
        let granted = std::sync::atomic::AtomicBool::new(false);
        runtime.add_near_heap_limit_callback(move |current, _initial| {
            let first_trip = !granted.swap(true, std::sync::atomic::Ordering::SeqCst);
            if first_trip {
                bevy::prelude::error!("scene exceeded its {MAX_SCENE_HEAP_BYTES}-byte heap cap; terminating the scene isolate");
            }
            // termination alone only unwinds the current js: the heap stays rooted by the
            // scene globals and the scene loop keeps ticking through uncaught errors, so it
            // would re-trip this callback forever. the kill flag makes the loop exit, which
            // drops the runtime and actually releases the heap.
            kill_flag.kill();
            terminate_handle.terminate_execution();
            if first_trip {
                // one-time margin so the termination unwind can run rather than hard-aborting
                current + 8 * 1024 * 1024
            } else {
                // kill already latched: stop re-granting, so a scene that keeps allocating
                // through termination can't ratchet the cap upward on every callback
                current
            }
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
    // per runtime, so no Blob or BroadcastChannel state is shared between scenes
    let mut web = deno_web::deno_web::init(
        deno_web::BlobStore::default_arc(),
        None,
        false,
        deno_web::InMemoryBroadcastChannel::default(),
    );
    // The cppgc class constructors (DOMMatrix, ImageData, CSSStyleSheet, ...) wrap whatever
    // `this` they are called on, so calling one without `new` corrupts the V8 heap. Scenes
    // get none of these classes.
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

/// Overrides that did not replace an op exactly, logged at runtime creation.
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) struct OpOverrideProblems(pub Vec<String>);

pub struct StorageRoot(pub String);

// main scene processing thread - constructs an isolate and runs the scene
#[allow(clippy::too_many_arguments)]
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
    // created, and aborts the process if there is none.
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .enable_io()
        .build()
        .unwrap();
    let _rt_guard = rt.enter();
    let (mut runtime, inspector) = create_runtime(inspect, super_user.is_some(), kill_flag.clone());

    // store handle
    let vm_handle = runtime.v8_isolate().thread_safe_handle();
    let mut guard = VM_HANDLES.lock().unwrap();
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
                .try_send(SceneResponse::Error(format!("{e:?}")));
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
        error!("[{scene_id:?}] onStart err: {e:?}");
        let _ = state
            .borrow_mut()
            .take::<SceneResponseSender>()
            .try_send(SceneResponse::Error(format!("{e:?}")));
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
            let outstanding = fetch::outstanding_calls(&guard);
            let open = websocket::open_sockets(&guard);
            let counters = guard.borrow_mut::<SceneResourceCounters>();
            counters.heap_used = heap.used_heap_size() as u64;
            counters.heap_limit = heap.heap_size_limit() as u64;
            counters.external_memory = heap.external_memory() as u64;
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
                error!("[{scene_id:?}] uncaught error: {e:?}");
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
                |e| format!("{e:?}"),
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

    let result = if true {
        runtime
            .with_event_loop_promise(f, PollEventLoopOptions::default())
            .await
            .map(|_| ())
    } else {
        timeout(
            Duration::from_secs(30),
            runtime.with_event_loop_promise(f, PollEventLoopOptions::default()),
        )
        .await
        .map_err(|_| anyhow!("script timed out"))?
        .map(|_| ())
    };

    Ok(result?)
}

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
        "~system/CommunicationsController" => {
            Ok(include_str!("../../../dcl_js/src/modules/CommunicationsController.js").to_owned())
        }
        "~system/CommsApi" => {
            Ok(include_str!("../../../dcl_js/src/modules/CommsApi.js").to_owned())
        }
        "~system/EngineApi" => {
            Ok(include_str!("../../../dcl_js/src/modules/EngineApi.js").to_owned())
        }
        "~system/EnvironmentApi" => {
            Ok(include_str!("../../../dcl_js/src/modules/EnvironmentApi.js").to_owned())
        }
        "~system/EthereumController" => {
            Ok(include_str!("../../../dcl_js/src/modules/EthereumController.js").to_owned())
        }
        "~system/Players" => Ok(include_str!("../../../dcl_js/src/modules/Players.js").to_owned()),
        "~system/PortableExperiences" => {
            Ok(include_str!("../../../dcl_js/src/modules/PortableExperiences.js").to_owned())
        }
        "~system/RestrictedActions" => {
            Ok(include_str!("../../../dcl_js/src/modules/RestrictedActions.js").to_owned())
        }
        "~system/Runtime" => Ok(include_str!("../../../dcl_js/src/modules/Runtime.js").to_owned()),
        "~system/Scene" => Ok(include_str!("../../../dcl_js/src/modules/Scene.js").to_owned()),
        "~system/SignedFetch" => {
            Ok(include_str!("../../../dcl_js/src/modules/SignedFetch.js").to_owned())
        }
        "~system/Testing" => Ok(include_str!("../../../dcl_js/src/modules/Testing.js").to_owned()),
        "~system/UserActionModule" => {
            Ok(include_str!("../../../dcl_js/src/modules/UserActionModule.js").to_owned())
        }
        "~system/UserIdentity" => {
            Ok(include_str!("../../../dcl_js/src/modules/UserIdentity.js").to_owned())
        }
        "~system/AdaptationLayerHelper" => {
            Ok(include_str!("../../../dcl_js/src/modules/AdaptationLayerHelper.js").to_owned())
        }
        _ => Err(generic_error(format!("invalid module request `{module_spec}`")).into()),
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
    mod invalid_op_input {
        include!("invalid_op_input_tests.rs");
    }
    mod websocket {
        include!("websocket_tests.rs");
    }
    use std::sync::Arc;

    use common::structs::SceneJsFile;
    use dcl_types::{interface::crdt_context::CrdtContext, SceneId};
    use deno_core::ascii_str;

    use deno_core::JsRuntime;

    use super::InspectorServer;

    /// `super::create_runtime` inside a tokio runtime, which V8's delayed tasks need.
    fn create_runtime(
        inspect: bool,
        super_user: bool,
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
        super::create_runtime(inspect, super_user, kill_flag)
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
        let (mut runtime, _) = create_runtime(false, false, Default::default());
        {
            let state = runtime.op_state();
            let mut state = state.borrow_mut();
            state.put(context(is_server));
            state.put(SceneJsFile(Arc::new(scene_js.to_owned())));
        }
        runtime
            .execute_script("<test>", ascii_str!("require(\"~scene.js\")"))
            .map(|_| ())
            .map_err(|e| e.to_string())
    }

    #[test]
    fn signed_fetch_scene_module_queues_only_rust_computed_body_hashes() {
        let (mut runtime, _) = create_runtime(false, false, Default::default());
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

    /// Scenes get no persistent browser storage, on the client or the server.
    #[test]
    fn scene_has_no_persistent_storage() {
        for is_server in [false, true] {
            run_scene(
                is_server,
                r#"
                for (const name of ["localStorage", "sessionStorage", "indexedDB", "caches"]) {
                    if (typeof globalThis[name] !== "undefined") { throw new Error(`${name} is reachable`); }
                }
                if (typeof navigator !== "undefined" && typeof navigator.storage !== "undefined") {
                    throw new Error("navigator.storage is reachable");
                }
                "#,
            )
            .unwrap();
        }
    }

    /// Scenes cannot create HTTP clients, so a client rid passed to `op_fetch` is refused.
    #[test]
    fn op_fetch_refuses_a_client_rid() {
        for is_server in [false, true] {
            let (mut runtime, _) = create_runtime(false, false, Default::default());
            runtime.op_state().borrow_mut().put(context(is_server));
            runtime
                .op_state()
                .borrow_mut()
                .put(dcl_types::SceneResourceCounters::default());
            let err = runtime
                .execute_script(
                    "<test>",
                    ascii_str!(
                        r#"Deno.core.ops.op_fetch("GET", "https://example.org/", [], 0, false, null, null)"#
                    ),
                )
                .expect_err("a client rid must be refused")
                .to_string();
            assert!(
                err.contains("custom HTTP clients"),
                "unexpected error: {err}"
            );
            let removed = runtime
                .execute_script(
                    "<test>",
                    ascii_str!("typeof Deno.core.ops.op_fetch_custom_client"),
                )
                .unwrap();
            deno_core::scope!(scope, runtime);
            assert_eq!(
                deno_core::v8::Local::new(scope, removed).to_rust_string_lossy(scope),
                "undefined"
            );
        }
    }

    /// Drive one scene `fetch` against a local server that answers `/redirect` with a 302 to
    /// `/target` on a second listener, and `/target` with a 200. Returns `"<status> <body>"`.
    /// Loopback needs `preview`; the fetch permission request that `op_fetch_send` parks on is
    /// granted from a plain thread, as the renderer would (`RpcResultSender::send` blocks).
    fn fetch_redirect_outcome(is_server: bool) -> String {
        use std::io::{Read, Write};
        use std::net::TcpListener;
        use std::sync::mpsc;

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

        let (mut runtime, _) = create_runtime(false, false, Default::default());
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

        let js = format!(
            r#"globalThis.__outcome = fetch("http://127.0.0.1:{}/redirect")
                .then(async (r) => `${{r.status}} ${{await r.text()}}`, (e) => `error ${{e}}`)
                .then((s) => globalThis.__outcome = s);"#,
            ports[0]
        );
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(async {
            runtime.execute_script("<test>", js).unwrap();
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

    /// The server never follows a redirect: the scene gets the 3xx back.
    #[test]
    fn server_hands_redirects_back_to_the_scene() {
        let outcome = fetch_redirect_outcome(true);
        assert!(outcome.starts_with("302 "), "unexpected outcome: {outcome}");
    }

    /// The client follows, like the browser does; each hop goes back through `op_fetch`.
    #[test]
    fn client_follows_redirects() {
        assert_eq!(fetch_redirect_outcome(false), "200 arrived");
    }
}
