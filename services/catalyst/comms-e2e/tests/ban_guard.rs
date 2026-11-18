//! The ban of `ban-eviction` is lifted on every way out of the scenario, against a stand-in for
//! the gatekeeper that records the requests it gets.

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use anyhow::bail;
use axum::Router;
use axum::routing::post;
use catalyst_comms_e2e::engine::Ctx;
use catalyst_comms_e2e::gatekeeper::Gatekeeper;
use catalyst_comms_e2e::scenarios::connections_support::BanGuard;
use catalyst_comms_e2e::stack::Stack;

const WALLET: &str = "0x00000000000000000000000000000000000000aa";

#[derive(Default)]
struct Seen {
    bans: AtomicU32,
    unbans: AtomicU32,
}

/// A gatekeeper that answers every ban and unban with success and counts them.
async fn stand_in(unban_status: u16) -> (std::net::SocketAddr, Arc<Seen>) {
    let seen = Arc::new(Seen::default());
    let (on_ban, on_unban) = (seen.clone(), seen.clone());
    let route = "/comms-gatekeeper/users/{address}/bans";
    let router = Router::new().route(
        route,
        post(move || {
            let seen = on_ban.clone();
            async move {
                seen.bans.fetch_add(1, Ordering::SeqCst);
                "{}"
            }
        })
        .delete(move || {
            let seen = on_unban.clone();
            async move {
                seen.unbans.fetch_add(1, Ordering::SeqCst);
                (
                    axum::http::StatusCode::from_u16(unban_status).unwrap(),
                    "{}",
                )
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    (address, seen)
}

fn ctx_for(gatekeeper: std::net::SocketAddr) -> Ctx {
    // the moderator token is read from an environment variable that always exists; the stand-in
    // does not check it
    let text = format!(
        "api = \"http://{gatekeeper}\"\npulse_enet = \"127.0.0.1:9\"\nmoderator_token_env = \"CARGO_PKG_NAME\"\n"
    );
    Ctx::new(
        Arc::new(Stack::parse(&text).unwrap()),
        &format!("t{}", std::process::id()),
        "ban-guard",
        0,
        1,
        std::env::temp_dir().join("comms-e2e-tests"),
    )
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_ban_lifted_on_the_normal_path_is_lifted_once() {
    let (address, seen) = stand_in(200).await;
    let ctx = ctx_for(address);
    let gatekeeper = Gatekeeper::new(&ctx).unwrap();
    let guard = BanGuard::place(&ctx, &gatekeeper, WALLET, "a test")
        .await
        .unwrap();
    assert_eq!(seen.bans.load(Ordering::SeqCst), 1);
    guard.lift().await.unwrap();
    assert_eq!(seen.unbans.load(Ordering::SeqCst), 1);
    // nothing is left for the guard to do when it is gone
    assert_eq!(seen.unbans.load(Ordering::SeqCst), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_scenario_body_that_fails_early_still_lifts_the_ban() {
    let (address, seen) = stand_in(200).await;
    let ctx = ctx_for(address);
    let gatekeeper = Gatekeeper::new(&ctx).unwrap();
    let body = async {
        let _guard = BanGuard::place(&ctx, &gatekeeper, WALLET, "a test").await?;
        bail!("the first check failed");
        #[allow(unreachable_code)]
        anyhow::Ok(())
    };
    let error: anyhow::Error = body.await.unwrap_err();
    assert!(error.to_string().contains("first check"));
    assert_eq!(seen.unbans.load(Ordering::SeqCst), 1, "the ban stayed");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_future_dropped_by_a_deadline_still_lifts_the_ban() {
    let (address, seen) = stand_in(200).await;
    let ctx = ctx_for(address);
    let gatekeeper = Gatekeeper::new(&ctx).unwrap();
    let body = async {
        let _guard = BanGuard::place(&ctx, &gatekeeper, WALLET, "a test").await?;
        tokio::time::sleep(Duration::from_secs(3600)).await;
        anyhow::Ok(())
    };
    let outcome = tokio::time::timeout(Duration::from_millis(300), body).await;
    assert!(outcome.is_err(), "the deadline must have ended it");
    // the request is complete when the future is gone, not some time later
    assert_eq!(seen.unbans.load(Ordering::SeqCst), 1, "the ban stayed");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_ban_that_is_already_gone_is_not_an_error_to_lift() {
    // an unban that finds nothing (404) is fine
    let (address, seen) = stand_in(404).await;
    let ctx = ctx_for(address);
    let gatekeeper = Gatekeeper::new(&ctx).unwrap();
    let guard = BanGuard::place(&ctx, &gatekeeper, WALLET, "a test")
        .await
        .unwrap();
    guard.lift().await.expect("nothing to lift is not an error");
    assert_eq!(seen.unbans.load(Ordering::SeqCst), 1);
}
