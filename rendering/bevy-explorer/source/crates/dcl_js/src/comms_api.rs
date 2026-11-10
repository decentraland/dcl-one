use std::{cell::RefCell, collections::HashMap, collections::VecDeque, rc::Rc, time::Duration};

use bevy::{log::debug, platform::time::Instant};
use common::rpc::{RpcCall, RpcResultSender, RpcStreamReceiver, RpcStreamSender};
use dcl_component::proto_components::kernel::apis::VideoTracksActiveStreamsResponse;
use serde::Serialize;

use crate::State;
use dcl_types::{CommsMessageType, RpcCalls, SceneResourceCounters};

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

pub const MAX_TOPIC_MESSAGES_PER_WINDOW: u32 = 10;
pub const TOPIC_RATE_WINDOW: Duration = Duration::from_millis(1000);
/// Longest topic, in UTF-8 bytes.
pub const MAX_TOPIC_BYTES: usize = 512;
/// Includes the type byte and the topic length prefix.
pub const MAX_TOPIC_MESSAGE_BYTES: usize = 13 * 1024;
/// Per subscribed topic; past it the oldest message is dropped.
pub const TOPIC_QUEUE_CAPACITY: usize = 1024;
/// Drained every tick, so it only has to hold one tick's traffic.
const TOPIC_INBOX_CAPACITY: usize = 4096;
const TOPIC_LENGTH_PREFIX_BYTES: usize = 2;

#[derive(Serialize, Debug, PartialEq, Eq)]
pub struct TopicMessage {
    pub sender: String,
    pub data: String,
}

/// Lives in the scene's op state, so a reloaded scene starts with no subscriptions.
#[derive(Default)]
pub struct SceneTopics {
    queues: HashMap<String, VecDeque<TopicMessage>>,
    rate_windows: HashMap<String, (u32, Instant)>,
    inbox: Option<RpcStreamReceiver<(String, Vec<u8>)>>,
}

impl SceneTopics {
    pub fn subscribe(&mut self, topic: String) {
        self.queues.entry(topic).or_default();
    }

    pub fn unsubscribe(&mut self, topic: &str) {
        self.queues.remove(topic);
    }

    pub fn consume(&mut self, topic: &str) -> Vec<TopicMessage> {
        self.queues
            .get_mut(topic)
            .map(|queue| queue.drain(..).collect())
            .unwrap_or_default()
    }

    /// A publish that reaches the rate check spends the topic's budget even if its size then
    /// drops it.
    pub fn frame_publish(&mut self, topic: &str, data: &str, now: Instant) -> Option<Vec<u8>> {
        if data.is_empty() || !self.spend_rate(topic, now) || topic.len() > MAX_TOPIC_BYTES {
            return None;
        }
        let len = 1 + TOPIC_LENGTH_PREFIX_BYTES + topic.len() + data.len();
        if len > MAX_TOPIC_MESSAGE_BYTES {
            return None;
        }
        let mut framed = Vec::with_capacity(len);
        framed.push(CommsMessageType::CommsData as u8);
        framed.extend((topic.len() as u16).to_le_bytes());
        framed.extend(topic.as_bytes());
        framed.extend(data.as_bytes());
        Some(framed)
    }

    /// A fixed window, opened by the first publish after the previous one ended.
    fn spend_rate(&mut self, topic: &str, now: Instant) -> bool {
        match self.rate_windows.get_mut(topic) {
            Some((count, start)) if now.saturating_duration_since(*start) < TOPIC_RATE_WINDOW => {
                if *count >= MAX_TOPIC_MESSAGES_PER_WINDOW {
                    return false;
                }
                *count += 1;
            }
            _ => {
                self.rate_windows.insert(topic.to_owned(), (1, now));
            }
        }
        true
    }

    /// `payload` is `[u16 LE topic length][topic][data]`.
    pub fn receive(&mut self, sender: String, payload: &[u8]) -> bool {
        let Some(len_bytes) = payload.get(..TOPIC_LENGTH_PREFIX_BYTES) else {
            return false;
        };
        let topic_len = u16::from_le_bytes([len_bytes[0], len_bytes[1]]) as usize;
        let Some(topic) =
            payload.get(TOPIC_LENGTH_PREFIX_BYTES..TOPIC_LENGTH_PREFIX_BYTES + topic_len)
        else {
            return false;
        };
        // Subscribed topics are valid UTF-8, so a topic that is not cannot match one.
        let Some(queue) = std::str::from_utf8(topic)
            .ok()
            .and_then(|topic| self.queues.get_mut(topic))
        else {
            return false;
        };
        if queue.len() >= TOPIC_QUEUE_CAPACITY {
            queue.pop_front();
        }
        let data = &payload[TOPIC_LENGTH_PREFIX_BYTES + topic_len..];
        queue.push_back(TopicMessage {
            sender,
            data: String::from_utf8_lossy(data).into_owned(),
        });
        true
    }
}

/// Runs before every topic op and once per tick, so a message is matched against the
/// subscriptions of the moment it arrived.
pub fn drain_topic_inbox(state: &mut impl State) {
    let Some(topics) = state.try_borrow_mut::<SceneTopics>() else {
        return;
    };
    let Some(inbox) = topics.inbox.as_mut() else {
        return;
    };
    let mut received = Vec::new();
    while let Ok(message) = inbox.try_recv() {
        received.push(message);
    }
    let (mut messages, mut bytes) = (0u64, 0u64);
    for (sender, payload) in received {
        bytes += payload.len() as u64;
        messages += 1;
        topics.receive(sender, &payload);
    }
    if messages > 0 {
        let counters = state.borrow_mut::<SceneResourceCounters>();
        counters.comms_msgs_in += messages;
        counters.comms_bytes_in += bytes;
    }
}

fn scene_topics(state: &mut impl State) -> &mut SceneTopics {
    if !state.has::<SceneTopics>() {
        state.put(SceneTopics::default());
    }
    state.borrow_mut::<SceneTopics>()
}

pub async fn op_comms_subscribe_topic(
    state: Rc<RefCell<impl State>>,
    topic: Option<String>,
) -> Result<(), anyhow::Error> {
    let topic = topic.ok_or_else(|| anyhow::anyhow!("topic is required"))?;
    let mut state = state.borrow_mut();
    drain_topic_inbox(&mut *state);
    if scene_topics(&mut *state).inbox.is_none() {
        let (sx, rx) = RpcStreamSender::channel_with_capacity(TOPIC_INBOX_CAPACITY);
        state
            .borrow_mut::<RpcCalls>()
            .push(RpcCall::SubscribeSceneTopics { sender: sx })?;
        scene_topics(&mut *state).inbox = Some(rx);
    }
    scene_topics(&mut *state).subscribe(topic);
    Ok(())
}

pub async fn op_comms_unsubscribe_topic(
    state: Rc<RefCell<impl State>>,
    topic: Option<String>,
) -> Result<(), anyhow::Error> {
    let topic = topic.ok_or_else(|| anyhow::anyhow!("topic is required"))?;
    let mut state = state.borrow_mut();
    drain_topic_inbox(&mut *state);
    scene_topics(&mut *state).unsubscribe(&topic);
    Ok(())
}

/// Never fails: an invalid or over-limit publish is dropped.
pub async fn op_comms_publish_data(
    state: Rc<RefCell<impl State>>,
    topic: Option<String>,
    data: Option<String>,
) {
    let (Some(data), mut state) = (data, state.borrow_mut()) else {
        return;
    };
    let Some(topic) = topic.filter(|_| !data.is_empty()) else {
        return;
    };
    drain_topic_inbox(&mut *state);
    let Some(framed) = scene_topics(&mut *state).frame_publish(&topic, &data, Instant::now())
    else {
        return;
    };
    let bytes = framed.len() as u64;
    if state
        .borrow_mut::<RpcCalls>()
        .push(RpcCall::PublishSceneTopic { data: framed })
        .is_ok()
    {
        let counters = state.borrow_mut::<SceneResourceCounters>();
        counters.comms_msgs_out += 1;
        counters.comms_bytes_out += bytes - 1;
    }
}

pub async fn op_comms_consume_messages(
    state: Rc<RefCell<impl State>>,
    topic: Option<String>,
) -> Vec<TopicMessage> {
    let Some(topic) = topic else {
        return Vec::new();
    };
    let mut state = state.borrow_mut();
    drain_topic_inbox(&mut *state);
    scene_topics(&mut *state).consume(&topic)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_state::TestState;

    fn state() -> Rc<RefCell<TestState>> {
        let mut state = TestState::default();
        state.put(RpcCalls::default());
        state.put(SceneResourceCounters::default());
        Rc::new(RefCell::new(state))
    }

    fn block_on<F: std::future::Future>(f: F) -> F::Output {
        futures_lite::future::block_on(f)
    }

    fn publish(state: &Rc<RefCell<TestState>>, topic: &str, data: &str) {
        block_on(op_comms_publish_data(
            state.clone(),
            Some(topic.to_owned()),
            Some(data.to_owned()),
        ));
    }

    fn subscribe(state: &Rc<RefCell<TestState>>, topic: &str) {
        block_on(op_comms_subscribe_topic(
            state.clone(),
            Some(topic.to_owned()),
        ))
        .unwrap();
    }

    fn consume(state: &Rc<RefCell<TestState>>, topic: &str) -> Vec<TopicMessage> {
        block_on(op_comms_consume_messages(
            state.clone(),
            Some(topic.to_owned()),
        ))
    }

    fn take_calls(
        state: &Rc<RefCell<TestState>>,
    ) -> (Vec<Vec<u8>>, Option<RpcStreamSender<(String, Vec<u8>)>>) {
        let calls = std::mem::take(&mut **state.borrow_mut().borrow_mut::<RpcCalls>());
        let mut published = Vec::new();
        let mut inbox = None;
        for call in calls {
            match call {
                RpcCall::PublishSceneTopic { data } => published.push(data),
                RpcCall::SubscribeSceneTopics { sender } => inbox = Some(sender),
                _ => panic!("unexpected call"),
            }
        }
        (published, inbox)
    }

    fn wire(topic: &str, data: &[u8]) -> Vec<u8> {
        let mut payload = (topic.len() as u16).to_le_bytes().to_vec();
        payload.extend(topic.as_bytes());
        payload.extend(data);
        payload
    }

    fn msg(sender: &str, data: &str) -> TopicMessage {
        TopicMessage {
            sender: sender.to_owned(),
            data: data.to_owned(),
        }
    }

    #[test]
    fn publish_frames_type_topic_length_topic_and_data() {
        let state = state();
        publish(&state, "chat", "{\"a\":1}");
        let (published, _) = take_calls(&state);
        let mut expected = vec![CommsMessageType::CommsData as u8, 4, 0];
        expected.extend(b"chat{\"a\":1}");
        assert_eq!(published, vec![expected]);
    }

    #[test]
    fn round_trip_delivers_sender_and_data() {
        let state = state();
        subscribe(&state, "t");
        let (_, inbox) = take_calls(&state);
        let inbox = inbox.expect("first subscribe asks the engine for topic messages");
        inbox
            .send(("0xabcd".to_owned(), wire("t", b"hello")))
            .unwrap();
        assert_eq!(consume(&state, "t"), vec![msg("0xabcd", "hello")]);
    }

    #[test]
    fn consume_is_empty_for_unsubscribed_and_idle_topics_and_drains() {
        let state = state();
        assert_eq!(consume(&state, "never"), vec![]);
        subscribe(&state, "t");
        assert_eq!(consume(&state, "t"), vec![]);
        let (_, inbox) = take_calls(&state);
        inbox
            .unwrap()
            .send(("0x1".to_owned(), wire("t", b"x")))
            .unwrap();
        assert_eq!(consume(&state, "t").len(), 1);
        assert_eq!(consume(&state, "t"), vec![]);
    }

    #[test]
    fn messages_before_subscribe_and_on_other_topics_are_dropped() {
        let state = state();
        subscribe(&state, "a");
        let (_, inbox) = take_calls(&state);
        let inbox = inbox.unwrap();
        inbox.send(("0x1".to_owned(), wire("b", b"early"))).unwrap();
        subscribe(&state, "b");
        inbox.send(("0x1".to_owned(), wire("b", b"late"))).unwrap();
        assert_eq!(consume(&state, "b"), vec![msg("0x1", "late")]);
        assert_eq!(consume(&state, "a"), vec![]);
    }

    #[test]
    fn unsubscribe_discards_the_queue_and_resubscribe_starts_empty() {
        let state = state();
        subscribe(&state, "t");
        let (_, inbox) = take_calls(&state);
        let inbox = inbox.unwrap();
        inbox.send(("0x1".to_owned(), wire("t", b"one"))).unwrap();
        block_on(op_comms_unsubscribe_topic(
            state.clone(),
            Some("t".to_owned()),
        ))
        .unwrap();
        subscribe(&state, "t");
        assert_eq!(consume(&state, "t"), vec![]);
        inbox.send(("0x1".to_owned(), wire("t", b"two"))).unwrap();
        assert_eq!(consume(&state, "t"), vec![msg("0x1", "two")]);
        assert!(take_calls(&state).1.is_none(), "one inbox per scene");
    }

    #[test]
    fn duplicate_subscribe_keeps_queued_messages() {
        let state = state();
        subscribe(&state, "t");
        let (_, inbox) = take_calls(&state);
        inbox
            .unwrap()
            .send(("0x1".to_owned(), wire("t", b"kept")))
            .unwrap();
        subscribe(&state, "t");
        assert_eq!(consume(&state, "t"), vec![msg("0x1", "kept")]);
    }

    #[test]
    fn empty_and_missing_data_send_nothing_and_spend_no_budget() {
        let state = state();
        block_on(op_comms_publish_data(
            state.clone(),
            Some("t".to_owned()),
            None,
        ));
        for _ in 0..20 {
            publish(&state, "t", "");
        }
        publish(&state, "t", "x");
        assert_eq!(take_calls(&state).0.len(), 1);
    }

    #[test]
    fn rate_limit_is_ten_per_window_per_topic() {
        let mut topics = SceneTopics::default();
        let start = Instant::now();
        let sent = (0..11)
            .filter(|_| topics.frame_publish("a", "x", start).is_some())
            .count();
        assert_eq!(sent, 10);
        assert!(topics.frame_publish("b", "x", start).is_some());
        let almost = start + Duration::from_millis(999);
        assert!(topics.frame_publish("a", "x", almost).is_none());
        let next = start + TOPIC_RATE_WINDOW;
        assert!(topics.frame_publish("a", "x", next).is_some());
    }

    #[test]
    fn oversized_topic_and_message_are_dropped_after_spending_budget() {
        let mut topics = SceneTopics::default();
        let now = Instant::now();
        let long_topic = "t".repeat(MAX_TOPIC_BYTES + 1);
        assert!(topics.frame_publish(&long_topic, "x", now).is_none());
        let topic = "t".repeat(MAX_TOPIC_BYTES);
        assert!(topics.frame_publish(&topic, "x", now).is_some());

        let fits = "d".repeat(MAX_TOPIC_MESSAGE_BYTES - 1 - 2 - 1);
        assert!(topics.frame_publish("s", &fits, now).is_some());
        let over = format!("{fits}d");
        for _ in 0..9 {
            assert!(topics.frame_publish("s", &over, now).is_none());
        }
        assert!(
            topics.frame_publish("s", "x", now).is_none(),
            "the dropped publishes used the window"
        );
    }

    #[test]
    fn a_full_queue_drops_its_oldest_message() {
        let mut topics = SceneTopics::default();
        topics.subscribe("t".to_owned());
        for i in 0..=TOPIC_QUEUE_CAPACITY {
            assert!(topics.receive("0x1".to_owned(), &wire("t", i.to_string().as_bytes())));
        }
        let messages = topics.consume("t");
        assert_eq!(messages.len(), TOPIC_QUEUE_CAPACITY);
        assert_eq!(messages[0].data, "1");
        assert_eq!(
            messages.last().unwrap().data,
            TOPIC_QUEUE_CAPACITY.to_string()
        );
    }

    #[test]
    fn malformed_payloads_are_dropped_and_empty_data_is_kept() {
        let mut topics = SceneTopics::default();
        topics.subscribe("t".to_owned());
        topics.subscribe(String::new());
        assert!(!topics.receive("0x1".to_owned(), &[1]));
        assert!(!topics.receive("0x1".to_owned(), &[5, 0, b't']));
        assert!(topics.receive("0x1".to_owned(), &wire("t", b"")));
        assert!(topics.receive("0x1".to_owned(), &wire("", b"\xff")));
        assert_eq!(topics.consume("t"), vec![msg("0x1", "")]);
        assert_eq!(topics.consume(""), vec![msg("0x1", "\u{fffd}")]);
    }

    #[test]
    fn missing_topic_rejects_subscribe_and_is_ignored_by_publish_and_consume() {
        let state = state();
        assert!(block_on(op_comms_subscribe_topic(state.clone(), None)).is_err());
        assert!(block_on(op_comms_unsubscribe_topic(state.clone(), None)).is_err());
        block_on(op_comms_publish_data(
            state.clone(),
            None,
            Some("x".to_owned()),
        ));
        assert!(block_on(op_comms_consume_messages(state.clone(), None)).is_empty());
        assert_eq!(take_calls(&state).0.len(), 0);
    }
}
