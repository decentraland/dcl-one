//! Multiplayer connectivity watchdog. Its policy uses injected time and probe results.

#[derive(Default)]
struct Watchdog {
    ever_connected: bool,
    next_poll: f64,
    lost_since: Option<f64>,
    probe_deadline: Option<f64>,
    connection_lost: bool,
}

impl Watchdog {
    /// Returns true only when a new reachability probe should start.
    fn sample(&mut self, now: f64, healthy: bool, suppressed: bool) -> bool {
        if healthy || suppressed {
            self.ever_connected |= healthy;
            self.lost_since = None;
            self.probe_deadline = None;
            self.connection_lost = false;
            self.next_poll = now + 3.0;
            return false;
        }
        if self.probe_deadline.is_some_and(|deadline| now >= deadline) {
            self.finish_probe(false);
            self.next_poll = now + 3.0;
        }
        if !self.ever_connected || now < self.next_poll {
            return false;
        }
        self.next_poll = now + 3.0;
        let lost = *self.lost_since.get_or_insert(now);
        if now - lost < 4.0 || self.probe_deadline.is_some() {
            return false;
        }
        self.probe_deadline = Some(now + 5.0);
        true
    }

    fn finish_probe(&mut self, reachable: bool) {
        // Results from a cancelled probe cannot resurrect a dismissed dialog.
        if self.probe_deadline.take().is_some() {
            self.connection_lost = !reachable;
        }
    }

    fn retry(&mut self, now: f64) {
        self.connection_lost = false;
        self.next_poll = now;
        if self.ever_connected {
            self.lost_since = Some(now - 4.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn startup_never_probes_without_previous_multiplayer_connection() {
        let mut watchdog = Watchdog::default();
        for now in [0.0, 3.0, 9.0, 30.0] {
            assert!(!watchdog.sample(now, false, false));
        }
        assert!(!watchdog.connection_lost);
    }

    #[test]
    fn polls_every_three_seconds_and_requires_four_second_grace() {
        let mut watchdog = Watchdog::default();
        watchdog.sample(0.0, true, false);
        assert!(!watchdog.sample(2.9, false, false));
        assert!(!watchdog.sample(3.0, false, false));
        assert!(!watchdog.sample(6.0, false, false));
        assert!(watchdog.sample(9.0, false, false));
        assert_eq!(watchdog.probe_deadline, Some(14.0));
        assert!(!watchdog.sample(12.0, false, false));
        assert!(!watchdog.connection_lost);
        watchdog.sample(14.0, false, false);
        assert!(watchdog.connection_lost);
    }

    #[test]
    fn successful_probe_means_degraded_rooms_and_failed_probe_means_offline() {
        let mut watchdog = lost();
        watchdog.finish_probe(true);
        assert!(!watchdog.connection_lost);
        assert!(watchdog.sample(12.0, false, false));
        watchdog.finish_probe(false);
        assert!(watchdog.connection_lost);
        assert!(!watchdog.sample(13.0, false, false));
        assert!(watchdog.sample(15.0, false, false));
        watchdog.finish_probe(true);
        assert!(!watchdog.connection_lost);
    }

    #[test]
    fn health_or_suppression_cancels_probe_and_stale_result_cannot_open_dialog() {
        for (healthy, suppressed) in [(true, false), (false, true)] {
            let mut watchdog = lost();
            watchdog.sample(9.1, healthy, suppressed);
            watchdog.finish_probe(false);
            assert!(!watchdog.connection_lost);
            assert!(watchdog.probe_deadline.is_none());
        }
    }

    #[test]
    fn retry_requests_one_immediate_probe_and_recovery_dismisses_dialog() {
        let mut watchdog = lost();
        watchdog.finish_probe(false);
        assert!(watchdog.connection_lost);
        watchdog.retry(10.0);
        assert!(!watchdog.connection_lost);
        assert!(watchdog.sample(10.0, false, false));
        assert!(!watchdog.sample(10.1, false, false));
        watchdog.sample(10.2, true, false);
        assert!(!watchdog.connection_lost);
        watchdog.finish_probe(false);
        assert!(!watchdog.connection_lost);
    }

    #[test]
    fn failed_manual_retry_restores_the_offline_warning() {
        let mut watchdog = lost();
        watchdog.finish_probe(false);
        watchdog.retry(10.0);
        assert!(!watchdog.connection_lost);
        assert!(watchdog.sample(10.0, false, false));
        watchdog.finish_probe(false);
        assert!(watchdog.connection_lost);
    }

    #[test]
    fn a_completed_probe_is_collected_before_its_deadline_is_sampled() {
        let mut app = App::new();
        app.insert_resource(ConnectivityMonitor(lost()));
        let task = IoTaskPool::get_or_init(bevy::tasks::TaskPool::new).spawn(async { true });
        while !task.is_finished() {
            std::thread::yield_now();
        }
        let probe = app.world_mut().spawn(ReachabilityProbe(task)).id();
        app.add_systems(
            Update,
            (
                collect_probes,
                |mut monitor: ResMut<ConnectivityMonitor>| {
                    monitor.0.sample(14.0, false, false);
                },
            )
                .chain(),
        );
        app.update();
        assert!(
            !app.world()
                .resource::<ConnectivityMonitor>()
                .0
                .connection_lost
        );
        assert!(app.world().get_entity(probe).is_err());
    }

    fn lost() -> Watchdog {
        let mut watchdog = Watchdog::default();
        watchdog.sample(0.0, true, false);
        watchdog.sample(3.0, false, false);
        watchdog.sample(6.0, false, false);
        assert!(watchdog.sample(9.0, false, false));
        watchdog
    }
}

// Engine integration. The policy above also runs in a small dependency-free test harness.
use bevy::{
    prelude::*,
    tasks::{IoTaskPool, Task},
};
use bevy_console::ConsoleCommand;
use common::{
    base_domain::{self, Service},
    rpc::RpcStreamSender,
    structs::{CurrentRealm, PreviewMode, WorldHold},
    util::{TaskCompat, TaskExt},
};
use console::{AddDebugSection, DebugSection, DoAddConsoleCommand};
use ipfs::IpfsAssetServer;
use serde::Serialize;
use system_bridge::{ConnectivityRoomStatus, ConnectivitySnapshot, SystemApi};
use wallet::Wallet;

use crate::{
    access_status::AccessScope,
    archipelago::IslandOwner,
    pulse::plugin::PulseSession,
    room_connection::{
        FailureKind, RealmRoom, RoomAccessScope, RoomRecovery, RoomState, RoomStatus, RoomSuspended,
    },
    DisableRealmComms, SceneRoom,
};

#[derive(Resource, Default)]
struct ConnectivityMonitor(Watchdog);

#[derive(Resource, Default, Deref, DerefMut, Serialize)]
#[serde(transparent)]
struct ConnectivityStatus(ConnectivitySnapshot);

#[derive(Component)]
struct ConnectivitySubscriber(RpcStreamSender<ConnectivitySnapshot>);

#[derive(Component)]
struct ReachabilityProbe(Task<bool>);

pub struct ConnectivityPlugin;

impl Plugin for ConnectivityPlugin {
    fn build(&self, app: &mut App) {
        if common::structs::server_mode() {
            return;
        }
        app.init_resource::<ConnectivityMonitor>()
            .init_resource::<ConnectivityStatus>()
            .add_systems(
                Update,
                (
                    retry_connectivity,
                    collect_probes,
                    sample_connectivity.pipe(start_probe),
                    publish_connectivity,
                )
                    .chain(),
            )
            .add_console_command::<ConnectivityState, _>(connectivity_state)
            .add_console_command::<ConnectivityRetry, _>(connectivity_retry)
            .add_debug_section(DebugSection {
                title: "Connectivity",
                query: "/connectivity_state",
                fields: &[
                    ("Connection lost", "connectionLost"),
                    ("Reachability check", "probing"),
                    ("Previously connected", "everConnected"),
                    ("Offline warning suppressed", "suppressed"),
                    ("Room status", "rooms"),
                ],
                actions: &[("Retry connections", "/connectivity_retry")],
                inputs: &[],
            });
    }
}

fn publish_connectivity(
    mut commands: Commands,
    mut events: EventReader<SystemApi>,
    status: Res<ConnectivityStatus>,
    subscribers: Query<(Entity, &ConnectivitySubscriber)>,
    mut previous: Local<Option<ConnectivitySnapshot>>,
) {
    let changed = previous.as_ref() != Some(&status.0);
    for (entity, subscriber) in &subscribers {
        if subscriber.0.is_closed() || (changed && subscriber.0.send(status.0.clone()).is_err()) {
            commands.entity(entity).try_despawn();
        }
    }
    for event in events.read() {
        if let SystemApi::GetConnectivityStream(sender) = event {
            if !sender.is_closed() && sender.send(status.0.clone()).is_ok() {
                commands.spawn(ConnectivitySubscriber(sender.clone()));
            }
        }
    }
    if changed {
        *previous = Some(status.0.clone());
    }
}

#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/connectivity_state")]
struct ConnectivityState;

fn connectivity_state(
    mut input: ConsoleCommand<ConnectivityState>,
    state: Res<ConnectivityStatus>,
) {
    if let Some(Ok(_)) = input.take() {
        input.reply_ok(serde_json::to_string(state.as_ref()).unwrap_or_default());
    }
}

#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/connectivity_retry")]
struct ConnectivityRetry;

fn connectivity_retry(
    mut input: ConsoleCommand<ConnectivityRetry>,
    mut events: EventWriter<SystemApi>,
) {
    if let Some(Ok(_)) = input.take() {
        events.write(SystemApi::RetryConnectivity);
        input.reply_ok("Recovery requested");
    }
}

fn retry_connectivity(
    mut events: EventReader<SystemApi>,
    time: Res<Time>,
    mut monitor: ResMut<ConnectivityMonitor>,
    mut rooms: Query<&mut RoomRecovery, Without<RoomSuspended>>,
) {
    if events
        .read()
        .any(|event| matches!(event, SystemApi::RetryConnectivity))
    {
        monitor.0.retry(time.elapsed_secs_f64());
        for mut recovery in &mut rooms {
            // Retry never overrides a terminal admission decision or deliberate pause.
            if recovery
                .failure
                .is_none_or(|kind| kind == FailureKind::Transient)
            {
                recovery.retry.due = time.elapsed_secs_f64();
            }
        }
    }
}

fn collect_probes(
    mut commands: Commands,
    mut probes: Query<(Entity, &mut ReachabilityProbe)>,
    mut monitor: ResMut<ConnectivityMonitor>,
) {
    // A response already available at the deadline wins over the timeout.
    for (entity, mut probe) in &mut probes {
        if monitor.0.probe_deadline.is_none() {
            commands.entity(entity).try_despawn();
        } else if let Some(reachable) = probe.0.complete() {
            monitor.0.finish_probe(reachable);
            commands.entity(entity).try_despawn();
        }
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn sample_connectivity(
    time: Res<Time>,
    wallet: Res<Wallet>,
    realm: Res<CurrentRealm>,
    preview: Res<PreviewMode>,
    hold: Option<Res<WorldHold>>,
    disabled: Option<Res<DisableRealmComms>>,
    pulse: Option<Res<PulseSession>>,
    rooms: Query<(
        RoomStatus,
        Option<&RoomAccessScope>,
        Has<RealmRoom>,
        Has<SceneRoom>,
        Has<IslandOwner>,
    )>,
    mut monitor: ResMut<ConnectivityMonitor>,
    mut status: ResMut<ConnectivityStatus>,
) -> bool {
    let now = time.elapsed_secs_f64();
    let (pulse_healthy, pulse_suppressed) = pulse
        .as_deref()
        .map(PulseSession::connectivity)
        .unwrap_or_default();
    let mut healthy = pulse_healthy;
    let mut required = pulse.is_some();
    let mut room_count = 0;
    for (room, scope, realm_room, scene_room, island) in &rooms {
        if !realm_room && !scene_room && !island && scope.is_none() {
            continue;
        }
        let state = room.state();
        let room_healthy = state == RoomState::Connected;
        let terminal = room
            .recovery
            .failure
            .is_some_and(|kind| kind != FailureKind::Transient);
        let monitored = (realm_room || scene_room || island) && !room.suspended && !terminal;
        required |= monitored;
        healthy |= monitored && room_healthy;
        let scope = match scope.map(|scope| &scope.0) {
            Some(AccessScope::Scene(_)) => "scene",
            Some(AccessScope::PrivateChat) => "private_chat",
            _ if scene_room => "scene",
            _ => "realm",
        };
        // No adapter, realm URL, scene id, session id, or server error enters the HUD.
        let retry_at = matches!(
            state,
            RoomState::Reconnecting | RoomState::Disconnected | RoomState::Failed
        )
        .then_some(room.recovery.retry.due);
        if let Some(row) = status.rooms.get_mut(room_count) {
            if row.scope != scope {
                row.scope.clear();
                row.scope.push_str(scope);
            }
            if row.state != state.name() {
                row.state.clear();
                row.state.push_str(state.name());
            }
            row.retry_at = retry_at;
        } else {
            status.rooms.push(ConnectivityRoomStatus {
                scope: scope.to_owned(),
                state: state.name().to_owned(),
                retry_at,
            });
        }
        room_count += 1;
    }
    status.rooms.truncate(room_count);
    let offline = realm
        .comms
        .as_ref()
        .and_then(|comms| comms.fixed_adapter.as_deref().or(comms.adapter.as_deref()))
        .is_some_and(|adapter| {
            crate::adapter::parse_adapter(adapter)
                .is_ok_and(|(kind, _)| kind == crate::adapter::AdapterKind::Offline)
        });
    let suppressed = !required
        || wallet.address().is_none()
        || preview.is_preview
        || realm.is_local()
        || hold.is_some()
        || disabled.is_some_and(|disabled| disabled.0)
        || offline
        || pulse_suppressed;
    let start = monitor.0.sample(now, healthy, suppressed);
    status.connection_lost = monitor.0.connection_lost;
    status.probing = monitor.0.probe_deadline.is_some();
    status.ever_connected = monitor.0.ever_connected;
    status.suppressed = suppressed;
    status.rooms.sort_unstable_by(|a, b| {
        a.scope
            .cmp(&b.scope)
            .then_with(|| a.state.cmp(&b.state))
            .then_with(|| {
                a.retry_at
                    .partial_cmp(&b.retry_at)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    });
    start
}

fn start_probe(In(start): In<bool>, mut commands: Commands, ipfs: IpfsAssetServer) {
    if start {
        let client = ipfs.ipfs().client();
        let url = base_domain::url(Service::Catalyst, "/about");
        commands.spawn(ReachabilityProbe(IoTaskPool::get().spawn_compat(
            async move {
                // Any HTTP response proves reachability; authorization and service failures are
                // room degradation, not evidence that the player's network is offline.
                client
                    .get(url)
                    .timeout(std::time::Duration::from_secs(5))
                    .send()
                    .await
                    .is_ok()
            },
        )));
    }
}

#[cfg(test)]
mod stream_tests {
    use super::*;

    fn app() -> App {
        let mut app = App::new();
        app.add_event::<SystemApi>()
            .init_resource::<ConnectivityStatus>()
            .add_systems(Update, publish_connectivity);
        app
    }

    #[test]
    fn connectivity_stream_sends_initial_and_changed_snapshots_once() {
        let mut app = app();
        let (first, mut first_rx) = RpcStreamSender::channel();
        app.world_mut()
            .send_event(SystemApi::GetConnectivityStream(first));
        app.update();
        assert_eq!(
            first_rx.try_recv().unwrap(),
            ConnectivitySnapshot::default()
        );
        app.update();
        assert!(first_rx.try_recv().is_err());

        let expected = ConnectivitySnapshot {
            connection_lost: true,
            rooms: vec![ConnectivityRoomStatus {
                scope: "realm".to_owned(),
                state: "reconnecting".to_owned(),
                retry_at: Some(10.0),
            }],
            ..Default::default()
        };
        app.world_mut().resource_mut::<ConnectivityStatus>().0 = expected.clone();
        let (second, mut second_rx) = RpcStreamSender::channel();
        app.world_mut()
            .send_event(SystemApi::GetConnectivityStream(second));
        app.update();
        assert_eq!(first_rx.try_recv().unwrap(), expected);
        assert_eq!(second_rx.try_recv().unwrap(), expected);
        // Writing an equal Resource value is not a stream transition.
        app.world_mut().resource_mut::<ConnectivityStatus>().0 = expected;
        app.update();
        assert!(first_rx.try_recv().is_err());
        assert!(second_rx.try_recv().is_err());
    }

    #[test]
    fn connectivity_stream_removes_closed_consumers_without_waiting_for_a_change() {
        let mut app = app();
        let (sender, receiver) = RpcStreamSender::channel();
        app.world_mut()
            .send_event(SystemApi::GetConnectivityStream(sender));
        app.update();
        drop(receiver);
        app.update();
        let world = app.world_mut();
        assert_eq!(
            world.query::<&ConnectivitySubscriber>().iter(world).count(),
            0
        );

        let (sender, receiver) = RpcStreamSender::channel();
        drop(receiver);
        app.world_mut()
            .send_event(SystemApi::GetConnectivityStream(sender));
        app.update();
        let world = app.world_mut();
        assert_eq!(
            world.query::<&ConnectivitySubscriber>().iter(world).count(),
            0
        );
    }
}

#[cfg(test)]
mod room_tests {
    use super::*;
    use crate::room_connection::ConnectionHealth;
    use bevy::ecs::system::RunSystemOnce;
    use std::time::Duration;

    fn monitor_world() -> World {
        let mut world = World::new();
        let mut wallet = Wallet::default();
        wallet.finalize_as_guest_with_seed([7; 32]);
        world.insert_resource(wallet);
        world.init_resource::<Time>();
        world.init_resource::<CurrentRealm>();
        world.init_resource::<PreviewMode>();
        world.init_resource::<ConnectivityMonitor>();
        world.init_resource::<ConnectivityStatus>();
        world
    }

    #[test]
    fn first_connection_is_not_reported_as_a_reconnection() {
        let mut world = monitor_world();
        world.spawn((SceneRoom("scene".into()), RoomRecovery::default()));
        world.run_system_once(sample_connectivity).unwrap();
        let status = world.resource::<ConnectivityStatus>();
        assert_eq!(status.rooms[0].state, "connecting");
        assert_eq!(status.rooms[0].retry_at, None);
    }

    #[test]
    fn standalone_call_rooms_are_not_reported_as_realm_connections() {
        let mut world = monitor_world();
        world.spawn(RoomRecovery {
            failure: Some(FailureKind::Transient),
            ..Default::default()
        });
        world.spawn((RealmRoom, RoomRecovery::default()));
        world.run_system_once(sample_connectivity).unwrap();
        let status = world.resource::<ConnectivityStatus>();
        assert_eq!(status.rooms.len(), 1);
        assert_eq!(status.rooms[0].scope, "realm");
        assert_eq!(status.rooms[0].state, "connecting");
    }

    #[test]
    fn denied_scene_does_not_mask_loss_of_a_required_room() {
        let mut world = monitor_world();
        let realm_health = ConnectionHealth::default();
        realm_health.connected();
        world.spawn((RealmRoom, RoomRecovery::default(), realm_health.clone()));
        world.spawn((
            SceneRoom("scene".into()),
            RoomRecovery {
                failure: Some(FailureKind::Denied),
                ..Default::default()
            },
        ));

        assert!(!world.run_system_once(sample_connectivity).unwrap());
        let status = world.resource::<ConnectivityStatus>();
        assert!(status.ever_connected);
        assert!(!status.suppressed);
        assert!(status.rooms.iter().any(|room| {
            room.scope == "scene" && room.state == "denied" && room.retry_at.is_none()
        }));

        realm_health.disconnected();
        for delta in [3, 3] {
            world
                .resource_mut::<Time>()
                .advance_by(Duration::from_secs(delta));
            assert!(!world.run_system_once(sample_connectivity).unwrap());
        }
        world
            .resource_mut::<Time>()
            .advance_by(Duration::from_secs(3));
        assert!(world.run_system_once(sample_connectivity).unwrap());
    }

    #[test]
    fn all_terminal_or_suspended_rooms_leave_nothing_to_monitor() {
        let mut world = monitor_world();
        let stale_health = ConnectionHealth::default();
        stale_health.connected();
        world.spawn((
            RealmRoom,
            RoomRecovery {
                failure: Some(FailureKind::Invalid),
                ..Default::default()
            },
            stale_health,
        ));
        world.spawn((
            SceneRoom("scene".into()),
            RoomRecovery::default(),
            RoomSuspended,
        ));

        assert!(!world.run_system_once(sample_connectivity).unwrap());
        let status = world.resource::<ConnectivityStatus>();
        assert!(status.suppressed);
        assert!(!status.ever_connected);
        assert!(status.rooms.iter().any(|room| {
            room.scope == "realm" && room.state == "invalid" && room.retry_at.is_none()
        }));
    }
}
