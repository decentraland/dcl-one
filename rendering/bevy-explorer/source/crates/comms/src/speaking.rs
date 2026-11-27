//! Who is speaking, as a marker on the player: what draws a player (a nametag's voice badge)
//! reads it there rather than following the rooms' participants.

use bevy::prelude::*;

/// On a player whose voice is active in a room the local user is in.
#[derive(Component, Clone, Copy, PartialEq, Eq, Debug)]
pub struct Speaking {
    /// The voice is in a private call or community stream, which a nearby mute leaves audible.
    pub in_call: bool,
}

pub struct SpeakingPlugin;

impl Plugin for SpeakingPlugin {
    fn build(&self, _app: &mut App) {
        #[cfg(feature = "livekit")]
        _app.add_systems(Update, livekit::mark_speaking_players);
    }
}

#[cfg(feature = "livekit")]
mod livekit {
    use alloy_core::primitives::Address;
    use bevy::{ecs::relationship::Relationship, platform::collections::HashMap, prelude::*};
    use common::{
        structs::{PlayerAddress, PrimaryUser},
        util::AsH160,
    };

    use super::Speaking;
    use crate::{
        global_crdt::ForeignPlayer,
        livekit::participant::{ActiveSpeaker, HostedBy, LivekitParticipant},
        voice_chat::VoiceRoom,
    };

    /// Derived from every room's active speakers at once: a wallet can be in several rooms, and
    /// leaving one must not clear what another still reports.
    pub fn mark_speaking_players(
        mut commands: Commands,
        speakers: Query<(&LivekitParticipant, Option<&HostedBy>), With<ActiveSpeaker>>,
        voice_rooms: Query<(), With<VoiceRoom>>,
        players: Query<
            (Entity, &PlayerAddress, Option<&Speaking>),
            Or<(With<PrimaryUser>, With<ForeignPlayer>)>,
        >,
    ) {
        let mut speaking = HashMap::<Address, bool>::new();
        for (participant, room) in &speakers {
            if let Some(address) = participant.identity().as_str().as_h160() {
                *speaking.entry(address).or_default() |=
                    room.is_some_and(|room| voice_rooms.contains(room.get()));
            }
        }

        for (entity, &PlayerAddress(address), current) in &players {
            match (speaking.get(&address), current) {
                (Some(&in_call), current) if current != Some(&Speaking { in_call }) => {
                    commands.entity(entity).try_insert(Speaking { in_call });
                }
                (None, Some(_)) => {
                    commands.entity(entity).try_remove::<Speaking>();
                }
                _ => {}
            }
        }
    }
}
