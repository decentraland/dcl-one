use crate::js::errors::OpError;
use crate::js::plain::JsPlain;
use deno_core::{op2, OpDecl, OpState};
use std::{cell::RefCell, rc::Rc};

// list of op declarations
pub fn ops() -> Vec<OpDecl> {
    vec![op_send_async()]
}

#[op2]
async fn op_send_async(
    state: Rc<RefCell<OpState>>,
    #[string] method: String,
    #[string] params: String,
) -> Result<JsPlain<serde_json::Value>, OpError> {
    Ok(JsPlain(
        dcl_js::ethereum_controller::op_send_async(state, method, params).await?,
    ))
}
