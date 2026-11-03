use super::*;
use bevy_console::ConsoleCommand;
use common::util::AsH160;
use console::{AddDebugSection, DebugInput, DebugInputKind, DebugSection, DoAddConsoleCommand};
use serde_json::json;

pub(super) fn register(app: &mut App) {
    let client = !app
        .world()
        .contains_resource::<common::structs::NoRenderApp>()
        && !common::structs::server_mode();
    if client {
        app.add_console_command::<PulseConnection, _>(pulse_connection);
    }
    app.init_resource::<PeerLookup>()
        .add_console_command::<PulseState, _>(pulse_state)
        .add_console_command::<PulseLookup, _>(pulse_lookup)
        .add_debug_section(DebugSection {
            title: "Pulse",
            query: "/pulse_state",
            fields: &[
                ("Transport state", "state"),
                ("Authenticated", "authenticated"),
                ("Outbound payload (B/s)", "sentBytes"),
                ("Inbound payload (B/s)", "receivedBytes"),
                ("Outbound packets/s", "sentPackets"),
                ("Inbound packets/s", "receivedPackets"),
                ("Resync requests (last 60s)", "resyncs"),
                ("Emote for selected peer", "emote"),
            ],
            actions: if client {
                &[
                    ("Connect", "/pulse_connection connect"),
                    ("Disconnect", "/pulse_connection disconnect"),
                ]
            } else {
                &[]
            },
            inputs: &[DebugInput {
                label: "Look up peer by wallet",
                field: "wallet",
                command: "/pulse_lookup {value}",
                kind: DebugInputKind::Text,
            }],
        });
}

#[derive(Resource, Default)]
struct PeerLookup(Option<Address>);

/// Select the remote wallet whose Pulse emote stream to inspect.
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/pulse_lookup")]
struct PulseLookup {
    wallet: String,
}

fn pulse_lookup(mut input: ConsoleCommand<PulseLookup>, mut lookup: ResMut<PeerLookup>) {
    if let Some(Ok(command)) = input.take() {
        let Some(wallet) = command.wallet.as_h160() else {
            input.reply_failed("Expected a wallet address");
            return;
        };
        lookup.0 = Some(wallet);
        input.reply_ok(format!("Tracking {wallet:#x}"));
    }
}

/// Read the connection lifecycle, protocol payload rates and decoder state.
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/pulse_state")]
struct PulseState;

fn pulse_state(
    mut input: ConsoleCommand<PulseState>,
    session: Option<Res<PulseSession>>,
    lookup: Res<PeerLookup>,
    time: Res<Time>,
) {
    if let Some(Ok(_)) = input.take() {
        let state = match session.as_deref().map(|s| &s.state) {
            None => "unconfigured",
            Some(Connection::Down { .. }) => "waiting",
            Some(Connection::Paused) => "paused",
            Some(Connection::Connecting) => "connecting",
            Some(Connection::Idle { .. }) => "connected",
            Some(Connection::Signing(_)) => "signing",
            Some(Connection::AwaitingResponse { .. }) => "authenticating",
            Some(Connection::Established) => "authenticated",
            Some(Connection::Dead) => "terminal disconnect",
        };
        let link = session.as_ref().and_then(|s| s.link.as_ref());
        let sent = link.map(|link| link.sent.last_second()).unwrap_or_default();
        let received = link
            .map(|link| link.received.last_second())
            .unwrap_or_default();
        let second = time.elapsed_secs_f64() as u64;
        let resyncs = session
            .as_ref()
            .map(|session| {
                session
                    .resyncs
                    .iter()
                    .filter(|(stamp, _)| second.saturating_sub(*stamp) < 60)
                    .map(|(_, count)| u64::from(*count))
                    .sum::<u64>()
            })
            .unwrap_or_default();
        let emote = lookup.0.map(|wallet| {
            session
                .as_ref()
                .and_then(|s| {
                    s.decoder
                        .subjects
                        .values()
                        .find(|peer| peer.wallet == wallet)
                })
                .map(|peer| peer.active_emote.as_deref().unwrap_or("not emoting"))
                .unwrap_or("peer not present")
        });
        input.reply_ok(json!({
            "state": state, "authenticated": matches!(session.as_deref().map(|s| &s.state), Some(Connection::Established)),
            "sentBytes": sent.bytes, "sentPackets": sent.packets, "receivedBytes": received.bytes,
            "receivedPackets": received.packets, "resyncs": resyncs,
            "wallet": lookup.0.map(|wallet| format!("{wallet:#x}")), "emote": emote,
            "unavailable": {"emote": "Select a peer wallet to inspect its emote"},
        }).to_string());
    }
}

#[derive(clap::ValueEnum, Clone)]
enum Operation {
    Connect,
    Disconnect,
}

/// Pause or resume the player connection. Terminal disconnects still require a fresh session.
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/pulse_connection")]
struct PulseConnection {
    operation: Operation,
}

fn pulse_connection(
    mut input: ConsoleCommand<PulseConnection>,
    session: Option<ResMut<PulseSession>>,
    mut commands: Commands,
) {
    let Some(Ok(command)) = input.take() else {
        return;
    };
    let Some(mut session) = session else {
        input.reply_failed("Pulse is not configured");
        return;
    };
    if !matches!(session.role, PulseRole::Player(_)) || matches!(session.state, Connection::Dead) {
        input.reply_failed("This session cannot be controlled");
        return;
    }
    match command.operation {
        Operation::Disconnect => {
            if let Some(transport) = session
                .role
                .player_mut()
                .and_then(|p| p.routing_transport.take())
            {
                commands.entity(transport).try_despawn();
            }
            session.link = None;
            session._driver = None;
            session.decoder.reset();
            session.last_state = None;
            session.state = Connection::Paused;
            input.reply_ok("Pulse paused");
        }
        Operation::Connect => {
            if matches!(session.state, Connection::Paused) {
                session.state = Connection::Down { respawn_at: 0.0 };
            }
            input.reply_ok("Pulse connection enabled");
        }
    }
}
