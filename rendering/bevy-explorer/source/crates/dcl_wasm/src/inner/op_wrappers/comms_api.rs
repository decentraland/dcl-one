use wasm_bindgen::{prelude::wasm_bindgen, JsValue};

use crate::{serde_result, WasmError, WorkerContext};

#[wasm_bindgen]
pub async fn op_get_active_video_streams(state: &WorkerContext) -> Result<JsValue, WasmError> {
    let data = dcl_js::comms_api::op_get_active_video_streams(state.rc()).await;
    serde_result!(data)
}

#[wasm_bindgen]
pub async fn op_comms_subscribe_topic(
    state: &WorkerContext,
    topic: Option<String>,
) -> Result<(), WasmError> {
    dcl_js::comms_api::op_comms_subscribe_topic(state.rc(), topic)
        .await
        .map_err(WasmError::from)
}

#[wasm_bindgen]
pub async fn op_comms_unsubscribe_topic(
    state: &WorkerContext,
    topic: Option<String>,
) -> Result<(), WasmError> {
    dcl_js::comms_api::op_comms_unsubscribe_topic(state.rc(), topic)
        .await
        .map_err(WasmError::from)
}

#[wasm_bindgen]
pub async fn op_comms_publish_data(
    state: &WorkerContext,
    topic: Option<String>,
    data: Option<String>,
) {
    dcl_js::comms_api::op_comms_publish_data(state.rc(), topic, data).await
}

#[wasm_bindgen]
pub async fn op_comms_consume_messages(
    state: &WorkerContext,
    topic: Option<String>,
) -> Result<JsValue, WasmError> {
    let data = dcl_js::comms_api::op_comms_consume_messages(state.rc(), topic).await;
    serde_result!(Ok::<_, anyhow::Error>(data))
}
