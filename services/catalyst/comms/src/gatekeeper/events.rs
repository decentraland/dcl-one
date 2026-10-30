//! The room events the gatekeeper hands to the crates above it: the LiveKit
//! webhook verifies each SFU participant event, applies it to the live
//! presence ([`crate::presence`]) itself and then passes it on to the
//! [`RoomEventSink`] `serve` installs (`CommsState::with_room_event_sink`).
//! Worlds' `CoreState` implements it (the worlds access log), so one webhook
//! URL feeds both.

/// A consumer of the SFU's participant events beside the gatekeeper (which
/// holds them and feeds the live presence): the worlds access log (worlds'
/// `CoreState`), so one LiveKit webhook URL feeds both. The world is already parsed.
pub trait RoomEventSink: Send + Sync {
    fn observe_room_event(&self, event: &crate::protocol::bus::room_events::RoomEvent);
}
