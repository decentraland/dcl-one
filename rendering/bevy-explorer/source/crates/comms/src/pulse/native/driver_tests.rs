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
