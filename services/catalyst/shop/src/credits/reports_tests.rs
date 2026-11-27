//! The credits reports against PostgreSQL: who may read them, and that each
//! row source lands in the old server's shape (`reports.rs`).

#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    reason = "test fixtures use small literal values"
)]

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

/// A claimed take-photo credit for `address` in season 1, with each of `consumed`
/// spent on chain in its own CreditUsed log.
async fn goal_credit(pool: &sqlx::PgPool, address: &str, wei: &str, consumed: &[&str]) -> String {
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
    sqlx::query("INSERT INTO credits.goal_grants(address,season_id,week_id,goal_id,salt,chain_id,contract,credit_hash,amount_wei,signature,created_at,expires_at,claimed_at) VALUES($1,1,1,'take-photo',$2,137,$3,$2,$4::numeric,'0xsig',$5,$5 + interval '30 days',$5)")
        .bind(address).bind(&salt).bind(contract).bind(wei).bind(now()).execute(pool).await.unwrap();
    for (i, part) in consumed.iter().enumerate() {
        sqlx::query("INSERT INTO credits.usages(chain_id,block_number,log_index,contract,tx_hash,sender,credit_id,salt,credit_value,credit_expires_at,value_wei) VALUES(137,$8,$1,$2,$3,$4,$3,$5,$6::numeric,0,$7::numeric)")
            .bind(i as i32)
            .bind(contract)
            .bind(format!("0x{}", "ab".repeat(32)))
            .bind(address)
            .bind(&salt)
            .bind(wei)
            .bind(*part)
            // One block per wallet, so two wallets' credits never share a log key.
            .bind(i64::from_str_radix(&address[2..9], 16).unwrap())
            .execute(pool)
            .await
            .unwrap();
    }
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
    let salt = goal_credit(&pool, &earner.address, "5000", &["700", "500"]).await;

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

/// The `/shop-api` mount binds its own route: a signature over another path
/// that merely ends in `/credits` does not verify through the proxy header.
#[tokio::test]
async fn the_canonical_mount_binds_its_own_route() {
    let Some(pool) = crate::db::scratch::isolated_pool(4).await else {
        return;
    };
    let admin = scratch_wallet();
    let shop = axum::Router::new().nest(
        "/shop-api",
        super::shop_router(reports_state(pool, &admin.address)),
    );

    for path in ["/shop-api/admin/credits", "/shop-api/admin/credit-reasons"] {
        let headers = signed_headers(&admin, "get", path).await;
        let (status, _, v) = send(&shop, "GET", path, headers, None).await;
        assert_eq!(status, StatusCode::OK, "{path}: {v}");
    }

    for (path, foreign) in [
        ("/shop-api/admin/credits", "/elsewhere/credits"),
        (
            "/shop-api/admin/credit-reasons",
            "/elsewhere/credit-reasons",
        ),
    ] {
        let mut headers = signed_headers(&admin, "get", foreign).await;
        headers.insert("x-original-path", foreign.parse().unwrap());
        let (status, _, _) = send(&shop, "GET", path, headers, None).await;
        assert_eq!(
            status,
            StatusCode::UNAUTHORIZED,
            "{path} must not accept a signature over {foreign}"
        );
    }
}

/// A grant made with a wallet signature (its audit actor is an address).
async fn wallet_grant(pool: &sqlx::PgPool, to: &str, granter: &str, credits: &str, reason: &str) {
    sqlx::query("WITH a AS (INSERT INTO ops.admin_audit (actor, domain, action, target, detail) VALUES ($1, 'credits', 'grant', $2, jsonb_build_object('reason', $4::text)) RETURNING id) INSERT INTO credits.ledger (address, kind, amount, bucket, admin_audit_id) SELECT $2, 'grant', $3::numeric, 'paid', a.id FROM a")
        .bind(granter)
        .bind(to)
        .bind(credits)
        .bind(reason)
        .execute(pool)
        .await
        .unwrap();
}

#[tokio::test]
async fn migrated_grants_filters_and_pages() {
    let Some(pool) = crate::db::scratch::isolated_pool(4).await else {
        return;
    };
    let admin = scratch_wallet();
    let app = app(reports_state(pool.clone(), &admin.address));
    let (a, b, c) = (scratch_wallet(), scratch_wallet(), scratch_wallet());
    let granter = "0x00000000000000000000000000000000000000aa";
    wallet_grant(&pool, &a.address, granter, "10", "alpha").await;
    wallet_grant(&pool, &b.address, granter, "20", "beta").await;
    wallet_grant(&pool, &c.address, "token:SOMETHING", "30", "gamma").await;
    // A grant the migration loaded: kind grant, no audit row.
    sqlx::query("INSERT INTO credits.ledger (address, kind, amount, bucket, tx_ref) VALUES ($1, 'grant', 5, 'paid', 'credits-server:credit:x')")
        .bind(&a.address)
        .execute(&pool)
        .await
        .unwrap();

    // The default view is the operator's grants: the migrated row is not one of them.
    let (_, _, v) = send(&app, "GET", "/credits/credits", bearer(), None).await;
    assert_eq!(v["total"], 3, "{v}");
    // It is listed under its own source, without a goal, reason or granter.
    let (_, _, v) = send(
        &app,
        "GET",
        "/credits/credits?source=migrated",
        bearer(),
        None,
    )
    .await;
    assert_eq!(v["total"], 1, "{v}");
    assert_eq!(v["data"][0]["usdCents"], 50);
    assert_eq!(v["data"][0]["reason"], Value::Null);
    assert_eq!(v["data"][0]["granterAddress"], Value::Null);

    // A wallet granter is listed and filters; a bearer grant has none.
    let (_, _, v) = send(
        &app,
        "GET",
        &format!(
            "/credits/credits?granter_address={}",
            granter.to_uppercase().replace("0X", "0x")
        ),
        bearer(),
        None,
    )
    .await;
    assert_eq!(v["total"], 2, "{v}");
    assert!(
        v["data"]
            .as_array()
            .unwrap()
            .iter()
            .all(|r| r["granterAddress"] == granter)
    );
    let (_, _, v) = send(&app, "GET", "/credits/credits?reason=gamma", bearer(), None).await;
    assert_eq!(v["total"], 1, "{v}");
    assert_eq!(v["data"][0]["userAddress"], c.address);
    assert_eq!(v["data"][0]["granterAddress"], Value::Null);

    // Ordering across pages is stable and covers every row once (amount is NULL on USD rows).
    let mut seen = Vec::new();
    for offset in 0..3 {
        let (_, _, v) = send(
            &app,
            "GET",
            &format!("/credits/credits?limit=1&offset={offset}&order_direction=ASC"),
            bearer(),
            None,
        )
        .await;
        assert_eq!(v["total"], 3, "{v}");
        seen.extend(ids(&v));
    }
    let mut unique = seen.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(unique.len(), 3, "every row on exactly one page: {seen:?}");
    let (_, _, v) = send(
        &app,
        "GET",
        "/credits/credits?order_direction=ASC",
        bearer(),
        None,
    )
    .await;
    assert_eq!(
        v["data"][0]["userAddress"], a.address,
        "uppercase ASC is ascending: {v}"
    );

    // A season filter matches goal credits only.
    let earner = scratch_wallet();
    goal_credit(&pool, &earner.address, "9", &["1"]).await;
    let (_, _, v) = send(
        &app,
        "GET",
        "/credits/credits?season_id=1&source=goal",
        bearer(),
        None,
    )
    .await;
    assert_eq!(v["total"], 1, "{v}");
    let (_, _, v) = send(
        &app,
        "GET",
        "/credits/credits?season_id=2&source=goal",
        bearer(),
        None,
    )
    .await;
    assert_eq!(v["total"], 0, "{v}");
    // A completed goal that was never claimed is not a delivered credit.
    let unclaimed = scratch_wallet();
    goal_credit(&pool, &unclaimed.address, "9", &[]).await;
    sqlx::query("UPDATE credits.goal_grants SET claimed_at = NULL WHERE address = $1")
        .bind(&unclaimed.address)
        .execute(&pool)
        .await
        .unwrap();
    let (_, _, v) = send(
        &app,
        "GET",
        "/credits/credits?season_id=1&source=goal",
        bearer(),
        None,
    )
    .await;
    assert_eq!(v["total"], 1, "{v}");

    // The old pagination: a non-numeric limit falls back, and the cap is 100.
    let (status, _, v) = send(&app, "GET", "/credits/credits?limit=abc", bearer(), None).await;
    assert_eq!(status, StatusCode::OK, "{v}");
    assert_eq!(v["data"].as_array().unwrap().len(), 3);

    // Reasons past the first page.
    let (_, _, v) = send(
        &app,
        "GET",
        "/credits/credit-reasons?limit=1&offset=1",
        bearer(),
        None,
    )
    .await;
    assert_eq!(v, json!({ "data": ["beta"], "total": 3 }));
}

/// An admin's routine balance call (`/credits/users/{admin}/credits`) ends
/// with `/credits`; replaying its signed headers with that path as the
/// proxy's `x-original-path` must not open the cross-wallet list.
#[tokio::test]
async fn a_replayed_balance_signature_does_not_open_the_list() {
    let Some(pool) = crate::db::scratch::isolated_pool(4).await else {
        return;
    };
    let admin = scratch_wallet();
    let app = app(reports_state(pool, &admin.address));

    let balance = format!("/credits/users/{}/credits", admin.address);
    let mut headers = signed_headers(&admin, "get", &balance).await;
    headers.insert("x-original-path", balance.parse().unwrap());
    let (status, _, _) = send(&app, "GET", "/credits/credits", headers, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    // The credits host's own rewrite: signed over `/credits`, forwarded with it.
    let mut headers = signed_headers(&admin, "get", "/credits").await;
    headers.insert("x-original-path", "/credits".parse().unwrap());
    let (status, _, v) = send(&app, "GET", "/credits/credits", headers, None).await;
    assert_eq!(status, StatusCode::OK, "{v}");
}

async fn ledger_row(
    pool: &sqlx::PgPool,
    address: &str,
    kind: &str,
    bucket: &str,
    credits: &str,
    tx_ref: Option<&str>,
) {
    sqlx::query("INSERT INTO credits.ledger (address, kind, amount, bucket, tx_ref) VALUES ($1, $2, $3::numeric, $4, $5)")
        .bind(address)
        .bind(kind)
        .bind(credits)
        .bind(bucket)
        .bind(tx_ref)
        .execute(pool)
        .await
        .unwrap();
}

#[allow(
    clippy::too_many_arguments,
    reason = "one argument per seeded purchase column"
)]
async fn card_purchase(
    pool: &sqlx::PgPool,
    address: &str,
    sku: &str,
    credits: &str,
    amount: i64,
    status: &str,
    refunded: i64,
    revoked: &str,
    method: &str,
) {
    sqlx::query("INSERT INTO credits.purchases (address, sku, credits, amount_cents, currency, method, status, refunded_cents, revoked_credits) VALUES ($1, $2, $3::numeric, $4, 'usd', $8, $5, $6, $7::numeric)")
        .bind(address)
        .bind(sku)
        .bind(credits)
        .bind(amount)
        .bind(status)
        .bind(refunded)
        .bind(revoked)
        .bind(method)
        .execute(pool)
        .await
        .unwrap();
}

/// A credit this node signed: `rate` USD per MANA at `decimals`, created
/// `age_mins` ago.
#[allow(
    clippy::too_many_arguments,
    reason = "one argument per seeded authorization column"
)]
async fn authorization(
    pool: &sqlx::PgPool,
    salt: &str,
    address: &str,
    usd_cents: i64,
    cap_wei: &str,
    status: &str,
    rate: Option<&str>,
    decimals: Option<i16>,
    age_mins: i32,
) {
    sqlx::query("INSERT INTO credits.authorizations (id, address, usd_cents, amount_wei, status, expires_at, oracle_rate, oracle_decimals, trade_id, created_at) VALUES ($1, $2, $3, $4::numeric, $5, timezone('UTC', now()) + interval '1 day', $6::numeric, $7, 'trade-' || $3::text, timezone('UTC', now()) - make_interval(mins => $8))")
        .bind(salt)
        .bind(address)
        .bind(usd_cents)
        .bind(cap_wei)
        .bind(status)
        .bind(rate)
        .bind(decimals)
        .bind(age_mins)
        .execute(pool)
        .await
        .unwrap();
}

/// One CreditUsed log of `salt` in `tx`, and the debit it settled (none when
/// `debited` is `None`).
#[allow(
    clippy::too_many_arguments,
    reason = "one argument per seeded log and debit column"
)]
async fn used(
    pool: &sqlx::PgPool,
    tx: &str,
    block: i64,
    log: i32,
    salt: &str,
    sender: &str,
    wei: &str,
    debited: Option<&str>,
) {
    sqlx::query("INSERT INTO credits.usages (chain_id, block_number, log_index, contract, tx_hash, block_time, sender, credit_id, salt, credit_value, credit_expires_at, value_wei) VALUES (137, $1, $2, '0x00000000000000000000000000000000000000cc', $3, timestamp '2026-11-20 10:00:00' + make_interval(secs => $1), $4, $5, $5, $6::numeric, 0, $6::numeric)")
        .bind(block)
        .bind(log)
        .bind(tx)
        .bind(sender)
        .bind(salt)
        .bind(wei)
        .execute(pool)
        .await
        .unwrap();
    if let Some(credits) = debited {
        sqlx::query("INSERT INTO credits.usage_settlements (chain_id, block_number, log_index, authorization_id, address, kind, value_wei, debited_credits, tx_ref) VALUES (137, $1, $2, $3, $4, 'purchase', $5::numeric, $6::numeric, 'credit-used:' || $3)")
            .bind(block)
            .bind(log)
            .bind(salt)
            .bind(sender)
            .bind(wei)
            .bind(credits)
            .execute(pool)
            .await
            .unwrap();
    }
}

fn hash(n: u8) -> String {
    format!("0x{}", format!("{n:02x}").repeat(32))
}

#[tokio::test]
async fn ledger_stats_add_up_the_ledger_orders_and_seasonal_credits() {
    let Some(pool) = crate::db::scratch::isolated_pool(4).await else {
        return;
    };
    let admin = scratch_wallet();
    let app = app(reports_state(pool.clone(), &admin.address));
    let (a, b) = (scratch_wallet(), scratch_wallet());

    // Issued 95 credits, reversed 11, spent 20 (one checkout split over both buckets), refunded 5.
    ledger_row(&pool, &a.address, "grant", "paid", "40", None).await;
    ledger_row(&pool, &a.address, "purchase", "paid", "50", Some("pi_1")).await;
    ledger_row(&pool, &a.address, "claim", "earned", "5", None).await;
    ledger_row(
        &pool,
        &a.address,
        "spend",
        "earned",
        "5",
        Some("checkout:1"),
    )
    .await;
    ledger_row(&pool, &a.address, "spend", "paid", "15", Some("checkout:1")).await;
    ledger_row(&pool, &a.address, "refund", "paid", "5", Some("checkout:1")).await;
    ledger_row(&pool, &a.address, "consume", "paid", "10", Some("revoke")).await;
    ledger_row(&pool, &a.address, "expire", "earned", "1", None).await;
    // Pledged: 300 + 100 cents still count, a consumed one does not.
    authorization(
        &pool,
        &hash(1),
        &a.address,
        300,
        "1",
        "authorized",
        None,
        None,
        1,
    )
    .await;
    authorization(
        &pool,
        &hash(2),
        &a.address,
        100,
        "1",
        "retired",
        None,
        None,
        1,
    )
    .await;
    authorization(
        &pool,
        &hash(3),
        &b.address,
        999,
        "1",
        "consumed",
        None,
        None,
        1,
    )
    .await;
    for (pos, cents) in [(0, 600), (1, 390)] {
        sqlx::query("INSERT INTO credits.authorization_lines (authorization_id, position, quoted_cents, charged_cents, sale_kind, asset_kind, asset_chain_id, asset_contract) VALUES ($1, $2, $3, $3, 'primary_mint', 'wearable', 137, '0x00000000000000000000000000000000000000c1')")
            .bind(hash(3))
            .bind(pos)
            .bind(cents as i64)
            .execute(&pool)
            .await
            .unwrap();
    }

    card_purchase(
        &pool,
        &a.address,
        "credits_usd_5",
        "50",
        500,
        "paid",
        100,
        "10",
        "card",
    )
    .await;
    card_purchase(
        &pool,
        &a.address,
        "credits_usd_15",
        "150",
        1500,
        "refunded",
        1500,
        "150",
        "card",
    )
    .await;
    card_purchase(
        &pool,
        &b.address,
        "credits_usd_25",
        "250",
        2500,
        "disputed",
        2500,
        "250",
        "card",
    )
    .await;
    card_purchase(
        &pool,
        &b.address,
        "credits_usd_5",
        "50",
        500,
        "pending",
        0,
        "0",
        "card",
    )
    .await;
    card_purchase(
        &pool,
        &b.address,
        "credits_usd_5",
        "50",
        500,
        "failed",
        0,
        "0",
        "card",
    )
    .await;
    card_purchase(
        &pool,
        &b.address,
        "credits_usd_5",
        "50",
        500,
        "abandoned",
        0,
        "0",
        "card",
    )
    .await;
    card_purchase(
        &pool,
        &b.address,
        "credits_usd_5",
        "50",
        500,
        "paid",
        0,
        "0",
        "mana",
    )
    .await;

    for (addr, mins) in [(&a.address, 5), (&b.address, 1)] {
        sqlx::query("INSERT INTO credits.wallet_flags (address, source, flagged_at) VALUES ($1, 'test', timezone('UTC', now()) - make_interval(mins => $2))")
            .bind(addr)
            .bind(mins)
            .execute(&pool)
            .await
            .unwrap();
    }
    let earner = scratch_wallet();
    goal_credit(&pool, &earner.address, "5000", &["700", "500"]).await;
    // Fully spent: no remaining MANA, so not counted as expiring.
    let spent = scratch_wallet();
    goal_credit(&pool, &spent.address, "100", &["100"]).await;
    // Completed but never claimed: not a delivered credit, so in none of the figures.
    let unclaimed = scratch_wallet();
    goal_credit(&pool, &unclaimed.address, "9000", &[]).await;
    sqlx::query("UPDATE credits.goal_grants SET claimed_at = NULL WHERE address = $1")
        .bind(&unclaimed.address)
        .execute(&pool)
        .await
        .unwrap();

    let (status, _, _) = send(
        &app,
        "GET",
        "/credits/admin/ledger-stats",
        HeaderMap::new(),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let headers = signed_headers(&admin, "get", "/credits/admin/ledger-stats").await;
    let (status, _, v) = send(&app, "GET", "/credits/admin/ledger-stats", headers, None).await;
    assert_eq!(status, StatusCode::OK, "{v}");

    assert_eq!(
        v["usd"],
        json!({
            "totalIssuedCents": 950, "totalReversedCents": 110, "netCreditedCents": 840,
            "totalSettledCents": 150, "totalPendingCents": 400, "outstandingLiabilityCents": 290
        })
    );
    assert_eq!(
        v["orders"]["byStatus"],
        json!([
            { "status": "abandoned", "count": 1, "grossCents": 500 },
            { "status": "credited", "count": 1, "grossCents": 500 },
            { "status": "failed", "count": 1, "grossCents": 500 },
            { "status": "processing", "count": 1, "grossCents": 500 },
            { "status": "reversed", "count": 2, "grossCents": 4000 }
        ])
    );
    let stripe = &v["analytics"]["stripe"];
    assert_eq!(stripe["grossCents"], 4500);
    assert_eq!(stripe["refundedGrossCents"], 4100);
    assert_eq!(stripe["netCents"], 400);
    assert_eq!(stripe["clawedBackCents"], 4100);
    assert_eq!(stripe["creditedCount"], 1);
    assert_eq!(stripe["reversedCount"], 2);
    assert!((stripe["refundRate"].as_f64().unwrap() - 2.0 / 3.0).abs() < 1e-9);
    assert_eq!(
        stripe["byPack"],
        json!([
            { "packId": "credits_usd_25", "paidOrders": 1, "grossCents": 2500, "netCents": 0, "credits": 0 },
            { "packId": "credits_usd_15", "paidOrders": 1, "grossCents": 1500, "netCents": 0, "credits": 0 },
            { "packId": "credits_usd_5", "paidOrders": 1, "grossCents": 500, "netCents": 400, "credits": 50 }
        ])
    );
    let trends = &v["analytics"]["usdTrends"];
    assert_eq!(trends["windowDays"], 90);
    // Summed over the days, so a run that crosses UTC midnight still adds up.
    let total = |key: &str| -> i64 {
        trends["daily"]
            .as_array()
            .unwrap()
            .iter()
            .map(|d| d[key].as_i64().unwrap())
            .sum()
    };
    assert_eq!(total("issuedUsdCents"), 950, "{trends}");
    assert_eq!(total("spentUsdCents"), 150);
    assert_eq!(total("stripeGrossCents"), 4500);
    assert_eq!(
        v["analytics"]["usdTopSpenders"],
        json!([{ "address": a.address, "spentUsdCents": 150, "purchases": 1 }])
    );
    assert_eq!(
        v["analytics"]["usdTopCollections"],
        json!([{ "collectionAddress": "0x00000000000000000000000000000000000000c1", "purchases": 2, "usdCents": 990, "primarySales": 2 }])
    );
    let flagged = &v["analytics"]["flaggedWallets"];
    assert_eq!(flagged["total"], 2);
    assert_eq!(flagged["recent"][0]["address"], b.address, "newest first");
    assert!(
        flagged["recent"][0]["flaggedAt"]
            .as_str()
            .unwrap()
            .ends_with('Z')
    );
    let legacy = &v["analytics"]["legacy"];
    assert_eq!(legacy["redemption"]["issuedManaWei"], "5100");
    assert_eq!(legacy["redemption"]["consumedManaWei"], "1300");
    assert_eq!(legacy["redemption"]["redemptionRate"], 0.2549);
    assert_eq!(legacy["redemption"]["breakageManaWei"], "0");
    assert_eq!(
        legacy["redemption"]["expiring"],
        json!([
            { "label": "≤ 7d", "withinDays": 7, "count": 0, "manaWei": "0" },
            { "label": "7–30d", "withinDays": 30, "count": 1, "manaWei": "3800" },
            { "label": "30–90d", "withinDays": 90, "count": 0, "manaWei": "0" }
        ])
    );
    assert_eq!(
        legacy["topSpenders"],
        json!([
            { "address": earner.address, "consumedManaWei": "1200", "consumptions": 2 },
            { "address": spent.address, "consumedManaWei": "100", "consumptions": 1 }
        ])
    );
    assert!(v["generatedAt"].as_i64().unwrap() > 0);
}

#[tokio::test]
async fn the_purchase_audit_reconciles_per_transaction() {
    let Some(pool) = crate::db::scratch::isolated_pool(4).await else {
        return;
    };
    let admin = scratch_wallet();
    let app = app(reports_state(pool.clone(), &admin.address));
    let (buyer, other) = (scratch_wallet(), scratch_wallet());

    // One cart, two credits at $0.25/MANA (8 decimals): $8.80 charged and fully spent,
    // $3.00 charged and $0.25 spent.
    authorization(
        &pool,
        &hash(1),
        &buyer.address,
        880,
        "40000000000000000000",
        "consumed",
        Some("25000000"),
        Some(8),
        10,
    )
    .await;
    authorization(
        &pool,
        &hash(2),
        &buyer.address,
        300,
        "20000000000000000000",
        "consumed",
        Some("25000000"),
        Some(8),
        9,
    )
    .await;
    used(
        &pool,
        &hash(0xa1),
        100,
        0,
        &hash(1),
        &buyer.address,
        "35200000000000000000",
        Some("88"),
    )
    .await;
    used(
        &pool,
        &hash(0xa1),
        100,
        1,
        &hash(2),
        &buyer.address,
        "1000000000000000000",
        Some("30"),
    )
    .await;
    // Another buyer, a rate at 18 decimals ($0.30/MANA): $1.00 charged, $0.09 spent.
    authorization(
        &pool,
        &hash(3),
        &other.address,
        100,
        "9000000000000000000",
        "consumed",
        Some("300000000000000000"),
        Some(18),
        5,
    )
    .await;
    used(
        &pool,
        &hash(0xb2),
        200,
        0,
        &hash(3),
        &other.address,
        "300000000000000000",
        Some("10"),
    )
    .await;
    // What each credit bought. A single-item credit keeps its item on the authorization
    // and has no lines (the cart's two), so it counts as one; the other credit has one line.
    sqlx::query("INSERT INTO credits.authorization_lines (authorization_id, position, quoted_cents, charged_cents, sale_kind, asset_kind, asset_chain_id, asset_contract) VALUES ($1, 0, 100, 100, 'primary_mint', 'wearable', 137, '0x00000000000000000000000000000000000000c1')")
        .bind(hash(3))
        .execute(&pool)
        .await
        .unwrap();
    // A credit nobody on this node signed is not a purchase here.
    used(
        &pool,
        &hash(0xc3),
        300,
        0,
        &hash(9),
        &other.address,
        "1",
        None,
    )
    .await;

    let path = "/credits/admin/purchase-audit";
    let (status, _, _) = send(&app, "GET", path, HeaderMap::new(), None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let headers = signed_headers(&admin, "get", path).await;
    let (status, _, v) = send(&app, "GET", path, headers, None).await;
    assert_eq!(status, StatusCode::OK, "{v}");

    let txs = v["transactions"].as_array().unwrap();
    assert_eq!(txs.len(), 2, "{v}");
    assert_eq!(txs[0]["txHash"], hash(0xb2), "newest first");
    assert_eq!(txs[1]["txHash"], hash(0xa1));
    let cart = &txs[1];
    assert_eq!(cart["buyerAddress"], buyer.address);
    assert_eq!(cart["block"], 100);
    assert_eq!(cart["settledAt"], 1_795_168_900_000i64);
    assert_eq!(cart["reservedUsdCents"], 1180);
    assert_eq!(cart["consumedUsdCents"], 905);
    assert_eq!(cart["retainedUsdCents"], 275);
    assert_eq!(cart["intents"][0]["creditId"], hash(1));
    assert_eq!(cart["intents"][0]["reservedUsdCents"], 880);
    assert_eq!(cart["intents"][0]["settledManaWei"], "35200000000000000000");
    assert_eq!(cart["intents"][0]["manaCapWei"], "40000000000000000000");
    assert_eq!(cart["intents"][0]["oracleRate"], "25000000");
    assert_eq!(cart["intents"][0]["tradeId"], "trade-880");
    assert_eq!(cart["intents"][1]["creditId"], hash(2));
    assert_eq!(txs[0]["consumedUsdCents"], 9);
    assert_eq!(txs[0]["retainedUsdCents"], 91);
    assert_eq!(
        txs[0]["intents"][0]["oracleRate"], "30000000",
        "normalised to 8 decimals"
    );
    assert_eq!(
        v["page"],
        json!({
            "limit": 50, "offset": 0, "count": 2, "total": 2, "sortBy": "settledAt", "sortDirection": "desc",
            "reservedUsdCents": 1280, "consumedUsdCents": 914, "retainedUsdCents": 366
        })
    );

    let get = async |q: &str| {
        let headers = signed_headers(&admin, "get", path).await;
        send(&app, "GET", &format!("{path}?{q}"), headers, None).await
    };
    // Every sort against the database: the cart (2 items, $11.80 charged, $9.05 spent,
    // $2.75 kept) against the single credit (1 item, $1.00, $0.09, $0.91).
    let cart_first_by_buyer = if buyer.address < other.address {
        hash(0xa1)
    } else {
        hash(0xb2)
    };
    for (q, first) in [
        ("sortBy=retained&sortDirection=asc", hash(0xb2)),
        ("sortBy=retained&sortDirection=desc", hash(0xa1)),
        ("sortBy=items&sortDirection=desc", hash(0xa1)),
        ("sortBy=items&sortDirection=asc", hash(0xb2)),
        ("sortBy=charged&sortDirection=desc", hash(0xa1)),
        ("sortBy=consumed&sortDirection=asc", hash(0xb2)),
        ("sortBy=settledAt&sortDirection=asc", hash(0xa1)),
        (
            "sortBy=buyer&sortDirection=asc",
            cart_first_by_buyer.clone(),
        ),
    ] {
        let (_, _, v) = get(q).await;
        assert_eq!(v["transactions"][0]["txHash"], first, "{q}: {v}");
        assert_eq!(v["page"]["total"], 2, "{q}");
    }
    let (_, _, v) = get("limit=1&offset=1").await;
    assert_eq!(v["page"]["count"], 1);
    assert_eq!(v["page"]["total"], 2);
    assert_eq!(v["transactions"][0]["txHash"], hash(0xa1));
    let (_, _, v) = get(&format!(
        "address={}",
        buyer.address.to_uppercase().replace("0X", "0x")
    ))
    .await;
    assert_eq!(v["page"]["total"], 1, "{v}");
    let (_, _, v) = get("limit=abc&offset=-4").await;
    assert_eq!(
        (v["page"]["limit"].clone(), v["page"]["offset"].clone()),
        (json!(50), json!(0))
    );
    let (_, _, v) = get("limit=900").await;
    assert_eq!(v["page"]["limit"], 200);

    for (q, error) in [
        ("address=0x12", "Invalid address"),
        (
            "sortBy=constructor",
            "Invalid sortBy. Valid: settledAt, buyer, items, charged, consumed, retained",
        ),
        (
            "sortDirection=ASC",
            "Invalid sortDirection. Valid: asc, desc",
        ),
    ] {
        let (status, _, v) = get(q).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{q}");
        assert_eq!(v["error"], error, "{q}");
    }
}

/// Both reports answer at the same local path under `/shop-api`, to the read
/// admins only.
#[tokio::test]
async fn the_canonical_mount_serves_the_reports() {
    let Some(pool) = crate::db::scratch::isolated_pool(4).await else {
        return;
    };
    let (admin, outsider) = (scratch_wallet(), scratch_wallet());
    let shop = axum::Router::new().nest(
        "/shop-api",
        super::shop_router(reports_state(pool, &admin.address)),
    );
    for path in [
        "/shop-api/admin/ledger-stats",
        "/shop-api/admin/purchase-audit",
    ] {
        let headers = signed_headers(&admin, "get", path).await;
        let (status, _, v) = send(&shop, "GET", path, headers, None).await;
        assert_eq!(status, StatusCode::OK, "{path}: {v}");

        // Unsigned, a wallet off the list, and the admin's signature over another path.
        let (status, _, _) = send(&shop, "GET", path, HeaderMap::new(), None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{path}");
        let headers = signed_headers(&outsider, "get", path).await;
        let (status, _, _) = send(&shop, "GET", path, headers, None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{path}");
        let mut headers = signed_headers(&admin, "get", "/elsewhere").await;
        headers.insert("x-original-path", "/elsewhere".parse().unwrap());
        let (status, _, _) = send(&shop, "GET", path, headers, None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{path}");
    }
}

/// A grant is `studio_grant` only on proof a studio paid for it: a studio
/// budget entry names its ledger row, or its audit is credits-server's own
/// record of a studio grant (an imported audit, `upstreamCreditId`, carrying
/// `studioName`). A `studioName` alone, as the admin route once wrote it on a
/// grant that spent no budget, proves nothing.
#[tokio::test]
async fn a_studio_grant_is_reported_on_proof_of_its_budget() {
    let Some(pool) = crate::db::scratch::isolated_pool(4).await else {
        return;
    };
    let admin = scratch_wallet();
    let app = app(reports_state(pool.clone(), &admin.address));
    let player = scratch_wallet();
    let operator = "0x00000000000000000000000000000000000000aa";
    let ids: Vec<i64> = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
        "WITH studio AS ( \
             INSERT INTO credits.studios (name, created_by) VALUES ('Acme ' || gen_random_uuid(), 'test') RETURNING id), \
         audits AS ( \
             INSERT INTO ops.admin_audit (actor, domain, action, target, detail) \
             SELECT v.actor, 'credits', 'grant', '{p}', v.detail::jsonb \
             FROM (VALUES \
                 (1, '{operator}', '{{\"reason\": \"operator\", \"studioName\": \"Acme\", \"studioId\": \"s\"}}'), \
                 (2, '{operator}', '{{\"reason\": \"imported studio\", \"upstreamCreditId\": \"c2\", \"studioName\": null}}'), \
                 (3, '{operator}', '{{\"reason\": \"imported grant\", \"upstreamCreditId\": \"c3\"}}'), \
                 (4, 'token:CATALYRST_CREDITS_ADMIN_TOKEN', '{{\"reason\": \"named admin\", \"studioName\": \"Acme\"}}') \
             ) v(n, actor, detail) \
             RETURNING id, detail->>'reason' AS reason), \
         audited AS ( \
             INSERT INTO credits.ledger (address, kind, amount, bucket, admin_audit_id) \
             SELECT '{p}', 'grant', 1, 'paid', a.id FROM audits a ORDER BY a.id RETURNING id, admin_audit_id), \
         unaudited AS ( \
             INSERT INTO credits.ledger (address, kind, amount, bucket, tx_ref) \
             VALUES ('{p}', 'grant', 1, 'paid', 'credits-server:credit:c5'), ('{p}', 'grant', 1, 'paid', 'credits-server:credit:c6') \
             RETURNING id, tx_ref), \
         entries AS ( \
             INSERT INTO credits.studio_budget_entries (studio_id, usd_cents, kind, ledger_id, note, created_by) \
             SELECT (SELECT id FROM studio), -10, 'grant', x.id, 'paid', 'test' FROM ( \
                 SELECT l.id FROM audited l JOIN audits a ON a.id = l.admin_audit_id WHERE a.reason = 'operator' \
                 UNION ALL SELECT id FROM unaudited WHERE tx_ref = 'credits-server:credit:c6') x) \
         SELECT l.id FROM audited l JOIN audits a ON a.id = l.admin_audit_id \
         UNION ALL SELECT id FROM unaudited ORDER BY 1",
        p = player.address,
    )))
    .fetch_all(&pool)
    .await
    .unwrap();
    // In insertion order: the operator's grant, the imported studio grant,
    // the imported Decentraland grant, the named admin grant, a migrated
    // credit and a migrated studio grant its budget entry links.
    let [
        operator_grant,
        imported_studio,
        imported_grant,
        named_admin,
        migrated,
        migrated_studio,
    ] = ids.as_slice()
    else {
        panic!("six rows: {ids:?}");
    };
    let listed = |source: &'static str| {
        let app = app.clone();
        let player = player.address.clone();
        async move {
            let (status, _, v) = send(
                &app,
                "GET",
                &format!("/credits/credits?source={source}&address={player}"),
                bearer(),
                None,
            )
            .await;
            assert_eq!(status, StatusCode::OK, "{v}");
            let mut ids: Vec<String> = v["data"]
                .as_array()
                .unwrap()
                .iter()
                .map(|r| r["id"].as_str().unwrap().to_string())
                .collect();
            ids.sort();
            ids
        }
    };
    let usd = |ids: &[&i64]| {
        let mut ids: Vec<String> = ids.iter().map(|id| format!("usd:{id}")).collect();
        ids.sort();
        ids
    };
    assert_eq!(
        listed("studio_grant").await,
        usd(&[operator_grant, imported_studio, migrated_studio]),
        "a budget entry or credits-server's studio record"
    );
    assert_eq!(
        listed("grant").await,
        usd(&[imported_grant, named_admin]),
        "an imported Decentraland grant, and a studio name with no budget behind it"
    );
    assert_eq!(listed("migrated").await, usd(&[migrated]));
}
