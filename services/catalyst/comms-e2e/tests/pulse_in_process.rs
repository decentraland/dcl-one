//! The Pulse bot against a real Pulse server in this process.

mod support;

use std::time::Duration;

use catalyst_comms::pulse::hardening::DisconnectReason;
use catalyst_comms_e2e::engine::expect_or;
use catalyst_comms_e2e::pulse::{
    Channel, ConnectOptions, HandshakeOptions, HandshakeOutcome, PulseBot, messages,
};

const HERE: [f32; 3] = [8.0, 0.0, 8.0];
const NEARBY: [f32; 3] = [10.0, 0.0, 9.0];
const SOON: Duration = Duration::from_secs(5);

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn two_bots_see_each_other() {
    let pulse = support::in_process_pulse().await;
    let ctx = support::ctx(pulse, "see-each-other");
    let realm = ctx.realm("plaza");
    let (alice_id, bob_id) = (ctx.identity(0), ctx.identity(1));

    let alice = PulseBot::join(&ctx, "alice", pulse, &alice_id).await.unwrap();
    alice.teleport(&realm, HERE).unwrap();
    let bob = PulseBot::join(&ctx, "bob", pulse, &bob_id).await.unwrap();
    bob.teleport(&realm, NEARBY).unwrap();

    expect_or(&ctx, "alice sees bob", SOON, || {
        alice.alive()?;
        alice
            .view()
            .of(&bob_id.address())
            .map(|_| ())
            .ok_or_else(|| format!("alice holds {:?}", alice.view().wallets()))
    })
    .await
    .unwrap();
    let seen = expect_or(&ctx, "bob sees alice", SOON, || {
        bob.alive()?;
        bob.view()
            .of(&alice_id.address())
            .cloned()
            .ok_or_else(|| format!("bob holds {:?}", bob.view().wallets()))
    })
    .await
    .unwrap();
    assert_eq!(seen.realm, realm);
    assert!(
        (seen.world[0] - HERE[0]).abs() < 0.1 && (seen.world[2] - HERE[2]).abs() < 0.1,
        "{:?}",
        seen.world
    );
    // nobody is told about themselves
    assert_eq!(alice.view().self_joins, 0);
    assert_eq!(bob.view().self_joins, 0);

    // alice walks; what bob holds of her follows
    for step in 1..=10 {
        alice
            .move_to([HERE[0] + step as f32 * 0.5, 0.0, HERE[2]])
            .unwrap();
        tokio::time::sleep(Duration::from_millis(60)).await;
    }
    expect_or(&ctx, "bob sees alice 5 m further", SOON, || {
        let world = bob
            .view()
            .of(&alice_id.address())
            .map(|subject| subject.world)
            .ok_or("alice is gone")?;
        ((world[0] - 13.0).abs() < 0.2)
            .then_some(())
            .ok_or_else(|| format!("alice is at {world:?}"))
    })
    .await
    .unwrap();

    alice.close();
    expect_or(&ctx, "bob no longer sees alice", SOON, || {
        (bob.view().count_of(&alice_id.address()) == 0)
            .then_some(())
            .ok_or_else(|| "alice is still there".to_owned())
    })
    .await
    .unwrap();
    bob.expect_alive().unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_out_of_grid_input_is_refused_with_code_11() {
    let pulse = support::in_process_pulse().await;
    let ctx = support::ctx(pulse, "out-of-grid");
    let bot = PulseBot::join(&ctx, "walker", pulse, &ctx.identity(0))
        .await
        .unwrap();
    bot.teleport(&ctx.realm("plaza"), HERE).unwrap();
    bot.send(
        messages::input(messages::state_in_parcel(i32::MAX, [1.0, 0.0, 1.0])),
        Channel::UnreliableSequenced,
    );
    bot.expect_disconnect_with(&ctx, DisconnectReason::InvalidInputField, SOON)
        .await
        .unwrap();
    assert_eq!(bot.view().disconnect, Some(11));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_long_emote_is_refused_with_code_12() {
    let pulse = support::in_process_pulse().await;
    let ctx = support::ctx(pulse, "long-emote");
    let bot = PulseBot::join(&ctx, "dancer", pulse, &ctx.identity(0))
        .await
        .unwrap();
    bot.teleport(&ctx.realm("plaza"), HERE).unwrap();
    let state = messages::state_at(bot.grid(), HERE);
    // exactly the limit is fine
    bot.send(
        messages::emote_start("wave", Some(60_000), state.clone(), None),
        Channel::Reliable,
    );
    tokio::time::sleep(Duration::from_millis(300)).await;
    bot.expect_alive().unwrap();
    // one millisecond more is not
    bot.send(
        messages::emote_start("wave", Some(60_001), state, None),
        Channel::Reliable,
    );
    bot.expect_disconnect_with(&ctx, DisconnectReason::InvalidEmoteField, SOON)
        .await
        .unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_unexpected_disconnect_fails_fast() {
    let pulse = support::in_process_pulse().await;
    let ctx = support::ctx(pulse, "fails-fast");
    let bot = PulseBot::join(&ctx, "walker", pulse, &ctx.identity(0))
        .await
        .unwrap();
    bot.send(messages::input_without_state(), Channel::UnreliableSequenced);
    bot.expect_disconnect(&ctx, SOON).await.unwrap();

    let error = bot.expect_alive().unwrap_err().to_string();
    assert!(
        error.contains("walker") && error.contains("11 (InvalidInputField)"),
        "{error}"
    );

    // a wait that depends on a live connection ends at once, naming the code, not at its deadline
    let started = std::time::Instant::now();
    let error = expect_or(
        &ctx,
        "the walker sees a friend",
        Duration::from_secs(60),
        || {
            bot.alive()?;
            Err::<(), _>("nobody yet".to_owned())
        },
    )
    .await
    .unwrap_err()
    .to_string();
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "waited {:?}",
        started.elapsed()
    );
    assert!(error.contains("InvalidInputField"), "{error}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_second_session_of_a_wallet_evicts_the_first() {
    let pulse = support::in_process_pulse().await;
    let ctx = support::ctx(pulse, "evicts");
    let identity = ctx.identity(0);
    let first = PulseBot::join(&ctx, "first", pulse, &identity)
        .await
        .unwrap();
    first.teleport(&ctx.realm("plaza"), HERE).unwrap();
    let second = PulseBot::join(&ctx, "second", pulse, &identity.second_device())
        .await
        .unwrap();
    first
        .expect_disconnect_with(&ctx, DisconnectReason::DuplicateSession, SOON)
        .await
        .unwrap();
    second.expect_alive().unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_replayed_handshake_is_refused() {
    let pulse = support::in_process_pulse().await;
    let ctx = support::ctx(pulse, "replay");
    let identity = ctx.identity(0);
    let timestamp = catalyst_comms_e2e::identity::now_ms();
    let options = HandshakeOptions {
        timestamp_ms: Some(timestamp),
        ..Default::default()
    };
    let first = PulseBot::connect(&ctx, "first", pulse, ConnectOptions::default())
        .await
        .unwrap();
    assert_eq!(
        first
            .handshake(&ctx, &identity, options.clone())
            .await
            .unwrap(),
        HandshakeOutcome::Accepted
    );
    first.close();

    // the same wallet and timestamp, signed again by the same device: a replay
    let replay = PulseBot::connect(&ctx, "replay", pulse, ConnectOptions::default())
        .await
        .unwrap();
    let outcome = replay
        .handshake(&ctx, &identity.same_device(), options)
        .await
        .unwrap();
    assert_ne!(
        outcome,
        HandshakeOutcome::Accepted,
        "a replayed handshake was accepted"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn nothing_listening_is_a_failed_connect_not_a_hang() {
    // a port nothing serves
    let dead = {
        let socket = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
        socket.local_addr().unwrap()
    };
    let ctx = support::ctx(dead, "dead");
    let options = ConnectOptions {
        deadline: Some(Duration::from_millis(1500)),
        ..Default::default()
    };
    let started = std::time::Instant::now();
    let error = PulseBot::connect(&ctx, "lonely", dead, options)
        .await
        .err()
        .expect("no server, no connection");
    assert!(started.elapsed() < Duration::from_secs(10));
    assert!(
        error.to_string().contains("lonely connects to Pulse"),
        "{error}"
    );
}
