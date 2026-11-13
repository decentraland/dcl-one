use crate::js::errors::OpError;
use deno_core::{op2, JsBuffer, OpDecl, OpState};
use std::{cell::RefCell, rc::Rc};

// list of op declarations
pub fn ops() -> Vec<OpDecl> {
    vec![
        op_comms_send_string(),
        op_comms_send_binary_single(),
        op_comms_recv_binary(),
    ]
}

#[op2]
async fn op_comms_send_string(
    state: Rc<RefCell<OpState>>,
    #[string] message: String,
) -> Result<(), OpError> {
    Ok(dcl_js::comms::op_comms_send_string(state, message).await?)
}

#[op2]
async fn op_comms_send_binary_single(
    state: Rc<RefCell<OpState>>,
    #[buffer(detach)] message: JsBuffer,
    #[string] recipient: Option<String>,
) -> Result<(), OpError> {
    Ok(dcl_js::comms::op_comms_send_binary_single(state, message, recipient).await?)
}

#[op2]
#[serde]
async fn op_comms_recv_binary(state: Rc<RefCell<OpState>>) -> Result<Vec<Vec<u8>>, OpError> {
    Ok(dcl_js::comms::op_comms_recv_binary(state).await?)
}
