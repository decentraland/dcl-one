use crate::{serde_result, WasmError, WorkerContext};
use dcl_types::RpcCalls;
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
            if !body.is_string() {
                anyhow::bail!("signed fetch body must be a string");
            }
            // UTF-16 code units are a lower bound on the UTF-8 byte count. Reject
            // before copying from JS when even that lower bound exceeds the budget.
            let len = js_sys::JsString::from(body.clone()).length() as usize;
            let state_rc = state.rc();
            let remaining = state_rc
                .borrow()
                .borrow::<RpcCalls>()
                .remaining_signing_body_bytes();
            if len > remaining {
                anyhow::bail!("per-tick signed fetch body limit exceeded");
            }
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
