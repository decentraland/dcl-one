// from: catalyrst/crates/catalyrst-server/tests/sync_frontier_monotonic.rs
//! Pins the writer semantics the sync path relies on (upstream snapshots-fetcher 53e9c07
//! parity round, plus the per-server cursor work): the persisted global sync frontier is
//! GREATEST-monotonic through `advance_sync_frontier`, so a stale offer can never rewind the
//! durable frontier or the freshness gauge derived from it -- and each server's OWN cursor
//! (`content.server_sync_cursors`) has the same guarantee through
//! `advance_server_sync_cursor`, while bootstrap resumes every server from its own cursor
//! rather than the max-over-servers frontier. Needs CATALYST_TEST_DATABASE_URL (one scratch
//! database per test); the upserts' GREATEST shapes are also pinned without a database by
//! the SQL-shape tests in content/sync/repository.rs. The old file's fifth test, the 42P01
//! missing-cursor-table degradation to the global frontier, was dropped with that fallback:
//! schema.sql always provides content.server_sync_cursors and startup refuses an
//! incompatible schema, so there is no degraded table to resume from.

use std::sync::Arc;

use sqlx::PgPool;

use catalyst::content::sync::{LiveDeploymentRepository, SyncStatus, resume_point};

#[path = "support/auth.rs"]
mod auth;
#[path = "support/db.rs"]
mod db;
#[path = "support/phased_bootstrap.rs"]
mod phased_bootstrap;

fn repo(pool: &PgPool) -> (LiveDeploymentRepository, Arc<SyncStatus>) {
    let status = Arc::new(SyncStatus::default());
    (
        LiveDeploymentRepository::new(pool.clone(), status.clone()),
        status,
    )
}

#[tokio::test]
async fn a_late_straggler_floor_cannot_rewind_the_persisted_frontier() {
    let Some(db) = db::scratch_database().await else {
        return;
    };
    let (repo, status) = repo(&db.pool);

    repo.advance_sync_frontier(1_700_000_000_000).await.unwrap();
    assert_eq!(repo.get_sync_frontier().await.unwrap(), 1_700_000_000_000);

    repo.advance_sync_frontier(1_600_000_000_000).await.unwrap();
    assert_eq!(
        repo.get_sync_frontier().await.unwrap(),
        1_700_000_000_000,
        "a lagging floor offer must never lower the persisted frontier"
    );
    assert_eq!(
        status.sync_frontier_ms(),
        Some(1_700_000_000_000),
        "the gauge /status reports follows the persisted frontier, never a stale offer"
    );

    repo.advance_sync_frontier(1_800_000_000_000).await.unwrap();
    assert_eq!(repo.get_sync_frontier().await.unwrap(), 1_800_000_000_000);
    assert_eq!(status.sync_frontier_ms(), Some(1_800_000_000_000));

    db.teardown().await;
}

#[tokio::test]
async fn advance_from_scratch_installs_the_first_value() {
    let Some(db) = db::scratch_database().await else {
        return;
    };
    let (repo, _) = repo(&db.pool);
    assert_eq!(repo.get_sync_frontier().await.unwrap(), 0);
    repo.advance_sync_frontier(123_456).await.unwrap();
    assert_eq!(repo.get_sync_frontier().await.unwrap(), 123_456);
    db.teardown().await;
}

#[tokio::test]
async fn each_server_resumes_from_its_own_cursor_after_restart() {
    let Some(db) = db::scratch_database().await else {
        return;
    };
    let (repo, _) = repo(&db.pool);
    let server_a = "https://stalled.example.com/content";
    let server_b = "https://healthy.example.com/content";
    let server_c = "https://never-seen.example.com/content";

    repo.advance_server_sync_cursor(server_a, 1_000_000)
        .await
        .unwrap();
    repo.advance_sync_frontier(1_000_000).await.unwrap();

    repo.advance_server_sync_cursor(server_b, 9_000_000)
        .await
        .unwrap();
    repo.advance_sync_frontier(9_000_000).await.unwrap();

    let frontier = repo.get_sync_frontier().await.unwrap();
    let cursor_a = repo.get_server_sync_cursor(server_a).await.unwrap();
    let cursor_b = repo.get_server_sync_cursor(server_b).await.unwrap();
    let cursor_c = repo.get_server_sync_cursor(server_c).await.unwrap();

    assert_eq!(
        resume_point(cursor_a, frontier),
        1_000_000,
        "the stalled server must resume from its own cursor, not ride the global frontier \
         past its undeployed entities"
    );
    assert_eq!(resume_point(cursor_b, frontier), 9_000_000);
    assert_eq!(
        (cursor_c, resume_point(cursor_c, frontier)),
        (None, 9_000_000),
        "a server never seen before falls back to the global frontier"
    );
    assert_eq!(
        frontier, 9_000_000,
        "the global frontier stays max-over-servers reporting state"
    );

    assert_eq!(
        repo.get_server_sync_cursor(server_a).await.unwrap(),
        Some(1_000_000)
    );

    db.teardown().await;
}

#[tokio::test]
async fn a_stale_offer_cannot_rewind_a_server_cursor() {
    let Some(db) = db::scratch_database().await else {
        return;
    };
    let (repo, _) = repo(&db.pool);
    let server_a = "https://a.example.com/content";
    let server_b = "https://b.example.com/content";

    repo.advance_server_sync_cursor(server_a, 2_000)
        .await
        .unwrap();
    repo.advance_server_sync_cursor(server_b, 5_000)
        .await
        .unwrap();

    repo.advance_server_sync_cursor(server_a, 1_500)
        .await
        .unwrap();
    assert_eq!(
        repo.get_server_sync_cursor(server_a).await.unwrap(),
        Some(2_000),
        "a stale offer must never lower a server's persisted cursor"
    );

    repo.advance_server_sync_cursor(server_a, 2_500)
        .await
        .unwrap();
    assert_eq!(
        repo.get_server_sync_cursor(server_a).await.unwrap(),
        Some(2_500)
    );
    assert_eq!(
        repo.get_server_sync_cursor(server_b).await.unwrap(),
        Some(5_000),
        "one server's writes must never touch another server's cursor"
    );

    db.teardown().await;
}
