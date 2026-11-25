//! `cast-bot` against a real LiveKit: a `livekit-server` this test starts on
//! free loopback ports (the binary named by `LIVEKIT_SERVER_BIN`), a viewer
//! joined with the Rust SDK, and the bot driven over stdio as the node
//! drives it. Without `LIVEKIT_SERVER_BIN` the test says so and passes,
//! except under CI (`CI=true`), where it fails. `CAST_BOT_BIN` names the bot
//! to drive when it is not this build's (CI runs the Nix-built test binary).

use std::process::Stdio;
use std::time::Duration;

use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use livekit::prelude::*;
use livekit_api::access_token::{AccessToken, VideoGrants};
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Lines};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};

const KEY: &str = "castbotkey";
const SECRET: &str = "castbotsecret-castbotsecret-castbotsecret";
const ROOM: &str = "scene-room";
const WAIT: Duration = Duration::from_secs(20);

fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

/// The binary under test.
fn bot_binary() -> String {
    std::env::var("CAST_BOT_BIN").unwrap_or_else(|_| env!("CARGO_BIN_EXE_cast-bot").to_string())
}

/// Sets the room's metadata through LiveKit's server API (Twirp JSON).
async fn set_room_metadata(port: u16, metadata: &str) {
    let admin = AccessToken::with_api_key(KEY, SECRET)
        .with_grants(VideoGrants {
            room_admin: true,
            room: ROOM.into(),
            ..Default::default()
        })
        .to_jwt()
        .unwrap();
    let body = json!({"room": ROOM, "metadata": metadata}).to_string();
    let request = format!(
        "POST /twirp/livekit.RoomService/UpdateRoomMetadata HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n\
         Authorization: Bearer {admin}\r\nContent-Type: application/json\r\n\
         Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let mut stream = tokio::net::TcpStream::connect(("127.0.0.1", port))
        .await
        .unwrap();
    stream.write_all(request.as_bytes()).await.unwrap();
    let mut response = String::new();
    tokio::io::AsyncReadExt::read_to_string(&mut stream, &mut response)
        .await
        .unwrap();
    assert!(response.starts_with("HTTP/1.1 200"), "{response}");
}

fn token(identity: &str, can_update_metadata: bool) -> String {
    AccessToken::with_api_key(KEY, SECRET)
        .with_identity(identity)
        .with_grants(VideoGrants {
            room_join: true,
            room: ROOM.into(),
            can_publish: Some(true),
            can_subscribe: Some(true),
            can_publish_data: Some(true),
            can_update_own_metadata: Some(can_update_metadata),
            ..Default::default()
        })
        .to_jwt()
        .unwrap()
}

struct Bot {
    child: Child,
    stdin: ChildStdin,
    stdout: Lines<BufReader<ChildStdout>>,
    next_id: u64,
}

impl Bot {
    fn start() -> Self {
        let mut child = Command::new(bot_binary())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .kill_on_drop(true)
            .spawn()
            .unwrap();
        let stdin = child.stdin.take().unwrap();
        let stdout = BufReader::new(child.stdout.take().unwrap()).lines();
        Self {
            child,
            stdin,
            stdout,
            next_id: 1,
        }
    }

    async fn send(&mut self, line: Value) {
        let mut bytes = serde_json::to_vec(&line).unwrap();
        bytes.push(b'\n');
        self.stdin.write_all(&bytes).await.unwrap();
        self.stdin.flush().await.unwrap();
    }

    async fn next(&mut self) -> Value {
        let line = tokio::time::timeout(WAIT, self.stdout.next_line())
            .await
            .expect("bot answers in time")
            .unwrap()
            .expect("bot still running");
        serde_json::from_str(&line).unwrap()
    }

    /// The next line that is not an unsolicited event.
    async fn answer(&mut self) -> Value {
        loop {
            let line = self.next().await;
            if line.get("id").is_some() || line["type"] == "ready" || line["type"] == "error" {
                return line;
            }
        }
    }

    async fn command(&mut self, mut line: Value) -> Value {
        let id = self.next_id;
        self.next_id += 1;
        line["id"] = json!(id);
        self.send(line).await;
        let answer = self.answer().await;
        assert_eq!(answer["id"], id, "{answer}");
        answer
    }
}

async fn until<T>(
    events: &mut tokio::sync::mpsc::UnboundedReceiver<RoomEvent>,
    mut pick: impl FnMut(RoomEvent) -> Option<T>,
) -> T {
    tokio::time::timeout(WAIT, async {
        while let Some(event) = events.recv().await {
            if let Some(found) = pick(event) {
                return found;
            }
        }
        panic!("viewer events ended")
    })
    .await
    .expect("viewer sees it in time")
}

#[tokio::test(flavor = "multi_thread")]
async fn the_bot_publishes_slides_and_carries_data_both_ways() {
    let Ok(server_bin) = std::env::var("LIVEKIT_SERVER_BIN") else {
        assert!(
            std::env::var("CI").as_deref() != Ok("true"),
            "LIVEKIT_SERVER_BIN is required in CI"
        );
        eprintln!("SKIPPED: set LIVEKIT_SERVER_BIN to a livekit-server binary to run this test");
        return;
    };
    let (port, tcp, udp) = (free_port(), free_port(), free_port());
    let config = format!(
        "port: {port}\nbind_addresses: [127.0.0.1]\nrtc:\n  tcp_port: {tcp}\n  udp_port: {udp}\n  use_external_ip: false\n  node_ip: 127.0.0.1\nkeys:\n  {KEY}: {SECRET}\nlogging:\n  level: warn\n"
    );
    let _server = Command::new(server_bin)
        .arg("--config-body")
        .arg(config)
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    let url = format!("ws://127.0.0.1:{port}");
    for _ in 0..100 {
        if tokio::net::TcpStream::connect(("127.0.0.1", port))
            .await
            .is_ok()
        {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    // A refused token: connect-failed, then the bot exits.
    let mut refused = Bot::start();
    refused
        .send(json!({"type": "init", "url": url, "token": "not-a-token"}))
        .await;
    let answer = refused.answer().await;
    assert_eq!(answer["type"], "error");
    assert_eq!(answer["code"], "connect-failed");
    assert!(
        tokio::time::timeout(WAIT, refused.child.wait())
            .await
            .unwrap()
            .is_ok()
    );

    // No LiveKit there: unavailable, not refused.
    let mut unreachable = Bot::start();
    unreachable
        .send(json!({"type": "init", "url": format!("ws://127.0.0.1:{}", free_port()), "token": token("x", false)}))
        .await;
    let answer = unreachable.answer().await;
    assert_eq!(answer["code"], "connect-unavailable", "{answer}");

    let (viewer, mut events) =
        Room::connect(&url, &token("0xViewer", false), RoomOptions::default())
            .await
            .unwrap();
    // The viewer presents; the bot reads the list as it joins.
    set_room_metadata(port, r#"{"presenters":["0xviewer"]}"#).await;

    let mut bot = Bot::start();
    let identity = "presentation-bot:scene-room:access:1";
    bot.send(json!({"type": "init", "url": url, "token": token(identity, true)}))
        .await;
    let ready = bot.answer().await;
    assert_eq!(ready["type"], "ready", "{ready}");
    assert_eq!(ready["participantCount"], 1);

    assert_eq!(
        bot.command(json!({"type": "publishVideo", "width": 64, "height": 48}))
            .await["type"],
        "ack"
    );
    let rgba = BASE64.encode(vec![200u8; 64 * 48 * 4]);
    assert_eq!(
        bot.command(json!({"type": "frame", "width": 64, "height": 48, "rgba": rgba}))
            .await["type"],
        "ack"
    );
    // A frame of the wrong size is refused, and the bot carries on.
    let refused_frame = bot
        .command(json!({"type": "frame", "width": 64, "height": 48, "rgba": "AAAA"}))
        .await;
    assert_eq!(refused_frame["type"], "error");

    let (source, name, publisher) = until(&mut events, |event| match event {
        RoomEvent::TrackSubscribed {
            publication,
            participant,
            ..
        } => Some((
            publication.source(),
            publication.name(),
            participant.identity().to_string(),
        )),
        _ => None,
    })
    .await;
    assert_eq!(source, TrackSource::Screenshare);
    assert_eq!(name, "presentation");
    assert_eq!(publisher, identity);

    let metadata = r#"{"role":"presentation","presentationId":"p1"}"#;
    assert_eq!(
        bot.command(json!({"type": "updateMetadata", "metadata": metadata}))
            .await["type"],
        "ack"
    );
    let seen = until(&mut events, |event| match event {
        RoomEvent::ParticipantMetadataChanged {
            participant,
            metadata,
            ..
        } if participant.identity().to_string() == identity => Some(metadata),
        _ => None,
    })
    .await;
    assert_eq!(seen, metadata);

    let packet = b"\x32\x05hello".to_vec();
    assert_eq!(
        bot.command(json!({"type": "publishData", "data": BASE64.encode(&packet)}))
            .await["type"],
        "ack"
    );
    let (payload, kind) = until(&mut events, |event| match event {
        RoomEvent::DataReceived { payload, kind, .. } => Some((payload.to_vec(), kind)),
        _ => None,
    })
    .await;
    assert_eq!(payload, packet);
    assert_eq!(kind, DataPacketKind::Reliable);

    // Only presenters' commands cross: the scene's other traffic stays in the room.
    let command = br#"{"type":"presentation:get-state"}"#;
    for payload in [&b"movement"[..], &command[..]] {
        viewer
            .local_participant()
            .publish_data(DataPacket {
                payload: payload.to_vec(),
                reliable: true,
                ..Default::default()
            })
            .await
            .unwrap();
    }
    let received = loop {
        let line = bot.next().await;
        if line["type"] == "dataReceived" {
            break line;
        }
    };
    assert_eq!(received["identity"], "0xViewer");
    assert_eq!(
        BASE64
            .decode(received["payload"].as_str().unwrap())
            .unwrap(),
        command
    );

    // A room without the viewer among its presenters: its commands stay out too.
    set_room_metadata(port, r#"{"presenters":["0xsomeoneelse"]}"#).await;
    let changed = loop {
        let line = bot.next().await;
        if line["type"] == "roomMetadata" {
            break line;
        }
    };
    assert_eq!(changed["metadata"], r#"{"presenters":["0xsomeoneelse"]}"#);
    viewer
        .local_participant()
        .publish_data(DataPacket {
            payload: command.to_vec(),
            reliable: true,
            ..Default::default()
        })
        .await
        .unwrap();

    // The viewer leaves: the count drops to zero.
    viewer.close().await.unwrap();
    let count = loop {
        let line = bot.next().await;
        assert_ne!(line["type"], "dataReceived", "{line}");
        if line["type"] == "participantCount" {
            break line["count"].clone();
        }
    };
    assert_eq!(count, 0);

    // A second bot is no audience: neither bot counts the other.
    let mut other = Bot::start();
    let second = token("presentation-bot:scene-room:access:2", true);
    other
        .send(json!({"type": "init", "url": url, "token": second}))
        .await;
    assert_eq!(other.answer().await["participantCount"], 0);
    let count = loop {
        let line = bot.next().await;
        if line["type"] == "participantCount" {
            break line["count"].clone();
        }
    };
    assert_eq!(count, 0);

    assert_eq!(
        bot.command(json!({"type": "shutdown"})).await["type"],
        "ack"
    );
    let status = tokio::time::timeout(WAIT, bot.child.wait())
        .await
        .unwrap()
        .unwrap();
    assert!(status.success());
}
