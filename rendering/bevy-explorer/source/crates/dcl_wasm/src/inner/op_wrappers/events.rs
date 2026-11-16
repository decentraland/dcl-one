use crate::{serde_result, WasmError, WorkerContext};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn op_subscribe(state: &WorkerContext, id: &str) -> Result<(), WasmError> {
    dcl_js::events::op_subscribe(&mut *state.state.borrow_mut(), id).map_err(WasmError::from)
}

#[wasm_bindgen]
pub fn op_unsubscribe(state: &WorkerContext, id: &str) {
    dcl_js::events::op_unsubscribe(&mut *state.state.borrow_mut(), id)
}

#[wasm_bindgen]
pub fn op_send_batch(state: &WorkerContext) -> js_sys::Array {
    dcl_js::events::op_send_batch(&mut *state.state.borrow_mut())
        .into_iter()
        .map(|ev| serde_wasm_bindgen::to_value(&ev).unwrap())
        .collect()
}

#[wasm_bindgen]
pub fn op_subscribe_legacy_avatar_events(state: &WorkerContext) -> Result<(), WasmError> {
    dcl_js::events::op_subscribe_legacy_avatar_events(&mut *state.state.borrow_mut())
        .map_err(WasmError::from)
}

#[wasm_bindgen]
pub async fn op_pull_legacy_avatar_events(state: &WorkerContext) -> Result<JsValue, WasmError> {
    serde_result!(dcl_js::events::op_pull_legacy_avatar_events(state.rc()).await)
}
