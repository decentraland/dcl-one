//! A completed `/ws` challenge handshake credits a login of its wallet, once
//! per handshake; a chain that did not sign this socket's challenge credits
//! nothing. With social's referral state behind it, the credit counts one
//! login day per UTC date and finalizes the referral as `client-login` does.

use std::sync::Arc;
use std::time::Duration;

use alloy::signers::{SignerSync, local::PrivateKeySigner};
use catalyst::comms::archipelago::logins::LoginCredits;
use catalyst::comms::interfaces::LoginRecorder;
use catalyst::comms::interfaces::test_support::RecordedLogins;
use catalyst::comms::protocol::archipelago::archipelago::{
    ChallengeRequestMessage, ClientPacket, ServerPacket, SignedChallengeMessage, client_packet,
    server_packet,
};
use catalyst::social::communities::referral::{Referral, Settings as ReferralSettings};
use futures::{SinkExt, StreamExt};
use prost::Message as _;
use serde_json::{Value, json};
use tokio_tungstenite::tungstenite::Message as WsMessage;

#[path = "support/archipelago.rs"]
mod archipelago;
#[path = "support/db.rs"]
mod db;

type WsStream =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

async fn serve(logins: Arc<dyn LoginRecorder>) -> u16 {
    let state = catalyst::comms::archipelago::build_state(
        archipelago::settings(),
        archipelago::gatekeeper(None),
        logins,
        &catalyst::jobs::Jobs::new(),
    )
    .await
    .expect("state");
    archipelago::serve(state).await
}

fn encode(msg: client_packet::Message) -> tokio_tungstenite::tungstenite::Bytes {
    ClientPacket {
        request_sequence: 0,
        message: Some(msg),
    }
    .encode_to_vec()
    .into()
}

async fn recv(ws: &mut WsStream) -> Option<server_packet::Message> {
    loop {
        let frame = tokio::time::timeout(Duration::from_secs(3), ws.next())
            .await
            .ok()??;
        match frame.ok()? {
            WsMessage::Binary(bytes) => return ServerPacket::decode(bytes.as_ref()).ok()?.message,
            WsMessage::Ping(_) | WsMessage::Pong(_) => continue,
            _ => return None,
        }
    }
}

/// Opens a socket for `wallet` and returns it with the challenge it was handed.
async fn challenged(port: u16, wallet: &PrivateKeySigner) -> (WsStream, String) {
    let (mut ws, _) = tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{port}/ws"))
        .await
        .expect("ws connect");
    ws.send(WsMessage::Binary(encode(
        client_packet::Message::ChallengeRequest(ChallengeRequestMessage {
            address: address(wallet),
        }),
    )))
    .await
    .unwrap();
    match recv(&mut ws).await {
        Some(server_packet::Message::ChallengeResponse(r)) => (ws, r.challenge_to_sign),
        other => panic!("expected a challenge, got {other:?}"),
    }
}

fn address(wallet: &PrivateKeySigner) -> String {
    format!("{:#x}", wallet.address())
}

fn sign(wallet: &PrivateKeySigner, payload: &str) -> String {
    let hash = alloy::primitives::eip191_hash_message(payload.as_bytes());
    wallet.sign_hash_sync(&hash).unwrap().to_string()
}

/// The owner's chain signing `challenge` through an ephemeral key that expires at `expiration`.
fn chain(
    owner: &PrivateKeySigner,
    challenge: &str,
    expiration: chrono::DateTime<chrono::Utc>,
) -> Value {
    let ephemeral = PrivateKeySigner::random();
    let delegation = format!(
        "Decentraland Login\nEphemeral address: {}\nExpiration: {}",
        address(&ephemeral),
        expiration.to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
    );
    json!([
        { "type": "SIGNER", "payload": address(owner), "signature": "" },
        { "type": "ECDSA_EPHEMERAL", "payload": delegation, "signature": sign(owner, &delegation) },
        { "type": "ECDSA_SIGNED_ENTITY", "payload": challenge, "signature": sign(&ephemeral, challenge) },
    ])
}

fn tomorrow() -> chrono::DateTime<chrono::Utc> {
    chrono::Utc::now() + chrono::Duration::days(1)
}

/// Sends `chain` as the signed challenge: true when the socket is welcomed.
async fn answer(ws: &mut WsStream, chain: &Value) -> bool {
    ws.send(WsMessage::Binary(encode(
        client_packet::Message::SignedChallenge(SignedChallengeMessage {
            auth_chain_json: chain.to_string(),
        }),
    )))
    .await
    .unwrap();
    matches!(recv(ws).await, Some(server_packet::Message::Welcome(_)))
}

/// Completes a handshake for `wallet`: its challenge.
async fn log_in(port: u16, wallet: &PrivateKeySigner) -> String {
    let (mut ws, challenge) = challenged(port, wallet).await;
    assert!(answer(&mut ws, &chain(wallet, &challenge, tomorrow())).await);
    challenge
}

async fn eventually(mut done: impl FnMut() -> bool) {
    for _ in 0..100 {
        if done() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    panic!("the login credit never landed");
}

#[tokio::test]
async fn each_completed_handshake_credits_one_login() {
    let recorded = Arc::new(RecordedLogins::default());
    let port = serve(recorded.clone()).await;
    let wallet = PrivateKeySigner::random();

    let first = log_in(port, &wallet).await;
    let second = log_in(port, &wallet).await;
    eventually(|| recorded.0.lock().unwrap().len() == 2).await;
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(
        *recorded.0.lock().unwrap(),
        vec![
            (address(&wallet), LoginCredits::login_id(&first)),
            (address(&wallet), LoginCredits::login_id(&second)),
        ]
    );
}

#[tokio::test]
async fn a_chain_that_did_not_sign_this_challenge_credits_nothing() {
    let recorded = Arc::new(RecordedLogins::default());
    let port = serve(recorded.clone()).await;
    let wallet = PrivateKeySigner::random();

    // A chain that signed an earlier socket's challenge.
    let (mut ws, old) = challenged(port, &wallet).await;
    let replayed = chain(&wallet, &old, tomorrow());
    drop(ws.close(None).await);
    let (mut ws, _) = challenged(port, &wallet).await;
    assert!(!answer(&mut ws, &replayed).await, "replayed chain");

    // The bare login delegation a signed fetch carries, without the challenge.
    let (mut ws, challenge) = challenged(port, &wallet).await;
    let mut links = chain(&wallet, &challenge, tomorrow());
    links.as_array_mut().unwrap().pop();
    assert!(!answer(&mut ws, &links).await, "bare delegation");

    // An expired delegation over this very challenge.
    let (mut ws, challenge) = challenged(port, &wallet).await;
    let expired = chain(
        &wallet,
        &challenge,
        chrono::Utc::now() - chrono::Duration::days(1),
    );
    assert!(!answer(&mut ws, &expired).await, "expired delegation");

    tokio::time::sleep(Duration::from_millis(200)).await;
    assert!(recorded.0.lock().unwrap().is_empty());
}

async fn referral(pool: &sqlx::PgPool, min_login_days: f64) -> Arc<Referral> {
    let settings = ReferralSettings {
        min_login_days,
        ..ReferralSettings::for_tests()
    };
    Arc::new(Referral::new(
        pool.clone(),
        catalyst::http::Outbound::for_tests(),
        settings,
    ))
}

/// `wallet` invited by a fresh referrer; the referrer.
async fn invite(referral: &Referral, wallet: &PrivateKeySigner) -> String {
    let referrer = address(&PrivateKeySigner::random());
    referral
        .create(
            &catalyst::shared::identity::Address::parse(&referrer).unwrap(),
            &catalyst::shared::identity::Address::parse(&address(wallet)).unwrap(),
            "192.0.2.1",
        )
        .await
        .unwrap();
    referrer
}

async fn progress(pool: &sqlx::PgPool, wallet: &PrivateKeySigner) -> (String, usize, i64) {
    let invited = address(wallet);
    let (status, days): (String, Vec<chrono::NaiveDate>) = sqlx::query_as(
        "SELECT status, login_days FROM social.referral_progress WHERE invited_user = $1",
    )
    .bind(&invited)
    .fetch_one(pool)
    .await
    .unwrap();
    let receipts: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM social.referral_login_receipts WHERE invited_user = $1",
    )
    .bind(&invited)
    .fetch_one(pool)
    .await
    .unwrap();
    (status, days.len(), receipts)
}

async fn accepted_notices(pool: &sqlx::PgPool, referrer: &str) -> i64 {
    sqlx::query_scalar(
        "SELECT count(*) FROM social.notifications \
         WHERE address = $1 AND type = 'referral_invited_users_accepted'",
    )
    .bind(referrer)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn eventually_progress(pool: &sqlx::PgPool, wallet: &PrivateKeySigner, receipts: i64) {
    for _ in 0..100 {
        if progress(pool, wallet).await.2 >= receipts {
            return;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    panic!("the login credit never landed");
}

#[tokio::test]
async fn handshakes_on_one_day_count_one_login_day() {
    let Some(scratch) = db::scratch_database().await else {
        eprintln!("skipping: CATALYST_TEST_DATABASE_URL not set");
        return;
    };
    let referral = referral(&scratch.pool, 3.0).await;
    let port = serve(referral.clone()).await;
    // A run that crosses UTC midnight counts two days; run it again with a fresh wallet.
    for _ in 0..2 {
        let day = utc_date(&scratch.pool).await;
        let wallet = PrivateKeySigner::random();
        let referrer = invite(&referral, &wallet).await;

        log_in(port, &wallet).await;
        eventually_progress(&scratch.pool, &wallet, 1).await;
        // Reconnects on a counted date add no day and take no quota slot.
        for _ in 0..3 {
            log_in(port, &wallet).await;
        }
        tokio::time::sleep(Duration::from_millis(300)).await;
        let (progress, notices) = (
            progress(&scratch.pool, &wallet).await,
            accepted_notices(&scratch.pool, &referrer).await,
        );
        if utc_date(&scratch.pool).await != day {
            continue;
        }
        assert_eq!(progress, ("pending".into(), 1, 1));
        assert_eq!(notices, 0);
        scratch.teardown().await;
        return;
    }
    panic!("two runs crossed UTC midnight");
}

async fn utc_date(pool: &sqlx::PgPool) -> chrono::NaiveDate {
    sqlx::query_scalar("SELECT timezone('UTC',clock_timestamp())::date")
        .fetch_one(pool)
        .await
        .unwrap()
}

#[tokio::test]
async fn a_handshake_finalizes_the_referral_and_notifies_once() {
    let Some(scratch) = db::scratch_database().await else {
        eprintln!("skipping: CATALYST_TEST_DATABASE_URL not set");
        return;
    };
    let referral = referral(&scratch.pool, 0.0).await;
    let port = serve(referral.clone()).await;
    let wallet = PrivateKeySigner::random();
    let referrer = invite(&referral, &wallet).await;

    let challenge = log_in(port, &wallet).await;
    eventually_progress(&scratch.pool, &wallet, 1).await;
    assert_eq!(progress(&scratch.pool, &wallet).await.0, "tier_granted");
    log_in(port, &wallet).await;
    // The same handshake recorded again is a duplicate receipt.
    referral
        .record_login(&address(&wallet), LoginCredits::login_id(&challenge))
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(accepted_notices(&scratch.pool, &referrer).await, 1);
    let granted: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM social.referral_progress WHERE referrer = $1 AND tier_granted",
    )
    .bind(&referrer)
    .fetch_one(&scratch.pool)
    .await
    .unwrap();
    assert_eq!(granted, 1, "the referral badge counts this grant");
    scratch.teardown().await;
}
