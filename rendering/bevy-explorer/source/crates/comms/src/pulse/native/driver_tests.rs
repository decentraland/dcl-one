use tokio::sync::mpsc::error::TryRecvError;

use super::*;
use crate::pulse::transport::pulse_channels;

fn address(text: &str) -> SocketAddr {
    text.parse().unwrap()
}

#[test]
fn prefers_the_first_ipv4_address() {
    let picked = pick_address([
        address("[::1]:7777"),
        address("10.0.0.2:7777"),
        address("10.0.0.3:7777"),
    ]);

    assert_eq!(picked, Some(address("10.0.0.2:7777")));
}

#[test]
fn falls_back_to_the_first_ipv6_address() {
    let picked = pick_address([address("[::1]:7777"), address("[::2]:7777")]);

    assert_eq!(picked, Some(address("[::1]:7777")));
    assert_eq!(pick_address([]), None);
}

#[test]
fn a_full_queue_holds_reliable_and_drops_unreliable() {
    let (tx, mut rx) = mpsc::channel(2);
    let mut backlog = Backlog::with_caps(8, MAX_HELD_BYTES);

    backlog.deliver(&tx, vec![1], true).unwrap();
    backlog.deliver(&tx, vec![2], true).unwrap();
    // The queue is full now: the reliable message is held, the unreliable one dropped.
    backlog.deliver(&tx, vec![3], true).unwrap();
    backlog.deliver(&tx, vec![4], false).unwrap();

    assert_eq!(rx.try_recv(), Ok(vec![1]));
    assert_eq!(rx.try_recv(), Ok(vec![2]));
    backlog.flush(&tx);
    assert_eq!(rx.try_recv(), Ok(vec![3]));
    assert_eq!(rx.try_recv(), Err(TryRecvError::Empty));
}

#[test]
fn a_reliable_message_never_overtakes_a_held_one() {
    let (tx, mut rx) = mpsc::channel(1);
    let mut backlog = Backlog::with_caps(8, MAX_HELD_BYTES);

    backlog.deliver(&tx, vec![1], true).unwrap();
    backlog.deliver(&tx, vec![2], true).unwrap();
    assert_eq!(rx.try_recv(), Ok(vec![1]));

    // There is room again, but the second message is still held, so the third queues behind it.
    backlog.deliver(&tx, vec![3], true).unwrap();
    backlog.flush(&tx);
    assert_eq!(rx.try_recv(), Ok(vec![2]));
    assert_eq!(rx.try_recv(), Err(TryRecvError::Empty));
    backlog.flush(&tx);
    assert_eq!(rx.try_recv(), Ok(vec![3]));
}

#[test]
fn holding_past_the_message_cap_is_an_error() {
    let (tx, _rx) = mpsc::channel(1);
    let mut backlog = Backlog::with_caps(1, MAX_HELD_BYTES);

    backlog.deliver(&tx, vec![1], true).unwrap();
    backlog.deliver(&tx, vec![2], true).unwrap();

    assert_eq!(backlog.deliver(&tx, vec![3], true), Err(BacklogFull));
}

#[test]
fn holding_past_the_byte_cap_is_an_error() {
    let (tx, mut rx) = mpsc::channel(1);
    let mut backlog = Backlog::with_caps(8, 4);

    backlog.deliver(&tx, vec![1], true).unwrap();
    // The queue is full now: three bytes are held, and two more would be over the four.
    backlog.deliver(&tx, vec![2; 3], true).unwrap();
    assert_eq!(backlog.deliver(&tx, vec![3; 2], true), Err(BacklogFull));

    // Handed over, a held message frees its bytes: with the queue full again, all four can be
    // held at once.
    assert_eq!(rx.try_recv(), Ok(vec![1]));
    backlog.flush(&tx);
    assert_eq!(backlog.deliver(&tx, vec![4; 4], true), Ok(()));
    assert_eq!(rx.try_recv(), Ok(vec![2; 3]));
}

/// A host on a loopback socket of its own, with the driver's settings.
fn loopback_host() -> enet::Host<std::net::UdpSocket> {
    enet::Host::new(
        std::net::UdpSocket::bind("127.0.0.1:0").unwrap(),
        enet::HostSettings {
            peer_limit: 1,
            channel_limit: CHANNEL_COUNT,
            ..Default::default()
        },
    )
    .unwrap()
}

/// A driver that gives up on a connected peer, its inbound backlog over a cap or ENet's service
/// failing, tells the server at once. Otherwise the server holds the peer until its own timeout
/// notices, which takes ENet at least 5 s. This covers a connected peer only: before VERIFY_CONNECT
/// the notice cannot reach the server (see `abandon`).
#[test]
fn abandoning_a_connection_tells_the_server_at_once() {
    let (mut client, peer, mut server) = connected_pair();

    let (mut link, mut channels) = pulse_channels(1);
    abandon(&mut client, peer, &mut channels, "gave up".to_owned());

    let deadline = Instant::now() + Duration::from_secs(1);
    let reason = loop {
        assert!(Instant::now() < deadline, "the server was not told");
        if let Ok(Some(enet::Event::Disconnect { data, .. })) = server.service() {
            break data;
        }
        thread::sleep(Duration::from_millis(1));
    };
    assert_eq!(PulseDisconnect::from_code(reason), PulseDisconnect::None);
    assert!(matches!(
        link.status.try_recv(),
        Ok(PulseStatus::Failed(message)) if message == "gave up"
    ));
}

/// Two loopback hosts, connected: the client and its peer id, and the server.
fn connected_pair() -> (
    enet::Host<std::net::UdpSocket>,
    enet::PeerID,
    enet::Host<std::net::UdpSocket>,
) {
    let mut server = loopback_host();
    let mut client = loopback_host();
    let address = server.socket().local_addr().unwrap();
    let peer = client.connect(address, CHANNEL_COUNT, 0).unwrap().id();
    let deadline = Instant::now() + Duration::from_secs(5);
    let (mut client_up, mut server_up) = (false, false);
    while !(client_up && server_up) {
        assert!(Instant::now() < deadline, "the connection never came up");
        client_up |= matches!(client.service(), Ok(Some(enet::Event::Connect { .. })));
        server_up |= matches!(server.service(), Ok(Some(enet::Event::Connect { .. })));
        thread::sleep(Duration::from_millis(1));
    }
    (client, peer, server)
}

/// The gate's signal on a live peer: a reliable message and a probe are unacknowledged until the
/// server's acknowledgements arrive, and then nothing is, so the gate does not hold forever.
#[test]
fn a_live_peer_reports_reliable_traffic_unacknowledged_until_acknowledged() {
    let (mut client, peer, mut server) = connected_pair();
    // Settle the connect handshake's own acknowledgements.
    let settle = |client: &mut enet::Host<_>, server: &mut enet::Host<_>| {
        let deadline = Instant::now() + Duration::from_secs(2);
        while client.peer(peer).has_unacknowledged_reliable() {
            assert!(Instant::now() < deadline, "never acknowledged");
            let _ = server.service();
            let _ = client.service();
            thread::sleep(Duration::from_millis(1));
        }
    };
    settle(&mut client, &mut server);
    // The acknowledgement mark is in whole milliseconds of the host clock.
    thread::sleep(Duration::from_millis(2));

    let (channel, packet) = packet_for_frame(&frame(FrameKind::Emote, 1));
    client.peer_mut(peer).send(channel, &packet).unwrap();
    client.peer_mut(peer).ping();
    client.flush();
    assert!(client.peer(peer).has_unacknowledged_reliable());
    // The ping is in flight but is no message.
    assert_eq!(client.peer(peer).unacknowledged_reliable_packets(), 1);
    let mark = client.peer(peer).last_acknowledgement();

    settle(&mut client, &mut server);
    assert!(!client.peer(peer).has_unacknowledged_reliable());
    assert_eq!(client.peer(peer).unacknowledged_reliable_packets(), 0);
    assert_ne!(client.peer(peer).last_acknowledgement(), mark);
}

#[test]
fn transient_socket_errors_leave_the_session_to_enets_timeout() {
    for kind in [
        ErrorKind::WouldBlock,
        ErrorKind::ConnectionReset,
        ErrorKind::ConnectionRefused,
        ErrorKind::NetworkUnreachable,
        ErrorKind::HostUnreachable,
        ErrorKind::NetworkDown,
    ] {
        assert!(is_transient(&io::Error::from(kind)), "{kind:?}");
    }
    if let Some(code) = ENOBUFS {
        assert!(is_transient(&io::Error::from_raw_os_error(code)));
    }
}

#[test]
fn other_socket_errors_still_end_the_session() {
    for kind in [
        ErrorKind::PermissionDenied,
        ErrorKind::InvalidInput,
        ErrorKind::AddrNotAvailable,
        ErrorKind::Other,
    ] {
        assert!(!is_transient(&io::Error::from(kind)), "{kind:?}");
    }
}

fn frame(kind: FrameKind, tag: u8) -> PulseFrame {
    PulseFrame {
        bytes: vec![tag],
        reliability: if kind == FrameKind::Input {
            PulseReliability::UnreliableSequenced
        } else {
            PulseReliability::Reliable
        },
        kind,
    }
}

fn withdraw() -> PulseFrame {
    PulseFrame {
        bytes: Vec::new(),
        reliability: PulseReliability::Reliable,
        kind: FrameKind::WithdrawTeleport,
    }
}

fn tags(frames: &[PulseFrame]) -> Vec<(FrameKind, u8)> {
    frames.iter().map(|f| (f.kind, f.bytes[0])).collect()
}

/// `packets` reliable messages in flight, the last acknowledgement at mark `acknowledged`.
fn in_flight(packets: usize, acknowledged: u32) -> PeerAcks {
    PeerAcks {
        unacknowledged: true,
        packets,
        last_acknowledgement: acknowledged,
        ..Default::default()
    }
}

/// Everything acknowledged, the last at mark `acknowledged`.
fn settled(acknowledged: u32) -> PeerAcks {
    PeerAcks {
        unacknowledged: false,
        packets: 0,
        last_acknowledgement: acknowledged,
        ..Default::default()
    }
}

#[test]
fn an_acknowledged_uplink_lets_everything_through() {
    let mut gate = UplinkGate::default();
    let now = Instant::now();
    // Briefly unacknowledged, as after any send, is no stall.
    gate.observe(now, in_flight(1, 1));
    let out = gate.admit(
        vec![frame(FrameKind::Emote, 1), frame(FrameKind::Input, 2)],
        now + STALL_AFTER / 2,
    );

    assert_eq!(tags(&out), [(FrameKind::Emote, 1), (FrameKind::Input, 2)]);
}

/// Overlapping reliable sends on a healthy link keep something in flight all the time; each
/// acknowledgement that arrives shows the uplink works, so it never reads as stalled.
#[test]
fn a_busy_healthy_link_never_reads_as_stalled() {
    let mut gate = UplinkGate::default();
    let start = Instant::now();

    for step in 0..30u32 {
        let now = start + Duration::from_millis(100) * step;
        // Two teleports or emotes always in flight at a 150 ms round trip, one acknowledged a pass.
        gate.observe(now, in_flight(2, step + 1));
        let out = gate.admit(
            vec![
                frame(FrameKind::Teleport, step as u8),
                frame(FrameKind::Input, 100),
            ],
            now,
        );
        assert_eq!(out.len(), 2, "held at step {step}");
        assert!(!gate.snapshot(now).stalled);
    }
}

#[test]
fn a_stalled_uplink_holds_only_the_latest_input_until_it_recovers() {
    let mut gate = UplinkGate::default();
    let start = Instant::now();
    gate.observe(start, in_flight(0, 1));
    let stalled = start + STALL_AFTER;

    for tag in 0..30 {
        gate.observe(stalled, in_flight(0, 1));
        assert!(gate
            .admit(vec![frame(FrameKind::Input, tag)], stalled)
            .is_empty());
    }
    // A handshake, a resync or an announcement is never held.
    let control = gate.admit(vec![frame(FrameKind::Control, 99)], stalled);
    assert_eq!(tags(&control), [(FrameKind::Control, 99)]);

    // An acknowledgement arrives: the latest input goes.
    gate.observe(stalled, in_flight(1, 2));
    let out = gate.admit(Vec::new(), stalled);
    assert_eq!(tags(&out), [(FrameKind::Input, 29)]);
}

#[test]
fn unacknowledged_teleports_and_emotes_stay_within_half_the_servers_burst() {
    let mut gate = UplinkGate::default();
    let now = Instant::now();
    gate.observe(now, in_flight(0, 1));

    let mut let_out = 0;
    for tag in 0..20u8 {
        let kind = if tag % 2 == 0 {
            FrameKind::Emote
        } else {
            FrameKind::Teleport
        };
        let_out += gate.admit(vec![frame(kind, tag)], now).len();
        // An input goes while nothing is held, and queues behind a held start, which carries an
        // older pose.
        let inputs = gate.admit(vec![frame(FrameKind::Input, 100 + tag)], now);
        let expected = if usize::from(tag) < MAX_UNACKNOWLEDGED_RELIABLE {
            1
        } else {
            0
        };
        assert_eq!(inputs.len(), expected, "tag {tag}");
    }
    assert_eq!(let_out, MAX_UNACKNOWLEDGED_RELIABLE);
    const _: () = assert!(MAX_UNACKNOWLEDGED_RELIABLE * 2 <= 16);

    // Everything acknowledged: the latest of each kind goes, the input after them.
    gate.observe(now, settled(2));
    let out = gate.admit(Vec::new(), now);
    assert_eq!(
        tags(&out),
        [
            (FrameKind::Emote, 18),
            (FrameKind::Teleport, 19),
            (FrameKind::Input, 119)
        ]
    );
}

/// Handshakes, resyncs and announcements in flight count toward the limit too.
#[test]
fn reliable_control_messages_in_flight_count_toward_the_limit() {
    let mut gate = UplinkGate::default();
    let now = Instant::now();
    gate.observe(now, in_flight(MAX_UNACKNOWLEDGED_RELIABLE, 1));

    assert!(gate.admit(vec![frame(FrameKind::Emote, 1)], now).is_empty());
    gate.observe(now, in_flight(MAX_UNACKNOWLEDGED_RELIABLE - 1, 2));
    assert_eq!(tags(&gate.admit(Vec::new(), now)), [(FrameKind::Emote, 1)]);
}

/// A start and its stop in one pass both go, in order, on a healthy link. Held back, only the stop
/// waits, as the pacer keeps only the latest emote command.
#[test]
fn an_emote_start_and_stop_together() {
    let mut gate = UplinkGate::default();
    let now = Instant::now();
    gate.observe(now, settled(1));
    let out = gate.admit(
        vec![frame(FrameKind::Emote, 1), frame(FrameKind::Emote, 2)],
        now,
    );
    assert_eq!(tags(&out), [(FrameKind::Emote, 1), (FrameKind::Emote, 2)]);

    gate.observe(now, in_flight(MAX_UNACKNOWLEDGED_RELIABLE, 1));
    let out = gate.admit(
        vec![frame(FrameKind::Emote, 3), frame(FrameKind::Emote, 4)],
        now,
    );
    assert!(out.is_empty());
    gate.observe(now, settled(2));
    assert_eq!(tags(&gate.admit(Vec::new(), now)), [(FrameKind::Emote, 4)]);
}

/// A realm change withdraws a held teleport, which names the realm left; a teleport offered after
/// the withdrawal is the new realm's and stays.
#[test]
fn a_realm_change_withdraws_a_held_teleport() {
    let mut gate = UplinkGate::default();
    let start = Instant::now();
    gate.observe(start, in_flight(0, 1));
    let stalled = start + STALL_AFTER;
    gate.admit(vec![frame(FrameKind::Teleport, 1)], stalled);
    assert!(gate.snapshot(stalled).held_teleport);

    // Never sent itself.
    assert!(gate.admit(vec![withdraw()], stalled).is_empty());
    assert!(!gate.snapshot(stalled).held_teleport);
    gate.observe(stalled, settled(2));
    assert!(gate.admit(Vec::new(), stalled).is_empty());

    gate.observe(stalled, in_flight(0, 2));
    let later = stalled + STALL_AFTER;
    gate.admit(
        vec![
            frame(FrameKind::Teleport, 2),
            withdraw(),
            frame(FrameKind::Teleport, 3),
        ],
        later,
    );
    gate.observe(later, settled(3));
    assert_eq!(
        tags(&gate.admit(Vec::new(), later)),
        [(FrameKind::Teleport, 3)]
    );
}

/// An input held behind a teleport goes in the pass that withdraws that teleport.
#[test]
fn a_withdrawal_releases_what_waited_behind_the_teleport_at_once() {
    let mut gate = UplinkGate::default();
    let now = Instant::now();
    gate.observe(now, in_flight(MAX_UNACKNOWLEDGED_RELIABLE, 1));
    let out = gate.admit(
        vec![frame(FrameKind::Teleport, 1), frame(FrameKind::Input, 2)],
        now,
    );
    assert!(out.is_empty());

    assert_eq!(
        tags(&gate.admit(vec![withdraw()], now)),
        [(FrameKind::Input, 2)]
    );
    assert!(gate.held.is_empty());
}

#[test]
fn the_snapshot_reports_what_the_gate_holds() {
    let mut gate = UplinkGate::default();
    let start = Instant::now();
    gate.observe(start, in_flight(0, 1));
    let stalled = start + STALL_AFTER;
    gate.admit(
        vec![frame(FrameKind::Teleport, 1), frame(FrameKind::Input, 2)],
        stalled,
    );

    assert_eq!(
        gate.snapshot(stalled),
        UplinkSnapshot {
            stalled: true,
            unacknowledged_reliable: 0,
            held_teleport: true,
            held_emote: false,
            held_input: true,
            probes: 0,
            input_holds: 1,
            ..Default::default()
        }
    );
    let later = stalled + Duration::from_millis(300);
    assert_eq!(
        gate.snapshot(later).longest_input_hold,
        Duration::from_millis(300)
    );
    gate.observe(later, settled(2));
    gate.admit(Vec::new(), later);
    assert_eq!(
        gate.snapshot(later),
        UplinkSnapshot {
            unacknowledged_reliable: 1,
            input_holds: 1,
            input_held_for: Duration::from_millis(300),
            longest_input_hold: Duration::from_millis(300),
            ..Default::default()
        }
    );
}

/// Holds add up over the connection, and the longest is kept; the round trip and the spacing it
/// sets are reported as observed.
#[test]
fn the_snapshot_counts_input_holds_and_reports_the_round_trip() {
    let mut gate = UplinkGate::default();
    let start = Instant::now();
    let round_trip = Duration::from_millis(900);
    let mut at = start;
    for (mark, hold) in [(1, 200u64), (2, 50)] {
        gate.observe(at, with_round_trip(in_flight(0, mark), round_trip));
        at += STALL_AFTER;
        assert!(gate.admit(vec![frame(FrameKind::Input, 1)], at).is_empty());
        at += Duration::from_millis(hold);
        gate.observe(at, with_round_trip(settled(mark + 1), round_trip));
        assert_eq!(gate.admit(Vec::new(), at).len(), 1);
    }

    let snapshot = gate.snapshot(at);
    assert_eq!(snapshot.input_holds, 2);
    assert_eq!(snapshot.input_held_for, Duration::from_millis(250));
    assert_eq!(snapshot.longest_input_hold, Duration::from_millis(200));
    assert_eq!(snapshot.round_trip, round_trip);
    assert_eq!(
        snapshot.input_spacing,
        (round_trip + STALL_AFTER) / INPUTS_PER_STALL_WINDOW
    );
}

/// A probe goes out every interval whatever is in flight, so on a link with a long round trip
/// several are in flight and their acknowledgements arrive about an interval apart.
#[test]
fn the_uplink_is_probed_every_interval_whatever_is_in_flight() {
    let mut gate = UplinkGate::default();
    let now = Instant::now();

    assert!(gate.wants_probe(now));
    assert!(!gate.wants_probe(now + PROBE_INTERVAL / 2));
    gate.observe(now + PROBE_INTERVAL, in_flight(0, 1));
    assert!(gate.wants_probe(now + PROBE_INTERVAL));
    gate.observe(now + PROBE_INTERVAL * 2, in_flight(3, 1));
    assert!(gate.wants_probe(now + PROBE_INTERVAL * 2));
    assert_eq!(gate.snapshot(now).probes, 3);
}

fn with_round_trip(acks: PeerAcks, round_trip: Duration) -> PeerAcks {
    PeerAcks { round_trip, ..acks }
}

/// Below `PACE_INPUTS_FROM` inputs go as they come; from it on, at most one per spacing, the
/// latest, with the driver woken when it is due. Waiting for its spacing is not a hold.
#[test]
fn inputs_are_spaced_by_a_long_round_trip() {
    let mut gate = UplinkGate::default();
    let now = Instant::now();
    gate.observe(now, with_round_trip(settled(1), PACE_INPUTS_FROM / 2));
    for step in 0..3u8 {
        let at = now + Duration::from_millis(10) * u32::from(step);
        assert_eq!(gate.admit(vec![frame(FrameKind::Input, step)], at).len(), 1);
    }

    let round_trip = Duration::from_secs(2);
    gate.observe(now, with_round_trip(settled(1), round_trip));
    let spacing = (round_trip + STALL_AFTER) / INPUTS_PER_STALL_WINDOW;
    assert_eq!(gate.input_spacing(), spacing);
    // The last input went at 20 ms.
    let sent = now + Duration::from_millis(20) + spacing;
    assert_eq!(gate.admit(vec![frame(FrameKind::Input, 10)], sent).len(), 1);
    let early = sent + spacing / 2;
    assert!(gate
        .admit(vec![frame(FrameKind::Input, 11)], early)
        .is_empty());
    assert!(gate
        .admit(vec![frame(FrameKind::Input, 12)], early)
        .is_empty());
    assert!(!gate.snapshot(early).held_input);
    assert_eq!(
        gate.wake_within(early),
        (spacing / 2).min(MAINTENANCE_INTERVAL)
    );

    let due = sent + spacing;
    assert_eq!(gate.wake_within(due), MAINTENANCE_INTERVAL);
    // A newer input arriving as the held one is due replaces it.
    assert_eq!(
        tags(&gate.admit(vec![frame(FrameKind::Input, 13)], due)),
        [(FrameKind::Input, 13)]
    );
}

/// A teleport overtaking an input held for its spacing drops it: the servers would otherwise get
/// the teleport and then the pose queued before it. The next input still goes a spacing after the
/// last one.
#[test]
fn a_teleport_drops_an_input_held_for_its_spacing() {
    let mut gate = UplinkGate::default();
    let now = Instant::now();
    gate.observe(now, with_round_trip(settled(1), Duration::from_millis(800)));
    assert_eq!(gate.input_spacing(), Duration::from_millis(175));
    let at = |ms| now + Duration::from_millis(ms);
    assert_eq!(gate.admit(vec![frame(FrameKind::Input, 1)], at(0)).len(), 1);
    assert!(gate
        .admit(vec![frame(FrameKind::Input, 2)], at(50))
        .is_empty());

    assert_eq!(
        tags(&gate.admit(vec![frame(FrameKind::Teleport, 3)], at(80))),
        [(FrameKind::Teleport, 3)]
    );
    assert!(gate.held.is_empty());
    assert_eq!(gate.wake_within(at(80)), MAINTENANCE_INTERVAL);
    assert!(gate.admit(Vec::new(), at(100)).is_empty());

    assert!(gate
        .admit(vec![frame(FrameKind::Input, 4)], at(120))
        .is_empty());
    assert_eq!(
        tags(&gate.admit(Vec::new(), at(175))),
        [(FrameKind::Input, 4)]
    );
}

/// An emote carries no position, so an input held for its spacing still goes after it.
#[test]
fn an_emote_keeps_an_input_held_for_its_spacing() {
    let mut gate = UplinkGate::default();
    let now = Instant::now();
    gate.observe(now, with_round_trip(settled(1), Duration::from_millis(800)));
    let at = |ms| now + Duration::from_millis(ms);
    assert_eq!(gate.admit(vec![frame(FrameKind::Input, 1)], at(0)).len(), 1);
    assert!(gate
        .admit(vec![frame(FrameKind::Input, 2)], at(50))
        .is_empty());

    assert_eq!(
        tags(&gate.admit(vec![frame(FrameKind::Emote, 3)], at(80))),
        [(FrameKind::Emote, 3)]
    );
    assert_eq!(
        tags(&gate.admit(Vec::new(), at(175))),
        [(FrameKind::Input, 2)]
    );
}

/// An input and a newer teleport held behind a stall go in order when it ends; if the input must
/// still wait for its spacing, the teleport goes and the input is dropped.
#[test]
fn an_input_held_with_a_teleport_never_goes_after_it() {
    for (round_trip, expected) in [
        (
            Duration::from_millis(800),
            vec![(FrameKind::Input, 2), (FrameKind::Teleport, 3)],
        ),
        // A spacing of 700 ms, not yet due when the stall ends.
        (Duration::from_secs(5), vec![(FrameKind::Teleport, 3)]),
    ] {
        let mut gate = UplinkGate::default();
        let now = Instant::now();
        let at = |ms| now + Duration::from_millis(ms);
        gate.observe(at(0), with_round_trip(in_flight(1, 1), round_trip));
        assert_eq!(gate.admit(vec![frame(FrameKind::Input, 1)], at(0)).len(), 1);
        assert!(gate
            .admit(vec![frame(FrameKind::Input, 2)], at(600))
            .is_empty());
        assert!(gate
            .admit(vec![frame(FrameKind::Teleport, 3)], at(610))
            .is_empty());

        gate.observe(at(650), with_round_trip(settled(2), round_trip));
        assert_eq!(tags(&gate.admit(Vec::new(), at(650))), expected);
        assert!(gate.held.is_empty());
    }
}

/// The driver itself, against a loopback server: probes go out on an idle link, and an input held
/// while the server stops answering goes on a later pass that no outbound frame woke, once the
/// server answers again.
#[test]
fn the_driver_probes_the_uplink_and_releases_held_frames_on_its_own_wake() {
    let mut server = loopback_host();
    let port = server.socket().local_addr().unwrap().port();
    let (mut link, channels) = pulse_channels(16);
    let stop = Arc::new(AtomicBool::new(false));
    let driver = spawn(
        PulseTransportConfig {
            host: "127.0.0.1".to_owned(),
            port,
            cert_hash: None,
        },
        channels,
        stop.clone(),
    );
    let pump_until =
        |server: &mut enet::Host<std::net::UdpSocket>,
         what: &str,
         mut done: Box<dyn FnMut(Option<(u8, Vec<u8>)>) -> bool + '_>| {
            let deadline = Instant::now() + Duration::from_secs(5);
            loop {
                assert!(Instant::now() < deadline, "{what}");
                let received = match server.service() {
                    Ok(Some(enet::Event::Receive {
                        channel_id, packet, ..
                    })) => Some((channel_id, packet.data().to_vec())),
                    _ => None,
                };
                if done(received) {
                    return;
                }
                thread::sleep(Duration::from_millis(1));
            }
        };

    pump_until(
        &mut server,
        "never connected",
        Box::new(|_| matches!(link.status.try_recv(), Ok(PulseStatus::Connected))),
    );
    // An idle link is probed, and the probes are answered.
    pump_until(
        &mut server,
        "never probed",
        Box::new(|_| {
            link.uplink()
                .is_some_and(|uplink| uplink.probes >= 2 && !uplink.stalled)
        }),
    );

    // The server stops answering: the probe in flight goes unacknowledged until the uplink reads
    // as stalled, and an input sent then is held.
    let deadline = Instant::now() + Duration::from_secs(5);
    while !link.uplink().is_some_and(|uplink| uplink.stalled) {
        assert!(Instant::now() < deadline, "never stalled");
        thread::sleep(Duration::from_millis(10));
    }
    assert!(link.send(frame(FrameKind::Input, 7)));
    let deadline = Instant::now() + Duration::from_secs(5);
    while !link.uplink().is_some_and(|uplink| uplink.held_input) {
        assert!(Instant::now() < deadline, "never held");
        thread::sleep(Duration::from_millis(10));
    }

    // Answering again, with nothing more sent, lets the input through.
    pump_until(
        &mut server,
        "the held input never arrived",
        Box::new(|received| received == Some((1, vec![7]))),
    );

    stop.store(true, Ordering::Relaxed);
    driver.join().unwrap();
}
