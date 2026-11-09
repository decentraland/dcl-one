//! The room events the gatekeeper hands to the crates above it: the LiveKit
//! webhook verifies each SFU participant event, applies it to the live
//! presence ([`crate::presence`]) itself and then passes it on to the
//! [`RoomEventSink`] `serve` installs (`CommsState::with_room_event_sink`).
//! `serve` installs a [`RoomEventFanout`] of worlds' `CoreState` (the worlds
//! access log) and, when enabled, social's presence-badge recorder, so one
//! webhook URL feeds all of them.

use std::sync::Arc;

/// A consumer of the SFU's participant events beside the gatekeeper (which
/// holds them and feeds the live presence): the worlds access log (worlds'
/// `CoreState`), so one LiveKit webhook URL feeds both. The world is already parsed.
/// Called on the webhook's path: an implementation must not block.
pub trait RoomEventSink: Send + Sync {
    fn observe_room_event(&self, event: &crate::protocol::bus::room_events::RoomEvent);
}

/// Hands every event to each sink, in order.
pub struct RoomEventFanout(Vec<Arc<dyn RoomEventSink>>);

impl RoomEventFanout {
    pub fn new(sinks: Vec<Arc<dyn RoomEventSink>>) -> Self {
        Self(sinks)
    }
}

impl RoomEventSink for RoomEventFanout {
    fn observe_room_event(&self, event: &crate::protocol::bus::room_events::RoomEvent) {
        for sink in &self.0 {
            sink.observe_room_event(event);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::bus::room_events::RoomEvent;

    #[derive(Default)]
    struct Recording(std::sync::Mutex<Vec<String>>);
    impl RoomEventSink for Recording {
        fn observe_room_event(&self, event: &RoomEvent) {
            self.0.lock().unwrap().push(event.event_id.clone());
        }
    }

    #[test]
    fn the_fanout_hands_each_event_to_every_sink() {
        let (a, b) = (
            Arc::new(Recording::default()),
            Arc::new(Recording::default()),
        );
        let fanout = RoomEventFanout::new(vec![a.clone(), b.clone()]);
        for id in ["EV_1", "EV_2"] {
            fanout.observe_room_event(&RoomEvent {
                event_id: id.into(),
                ..RoomEvent::default()
            });
        }
        assert_eq!(*a.0.lock().unwrap(), ["EV_1", "EV_2"]);
        assert_eq!(*b.0.lock().unwrap(), ["EV_1", "EV_2"]);
    }
}
