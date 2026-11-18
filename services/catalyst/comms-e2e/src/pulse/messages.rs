//! Builders for the messages a Pulse client sends: the valid ones, and the ones a correct client
//! never sends but the servers must answer exactly.

use catalyst_comms::pulse::decentraland::pulse::{
    ClientMessage, EmoteStart, EmoteStop, ParcelRect, PlayerInitialState, PlayerState,
    PlayerStateInput, ProfileVersionAnnouncement, ResyncRequest, SceneListenerAoi,
    SceneListenerUpdate, TeleportRequest, client_message::Message,
};
use catalyst_comms::pulse::hardening::DisconnectReason;

use crate::stack::Grid;

fn wrap(message: Message) -> ClientMessage {
    ClientMessage {
        message: Some(message),
    }
}

/// A grounded player standing at `world`, or `None` outside the grid.
pub fn state_at(grid: &Grid, world: [f32; 3]) -> Option<PlayerState> {
    let (parcel_index, local) = grid.locate(world)?;
    Some(state_in_parcel(parcel_index, local))
}

/// A player at an offset inside the parcel `parcel_index`, whatever that index is: the way to
/// build a state the server must refuse.
pub fn state_in_parcel(parcel_index: i32, local: [f32; 3]) -> PlayerState {
    let mut state = PlayerState {
        parcel_index,
        ..Default::default()
    };
    state.set_position_x_f(local[0]);
    state.set_position_y_f(local[1]);
    state.set_position_z_f(local[2]);
    state
}

/// The world position a state encodes.
pub fn world_of(grid: &Grid, state: &PlayerState) -> [f32; 3] {
    grid.world(
        state.parcel_index,
        [
            state.position_x_f(),
            state.position_y_f(),
            state.position_z_f(),
        ],
    )
}

pub fn input(state: PlayerState) -> ClientMessage {
    wrap(Message::Input(PlayerStateInput { state: Some(state) }))
}

/// An input with no state at all.
pub fn input_without_state() -> ClientMessage {
    wrap(Message::Input(PlayerStateInput { state: None }))
}

pub fn input_at(grid: &Grid, world: [f32; 3]) -> Option<ClientMessage> {
    state_at(grid, world).map(input)
}

pub fn teleport(grid: &Grid, realm: &str, world: [f32; 3]) -> Option<ClientMessage> {
    let state = state_at(grid, world)?;
    Some(teleport_to_parcel(realm, state.parcel_index, &state))
}

/// A teleport into the parcel `parcel_index`, whatever that index is.
pub fn teleport_to_parcel(realm: &str, parcel_index: i32, offset: &PlayerState) -> ClientMessage {
    wrap(Message::Teleport(TeleportRequest {
        parcel_index,
        position_x: offset.position_x,
        position_y: offset.position_y,
        position_z: offset.position_z,
        realm: realm.to_owned(),
    }))
}

pub fn emote_start(
    emote_id: &str,
    duration_ms: Option<u32>,
    state: Option<PlayerState>,
    mask: Option<i32>,
) -> ClientMessage {
    wrap(Message::EmoteStart(EmoteStart {
        emote_id: emote_id.to_owned(),
        duration_ms,
        player_state: state,
        mask,
    }))
}

pub fn emote_stop() -> ClientMessage {
    wrap(Message::EmoteStop(EmoteStop {}))
}

pub fn resync(subject_id: u32, known_sequence: u32) -> ClientMessage {
    wrap(Message::Resync(ResyncRequest {
        subject_id,
        known_seq: known_sequence,
    }))
}

pub fn profile_announcement(version: i32) -> ClientMessage {
    wrap(Message::ProfileAnnouncement(ProfileVersionAnnouncement {
        version,
    }))
}

/// The initial state a handshake may carry: where the player already is.
pub fn initial_state(grid: &Grid, realm: &str, world: [f32; 3]) -> Option<PlayerInitialState> {
    Some(PlayerInitialState {
        state: Some(state_at(grid, world)?),
        emote_id: None,
        emote_duration_ms: None,
        emote_start_offset_ms: None,
        realm: realm.to_owned(),
        emote_mask: None,
    })
}

/// One realm and one rectangle of parcels, as a scene listener's area of interest.
pub fn area_of_interest(realm: &str, min: (i32, i32), max: (i32, i32)) -> SceneListenerAoi {
    SceneListenerAoi {
        realm: realm.to_owned(),
        parcel_rects: vec![ParcelRect {
            min_x: min.0,
            min_z: min.1,
            max_x: max.0,
            max_z: max.1,
        }],
    }
}

pub fn scene_listener_update(area_of_interest: Vec<SceneListenerAoi>) -> ClientMessage {
    wrap(Message::SceneListenerUpdate(SceneListenerUpdate {
        aoi: area_of_interest,
    }))
}

const REASONS: [DisconnectReason; 19] = [
    DisconnectReason::Graceful,
    DisconnectReason::AuthTimeout,
    DisconnectReason::AuthFailed,
    DisconnectReason::DuplicateSession,
    DisconnectReason::Banned,
    DisconnectReason::ServerFull,
    DisconnectReason::PreAuthIpLimitExhausted,
    DisconnectReason::PreAuthBudgetExhausted,
    DisconnectReason::InputRateExceeded,
    DisconnectReason::DiscreteEventRateExceeded,
    DisconnectReason::InvalidInputField,
    DisconnectReason::InvalidEmoteField,
    DisconnectReason::InvalidTeleportField,
    DisconnectReason::HandshakeReplayRejected,
    DisconnectReason::InvalidHandshakeField,
    DisconnectReason::PacketCorrupted,
    DisconnectReason::IpConnectionLimitExceeded,
    DisconnectReason::SceneListenerIpLimitExceeded,
    DisconnectReason::InvalidSceneListenerField,
];

/// A disconnect code as text: `11 (InvalidInputField)`. Code 0 is a drop with no reason given.
pub fn reason(code: u32) -> String {
    match REASONS.iter().find(|reason| reason.code() == code) {
        Some(reason) => format!("{code} ({reason:?})"),
        None if code == 0 => "0 (no reason given)".to_owned(),
        None => format!("{code} (unknown)"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_state_round_trips_within_the_quantization_step() {
        let grid = Grid::default();
        let world = [-1651.3, 2.25, 87.9];
        let back = world_of(&grid, &state_at(&grid, world).unwrap());
        // 8 bits over 16 m, 13 bits over 200 m
        assert!((back[0] - world[0]).abs() < 0.07, "{back:?}");
        assert!((back[1] - world[1]).abs() < 0.03, "{back:?}");
        assert!((back[2] - world[2]).abs() < 0.07, "{back:?}");
    }

    #[test]
    fn outside_the_grid_there_is_no_valid_message() {
        let grid = Grid::default();
        assert!(input_at(&grid, [1e6, 0.0, 0.0]).is_none());
        assert!(teleport(&grid, "realm", [0.0, 0.0, -1e6]).is_none());
    }

    #[test]
    fn every_reason_has_its_name() {
        assert_eq!(reason(11), "11 (InvalidInputField)");
        assert_eq!(reason(4), "4 (DuplicateSession)");
        assert_eq!(reason(0), "0 (no reason given)");
        assert_eq!(reason(999), "999 (unknown)");
        for (position, known) in REASONS.iter().enumerate() {
            assert_eq!(known.code() as usize, position + 1);
        }
    }
}
