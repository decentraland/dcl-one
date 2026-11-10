use crate as enet;

#[allow(dead_code)]
mod network;
use network::*;

#[test]
fn events() {
    let mut network = Network::new();
    let host1 = network.create_host(enet::HostSettings {
        peer_limit: 1,
        ..Default::default()
    });
    let host2 = network.create_host(enet::HostSettings {
        peer_limit: 1,
        ..Default::default()
    });

    network.connect(host1, host2, 255, 5);
    network.update(1);
    let events = network.update(1);
    assert_eq!(events.len(), 2);
    assert!(events[0].is_connect_and(|event| event.to == host1
        && event.from == host2
        && event.peer == enet::PeerID(0)
        && event.data == 0));
    assert!(events[1].is_connect_and(|event| event.to == host2
        && event.from == host1
        && event.peer == enet::PeerID(0)
        && event.data == 5));

    network.send(
        host1,
        host2,
        0,
        &enet::Packet::reliable("hello world".as_bytes()),
    );
    let events = network.update(1);
    assert_eq!(events.len(), 1);
    assert!(events[0].is_receive_and(|event| event.from == host1
        && event.to == host2
        && event.channel_id == 0
        && event.packet.data().len() == 11
        && event.packet.kind() == enet::PacketKind::Reliable));

    network.disconnect(host1, host2, 10);
    let events = network.update(1);
    assert_eq!(events.len(), 1);
    assert!(events[0]
        .is_disconnect_and(|event| event.from == host1 && event.to == host2 && event.data == 10));
    let events = network.update(1);
    assert_eq!(events.len(), 1);
    assert!(events[0]
        .is_disconnect_and(|event| event.from == host2 && event.to == host1 && event.data == 0));
}

#[test]
fn resend_reliable_packet() {
    let mut network = Network::new();
    let host1 = network.create_host(enet::HostSettings {
        peer_limit: 1,
        ..Default::default()
    });
    let host2 = network.create_host(enet::HostSettings {
        peer_limit: 1,
        ..Default::default()
    });

    network.connect(host1, host2, 255, 5);
    network.update(2);

    network.send(
        host1,
        host2,
        0,
        &enet::Packet::reliable("reliable".as_bytes()),
    );
    network.send(
        host1,
        host2,
        0,
        &enet::Packet::unreliable("unreliable".as_bytes()),
    );
    let events = network.update(1);
    assert_eq!(events.len(), 2);
    assert!(events[0].is_receive_and(|event| event.from == host1
        && event.to == host2
        && event.channel_id == 0
        && event.packet.data() == "reliable".as_bytes()));
    assert!(events[1].is_receive_and(|event| event.from == host1
        && event.to == host2
        && event.channel_id == 0
        && event.packet.data() == "unreliable".as_bytes()));

    network.conditions(host1, host2, NetworkConditions::disconnected());
    network.send(
        host1,
        host2,
        0,
        &enet::Packet::reliable("reliable1".as_bytes()),
    );
    network.send(
        host1,
        host2,
        0,
        &enet::Packet::reliable("reliable2".as_bytes()),
    );
    network.send(
        host1,
        host2,
        0,
        &enet::Packet::unreliable("unreliable".as_bytes()),
    );
    let events = network.update(1);
    assert_eq!(events.len(), 0);
    assert!(events.is_empty());
    network.conditions(host1, host2, NetworkConditions::perfect());
    let events = network.update(enet::consts::PEER_DEFAULT_ROUND_TRIP_TIME as usize - 1);
    assert_eq!(events.len(), 0);

    let events = network.update(1);
    assert_eq!(events.len(), 2);
    assert!(events[0].is_receive_and(|event| event.from == host1
        && event.to == host2
        && event.channel_id == 0
        && event.packet.data() == "reliable1".as_bytes()));
    assert!(events[1].is_receive_and(|event| event.from == host1
        && event.to == host2
        && event.channel_id == 0
        && event.packet.data() == "reliable2".as_bytes()));
    let events = network.update(10000);
    assert_eq!(events.len(), 0);
}

#[test]
fn round_trip_time() {
    let mut network = Network::new();
    let host1 = network.create_host(enet::HostSettings {
        peer_limit: 1,
        ..Default::default()
    });
    let host2 = network.create_host(enet::HostSettings {
        peer_limit: 1,
        ..Default::default()
    });

    network.connect(host1, host2, 255, 5);
    network.update(2);

    network.conditions(host1, host2, NetworkConditions::perfect());
    network.update(10000);
    assert_eq!(network.round_trip_time(host1, host2).as_millis(), 1);

    network.conditions(host1, host2, NetworkConditions::good());
    network.update(10000);
    assert_eq!(network.round_trip_time(host1, host2).as_millis(), 93);

    network.conditions(host1, host2, NetworkConditions::bad());
    network.update(10000);
    assert_eq!(network.round_trip_time(host1, host2).as_millis(), 302);
}

#[test]
fn timeout() {
    let mut network = Network::new();
    let host1 = network.create_host(enet::HostSettings {
        peer_limit: 1,
        ..Default::default()
    });
    let host2 = network.create_host(enet::HostSettings {
        peer_limit: 1,
        ..Default::default()
    });

    network.connect(host1, host2, 255, 5);
    network.update(2);

    network.conditions(host1, host2, NetworkConditions::disconnected());
    let events = network.update(5614);
    assert_eq!(events.len(), 0);
    let events = network.update(1);
    assert_eq!(events.len(), 1);
    assert!(events[0].is_disconnect_and(|event| event.from == 0));
    let events = network.update(26383);
    assert_eq!(events.len(), 0);
    let events = network.update(1);
    assert_eq!(events.len(), 1);
    assert!(events[0].is_disconnect_and(|event| event.from == 1));
}

#[test]
fn packet_kind_preserves_all_construction_kinds() {
    for kind in [
        enet::PacketKind::Reliable,
        enet::PacketKind::Unreliable { sequenced: true },
        enet::PacketKind::Unreliable { sequenced: false },
        enet::PacketKind::AlwaysUnreliable { sequenced: true },
        enet::PacketKind::AlwaysUnreliable { sequenced: false },
    ] {
        assert_eq!(enet::Packet::new(vec![1], kind).kind(), kind);
    }
}

#[test]
fn per_packet_unthrottled_preserves_control_lane() {
    let mut network = Network::new();
    let a = network.create_host(enet::HostSettings {
        peer_limit: 1,
        channel_limit: 3,
        ..Default::default()
    });
    let b = network.create_host(enet::HostSettings {
        peer_limit: 1,
        channel_limit: 3,
        ..Default::default()
    });
    network.connect(a, b, 3, 0);
    network.update(3);
    let kind = enet::PacketKind::Unreliable { sequenced: true };
    network.force_throttle(a, b, 0);
    for id in 0..8u8 {
        network.send(a, b, 1, &enet::Packet::new(vec![0, id], kind));
        let bypass = enet::Packet::new_unthrottled(vec![1, id], kind);
        assert!(bypass.is_unthrottled());
        assert_eq!(bypass.kind(), kind);
        network.send(a, b, 1, &bypass);
    }
    let events = network.update(3);
    let received: std::vec::Vec<_> = events.iter().filter(|e| e.is_receive()).collect();
    assert_eq!(
        received.len(),
        8,
        "only eight bypass packets survive zero throttle"
    );
    for (id, event) in received.iter().enumerate() {
        assert!(event.is_receive_and(|e| e.packet.data() == [1, id as u8]
            && e.channel_id == 1
            && e.packet.kind() == kind));
    }
    network.force_throttle(a, b, enet::consts::PEER_PACKET_THROTTLE_SCALE);
    for id in 0..8u8 {
        network.send(a, b, 1, &enet::Packet::new(vec![2, id], kind));
    }
    assert_eq!(
        network.update(3).iter().filter(|e| e.is_receive()).count(),
        8
    );
    network.force_throttle(a, b, 0);
    // Ordinary oversized unreliable packets still take ENet's reliable fallback.
    network.send(a, b, 1, &enet::Packet::new(vec![3; 4096], kind));
    network.send(a, b, 1, &enet::Packet::new_unthrottled(vec![4; 4096], kind));
    network.send(a, b, 0, &enet::Packet::reliable(vec![5]));
    let events = network.update(50);
    assert_eq!(events.iter().filter(|e| e.is_receive()).count(), 3);
    assert!(events.iter().any(|e| e.is_receive_and(
        |e| e.packet.data() == [3; 4096] && e.packet.kind() == enet::PacketKind::Reliable
    )));
    assert!(events.iter().any(|e| e.is_receive_and(
        |e| e.packet.data() == [4; 4096] && e.packet.kind() == enet::PacketKind::Reliable
    )));
    let kind = enet::PacketKind::AlwaysUnreliable { sequenced: true };
    network.send(a, b, 1, &enet::Packet::new_unthrottled(vec![6; 4096], kind));
    assert!(network
        .update(5)
        .iter()
        .any(|e| e.is_receive_and(|e| e.packet.data() == [6; 4096] && e.packet.kind() == kind)));
    network.disconnect_later(a, b, 16);
    assert!(network
        .update(10)
        .iter()
        .any(|e| e.is_disconnect_and(|e| e.data == 16)));
}

#[test]
fn throttle_drop_does_not_consume_next_packet_on_other_channel() {
    for sequenced in [false, true] {
        let mut network = Network::new();
        let a = network.create_host(enet::HostSettings {
            peer_limit: 1,
            channel_limit: 3,
            ..Default::default()
        });
        let b = network.create_host(enet::HostSettings {
            peer_limit: 1,
            channel_limit: 3,
            ..Default::default()
        });
        network.connect(a, b, 3, 0);
        network.update(3);
        network.force_throttle(a, b, 0);
        let kind = enet::PacketKind::Unreliable { sequenced };
        network.send(a, b, 0, &enet::Packet::new(vec![0], kind));
        network.send(
            a,
            b,
            if sequenced { 1 } else { 0 },
            &enet::Packet::new_unthrottled(vec![1], kind),
        );
        let events = network.update(3);
        assert_eq!(
            events.iter().filter(|e| e.is_receive()).count(),
            1,
            "bypass survives preceding drop, sequenced={sequenced}"
        );
        assert!(events.iter().any(|e| e.is_receive_and(|e| e.channel_id
            == if sequenced { 1 } else { 0 }
            && e.packet.data() == [1])));
    }
}

#[test]
#[ignore = "requires pinned native C# probe; set PULSE_NATIVE_PROBE and PULSE_NATIVE_TRACE"]
fn native_csharp_all_lanes_and_controlled_sender_throttle() {
    let Some(probe) = std::env::var_os("PULSE_NATIVE_PROBE") else {
        panic!("set PULSE_NATIVE_PROBE to built NativeProbe.dll");
    };
    let trace = std::env::var("PULSE_NATIVE_TRACE").expect("set PULSE_NATIVE_TRACE");
    let socket = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
    let port = socket.local_addr().unwrap().port();
    let mut host = enet::Host::new(
        socket,
        enet::HostSettings {
            peer_limit: 1,
            channel_limit: 3,
            ..Default::default()
        },
    )
    .unwrap();
    let mut child = std::process::Command::new("dotnet")
        .arg(probe)
        .arg(std::format!("{port}"))
        .arg(&trace)
        .spawn()
        .unwrap();
    let started = std::time::Instant::now();
    let mut received = [false; 3];
    let mut sent = false;
    while started.elapsed() < std::time::Duration::from_secs(15) {
        if let Some(event) = host.service().unwrap() {
            match event {
                enet::Event::Receive {
                    channel_id, packet, ..
                } => {
                    assert_eq!(packet.data()[0], 10 + channel_id);
                    assert_eq!(packet.data().len(), if channel_id == 0 { 4096 } else { 32 });
                    received[channel_id as usize] = true;
                    std::println!(
                        "native->rust channel={channel_id} length={} kind={:?}",
                        packet.data().len(),
                        packet.kind()
                    );
                }
                _ => {}
            }
        }
        if !sent && received.iter().all(|v| *v) {
            let peer = host.peer_mut(enet::PeerID(0));
            unsafe {
                (*peer.0).packet_throttle = 0;
                (*peer.0).packet_throttle_limit = 0;
                (*peer.0).packet_throttle_counter = 0;
            }
            peer.send(0, &enet::Packet::unreliable(vec![99])).unwrap();
            for id in 100..108u8 {
                peer.send(
                    1,
                    &enet::Packet::new_unthrottled(
                        vec![id],
                        enet::PacketKind::Unreliable { sequenced: true },
                    ),
                )
                .unwrap();
            }
            host.flush();
            let peer = host.peer_mut(enet::PeerID(0));
            unsafe {
                (*peer.0).packet_throttle = 32;
                (*peer.0).packet_throttle_limit = 32;
            }
            for channel in 0..3 {
                let kind = match channel {
                    0 => enet::PacketKind::Reliable,
                    1 => enet::PacketKind::Unreliable { sequenced: true },
                    _ => enet::PacketKind::Unreliable { sequenced: false },
                };
                peer.send(
                    channel,
                    &enet::Packet::new(
                        vec![10 + channel; if channel == 0 { 4096 } else { 32 }],
                        kind,
                    ),
                )
                .unwrap();
                peer.send(channel, &enet::Packet::new(vec![110 + channel], kind))
                    .unwrap();
            }
            peer.disconnect_later(16);
            sent = true;
        }
        if let Some(status) = child.try_wait().unwrap() {
            std::println!("native trace: {}", std::fs::read_to_string(&trace).unwrap());
            assert!(received.iter().all(|v| *v));
            assert!(status.success(), "native probe failed {status}");
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    child.kill().unwrap();
    panic!("native probe timeout");
}

#[test]
fn ordinary_unreliable_fragment_group_drops_without_consuming_bypass_group() {
    let mut network = Network::new();
    let a = network.create_host(enet::HostSettings {
        peer_limit: 1,
        channel_limit: 3,
        ..Default::default()
    });
    let b = network.create_host(enet::HostSettings {
        peer_limit: 1,
        channel_limit: 3,
        ..Default::default()
    });
    network.connect(a, b, 3, 0);
    network.update(3);
    network.force_throttle(a, b, 0);
    let kind = enet::PacketKind::AlwaysUnreliable { sequenced: true };
    network.send(a, b, 0, &enet::Packet::new(vec![0; 4096], kind));
    network.send(a, b, 1, &enet::Packet::new_unthrottled(vec![1; 4096], kind));
    let events = network.update(20);
    assert_eq!(events.iter().filter(|e| e.is_receive()).count(), 1);
    assert!(events
        .iter()
        .any(|e| e.is_receive_and(|e| e.packet.data() == [1; 4096] && e.channel_id == 1)));
}

#[test]
fn repeated_packet_sends_keep_independent_throttle_decisions() {
    let mut network = Network::new();
    let a = network.create_host(enet::HostSettings {
        peer_limit: 1,
        channel_limit: 3,
        ..Default::default()
    });
    let b = network.create_host(enet::HostSettings {
        peer_limit: 1,
        channel_limit: 3,
        ..Default::default()
    });
    network.connect(a, b, 3, 0);
    network.update(3);
    network.force_throttle(a, b, 0);
    let packet = enet::Packet::unreliable(vec![9]);
    for _ in 0..32 {
        network.send(a, b, 1, &packet.clone());
    }
    let events = network.update(5);
    assert_eq!(
        events.iter().filter(|e| e.is_receive()).count(),
        1,
        "counter wrap preserves one ordinary repeated send"
    );
}

mod receive_policy {
    use crate::*;
    use std::net::UdpSocket;

    // A connected transport slot with no application admission: native allocation starts here.
    fn rig() -> (Host<UdpSocket>, UdpSocket) {
        rig_with_policy(Some(ReceivePolicy {
            maximum_packet_size: 8192,
            maximum_queued_bytes: 65536,
            maximum_queued_commands: 256,
        }))
    }

    fn rig_with_policy(policy: Option<ReceivePolicy>) -> (Host<UdpSocket>, UdpSocket) {
        let sender = UdpSocket::bind("127.0.0.1:0").unwrap();
        let mut host = Host::new(
            UdpSocket::bind("127.0.0.1:0").unwrap(),
            HostSettings {
                peer_limit: 2,
                channel_limit: 3,
                receive_policy: policy,
                ..Default::default()
            },
        )
        .unwrap();
        let peer = host.connect(sender.local_addr().unwrap(), 3, 0).unwrap();
        unsafe {
            (*peer.0).state = ENET_PEER_STATE_CONNECTED;
            (*peer.0).outgoing_peer_id = 0;
            (*peer.0).incoming_session_id = 0;
        }
        (host, sender)
    }

    fn fragment(
        total: u32,
        count: u32,
        number: u32,
        offset: u32,
        reliable: bool,
        channel: u8,
        sequence: u16,
    ) -> Vec<u8> {
        let mut bytes = vec![0x40, 0, 0, 1, if reliable { 0x88 } else { 12 }, channel];
        bytes.extend_from_slice(&sequence.to_be_bytes());
        bytes.extend_from_slice(&sequence.to_be_bytes());
        bytes.extend_from_slice(&1u16.to_be_bytes());
        bytes.extend_from_slice(&count.to_be_bytes());
        bytes.extend_from_slice(&number.to_be_bytes());
        bytes.extend_from_slice(&total.to_be_bytes());
        bytes.extend_from_slice(&offset.to_be_bytes());
        bytes.push(42);
        bytes
    }

    #[test]
    fn packet_payload_initialization_preserves_copy_and_borrowed_payloads() {
        enet_test_with_allocation_fill(0xa5, || unsafe {
            let packet = enet_packet_create(core::ptr::null(), 7, 0);
            assert_eq!(core::slice::from_raw_parts((*packet).data, 7), &[0; 7]);
            enet_packet_destroy(packet);

            let input = [11, 22, 33];
            let packet = enet_packet_create(input.as_ptr(), input.len(), 0);
            assert_eq!(core::slice::from_raw_parts((*packet).data, 3), &input);
            assert_ne!((*packet).data, input.as_ptr().cast_mut());
            enet_packet_destroy(packet);

            let mut borrowed = [41, 42];
            let packet = enet_packet_create(
                borrowed.as_mut_ptr(),
                borrowed.len(),
                ENET_PACKET_FLAG_NO_ALLOCATE,
            );
            assert_eq!((*packet).data, borrowed.as_mut_ptr());
            assert_eq!(core::slice::from_raw_parts((*packet).data, 2), &[41, 42]);
            enet_packet_destroy(packet);
            assert_eq!(borrowed, [41, 42]);

            let packet = enet_packet_create(core::ptr::null(), 0, 0);
            assert!((*packet).data.is_null());
            enet_packet_destroy(packet);
        });
    }

    #[test]
    fn reliable_fragment_payload_initialization_preserves_ranges_order_and_duplicates() {
        check_fragment_cases(true);
    }

    #[test]
    fn unreliable_fragment_payload_initialization_preserves_ranges_order_and_duplicates() {
        check_fragment_cases(false);
        // The unreliable handler also admits an empty fragment range.
        for limited in [false, true] {
            check_fragment_payload(
                limited,
                false,
                3,
                2,
                &[(0, 0, &[]), (1, 2, &[22])],
                &[0, 0, 22],
            );
        }
    }

    fn check_fragment_cases(reliable_fragments: bool) {
        // Literal expected bytes describe existing arrival/duplicate behavior.
        // Allocation fill keeps every pre-fix read initialized, without relying on heap reuse.
        let cases: &[(u32, &[(u32, u32, &[u8])], &[u8])] = &[
            (2, &[(0, 0, &[11]), (1, 0, &[22])], &[22, 0]),
            (5, &[(0, 2, &[11]), (1, 3, &[22])], &[0, 0, 11, 22, 0]),
            (4, &[(0, 0, &[11]), (1, 3, &[22])], &[11, 0, 0, 22]),
            (
                6,
                &[
                    (2, 4, &[44, 55]),
                    (0, 0, &[11]),
                    (0, 0, &[99]),
                    (1, 1, &[22, 33, 34]),
                ],
                &[11, 22, 33, 34, 44, 55],
            ),
        ];
        for limited in [false, true] {
            for &(total, fragments, expected) in cases {
                let count = if total == 6 { 3 } else { 2 };
                check_fragment_payload(
                    limited,
                    reliable_fragments,
                    total,
                    count,
                    fragments,
                    expected,
                );
            }
        }
    }

    fn check_fragment_payload(
        limited: bool,
        reliable_fragments: bool,
        total: u32,
        count: u32,
        fragments: &[(u32, u32, &[u8])],
        expected: &[u8],
    ) {
        enet_test_with_allocation_fill(0xa5, || {
            let policy = limited.then_some(ReceivePolicy {
                maximum_packet_size: 8192,
                maximum_queued_bytes: 65536,
                maximum_queued_commands: 256,
            });
            let (mut host, sender) = rig_with_policy(policy);
            for &(number, offset, payload) in fragments {
                let sequence = if reliable_fragments {
                    number as u16 + 1
                } else {
                    0
                };
                let mut bytes = fragment(
                    total,
                    count,
                    number,
                    offset,
                    reliable_fragments,
                    0,
                    sequence,
                );
                bytes[8..10].copy_from_slice(&1u16.to_be_bytes());
                bytes[10..12].copy_from_slice(&(payload.len() as u16).to_be_bytes());
                bytes.truncate(28);
                bytes.extend_from_slice(payload);
                assert!(inject(&mut host, &sender, bytes).is_none());
            }
            match host.check_events() {
                Some(Event::Receive {
                    packet,
                    channel_id: 0,
                    ..
                }) => {
                    assert_eq!(
                        packet.data(),
                        expected,
                        "limited={limited}, reliable={reliable_fragments}"
                    );
                }
                _ => panic!("completed fragment group must produce public Receive"),
            }
            assert!(host.check_events().is_none());
            assert_eq!(counters(&host, 0), (0, 0));
            // Fully consumed groups must not block the following ordinary reliable packet.
            let next = if reliable_fragments {
                count as u16 + 1
            } else {
                1
            };
            assert!(inject(&mut host, &sender, reliable(next, 0, &[77], 0x86)).is_none());
            assert!(matches!(host.check_events(),
                Some(Event::Receive { packet, .. }) if packet.data() == [77]));
            assert_eq!(counters(&host, 0), (0, 0));
        });
    }

    fn pending_aggregate_limit(host: &mut Host<UdpSocket>, sender: &UdpSocket, bytes: bool) {
        unsafe {
            let policy = (*raw(host)).receive_policy.as_mut().unwrap();
            if bytes {
                policy.maximum_queued_bytes = core::mem::size_of::<ENetIncomingCommand>()
                    + core::mem::size_of::<ENetPacket>()
                    + 1;
            } else {
                policy.maximum_queued_commands = 1;
            }
        }
        // Leave sequence 1 missing, so accepted data cannot supply another dispatch entry.
        let mut datagram = reliable(2, 0, &[11], 0x86);
        datagram.extend_from_slice(&reliable(3, 0, &[22], 0x86)[4..]);
        assert!(inject(host, sender, datagram).is_none());
    }

    fn aggregate_then_disconnect(bytes: bool) {
        let (mut host, sender) = rig();
        pending_aggregate_limit(&mut host, &sender, bytes);
        // A rejected command ends its datagram. Receive a separate DISCONNECT
        // before dispatch, as the native service's receive pass can do.
        let mut datagram = vec![0x40, 0, 0, 1, 0x44, 0xff, 0, 0];
        datagram.extend_from_slice(&77u32.to_be_bytes());
        assert!(inject(&mut host, &sender, datagram).is_none());
        assert_eq!(host.peer(PeerID(0)).state(), PeerState::Zombie);
        assert_eq!(counters(&host, 0), (0, 0));
        let expected = if bytes {
            ReceiveLimitKind::QueuedBytes
        } else {
            ReceiveLimitKind::QueuedCommands
        };
        assert!(matches!(host.check_events(),
            Some(Event::ReceiveLimitExceeded { kind, .. }) if kind == expected));
        assert!(
            matches!(
                host.check_events(),
                Some(Event::Disconnect { data: 77, .. })
            ),
            "aggregate report consumed native Disconnect"
        );
        assert!(host.check_events().is_none());
        assert_eq!(counters(&host, 0), (0, 0));
        assert_eq!(host.peer(PeerID(0)).state(), PeerState::Disconnected);
        let replacement = UdpSocket::bind("127.0.0.1:0").unwrap();
        assert_eq!(
            host.connect(replacement.local_addr().unwrap(), 3, 0)
                .unwrap()
                .id(),
            PeerID(0)
        );
    }

    #[test]
    fn aggregate_limit_commands_preserve_same_pass_disconnect_and_slot_reuse() {
        aggregate_then_disconnect(false);
    }

    #[test]
    fn aggregate_limit_bytes_preserve_same_pass_disconnect_and_slot_reuse() {
        aggregate_then_disconnect(true);
    }

    #[test]
    fn aggregate_limit_preserves_pending_connect_lifecycle_dispatch() {
        for state in [
            ENET_PEER_STATE_CONNECTION_PENDING,
            ENET_PEER_STATE_CONNECTION_SUCCEEDED,
        ] {
            let (mut host, sender) = rig();
            pending_aggregate_limit(&mut host, &sender, false);
            // Stage each existing connect lifecycle state on the already queued node.
            // Receive handlers normally require Connected; this fixture isolates dispatch ownership.
            unsafe {
                (*host.peer_mut(PeerID(0)).0).state = state;
                (*host.peer_mut(PeerID(0)).0).event_data = 91;
            }
            assert!(matches!(
                host.check_events(),
                Some(Event::ReceiveLimitExceeded {
                    kind: ReceiveLimitKind::QueuedCommands,
                    ..
                })
            ));
            assert!(
                matches!(host.check_events(), Some(Event::Connect { data: 91, .. })),
                "aggregate report consumed pending Connect state {state}"
            );
            assert_eq!(host.peer(PeerID(0)).state(), PeerState::Connected);
            assert!(host.check_events().is_none());
            host.peer_mut(PeerID(0)).reset();
            assert_eq!(counters(&host, 0), (0, 0));
        }
    }

    #[test]
    fn aggregate_limit_preserves_timeout_disconnect_and_slot_reuse() {
        let (mut host, sender) = rig();
        host.flush();
        host.peer_mut(PeerID(0)).set_timeout(1, 1, 1);
        unsafe {
            let h = raw(&host);
            *(*h).time.assume_init_mut() = Box::new(|| core::time::Duration::from_millis(1000));
            let peer = host.peer_mut(PeerID(0)).0;
            (*peer).next_timeout = 1;
            (*peer).earliest_timeout = 1;
            let sent = (*peer)
                .sent_reliable_commands
                .sentinel
                .next
                .cast::<ENetOutgoingCommand>();
            assert_ne!(
                sent.cast::<ENetListNode>(),
                core::ptr::addr_of_mut!((*peer).sent_reliable_commands.sentinel)
            );
            (*sent).sent_time = 1;
            (*sent).round_trip_timeout = 1;
            (*sent).send_attempts = 1;
        }
        pending_aggregate_limit(&mut host, &sender, false);
        assert!(matches!(
            host.service().unwrap(),
            Some(Event::ReceiveLimitExceeded {
                kind: ReceiveLimitKind::QueuedCommands,
                ..
            })
        ));
        assert!(matches!(
            host.service().unwrap(),
            Some(Event::Disconnect { .. })
        ));
        assert_eq!(counters(&host, 0), (0, 0));
        let replacement = UdpSocket::bind("127.0.0.1:0").unwrap();
        assert_eq!(
            host.connect(replacement.local_addr().unwrap(), 3, 0)
                .unwrap()
                .id(),
            PeerID(0)
        );
    }

    #[test]
    fn native_receive_valid_declaration_above_old_max_reports_before_allocation() {
        let (mut host, sender) = rig();
        enet_test_allocation_peak(true);
        sender
            .send_to(
                &fragment(64 * 1024 * 1024, 2, 0, 0, true, 0, 1),
                host.socket().local_addr().unwrap(),
            )
            .unwrap();
        // Loopback delivery is asynchronous on macOS: the datagram is not always readable on
        // the first non-blocking poll after send_to, so poll until the host reports it.
        let started = std::time::Instant::now();
        let mut event = None;
        while event.is_none() && started.elapsed() < core::time::Duration::from_secs(5) {
            event = host.service().unwrap().map(Event::no_ref);
            if event.is_none() {
                std::thread::sleep(core::time::Duration::from_millis(1));
            }
        }
        assert_eq!(
            enet_test_allocation_peak(false),
            0,
            "oversized declaration must not allocate advertised buffers or descriptors"
        );
        assert!(
            matches!(
                event,
                Some(EventNoRef::ReceiveLimitExceeded {
                    peer: PeerID(0),
                    channel_id: 0,
                    kind: ReceiveLimitKind::PacketSize
                })
            ),
            "valid oversized first fragment must produce a local event before allocation"
        );
        unsafe {
            assert_eq!((*host.peer(PeerID(0)).0).queued_receive_bytes, 0);
        }
    }

    fn raw(host: &Host<UdpSocket>) -> *mut ENetHost<UdpSocket> {
        unsafe { (*host.peer(PeerID(0)).0).host }
    }

    fn inject(
        host: &mut Host<UdpSocket>,
        sender: &UdpSocket,
        mut bytes: Vec<u8>,
    ) -> Option<EventNoRef> {
        unsafe {
            let h = raw(host);
            (*h).received_address
                .write(Some(sender.local_addr().unwrap()));
            (*h).received_data = bytes.as_mut_ptr();
            (*h).received_data_length = bytes.len();
            let mut event: ENetEvent<UdpSocket> = core::mem::zeroed();
            if enet_protocol_handle_incoming_commands(h, &mut event) {
                assert_eq!(event.type_0, ENET_EVENT_TYPE_RECEIVE_LIMIT_EXCEEDED);
                return Some(EventNoRef::ReceiveLimitExceeded {
                    peer: PeerID(usize::from((*event.peer).incoming_peer_id)),
                    channel_id: event.channel_id,
                    kind: match event.data {
                        1 => ReceiveLimitKind::PacketSize,
                        2 => ReceiveLimitKind::QueuedBytes,
                        3 => ReceiveLimitKind::QueuedCommands,
                        _ => panic!("invalid limit kind"),
                    },
                });
            }
            None
        }
    }

    fn counters(host: &Host<UdpSocket>, peer: usize) -> (usize, usize) {
        unsafe {
            let peer = host.peer(PeerID(peer)).0;
            assert!((*peer).queued_receive_bytes <= 65536);
            assert!((*peer).queued_receive_commands <= 256);
            (
                (*peer).queued_receive_bytes,
                (*peer).queued_receive_commands,
            )
        }
    }

    unsafe fn queue(
        host: &mut Host<UdpSocket>,
        peer: usize,
        channel: u8,
        sequence: u16,
        unreliable: Option<u16>,
        length: usize,
        fragments: u32,
    ) -> *mut ENetIncomingCommand {
        let mut command: ENetProtocol = core::mem::zeroed();
        command.header.command = if unreliable.is_some() { 7 } else { 6 };
        command.header.channel_id = channel;
        command.header.reliable_sequence_number = sequence;
        if let Some(n) = unreliable {
            command.send_unreliable.unreliable_sequence_number = n.to_be();
        }
        enet_peer_queue_incoming_command(
            host.peer_mut(PeerID(peer)).0,
            &command,
            core::ptr::null(),
            length,
            0,
            fragments,
        )
    }

    fn reliable(sequence: u16, channel: u8, data: &[u8], kind: u8) -> Vec<u8> {
        let mut bytes = vec![0x40, 0, 0, 1, kind, channel];
        bytes.extend_from_slice(&sequence.to_be_bytes());
        if kind == 7 || kind == 9 {
            bytes.extend_from_slice(&sequence.to_be_bytes());
        }
        bytes.extend_from_slice(&(data.len() as u16).to_be_bytes());
        bytes.extend_from_slice(data);
        bytes
    }

    #[test]
    fn native_receive_charge_exact_boundary_and_transfer_release() {
        let (mut host, _) = rig();
        unsafe {
            let h = raw(&host);
            let descriptor =
                core::mem::size_of::<ENetIncomingCommand>() + core::mem::size_of::<ENetPacket>();
            (*h).receive_policy.as_mut().unwrap().maximum_queued_bytes = 2 * (descriptor + 7);
            assert!(!queue(&mut host, 0, 0, 1, None, 7, 0).is_null());
            assert!(!queue(&mut host, 0, 0, 2, None, 7, 0).is_null());
            assert_eq!(counters(&host, 0), (2 * (descriptor + 7), 2));
            assert!(queue(&mut host, 0, 0, 3, None, 0, 0).is_null());
            assert_eq!(counters(&host, 0), (2 * (descriptor + 7), 2));
            assert!(matches!(
                host.check_events(),
                Some(Event::ReceiveLimitExceeded {
                    kind: ReceiveLimitKind::QueuedBytes,
                    ..
                })
            ));
            assert!(matches!(host.check_events(), Some(Event::Receive { .. })));
            assert_eq!(counters(&host, 0), (descriptor + 7, 1));
            assert!(matches!(host.check_events(), Some(Event::Receive { .. })));
            assert_eq!(counters(&host, 0), (0, 0));
            assert!(!queue(&mut host, 0, 0, 3, None, 7, 0).is_null());
        }
    }

    #[test]
    fn native_receive_zero_payload_command_cap_duplicates_and_independent_peers() {
        let (mut host, sender) = rig();
        let other = UdpSocket::bind("127.0.0.1:0").unwrap();
        let second = host.connect(other.local_addr().unwrap(), 3, 0).unwrap();
        unsafe {
            (*second.0).state = ENET_PEER_STATE_CONNECTED;
        }
        unsafe {
            for n in 2..=257 {
                assert!(!queue(&mut host, 0, 0, n, None, 0, 0).is_null());
                counters(&host, 0);
            }
            let before = counters(&host, 0);
            assert_eq!(before.1, 256);
            assert!(!queue(&mut host, 0, 0, 2, None, 0, 0).is_null());
            assert_eq!(counters(&host, 0), before);
            assert!(queue(&mut host, 0, 0, 258, None, 0, 0).is_null());
            assert!(matches!(
                host.check_events(),
                Some(Event::ReceiveLimitExceeded {
                    kind: ReceiveLimitKind::QueuedCommands,
                    ..
                })
            ));
            assert!(!queue(&mut host, 1, 0, 1, None, 1, 0).is_null());
            assert_eq!(counters(&host, 1).1, 1);
            enet_peer_reset(host.peer_mut(PeerID(0)).0);
            assert_eq!(counters(&host, 0), (0, 0));
            assert_eq!(counters(&host, 1).1, 1);
        }
        drop(sender);
    }

    #[test]
    fn native_receive_fragment_bitmap_budget_and_duplicate_reservations() {
        for is_reliable in [true, false] {
            let (mut host, sender) = rig();
            assert!(inject(
                &mut host,
                &sender,
                fragment(8192, 2, 0, 0, is_reliable, 1, 1)
            )
            .is_none());
            let before = counters(&host, 0);
            assert_eq!(before.1, 1);
            assert!(inject(
                &mut host,
                &sender,
                fragment(8192, 2, 0, 0, is_reliable, 1, 1)
            )
            .is_none());
            assert_eq!(counters(&host, 0), before);
            unsafe {
                enet_peer_reset_queues(host.peer_mut(PeerID(0)).0);
            }
            assert_eq!(counters(&host, 0), (0, 0));
        }
        let (mut host, sender) = rig();
        enet_test_allocation_peak(true);
        assert!(inject(
            &mut host,
            &sender,
            fragment(1, 1_048_576, 0, 0, false, 1, 1)
        )
        .is_none());
        assert_eq!(counters(&host, 0), (0, 0));
        assert_eq!(
            enet_test_allocation_peak(false),
            0,
            "bitmap admission must precede allocation"
        );
        assert!(matches!(
            host.check_events(),
            Some(Event::ReceiveLimitExceeded {
                kind: ReceiveLimitKind::QueuedBytes,
                ..
            })
        ));
    }

    #[test]
    fn native_receive_unfragmented_paths_enforce_payload_after_window_validation() {
        for kind in [0x86, 7, 9] {
            let (mut host, sender) = rig();
            unsafe {
                (*raw(&host))
                    .receive_policy
                    .as_mut()
                    .unwrap()
                    .maximum_packet_size = 2;
            }
            assert!(matches!(
                inject(&mut host, &sender, reliable(1, 2, &[1, 2, 3], kind)),
                Some(EventNoRef::ReceiveLimitExceeded {
                    channel_id: 2,
                    kind: ReceiveLimitKind::PacketSize,
                    ..
                })
            ));
            assert_eq!(counters(&host, 0), (0, 0));
        }
    }

    #[test]
    fn native_receive_terminal_wins_over_pending_receive_and_following_input() {
        for is_reliable in [true, false] {
            for total in [8193, 64 * 1024 * 1024] {
                let (mut host, sender) = rig();
                unsafe {
                    assert!(!queue(&mut host, 0, 0, 1, None, 1, 0).is_null());
                }
                let mut bytes = fragment(total, 2, 0, 0, is_reliable, 1, 2);
                bytes.extend_from_slice(&reliable(2, 0, &[9], 0x86)[4..]);
                assert!(matches!(
                    inject(&mut host, &sender, bytes),
                    Some(EventNoRef::ReceiveLimitExceeded {
                        kind: ReceiveLimitKind::PacketSize,
                        ..
                    })
                ));
                assert_eq!(
                    counters(&host, 0).1,
                    1,
                    "no oversized or following command allocated"
                );
                assert!(host.check_events().is_none());
                assert!(inject(&mut host, &sender, reliable(2, 0, &[10], 0x86)).is_none());
                assert_eq!(counters(&host, 0).1, 1);
                host.peer_mut(PeerID(0)).disconnect_now(16);
                assert_eq!(counters(&host, 0), (0, 0));
                assert!(host.check_events().is_none());
                let peer = host.connect(sender.local_addr().unwrap(), 3, 0).unwrap();
                assert_eq!(peer.id(), PeerID(0));
                unsafe {
                    (*peer.0).state = ENET_PEER_STATE_CONNECTED;
                    (*peer.0).outgoing_peer_id = 0;
                    (*peer.0).incoming_session_id = 0;
                }
                assert!(inject(&mut host, &sender, reliable(1, 0, &[11], 0x86)).is_none());
                assert!(
                    matches!(host.check_events(), Some(Event::Receive { packet, .. }) if packet.data()==[11])
                );
                assert_eq!(counters(&host, 0), (0, 0));
            }
        }
    }

    #[test]
    fn native_receive_malformed_out_of_window_and_spoofed_headers_have_no_attribution() {
        for is_reliable in [true, false] {
            for variant in 0..7 {
                let (mut host, sender) = rig();
                let mut bytes = fragment(8193, 2, 0, 0, is_reliable, 0, 1);
                match variant {
                    0 => {
                        bytes.pop();
                    } // truncated payload
                    1 => {
                        bytes[12..16].copy_from_slice(&0u32.to_be_bytes());
                    } // no fragments
                    2 => {
                        bytes[16..20].copy_from_slice(&2u32.to_be_bytes());
                    }
                    3 => {
                        bytes[24..28].copy_from_slice(&8193u32.to_be_bytes());
                    }
                    4 => {
                        bytes[5] = 3;
                    } // invalid channel
                    5 => {
                        bytes[0] |= 0x10;
                    } // wrong session
                    6 => {
                        bytes[8..10].copy_from_slice(&32768u16.to_be_bytes());
                        bytes[6..8].copy_from_slice(&32768u16.to_be_bytes());
                    }
                    _ => unreachable!(),
                }
                assert!(
                    inject(&mut host, &sender, bytes).is_none(),
                    "variant {variant}"
                );
                assert!(host.check_events().is_none(), "variant {variant}");
                assert_eq!(counters(&host, 0), (0, 0));
            }
            let (mut host, _) = rig();
            let spoof = UdpSocket::bind("127.0.0.1:0").unwrap();
            assert!(inject(
                &mut host,
                &spoof,
                fragment(8193, 2, 0, 0, is_reliable, 0, 1)
            )
            .is_none());
            assert!(host.check_events().is_none());
        }
    }

    #[test]
    fn native_receive_stale_unreliable_removal_releases_while_connected() {
        let (mut host, _) = rig();
        unsafe {
            // Partial unreliable group is superseded once reliable channel progress reaches it.
            assert!(!queue(&mut host, 0, 0, 1, Some(1), 13, 2).is_null());
            assert_eq!(counters(&host, 0).1, 1);
            assert!(!queue(&mut host, 0, 0, 1, None, 0, 0).is_null());
            assert!(!queue(&mut host, 0, 0, 2, None, 0, 0).is_null());
            assert_eq!(
                counters(&host, 0).1,
                2,
                "stale group must be released during reliable progress"
            );
            while host.check_events().is_some() {}
            assert_eq!(counters(&host, 0), (0, 0));
            assert_eq!((*host.peer(PeerID(0)).0).state, ENET_PEER_STATE_CONNECTED);
        }
    }

    #[test]
    fn native_receive_checked_charge_overflow_and_recoverable_refusal_leave_no_reservation() {
        assert_eq!(enet_receive_charge(usize::MAX, 0), None);
        assert_eq!(enet_receive_charge(usize::MAX - 2, 32), None);
        let (mut host, _) = rig();
        unsafe {
            assert!(queue(&mut host, 0, 0, 1, None, 0, 1_048_577).is_null());
            assert_eq!(counters(&host, 0), (0, 0));
            // Force a large policy to reach the post-reservation structural refusal path.
            (*raw(&host))
                .receive_policy
                .as_mut()
                .unwrap()
                .maximum_queued_bytes = usize::MAX;
            assert!(queue(&mut host, 0, 0, 1, None, 0, 1_048_577).is_null());
            assert_eq!(counters(&host, 0), (0, 0));
        }
    }

    #[test]
    fn native_receive_reliable_refusal_is_not_acked_and_retry_progresses() {
        let (mut host, sender) = rig();
        unsafe {
            (*raw(&host))
                .receive_policy
                .as_mut()
                .unwrap()
                .maximum_queued_commands = 1;
        }
        inject(&mut host, &sender, fragment(2, 2, 0, 0, true, 0, 1));
        inject(&mut host, &sender, reliable(3, 0, &[9], 0x86));
        unsafe {
            let peer = host.peer(PeerID(0)).0;
            let sentinel = core::ptr::addr_of_mut!((*peer).acknowledgements.sentinel);
            let mut node = (*sentinel).next;
            while node != sentinel {
                let ack = node.cast::<ENetAcknowledgement>();
                let sequence = (*ack).command.header.reliable_sequence_number;
                assert_ne!(sequence, 3, "refused data must not be ACKed");
                node = (*node).next;
            }
        }
        assert!(matches!(
            host.check_events(),
            Some(Event::ReceiveLimitExceeded {
                kind: ReceiveLimitKind::QueuedCommands,
                ..
            })
        ));
        let mut tail = fragment(2, 2, 1, 1, true, 0, 2);
        tail[8..10].copy_from_slice(&1u16.to_be_bytes());
        inject(&mut host, &sender, tail);
        assert!(matches!(host.check_events(), Some(Event::Receive { .. })));
        assert_eq!(counters(&host, 0), (0, 0));
        inject(&mut host, &sender, reliable(3, 0, &[9], 0x86));
        assert!(
            matches!(host.check_events(),Some(Event::Receive { packet,.. }) if packet.data()==[9])
        );
        assert_eq!(counters(&host, 0), (0, 0));
    }

    #[test]
    fn native_receive_default_policy_preserves_large_packets_and_settings_validation() {
        let (mut host, sender) = rig();
        unsafe {
            let h = raw(&host);
            (*h).receive_policy = None;
            assert_eq!((*h).maximum_packet_size, 32 * 1024 * 1024);
            assert_eq!((*h).maximum_waiting_data, 32 * 1024 * 1024);
            assert!(inject(&mut host, &sender, fragment(8193, 2, 0, 0, true, 0, 1)).is_none());
            assert_eq!((*host.peer(PeerID(0)).0).total_waiting_data, 8193);
            assert_eq!(counters(&host, 0), (0, 0));
            host.peer_mut(PeerID(0))
                .send(0, &Packet::reliable(vec![0; 9000]))
                .unwrap();
        }
        assert!(HostSettings::default().receive_policy.is_none());
        for field in 0..3 {
            let mut policy = ReceivePolicy {
                maximum_packet_size: 8192,
                maximum_queued_bytes: 65536,
                maximum_queued_commands: 256,
            };
            match field {
                0 => policy.maximum_packet_size = 0,
                1 => policy.maximum_queued_bytes = 0,
                _ => policy.maximum_queued_commands = 0,
            }
            let result = Host::new(
                UdpSocket::bind("127.0.0.1:0").unwrap(),
                HostSettings {
                    receive_policy: Some(policy),
                    ..Default::default()
                },
            );
            assert!(matches!(result, Err(error::HostNewError::BadParameter(_))));
        }
    }

    #[test]
    fn native_receive_sequence_wrap_and_aggregate_payload_precedence() {
        let (mut host, sender) = rig();
        unsafe {
            (*(*host.peer(PeerID(0)).0).channels).incoming_reliable_sequence_number = 65535;
            (*raw(&host))
                .receive_policy
                .as_mut()
                .unwrap()
                .maximum_queued_commands = 1;
            assert!(!queue(&mut host, 0, 0, 0, None, 1, 0).is_null());
            assert!(queue(&mut host, 0, 1, 1, None, 1, 0).is_null());
        }
        // Payload must replace a pending aggregate event, even across channels and sequence wrap.
        assert!(matches!(
            inject(&mut host, &sender, fragment(8193, 2, 0, 0, true, 2, 1)),
            Some(EventNoRef::ReceiveLimitExceeded {
                channel_id: 2,
                kind: ReceiveLimitKind::PacketSize,
                ..
            })
        ));
        assert!(host.check_events().is_none());
        assert_eq!(counters(&host, 0).1, 1);
        host.peer_mut(PeerID(0)).disconnect_now(16);
        assert_eq!(counters(&host, 0), (0, 0));
    }
    struct FreeWitness {
        peer: *mut ENetPeer<UdpSocket>,
        calls: usize,
    }
    unsafe fn witness_release(packet: *mut ENetPacket) {
        let witness = &mut *((*packet).user_data.cast::<FreeWitness>());
        assert_eq!((*witness.peer).queued_receive_bytes, 0);
        assert_eq!((*witness.peer).queued_receive_commands, 0);
        witness.calls += 1;
    }

    #[test]
    fn native_receive_host_destruction_releases_before_packet_free() {
        let (mut host, _sender) = rig();
        let mut witness = FreeWitness {
            peer: host.peer(PeerID(0)).0,
            calls: 0,
        };
        unsafe {
            let command = queue(&mut host, 0, 0, 2, None, 7, 2);
            assert!(!command.is_null());
            (*(*command).packet).user_data = core::ptr::addr_of_mut!(witness).cast();
            (*(*command).packet).free_callback = Some(witness_release);
        }
        assert_eq!(counters(&host, 0).1, 1);
        drop(host);
        assert_eq!(witness.calls, 1);
    }

    #[test]
    fn native_receive_timeout_releases_partial_group_and_pending_reports() {
        let (mut host, _sender) = rig();
        unsafe {
            assert!(!queue(&mut host, 0, 0, 2, None, 7, 2).is_null());
        }
        host.flush();
        host.peer_mut(PeerID(0)).set_timeout(1, 1, 1);
        unsafe {
            let h = raw(&host);
            *(*h).time.assume_init_mut() = Box::new(|| core::time::Duration::from_millis(1000));
            let peer = host.peer(PeerID(0)).0;
            (*peer).next_timeout = 1;
            (*peer).earliest_timeout = 1;
            let sent = (*peer)
                .sent_reliable_commands
                .sentinel
                .next
                .cast::<ENetOutgoingCommand>();
            assert_ne!(
                sent.cast::<ENetListNode>(),
                core::ptr::addr_of_mut!((*peer).sent_reliable_commands.sentinel)
            );
            (*sent).sent_time = 1;
            (*sent).round_trip_timeout = 1;
            (*sent).send_attempts = 1;
        }
        assert!(matches!(
            host.service().unwrap(),
            Some(Event::Disconnect { .. })
        ));
        assert_eq!(counters(&host, 0), (0, 0));
        assert!(host.check_events().is_none());
    }
    #[test]
    fn native_receive_connected_adapter_forwards_opt_in_violation() {
        use crate::connected::{self, ConnectionID, ConnectionKind};
        type Connection = ReadWrite<(), std::convert::Infallible>;
        fn transfer(a: &mut connected::Host<Connection>, b: &mut connected::Host<Connection>) {
            while let Some(((), bytes)) =
                a.peer_mut(ConnectionID(0)).connection_mut().unwrap().read()
            {
                b.peer_mut(ConnectionID(0))
                    .connection_mut()
                    .unwrap()
                    .write((), bytes);
            }
        }
        let settings = HostSettings {
            peer_limit: 1,
            channel_limit: 3,
            receive_policy: Some(ReceivePolicy {
                maximum_packet_size: 2,
                maximum_queued_bytes: 65536,
                maximum_queued_commands: 256,
            }),
            ..Default::default()
        };
        let mut server = connected::Host::new(settings).unwrap();
        let mut client = connected::Host::new(HostSettings {
            peer_limit: 1,
            channel_limit: 3,
            ..Default::default()
        })
        .unwrap();
        server
            .add_connection(ConnectionKind::Receiver {
                connection: Connection::new(),
                timeout: core::time::Duration::from_secs(3),
            })
            .unwrap();
        client
            .add_connection(ConnectionKind::Initiator {
                connection: Connection::new(),
                channel_count: 3,
                data: 0,
            })
            .unwrap();
        let (mut a, mut b) = (false, false);
        for _ in 0..32 {
            a |= matches!(client.service(), Some(connected::Event::Connect { .. }));
            transfer(&mut client, &mut server);
            b |= matches!(server.service(), Some(connected::Event::Connect { .. }));
            transfer(&mut server, &mut client);
            if a && b {
                break;
            }
        }
        assert!(a && b, "connected adapter handshake missing");
        client
            .peer_mut(ConnectionID(0))
            .send(2, &Packet::reliable(vec![1, 2, 3]))
            .unwrap();
        for _ in 0..32 {
            let _ = client.service();
            transfer(&mut client, &mut server);
            if let Some(event) = server.service() {
                assert!(matches!(
                    event.no_ref(),
                    connected::EventNoRef::ReceiveLimitExceeded {
                        peer: ConnectionID(0),
                        channel_id: 2,
                        kind: ReceiveLimitKind::PacketSize
                    }
                ));
                return;
            }
            transfer(&mut server, &mut client);
        }
        panic!("connected adapter dropped receive-limit event");
    }
    #[test]
    fn native_receive_consumed_reliable_fragment_is_not_attributed_and_next_sequence_progresses() {
        for consumed in [0u16, 1, u16::MAX] {
            let (mut host, sender) = rig();
            if consumed != 0 {
                unsafe {
                    (*(*host.peer(PeerID(0)).0).channels).incoming_reliable_sequence_number =
                        consumed - 1;
                }
                assert!(inject(&mut host, &sender, reliable(consumed, 0, &[7], 0x86)).is_none());
                assert!(
                    matches!(host.check_events(),Some(Event::Receive { packet,.. }) if packet.data()==[7])
                );
            }
            enet_test_allocation_peak(true);
            let stale = inject(
                &mut host,
                &sender,
                fragment(8193, 1, 0, 0, true, 0, consumed),
            );
            assert!(
                stale.is_none(),
                "consumed sequence {consumed} must not be attributed as oversized"
            );
            assert_eq!(enet_test_allocation_peak(false), 0);
            assert!(host.check_events().is_none());
            assert_eq!(counters(&host, 0), (0, 0));
            unsafe {
                assert!(!(*host.peer(PeerID(0)).0).receive_terminal);
            }
            let next = consumed.wrapping_add(1);
            assert!(inject(&mut host, &sender, reliable(next, 0, &[8], 0x86)).is_none());
            assert!(
                matches!(host.check_events(),Some(Event::Receive { packet,.. }) if packet.data()==[8])
            );
            assert_eq!(counters(&host, 0), (0, 0));
        }
    }
}
