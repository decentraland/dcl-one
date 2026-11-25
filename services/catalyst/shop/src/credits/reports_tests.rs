//! The credits reports against PostgreSQL: who may read them, and that each
//! row source lands in the old server's shape (`reports.rs`).

use std::sync::Arc;

use axum::http::{HeaderMap, StatusCode, header};
use serde_json::{Value, json};

use super::tests::{app, scratch_wallet, send, signed_headers};
use super::{RawSettings, Settings, SharedState, State};
use crate::credits::store::now;
use crate::credits::tests::{FakeMarket, MockBroker};
use crate::credits::usage::CHAIN_POLYGON;
use crate::http::Outbound;

const TOKEN: &str = "s3cret";

fn reports_state(pool: sqlx::PgPool, read_admin: &str) -> SharedState {
    let settings = Settings::parse(RawSettings {
        writes_enabled: true,
        usage_chain_id: CHAIN_POLYGON,
        checkout_fulfillment_mode: "secondary".to_string(),
        admin_token: Some(TOKEN.to_string()),
        read_admin_addresses: format!("  {} ,", read_admin.to_uppercase().replace("0X", "0x")),
        marketplace_markup_bps: 0,
        mana_price_max_staleness_secs: 3600,
        ..RawSettings::default()
    })
    .unwrap();
    Arc::new(State::with_sources(
        pool,
        Outbound::for_tests(),
        settings,
        Arc::new(FakeMarket::default()),
        Arc::new(MockBroker::configured()),
        None,
    ))
}

fn bearer() -> HeaderMap {
    let mut h = HeaderMap::new();
    h.insert(
        header::AUTHORIZATION,
        format!("Bearer {TOKEN}").parse().unwrap(),
    );
    h
}

/// A take-photo credit for `address` in season 1, with `consumed` wei of it spent on chain.
async fn goal_credit(pool: &sqlx::PgPool, address: &str, wei: &str, consumed: &str) -> String {
    sqlx::query("INSERT INTO credits.seasons(id,name,start_date,end_date,max_mana_wei) VALUES(1,'Season One',timezone('UTC',now())-interval '1 day',timezone('UTC',now())+interval '30 days',0) ON CONFLICT DO NOTHING")
        .execute(pool).await.unwrap();
    sqlx::query("INSERT INTO credits.season_goals(season_id,goal_id,title,thumbnail,total_steps,reward_wei,start_week,end_week) VALUES(1,'take-photo','Take a photo','p.png',1,0,1,20) ON CONFLICT DO NOTHING")
        .execute(pool).await.unwrap();
    sqlx::query("INSERT INTO credits.wallets(address) VALUES($1) ON CONFLICT DO NOTHING")
        .bind(address)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO credits.goal_progress(address,season_id,week_id,goal_id,title,thumbnail,total_steps,reward_wei,completed_steps,completed_at) VALUES($1,1,1,'take-photo','Take a photo','p.png',1,0,1,$2)")
        .bind(address).bind(now()).execute(pool).await.unwrap();
    let salt = format!("0x{}{}", "1".repeat(24), &address[2..]);
    let contract = "0x00000000000000000000000000000000000000cc";
    sqlx::query("INSERT INTO credits.goal_grants(address,season_id,week_id,goal_id,salt,chain_id,contract,credit_hash,amount_wei,signature,created_at,expires_at) VALUES($1,1,1,'take-photo',$2,137,$3,$2,$4::numeric,'0xsig',$5,$5 + interval '30 days')")
        .bind(address).bind(&salt).bind(contract).bind(wei).bind(now()).execute(pool).await.unwrap();
    sqlx::query("INSERT INTO credits.usages(chain_id,block_number,log_index,contract,tx_hash,sender,credit_id,salt,credit_value,credit_expires_at,value_wei) VALUES(137,1,0,$1,$2,$3,$2,$4,$5::numeric,0,$6::numeric)")
        .bind(contract)
        .bind(format!("0x{}", "ab".repeat(32)))
        .bind(address)
        .bind(&salt)
        .bind(wei)
        .bind(consumed)
        .execute(pool)
        .await
        .unwrap();
    salt
}

fn ids(v: &Value) -> Vec<String> {
    v["data"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["id"].as_str().unwrap().to_string())
        .collect()
}

#[tokio::test]
async fn the_reports_answer_read_admins_in_the_old_row_shape() {
    // The list reads whole tables and the fixture opens a season, so it gets
    // its own database: in the shared one an open season changes what other
    // tests' goal routes write.
    let Some(pool) = crate::db::scratch::isolated_pool(8).await else {
        return;
    };
    let admin = scratch_wallet();
    let outsider = scratch_wallet();
    let app = app(reports_state(pool.clone(), &admin.address));

    let granted = scratch_wallet();
    let (status, _, v) = send(
        &app,
        "POST",
        "/credits/admin/credits/grant",
        bearer(),
        Some(json!({ "address": granted.address, "amount": "40", "reason": "launch", "idempotencyKey": uuid::Uuid::new_v4().to_string() })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{v}");
    let buyer = scratch_wallet();
    sqlx::query("INSERT INTO credits.ledger (address, kind, amount, bucket) VALUES ($1, 'purchase', 10, 'paid')")
        .bind(&buyer.address)
        .execute(&pool)
        .await
        .unwrap();
    let earner = scratch_wallet();
    let salt = goal_credit(&pool, &earner.address, "5000", "1200").await;

    // Neither the bearer nor an admin wallet: the old 401.
    let (status, _, _) = send(&app, "GET", "/credits/credits", HeaderMap::new(), None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let headers = signed_headers(&outsider, "get", "/credits/credits").await;
    let (status, _, _) = send(&app, "GET", "/credits/credits", headers, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "a wallet off the list");

    // An admin wallet, no filters: the operator's grants, as the old dashboard expects.
    let headers = signed_headers(&admin, "get", "/credits/credits").await;
    let (status, _, v) = send(&app, "GET", "/credits/credits", headers, None).await;
    assert_eq!(status, StatusCode::OK, "{v}");
    assert_eq!(v["total"], 1, "{v}");
    let row = &v["data"][0];
    assert_eq!(row["userAddress"], granted.address);
    assert_eq!(row["denomination"], "USD");
    assert_eq!(row["usdCents"], 400, "40 credits are 400 cents");
    assert_eq!(row["reason"], "launch");
    assert_eq!(row["amount"], Value::Null);
    assert_eq!(row["contract"], Value::Null);
    assert_eq!(row["signature"], Value::Null);
    assert_eq!(row["expiresAt"], "0");
    assert_eq!(row["totalConsumed"], "0");
    assert_eq!(
        row["granterAddress"],
        Value::Null,
        "the bearer is not a wallet"
    );
    assert!(row["timestamp"].as_str().unwrap().parse::<i64>().unwrap() > 0);

    // A source drops the goal default.
    let (status, _, v) = send(
        &app,
        "GET",
        "/credits/credits?source=purchase",
        bearer(),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{v}");
    assert_eq!(v["total"], 1, "{v}");
    assert_eq!(v["data"][0]["userAddress"], buyer.address);
    assert_eq!(v["data"][0]["usdCents"], 100);

    // A goal credit is a MANA row with its season, contract, signature, expiry and consumption.
    let (status, _, v) = send(
        &app,
        "GET",
        "/credits/credits?goal_id=take-photo",
        bearer(),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{v}");
    assert_eq!(ids(&v), vec![format!("goal:{salt}")]);
    let row = &v["data"][0];
    assert_eq!(row["denomination"], "MANA");
    assert_eq!(row["amount"], "5000");
    assert_eq!(row["usdCents"], Value::Null);
    assert_eq!(row["seasonId"], 1);
    assert_eq!(row["seasonName"], "Season One");
    assert_eq!(
        row["contract"],
        "0x00000000000000000000000000000000000000cc"
    );
    assert_eq!(row["signature"], "0xsig");
    assert_eq!(row["totalConsumed"], "1200");
    assert!(row["expiresAt"].as_str().unwrap().parse::<i64>().unwrap() > 0);

    // An amount bound is MANA wei, so it never matches a USD row (as in the old server).
    let (_, _, v) = send(
        &app,
        "GET",
        "/credits/credits?source=goal&min_amount=4000",
        bearer(),
        None,
    )
    .await;
    assert_eq!(v["total"], 1, "{v}");
    let (_, _, v) = send(
        &app,
        "GET",
        "/credits/credits?source=goal&min_amount=6000",
        bearer(),
        None,
    )
    .await;
    assert_eq!(v["total"], 0, "{v}");
    let (_, _, v) = send(
        &app,
        "GET",
        "/credits/credits?source=purchase&max_amount=99999",
        bearer(),
        None,
    )
    .await;
    assert_eq!(v["total"], 0, "{v}");

    // The address filter, case-insensitively, and an empty page keeps the total.
    let upper = granted.address.to_uppercase().replace("0X", "0x");
    let (_, _, v) = send(
        &app,
        "GET",
        &format!("/credits/credits?address={upper}&offset=5"),
        bearer(),
        None,
    )
    .await;
    assert_eq!(v["total"], 1, "{v}");
    assert!(v["data"].as_array().unwrap().is_empty());

    // The grant reasons.
    let headers = signed_headers(&admin, "get", "/credits/credit-reasons").await;
    let (status, _, v) = send(&app, "GET", "/credits/credit-reasons", headers, None).await;
    assert_eq!(status, StatusCode::OK, "{v}");
    assert_eq!(v, json!({ "data": ["launch"], "total": 1 }));
    let (status, _, _) = send(
        &app,
        "GET",
        "/credits/credit-reasons",
        HeaderMap::new(),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[test]
fn a_bad_read_admin_address_fails_startup() {
    let err = Settings::parse(RawSettings {
        read_admin_addresses: "0x12".to_string(),
        ..RawSettings::default()
    })
    .err()
    .expect("refused");
    assert!(
        err.to_string().contains("CREDITS_READ_ADMIN_ADDRESSES"),
        "{err}"
    );
}
