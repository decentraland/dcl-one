use crate::js::errors::OpError;
use crate::js::plain::JsPlain;
use std::{cell::RefCell, rc::Rc};

use dcl_js::user_identity::UserData;
use deno_core::{op2, OpDecl, OpState};

// list of op declarations
pub fn ops() -> Vec<OpDecl> {
    vec![op_get_user_data(), op_get_player_data()]
}
pub struct UserEthAddress(pub String);

#[op2]
async fn op_get_user_data(state: Rc<RefCell<OpState>>) -> Result<JsPlain<UserData>, OpError> {
    Ok(JsPlain(
        dcl_js::user_identity::op_get_user_data(state).await?,
    ))
}

#[op2]
async fn op_get_player_data(
    state: Rc<RefCell<OpState>>,
    #[string] id: String,
) -> Result<JsPlain<UserData>, OpError> {
    Ok(JsPlain(
        dcl_js::user_identity::op_get_player_data(state, id).await?,
    ))
}
