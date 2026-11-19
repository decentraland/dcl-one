use crate::{to_js_array, WasmError, WorkerContext};
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
pub fn op_send_batch(state: &WorkerContext) -> Result<js_sys::Array, WasmError> {
    to_js_array(dcl_js::events::op_send_batch(
        &mut *state.state.borrow_mut(),
    ))
}
