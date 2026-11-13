mod control;
pub(super) mod diagnostics;
pub(super) mod plugin;

pub use plugin::ServerRoomTerminal;

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
    traffic: crate::traffic::PayloadTraffic,
    /// Keep pending publishes with the SDK connection whose payload they contribute to.
    publishes: Vec<JoinHandle<RoomResult<usize>>>,
    _lifetime: RoomLifetime,
}

/// Starts at successful SDK admission, before the ECS claims the connection.
/// This also closes results discarded by an aborted task or a stale insert command.
struct RoomLifetime(Option<Box<dyn FnOnce() + Send + Sync>>);

impl Drop for RoomLifetime {
    fn drop(&mut self) {
        if let Some(close) = self.0.take() {
            close();
        }
    }
}

impl LivekitRoom {
    fn new(room: Room, room_event_receiver: mpsc::UnboundedReceiver<RoomEvent>) -> Self {
        let room = Arc::new(room);
        let closing_room = room.clone();
        #[cfg(not(target_arch = "wasm32"))]
        let runtime = tokio::runtime::Handle::current();
        let lifetime = RoomLifetime(Some(Box::new(move || {
            #[cfg(not(target_arch = "wasm32"))]
            runtime.spawn(async move {
                let _ = closing_room.close().await;
            });
            #[cfg(target_arch = "wasm32")]
            closing_room.dispose();
        })));
        Self {
            room,
            room_event_receiver,
            closed: false,
            traffic: Default::default(),
            publishes: Vec::new(),
            _lifetime: lifetime,
        }
    }
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
        if !deferred_world.entity(entity).contains::<LivekitTransport>() {
            debug_panic!("Connecting room {entity} did not have LivekitTransport.");
        }
        debug!("Room {entity} connecting.");

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
struct ConnectingLivekitRoom(
    JoinHandle<Result<LivekitRoom, crate::room_connection::AdmissionError>>,
);

impl Drop for ConnectingLivekitRoom {
    fn drop(&mut self) {
        self.0.abort()
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod lifetime_tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn aborting_a_completed_unclaimed_connection_runs_cleanup_once() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        let closed = Arc::new(AtomicUsize::new(0));
        let closing = closed.clone();
        let task = runtime.spawn(async move {
            RoomLifetime(Some(Box::new(move || {
                closing.fetch_add(1, Ordering::SeqCst);
            })))
        });
        runtime.block_on(async {
            while !task.is_finished() {
                tokio::task::yield_now().await;
            }
        });
        task.abort();
        drop(task);
        assert_eq!(closed.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn claiming_a_connection_transfers_cleanup_to_the_ecs_owner() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        let closed = Arc::new(AtomicUsize::new(0));
        let closing = closed.clone();
        let task = runtime.spawn(async move {
            RoomLifetime(Some(Box::new(move || {
                closing.fetch_add(1, Ordering::SeqCst);
            })))
        });
        let owner = runtime.block_on(task).unwrap();
        assert_eq!(closed.load(Ordering::SeqCst), 0);
        drop(owner);
        assert_eq!(closed.load(Ordering::SeqCst), 1);
    }
}
