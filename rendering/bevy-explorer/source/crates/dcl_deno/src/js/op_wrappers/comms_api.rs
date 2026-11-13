use crate::js::errors::OpError;
use crate::js::plain::JsPlain;
use std::{cell::RefCell, rc::Rc};

use dcl_component::proto_components::kernel::apis::VideoTracksActiveStreamsResponse;
use dcl_js::comms_api::TopicMessage;
use deno_core::{op2, OpDecl, OpState};

// list of op declarations
pub fn ops() -> Vec<OpDecl> {
    vec![
        op_get_active_video_streams(),
        op_comms_subscribe_topic(),
        op_comms_unsubscribe_topic(),
        op_comms_publish_data(),
        op_comms_consume_messages(),
    ]
}

#[op2]
async fn op_get_active_video_streams(
    state: Rc<RefCell<OpState>>,
) -> Result<JsPlain<VideoTracksActiveStreamsResponse>, OpError> {
    Ok(JsPlain(
        dcl_js::comms_api::op_get_active_video_streams(state).await?,
    ))
}

#[op2]
async fn op_comms_subscribe_topic(
    state: Rc<RefCell<OpState>>,
    #[string] topic: Option<String>,
) -> Result<(), OpError> {
    Ok(dcl_js::comms_api::op_comms_subscribe_topic(state, topic).await?)
}

#[op2]
async fn op_comms_unsubscribe_topic(
    state: Rc<RefCell<OpState>>,
    #[string] topic: Option<String>,
) -> Result<(), OpError> {
    Ok(dcl_js::comms_api::op_comms_unsubscribe_topic(state, topic).await?)
}

#[op2]
async fn op_comms_publish_data(
    state: Rc<RefCell<OpState>>,
    #[string] topic: Option<String>,
    #[string] data: Option<String>,
) {
    dcl_js::comms_api::op_comms_publish_data(state, topic, data).await
}

#[op2]
async fn op_comms_consume_messages(
    state: Rc<RefCell<OpState>>,
    #[string] topic: Option<String>,
) -> JsPlain<Vec<TopicMessage>> {
    JsPlain(dcl_js::comms_api::op_comms_consume_messages(state, topic).await)
}
