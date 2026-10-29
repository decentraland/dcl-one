//! The MANA spot's freshness budget across its two readers: the price
//! route (`crate::price`) and the shop's credit feeds
//! (`shop::market::items::mana_usd_rate`) both refuse a snapshot past
//! `SNAPSHOT_MAX_AGE`. Serialized with the price tests on
//! `price::test_support::SNAPSHOTS`, since both read the newest coingecko row.

use std::sync::Arc;

use axum::http::{StatusCode, header};

use crate::price::test_support::{SNAPSHOTS, send, state_over};
use crate::price::{Settings, router};

/// A snapshot past `SNAPSHOT_MAX_AGE` is no quote (review finding 11): the
/// route answers the envelope naming the age, the credit feeds' rate reader
/// refuses it too, and a fresh row serves with its `Age`.
#[tokio::test]
async fn a_snapshot_past_the_freshness_budget_is_the_envelope_not_a_quote() {
    let Some(pool) = crate::db::scratch::pool(4).await else {
        return;
    };
    let _serial = SNAPSHOTS.lock().await;
    const STALE: f64 = 0.077_777_77;
    const FRESH: f64 = 0.088_888_88;
    let cleanup = || async {
        sqlx::query("DELETE FROM economy.price_snapshots WHERE source = 'coingecko' AND mana_usd IN ($1::numeric, $2::numeric)")
            .bind(STALE.to_string())
            .bind(FRESH.to_string())
            .execute(&pool)
            .await
            .unwrap();
    };
    cleanup().await;
    let others: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM economy.price_snapshots WHERE source = 'coingecko'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(others, 0, "another test left coingecko rows behind");
    sqlx::query("INSERT INTO economy.price_snapshots (source, mana_usd, taken_at) VALUES ('coingecko', $1::numeric, timezone('UTC', now()) - interval '40 days')")
        .bind(STALE.to_string())
        .execute(&pool)
        .await
        .unwrap();
    let state = state_over(pool.clone(), Settings::for_tests());
    let app = router(state.clone());
    let (status, headers, body) = send(
        app.clone(),
        "GET",
        "/api/v3/simple/price?ids=decentraland&vs_currencies=usd",
        vec![],
        None,
    )
    .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{body}");
    assert_eq!(body["source"], "mana-price-poll");
    assert!(
        body["message"]
            .as_str()
            .unwrap()
            .contains("past the 1800 s freshness budget"),
        "{body}"
    );
    assert_eq!(headers.get(header::RETRY_AFTER).unwrap(), "60");
    let market = Arc::new(crate::shop::market::MarketplaceState::new(
        pool.clone(),
        crate::http::Outbound::for_tests(),
        crate::shop::market::Settings::for_tests(),
    ));
    let err = crate::shop::market::items::mana_usd_rate(&market)
        .await
        .unwrap_err();
    assert_eq!(
        err.status(),
        StatusCode::SERVICE_UNAVAILABLE,
        "the credit feeds refuse the stale spot too"
    );

    sqlx::query("INSERT INTO economy.price_snapshots (source, mana_usd, taken_at) VALUES ('coingecko', $1::numeric, timezone('UTC', now()) - interval '90 seconds')")
        .bind(FRESH.to_string())
        .execute(&pool)
        .await
        .unwrap();
    let (status, headers, body) = send(
        app,
        "GET",
        "/api/v3/simple/price?ids=decentraland&vs_currencies=usd",
        vec![],
        None,
    )
    .await;
    assert_eq!(
        (status, body["decentraland"]["usd"].as_f64()),
        (StatusCode::OK, Some(FRESH)),
        "{body}"
    );
    let age: u64 = headers
        .get(header::AGE)
        .unwrap()
        .to_str()
        .unwrap()
        .parse()
        .unwrap();
    assert!(
        (85..=120).contains(&age),
        "the quote's age on this node rides the Age header: {age}"
    );
    market.readers.clear();
    assert_eq!(
        crate::shop::market::items::mana_usd_rate(&market)
            .await
            .unwrap(),
        FRESH
    );
    cleanup().await;
}
