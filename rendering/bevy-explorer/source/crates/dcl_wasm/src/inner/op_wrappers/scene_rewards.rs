use crate::{serde_result, WasmError, WorkerContext};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn op_grant_reward(
    state: &WorkerContext,
    program_id: String,
    recipient: String,
    outcome_id: String,
) -> js_sys::Promise {
    match dcl_js::scene_rewards::start_grant_reward(state.rc(), program_id, recipient, outcome_id) {
        Ok(rx) => wasm_bindgen_futures::future_to_promise(async move {
            serde_result!(Ok::<_, anyhow::Error>(
                dcl_js::scene_rewards::finish_grant_reward(rx).await
            ))
            .map_err(Into::into)
        }),
        Err(error) => js_sys::Promise::reject(&JsValue::from(WasmError::from(error))),
    }
}
