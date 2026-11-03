use tokio::sync::mpsc::error::TryRecvError;

use super::*;

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
    let mut backlog = Backlog::with_cap(8);

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
    let mut backlog = Backlog::with_cap(8);

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
fn holding_past_the_cap_is_an_error() {
    let (tx, _rx) = mpsc::channel(1);
    let mut backlog = Backlog::with_cap(1);

    backlog.deliver(&tx, vec![1], true).unwrap();
    backlog.deliver(&tx, vec![2], true).unwrap();

    assert_eq!(backlog.deliver(&tx, vec![3], true), Err(BacklogFull));
}
