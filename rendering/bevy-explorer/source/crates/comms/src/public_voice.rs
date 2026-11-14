//! Read-only public speaking state, sampled from the room's authoritative participants.
#[cfg(feature = "livekit")]
use crate::{
    livekit::participant::{ActiveSpeaker, HostedBy, ParticipantIndex},
    private_chat::PrivateChatRoom,
    voice_chat::VoiceRoom,
    SceneRoom, Transport,
};
use bevy::{
    ecs::system::SystemParam,
    platform::collections::{HashMap, HashSet},
    prelude::*,
};

#[derive(Default)]
pub struct PublicVoiceSnapshot(HashMap<Entity, HashMap<String, HashSet<String>>>);

impl PublicVoiceSnapshot {
    pub fn is_speaking(&self, address: &str, scene: &str, context: Entity, shared: bool) -> bool {
        self.0.get(&context).is_some_and(|channels| {
            channels
                .get(scene)
                .is_some_and(|speakers| speakers.contains(address))
                || (shared
                    && channels
                        .get("")
                        .is_some_and(|speakers| speakers.contains(address)))
        })
    }
}

#[derive(SystemParam)]
#[allow(clippy::type_complexity)]
pub struct PublicVoiceParticipants<'w, 's> {
    #[cfg(feature = "livekit")]
    rooms: Query<
        'w,
        's,
        (
            Entity,
            &'static ParticipantIndex,
            &'static Transport,
            Option<&'static SceneRoom>,
        ),
        (Without<VoiceRoom>, Without<PrivateChatRoom>),
    >,
    #[cfg(feature = "livekit")]
    speakers: Query<'w, 's, &'static HostedBy, With<ActiveSpeaker>>,
    #[cfg(not(feature = "livekit"))]
    _entities: Query<'w, 's, Entity>,
}

impl PublicVoiceParticipants<'_, '_> {
    pub fn snapshot(&self) -> PublicVoiceSnapshot {
        #[cfg(not(feature = "livekit"))]
        {
            PublicVoiceSnapshot::default()
        }
        #[cfg(feature = "livekit")]
        {
            use bevy::ecs::relationship::Relationship;
            let mut snapshot = PublicVoiceSnapshot::default();
            for (room, index, transport, scene) in &self.rooms {
                for (address, participant) in index.iter() {
                    if self
                        .speakers
                        .get(*participant)
                        .is_ok_and(|host| host.get() == room)
                    {
                        snapshot
                            .0
                            .entry(transport.context)
                            .or_default()
                            .entry(scene.map(|scene| scene.0.clone()).unwrap_or_default())
                            .or_default()
                            .insert(address.clone());
                    }
                }
            }
            snapshot
        }
    }
}
