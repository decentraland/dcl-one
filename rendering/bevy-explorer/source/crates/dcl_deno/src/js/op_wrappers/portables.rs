use crate::js::errors::OpError;
use crate::js::plain::JsPlain;
use common::rpc::SpawnResponse;
use deno_core::{op2, OpDecl, OpState};
use std::{cell::RefCell, rc::Rc};

// list of op declarations
pub fn ops() -> Vec<OpDecl> {
    vec![op_portable_spawn(), op_portable_list(), op_portable_kill()]
}

#[op2]
async fn op_portable_spawn(
    state: Rc<RefCell<OpState>>,
    #[string] pid: Option<String>,
    #[string] ens: Option<String>,
) -> Result<JsPlain<SpawnResponse>, OpError> {
    Ok(JsPlain(
        dcl_js::portables::op_portable_spawn(state, pid, ens).await?,
    ))
}

#[op2]
async fn op_portable_kill(
    state: Rc<RefCell<OpState>>,
    #[string] pid: String,
) -> Result<bool, OpError> {
    Ok(dcl_js::portables::op_portable_kill(state, pid).await?)
}

#[op2]
async fn op_portable_list(
    state: Rc<RefCell<OpState>>,
) -> Result<JsPlain<Vec<SpawnResponse>>, OpError> {
    Ok(JsPlain(dcl_js::portables::op_portable_list(state).await?))
}
