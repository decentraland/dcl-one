pub(super) mod plugin;

pub use plugin::{ConnectFailed, ServerRoomTerminal};

use bevy::{
    ecs::{component::HookContext, world::DeferredWorld},
    platform::{collections::HashMap, sync::Arc},
    prelude::*,
};
use common::debug_panic;
#[cfg(not(target_arch = "wasm32"))]
use livekit::{Room, RoomEvent, RoomResult};
use tokio::{sync::mpsc, task::JoinHandle};

#[cfg(target_arch = "wasm32")]
use crate::livekit::web::{Room, RoomEvent, RoomResult};
use crate::livekit::{
    participant::{HostingParticipants, LivekitParticipant, ParticipantDisconnected},
    LivekitTransport,
};

#[derive(Component, Deref)]
#[require(UnannouncedParticipants)]
pub struct LivekitRoom {
    #[deref]
    room: Arc<Room>,
    room_event_receiver: mpsc::UnboundedReceiver<RoomEvent>,
    /// The SDK reported this room disconnected (gave up reconnecting); it is closed already.
    closed: bool,
}

/// Senders whose data reached the room before LiveKit announced them, by identity, with the
/// elapsed time by which the announcement must have come. Their presence was created from the
/// data alone; if they vanish before being announced no `ParticipantDisconnected` follows, so
/// expired entries are withdrawn by hand. The room's disconnect expires them all.
#[derive(Component, Default, Deref, DerefMut)]
pub struct UnannouncedParticipants(HashMap<String, f64>);

/// Marks that a [`LivekitRoom`] as connected
#[derive(Component)]
#[component(on_add=Self::on_add)]
pub struct Connected;

impl Connected {
    pub fn on_add(mut deferred_world: DeferredWorld, hook_context: HookContext) {
        let entity = hook_context.entity;
        let Some(room) = deferred_world.entity(entity).get::<LivekitRoom>() else {
            debug_panic!("Connected room {entity} did not have LivekitRoom.");
        };
        debug!("Room {} connected.", room.name());

        deferred_world
            .commands()
            .entity(entity)
            .remove::<(Connecting, Reconnecting, Disconnected)>();
    }
}

/// Marks that a [`LivekitRoom`] is connecting
#[derive(Component)]
#[component(on_add=Self::on_add, on_remove=Self::on_remove)]
pub struct Connecting;

impl Connecting {
    pub fn on_add(mut deferred_world: DeferredWorld, hook_context: HookContext) {
        let entity = hook_context.entity;
        let Some(transport) = deferred_world.entity(entity).get::<LivekitTransport>() else {
            debug_panic!("Connecting room {entity} did not have LivekitTransport.");
        };
        debug!("Room {} connecting.", transport.address);

        deferred_world
            .commands()
            .entity(entity)
            .remove::<(Connected, Reconnecting, Disconnected)>();
    }

    pub fn on_remove(mut deferred_world: DeferredWorld, hook_context: HookContext) {
        let entity = hook_context.entity;

        // This hook will also run on despawn
        // so `try_remove` is used
        deferred_world
            .commands()
            .entity(entity)
            .try_remove::<ConnectingLivekitRoom>();
    }
}

/// Marks that a [`LivekitRoom`] as
/// attempting to reconnect
#[derive(Component)]
#[component(on_add=Self::on_add, on_remove=Self::on_remove)]
pub struct Reconnecting;

impl Reconnecting {
    pub fn on_add(mut deferred_world: DeferredWorld, hook_context: HookContext) {
        let entity = hook_context.entity;
        let Some(room) = deferred_world.entity(entity).get::<LivekitRoom>() else {
            debug_panic!("Reconnecting room {entity} did not have LivekitRoom.");
        };
        debug!("Room {} is reconnecting.", room.name());

        deferred_world
            .commands()
            .entity(entity)
            .remove::<(Connected, Connecting, Disconnected)>();
    }

    pub fn on_remove(mut deferred_world: DeferredWorld, hook_context: HookContext) {
        let entity = hook_context.entity;

        // This hook will also run on despawn
        // so `try_remove` is used
        deferred_world
            .commands()
            .entity(entity)
            .try_remove::<ConnectingLivekitRoom>();
    }
}

/// Marks that a [`LivekitRoom`] as
/// attempting to disconnected
#[derive(Component)]
#[component(on_add=Self::on_add, on_remove=Self::on_remove)]
pub struct Disconnected;

impl Disconnected {
    pub fn on_add(mut deferred_world: DeferredWorld, hook_context: HookContext) {
        let entity = hook_context.entity;
        let Some(room) = deferred_world.entity(entity).get::<LivekitRoom>() else {
            debug_panic!("Disconnected room {entity} did not have LivekitRoom.");
        };
        let name = room.name();

        // The room's participants are gone with it; a reconnect announces them afresh. They
        // leave through the normal path so each player's membership in this transport is
        // withdrawn: a player who does not come back must not keep it.
        let participants = deferred_world
            .get::<HostingParticipants>(entity)
            .map(|hosting| hosting.collection().clone())
            .into_iter()
            .flatten()
            .filter_map(|participant| deferred_world.get::<LivekitParticipant>(participant))
            .cloned()
            .collect::<Vec<_>>();
        info!(
            "Room {name} disconnected; retiring {} participants.",
            participants.len()
        );
        // senders never announced have no participant to retire: expire their presence
        if let Some(mut unannounced) = deferred_world.get_mut::<UnannouncedParticipants>(entity) {
            for deadline in unannounced.values_mut() {
                *deadline = 0.0;
            }
        }
        let mut commands = deferred_world.commands();
        commands
            .entity(entity)
            .remove::<(Connected, Connecting, Reconnecting)>();
        for participant in participants {
            commands.trigger(ParticipantDisconnected {
                participant,
                room: entity,
            });
        }
    }

    pub fn on_remove(mut deferred_world: DeferredWorld, hook_context: HookContext) {
        let entity = hook_context.entity;

        // This hook will also run on despawn
        // so `try_remove` is used
        deferred_world
            .commands()
            .entity(entity)
            .try_remove::<ConnectingLivekitRoom>();
    }
}

#[derive(Component, Deref, DerefMut)]
struct ConnectingLivekitRoom(JoinHandle<RoomResult<(Room, mpsc::UnboundedReceiver<RoomEvent>)>>);

impl Drop for ConnectingLivekitRoom {
    fn drop(&mut self) {
        self.0.abort()
    }
}
