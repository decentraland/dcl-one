use std::{cell::RefCell, rc::Rc};

use dcl_component::proto_components::kernel::apis::{
    ConsumeMessagesResponse, VideoTracksActiveStreamsResponse,
};
use deno_core::{anyhow, op2, OpDecl, OpState};

// list of op declarations
pub fn ops() -> Vec<OpDecl> {
    vec![
        op_get_active_video_streams(),
        op_set_topic_subscription(),
        op_publish_topic_data(),
        op_consume_topic_messages(),
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
async fn op_set_topic_subscription(
    state: Rc<RefCell<OpState>>,
    #[string] topic: String,
    subscribed: bool,
) -> Result<(), anyhow::Error> {
    dcl_js::comms_api::op_set_topic_subscription(state, topic, subscribed).await
}

#[op2(async)]
async fn op_publish_topic_data(
    state: Rc<RefCell<OpState>>,
    #[string] topic: String,
    #[string] data: String,
) -> Result<(), anyhow::Error> {
    dcl_js::comms_api::op_publish_topic_data(state, topic, data).await
}

#[op2(async)]
#[serde]
async fn op_consume_topic_messages(
    state: Rc<RefCell<OpState>>,
    #[string] topic: String,
) -> Result<ConsumeMessagesResponse, anyhow::Error> {
    dcl_js::comms_api::op_consume_topic_messages(state, topic).await
}
