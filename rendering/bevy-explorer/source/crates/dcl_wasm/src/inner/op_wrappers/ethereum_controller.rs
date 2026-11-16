use crate::{serde_result, WasmError, WorkerContext};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub async fn op_send_async(
    state: &WorkerContext,
    method: String,
    params: String,
) -> Result<JsValue, WasmError> {
    serde_result!(dcl_js::ethereum_controller::op_send_async(state.rc(), method, params).await)
}

#[wasm_bindgen]
pub async fn op_get_user_account(state: &WorkerContext) -> Result<JsValue, WasmError> {
    serde_result!(dcl_js::ethereum_controller::op_get_user_account(state.rc()).await)
}

#[wasm_bindgen]
pub async fn op_sign_legacy_message(
    state: &WorkerContext,
    message: String,
) -> Result<JsValue, WasmError> {
    serde_result!(dcl_js::ethereum_controller::op_sign_legacy_message(state.rc(), message).await)
}
