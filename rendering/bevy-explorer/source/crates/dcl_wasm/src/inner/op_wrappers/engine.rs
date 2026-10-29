use crate::inner::SceneStatsFlush;
use crate::WorkerContext;
use dcl_js::KillFlag;
use dcl_types::{SceneElapsedTime, SceneResourceCounters, SceneResponse, SceneResponseSender};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn op_log(state: &WorkerContext, message: String) {
    dcl_js::op_log(state.rc(), message);
}

#[wasm_bindgen]
pub fn op_error(state: &WorkerContext, message: String) {
    dcl_js::op_error(state.rc(), message);
}

#[wasm_bindgen]
pub fn op_is_server(op_state: &WorkerContext) -> bool {
    dcl_js::engine::op_is_server(op_state.rc())
}

#[wasm_bindgen]
pub fn op_crdt_send_to_renderer(op_state: &WorkerContext, messages: js_sys::ArrayBuffer) {
    let view = js_sys::Uint8Array::new(&messages);
    let state = op_state.rc();
    dcl_js::engine::crdt_send_to_renderer(state.clone(), &view.to_vec());

    let mut state = state.borrow_mut();
    if state.borrow::<KillFlag>().killed() {
        return;
    }
    let elapsed = state.borrow::<SceneElapsedTime>().0;
    if elapsed - state.borrow::<SceneStatsFlush>().0 >= 5.0 {
        state.put(SceneStatsFlush(elapsed));
        let snapshot = state.borrow::<SceneResourceCounters>().clone();
        // Advisory telemetry must not block a scene on a full response channel.
        let _ = state
            .borrow::<SceneResponseSender>()
            .try_send(SceneResponse::Stats(snapshot));
    }
}

#[wasm_bindgen]
pub async fn op_crdt_recv_from_renderer(op_state: &WorkerContext) -> js_sys::Array {
    let data = dcl_js::engine::op_crdt_recv_from_renderer(op_state.rc()).await;

    data.into_iter()
        .map(|v| v.into_boxed_slice())
        .map(JsValue::from)
        .collect()
}
