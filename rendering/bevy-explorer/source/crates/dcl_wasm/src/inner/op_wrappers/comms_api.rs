use wasm_bindgen::{prelude::wasm_bindgen, JsValue};

use crate::{serde_result, WasmError, WorkerContext};

#[wasm_bindgen]
pub async fn op_get_active_video_streams(state: &WorkerContext) -> Result<JsValue, WasmError> {
    let data = dcl_js::comms_api::op_get_active_video_streams(state.rc()).await;
    serde_result!(data)
}

#[wasm_bindgen]
pub async fn op_set_topic_subscription(
    state: &WorkerContext,
    topic: String,
    subscribed: bool,
) -> Result<(), WasmError> {
    dcl_js::comms_api::op_set_topic_subscription(state.rc(), topic, subscribed)
        .await
        .map_err(WasmError::from)
}

#[wasm_bindgen]
pub async fn op_publish_topic_data(
    state: &WorkerContext,
    topic: String,
    data: String,
) -> Result<(), WasmError> {
    dcl_js::comms_api::op_publish_topic_data(state.rc(), topic, data)
        .await
        .map_err(WasmError::from)
}

#[wasm_bindgen]
pub async fn op_consume_topic_messages(
    state: &WorkerContext,
    topic: String,
) -> Result<JsValue, WasmError> {
    let data = dcl_js::comms_api::op_consume_topic_messages(state.rc(), topic).await;
    serde_result!(data)
}
