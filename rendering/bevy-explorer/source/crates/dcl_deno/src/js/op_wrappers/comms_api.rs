use crate::js::errors::OpError;
use crate::js::plain::JsPlain;
use std::{cell::RefCell, rc::Rc};

use dcl_component::proto_components::kernel::apis::VideoTracksActiveStreamsResponse;
use deno_core::{op2, OpDecl, OpState};

// list of op declarations
pub fn ops() -> Vec<OpDecl> {
    vec![op_get_active_video_streams()]
}

#[op2]
async fn op_get_active_video_streams(
    state: Rc<RefCell<OpState>>,
) -> Result<JsPlain<VideoTracksActiveStreamsResponse>, OpError> {
    Ok(JsPlain(
        dcl_js::comms_api::op_get_active_video_streams(state).await?,
    ))
}
