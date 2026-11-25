pub mod js;

use common::sync::MutexExt;
use std::{
    panic::{self, AssertUnwindSafe},
    sync::Mutex,
};

use bevy::{log::error, platform::collections::HashMap};
use common::structs::GlobalCrdtStateUpdate;
use deno_core::v8::IsolateHandle;
use once_cell::sync::Lazy;
use system_bridge::SystemApi;
use tokio::sync::mpsc::UnboundedSender;

use common::structs::SceneJsFile;

use dcl_js::KillFlag;
use dcl_types::{
    interface::{crdt_context::CrdtContext, CrdtComponentInterfaces, CrdtStore},
    RendererResponse, SceneId, SceneResponseSender,
};

use crate::js::scene_thread;

pub(crate) static VM_HANDLES: Lazy<Mutex<HashMap<SceneId, (IsolateHandle, KillFlag)>>> =
    Lazy::new(Default::default);

// when each scene was force-terminated; entries whose thread has exited are pruned on read
static TERMINATED: Lazy<Mutex<HashMap<SceneId, std::time::Instant>>> = Lazy::new(Default::default);

/// interrupt the scene's isolate even if it is stuck in a JS loop, so the scene
/// thread unwinds and exits. no-op if the scene thread has already exited. a
/// scene parked on an async op is released at once by the kill flag; one
/// blocked inside a synchronous rust op runs on until control returns to JS
/// (see [`zombie_scene_count`]). the kill flag must be set as well as terminating:
/// v8's termination request is consumed once the stack unwinds, and the scene
/// loop doesn't exit on uncaught errors, so without the flag a deterministic
/// wedge would just re-enter its loop on the next tick.
pub fn terminate_scene(scene_id: SceneId) {
    let running = match VM_HANDLES.lock_ignore_poison().get(&scene_id) {
        Some((handle, kill_flag)) => {
            bevy::log::warn!(
                "[{scene_id:?}] scene thread still running after kill; force-terminating"
            );
            kill_flag.kill();
            handle.terminate_execution();
            true
        }
        None => false,
    };
    if running {
        TERMINATED
            .lock_ignore_poison()
            .entry(scene_id)
            .or_insert_with(std::time::Instant::now);
    }
}

/// Scene threads still running at least `min_age` after [`terminate_scene`] forced them.
pub fn zombie_scene_count(min_age: std::time::Duration) -> usize {
    let mut terminated = TERMINATED.lock_ignore_poison();
    let running = VM_HANDLES.lock_ignore_poison();
    terminated.retain(|id, _| running.contains_key(id));
    terminated
        .values()
        .filter(|at| at.elapsed() >= min_age)
        .count()
}

/// must be called from main thread on linux before any isolates are created
pub fn init_runtime() {
    let _ = deno_core::v8::Platform::new(1, false);
}

/// The test harness runs each test on its own thread. V8's default platform lets an isolate run
/// only on the thread that started V8 and on that thread's children, so on a host with memory
/// protection keys the second test that builds a runtime stops the process. The tests start V8
/// with the unprotected platform, which has no such rule.
#[cfg(test)]
pub(crate) fn test_platform() {
    static INIT: std::sync::Once = std::sync::Once::new();
    INIT.call_once(|| {
        deno_core::JsRuntime::init_platform(
            Some(deno_core::v8::new_unprotected_default_platform(0, false).make_shared()),
            false,
        );
    });
}

pub fn spawn_scene(
    initial_crdt_store: CrdtStore,
    scene_context: CrdtContext,
    scene_js: SceneJsFile,
    crdt_component_interfaces: CrdtComponentInterfaces,
    renderer_sender: SceneResponseSender,
    global_update_receiver: tokio::sync::broadcast::Receiver<GlobalCrdtStateUpdate>,
    storage_root: String,
    inspect: bool,
    super_user: Option<tokio::sync::mpsc::UnboundedSender<SystemApi>>,
    scene_origin: bevy::prelude::Vec3,
) -> UnboundedSender<RendererResponse> {
    let id = scene_context.scene_id;
    let (main_sx, thread_rx) = tokio::sync::mpsc::unbounded_channel::<RendererResponse>();

    std::thread::Builder::new()
        .name(format!("scene thread {:?}", id.0))
        .stack_size(8388608)
        .spawn(move || {
            let thread_result = panic::catch_unwind(AssertUnwindSafe(|| {
                scene_thread(
                    initial_crdt_store,
                    scene_context,
                    storage_root,
                    scene_js,
                    crdt_component_interfaces,
                    renderer_sender,
                    thread_rx,
                    global_update_receiver,
                    inspect,
                    super_user,
                    scene_origin,
                )
            }));

            if let Err(e) = thread_result {
                error!("[{id:?}] caught scene thread panic: {e:?}");
            }

            VM_HANDLES.lock_ignore_poison().remove(&id);
        })
        .unwrap();

    main_sx
}
