use std::{cell::RefCell, rc::Rc};

use bevy::log::debug;
use common::rpc::{RpcCall, RpcResultSender};
use dcl_component::proto_components::kernel::apis::{
    ConsumeMessages, ConsumeMessagesResponse, VideoTracksActiveStreamsResponse,
};
use dcl_types::{
    scene_topic::{encode_topic, valid_topic},
    RpcCalls, SceneResourceCounters, SceneResponse, SceneResponseSender,
};

use crate::{comms::try_spend_budget, State};

pub async fn op_get_active_video_streams(
    state: Rc<RefCell<impl State>>,
) -> Result<VideoTracksActiveStreamsResponse, anyhow::Error> {
    debug!("op_get_active_video_streams");
    let (sx, rx) = RpcResultSender::channel();

    {
        let mut state = state.borrow_mut();

        state
            .borrow_mut::<RpcCalls>()
            .push(RpcCall::ActiveVideoStreams { response: sx })?;
    }

    Ok(rx.await.unwrap_or_default())
}

pub async fn op_set_topic_subscription(
    state: Rc<RefCell<impl State>>,
    topic: String,
    subscribed: bool,
) -> Result<(), anyhow::Error> {
    if !valid_topic(&topic) {
        return Ok(());
    }
    let (response, rx) = RpcResultSender::channel();
    send_topic_request(
        &*state.borrow(),
        RpcCall::SetTopicSubscription {
            topic,
            subscribed,
            response,
        },
    )?;
    rx.await?;
    Ok(())
}

pub async fn op_publish_topic_data(
    state: Rc<RefCell<impl State>>,
    topic: String,
    data: String,
) -> Result<(), anyhow::Error> {
    let Some(frame) = encode_topic(&topic, &data) else {
        return Ok(());
    };

    let mut state = state.borrow_mut();
    if !try_spend_budget(&mut *state) {
        return Ok(());
    }

    let payload_bytes = frame.len() - 1;
    state
        .borrow_mut::<RpcCalls>()
        .push(RpcCall::SendMessageBus {
            data: frame,
            recipient: None,
        })?;

    let counters = state.borrow_mut::<SceneResourceCounters>();
    counters.comms_msgs_out += 1;
    counters.comms_bytes_out += payload_bytes as u64;
    Ok(())
}

pub async fn op_consume_topic_messages(
    state: Rc<RefCell<impl State>>,
    topic: String,
) -> Result<ConsumeMessagesResponse, anyhow::Error> {
    if !valid_topic(&topic) {
        return Ok(ConsumeMessagesResponse::default());
    }
    let (response, rx) = RpcResultSender::channel();
    send_topic_request(
        &*state.borrow(),
        RpcCall::ConsumeTopicMessages { topic, response },
    )?;

    let messages: Vec<_> = rx
        .await?
        .into_iter()
        .map(|(sender, data)| ConsumeMessages { sender, data })
        .collect();
    let mut state = state.borrow_mut();
    let counters = state.borrow_mut::<SceneResourceCounters>();
    counters.comms_msgs_in += messages.len() as u64;
    counters.comms_bytes_in += messages
        .iter()
        .map(|msg| msg.data.len() as u64)
        .sum::<u64>();
    Ok(ConsumeMessagesResponse { messages })
}

fn send_topic_request(state: &impl State, call: RpcCall) -> Result<(), anyhow::Error> {
    state
        .borrow::<SceneResponseSender>()
        .try_send(SceneResponse::ImmediateRpcCall(call))
        .map_err(|_| anyhow::anyhow!("scene topic request could not reach the engine"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        comms::{CommsSendBudget, MAX_SEND_MESSAGES_PER_TICK},
        test_state::TestState,
    };
    use dcl_types::{scene_response_channel, SceneId};

    #[test]
    fn topic_publishes_share_the_scene_message_budget() {
        let mut state = TestState::default();
        state.put(RpcCalls::default());
        state.put(SceneResourceCounters::default());
        state.put(CommsSendBudget {
            sent: MAX_SEND_MESSAGES_PER_TICK - 1,
        });
        let state = Rc::new(RefCell::new(state));

        for data in ["first", "second"] {
            futures_lite::future::block_on(op_publish_topic_data(
                state.clone(),
                "updates".into(),
                data.into(),
            ))
            .unwrap();
        }
        let state = state.borrow();
        let calls = state.borrow::<RpcCalls>();
        assert!(matches!(
            calls.as_slice(),
            [RpcCall::SendMessageBus { data, recipient: None }]
                if data.as_slice() == b"\x03\x07\x00updatesfirst"
        ));
        assert_eq!(state.borrow::<SceneResourceCounters>().comms_msgs_out, 1);
    }

    #[test]
    fn consuming_a_topic_returns_engine_messages_without_a_tick_flush() {
        let mut state = TestState::default();
        let (tx, mut rx) = scene_response_channel();
        state.put(SceneResponseSender::new(SceneId::DUMMY, tx));
        state.put(SceneResourceCounters::default());
        let state = Rc::new(RefCell::new(state));

        futures_lite::future::block_on(async {
            let mut consume =
                std::pin::pin!(op_consume_topic_messages(state.clone(), "updates".into()));
            assert!(futures_lite::future::poll_once(consume.as_mut())
                .await
                .is_none());
            let (_, call) = rx.try_recv().unwrap();
            let SceneResponse::ImmediateRpcCall(RpcCall::ConsumeTopicMessages { topic, response }) =
                call
            else {
                panic!("expected a topic consume call");
            };
            assert_eq!(topic, "updates");
            response.send(vec![("peer".into(), "ready".into())]);

            let reply = consume.await.unwrap();
            assert_eq!(
                serde_json::to_value(reply).unwrap(),
                serde_json::json!({ "messages": [{ "sender": "peer", "data": "ready" }] })
            );
        });
    }

    #[test]
    fn subscribing_waits_for_the_engine_ack_without_a_tick_flush() {
        let mut state = TestState::default();
        let (tx, mut rx) = scene_response_channel();
        state.put(SceneResponseSender::new(SceneId::DUMMY, tx));
        let state = Rc::new(RefCell::new(state));

        futures_lite::future::block_on(async {
            let mut subscribe =
                std::pin::pin!(op_set_topic_subscription(state, "updates".into(), true));
            assert!(futures_lite::future::poll_once(subscribe.as_mut())
                .await
                .is_none());
            let (_, call) = rx.try_recv().unwrap();
            let SceneResponse::ImmediateRpcCall(RpcCall::SetTopicSubscription {
                topic,
                subscribed: true,
                response,
            }) = call
            else {
                panic!("expected a topic subscription");
            };
            assert_eq!(topic, "updates");
            response.send(());
            subscribe.await.unwrap();
        });
    }
}
