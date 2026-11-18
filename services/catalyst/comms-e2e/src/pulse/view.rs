//! What one Pulse connection has been told: who is there, where, and how the connection itself
//! fared. A scenario asserts on this, never on what a bot assumes.

use std::collections::HashMap;

use catalyst_comms::pulse::decentraland::pulse::{
    ClientMessage, PlayerState, PlayerStateDeltaTier0, ServerMessage, server_message::Message,
};
use serde::Serialize;

use super::Channel;
use super::messages;
use crate::stack::Grid;

/// One other player as this connection sees them.
#[derive(Debug, Clone, Serialize)]
pub struct Subject {
    pub subject_id: u32,
    pub wallet: String,
    pub realm: String,
    pub sequence: u32,
    pub world: [f32; 3],
    pub emote: Option<String>,
    pub emote_mask: Option<i32>,
    pub profile_version: i32,
    #[serde(skip)]
    state: PlayerState,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct PulseView {
    /// The transport connected.
    pub connected: bool,
    /// The server answered the handshake: `Some(true)` accepted, `Some(false)` refused.
    pub handshake: Option<bool>,
    pub handshake_error: Option<String>,
    /// The disconnect code, once the server or the transport ended the connection.
    pub disconnect: Option<u32>,
    /// Players by subject id.
    pub subjects: HashMap<u32, Subject>,
    pub joins: u32,
    pub leaves: u32,
    /// Joins and leaves per wallet: a session that was dropped and made again shows here even
    /// when the new one gets the old subject id.
    pub joins_by_wallet: HashMap<String, u32>,
    pub leaves_by_wallet: HashMap<String, u32>,
    /// Joins that named this connection's own wallet.
    pub self_joins: u32,
    pub teleports: u32,
    pub emote_starts: u32,
    pub emote_stops: u32,
    pub full_states: u32,
    pub deltas: u32,
    /// Deltas whose baseline was not the sequence held: each asks for a resync.
    pub sequence_gaps: u32,
    pub resyncs_requested: u32,
    /// Messages about a subject this connection was never told had joined.
    pub unknown_subject_messages: u32,
    pub profile_announcements: u32,
    /// Frames that did not decode as a server message.
    pub undecodable: u32,
    pub messages_received: u64,
}

impl PulseView {
    /// The subject of `wallet`, if exactly one is held.
    pub fn of(&self, wallet: &str) -> Option<&Subject> {
        let mut matches = self.subjects.values().filter(|subject| subject.wallet == wallet);
        let first = matches.next()?;
        matches.next().is_none().then_some(first)
    }

    /// How many subjects carry `wallet`. More than one is a ghost.
    pub fn count_of(&self, wallet: &str) -> usize {
        self.subjects.values().filter(|subject| subject.wallet == wallet).count()
    }

    /// The wallets held, sorted.
    pub fn wallets(&self) -> Vec<String> {
        let mut wallets: Vec<String> =
            self.subjects.values().map(|subject| subject.wallet.clone()).collect();
        wallets.sort();
        wallets
    }

    pub fn accepted(&self) -> bool {
        self.handshake == Some(true)
    }

    /// How many times `wallet` was announced as joined.
    pub fn joins_of(&self, wallet: &str) -> u32 {
        self.joins_by_wallet.get(wallet).copied().unwrap_or(0)
    }

    /// How many times a subject of `wallet` was announced as left.
    pub fn leaves_of(&self, wallet: &str) -> u32 {
        self.leaves_by_wallet.get(wallet).copied().unwrap_or(0)
    }
}

/// The receiving half of a connection: decodes what arrives, keeps the view, and answers a
/// sequence gap with a resync request, as any client must to keep a correct picture.
pub(crate) struct Session {
    pub view: PulseView,
    pub own_wallet: Option<String>,
    pub grid: Grid,
    pub auto_resync: bool,
}

/// What a frame meant, for the timeline.
pub(crate) struct Seen {
    pub kind: &'static str,
    pub detail: serde_json::Value,
    /// Frames to send back at once.
    pub replies: Vec<(Channel, ClientMessage)>,
}

impl Session {
    pub fn new(grid: Grid, auto_resync: bool) -> Self {
        Self {
            view: PulseView::default(),
            own_wallet: None,
            grid,
            auto_resync,
        }
    }

    fn place(&self, subject: &mut Subject) {
        subject.world = messages::world_of(&self.grid, &subject.state);
    }

    pub fn on_frame(&mut self, bytes: &[u8]) -> Seen {
        use prost::Message as _;
        self.view.messages_received += 1;
        let Ok(ServerMessage {
            message: Some(message),
        }) = ServerMessage::decode(bytes)
        else {
            self.view.undecodable += 1;
            return Seen {
                kind: "undecodable",
                detail: serde_json::json!({ "bytes": bytes.len() }),
                replies: Vec::new(),
            };
        };
        let mut replies = Vec::new();
        let (kind, detail) = match message {
            Message::Handshake(response) => {
                self.view.handshake = Some(response.success);
                self.view.handshake_error = response.error.clone();
                (
                    "handshake response",
                    serde_json::json!({ "success": response.success, "error": response.error }),
                )
            }
            Message::PlayerJoined(joined) => {
                self.view.joins += 1;
                let wallet = joined.user_id.to_lowercase();
                *self.view.joins_by_wallet.entry(wallet.clone()).or_default() += 1;
                if self.own_wallet.as_deref() == Some(wallet.as_str()) {
                    self.view.self_joins += 1;
                }
                let full = joined.state.unwrap_or_default();
                let mut subject = Subject {
                    subject_id: full.subject_id,
                    wallet: wallet.clone(),
                    realm: joined.realm.clone(),
                    sequence: full.sequence,
                    world: [0.0; 3],
                    emote: None,
                    emote_mask: None,
                    profile_version: joined.profile_version,
                    state: full.state.unwrap_or_default(),
                };
                self.place(&mut subject);
                let detail = serde_json::json!({ "subject": subject.subject_id, "wallet": wallet, "realm": joined.realm, "sequence": subject.sequence });
                self.view.subjects.insert(subject.subject_id, subject);
                ("joined", detail)
            }
            Message::PlayerLeft(left) => {
                self.view.leaves += 1;
                let known = self.view.subjects.remove(&left.subject_id);
                match &known {
                    Some(subject) => {
                        *self
                            .view
                            .leaves_by_wallet
                            .entry(subject.wallet.clone())
                            .or_default() += 1;
                    }
                    None => self.view.unknown_subject_messages += 1,
                }
                (
                    "left",
                    serde_json::json!({ "subject": left.subject_id, "wallet": known.map(|subject| subject.wallet) }),
                )
            }
            Message::PlayerStateFull(full) => {
                self.view.full_states += 1;
                let grid = self.grid;
                match self.view.subjects.get_mut(&full.subject_id) {
                    Some(subject) => {
                        subject.sequence = full.sequence;
                        subject.state = full.state.unwrap_or_default();
                        subject.world = messages::world_of(&grid, &subject.state);
                    }
                    None => self.view.unknown_subject_messages += 1,
                }
                (
                    "full state",
                    serde_json::json!({ "subject": full.subject_id, "sequence": full.sequence }),
                )
            }
            Message::PlayerStateDelta(delta) => {
                self.view.deltas += 1;
                let grid = self.grid;
                let mut gap = None;
                match self.view.subjects.get_mut(&delta.subject_id) {
                    Some(subject) if subject.sequence == delta.baseline_seq => {
                        apply_delta(&mut subject.state, &delta);
                        subject.sequence = delta.new_seq;
                        subject.world = messages::world_of(&grid, &subject.state);
                    }
                    // a delta at or behind what is held is a late duplicate, not a gap
                    Some(subject) if delta.new_seq <= subject.sequence => {}
                    Some(subject) => gap = Some(subject.sequence),
                    None => self.view.unknown_subject_messages += 1,
                }
                if let Some(known) = gap {
                    self.view.sequence_gaps += 1;
                    if self.auto_resync {
                        self.view.resyncs_requested += 1;
                        replies.push((Channel::Reliable, messages::resync(delta.subject_id, known)));
                    }
                }
                (
                    if gap.is_some() { "delta with a gap" } else { "delta" },
                    serde_json::json!({ "subject": delta.subject_id, "baseline": delta.baseline_seq, "sequence": delta.new_seq, "held": gap }),
                )
            }
            Message::Teleported(teleported) => {
                self.view.teleports += 1;
                let grid = self.grid;
                match self.view.subjects.get_mut(&teleported.subject_id) {
                    Some(subject) => {
                        subject.sequence = teleported.sequence;
                        subject.state = teleported.state.unwrap_or_default();
                        subject.realm = teleported.realm.clone();
                        subject.world = messages::world_of(&grid, &subject.state);
                    }
                    None => self.view.unknown_subject_messages += 1,
                }
                (
                    "teleported",
                    serde_json::json!({ "subject": teleported.subject_id, "realm": teleported.realm, "sequence": teleported.sequence }),
                )
            }
            Message::EmoteStarted(started) => {
                self.view.emote_starts += 1;
                let grid = self.grid;
                match self.view.subjects.get_mut(&started.subject_id) {
                    Some(subject) => {
                        subject.sequence = started.sequence;
                        subject.emote = Some(started.emote_id.clone());
                        subject.emote_mask = started.mask;
                        if let Some(state) = started.player_state {
                            subject.state = state;
                            subject.world = messages::world_of(&grid, &subject.state);
                        }
                    }
                    None => self.view.unknown_subject_messages += 1,
                }
                (
                    "emote started",
                    serde_json::json!({ "subject": started.subject_id, "emote": started.emote_id, "mask": started.mask, "sequence": started.sequence }),
                )
            }
            Message::EmoteStopped(stopped) => {
                self.view.emote_stops += 1;
                match self.view.subjects.get_mut(&stopped.subject_id) {
                    Some(subject) => {
                        subject.sequence = stopped.sequence;
                        subject.emote = None;
                        subject.emote_mask = None;
                    }
                    None => self.view.unknown_subject_messages += 1,
                }
                (
                    "emote stopped",
                    serde_json::json!({ "subject": stopped.subject_id, "reason": stopped.reason, "sequence": stopped.sequence }),
                )
            }
            Message::PlayerProfileVersionAnnounced(announced) => {
                self.view.profile_announcements += 1;
                if let Some(subject) = self.view.subjects.get_mut(&announced.subject_id) {
                    subject.profile_version = announced.version;
                }
                (
                    "profile version",
                    serde_json::json!({ "subject": announced.subject_id, "version": announced.version }),
                )
            }
            other => (
                "other",
                serde_json::json!({ "message": format!("{other:?}").chars().take(80).collect::<String>() }),
            ),
        };
        Seen {
            kind,
            detail,
            replies,
        }
    }
}

fn apply_delta(state: &mut PlayerState, delta: &PlayerStateDeltaTier0) {
    macro_rules! copy {
        ($($field:ident),*) => {$(
            if let Some(value) = delta.$field {
                state.$field = value;
            }
        )*};
    }
    copy!(
        parcel_index,
        position_x,
        position_y,
        position_z,
        velocity_x,
        velocity_y,
        velocity_z,
        rotation_y,
        movement_blend,
        slide_blend,
        state_flags,
        glide_state,
        jump_count
    );
    macro_rules! copy_optional {
        ($($field:ident),*) => {$(
            if delta.$field.is_some() {
                state.$field = delta.$field;
            }
        )*};
    }
    copy_optional!(head_yaw, head_pitch, point_at_x, point_at_y, point_at_z);
}

#[cfg(test)]
mod tests {
    use super::*;
    use catalyst_comms::pulse::decentraland::pulse::{PlayerJoined, PlayerLeft, PlayerStateFull};
    use prost::Message as _;

    fn frame(message: Message) -> Vec<u8> {
        ServerMessage {
            message: Some(message),
        }
        .encode_to_vec()
    }

    fn joined(subject_id: u32, wallet: &str, sequence: u32) -> Vec<u8> {
        frame(Message::PlayerJoined(PlayerJoined {
            user_id: wallet.to_owned(),
            profile_version: 1,
            state: Some(PlayerStateFull {
                subject_id,
                sequence,
                server_tick: 0,
                state: Some(messages::state_at(&Grid::default(), [8.0, 0.0, 8.0]).unwrap()),
            }),
            realm: "realm".to_owned(),
        }))
    }

    fn delta(subject_id: u32, baseline: u32, sequence: u32) -> Vec<u8> {
        frame(Message::PlayerStateDelta(PlayerStateDeltaTier0 {
            subject_id,
            baseline_seq: baseline,
            new_seq: sequence,
            position_x: Some(200),
            ..Default::default()
        }))
    }

    #[test]
    fn a_delta_on_the_held_sequence_moves_the_subject() {
        let mut session = Session::new(Grid::default(), true);
        session.on_frame(&joined(7, "0xAbC", 5));
        let before = session.view.of("0xabc").unwrap().world;
        let seen = session.on_frame(&delta(7, 5, 6));
        assert!(seen.replies.is_empty());
        let subject = session.view.of("0xabc").unwrap();
        assert_eq!(subject.sequence, 6);
        assert_ne!(subject.world, before);
        assert_eq!(session.view.sequence_gaps, 0);
    }

    #[test]
    fn a_delta_on_another_baseline_asks_for_a_resync_with_the_held_sequence() {
        let mut session = Session::new(Grid::default(), true);
        session.on_frame(&joined(7, "0xabc", 5));
        let seen = session.on_frame(&delta(7, 6, 7));
        assert_eq!(session.view.sequence_gaps, 1);
        assert_eq!(session.view.resyncs_requested, 1);
        assert_eq!(seen.replies.len(), 1);
        assert_eq!(seen.replies[0].1, messages::resync(7, 5));
        // the subject did not advance on a delta it could not apply
        assert_eq!(session.view.of("0xabc").unwrap().sequence, 5);

        // a late duplicate is neither applied nor a gap
        session.on_frame(&delta(7, 3, 4));
        assert_eq!(session.view.sequence_gaps, 1);
    }

    #[test]
    fn two_subjects_of_one_wallet_are_a_ghost_the_view_can_show() {
        let mut session = Session::new(Grid::default(), true);
        session.own_wallet = Some("0xme".to_owned());
        session.on_frame(&joined(1, "0xabc", 1));
        session.on_frame(&joined(2, "0xabc", 1));
        session.on_frame(&joined(3, "0xME", 1));
        assert_eq!(session.view.count_of("0xabc"), 2);
        assert!(session.view.of("0xabc").is_none());
        assert_eq!(session.view.self_joins, 1);
        assert_eq!((session.view.joins_of("0xabc"), session.view.leaves_of("0xabc")), (2, 0));
        session.on_frame(&frame(Message::PlayerLeft(PlayerLeft { subject_id: 1 })));
        assert_eq!(session.view.count_of("0xabc"), 1);
        assert_eq!((session.view.joins_of("0xabc"), session.view.leaves_of("0xabc")), (2, 1));
        assert_eq!(session.view.leaves_of("0xme"), 0);
        // a leave for a subject never announced is counted, not ignored
        session.on_frame(&frame(Message::PlayerLeft(PlayerLeft { subject_id: 99 })));
        assert_eq!(session.view.unknown_subject_messages, 1);
    }

    #[test]
    fn garbage_is_counted_and_changes_nothing() {
        let mut session = Session::new(Grid::default(), true);
        let seen = session.on_frame(&[0xff, 0xff, 0xff]);
        assert_eq!(seen.kind, "undecodable");
        assert_eq!(session.view.undecodable, 1);
        assert!(session.view.subjects.is_empty());
    }
}
