use std::{cell::RefCell, collections::HashMap, collections::VecDeque, rc::Rc, time::Duration};

use bevy::{log::debug, platform::time::Instant};
use common::rpc::{RpcCall, RpcResultSender, RpcStreamReceiver, RpcStreamSender};
use dcl_component::proto_components::kernel::apis::VideoTracksActiveStreamsResponse;
use serde::Serialize;

use crate::State;
use dcl_types::{CommsMessageType, RpcCalls, SceneResourceCounters};
pub use dcl_types::{MAX_TOPIC_BYTES, MAX_TOPIC_MESSAGE_BYTES};

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
/// Per subscribed topic; past it the oldest message is dropped.
pub const TOPIC_QUEUE_CAPACITY: usize = 1024;
/// Topics a scene may subscribe to at once.
pub const MAX_SUBSCRIBED_TOPICS: usize = 256;
/// Sender and data bytes queued across all of a scene's topics; past it new messages are dropped.
pub const MAX_QUEUED_TOPIC_BYTES: usize = 8 * 1024 * 1024;
/// Drained every tick, so it only has to hold one tick's traffic.
const TOPIC_INBOX_CAPACITY: usize = 4096;
const TOPIC_LENGTH_PREFIX_BYTES: usize = 2;
/// Expired rate windows are pruned once this many are held, then at twice what is left.
const RATE_WINDOW_PRUNE_MIN: usize = 64;
/// Rate windows a scene may hold open at once; past it a publish to a topic with no window is
/// dropped. Expired windows are pruned before the cap is applied, so it only binds within a window.
pub const MAX_RATE_WINDOWS: usize = 1024;

#[derive(Serialize, Debug, PartialEq, Eq)]
pub struct TopicMessage {
    pub sender: String,
    pub data: String,
}

impl TopicMessage {
    fn bytes(&self) -> usize {
        self.sender.len() + self.data.len()
    }
}

/// Lives in the scene's op state, so a reloaded scene starts with no subscriptions.
#[derive(Default)]
pub struct SceneTopics {
    queues: HashMap<String, VecDeque<TopicMessage>>,
    queued_bytes: usize,
    rate_windows: HashMap<String, (u32, Instant)>,
    prune_rate_windows_at: usize,
    inbox: Option<RpcStreamReceiver<(String, Vec<u8>)>>,
}

impl SceneTopics {
    /// A topic longer than any message can carry is accepted and never receives anything.
    pub fn subscribe(&mut self, topic: String) -> Result<(), anyhow::Error> {
        if topic.len() > MAX_TOPIC_BYTES || self.queues.contains_key(&topic) {
            return Ok(());
        }
        if self.queues.len() >= MAX_SUBSCRIBED_TOPICS {
            anyhow::bail!("too many subscribed topics ({MAX_SUBSCRIBED_TOPICS})");
        }
        self.queues.insert(topic, VecDeque::new());
        Ok(())
    }

    pub fn unsubscribe(&mut self, topic: &str) {
        if let Some(queue) = self.queues.remove(topic) {
            self.queued_bytes -= queue.iter().map(TopicMessage::bytes).sum::<usize>();
        }
    }

    pub fn consume(&mut self, topic: &str) -> Vec<TopicMessage> {
        let messages: Vec<_> = self
            .queues
            .get_mut(topic)
            .map(|queue| queue.drain(..).collect())
            .unwrap_or_default();
        self.queued_bytes -= messages.iter().map(TopicMessage::bytes).sum::<usize>();
        messages
    }

    /// A publish that reaches the rate check spends the topic's budget even if its size then
    /// drops it. A publish to a new topic is dropped while [`MAX_RATE_WINDOWS`] windows are open.
    pub fn frame_publish(&mut self, topic: &str, data: &str, now: Instant) -> Option<Vec<u8>> {
        if data.is_empty() || topic.len() > MAX_TOPIC_BYTES || !self.spend_rate(topic, now) {
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

    /// A fixed window, opened by the first publish after the previous one ended. An expired
    /// window and a missing one behave the same, so expired ones are pruned.
    fn spend_rate(&mut self, topic: &str, now: Instant) -> bool {
        let open = |start: &Instant| now.saturating_duration_since(*start) < TOPIC_RATE_WINDOW;
        match self.rate_windows.get_mut(topic) {
            Some((count, start)) if open(start) => {
                if *count >= MAX_TOPIC_MESSAGES_PER_WINDOW {
                    return false;
                }
                *count += 1;
            }
            Some(window) => *window = (1, now),
            None => {
                let prune_at = self
                    .prune_rate_windows_at
                    .clamp(RATE_WINDOW_PRUNE_MIN, MAX_RATE_WINDOWS);
                if self.rate_windows.len() >= prune_at {
                    self.rate_windows.retain(|_, (_, start)| open(start));
                    self.prune_rate_windows_at = 2 * self.rate_windows.len();
                }
                if self.rate_windows.len() >= MAX_RATE_WINDOWS {
                    return false;
                }
                self.rate_windows.insert(topic.to_owned(), (1, now));
            }
        }
        true
    }

    /// `payload` is `[u16 LE topic length][topic][data]`. A payload no conforming publisher
    /// can send is dropped.
    pub fn receive(&mut self, sender: String, payload: &[u8]) -> bool {
        if 1 + payload.len() > MAX_TOPIC_MESSAGE_BYTES {
            return false;
        }
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
        let data = &payload[TOPIC_LENGTH_PREFIX_BYTES + topic_len..];
        let message = TopicMessage {
            sender,
            data: String::from_utf8_lossy(data).into_owned(),
        };
        let freed = match queue.front() {
            Some(oldest) if queue.len() >= TOPIC_QUEUE_CAPACITY => oldest.bytes(),
            _ => 0,
        };
        if self.queued_bytes - freed + message.bytes() > MAX_QUEUED_TOPIC_BYTES {
            return false;
        }
        if queue.len() >= TOPIC_QUEUE_CAPACITY {
            queue.pop_front();
        }
        self.queued_bytes = self.queued_bytes - freed + message.bytes();
        queue.push_back(message);
        true
    }
}

/// Runs before subscribe, unsubscribe and consume, and once per tick, so a message is matched
/// against the subscriptions of the moment it arrived.
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
    scene_topics(&mut *state).subscribe(topic)
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

/// Never fails: an invalid or over-limit publish is dropped. A publish that passes its topic's
/// limits also spends the per-tick budget the message bus sends share.
pub async fn op_comms_publish_data(
    state: Rc<RefCell<impl State>>,
    topic: Option<String>,
    data: Option<String>,
) {
    let (Some(topic), Some(data)) = (topic, data) else {
        return;
    };
    let mut state = state.borrow_mut();
    let Some(framed) = scene_topics(&mut *state).frame_publish(&topic, &data, Instant::now())
    else {
        return;
    };
    if !crate::comms::try_spend_budget(&mut *state) {
        return;
    }
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
        topics.subscribe("t".to_owned()).unwrap();
        for i in 0..=TOPIC_QUEUE_CAPACITY {
            assert!(topics.receive("0x1".to_owned(), &wire("t", i.to_string().as_bytes())));
        }
        let queued: usize = topics.queues["t"].iter().map(TopicMessage::bytes).sum();
        assert_eq!(
            topics.queued_bytes, queued,
            "the dropped message's bytes are freed"
        );
        let messages = topics.consume("t");
        assert_eq!(topics.queued_bytes, 0);
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
        topics.subscribe("t".to_owned()).unwrap();
        topics.subscribe(String::new()).unwrap();
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

    #[test]
    fn an_oversized_topic_opens_no_rate_window() {
        let mut topics = SceneTopics::default();
        let long_topic = "t".repeat(MAX_TOPIC_BYTES + 1);
        assert!(topics
            .frame_publish(&long_topic, "x", Instant::now())
            .is_none());
        assert!(topics.rate_windows.is_empty());
    }

    #[test]
    fn expired_rate_windows_are_pruned() {
        let mut topics = SceneTopics::default();
        let start = Instant::now();
        for i in 0..RATE_WINDOW_PRUNE_MIN {
            assert!(topics.frame_publish(&i.to_string(), "x", start).is_some());
        }
        let later = start + TOPIC_RATE_WINDOW;
        assert!(topics.frame_publish("new", "x", later).is_some());
        assert_eq!(topics.rate_windows.len(), 1);
    }

    #[test]
    fn open_rate_windows_are_capped() {
        let mut topics = SceneTopics::default();
        let start = Instant::now();
        let sent = (0..MAX_RATE_WINDOWS + 100)
            .filter(|i| topics.frame_publish(&i.to_string(), "x", start).is_some())
            .count();
        assert_eq!(sent, MAX_RATE_WINDOWS);
        assert_eq!(topics.rate_windows.len(), MAX_RATE_WINDOWS);
        assert!(
            topics.frame_publish("0", "x", start).is_some(),
            "a topic with an open window still publishes"
        );
    }

    #[test]
    fn a_full_map_of_rate_windows_accepts_publishes_once_they_expire() {
        let mut topics = SceneTopics::default();
        let start = Instant::now();
        // A prune at the cap that finds every window still open sets the next prune above it.
        for i in 0..MAX_RATE_WINDOWS / 4 {
            assert!(topics
                .frame_publish(&format!("old{i}"), "x", start)
                .is_some());
        }
        let later = start + Duration::from_millis(500);
        for i in 0..MAX_RATE_WINDOWS {
            topics.frame_publish(&format!("new{i}"), "x", later);
        }
        assert!(topics.prune_rate_windows_at > MAX_RATE_WINDOWS);
        assert!(topics.frame_publish("blocked", "x", later).is_none());

        let expired = later + TOPIC_RATE_WINDOW;
        assert!(topics.frame_publish("new0", "x", expired).is_some());
        assert!(topics.frame_publish("fresh", "x", expired).is_some());
        assert!(topics.rate_windows.len() <= MAX_RATE_WINDOWS);
    }

    #[test]
    fn publishes_share_the_message_bus_send_budget() {
        let state = state();
        publish(&state, "t", "x");
        assert_eq!(
            state
                .borrow_mut()
                .borrow_mut::<crate::comms::CommsSendBudget>()
                .sent,
            1
        );
        state
            .borrow_mut()
            .borrow_mut::<crate::comms::CommsSendBudget>()
            .sent = crate::comms::MAX_SEND_MESSAGES_PER_TICK;
        publish(&state, "u", "x");
        assert_eq!(take_calls(&state).0.len(), 1);
    }

    #[test]
    fn subscriptions_are_capped_and_overlong_topics_never_subscribe() {
        let mut topics = SceneTopics::default();
        for i in 0..MAX_SUBSCRIBED_TOPICS {
            topics.subscribe(i.to_string()).unwrap();
        }
        assert!(topics.subscribe("one more".to_owned()).is_err());
        assert!(
            topics.subscribe("0".to_owned()).is_ok(),
            "already subscribed"
        );
        topics.unsubscribe("0");
        let long_topic = "t".repeat(MAX_TOPIC_BYTES + 1);
        assert!(topics.subscribe(long_topic.clone()).is_ok());
        assert!(!topics.queues.contains_key(&long_topic));
        assert!(topics.subscribe("one more".to_owned()).is_ok());
    }

    #[test]
    fn received_payloads_over_the_message_limit_are_dropped() {
        let mut topics = SceneTopics::default();
        topics.subscribe("t".to_owned()).unwrap();
        let fits = vec![b'd'; MAX_TOPIC_MESSAGE_BYTES - 1 - 2 - 1];
        assert!(topics.receive("0x1".to_owned(), &wire("t", &fits)));
        let mut over = fits.clone();
        over.push(b'd');
        assert!(!topics.receive("0x1".to_owned(), &wire("t", &over)));
        assert_eq!(topics.consume("t").len(), 1);
    }

    #[test]
    fn queued_bytes_are_capped_across_topics_and_freed_by_consume() {
        let mut topics = SceneTopics::default();
        topics.subscribe("a".to_owned()).unwrap();
        topics.subscribe("b".to_owned()).unwrap();
        let data = vec![b'd'; MAX_TOPIC_MESSAGE_BYTES - 1 - 2 - 1];
        let per_message = "0x1".len() + data.len();
        let fit = MAX_QUEUED_TOPIC_BYTES / per_message;
        let accepted = (0..fit + 10)
            .filter(|i| {
                let topic = if i % 2 == 0 { "a" } else { "b" };
                topics.receive("0x1".to_owned(), &wire(topic, &data))
            })
            .count();
        assert_eq!(accepted, fit);
        assert!(!topics.receive("0x1".to_owned(), &wire("a", &data)));
        topics.unsubscribe("b");
        assert!(topics.receive("0x1".to_owned(), &wire("a", &data)));
        topics.consume("a");
        assert_eq!(topics.queued_bytes, 0);
    }
}
