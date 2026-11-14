use deno_core::{error::AnyError, op2, OpDecl, OpState};
use std::{cell::RefCell, rc::Rc};

// list of op declarations
pub fn ops() -> Vec<OpDecl> {
    vec![op_send_async(), op_get_user_account()]
}

#[op2(async)]
#[serde]
async fn op_send_async(
    state: Rc<RefCell<OpState>>,
    #[string] method: String,
    #[string] params: String,
) -> Result<serde_json::Value, AnyError> {
    dcl_js::ethereum_controller::op_send_async(state, method, params).await
}

#[op2(async)]
#[string]
async fn op_get_user_account(state: Rc<RefCell<OpState>>) -> Result<Option<String>, AnyError> {
    dcl_js::ethereum_controller::op_get_user_account(state).await
}
