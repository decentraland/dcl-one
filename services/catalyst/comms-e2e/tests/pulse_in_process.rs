//! The Pulse bot against a real Pulse server in this process.
#![allow(clippy::unwrap_used, reason = "test support")]
#![allow(
    clippy::cast_precision_loss,
    reason = "test fixtures use small literal values"
)]

mod support;

use std::time::Duration;

use catalyst_comms::pulse::hardening::{DEFAULT_MAX_EMOTE_DURATION_MS, DisconnectReason};
use catalyst_comms_e2e::engine::{Transport, expect_or};
use catalyst_comms_e2e::fault::{Direction, Fault, FaultLink};
use catalyst_comms_e2e::pulse::{
    Channel, ConnectOptions, HandshakeOptions, HandshakeOutcome, PulseBot, messages,
};
use catalyst_comms_e2e::scenarios::webtransport::{MAX_RESYNCS, bursts_for};

const HERE: [f32; 3] = [8.0, 0.0, 8.0];
const NEARBY: [f32; 3] = [10.0, 0.0, 9.0];
const SOON: Duration = Duration::from_secs(5);

async fn two_bots_see_each_other(transport: Transport) {
    let pulse = support::in_process_pulse_both().await;
    let ctx = pulse.ctx_on(transport, "see-each-other");
    let realm = ctx.realm("plaza");
    let (alice_id, bob_id) = (ctx.identity(0), ctx.identity(1));

    let alice = PulseBot::enter(&ctx, "alice", &alice_id).await.unwrap();
    alice.teleport(&realm, HERE).unwrap();
    let bob = PulseBot::enter(&ctx, "bob", &bob_id).await.unwrap();
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

async fn an_out_of_grid_input_is_refused_with_code_11(transport: Transport) {
    let pulse = support::in_process_pulse_both().await;
    let ctx = pulse.ctx_on(transport, "out-of-grid");
    let bot = PulseBot::enter(&ctx, "walker", &ctx.identity(0))
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
    assert_eq!(
        bot.view().disconnect,
        Some(DisconnectReason::InvalidInputField.code())
    );
}

async fn a_long_emote_is_refused_with_code_12(transport: Transport) {
    let pulse = support::in_process_pulse_both().await;
    let ctx = pulse.ctx_on(transport, "long-emote");
    let bot = PulseBot::enter(&ctx, "dancer", &ctx.identity(0))
        .await
        .unwrap();
    bot.teleport(&ctx.realm("plaza"), HERE).unwrap();
    let state = messages::state_at(bot.grid(), HERE);
    // exactly the limit is fine
    bot.send(
        messages::emote_start("wave", Some(DEFAULT_MAX_EMOTE_DURATION_MS), state, None),
        Channel::Reliable,
    );
    tokio::time::sleep(Duration::from_millis(300)).await;
    bot.expect_alive().unwrap();
    // one millisecond more is not
    bot.send(
        messages::emote_start("wave", Some(DEFAULT_MAX_EMOTE_DURATION_MS + 1), state, None),
        Channel::Reliable,
    );
    bot.expect_disconnect_with(&ctx, DisconnectReason::InvalidEmoteField, SOON)
        .await
        .unwrap();
}

async fn an_unexpected_disconnect_fails_fast(transport: Transport) {
    let pulse = support::in_process_pulse_both().await;
    let ctx = pulse.ctx_on(transport, "fails-fast");
    let bot = PulseBot::enter(&ctx, "walker", &ctx.identity(0))
        .await
        .unwrap();
    bot.send(
        messages::input_without_state(),
        Channel::UnreliableSequenced,
    );
    bot.expect_disconnect(&ctx, SOON).await.unwrap();

    let error = bot.expect_alive().unwrap_err().to_string();
    assert!(
        error.contains("walker") && error.contains("11 (InvalidInputField)"),
        "{error}"
    );

    // a wait that depends on a live connection ends at once, naming the code, not at its 60 s
    // deadline: the bound is a third of it, which a loaded machine keeps
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
        started.elapsed() < Duration::from_secs(20),
        "waited {:?}",
        started.elapsed()
    );
    assert!(error.contains("InvalidInputField"), "{error}");
}

async fn the_second_session_of_a_wallet_evicts_the_first(transport: Transport) {
    let pulse = support::in_process_pulse_both().await;
    let ctx = pulse.ctx_on(transport, "evicts");
    let identity = ctx.identity(0);
    let first = PulseBot::enter(&ctx, "first", &identity).await.unwrap();
    first.teleport(&ctx.realm("plaza"), HERE).unwrap();
    let second = PulseBot::enter(&ctx, "second", &identity.second_device())
        .await
        .unwrap();
    first
        .expect_disconnect_with(&ctx, DisconnectReason::DuplicateSession, SOON)
        .await
        .unwrap();
    second.expect_alive().unwrap();
}

async fn a_replayed_handshake_is_refused(transport: Transport) {
    let pulse = support::in_process_pulse_both().await;
    let ctx = pulse.ctx_on(transport, "replay");
    let identity = ctx.identity(0);
    let first = PulseBot::dial(&ctx, "first", ConnectOptions::default())
        .await
        .unwrap();
    // Signed once the transport is connected: a timestamp taken before the server has started
    // serving can be older than the floor of its replay table, and is refused as too old.
    let timestamp = catalyst_comms_e2e::identity::now_ms();
    let options = HandshakeOptions {
        timestamp_ms: Some(timestamp),
        ..Default::default()
    };
    assert_eq!(
        first
            .handshake(&ctx, &identity, options.clone())
            .await
            .unwrap(),
        HandshakeOutcome::Accepted
    );
    first.close();

    // the same wallet and timestamp, signed again by the same device: a replay
    let replay = PulseBot::dial(&ctx, "replay", ConnectOptions::default())
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

async fn nothing_listening_is_a_failed_connect_not_a_hang(transport: Transport) {
    // a port something holds and never answers on: held to the end, so no other test is given it
    let silent = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
    let dead = silent.local_addr().unwrap();
    let mut stack = support::Stack::pulse_only(dead);
    stack.pulse_webtransport = Some(support::WebTransportTarget {
        url: format!("https://127.0.0.1:{}/", dead.port())
            .parse()
            .unwrap(),
        certificate_hash: Some("00".repeat(32)),
    });
    let ctx = support::ctx_on_stack(std::sync::Arc::new(stack), transport, "dead");
    let options = ConnectOptions {
        deadline: Some(Duration::from_millis(1500)),
        ..Default::default()
    };
    let started = std::time::Instant::now();
    let error = PulseBot::dial(&ctx, "lonely", options)
        .await
        .err()
        .expect("no server, no connection");
    // the dial ends near its 1.5 s deadline, not never: the bound only catches a hang
    assert!(
        started.elapsed() < Duration::from_secs(30),
        "the dial took {:?}",
        started.elapsed()
    );
    // ENet keeps trying until the deadline. QUIC has no answer to wait for either: the dial
    // ends at the deadline with the client's own error, named in the message.
    assert!(
        error.to_string().contains("lonely connects to Pulse"),
        "{error}"
    );
}

/// A bot that closes ends its session at once: the server hears the close, not an idle timeout 30 s
/// later, and the others are told it left.
async fn a_closed_bot_is_gone_for_the_others_at_once(transport: Transport) {
    let pulse = support::in_process_pulse_both().await;
    let ctx = pulse.ctx_on(transport, "closed-is-gone");
    let realm = ctx.realm("plaza");
    let (stayer_id, leaver_id) = (ctx.identity(0), ctx.identity(1));
    let stayer = PulseBot::enter(&ctx, "stayer", &stayer_id).await.unwrap();
    stayer.teleport(&realm, HERE).unwrap();
    let leaver = PulseBot::enter(&ctx, "leaver", &leaver_id).await.unwrap();
    leaver.teleport(&realm, NEARBY).unwrap();
    expect_or(&ctx, "the stayer sees the leaver", SOON, || {
        stayer
            .view()
            .of(&leaver_id.address())
            .map(|_| ())
            .ok_or_else(|| "not yet".to_owned())
    })
    .await
    .unwrap();
    // straight after arriving, with no traffic in between: the close is the last thing the bot does
    leaver.close();
    expect_or(&ctx, "the stayer no longer sees the leaver", SOON, || {
        (stayer.view().count_of(&leaver_id.address()) == 0)
            .then_some(())
            .ok_or_else(|| "the leaver is still there".to_owned())
    })
    .await
    .unwrap();
}

/// Each case runs once per transport: `<case>::enet` and `<case>::webtransport`.
macro_rules! on_both_transports {
    ($($case:ident),* $(,)?) => {$(
        mod $case {
            use super::*;

            #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
            async fn enet() {
                super::$case(Transport::Enet).await
            }

            #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
            async fn webtransport() {
                super::$case(Transport::WebTransport).await
            }
        }
    )*};
}

on_both_transports!(
    two_bots_see_each_other,
    an_out_of_grid_input_is_refused_with_code_11,
    a_long_emote_is_refused_with_code_12,
    an_unexpected_disconnect_fails_fast,
    the_second_session_of_a_wallet_evicts_the_first,
    a_replayed_handshake_is_refused,
    nothing_listening_is_a_failed_connect_not_a_hang,
    a_closed_bot_is_gone_for_the_others_at_once,
);

/// QUIC through an opaque UDP relay: the walker dials WebTransport through the fault link, the
/// link stalls both ways for two seconds, and the session lives on.
async fn a_bot_through_a_stalled_link_survives_a_short_stall(transport: Transport) {
    let pulse = support::in_process_pulse_both().await;
    let ctx = pulse.ctx_on(transport, "bot-through-link");
    let realm = ctx.realm("plaza");
    let front = match transport {
        Transport::Enet => pulse.enet,
        Transport::WebTransport => {
            let url = &pulse.webtransport.url;
            std::net::SocketAddr::new("127.0.0.1".parse().unwrap(), url.port().unwrap())
        }
    };
    let link = FaultLink::udp(&ctx, "link", "127.0.0.1:0".parse().unwrap(), front)
        .await
        .unwrap();

    let walker_id = ctx.identity(0);
    let walker = PulseBot::dial(
        &ctx,
        "walker",
        ConnectOptions {
            via: Some(link.address()),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    walker.accepted(&ctx, &walker_id).await.unwrap();
    walker.teleport(&realm, HERE).unwrap();
    let observer = PulseBot::enter(&ctx, "observer", &ctx.identity(1))
        .await
        .unwrap();
    observer.teleport(&realm, NEARBY).unwrap();
    expect_or(&ctx, "the observer sees the walker", SOON, || {
        observer
            .view()
            .of(&walker_id.address())
            .map(|_| ())
            .ok_or_else(String::new)
    })
    .await
    .unwrap();

    link.set(Direction::Both, Fault::Stall);
    // An idle WebTransport connection can be quiet for the entire stall.
    // Send positions during the stall to exercise the link on both transports.
    for step in 1..=20 {
        walker
            .move_to([HERE[0] - step as f32 * 0.05, HERE[1], HERE[2]])
            .unwrap();
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert!(link.counters().dropped > 0, "the stall dropped nothing");
    link.restore();

    walker.expect_alive().unwrap();
    for step in 1..=20 {
        walker
            .move_to([HERE[0] + step as f32 * 0.25, 0.0, HERE[2]])
            .unwrap();
        tokio::time::sleep(Duration::from_millis(60)).await;
    }
    expect_or(
        &ctx,
        "the observer sees the walker 5 m further after the stall",
        SOON,
        || {
            walker.alive()?;
            let world = observer
                .view()
                .of(&walker_id.address())
                .map(|subject| subject.world)
                .ok_or("the walker is gone from the observer")?;
            ((world[0] - 13.0).abs() < 0.2)
                .then_some(())
                .ok_or_else(|| format!("the walker is at {world:?}"))
        },
    )
    .await
    .unwrap();
}

on_both_transports!(a_bot_through_a_stalled_link_survives_a_short_stall);

/// Five walkers and a WebTransport observer, shortened: the resyncs stay under the limit.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn webtransport_bursts_for_a_short_while() {
    let pulse = support::in_process_pulse_both().await;
    let ctx = pulse.ctx_on(Transport::WebTransport, "bursts-short");
    let resyncs = bursts_for(&ctx, Duration::from_secs(10), MAX_RESYNCS)
        .await
        .unwrap();
    eprintln!("resyncs in 10 s: {resyncs}");
}

/// The scenario at its full length, walkers on ENet and the observer on WebTransport.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "runs 100 s"]
async fn webtransport_bursts_at_full_length() {
    let pulse = support::in_process_pulse_both().await;
    let ctx = pulse.ctx_on(Transport::Enet, "bursts-full");
    let resyncs = bursts_for(
        &ctx,
        catalyst_comms_e2e::scenarios::webtransport::BURSTS_DURATION,
        MAX_RESYNCS,
    )
    .await
    .unwrap();
    eprintln!("resyncs in the full run: {resyncs}");
}

/// A frame far over the message limit (20 000 bytes, over twice the 4096-byte cap) ends the
/// connection at once with 16 on both transports: ENet's packet envelope refuses anything over
/// 8192 bytes, and the WebTransport stream reader applies the same envelope to the cap.
async fn an_oversized_frame_ends_the_connection_at_once(transport: Transport) {
    let pulse = support::in_process_pulse_both().await;
    let ctx = pulse.ctx_on(transport, "oversized");
    let bot = PulseBot::enter(&ctx, "walker", &ctx.identity(0))
        .await
        .unwrap();
    bot.send_raw(vec![0_u8; 20_000], Channel::Reliable);
    bot.expect_disconnect_with(&ctx, DisconnectReason::PacketCorrupted, SOON)
        .await
        .unwrap();
}

on_both_transports!(an_oversized_frame_ends_the_connection_at_once);
