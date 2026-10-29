use super::*;
use crate::pulse::application_relay::state::Delivery;
use std::collections::{HashSet, VecDeque};

impl PulseServer {
    pub(super) async fn apply_application(
        &mut self,
        transports: &mut Transports,
        peer: u32,
        message: client_message::Message,
    ) -> anyhow::Result<()> {
        let Some(relay) = self.application_relay.as_mut() else {
            return Ok(());
        };
        let mut retire = vec![];
        let out = match message {
            client_message::Message::ApplicationJoin(join) => {
                relay.join(peer, join, chrono::Utc::now().timestamp().max(0) as u64)
            }
            client_message::Message::ApplicationLeave(leave) => relay.leave(peer, leave.room_id),
            client_message::Message::ApplicationSend(send) => {
                let (out, close) = relay.send(peer, send, std::time::Instant::now());
                if close {
                    retire.push(peer);
                }
                out
            }
            _ => vec![],
        };
        self.deliver_application(transports, out, retire).await
    }

    pub(super) fn silent_relay_scopes(
        &mut self,
        transports: &Transports,
        now: std::time::Instant,
    ) -> Vec<Delivery> {
        let Some(relay) = self.application_relay.as_mut() else {
            return vec![];
        };
        transports
            .silent_relay_peers(now)
            .into_iter()
            .flat_map(|peer| relay.retire_scopes(peer))
            .collect()
    }

    pub(super) async fn deliver_application(
        &mut self,
        transports: &mut Transports,
        out: Vec<Delivery>,
        retire: Vec<u32>,
    ) -> anyhow::Result<()> {
        let mut queue = VecDeque::from(out);
        let mut retire = VecDeque::from(retire);
        let mut retired = HashSet::new();
        let mut noticed = HashSet::new();
        loop {
            while let Some(peer) = retire.pop_front() {
                if !retired.insert(peer) {
                    continue;
                }
                if let Some(relay) = self.application_relay.as_mut() {
                    queue.extend(relay.disconnect(peer));
                }
                let notify = self.remove_peer_state(peer);
                transports
                    .disconnect_now(peer, DisconnectReason::AuthFailed.code())
                    .await?;
                // ENet resets immediately and emits no local Disconnect callback. WT keeps
                // its slot until its worker emits Disconnect, so retain that reservation.
                if !transports.owns_wt(peer) {
                    self.ip_limiter.release(peer);
                }
                if notify {
                    self.notify_player_left(transports, peer).await?;
                }
            }
            let Some(delivery) = queue.pop_front() else {
                break;
            };
            if retired.contains(&delivery.target) {
                continue;
            }
            let bytes = delivery.message.encode_to_vec();
            let packet = if delivery.unreliable {
                Packet::unsequenced(channel::UNRELIABLE_UNSEQUENCED, bytes)
            } else {
                Packet::reliable(channel::RELIABLE, bytes)
            };
            let sent = if delivery.is_data() {
                transports.send_application(delivery.target, packet).await
            } else {
                noticed.insert(delivery.target);
                transports.send_control(delivery.target, packet).await
            };
            let Err(error) = sent else {
                continue;
            };
            if delivery.unreliable {
                continue;
            }
            let full_scope = delivery
                .room_id()
                .filter(|_| delivery.is_data() && error.kind() == std::io::ErrorKind::WouldBlock);
            match (full_scope, self.application_relay.as_mut()) {
                (Some(room), Some(relay)) => {
                    let mut failed = Vec::from_iter(relay.state.send_failed(&delivery));
                    queue.retain(|queued| {
                        let kept =
                            queued.target != delivery.target || queued.room_id() != Some(room);
                        if !kept {
                            failed.extend(relay.state.send_failed(queued));
                        }
                        kept
                    });
                    queue.extend(relay.leave(delivery.target, room));
                    queue.extend(failed);
                }
                _ => retire.push_back(delivery.target),
            }
        }
        if let Some(relay) = self.application_relay.as_ref() {
            for peer in noticed.difference(&retired) {
                let sends_input = self
                    .peers
                    .get(peer)
                    .is_some_and(|state| !state.is_listener());
                transports.set_relay_timeout(*peer, relay.state.holds_scope(*peer), sends_input);
            }
        }
        transports.flush();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::pulse::decentraland::pulse::{
        ApplicationData, ApplicationLeave, ApplicationPeerLeft, ApplicationSend,
        ApplicationSendFailed,
    };
    use crate::protocol::room_authority::AdmittedRoom;
    use crate::pulse::application_relay::{
        Admission, AdmittedScope, AuthorityError, AuthorityInput,
    };
    use crate::pulse::application_relay::{ApplicationRelay, RoomAuthority};
    use crate::pulse::transport::host::ManualClock;
    use crate::pulse::transport::peer::PeerId;
    use crate::shared::test_wait::HANG_GUARD;
    use std::future::Future;
    use std::pin::Pin;
    use std::time::{Duration, Instant};

    const SENDER: usize = 0;
    const RECIPIENT: usize = 1;
    const BYSTANDER: usize = 2;
    const ABSENT_PEER: u32 = 7;
    /// The probe a round trip sends each way; no encoded server message is a lone zero byte.
    const PROBE: [u8; 1] = [0];

    struct Authority;

    impl RoomAuthority for Authority {
        fn authorize(
            &self,
            input: AuthorityInput,
        ) -> Pin<Box<dyn Future<Output = Result<Admission, AuthorityError>> + Send + '_>> {
            Box::pin(async move {
                match input {
                    AuthorityInput::Renew(scope) => Ok(Admission {
                        scope,
                        ttl: Duration::from_secs(1),
                    }),
                    AuthorityInput::Join(_) => Err(AuthorityError::Unavailable),
                }
            })
        }
    }

    #[derive(Debug, PartialEq)]
    enum Seen {
        Message(Box<server_message::Message>),
        Disconnected,
    }

    struct Rig {
        server: PulseServer,
        transports: Transports,
        clients: Vec<Host>,
        /// Each client's peer id for the server.
        servers: Vec<PeerId>,
        sessions: Vec<web_transport_quinn::Session>,
        peers: Vec<u32>,
    }

    impl Rig {
        async fn connect(clients: usize) -> Self {
            Self::connect_on(clients, None).await
        }

        /// The server's ENet host reads `clock` when given one; the clients keep real time.
        async fn connect_on(clients: usize, clock: Option<ManualClock>) -> Self {
            let config = || HostConfig {
                bind: "127.0.0.1:0".parse().unwrap(),
                max_peers: 8,
                channel_limit: 3,
            };
            let mut host = match clock {
                Some(clock) => Host::bind_with_clock(config(), clock).await,
                None => Host::bind(config()).await,
            }
            .unwrap();
            let address = host.local_addr().unwrap();
            let mut connected = vec![];
            let mut servers = vec![];
            let mut peers = vec![];
            let connecting = Instant::now();
            for _ in 0..clients {
                let mut client = Host::bind(config()).await.unwrap();
                servers.push(client.connect(address, 3).unwrap());
                peers.push(loop {
                    assert!(connecting.elapsed() < HANG_GUARD, "ENet connect hung");
                    if let Some(Event::Connect { peer, .. }) = host.service().await.unwrap() {
                        break peer as u32;
                    }
                    client.service().await.unwrap();
                });
                connected.push(client);
            }
            let mut server = PulseServer::new();
            let mut relay = ApplicationRelay::new(Arc::new(Authority));
            for peer in &peers {
                relay.authenticated(*peer, [*peer as u8; 32], String::new(), String::new());
            }
            server.application_relay = Some(relay);
            Self {
                server,
                transports: Transports::enet_only(host, 8),
                clients: connected,
                servers,
                sessions: vec![],
                peers,
            }
        }

        async fn webtransport(
            listening: crate::pulse::transport::multi::testing::Listening,
            clients: usize,
        ) -> Self {
            let (transports, connected) =
                crate::pulse::transport::multi::testing::connect(listening, clients).await;
            let (peers, sessions): (Vec<u32>, Vec<_>) = connected.into_iter().unzip();
            let mut server = PulseServer::new();
            let mut relay = ApplicationRelay::new(Arc::new(Authority));
            for peer in &peers {
                relay.authenticated(*peer, [*peer as u8; 32], String::new(), String::new());
                server.peers.insert(
                    *peer,
                    crate::pulse::simulation::PeerState::new(
                        crate::pulse::simulation::PeerConnectionState::Authenticated,
                        0,
                    ),
                );
            }
            server.application_relay = Some(relay);
            Self {
                server,
                transports,
                clients: vec![],
                servers: vec![],
                sessions,
                peers,
            }
        }

        fn enrol(&mut self, peer: u32, room: &str) -> u32 {
            self.join(peer, room).0
        }

        async fn admit(&mut self, peer: u32, room: &str) -> u32 {
            let (room_id, joined) = self.join(peer, room);
            self.server
                .deliver_application(&mut self.transports, joined, vec![])
                .await
                .unwrap();
            room_id
        }

        fn join(&mut self, peer: u32, room: &str) -> (u32, Vec<Delivery>) {
            let relay = self.server.application_relay.as_mut().unwrap();
            let now = Instant::now();
            let verified = AdmittedScope {
                admitted: AdmittedRoom {
                    room: room.into(),
                    identity: format!("0x{peer:040x}"),
                    is_guest: Some(true),
                },
                nonce: [0; 32],
                fingerprint: [0; 32],
            };
            let joined = relay
                .state
                .join(peer, 1, verified, now + Duration::from_secs(60), now);
            (relay.state.membership(peer, room).unwrap().room_id, joined)
        }

        fn member(&self, peer: u32, room: &str) -> bool {
            let relay = self.server.application_relay.as_ref().unwrap();
            relay.state.membership(peer, room).is_some()
        }

        async fn send(&mut self, peer: u32, room_id: u32, payload: Vec<u8>) {
            let send = ApplicationSend {
                room_id,
                payload,
                ..Default::default()
            };
            self.apply(peer, client_message::Message::ApplicationSend(send))
                .await;
        }

        async fn apply(&mut self, peer: u32, message: client_message::Message) {
            self.server
                .apply_application(&mut self.transports, peer, message)
                .await
                .unwrap();
        }

        async fn connected(&mut self, peer: u32) -> bool {
            let probe = Packet::reliable(channel::RELIABLE, vec![0]);
            let sent = self.transports.send_application(peer, probe).await;
            !sent.is_err_and(|error| error.kind() == std::io::ErrorKind::NotConnected)
        }

        /// Gives `clients` a 300 ms round trip on the server's clock: each probe's
        /// acknowledgement is only taken in once that clock has moved 300 ms past its send.
        async fn slow_round_trips(&mut self, clock: &ManualClock, clients: &[usize]) {
            for _ in 0..4 {
                self.probe(clients).await;
                clock.advance(Duration::from_millis(300));
                let disconnected = self.answer(clients).await;
                assert!(disconnected.is_empty(), "{disconnected:?} dropped");
            }
        }

        async fn probe(&mut self, clients: &[usize]) {
            for client in clients {
                let probe = Packet::reliable(channel::RELIABLE, PROBE.to_vec());
                self.transports
                    .send(self.peers[*client], probe)
                    .await
                    .unwrap();
            }
            self.transports.flush();
        }

        /// Each of `clients` takes in its probe and answers it; the server then takes in every
        /// answer, and with each one the acknowledgement the client sent ahead of it. Returns
        /// the peers the server saw disconnect on the way, already handled.
        async fn answer(&mut self, clients: &[usize]) -> Vec<u32> {
            let started = Instant::now();
            for client in clients {
                loop {
                    assert!(
                        started.elapsed() < HANG_GUARD,
                        "client {client} never took in its probe"
                    );
                    if let Some(Event::Receive { packet, .. }) =
                        self.clients[*client].service().await.unwrap()
                        && packet.data.as_ref() == PROBE
                    {
                        break;
                    }
                }
                let answer = Packet::reliable(channel::RELIABLE, PROBE.to_vec());
                self.clients[*client]
                    .send(self.servers[*client], answer)
                    .await
                    .unwrap();
                self.clients[*client].flush();
            }
            let mut waiting: Vec<u32> = clients.iter().map(|client| self.peers[*client]).collect();
            let mut disconnected = vec![];
            while !waiting.is_empty() {
                assert!(
                    started.elapsed() < HANG_GUARD,
                    "the server never took in the answers of {waiting:?}"
                );
                match self.transports.service().await.unwrap() {
                    Some(Event::Receive { peer, .. }) => waiting.retain(|p| *p != peer as u32),
                    Some(Event::Disconnect { peer }) => {
                        self.server
                            .on_disconnect(&mut self.transports, peer as u32)
                            .await
                            .unwrap();
                        disconnected.push(peer as u32);
                    }
                    _ => {}
                }
            }
            disconnected
        }

        /// Everything each client is sent, up to a marker queued behind it all on the reliable
        /// channel, which ENet delivers in order: a client is done at its marker, or at its
        /// disconnect. The `settled` clients must end on a disconnect or a peer-left notice.
        async fn observe(&mut self, settled: &[usize]) -> Vec<Vec<Seen>> {
            let marker = ServerMessage {
                message: Some(server_message::Message::ApplicationData(ApplicationData {
                    room_id: u32::MAX,
                    sender_id: u32::MAX,
                    payload: b"observed".to_vec(),
                })),
            };
            for peer in self.peers.clone() {
                let packet = Packet::reliable(channel::RELIABLE, marker.encode_to_vec());
                self.transports.send(peer, packet).await.unwrap();
            }
            self.transports.flush();
            let marker = marker.message.unwrap();
            let mut seen: Vec<Vec<Seen>> = self.clients.iter().map(|_| vec![]).collect();
            let mut done = vec![false; self.clients.len()];
            let started = Instant::now();
            while done.contains(&false) {
                assert!(
                    started.elapsed() < HANG_GUARD,
                    "a client never reached its marker: {seen:?}"
                );
                for ((client, seen), done) in self.clients.iter_mut().zip(&mut seen).zip(&mut done)
                {
                    if *done {
                        continue;
                    }
                    match client.service().await.unwrap() {
                        Some(Event::Receive { packet, .. }) => {
                            let message = ServerMessage::decode(packet.data.as_ref())
                                .unwrap()
                                .message
                                .unwrap();
                            if message == marker {
                                *done = true;
                            } else {
                                seen.push(Seen::Message(Box::new(message)));
                            }
                        }
                        Some(Event::Disconnect { .. }) => {
                            seen.push(Seen::Disconnected);
                            *done = true;
                        }
                        _ => {}
                    }
                }
                self.transports.service().await.unwrap();
            }
            for client in settled {
                assert!(
                    closed(&seen[*client]),
                    "client {client} did not settle: {:?}",
                    seen[*client]
                );
            }
            seen
        }
    }

    #[tokio::test]
    async fn immediate_enet_relay_retirement_releases_ip_slot_without_disconnect_callback() {
        let mut rig = Rig::connect(1).await;
        let peer = rig.peers[0];
        rig.server.ip_limiter_options.enabled = true;
        rig.server.ip_limiter_options.max_concurrency = 1;
        rig.server
            .connect_pending(peer, "192.0.2.1".into(), 0)
            .unwrap();
        rig.server
            .deliver_application(&mut rig.transports, vec![], vec![peer])
            .await
            .unwrap();
        assert!(!rig.server.peers.contains_key(&peer));
        // ENet disconnect_now does not yield a server Disconnect event. Its ID can be reused.
        assert_eq!(
            rig.server.connect_pending(peer, "192.0.2.1".into(), 1),
            Ok(())
        );
        assert!(
            rig.server
                .connect_pending(peer + 1, "192.0.2.1".into(), 1)
                .is_err()
        );
    }

    struct ComposedPolicy;
    #[async_trait::async_trait]
    impl crate::gatekeeper::admission::relay_authority::RelayPolicy for ComposedPolicy {
        async fn authorize(
            &self,
            room: &crate::gatekeeper::admission::relay_authority::proof::VerifiedRoom,
        ) -> Result<(), crate::protocol::room_authority::denied::Reason> {
            assert_eq!(room.wallet, "0x1111111111111111111111111111111111111111");
            assert_eq!(room.session, "0x2222222222222222222222222222222222222222");
            assert_eq!(room.claims.video.room, "world-a.eth");
            Ok(())
        }
    }

    // The transport rigs start at an authenticated connection, as do the relay's
    // other transport tests. This exercises client bytes across both real network
    // boundaries; the existing handshake suites own connection authentication.
    async fn composed_authority(mut rig: Rig, webtransport: bool) {
        use crate::gatekeeper::admission::relay_authority::{self, RelayAuthority};
        use crate::protocol::pulse::decentraland::pulse::{
            ApplicationErrorCode, ApplicationJoin, ClientMessage,
        };
        use crate::protocol::room_authority::{AUTHORITY_PATH, PROOF_DOMAIN};
        use crate::pulse::application_relay::http::HttpAuthority;
        use crate::pulse::transport::webtransport::framing::stream_frame;
        use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
        use hmac::{Hmac, KeyInit, Mac};
        use sha2::{Digest, Sha256};
        let authority = Arc::new(
            RelayAuthority::new(Arc::new(ComposedPolicy), "key".into(), vec![7; 32], [9; 32])
                .unwrap(),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!("http://{}{AUTHORITY_PATH}", listener.local_addr().unwrap());
        let authority_server = tokio::spawn(async move {
            axum::serve(listener, relay_authority::router::<()>(authority))
                .await
                .unwrap();
        });
        let peer = rig.peers[0];
        let mut relay = ApplicationRelay::new(Arc::new(
            HttpAuthority::new(&endpoint, &URL_SAFE_NO_PAD.encode([9; 32]), true).unwrap(),
        ));
        relay.authenticated(
            peer,
            [3; 32],
            "0x1111111111111111111111111111111111111111".into(),
            "0x2222222222222222222222222222222222222222".into(),
        );
        rig.server.application_relay = Some(relay);
        let claims = serde_json::json!({"iss":"key", "sub":"0x1111111111111111111111111111111111111111", "exp":chrono::Utc::now().timestamp() + 60,
            "metadata": r#"{"isGuest":false}"#, "video":{"room":"world-a.eth","roomJoin":true,"canPublishData":true}});
        let hp = format!(
            "{}.{}",
            URL_SAFE_NO_PAD.encode(br#"{"alg":"HS256"}"#),
            URL_SAFE_NO_PAD.encode(serde_json::to_vec(&claims).unwrap())
        );
        let mut signature = Hmac::<Sha256>::new_from_slice(&[7; 32]).unwrap();
        signature.update(hp.as_bytes());
        let mut proof = Hmac::<Sha256>::new_from_slice(&signature.finalize().into_bytes()).unwrap();
        proof.update(PROOF_DOMAIN);
        proof.update(&[3; 32]);
        proof.update(&[0x11; 20]);
        proof.update(&[0x22; 20]);
        proof.update(&Sha256::digest(hp.as_bytes()));
        let proof = proof.finalize().into_bytes().to_vec();
        let mut stream = if webtransport {
            Some(rig.sessions[0].open_bi().await.unwrap())
        } else {
            None
        };
        for request_id in 1..=3 {
            let mut supplied = proof.clone();
            if request_id != 2 {
                supplied[0] ^= 1;
            }
            let bytes = ClientMessage {
                message: Some(client_message::Message::ApplicationJoin(ApplicationJoin {
                    request_id,
                    jwt_header_payload: hp.clone(),
                    proof: supplied,
                })),
            }
            .encode_to_vec();
            if let Some((send, _)) = stream.as_mut() {
                send.write_all(&stream_frame(&bytes)).await.unwrap();
            } else {
                rig.clients[0]
                    .send(rig.servers[0], Packet::reliable(channel::RELIABLE, bytes))
                    .await
                    .unwrap();
                rig.clients[0].flush();
            }
            let started = Instant::now();
            loop {
                assert!(
                    started.elapsed() < HANG_GUARD,
                    "composed join transport hung"
                );
                if !webtransport {
                    rig.clients[0].service().await.unwrap();
                }
                if let Some(Event::Receive {
                    peer: sender,
                    packet,
                    ..
                }) = rig.transports.service().await.unwrap()
                {
                    assert_eq!(sender as u32, peer);
                    let message = ClientMessage::decode(packet.data.as_ref())
                        .unwrap()
                        .message
                        .unwrap();
                    rig.apply(peer, message).await;
                    break;
                }
            }
            assert_eq!(
                rig.server
                    .application_relay
                    .as_ref()
                    .unwrap()
                    .state
                    .memberships
                    .is_empty(),
                request_id < 3,
            );
            let (out, retire) = tokio::time::timeout(
                HANG_GUARD,
                rig.server.application_relay.as_mut().unwrap().completed(),
            )
            .await
            .unwrap();
            assert!(retire.is_empty());
            rig.server
                .deliver_application(&mut rig.transports, out, retire)
                .await
                .unwrap();
            rig.transports.flush();
            let message = if let Some((_, recv)) = stream.as_mut() {
                let mut length = [0; 4];
                tokio::time::timeout(HANG_GUARD, recv.read_exact(&mut length))
                    .await
                    .unwrap()
                    .unwrap();
                let mut bytes = vec![0; u32::from_be_bytes(length) as usize];
                tokio::time::timeout(HANG_GUARD, recv.read_exact(&mut bytes))
                    .await
                    .unwrap()
                    .unwrap();
                ServerMessage::decode(bytes.as_slice())
                    .unwrap()
                    .message
                    .unwrap()
            } else {
                loop {
                    assert!(
                        started.elapsed() < HANG_GUARD,
                        "composed response transport hung"
                    );
                    if let Some(Event::Receive { packet, .. }) =
                        rig.clients[0].service().await.unwrap()
                    {
                        break ServerMessage::decode(packet.data.as_ref())
                            .unwrap()
                            .message
                            .unwrap();
                    }
                    rig.transports.service().await.unwrap();
                }
            };
            let server_message::Message::ApplicationJoinResult(result) = message else {
                panic!("expected join result")
            };
            assert_eq!(result.request_id, request_id);
            if request_id != 2 {
                assert_eq!(
                    result.error,
                    ApplicationErrorCode::ApplicationNotAuthorized as i32
                );
                assert_eq!(result.room_id, 0);
            } else {
                assert_eq!(result.error, 0);
                assert_ne!(result.room_id, 0);
                assert_eq!(result.peers.len(), 1);
                assert_eq!(result.peers[0].identity, claims["sub"].as_str().unwrap());
                assert_eq!(result.peers[0].is_guest, Some(false));
            }
        }
        assert!(rig.member(peer, "world-a.eth"));
        authority_server.abort();
    }

    #[tokio::test]
    async fn enet_client_join_crosses_real_http_authority_without_pulse_livekit_configuration() {
        composed_authority(Rig::connect(1).await, false).await;
    }

    #[test]
    fn webtransport_client_join_crosses_real_http_authority_without_pulse_livekit_configuration() {
        let listening = crate::pulse::transport::multi::testing::listen(1);
        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(async move {
                composed_authority(Rig::webtransport(listening, 1).await, true).await;
            });
    }

    #[tokio::test]
    async fn bans_snapshot_disconnects_transport_and_cleans_application_membership() {
        let mut rig = Rig::connect(2).await;
        let banned = rig.peers[0];
        let other = rig.peers[1];
        let wallet = "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        for (peer, address) in [
            (banned, wallet),
            (other, "0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"),
        ] {
            let mut state = crate::pulse::simulation::PeerState::new(
                crate::pulse::simulation::PeerConnectionState::Authenticated,
                0,
            );
            state.wallet_id = Some(address.into());
            rig.server.peers.insert(peer, state);
            rig.server.identity.set(peer, address.into());
            rig.server.board.set_active(peer);
            rig.enrol(peer, "room");
        }
        assert!(rig.member(banned, "room"));
        rig.server
            .apply_bans(&mut rig.transports, vec![wallet.into()])
            .await
            .unwrap();
        assert!(!rig.member(banned, "room"));
        assert!(rig.member(other, "room"));
        assert!(!rig.server.peers.contains_key(&banned));
        assert_eq!(rig.server.identity.wallet_by_peer(banned), None);
        let started = Instant::now();
        loop {
            assert!(
                started.elapsed() < HANG_GUARD,
                "ban did not disconnect the transport"
            );
            if let Some(Event::Disconnect { .. }) = rig.clients[0].service().await.unwrap() {
                break;
            }
            rig.transports.service().await.unwrap();
        }
    }

    fn closed(seen: &[Seen]) -> bool {
        match seen.last() {
            Some(Seen::Disconnected) => true,
            Some(Seen::Message(message)) => matches!(
                message.as_ref(),
                server_message::Message::ApplicationPeerLeft(_)
            ),
            None => false,
        }
    }

    fn left(room_id: u32, peer_id: u32, roster_version: u32) -> Seen {
        Seen::Message(Box::new(server_message::Message::ApplicationPeerLeft(
            ApplicationPeerLeft {
                room_id,
                peer_id,
                roster_version,
            },
        )))
    }

    fn failed(room_id: u32, peer: u32, payload: Vec<u8>) -> Seen {
        Seen::Message(Box::new(server_message::Message::ApplicationSendFailed(
            ApplicationSendFailed {
                room_id,
                identities: vec![format!("0x{peer:040x}")],
                payload,
            },
        )))
    }

    fn data(seen: &Seen) -> Option<&ApplicationData> {
        let Seen::Message(message) = seen else {
            return None;
        };
        match message.as_ref() {
            server_message::Message::ApplicationData(data) => Some(data),
            _ => None,
        }
    }

    #[tokio::test]
    async fn a_full_recipient_queue_retires_that_room_scope_and_keeps_the_pulse_session() {
        let mut rig = Rig::connect(2).await;
        let (sender, recipient) = (rig.peers[SENDER], rig.peers[RECIPIENT]);
        let sender_room = rig.enrol(sender, "busy");
        let recipient_room = rig.enrol(recipient, "busy");
        rig.enrol(recipient, "calm");
        for _ in 0..40 {
            rig.send(sender, sender_room, vec![7; 3072]).await;
        }
        let seen = rig.observe(&[SENDER, RECIPIENT]).await;
        assert!(
            !seen[RECIPIENT].contains(&Seen::Disconnected),
            "a full application queue disconnected a healthy recipient from Pulse"
        );
        assert!(data(&seen[RECIPIENT][0]).is_some_and(|data| data.room_id == recipient_room));
        assert_eq!(
            seen[RECIPIENT].last(),
            Some(&left(recipient_room, recipient_room, 3))
        );
        assert!(!rig.member(recipient, "busy"));
        assert!(rig.member(recipient, "calm"));
        assert!(rig.member(sender, "busy"));
        assert_eq!(seen[SENDER], vec![left(sender_room, recipient_room, 3)]);
        assert!(rig.connected(recipient).await);
    }

    #[tokio::test]
    async fn deliveries_queued_behind_a_retired_scope_are_not_sent() {
        let mut rig = Rig::connect(2).await;
        let (sender, recipient) = (rig.peers[SENDER], rig.peers[RECIPIENT]);
        let sender_room = rig.enrol(sender, "busy");
        let recipient_room = rig.enrol(recipient, "busy");
        let relay = rig.server.application_relay.as_mut().unwrap();
        let mut batch = vec![];
        for payload in std::iter::repeat_n(vec![7; 3072], 30).chain([vec![9]]) {
            let send = ApplicationSend {
                room_id: sender_room,
                payload,
                ..Default::default()
            };
            batch.extend(relay.send(sender, send, Instant::now()).0);
        }
        rig.server
            .deliver_application(&mut rig.transports, batch, vec![])
            .await
            .unwrap();
        let seen = rig.observe(&[RECIPIENT]).await;
        assert!(
            !seen[RECIPIENT].contains(&Seen::Disconnected),
            "a full application queue disconnected a healthy recipient from Pulse"
        );
        assert!(
            !seen[RECIPIENT]
                .iter()
                .filter_map(data)
                .any(|data| data.payload == [9])
        );
        assert_eq!(
            seen[RECIPIENT].last(),
            Some(&left(recipient_room, recipient_room, 3))
        );
    }

    #[tokio::test]
    async fn a_removed_member_whose_notice_cannot_be_queued_is_disconnected() {
        let mut rig = Rig::connect(2).await;
        let (sender, recipient) = (rig.peers[SENDER], rig.peers[RECIPIENT]);
        let sender_room = rig.enrol(sender, "busy");
        rig.enrol(recipient, "busy");
        rig.enrol(recipient, "calm");
        for _ in 0..40 {
            rig.send(sender, sender_room, vec![7; 3072]).await;
        }
        assert!(!rig.member(recipient, "busy"));
        assert!(rig.member(recipient, "calm"));
        assert!(rig.connected(recipient).await);
        for _ in 0..400 {
            if !rig.member(recipient, "calm") {
                break;
            }
            let relay = rig.server.application_relay.as_mut().unwrap();
            let now = Instant::now();
            let mut verified = relay
                .state
                .membership(recipient, "calm")
                .unwrap()
                .scope
                .clone();
            verified.admitted.identity = "0xabsent".into();
            let joined =
                relay
                    .state
                    .join(ABSENT_PEER, 1, verified, now + Duration::from_secs(60), now);
            rig.server
                .deliver_application(&mut rig.transports, joined, vec![])
                .await
                .unwrap();
            assert!(!rig.member(ABSENT_PEER, "calm"));
        }
        assert!(!rig.member(recipient, "calm"));
        assert!(!rig.member(recipient, "busy"));
        let relay = rig.server.application_relay.as_ref().unwrap();
        assert!(relay.nonce(recipient).is_none());
        assert!(!rig.connected(recipient).await);
        assert!(rig.connected(sender).await);
    }

    #[tokio::test]
    async fn an_over_limit_reliable_sender_falls_back_without_losing_the_pulse_session() {
        let mut rig = Rig::connect(2).await;
        let (sender, recipient) = (rig.peers[SENDER], rig.peers[RECIPIENT]);
        let sender_room = rig.enrol(sender, "busy");
        let recipient_room = rig.enrol(recipient, "busy");
        for _ in 0..16 + 2 * 60 {
            rig.send(sender, sender_room, vec![1]).await;
        }
        let seen = rig.observe(&[SENDER, RECIPIENT]).await;
        assert_eq!(
            seen[SENDER],
            vec![left(sender_room, sender_room, 3)],
            "an over-limit reliable send must retire the sender's scope, not its Pulse session"
        );
        assert!(!rig.member(sender, "busy"));
        let relay = rig.server.application_relay.as_ref().unwrap();
        assert!(relay.nonce(sender).is_some());
        assert!(rig.connected(sender).await);
        assert_eq!(seen[RECIPIENT].iter().filter_map(data).count(), 120);
        assert_eq!(
            seen[RECIPIENT].last(),
            Some(&left(recipient_room, sender_room, 3))
        );
    }

    #[tokio::test]
    async fn a_full_queue_answers_a_notice_capable_sender_for_the_blocked_and_the_purged_payloads()
    {
        let mut rig = Rig::connect(3).await;
        let (capable, recipient, legacy) = (
            rig.peers[SENDER],
            rig.peers[RECIPIENT],
            rig.peers[BYSTANDER],
        );
        let capable_room = rig.enrol(capable, "busy");
        let recipient_room = rig.enrol(recipient, "busy");
        let legacy_room = rig.enrol(legacy, "busy");
        let relay = rig.server.application_relay.as_mut().unwrap();
        let mut batch = vec![];
        for index in 0..32u8 {
            let (peer, room_id, roster_version) = if (24..28).contains(&index) {
                (legacy, legacy_room, 0)
            } else {
                (capable, capable_room, 3)
            };
            let send = ApplicationSend {
                room_id,
                recipient_id: recipient_room,
                payload: vec![index; 3072],
                unreliable: false,
                roster_version,
            };
            batch.extend(relay.send(peer, send, Instant::now()).0);
        }
        assert_eq!(batch.len(), 32);
        rig.server
            .deliver_application(&mut rig.transports, batch, vec![])
            .await
            .unwrap();
        let seen = rig.observe(&[RECIPIENT]).await;
        let delivered = seen[RECIPIENT].iter().filter_map(data).count() as u8;
        assert!((1..24).contains(&delivered));
        let mut expected = vec![left(capable_room, recipient_room, 4)];
        expected.extend(
            (delivered..24)
                .chain(28..32)
                .map(|index| failed(capable_room, recipient, vec![index; 3072])),
        );
        assert_eq!(seen[SENDER], expected);
        assert_eq!(
            seen[BYSTANDER],
            vec![left(legacy_room, recipient_room, 4)],
            "a sender that never stamped a roster version must not be sent the new notice"
        );
        assert!(rig.member(capable, "busy"));
        assert!(rig.member(legacy, "busy"));
    }

    #[tokio::test]
    async fn a_send_failed_notice_that_cannot_be_queued_retires_the_senders_scope_unanswered() {
        let mut rig = Rig::connect(2).await;
        let (sender, recipient) = (rig.peers[SENDER], rig.peers[RECIPIENT]);
        let sender_room = rig.enrol(sender, "busy");
        let recipient_room = rig.enrol(recipient, "busy");
        let filler = ServerMessage {
            message: Some(server_message::Message::ApplicationData(ApplicationData {
                room_id: u32::MAX,
                sender_id: 0,
                payload: vec![0; 3072],
            })),
        }
        .encode_to_vec();
        loop {
            let packet = Packet::reliable(channel::RELIABLE, filler.clone());
            if rig
                .transports
                .send_application(sender, packet)
                .await
                .is_err()
            {
                break;
            }
        }
        for index in 0..40u8 {
            let send = ApplicationSend {
                room_id: sender_room,
                payload: vec![index; 3072],
                roster_version: 2,
                ..Default::default()
            };
            rig.apply(sender, client_message::Message::ApplicationSend(send))
                .await;
        }
        assert!(!rig.member(recipient, "busy"));
        assert!(
            !rig.member(sender, "busy"),
            "a failure notice that cannot be queued must retire the sender's scope"
        );
        let seen = rig.observe(&[SENDER, RECIPIENT]).await;
        let notices = |client: usize| {
            seen[client]
                .iter()
                .filter(|seen| data(seen).is_none())
                .collect::<Vec<_>>()
        };
        assert_eq!(
            notices(SENDER),
            vec![
                &left(sender_room, recipient_room, 3),
                &left(sender_room, sender_room, 4)
            ]
        );
        assert_eq!(
            notices(RECIPIENT),
            vec![&left(recipient_room, recipient_room, 3)]
        );
        let relay = rig.server.application_relay.as_ref().unwrap();
        assert!(relay.nonce(sender).is_some());
        assert!(rig.connected(sender).await);
        assert!(rig.connected(recipient).await);
    }

    #[tokio::test]
    async fn a_connection_holding_a_relay_scope_has_a_flat_five_second_enet_timeout_until_its_last_scope_leaves()
     {
        let mut rig = Rig::connect(2).await;
        let (member, idle) = (rig.peers[SENDER], rig.peers[RECIPIENT]);
        let (default, relay) = (Some((5000, 30000)), Some((5000, 5000)));
        assert_eq!(rig.transports.enet_timeout(member), default);
        let busy = rig.admit(member, "busy").await;
        let calm = rig.admit(member, "calm").await;
        assert_eq!(rig.transports.enet_timeout(member), relay);
        assert_eq!(rig.transports.enet_timeout(idle), default);
        let leave =
            |room_id| client_message::Message::ApplicationLeave(ApplicationLeave { room_id });
        rig.apply(member, leave(busy)).await;
        assert_eq!(rig.transports.enet_timeout(member), relay);
        rig.apply(member, leave(calm)).await;
        assert_eq!(rig.transports.enet_timeout(member), default);
        assert_eq!(rig.transports.enet_timeout(idle), default);
    }

    #[test]
    fn a_silent_webtransport_player_loses_its_relay_scopes_in_five_seconds_and_keeps_its_session() {
        use crate::pulse::transport::silence::RELAY_SILENCE;
        use crate::pulse::transport::webtransport::framing::stream_frame;
        let listening = crate::pulse::transport::multi::testing::listen(3);
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async move {
            let mut rig = Rig::webtransport(listening, 3).await;
            let (quiet, talking, listener) = (rig.peers[0], rig.peers[1], rig.peers[2]);
            rig.server.peers.get_mut(&listener).unwrap().scene_listener =
                Some(Arc::new(crate::pulse::interest::SceneListenerState::new(
                    Default::default(),
                    Default::default(),
                )));
            let short = RELAY_SILENCE - Duration::from_nanos(1);
            let joining = Instant::now();
            let busy = rig.admit(quiet, "busy").await;
            let calm = rig.admit(quiet, "calm").await;
            let talking_room = rig.admit(talking, "busy").await;
            let listener_room = rig.admit(listener, "busy").await;
            let fresh = rig
                .server
                .silent_relay_scopes(&rig.transports, joining + short);
            assert!(fresh.is_empty());

            let (mut send, _recv) = rig.sessions[1].open_bi().await.unwrap();
            let speaking = Instant::now();
            send.write_all(&stream_frame(b"input")).await.unwrap();
            loop {
                assert!(speaking.elapsed() < HANG_GUARD, "WebTransport receive hung");
                if let Some(Event::Receive { peer, .. }) = rig.transports.service().await.unwrap() {
                    assert_eq!(peer as u32, talking);
                    break;
                }
            }

            let out = rig
                .server
                .silent_relay_scopes(&rig.transports, speaking + short);
            let mut told: Vec<(u32, u32)> = out
                .iter()
                .map(
                    |delivery| match delivery.message.message.as_ref().unwrap() {
                        server_message::Message::ApplicationPeerLeft(left) => {
                            (delivery.target, left.room_id)
                        }
                        other => panic!("unexpected delivery {other:?}"),
                    },
                )
                .collect();
            told.sort_unstable();
            let mut expected = vec![
                (quiet, busy),
                (quiet, calm),
                (talking, talking_room),
                (listener, listener_room),
            ];
            expected.sort_unstable();
            let actor = |room: &str| {
                let relay = rig.server.application_relay.as_ref().unwrap();
                relay.state.membership(quiet, room)
            };
            assert!(actor("busy").is_none() && actor("calm").is_none());
            assert_eq!(told, expected);
            rig.server
                .deliver_application(&mut rig.transports, out, vec![])
                .await
                .unwrap();

            assert!(rig.member(talking, "busy"));
            assert!(rig.member(listener, "busy"));
            assert!(rig.connected(quiet).await);
            let relay = rig.server.application_relay.as_ref().unwrap();
            assert!(relay.nonce(quiet).is_some());
            let later = Instant::now() + RELAY_SILENCE;
            assert_eq!(rig.transports.silent_relay_peers(later), vec![talking]);

            rig.admit(quiet, "busy").await;
            let later = Instant::now() + RELAY_SILENCE;
            assert_eq!(
                rig.transports.silent_relay_peers(later),
                vec![quiet, talking]
            );
            let out = rig.server.silent_relay_scopes(&rig.transports, later);
            rig.server
                .deliver_application(&mut rig.transports, out, vec![])
                .await
                .unwrap();
            assert!(!rig.member(quiet, "busy") && !rig.member(talking, "busy"));
            assert!(rig.member(listener, "busy"));
        });
    }

    #[tokio::test]
    async fn a_silently_dead_relay_member_is_removed_in_about_five_seconds_and_an_idle_connection_is_kept()
     {
        let clock = ManualClock::new();
        let mut rig = Rig::connect_on(3, Some(clock.clone())).await;
        let (sender, dead, idle) = (
            rig.peers[SENDER],
            rig.peers[RECIPIENT],
            rig.peers[BYSTANDER],
        );
        let sender_room = rig.admit(sender, "busy").await;
        rig.admit(dead, "busy").await;
        rig.slow_round_trips(&clock, &[RECIPIENT, BYSTANDER]).await;
        // The server's clock steps 50 ms only once the sender has answered, so the sends to the
        // dead member are the only ones it ever sees go unacknowledged.
        let silent = clock.now();
        let mut disconnected = vec![];
        let removed_after = loop {
            let elapsed = clock.now() - silent;
            assert!(
                elapsed < Duration::from_secs(8),
                "a silent relay member is still on the roster after {elapsed:?}"
            );
            rig.send(sender, sender_room, vec![1]).await;
            rig.probe(&[SENDER]).await;
            disconnected.extend(rig.answer(&[SENDER]).await);
            if !rig.member(dead, "busy") {
                break clock.now() - silent;
            }
            clock.advance(Duration::from_millis(50));
        };
        assert!(
            removed_after > Duration::from_millis(4500)
                && removed_after < Duration::from_millis(6500),
            "the silent relay member left the roster after {removed_after:?}"
        );
        assert_eq!(disconnected, vec![dead]);
        assert!(rig.member(sender, "busy"));
        assert!(rig.connected(idle).await);
    }
}
