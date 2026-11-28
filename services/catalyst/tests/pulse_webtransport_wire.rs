// from: catalyrst/crates/catalyrst-pulse/tests/webtransport_wire.rs
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use prost::Message as _;
use sha2::{Digest, Sha256};

use catalyst::comms::pulse::PulseServer;
use catalyst::comms::pulse::decentraland::pulse::{
    ClientMessage, HandshakeRequest, PlayerState, PlayerStateInput, ServerMessage, client_message,
    server_message,
};
use catalyst::comms::pulse::hardening::DEFAULT_CORRUPT_BURST;
use catalyst::comms::pulse::server::ENET_CAPACITY;
use catalyst::comms::pulse::transport::webtransport::framing::StreamFrameReader;
use catalyst::comms::pulse::transport::webtransport::framing::{datagram_frame, stream_frame};
use catalyst::comms::pulse::transport::webtransport::{WtConfig, WtHost};
use catalyst::comms::pulse::transport::{Host, HostConfig, Transports};

use catalyst::comms::pulse::transport::webtransport::client::{Client, ClientConfig, ClientEvent};

const ROOT_KEY: &str = "ac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";
const EPH_KEY: &str = "59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d";

fn dev_cert() -> (String, String, Vec<u8>) {
    let certified = rcgen::generate_simple_self_signed(vec!["localhost".to_string()]).unwrap();
    let hash = Sha256::digest(certified.cert.der().as_ref()).to_vec();
    (
        certified.cert.pem(),
        certified.signing_key.serialize_pem(),
        hash,
    )
}

fn now_ms() -> i64 {
    let ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis();
    i64::try_from(ms).unwrap()
}

async fn signed_handshake(ts_ms: i64) -> (Vec<u8>, String) {
    use alloy::signers::{Signer, local::PrivateKeySigner};
    use catalyst::auth::{AuthLink, AuthLinkType};
    use catalyst::comms::pulse::handshake::build_signed_fetch_payload;

    let root: PrivateKeySigner = ROOT_KEY.parse().unwrap();
    let root_addr = format!("{:#x}", root.address());
    let ephemeral: PrivateKeySigner = EPH_KEY.parse().unwrap();
    let eph_addr = format!("{:#x}", ephemeral.address());

    let ts = ts_ms.to_string();
    let metadata = "{\"signer\":\"dcl:explorer\"}";
    let connect_payload = build_signed_fetch_payload("connect", "/", &ts, metadata);
    let eph_payload = format!(
        "Decentraland Login\nEphemeral address: {eph_addr}\nExpiration: 2099-01-01T00:00:00.000Z"
    );
    let eph_sig = root
        .sign_message(eph_payload.as_bytes())
        .await
        .unwrap()
        .to_string();
    let final_sig = ephemeral
        .sign_message(connect_payload.as_bytes())
        .await
        .unwrap()
        .to_string();

    let chain = [
        AuthLink {
            link_type: AuthLinkType::SIGNER,
            payload: root_addr.clone(),
            signature: None,
        },
        AuthLink {
            link_type: AuthLinkType::EcdsaEphemeral,
            payload: eph_payload,
            signature: Some(eph_sig),
        },
        AuthLink {
            link_type: AuthLinkType::EcdsaSignedEntity,
            payload: connect_payload,
            signature: Some(final_sig),
        },
    ];
    let mut map = serde_json::Map::new();
    for (i, link) in chain.iter().enumerate() {
        map.insert(
            format!("x-identity-auth-chain-{i}"),
            serde_json::Value::String(serde_json::to_string(link).unwrap()),
        );
    }
    map.insert("x-identity-timestamp".into(), serde_json::Value::String(ts));
    map.insert(
        "x-identity-metadata".into(),
        serde_json::Value::String(metadata.into()),
    );
    let bag = serde_json::to_string(&serde_json::Value::Object(map)).unwrap();

    let msg = ClientMessage {
        message: Some(client_message::Message::Handshake(HandshakeRequest {
            auth_chain: bag.into_bytes(),
            profile_version: 0,
            initial_state: None,
            protocol_features: 0,
        })),
    };
    (msg.encode_to_vec(), root_addr.to_lowercase())
}

/// The server's cap on one reliable message, the WebTransport default.
const MAX_MESSAGE_BYTES: usize = 4096;
/// The protocol's `PacketCorrupted` disconnect reason.
const PACKET_CORRUPTED: u32 = 16;

/// Starts a Pulse server that listens on WebTransport and connects one client to it. The runtime
/// runs the server; dropping it stops the server.
fn serve_and_connect() -> (tokio::runtime::Runtime, Client) {
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .with_test_writer()
        .try_init();
    let (cert_pem, key_pem, cert_hash) = dev_cert();

    let rt = tokio::runtime::Runtime::new().unwrap();

    let (wt, events) = WtHost::start(WtConfig {
        bind_addr: "127.0.0.1:0".parse().unwrap(),
        cert_pem,
        key_pem,
        slot_base: u32::try_from(ENET_CAPACITY).unwrap(),
        slot_capacity: 64,
        max_datagram_bytes: 1200,
        max_message_bytes: MAX_MESSAGE_BYTES,
        service_timeout_ms: 1,
        server_full_reason: 6,
    })
    .unwrap();
    let wt_addr = wt.local_addr();

    rt.block_on(async {
        let enet = Host::bind(HostConfig {
            bind: "127.0.0.1:0".parse().unwrap(),
            max_peers: ENET_CAPACITY,
            channel_limit: 8,
        })
        .await
        .unwrap();
        let transports =
            Transports::with_webtransport(enet, u32::try_from(ENET_CAPACITY).unwrap(), wt, events);
        tokio::spawn(PulseServer::new().serve(transports, 50));
    });

    let url = format!("https://127.0.0.1:{}/", wt_addr.port());
    let client = Client::connect(ClientConfig {
        url,
        server_cert_hash: Some(cert_hash),
    })
    .expect("client connects to the real server over QUIC");
    (rt, client)
}

/// Reads the server's frames until a successful HandshakeResponse arrives.
fn expect_authenticated(client: &mut Client, wallet: &str) {
    let mut reader = StreamFrameReader::new(MAX_MESSAGE_BYTES);
    let deadline = Instant::now() + Duration::from_secs(15);
    while Instant::now() < deadline {
        match client.service(Duration::from_millis(200)) {
            Some(ClientEvent::StreamData { data }) => {
                reader.append(&data);
                while let Ok(Some(frame)) = reader.try_read() {
                    let msg = ServerMessage::decode(&frame[..]).expect("valid ServerMessage");
                    if let Some(server_message::Message::Handshake(h)) = msg.message {
                        assert!(h.success, "handshake rejected on the wire: {:?}", h.error);
                        return;
                    }
                }
            }
            Some(ClientEvent::Disconnected { reason }) => {
                panic!("server closed the session before authenticating (reason {reason})");
            }
            _ => {}
        }
    }
    panic!("no successful HandshakeResponse received over the wire (wallet {wallet})");
}

/// The code the server closes the session with within `within`; `None` while it stays open.
fn close_code_within(client: &mut Client, within: Duration) -> Option<u32> {
    let deadline = Instant::now() + within;
    while Instant::now() < deadline {
        if let Some(ClientEvent::Disconnected { reason }) =
            client.service(Duration::from_millis(50))
        {
            return Some(reason);
        }
    }
    None
}

#[test]
fn full_handshake_and_gameplay_over_the_wire() {
    let (rt, mut client) = serve_and_connect();

    let (handshake, wallet) = rt.block_on(signed_handshake(now_ms()));
    assert!(client.send_stream(&stream_frame(&handshake)));
    expect_authenticated(&mut client, &wallet);

    let mut state = PlayerState {
        parcel_index: 5,
        ..Default::default()
    };
    state.set_position_x_f(8.0);
    state.set_position_z_f(8.0);
    let input = ClientMessage {
        message: Some(client_message::Message::Input(PlayerStateInput {
            state: Some(state),
        })),
    }
    .encode_to_vec();
    assert!(client.send_datagram(&datagram_frame(1, 0, &input)));

    let watch = Instant::now() + Duration::from_millis(600);
    while Instant::now() < watch {
        if let Some(ClientEvent::Disconnected { reason }) =
            client.service(Duration::from_millis(100))
        {
            panic!("session dropped after a valid movement update (reason {reason})");
        }
    }

    client.disconnect(0);
    drop(rt);
}

// A stream frame declares its length before its payload. Like an ENet packet against the 4096-byte
// receive buffer, a frame over the message cap and up to twice it is charged to the corruption
// budget, and a larger one ends the session at once with PacketCorrupted.

#[test]
fn a_stream_frame_over_twice_the_message_cap_ends_the_session_at_once() {
    let (rt, mut client) = serve_and_connect();

    // Zero bytes: a reader that lost the frame's boundary reads them as empty frames.
    assert!(client.send_stream(&stream_frame(&vec![0u8; 2 * MAX_MESSAGE_BYTES + 1])));

    assert_eq!(
        close_code_within(&mut client, Duration::from_secs(3)),
        Some(PACKET_CORRUPTED)
    );
    drop(rt);
}

#[test]
fn a_stream_frame_up_to_twice_the_message_cap_is_skipped_and_the_next_frame_is_read() {
    let (rt, mut client) = serve_and_connect();

    // One write, so the handshake arrives in the same chunks as the rejected payload.
    let (handshake, wallet) = rt.block_on(signed_handshake(now_ms()));
    let mut bytes = stream_frame(&vec![0xff; MAX_MESSAGE_BYTES + 1]);
    bytes.extend_from_slice(&stream_frame(&handshake));
    assert!(client.send_stream(&bytes));

    expect_authenticated(&mut client, &wallet);
    client.disconnect(0);
    drop(rt);
}

#[test]
fn each_stream_frame_up_to_twice_the_message_cap_spends_one_corruption_token() {
    let (rt, mut client) = serve_and_connect();
    let oversized = stream_frame(&vec![0xff; 2 * MAX_MESSAGE_BYTES]);

    let budget: Vec<u8> = (0..DEFAULT_CORRUPT_BURST)
        .flat_map(|_| oversized.iter().copied())
        .collect();
    assert!(client.send_stream(&budget));
    assert_eq!(
        close_code_within(&mut client, Duration::from_millis(500)),
        None,
        "frames within the corruption budget must not end the session"
    );

    assert!(client.send_stream(&oversized));
    assert_eq!(
        close_code_within(&mut client, Duration::from_secs(3)),
        Some(PACKET_CORRUPTED)
    );
    drop(rt);
}
