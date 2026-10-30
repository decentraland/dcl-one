//! The waitlist over the scratch database (shop-server@dc1a8be96
//! test/unit/controllers/handlers/{create,get,delete}-notify-request.spec.ts,
//! notify-on-sale-handler.spec.ts, test/unit/logic/notify/consumer.spec.ts,
//! test/unit/ports/email.spec.ts re-expressed through the router, the outbox
//! and a recording mailer), plus the crash and idempotency cases the local
//! outbox adds.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use axum::Router;
use axum::http::{HeaderMap, StatusCode};
use parking_lot::Mutex;
use serde_json::json;
use sqlx::PgPool;

use super::delivery::*;
use super::*;
use crate::outfits::testkit::{scratch_wallet, send, signed_headers, signed_headers_with};
use crate::social::notifications::email::SendFailure;

const BASE: &str = "https://decentraland.org/shop/";

/// to, item name, item url, unsubscribe url, outbox id.
type Sent = (String, Option<String>, String, String, i64);

#[derive(Default)]
struct Recorder {
    configured: bool,
    sent: Mutex<Vec<Sent>>,
    fail_next: Mutex<Vec<SendFailure>>,
    calls: AtomicUsize,
}

#[async_trait::async_trait]
impl AvailabilityMailer for Recorder {
    fn is_configured(&self) -> bool {
        self.configured
    }
    async fn send_item_available(
        &self,
        to: &str,
        item_name: Option<&str>,
        item_url: &str,
        unsubscribe_url: &str,
        outbox_id: i64,
    ) -> Result<Option<String>, SendFailure> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if let Some(f) = self.fail_next.lock().pop() {
            return Err(f);
        }
        self.sent.lock().push((
            to.to_string(),
            item_name.map(str::to_string),
            item_url.to_string(),
            unsubscribe_url.to_string(),
            outbox_id,
        ));
        Ok(Some(format!("msg-{outbox_id}")))
    }
}

#[test]
fn email_format_and_body_schema_follow_ajv() {
    for ok in [
        "a@b.co",
        "first.last+tag@example.org",
        "x@sub.domain.museum",
    ] {
        assert!(is_email(ok), "{ok}");
    }
    for bad in [
        "",
        "plain",
        "@no-local.com",
        "no-domain@",
        "a b@c.d",
        "a@nodot",
        "a@-bad.com",
        "a@b..c",
        ".a@b.co",
        &format!("{}@b.co", "x".repeat(65)),
    ] {
        assert!(!is_email(bad), "{bad:?}");
    }
    let req = CreateNotifyRequest {
        contract_address: "0xABCDEF0000000000000000000000000000000001".into(),
        item_id: "7".into(),
        chain_id: 137,
        email: "a@b.co".into(),
    };
    assert!(validate_create(&req).is_ok());
    assert!(
        validate_create(&CreateNotifyRequest {
            contract_address: "0x12".into(),
            ..req.clone()
        })
        .is_err()
    );
    assert!(
        validate_create(&CreateNotifyRequest {
            item_id: String::new(),
            ..req.clone()
        })
        .is_err()
    );
    assert!(
        validate_create(&CreateNotifyRequest {
            item_id: "9".repeat(79),
            ..req.clone()
        })
        .is_err()
    );
    assert!(
        validate_create(&CreateNotifyRequest {
            chain_id: 0,
            ..req.clone()
        })
        .is_err()
    );
    assert!(
        validate_create(&CreateNotifyRequest {
            email: "nope".into(),
            ..req.clone()
        })
        .is_err()
    );
    let extra: Result<CreateNotifyRequest, _> = serde_json::from_str(
        r#"{"contractAddress":"0xabcdef0000000000000000000000000000000001","itemId":"1","chainId":137,"email":"a@b.co","address":"0x1"}"#,
    );
    assert!(
        extra.is_err(),
        "additionalProperties: false (the address is never taken from the body)"
    );
}

#[test]
fn the_email_is_web2_copy_with_every_value_escaped() {
    let e = render_item_available_email(
        "<b>Hat</b> & \"cap\"",
        "https://x.test/item/0xa/1?a=1&b=2",
        Some("https://x.test/unsubscribe?contractAddress=0xa&itemId=1"),
    );
    assert_eq!(e.subject, "<b>Hat</b> & \"cap\" is available again");
    assert!(
        e.text
            .contains("View it here: https://x.test/item/0xa/1?a=1&b=2")
    );
    assert!(
        e.text
            .contains("Stop these alerts: https://x.test/unsubscribe?contractAddress=0xa&itemId=1")
    );
    assert!(
        e.html
            .contains("<strong>&lt;b&gt;Hat&lt;/b&gt; &amp; &quot;cap&quot;</strong>")
    );
    assert!(
        e.html
            .contains(r#"href="https://x.test/item/0xa/1?a=1&amp;b=2""#)
    );
    assert!(
        e.html
            .contains(r#"href="https://x.test/unsubscribe?contractAddress=0xa&amp;itemId=1""#)
    );
    assert!(!e.html.contains("<b>Hat</b>"));
    for banned in ["MANA", "wallet", "chain"] {
        assert!(
            !e.html.contains(banned) && !e.text.contains(banned),
            "{banned} is not web2 copy"
        );
    }
    let plain = render_item_available_email("Hat", "https://x.test/item/0xa/1", None);
    assert!(!plain.html.contains("Unsubscribe") && !plain.text.contains("Stop these alerts"));
    assert_eq!(escape_html("a'b"), "a&#39;b");

    assert_eq!(
        item_url(BASE, "0xa", "1"),
        "https://decentraland.org/shop/item/0xa/1"
    );
    assert_eq!(
        unsubscribe_url("https://decentraland.org/shop", "0xa", "1"),
        "https://decentraland.org/shop/unsubscribe?contractAddress=0xa&itemId=1"
    );
    assert_eq!(backoff(1), std::time::Duration::from_secs(30));
    assert_eq!(backoff(3), std::time::Duration::from_secs(120));
    assert_eq!(backoff(9), std::time::Duration::from_secs(3600));
}

struct Rig {
    pool: PgPool,
    app: Router,
    mailer: Arc<Recorder>,
    buyer: crate::outfits::testkit::Wallet,
    _serial: tokio::sync::MutexGuard<'static, ()>,
}

/// The delivery and watcher passes claim every due row in the scratch
/// database, so tests that run them must not see each other's rows.
static PASSES: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

async fn rig(configured: bool) -> Option<Rig> {
    let pool = crate::db::scratch::pool(4).await?;
    let serial = PASSES.lock().await;
    let mailer = Arc::new(Recorder {
        configured,
        ..Default::default()
    });
    let state = Arc::new(State::new(
        pool.clone(),
        mailer.clone(),
        Settings {
            shop_base_url: BASE.into(),
            writes_enabled: true,
            ..Default::default()
        },
        None,
    ));
    let app = Router::new().nest("/shop-api", shop_router(state));
    Some(Rig {
        pool,
        app,
        mailer,
        buyer: scratch_wallet(),
        _serial: serial,
    })
}

fn contract() -> String {
    format!("0x{:040x}", rand_u128())
}

fn rand_u128() -> u128 {
    uuid::Uuid::new_v4().as_u128() & ((1u128 << 120) - 1)
}

async fn subscribe(
    r: &Rig,
    wallet: &crate::outfits::testkit::Wallet,
    contract: &str,
    item: &str,
    email: &str,
) -> StatusCode {
    let h = signed_headers(wallet, "post", "/shop-api/v1/notify-requests").await;
    let (status, _, v) = send(
        &r.app,
        "POST",
        "/shop-api/v1/notify-requests",
        h,
        Some(
            json!({ "contractAddress": contract, "itemId": item, "chainId": 137, "email": email }),
        ),
    )
    .await;
    assert!(status == StatusCode::CREATED, "{status} {v}");
    status
}

async fn outbox_rows(pool: &PgPool, contract: &str) -> Vec<(String, i32)> {
    sqlx::query_as("SELECT status, attempts FROM shop.item_notify_outbox WHERE contract_address = $1 ORDER BY id").bind(contract).fetch_all(pool).await.unwrap()
}

#[tokio::test]
async fn the_signer_owns_the_subscription_and_the_wire_is_upstreams() {
    let Some(r) = rig(true).await else { return };
    let c = contract();
    let path = format!(
        "/shop-api/v1/notify-requests?contractAddress={}&itemId=7",
        c.to_uppercase()
    );

    let (status, _, v) = send(
        &r.app,
        "POST",
        "/shop-api/v1/notify-requests",
        HeaderMap::new(),
        Some(json!({})),
    )
    .await;
    assert_eq!(
        (status, v),
        (
            StatusCode::UNAUTHORIZED,
            json!({ "error": "unauthorized", "message": "This endpoint requires a signed-fetch request (ADR-44)." })
        )
    );
    let h = signed_headers_with(
        &r.buyer,
        "post",
        "/shop-api/v1/notify-requests",
        r#"{"signer":"decentraland-kernel-scene"}"#,
    )
    .await;
    let (status, _, v) = send(
        &r.app,
        "POST",
        "/shop-api/v1/notify-requests",
        h,
        Some(json!({ "contractAddress": c, "itemId": "7", "chainId": 137, "email": "a@b.co" })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "a scene-signed chain must not act as the buyer: {v}"
    );
    let h = signed_headers(&r.buyer, "post", "/shop-api/v1/notify-requests").await;
    let (status, _, v) = send(
        &r.app,
        "POST",
        "/shop-api/v1/notify-requests",
        h,
        Some(
            json!({ "contractAddress": c, "itemId": "7", "chainId": 137, "email": "not-an-email" }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{v}");

    subscribe(&r, &r.buyer, &c.to_uppercase(), "7", "buyer@example.com").await;
    let h = signed_headers(&r.buyer, "get", "/shop-api/v1/notify-requests").await;
    let (status, _, v) = send(&r.app, "GET", &path, h, None).await;
    assert_eq!(
        (status, v),
        (
            StatusCode::OK,
            json!({ "subscribed": true, "email": "buyer@example.com" })
        )
    );
    let stranger = scratch_wallet();
    let h = signed_headers(&stranger, "get", "/shop-api/v1/notify-requests").await;
    let (_, _, v) = send(&r.app, "GET", &path, h, None).await;
    assert_eq!(
        v,
        json!({ "subscribed": false }),
        "another wallet never sees the email"
    );
    let h = signed_headers(&r.buyer, "get", "/shop-api/v1/notify-requests").await;
    let (status, _, v) = send(
        &r.app,
        "GET",
        "/shop-api/v1/notify-requests?itemId=7",
        h,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{v}");
    let h = signed_headers(&r.buyer, "get", "/shop-api/v1/notify-requests").await;
    let (status, _, _) = send(
        &r.app,
        "GET",
        "/shop-api/v1/notify-requests?contractAddress=0x12&itemId=7",
        h,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // The row is stored lowercase; a re-subscribe updates the email in place (one row).
    subscribe(&r, &r.buyer, &c, "7", "new@example.com").await;
    let rows: Vec<(String, String, String)> = sqlx::query_as("SELECT address, contract_address, email FROM shop.item_notify_requests WHERE contract_address = $1").bind(&c).fetch_all(&r.pool).await.unwrap();
    assert_eq!(
        rows,
        vec![(
            r.buyer.address.clone(),
            c.clone(),
            "new@example.com".to_string()
        )]
    );

    // Opt out: one of the two is ambiguous; one item; then all (idempotent, counts).
    subscribe(&r, &r.buyer, &c, "8", "new@example.com").await;
    let h = signed_headers(&r.buyer, "delete", "/shop-api/v1/notify-requests").await;
    let (status, _, _) = send(
        &r.app,
        "DELETE",
        "/shop-api/v1/notify-requests?contractAddress=0xabc",
        h,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let h = signed_headers(&r.buyer, "delete", "/shop-api/v1/notify-requests").await;
    let (status, _, v) = send(&r.app, "DELETE", &path, h, None).await;
    assert_eq!((status, v), (StatusCode::OK, json!({ "unsubscribed": 1 })));
    let h = signed_headers(&r.buyer, "get", "/shop-api/v1/notify-requests").await;
    let (_, _, v) = send(&r.app, "GET", &path, h, None).await;
    assert_eq!(
        v,
        json!({ "subscribed": false }),
        "an unsubscribed row reads as not subscribed"
    );
    let h = signed_headers(&stranger, "delete", "/shop-api/v1/notify-requests").await;
    let (_, _, v) = send(&r.app, "DELETE", "/shop-api/v1/notify-requests", h, None).await;
    assert_eq!(
        v,
        json!({ "unsubscribed": 0 }),
        "a stranger cancels nothing of the buyer's"
    );
    let h = signed_headers(&r.buyer, "delete", "/shop-api/v1/notify-requests").await;
    let (_, _, v) = send(&r.app, "DELETE", "/shop-api/v1/notify-requests", h, None).await;
    assert_eq!(
        v["unsubscribed"], 1,
        "the remaining item; the already-unsubscribed one stays put"
    );
    let h = signed_headers(&r.buyer, "delete", "/shop-api/v1/notify-requests").await;
    let (_, _, v) = send(&r.app, "DELETE", "/shop-api/v1/notify-requests", h, None).await;
    assert_eq!(v["unsubscribed"], 0);
}

#[tokio::test]
async fn an_on_sale_signal_enqueues_once_and_the_worker_notifies_each_subscriber_exactly_once() {
    let Some(r) = rig(true).await else { return };
    let c = contract();
    let second = scratch_wallet();
    let third = scratch_wallet();
    subscribe(&r, &r.buyer, &c, "1", "one@example.com").await;
    subscribe(&r, &second, &c, "1", "two@example.com").await;
    subscribe(&r, &third, &c, "2", "other-item@example.com").await;
    let item = OnSaleItem {
        contract_address: c.clone(),
        item_id: "1".into(),
        item_name: Some("Neon Hat".into()),
    };

    assert_eq!(
        item_on_sale(&r.pool, &item, "test").await.unwrap(),
        2,
        "one row per pending subscriber of that item"
    );
    assert_eq!(
        item_on_sale(&r.pool, &item, "test-again").await.unwrap(),
        0,
        "a duplicate trigger enqueues nothing while the rows are pending"
    );

    let summary = run_pass(&r.pool, r.mailer.as_ref(), BASE).await.unwrap();
    assert_eq!((summary.claimed, summary.sent), (2, 2), "{summary:?}");
    let sent = r.mailer.sent.lock().clone();
    let mut to: Vec<String> = sent.iter().map(|s| s.0.clone()).collect();
    to.sort();
    assert_eq!(
        to,
        vec!["one@example.com".to_string(), "two@example.com".to_string()]
    );
    assert_eq!(sent[0].1.as_deref(), Some("Neon Hat"));
    assert_eq!(
        sent[0].2,
        format!("https://decentraland.org/shop/item/{c}/1")
    );
    assert_eq!(
        sent[0].3,
        format!("https://decentraland.org/shop/unsubscribe?contractAddress={c}&itemId=1")
    );
    assert_eq!(
        outbox_rows(&r.pool, &c).await,
        vec![("sent".to_string(), 1), ("sent".to_string(), 1)]
    );
    let statuses: Vec<(String, String)> = sqlx::query_as("SELECT item_id, status FROM shop.item_notify_requests WHERE contract_address = $1 ORDER BY id").bind(&c).fetch_all(&r.pool).await.unwrap();
    assert_eq!(
        statuses,
        vec![
            ("1".into(), "notified".into()),
            ("1".into(), "notified".into()),
            ("2".into(), "pending".into())
        ]
    );
    let msg: Option<String> = sqlx::query_scalar("SELECT provider_message_id FROM shop.item_notify_outbox WHERE contract_address = $1 AND status = 'sent' LIMIT 1").bind(&c).fetch_one(&r.pool).await.unwrap();
    assert!(msg.unwrap().starts_with("msg-"));

    // Notified rows are never re-notified: a second signal finds no pending subscriber; a second pass sends nothing.
    assert_eq!(item_on_sale(&r.pool, &item, "test").await.unwrap(), 0);
    let summary = run_pass(&r.pool, r.mailer.as_ref(), BASE).await.unwrap();
    assert_eq!(summary.claimed, 0);
    assert_eq!(r.mailer.calls.load(Ordering::SeqCst), 2);

    // A re-subscribe queues again, and the next signal reaches them once more.
    subscribe(&r, &r.buyer, &c, "1", "one@example.com").await;
    assert_eq!(item_on_sale(&r.pool, &item, "test").await.unwrap(), 1);
    let summary = run_pass(&r.pool, r.mailer.as_ref(), BASE).await.unwrap();
    assert_eq!(summary.sent, 1);

    // An unsubscribe between enqueue and send skips the row without a send.
    subscribe(&r, &second, &c, "1", "two@example.com").await;
    assert_eq!(item_on_sale(&r.pool, &item, "test").await.unwrap(), 1);
    let h = signed_headers(&second, "delete", "/shop-api/v1/notify-requests").await;
    let (_, _, v) = send(&r.app, "DELETE", "/shop-api/v1/notify-requests", h, None).await;
    assert_eq!(v["unsubscribed"], 1);
    let before = r.mailer.calls.load(Ordering::SeqCst);
    let summary = run_pass(&r.pool, r.mailer.as_ref(), BASE).await.unwrap();
    assert_eq!(
        (summary.claimed, summary.skipped, summary.sent),
        (1, 1, 0),
        "{summary:?}"
    );
    assert_eq!(r.mailer.calls.load(Ordering::SeqCst), before);
}

#[tokio::test]
async fn a_send_failure_retries_under_the_lease_and_a_dead_worker_never_loses_the_row() {
    let Some(r) = rig(true).await else { return };
    let c = contract();
    subscribe(&r, &r.buyer, &c, "1", "one@example.com").await;
    let item = OnSaleItem {
        contract_address: c.clone(),
        item_id: "1".into(),
        item_name: None,
    };
    assert_eq!(item_on_sale(&r.pool, &item, "test").await.unwrap(), 1);

    // Transient: the row backs off (30 s after the first attempt) and the request stays pending.
    r.mailer
        .fail_next
        .lock()
        .push(SendFailure::Transient("sendgrid 503".into()));
    let summary = run_pass(&r.pool, r.mailer.as_ref(), BASE).await.unwrap();
    assert_eq!((summary.claimed, summary.retried), (1, 1));
    let (status, attempts, next_in): (String, i32, f64) = sqlx::query_as(
        "SELECT status, attempts, EXTRACT(EPOCH FROM next_attempt_at - timezone('UTC', now()))::float8 FROM shop.item_notify_outbox WHERE contract_address = $1",
    )
    .bind(&c)
    .fetch_one(&r.pool)
    .await
    .unwrap();
    assert_eq!((status.as_str(), attempts), ("pending", 1));
    assert!(
        (20.0..=31.0).contains(&next_in),
        "backoff 30 s, got {next_in}"
    );
    let summary = run_pass(&r.pool, r.mailer.as_ref(), BASE).await.unwrap();
    assert_eq!(summary.claimed, 0, "not due yet");

    // The crash shape: a claim without an outcome leaves the row leased, due again after LEASE, still pending.
    sqlx::query("UPDATE shop.item_notify_outbox SET next_attempt_at = timezone('UTC', now()) WHERE contract_address = $1").bind(&c).execute(&r.pool).await.unwrap();
    let claimed = claim(&r.pool, chrono::Utc::now().naive_utc(), 10)
        .await
        .unwrap();
    assert_eq!(claimed.len(), 1);
    assert_eq!(claimed[0].email, "one@example.com");
    let summary = run_pass(&r.pool, r.mailer.as_ref(), BASE).await.unwrap();
    assert_eq!(
        summary.claimed, 0,
        "leased: nobody else takes it before the lease ends"
    );
    let (status, attempts, next_in): (String, i32, f64) = sqlx::query_as(
        "SELECT status, attempts, EXTRACT(EPOCH FROM next_attempt_at - timezone('UTC', now()))::float8 FROM shop.item_notify_outbox WHERE contract_address = $1",
    )
    .bind(&c)
    .fetch_one(&r.pool)
    .await
    .unwrap();
    assert_eq!((status.as_str(), attempts), ("pending", 2));
    assert!(
        (290.0..=301.0).contains(&next_in),
        "the lease, got {next_in}"
    );
    // After the lease the row is delivered once, with the generic item name.
    sqlx::query("UPDATE shop.item_notify_outbox SET next_attempt_at = timezone('UTC', now()) WHERE contract_address = $1").bind(&c).execute(&r.pool).await.unwrap();
    let summary = run_pass(&r.pool, r.mailer.as_ref(), BASE).await.unwrap();
    assert_eq!((summary.claimed, summary.sent), (1, 1));
    assert_eq!(r.mailer.sent.lock().last().unwrap().1, None);
    assert_eq!(
        outbox_rows(&r.pool, &c).await,
        vec![("sent".to_string(), 3)]
    );

    // Permanent: failed at once. Transient past MAX_ATTEMPTS: failed.
    subscribe(&r, &r.buyer, &c, "1", "one@example.com").await;
    assert_eq!(item_on_sale(&r.pool, &item, "test").await.unwrap(), 1);
    r.mailer
        .fail_next
        .lock()
        .push(SendFailure::Permanent("sendgrid 400".into()));
    let summary = run_pass(&r.pool, r.mailer.as_ref(), BASE).await.unwrap();
    assert_eq!(summary.failed, 1);
    let claimed = Claimed {
        id: 0,
        request_id: 0,
        attempts: MAX_ATTEMPTS,
        contract_address: c.clone(),
        item_id: "1".into(),
        item_name: None,
        email: String::new(),
        request_pending: true,
    };
    assert!(matches!(
        outcome_of(&claimed, Err(SendFailure::Transient("x".into()))),
        Outcome::Failed(_)
    ));
    assert_eq!(
        outcome_of(&claimed, Err(SendFailure::NotConfigured)),
        Outcome::Skipped("email delivery not configured".into())
    );
}

#[tokio::test]
async fn without_a_sender_every_claimed_row_ends_skipped_never_notified() {
    let Some(r) = rig(false).await else { return };
    let c = contract();
    subscribe(&r, &r.buyer, &c, "1", "one@example.com").await;
    assert_eq!(
        item_on_sale(
            &r.pool,
            &OnSaleItem {
                contract_address: c.clone(),
                item_id: "1".into(),
                item_name: None
            },
            "test"
        )
        .await
        .unwrap(),
        1
    );
    let summary = run_pass(&r.pool, r.mailer.as_ref(), BASE).await.unwrap();
    assert_eq!((summary.claimed, summary.skipped, summary.sent), (1, 1, 0));
    assert_eq!(r.mailer.calls.load(Ordering::SeqCst), 0);
    let status: String = sqlx::query_scalar(
        "SELECT status FROM shop.item_notify_requests WHERE contract_address = $1",
    )
    .bind(&c)
    .fetch_one(&r.pool)
    .await
    .unwrap();
    assert_eq!(
        status, "pending",
        "a skipped delivery is not a notification"
    );
}

async fn seed_item(pool: &PgPool, contract: &str, item_id: &str, available: i64, minter: bool) {
    let urn = format!("urn:decentraland:matic:collections-v2:{contract}:{item_id}");
    sqlx::query(
        "INSERT INTO shop.items (network, contract_address, item_id, chain_id, item_type, urn, creator, rarity, total_supply, max_supply, available, price, name, category, search_is_store_minter, search_is_collection_approved, created_at, updated_at, reviewed_at, mirrored_at) \
         VALUES ('matic', $1, $2::numeric, 137, 'wearable_v2', $3, '0x00000000000000000000000000000000000000c0', 'epic', 10, 100, $4, 1000, 'Neon Hat', 'hat', $5, true, 1, 1, 1, timezone('UTC', now())) \
         ON CONFLICT (network, contract_address, item_id) DO UPDATE SET available = EXCLUDED.available, search_is_store_minter = EXCLUDED.search_is_store_minter",
    )
    .bind(contract)
    .bind(item_id)
    .bind(urn)
    .bind(available)
    .bind(minter)
    .execute(pool)
    .await
    .unwrap();
}

#[tokio::test]
async fn the_watcher_baselines_first_and_notifies_the_next_on_sale_transition() {
    let Some(r) = rig(true).await else { return };
    let c = contract();
    subscribe(&r, &r.buyer, &c, "5", "one@example.com").await;
    // Not mirrored yet: nothing observed, nothing enqueued.
    let s = watch_pass(&r.pool).await.unwrap();
    assert!(s.items >= 1);
    assert_eq!(outbox_rows(&r.pool, &c).await, vec![]);

    // First observation is the baseline, even when already on sale.
    seed_item(&r.pool, &c, "5", 3, true).await;
    watch_pass(&r.pool).await.unwrap();
    assert_eq!(
        outbox_rows(&r.pool, &c).await,
        vec![],
        "an item already on sale when observed first does not notify"
    );
    let on_sale: Option<bool> = sqlx::query_scalar("SELECT on_sale FROM shop.item_availability_watch WHERE contract_address = $1 AND item_id = '5'").bind(&c).fetch_one(&r.pool).await.unwrap();
    assert_eq!(on_sale, Some(true));

    // Sold out, then restocked: the transition enqueues the waitlist once.
    seed_item(&r.pool, &c, "5", 0, true).await;
    watch_pass(&r.pool).await.unwrap();
    assert_eq!(outbox_rows(&r.pool, &c).await, vec![]);
    seed_item(&r.pool, &c, "5", 2, true).await;
    let s = watch_pass(&r.pool).await.unwrap();
    assert!(s.transitions >= 1 && s.enqueued >= 1, "{s:?}");
    assert_eq!(
        outbox_rows(&r.pool, &c).await,
        vec![("pending".to_string(), 0)]
    );
    watch_pass(&r.pool).await.unwrap();
    assert_eq!(
        outbox_rows(&r.pool, &c).await,
        vec![("pending".to_string(), 0)],
        "no transition, no second row"
    );
    let summary = run_pass(&r.pool, r.mailer.as_ref(), BASE).await.unwrap();
    assert_eq!(summary.sent, 1);
    assert_eq!(
        r.mailer.sent.lock()[0].1.as_deref(),
        Some("Neon Hat"),
        "the mirror's name rides on the email"
    );

    // A store minter that is switched off is not on sale, whatever the stock
    // says. The watcher observes items with a pending subscriber only.
    subscribe(&r, &scratch_wallet(), &c, "5", "two@example.com").await;
    seed_item(&r.pool, &c, "5", 5, false).await;
    watch_pass(&r.pool).await.unwrap();
    let on_sale: Option<bool> = sqlx::query_scalar("SELECT on_sale FROM shop.item_availability_watch WHERE contract_address = $1 AND item_id = '5'").bind(&c).fetch_one(&r.pool).await.unwrap();
    assert_eq!(on_sale, Some(false));
    sqlx::query("DELETE FROM shop.items WHERE contract_address = $1")
        .bind(&c)
        .execute(&r.pool)
        .await
        .unwrap();
}
