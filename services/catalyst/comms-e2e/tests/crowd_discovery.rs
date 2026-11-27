//! The crowd that finds each other as clients do: the parts that run without a stack. The
//! archipelago bot against a connector played by this test (what it records of an island
//! assignment, and the island roster it keeps), and the verdicts the scenario gives on what the
//! clients saw.
#![allow(clippy::unwrap_used, reason = "test support")]

use std::sync::Arc;
use std::time::Duration;

use catalyst_comms::protocol::archipelago::Position;
use catalyst_comms::protocol::archipelago::archipelago::{
    ChallengeResponseMessage, ClientPacket, IslandChangedMessage, JoinIslandMessage,
    LeftIslandMessage, ServerPacket, WelcomeMessage, client_packet, server_packet,
};
use catalyst_comms_e2e::archipelago::{ArchipelagoBot, ArchipelagoOptions};
use catalyst_comms_e2e::engine::{
    CrowdSize, Ctx, Outcome, RunOptions, Transport, expect_or, run_scenarios,
};
use catalyst_comms_e2e::identity::Identity;
use catalyst_comms_e2e::scenarios::crowd_discovery::{
    CROWD_DISCOVERY, ClientView, adapter_socket, island_peers_sent, island_verdict, pulse_verdict,
    room_verdict, strangers,
};
use catalyst_comms_e2e::stack::Stack;
use futures::{SinkExt, StreamExt};
use prost::Message as _;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::handshake::server::{Request, Response};

/// What no record, note or report may hold.
const SECRET: &str = "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIweGFiYyJ9.c2VjcmV0LXNpZ25hdHVyZQ";

fn packet(message: server_packet::Message) -> Message {
    Message::Binary(
        ServerPacket {
            request_sequence: 0,
            message: Some(message),
        }
        .encode_to_vec()
        .into(),
    )
}

fn at(x: f32) -> Position {
    Position { x, y: 0.0, z: 0.0 }
}

/// A connector that welcomes one socket, assigns it to `island-1` with two peers and a connection
/// string that carries [`SECRET`], then announces a third peer joining and one of the first two
/// leaving.
async fn connector() -> std::net::SocketAddr {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let (socket, _) = listener.accept().await.unwrap();
        let mut stream =
            tokio_tungstenite::accept_hdr_async(socket, |_: &Request, mut response: Response| {
                response
                    .headers_mut()
                    .append("Sec-WebSocket-Protocol", "archipelago".parse().unwrap());
                Ok(response)
            })
            .await
            .unwrap();
        while let Some(Ok(frame)) = stream.next().await {
            let Message::Binary(bytes) = frame else {
                continue;
            };
            let Ok(ClientPacket {
                message: Some(message),
                ..
            }) = ClientPacket::decode(bytes.as_ref())
            else {
                continue;
            };
            match message {
                client_packet::Message::ChallengeRequest(_) => {
                    stream
                        .send(packet(server_packet::Message::ChallengeResponse(
                            ChallengeResponseMessage {
                                challenge_to_sign: "dcl-test".into(),
                                already_connected: false,
                            },
                        )))
                        .await
                        .unwrap();
                }
                client_packet::Message::SignedChallenge(_) => {
                    stream
                        .send(packet(server_packet::Message::Welcome(WelcomeMessage {
                            peer_id: "me".into(),
                        })))
                        .await
                        .unwrap();
                    stream
                        .send(packet(server_packet::Message::IslandChanged(
                            IslandChangedMessage {
                                island_id: "island-1".into(),
                                conn_str: format!(
                                    "livekit:wss://livekit.example/rtc?access_token={SECRET}"
                                ),
                                from_island_id: None,
                                peers: [("0xaa".to_owned(), at(1.0)), ("0xbb".to_owned(), at(2.0))]
                                    .into_iter()
                                    .collect(),
                                assignment_context: None,
                            },
                        )))
                        .await
                        .unwrap();
                    stream
                        .send(packet(server_packet::Message::JoinIsland(
                            JoinIslandMessage {
                                island_id: "island-1".into(),
                                peer_id: "0xcc".into(),
                            },
                        )))
                        .await
                        .unwrap();
                    // another island's news is not this island's
                    stream
                        .send(packet(server_packet::Message::JoinIsland(
                            JoinIslandMessage {
                                island_id: "island-2".into(),
                                peer_id: "0xdd".into(),
                            },
                        )))
                        .await
                        .unwrap();
                    stream
                        .send(packet(server_packet::Message::LeftIsland(
                            LeftIslandMessage {
                                island_id: "island-1".into(),
                                peer_id: "0xaa".into(),
                            },
                        )))
                        .await
                        .unwrap();
                }
                _ => {}
            }
        }
    });
    address
}

fn ctx() -> Ctx {
    Ctx::new(
        Arc::new(Stack::pulse_only("127.0.0.1:9".parse().unwrap())),
        "unit",
        "crowd-discovery",
        0,
        1,
        std::env::temp_dir().join("comms-e2e-tests"),
    )
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_archipelago_bot_keeps_the_island_roster_and_records_no_token() {
    let address = connector().await;
    let ctx = ctx();
    let url = url::Url::parse(&format!("ws://{address}/ws")).unwrap();
    let identity = Identity::derive("crowd-discovery", 0);
    let bot = ArchipelagoBot::connect(
        &ctx,
        "client",
        &url,
        &identity,
        ArchipelagoOptions::default(),
    )
    .await
    .unwrap();
    expect_or(&ctx, "the roster settles", Duration::from_secs(5), || {
        let view = bot.view();
        (view.island_peers == ["0xbb", "0xcc"])
            .then_some(())
            .ok_or_else(|| format!("{:?}", view.island_peers))
    })
    .await
    .unwrap();
    assert_eq!(bot.assignments().len(), 1);
    assert_eq!(bot.assignments()[0].island, "island-1");

    // nothing the bot recorded holds the token, and the record of the assignment says so
    let timeline = serde_json::to_string(&ctx.timeline().snapshot()).unwrap();
    assert!(!timeline.contains(SECRET), "{timeline}");
    assert!(!timeline.contains("c2VjcmV0"), "{timeline}");
    assert!(timeline.contains("access_token=<redacted>"), "{timeline}");
    assert!(!format!("{:?}", bot.view()).contains(SECRET));
    bot.close();
}

fn client(wallet: &str, island: &str, peers: &[&str], room: &[&str], pulse: usize) -> ClientView {
    ClientView {
        wallet: wallet.into(),
        island: Some(island.into()),
        island_peers: peers.iter().map(|peer| (*peer).to_owned()).collect(),
        room_participants: room.iter().map(|peer| (*peer).to_owned()).collect(),
        pulse_held: pulse,
    }
}

#[test]
fn the_adapter_a_realm_names_is_dialed_as_it_says() {
    assert_eq!(
        adapter_socket("archipelago:archipelago:wss://realm.example/ws")
            .unwrap()
            .as_str(),
        "wss://realm.example/ws"
    );
    assert_eq!(
        adapter_socket("archipelago:archipelago:ws://127.0.0.1:6210/ws")
            .unwrap()
            .as_str(),
        "ws://127.0.0.1:6210/ws"
    );
    for other in [
        "offline:offline",
        "ws-room:wss://x/rooms/a",
        "archipelago:archipelago:https://x",
        "",
    ] {
        assert!(adapter_socket(other).is_err(), "{other:?}");
    }
}

#[test]
fn one_island_for_all_passes_and_a_split_or_a_missing_one_fails() {
    let together = [
        client("0x1", "island-a", &[], &["0x2"], 1),
        client("0x2", "island-a", &[], &["0x1"], 1),
    ];
    assert_eq!(island_verdict(&together).unwrap(), "island-a");

    let split = [
        client("0x1", "island-a", &[], &[], 1),
        client("0x2", "island-b", &[], &[], 1),
    ];
    let failure = island_verdict(&split).unwrap_err();
    assert!(failure.contains("2 islands"), "{failure}");

    let mut missing = together.clone();
    missing[1].island = None;
    assert!(island_verdict(&missing).unwrap_err().contains("0x2"));
}

#[test]
fn an_island_roster_the_message_carries_must_list_the_others() {
    // the roster lists both others: passes
    let full = [
        client("0x1", "i", &["0x2", "0x3"], &[], 0),
        client("0x2", "i", &["0x1", "0x3"], &[], 0),
        client("0x3", "i", &["0x1", "0x2"], &[], 0),
    ];
    assert!(island_verdict(&full).is_ok());
    // one roster lacks a player: fails, naming who
    let mut short = full.clone();
    short[2].island_peers = vec!["0x1".into()];
    let failure = island_verdict(&short).unwrap_err();
    assert!(
        failure.contains("0x3") && failure.contains("0x2"),
        "{failure}"
    );
}

#[test]
fn every_room_view_must_list_every_other_player() {
    let met = [
        client("0x1", "i", &[], &["0x2", "0x3"], 0),
        client("0x2", "i", &[], &["0x1", "0x3"], 0),
        client("0x3", "i", &[], &["0x1", "0x2"], 0),
    ];
    assert!(room_verdict(&met).is_ok());
    let mut short = met.clone();
    short[0].room_participants = vec!["0x2".into()];
    let failure = room_verdict(&short).unwrap_err();
    assert!(failure.contains("2 of 3"), "{failure}");
    // identities compare without case, as LiveKit hands back what the token says
    let mut cased = met.clone();
    cased[0].room_participants = vec!["0X2".into(), "0x3".into()];
    assert!(room_verdict(&cased).is_ok());
}

#[test]
fn someone_who_is_not_one_of_the_crowd_is_a_stranger() {
    let views = [
        client("0x1", "i", &["0x2", "0xfeed"], &["0x2", "0xbeef"], 0),
        client("0x2", "i", &["0x1"], &["0x1"], 0),
    ];
    assert_eq!(strangers(&views), vec!["0xbeef", "0xfeed"]);
    let clean = [
        client("0x1", "i", &["0X2"], &["0x2"], 0),
        client("0x2", "i", &["0x1"], &["0x1"], 0),
    ];
    assert!(strangers(&clean).is_empty());
}

#[test]
fn pulse_must_hold_every_other_player() {
    let views = [
        client("0x1", "i", &[], &[], 1),
        client("0x2", "i", &[], &[], 0),
    ];
    assert!(pulse_verdict(&views).unwrap_err().contains("1 of 2"));
    let both = [
        client("0x1", "i", &[], &[], 1),
        client("0x2", "i", &[], &[], 1),
    ];
    assert!(pulse_verdict(&both).is_ok());
}

#[test]
fn the_scenario_is_registered_and_not_hostile() {
    let registry = catalyst_comms_e2e::scenarios::registry();
    assert!(
        registry
            .iter()
            .any(|known| known.name == CROWD_DISCOVERY.name)
    );
    assert_eq!(CROWD_DISCOVERY.name, "crowd-discovery");
    assert!(!CROWD_DISCOVERY.hostile);
    assert!(CROWD_DISCOVERY.needs.api);
}

/// Runs `crowd-discovery` once against `stack` with a crowd of `players`.
async fn discovery_with(stack: &str, players: usize) -> catalyst_comms_e2e::engine::ScenarioResult {
    discovery_with_room(stack, players, None).await
}

/// Runs `crowd-discovery` once against `stack` with a crowd of `players`, `room_players` of them
/// asked to join the room.
async fn discovery_with_room(
    stack: &str,
    players: usize,
    room_players: Option<usize>,
) -> catalyst_comms_e2e::engine::ScenarioResult {
    let options = RunOptions {
        repeat: Some(1),
        seed: Some(7),
        transport: Transport::Enet,
        crowd: CrowdSize {
            players: Some(players),
            room_players,
        },
        ..Default::default()
    };
    let stack = Arc::new(Stack::parse(stack).unwrap());
    run_scenarios(stack, &[&CROWD_DISCOVERY], &options)
        .await
        .results
        .remove(0)
}

/// Each player holds an archipelago socket from the one address the crowd shares: a crowd larger
/// than the sockets the connector admits from one address is refused, naming the limit. The
/// Pulse handshakes per address count too. Nothing listens at these addresses: the refusal comes
/// first.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_crowd_larger_than_the_sockets_one_address_may_hold_is_refused() {
    let base = "api = \"http://127.0.0.1:9\"\npulse_enet = \"127.0.0.1:9\"\n[limits]\n";
    for (limits, key) in [
        ("max_sockets_per_ip = 5\n", "limits.max_sockets_per_ip"),
        (
            "max_concurrent_pre_auth_per_ip = 5\n",
            "limits.max_concurrent_pre_auth_per_ip",
        ),
    ] {
        let result = discovery_with(&format!("{base}{limits}"), 6).await;
        let Outcome::Failed { expectation, .. } = &result.outcome else {
            panic!(
                "{key}: a crowd of 6 should be refused, got {:?}",
                result.outcome
            );
        };
        assert!(
            expectation.contains(key) && expectation.contains("is 5"),
            "{key}: {expectation}"
        );
    }
}

#[test]
fn the_archipelago_sockets_per_address_default_to_the_servers() {
    let stack = Stack::parse("pulse_enet = \"127.0.0.1:9\"\n").unwrap();
    assert_eq!(
        stack.limits.max_sockets_per_ip,
        catalyst_comms::archipelago::config::DEFAULT_MAX_SOCKETS_PER_IP
    );
}

/// A stack file with no LiveKit client leaves the room step out and says so in a note; the
/// scenario goes on to discovery (which fails here: nothing answers at the API address).
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn without_a_livekit_client_the_room_step_is_left_out_with_a_note() {
    let result = discovery_with(
        "api = \"http://127.0.0.1:9\"\npulse_enet = \"127.0.0.1:9\"\n",
        2,
    )
    .await;
    let Outcome::Failed { expectation, .. } = &result.outcome else {
        panic!("nothing answers at the API, got {:?}", result.outcome);
    };
    assert!(expectation.contains("realm description"), "{expectation}");
    let room = result
        .notes
        .iter()
        .find(|note| note.key == "room")
        .unwrap_or_else(|| panic!("no room note: {:?}", result.notes));
    let text = room.value.as_str().unwrap();
    assert!(
        text.contains("not done") && text.contains("LiveKit client"),
        "{text}"
    );
    assert!(
        result
            .notes
            .iter()
            .any(|note| note.key == "room_players" && note.value.as_u64() == Some(0)),
        "{:?}",
        result.notes
    );
}

/// A room count set above 0 with no LiveKit client in the stack file is a mistake in the stack
/// file: the run fails, naming the client, before it dials anything. A count of 0 asks for no
/// room and needs no client.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_room_count_set_without_a_livekit_client_fails() {
    let stack = "api = \"http://127.0.0.1:9\"\npulse_enet = \"127.0.0.1:9\"\n";
    let result = discovery_with_room(stack, 2, Some(2)).await;
    let Outcome::Failed { expectation, .. } = &result.outcome else {
        panic!(
            "a room count with no client should fail, got {:?}",
            result.outcome
        );
    };
    assert!(
        expectation.contains("LiveKit client") && expectation.contains("CE2E_CROWD_ROOM_PLAYERS"),
        "{expectation}"
    );
    // no room asked for: the run goes on to discovery
    let result = discovery_with_room(stack, 2, Some(0)).await;
    let Outcome::Failed { expectation, .. } = &result.outcome else {
        panic!("nothing answers at the API, got {:?}", result.outcome);
    };
    assert!(expectation.contains("realm description"), "{expectation}");
}

/// Whether any island message carried peers: the gatekeeper may send assignments with none, and
/// then the peer list is not checked.
#[test]
fn the_report_says_whether_any_island_message_carried_peers() {
    let view = |wallet: &str, peers: &[&str]| ClientView {
        wallet: wallet.into(),
        island: Some("I1".into()),
        island_peers: peers.iter().map(|peer| (*peer).to_owned()).collect(),
        ..Default::default()
    };
    let without = [view("0xaa", &[]), view("0xbb", &[])];
    assert!(!island_peers_sent(&without));
    assert_eq!(island_verdict(&without).unwrap(), "I1");
    let with = [view("0xaa", &["0xbb"]), view("0xbb", &[])];
    assert!(island_peers_sent(&with));
}
