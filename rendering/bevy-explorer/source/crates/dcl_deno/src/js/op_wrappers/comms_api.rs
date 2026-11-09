use crate::js::errors::OpError;
use std::{cell::RefCell, rc::Rc};

use dcl_component::proto_components::kernel::apis::VideoTracksActiveStreamsResponse;
use deno_core::{op2, OpDecl, OpState};

// list of op declarations
pub fn ops() -> Vec<OpDecl> {
    vec![op_get_active_video_streams()]
}

#[op2]
#[serde]
async fn op_get_active_video_streams(
    state: Rc<RefCell<OpState>>,
) -> Result<VideoTracksActiveStreamsResponse, OpError> {
    Ok(dcl_js::comms_api::op_get_active_video_streams(state).await?)
}
