use crate::{serde_result, WasmError, WorkerContext};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn op_signed_fetch_headers(
    state: &WorkerContext,
    uri: String,
    method: Option<String>,
    body: JsValue,
) -> js_sys::Promise {
    let result = (|| {
        let body = if body.is_null() || body.is_undefined() {
            None
        } else {
            Some(
                body.as_string()
                    .ok_or_else(|| anyhow::anyhow!("signed fetch body must be a string"))?,
            )
        };
        dcl_js::fetch::start_signed_fetch_headers(state.rc(), uri, method, body)
    })();
    match result {
        Ok(rx) => wasm_bindgen_futures::future_to_promise(async move {
            serde_result!(dcl_js::fetch::finish_signed_fetch_headers(rx).await).map_err(Into::into)
        }),
        Err(error) => js_sys::Promise::reject(&JsValue::from(WasmError::from(error))),
    }
}
