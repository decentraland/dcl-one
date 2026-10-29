// from: catalyrst/crates/catalyrst-archipelago/tests/ban_reaches_v4_sockets.rs
//! A ban ends a wallet's v4 sockets as it ends its legacy ones. A v4 socket is registered beside
//! the legacy sessions and enters the peer directory only with its first position, so the sweep
//! has to find it in the registry and the kick has to reach it there. The ban authority is the
//! local policy over a scratch database (skips without `CATALYST_TEST_DATABASE_URL`).

use std::sync::Arc;

use catalyst::comms::archipelago::config::ClusterConfig;
use catalyst::comms::archipelago::peers::PeerDirectory;
use catalyst::comms::archipelago::registry::{PeersRegistry, SocketEvent};
use catalyst::worlds::policy::{PersonalWorldsPolicy, Policy};

#[path = "support/archipelago.rs"]
mod archipelago;
#[path = "support/db.rs"]
mod db;

const BANNED: &str = "0x00000000000000000000000000000000000bad01";
const CLEAN: &str = "0x00000000000000000000000000000000000c1ea0";

async fn banning(pool: &sqlx::PgPool) -> Arc<Policy> {
    sqlx::query("INSERT INTO worlds.blocked_wallets (wallet) VALUES ($1)")
        .bind(BANNED)
        .execute(pool)
        .await
        .unwrap();
    Arc::new(Policy::new(
        pool.clone(),
        PersonalWorldsPolicy::disabled(),
        None,
    ))
}

fn directory(policy: Arc<Policy>, registry: &Arc<PeersRegistry>) -> Arc<PeerDirectory> {
    PeerDirectory::new(
        ClusterConfig::default(),
        archipelago::gatekeeper(None),
        archipelago::ban_checker(policy),
        Arc::clone(registry),
    )
}

#[tokio::test]
async fn the_ban_sweep_ends_a_v4_socket_that_has_sent_no_position() {
    let Some(scratch) = db::scratch_database().await else {
        eprintln!("skipping: CATALYST_TEST_DATABASE_URL not set");
        return;
    };
    let registry = PeersRegistry::new();
    let peers = directory(banning(&scratch.pool).await, &registry);
    let (_banned, mut banned_events) = registry.on_v4_peer_connected(BANNED, "0xs1", "conn-1");
    let (_clean, mut clean_events) = registry.on_v4_peer_connected(CLEAN, "0xs2", "conn-2");

    peers.ban_sweep_once().await;

    assert!(matches!(banned_events.try_recv(), Ok(SocketEvent::Kicked)));
    assert!(
        clean_events.try_recv().is_err(),
        "the clean wallet's socket is left alone"
    );
    scratch.teardown().await;
}

#[tokio::test]
async fn the_ban_sweep_ends_both_listeners_sockets_of_a_wallet_in_the_directory() {
    let Some(scratch) = db::scratch_database().await else {
        eprintln!("skipping: CATALYST_TEST_DATABASE_URL not set");
        return;
    };
    let registry = PeersRegistry::new();
    let peers = directory(banning(&scratch.pool).await, &registry);
    let (_legacy, mut legacy_events, _) = registry.on_peer_connected(BANNED, "0xs1");
    let (_v4, mut v4_events) = registry.on_v4_peer_connected(BANNED, "0xs2", "conn-1");
    peers.upsert_peer(BANNED.into(), [0.0, 0.0, 0.0], [0, 0], "r".into());

    peers.ban_sweep_once().await;

    assert!(matches!(legacy_events.try_recv(), Ok(SocketEvent::Kicked)));
    assert!(matches!(v4_events.try_recv(), Ok(SocketEvent::Kicked)));
    assert!(peers.peer(BANNED).is_none());
    scratch.teardown().await;
}
