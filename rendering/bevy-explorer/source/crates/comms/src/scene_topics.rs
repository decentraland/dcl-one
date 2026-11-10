use std::{
    collections::{HashMap, HashSet, VecDeque},
    time::Duration,
};

const MAX_TOPIC_SUBSCRIPTIONS: usize = 128;
const MAX_QUEUED_TOPIC_MESSAGES: usize = 1024;
const MAX_ACTIVE_PUBLISH_TOPICS: usize = 512;
const PUBLISH_WINDOW: Duration = Duration::from_secs(1);
const PUBLISHES_PER_WINDOW: u8 = 10;

#[derive(Default)]
pub struct TopicPublishBudget {
    windows: HashMap<String, (Duration, u8)>,
    next_expiry: Option<Duration>,
}

impl TopicPublishBudget {
    pub(crate) fn try_publish(&mut self, topic: &str, now: Duration) -> bool {
        if let Some((expires, count)) = self.windows.get_mut(topic) {
            if now >= *expires {
                *expires = now + PUBLISH_WINDOW;
                *count = 0;
            }
            if *count >= PUBLISHES_PER_WINDOW {
                return false;
            }
            *count += 1;
            return true;
        }

        if self.windows.len() == MAX_ACTIVE_PUBLISH_TOPICS {
            if self.next_expiry.is_some_and(|expires| now < expires) {
                return false;
            }
            self.windows.retain(|_, (expires, _)| now < *expires);
            self.next_expiry = self.windows.values().map(|(expires, _)| *expires).min();
            if self.windows.len() == MAX_ACTIVE_PUBLISH_TOPICS {
                return false;
            }
        }
        self.windows
            .insert(topic.to_owned(), (now + PUBLISH_WINDOW, 1));
        true
    }
}

struct TopicMessage {
    topic: String,
    sender: String,
    data: String,
}

#[derive(Default)]
pub(crate) struct TopicInbox {
    subscriptions: HashSet<String>,
    messages: VecDeque<TopicMessage>,
}

impl TopicInbox {
    pub(crate) fn set_subscription(&mut self, topic: String, subscribed: bool) {
        if subscribed {
            if self.subscriptions.len() < MAX_TOPIC_SUBSCRIPTIONS {
                self.subscriptions.insert(topic);
            }
        } else if self.subscriptions.remove(&topic) {
            self.messages.retain(|message| message.topic != topic);
        }
    }

    pub(crate) fn receive(&mut self, topic: &str, sender: &str, data: &str) {
        if !self.subscriptions.contains(topic) {
            return;
        }
        if self.messages.len() == MAX_QUEUED_TOPIC_MESSAGES {
            self.messages.pop_front();
        }
        self.messages.push_back(TopicMessage {
            topic: topic.to_owned(),
            sender: sender.to_owned(),
            data: data.to_owned(),
        });
    }

    pub(crate) fn consume(&mut self, topic: &str) -> Vec<(String, String)> {
        let mut consumed = Vec::new();
        if self.subscriptions.contains(topic) {
            self.messages.retain_mut(|message| {
                if message.topic != topic {
                    return true;
                }
                consumed.push((
                    std::mem::take(&mut message.sender),
                    std::mem::take(&mut message.data),
                ));
                false
            });
        }
        consumed
    }

    pub(crate) fn counts(&self) -> (usize, usize) {
        (self.subscriptions.len(), self.messages.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn topic_publish_rate_is_scoped_and_restarts_after_one_second() {
        let mut budget = TopicPublishBudget::default();
        for _ in 0..10 {
            assert!(budget.try_publish("a", Duration::ZERO));
        }
        assert!(!budget.try_publish("a", Duration::from_millis(999)));
        assert!(budget.try_publish("b", Duration::from_millis(999)));
        assert!(budget.try_publish("a", Duration::from_secs(1)));
    }

    #[test]
    fn full_publish_budget_reclaims_expired_topics() {
        let mut budget = TopicPublishBudget::default();
        for i in 0..MAX_ACTIVE_PUBLISH_TOPICS {
            assert!(budget.try_publish(&i.to_string(), Duration::ZERO));
        }
        assert!(!budget.try_publish("extra", Duration::from_millis(999)));
        assert!(budget.try_publish("extra", Duration::from_secs(1)));
    }

    #[test]
    fn only_subscribed_topics_are_buffered_and_consumption_drains_one_topic() {
        let mut inbox = TopicInbox::default();
        inbox.set_subscription("a".into(), true);
        inbox.set_subscription("b".into(), true);
        inbox.receive("a", "peer-a", "first");
        inbox.receive("c", "peer-b", "unsubscribed");
        inbox.receive("b", "peer-b", "other topic");
        inbox.receive("a", "peer-b", "second");

        assert!(inbox.consume("c").is_empty());
        assert_eq!(
            inbox.consume("a"),
            [
                ("peer-a".into(), "first".into()),
                ("peer-b".into(), "second".into())
            ],
        );
        assert!(inbox.consume("a").is_empty());
        assert_eq!(
            inbox.consume("b"),
            [("peer-b".into(), "other topic".into())]
        );
    }

    #[test]
    fn overflow_keeps_new_messages_and_unsubscribe_discards_the_queue() {
        let mut inbox = TopicInbox::default();
        inbox.set_subscription("topic".into(), true);
        inbox.receive("topic", "peer", "oldest");
        for _ in 0..MAX_QUEUED_TOPIC_MESSAGES {
            inbox.receive("topic", "peer", "recent");
        }
        let messages = inbox.consume("topic");
        assert_eq!(messages.len(), MAX_QUEUED_TOPIC_MESSAGES);
        assert!(messages.iter().all(|(_, data)| data == "recent"));

        inbox.receive("topic", "peer", "before unsubscribe");
        inbox.set_subscription("topic".into(), false);
        inbox.receive("topic", "peer", "while unsubscribed");
        inbox.set_subscription("topic".into(), true);
        assert!(inbox.consume("topic").is_empty());
    }

    #[test]
    fn subscriptions_are_bounded_and_reuse_freed_slots() {
        let mut inbox = TopicInbox::default();
        for i in 0..MAX_TOPIC_SUBSCRIPTIONS {
            inbox.set_subscription(i.to_string(), true);
        }
        inbox.set_subscription("extra".into(), true);
        inbox.receive("extra", "peer", "first");
        assert!(inbox.consume("extra").is_empty());
        inbox.set_subscription("0".into(), false);
        inbox.set_subscription("extra".into(), true);
        inbox.receive("extra", "peer", "second");
        assert_eq!(inbox.consume("extra"), [("peer".into(), "second".into())]);
    }
}
