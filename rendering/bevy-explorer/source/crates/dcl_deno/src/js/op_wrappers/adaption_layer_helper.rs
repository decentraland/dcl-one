use crate::js::errors::OpError;
use crate::js::plain::JsPlain;
use dcl_js::adaption_layer_helper::TextureSize;
use deno_core::{op2, OpDecl, OpState};
use std::{cell::RefCell, rc::Rc};

// list of op declarations
pub fn ops() -> Vec<OpDecl> {
    vec![op_get_texture_size()]
}

#[op2]
async fn op_get_texture_size(
    state: Rc<RefCell<OpState>>,
    #[string] src: String,
) -> Result<JsPlain<TextureSize>, OpError> {
    Ok(JsPlain(
        dcl_js::adaption_layer_helper::op_get_texture_size(state, src).await?,
    ))
}
