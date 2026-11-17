use std::{cell::RefCell, rc::Rc};

use dcl_component::proto_components::kernel::apis::VideoTracksActiveStreamsResponse;
use dcl_js::comms_api::TopicMessage;
use deno_core::{anyhow, op2, OpDecl, OpState};

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

#[op2(async)]
#[serde]
async fn op_get_active_video_streams(
    state: Rc<RefCell<OpState>>,
) -> Result<VideoTracksActiveStreamsResponse, anyhow::Error> {
    dcl_js::comms_api::op_get_active_video_streams(state).await
}

#[op2(async)]
async fn op_comms_subscribe_topic(
    state: Rc<RefCell<OpState>>,
    #[string] topic: Option<String>,
) -> Result<(), anyhow::Error> {
    dcl_js::comms_api::op_comms_subscribe_topic(state, topic).await
}

#[op2(async)]
async fn op_comms_unsubscribe_topic(
    state: Rc<RefCell<OpState>>,
    #[string] topic: Option<String>,
) -> Result<(), anyhow::Error> {
    dcl_js::comms_api::op_comms_unsubscribe_topic(state, topic).await
}

#[op2(async)]
async fn op_comms_publish_data(
    state: Rc<RefCell<OpState>>,
    #[string] topic: Option<String>,
    #[string] data: Option<String>,
) {
    dcl_js::comms_api::op_comms_publish_data(state, topic, data).await
}

#[op2(async)]
#[serde]
async fn op_comms_consume_messages(
    state: Rc<RefCell<OpState>>,
    #[string] topic: Option<String>,
) -> Vec<TopicMessage> {
    dcl_js::comms_api::op_comms_consume_messages(state, topic).await
}
