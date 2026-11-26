//! The purchase receipt (`receipts.rs`): one row per grant, written in its
//! transaction; one send per claim; the recipient order; the skipped and
//! failed rows; the template; the operator's test send.

use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::Router;
use axum::http::{HeaderMap, StatusCode, header};
use serde_json::{Value, json};
use sqlx::PgPool;

use super::receipts::{self, BuyerNames, Links, Sender};
use super::tests::{
    FakeMarket, MockBroker, OwnDatabase, WEBHOOK_SECRET, app, deliver, scratch_wallet, send,
    session_order_fixture, stub_stripe, unique,
};
use super::{RawSettings, Settings, SharedState, State};
use crate::http::Outbound;
use crate::shared::notifications::SendFailure;
use crate::subscriptions::AvailabilityMailer;
use crate::subscriptions::delivery::Outcome;

const ADMIN_TOKEN: &str = "receipts-admin";

#[derive(Debug, Clone)]
struct Mail {
    to: String,
    subject: String,
    html: String,
    text: String,
    args: Value,
}

/// A mailer that records every send, or answers the queued failures first.
#[derive(Default)]
struct Recorder {
    configured: bool,
    sent: Mutex<Vec<Mail>>,
    failures: Mutex<Vec<SendFailure>>,
}

impl Recorder {
    fn configured() -> Arc<Self> {
        Arc::new(Self {
            configured: true,
            ..Self::default()
        })
    }

    fn failing(failures: Vec<SendFailure>) -> Arc<Self> {
        Arc::new(Self {
            configured: true,
            failures: Mutex::new(failures),
            ..Self::default()
        })
    }

    fn sent(&self) -> Vec<Mail> {
        self.sent.lock().unwrap().clone()
    }
}

#[async_trait::async_trait]
impl AvailabilityMailer for Recorder {
    fn is_configured(&self) -> bool {
        self.configured
    }

    async fn send_html(
        &self,
        to: &str,
        subject: &str,
        html: &str,
        text: &str,
        custom_args: &Value,
    ) -> Result<Option<String>, SendFailure> {
        {
            let mut failures = self.failures.lock().unwrap();
            if !failures.is_empty() {
                return Err(failures.remove(0));
            }
        }
        let mut sent = self.sent.lock().unwrap();
        sent.push(Mail {
            to: to.into(),
            subject: subject.into(),
            html: html.into(),
            text: text.into(),
            args: custom_args.clone(),
        });
        Ok(Some(format!("msg-{}", sent.len())))
    }
}

struct Names(Option<&'static str>, Duration);

#[async_trait::async_trait]
impl BuyerNames for Names {
    async fn claimed_name(&self, _address: &str) -> Option<String> {
        tokio::time::sleep(self.1).await;
        self.0.map(str::to_string)
    }
}

fn links() -> Links {
    Links::new(
        "https://shop.example.org/shop/",
        None,
        "https://account.example.org",
    )
}

fn sender(mailer: Arc<Recorder>) -> Sender {
    Sender {
        mailer,
        names: None,
        links: links(),
    }
}

/// A credits state that takes signed webhooks and, with `stripe_url`, reads
/// sessions there; its receipts go through `mailer`.
fn receipts_state(pool: &PgPool, stripe_url: Option<String>, mailer: Arc<Recorder>) -> SharedState {
    let mut settings = Settings::parse(RawSettings {
        writes_enabled: true,
        usage_chain_id: super::usage::CHAIN_POLYGON,
        checkout_fulfillment_mode: "secondary".into(),
        stripe_webhook_secret: Some(WEBHOOK_SECRET.into()),
        admin_token: Some(ADMIN_TOKEN.into()),
        marketplace_markup_bps: 0,
        mana_price_max_staleness_secs: 3600,
        ..RawSettings::default()
    })
    .unwrap();
    if let Some(url) = stripe_url {
        settings.stripe_secret_key = Some("sk_test_stub".into());
        settings.stripe_api_base = url;
    }
    let state: SharedState = Arc::new(State::with_sources(
        pool.clone(),
        Outbound::for_tests(),
        settings,
        Arc::new(FakeMarket::default()),
        Arc::new(MockBroker::configured()),
        None,
    ));
    assert!(state.receipts.set(Arc::new(sender(mailer))).is_ok());
    state
}

/// A settled Checkout Session event for the fixture's 500-cent order.
fn session_event(kind: &str, session: &str, pi: &str, paid: bool, email: Option<&str>) -> String {
    let mut object = json!({
        "id": session,
        "payment_status": if paid { "paid" } else { "unpaid" },
        "amount_total": 500,
        "currency": "usd",
        "payment_intent": pi,
    });
    if let Some(email) = email {
        object["customer_details"] = json!({ "email": email });
    }
    json!({ "id": unique("evt_receipt"), "type": kind, "data": { "object": object } }).to_string()
}

fn intent_succeeded(pi: &str) -> String {
    json!({ "id": unique("evt_receipt_pi"), "type": "payment_intent.succeeded", "data": { "object": { "id": pi, "amount": 500, "currency": "usd" } } }).to_string()
}

/// An order settled by `payment_intent.succeeded` (its session completed
/// unpaid first, recording the intent): the event carries no address.
async fn settle_by_intent(app: &Router, state: &SharedState, buyer: &str) -> String {
    let (_, session) = session_order_fixture(state, buyer).await;
    let pi = unique("pi_receipt");
    let unpaid = session_event("checkout.session.completed", &session, &pi, false, None);
    assert_eq!(deliver(app, &unpaid).await.1["detail"], "processed");
    assert_eq!(
        deliver(app, &intent_succeeded(&pi)).await.1["detail"],
        "processed"
    );
    session
}

type ReceiptRow = (String, Option<String>, i32, Option<String>, Option<String>);

async fn receipt(pool: &PgPool, session: &str) -> Option<ReceiptRow> {
    sqlx::query_as(
        "SELECT r.status, r.buyer_email, r.attempts, r.last_error, r.provider_message_id \
         FROM credits.purchase_receipts r JOIN credits.purchases p ON p.id = r.purchase_id \
         WHERE p.stripe_checkout_session = $1",
    )
    .bind(session)
    .fetch_optional(pool)
    .await
    .unwrap()
}

async fn receipts_count(pool: &PgPool) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM credits.purchase_receipts")
        .fetch_one(pool)
        .await
        .unwrap()
}

/// Makes every pending receipt due now (a lease or a backoff elapsed).
async fn make_due(pool: &PgPool) {
    sqlx::query(
        "UPDATE credits.purchase_receipts SET next_attempt_at = timezone('UTC', now()) - interval '1 second' \
         WHERE status = 'pending'",
    )
    .execute(pool)
    .await
    .unwrap();
}

async fn pass(
    pool: &PgPool,
    sender: &Sender,
    state: &SharedState,
) -> crate::subscriptions::delivery::PassSummary {
    receipts::run_pass(pool, sender, state.stripe.as_ref())
        .await
        .unwrap()
}

/// A paid session writes one receipt with the address the buyer typed; the
/// redelivered event, the purchase's other settled events and a second
/// enqueue add none; one pass sends it once and the next sends nothing.
#[tokio::test]
async fn a_grant_enqueues_one_receipt_and_one_pass_sends_it() {
    let Some(db) = OwnDatabase::create("receipt_once").await else {
        return;
    };
    let pool = db.pool.clone();
    let (stripe_url, _, reads) = stub_stripe(
        StatusCode::OK,
        json!({ "status": "complete", "payment_status": "paid", "customer_details": { "email": "session@example.org" } }),
    )
    .await;
    let mailer = Recorder::configured();
    let state = receipts_state(&pool, Some(stripe_url), mailer.clone());
    let app = app(state.clone());
    let buyer = scratch_wallet().address;
    let (_, session) = session_order_fixture(&state, &buyer).await;
    let pi = unique("pi_receipt");
    let completed = session_event(
        "checkout.session.completed",
        &session,
        &pi,
        true,
        Some(" buyer@example.org "),
    );
    assert_eq!(deliver(&app, &completed).await.1["detail"], "processed");
    assert_eq!(
        deliver(&app, &completed).await.1["detail"],
        "already processed"
    );
    assert_eq!(
        deliver(&app, &intent_succeeded(&pi)).await.1["detail"],
        "processed"
    );
    let async_paid = session_event(
        "checkout.session.async_payment_succeeded",
        &session,
        &pi,
        true,
        Some("other@example.org"),
    );
    assert_eq!(deliver(&app, &async_paid).await.1["detail"], "processed");
    assert_eq!(receipts_count(&pool).await, 1);
    assert_eq!(
        receipt(&pool, &session).await,
        Some((
            "pending".into(),
            Some("buyer@example.org".into()),
            0,
            None,
            None
        ))
    );
    let purchase_id: i64 =
        sqlx::query_scalar("SELECT id FROM credits.purchases WHERE stripe_checkout_session = $1")
            .bind(&session)
            .fetch_one(&pool)
            .await
            .unwrap();
    let mut tx = pool.begin().await.unwrap();
    assert!(
        !receipts::enqueue_in_tx(&mut tx, purchase_id, Some("late@example.org"))
            .await
            .unwrap(),
        "a purchase has one receipt"
    );
    tx.commit().await.unwrap();

    let sender = state.receipts.get().unwrap();
    let summary = pass(&pool, sender, &state).await;
    assert_eq!((summary.claimed, summary.sent), (1, 1));
    let sent = mailer.sent();
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].to, "buyer@example.org");
    assert_eq!(sent[0].subject, "Your purchase was successful");
    assert!(sent[0].text.contains("50 CREDITS"), "{}", sent[0].text);
    assert!(
        sent[0]
            .html
            .contains("https://shop.example.org/shop/email/logo.png")
    );
    assert_eq!(
        sent[0].args,
        json!({ "credits_purchase_id": purchase_id.to_string() })
    );
    assert_eq!(
        reads.load(Ordering::SeqCst),
        0,
        "the event's address needs no Stripe read"
    );
    assert_eq!(
        receipt(&pool, &session).await,
        Some(("sent".into(), None, 1, None, Some("msg-1".into())))
    );

    make_due(&pool).await;
    let again = pass(&pool, sender, &state).await;
    assert_eq!(again.claimed, 0);
    assert_eq!(mailer.sent().len(), 1, "sent once");
}

/// The receipt is part of the grant's transaction: a grant whose commit
/// fails leaves no receipt, and the redelivery that grants writes one.
#[tokio::test]
async fn a_receipt_commits_with_its_grant() {
    let Some(db) = OwnDatabase::create("receipt_tx").await else {
        return;
    };
    let pool = db.pool.clone();
    let state = receipts_state(&pool, None, Recorder::configured());
    let app = app(state.clone());
    sqlx::raw_sql(
        "CREATE FUNCTION public.refuse_at_commit() RETURNS trigger LANGUAGE plpgsql AS \
         $$ BEGIN RAISE EXCEPTION 'refused at commit'; END $$; \
         CREATE CONSTRAINT TRIGGER refuse_grant AFTER INSERT ON credits.ledger \
         DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION public.refuse_at_commit();",
    )
    .execute(&pool)
    .await
    .unwrap();
    let buyer = scratch_wallet().address;
    let (_, session) = session_order_fixture(&state, &buyer).await;
    let completed = session_event(
        "checkout.session.completed",
        &session,
        &unique("pi_receipt"),
        true,
        Some("buyer@example.org"),
    );
    let (status, _) = deliver(&app, &completed).await;
    assert!(status.is_server_error(), "{status}");
    assert_eq!(
        receipts_count(&pool).await,
        0,
        "no receipt without its grant"
    );
    let purchase_status: String = sqlx::query_scalar(
        "SELECT status FROM credits.purchases WHERE stripe_checkout_session = $1",
    )
    .bind(&session)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(purchase_status, "pending");

    sqlx::query("DROP TRIGGER refuse_grant ON credits.ledger")
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(deliver(&app, &completed).await.1["detail"], "processed");
    assert_eq!(
        receipt(&pool, &session).await.map(|r| (r.0, r.1)),
        Some(("pending".into(), Some("buyer@example.org".into())))
    );
}

/// Without an address on the event the receipt goes to the Checkout
/// Session's, read back from Stripe; without one there (or when Stripe
/// cannot be read) to the account's email; with none, nowhere.
#[tokio::test]
async fn the_receipt_goes_to_the_session_then_the_account() {
    let Some(db) = OwnDatabase::create("receipt_to").await else {
        return;
    };
    let pool = db.pool.clone();
    let mailer = Recorder::configured();
    let buyer = scratch_wallet().address;

    let (typed, _, reads) = stub_stripe(
        StatusCode::OK,
        json!({ "status": "complete", "payment_status": "paid", "customer_details": { "email": "typed@example.org" } }),
    )
    .await;
    let state = receipts_state(&pool, Some(typed), mailer.clone());
    let session = settle_by_intent(&app(state.clone()), &state, &buyer).await;
    assert_eq!(receipt(&pool, &session).await.unwrap().1, None);
    let summary = pass(&pool, state.receipts.get().unwrap(), &state).await;
    assert_eq!(summary.sent, 1);
    assert_eq!(mailer.sent().last().unwrap().to, "typed@example.org");
    assert_eq!(reads.load(Ordering::SeqCst), 1);

    sqlx::query(
        "INSERT INTO social.subscriptions (address, email) VALUES ($1, ' account@example.org ')",
    )
    .bind(&buyer)
    .execute(&pool)
    .await
    .unwrap();
    let (blank, _, _) = stub_stripe(
        StatusCode::OK,
        json!({ "status": "complete", "payment_status": "paid", "customer_details": null }),
    )
    .await;
    let state = receipts_state(&pool, Some(blank), mailer.clone());
    settle_by_intent(&app(state.clone()), &state, &buyer).await;
    assert_eq!(
        pass(&pool, state.receipts.get().unwrap(), &state)
            .await
            .sent,
        1
    );
    assert_eq!(mailer.sent().last().unwrap().to, "account@example.org");

    let state = receipts_state(&pool, Some("http://127.0.0.1:9".into()), mailer.clone());
    settle_by_intent(&app(state.clone()), &state, &buyer).await;
    assert_eq!(
        pass(&pool, state.receipts.get().unwrap(), &state)
            .await
            .sent,
        1
    );
    assert_eq!(
        mailer.sent().last().unwrap().to,
        "account@example.org",
        "a Stripe read that fails falls through to the account"
    );

    sqlx::query("UPDATE social.subscriptions SET email = NULL WHERE address = $1")
        .bind(&buyer)
        .execute(&pool)
        .await
        .unwrap();
    let state = receipts_state(&pool, None, mailer.clone());
    let session = settle_by_intent(&app(state.clone()), &state, &buyer).await;
    let summary = pass(&pool, state.receipts.get().unwrap(), &state).await;
    assert_eq!((summary.sent, summary.skipped), (0, 1));
    assert_eq!(
        receipt(&pool, &session).await,
        Some(("skipped".into(), None, 1, Some("no recipient".into()), None))
    );
    assert_eq!(mailer.sent().len(), 3);
}

/// An unconfigured sender ends the row `skipped`, never a silent success; a
/// transient failure backs off and fails after five attempts; a permanent
/// one fails at once. Every terminal row drops its address.
#[tokio::test]
async fn an_unconfigured_sender_skips_and_failures_retry_then_fail() {
    let Some(db) = OwnDatabase::create("receipt_fail").await else {
        return;
    };
    let pool = db.pool.clone();
    let state = receipts_state(&pool, None, Recorder::configured());
    let app = app(state.clone());
    let buyer = scratch_wallet().address;
    let settle = |app: Router, state: SharedState, buyer: String| async move {
        let (_, session) = session_order_fixture(&state, &buyer).await;
        let paid = session_event(
            "checkout.session.completed",
            &session,
            &unique("pi_receipt"),
            true,
            Some("buyer@example.org"),
        );
        assert_eq!(deliver(&app, &paid).await.1["detail"], "processed");
        session
    };

    let unconfigured = Arc::new(Recorder::default());
    let session = settle(app.clone(), state.clone(), buyer.clone()).await;
    let summary = pass(&pool, &sender(unconfigured.clone()), &state).await;
    assert_eq!((summary.sent, summary.skipped), (0, 1));
    assert!(unconfigured.sent().is_empty());
    assert_eq!(
        receipt(&pool, &session).await,
        Some((
            "skipped".into(),
            None,
            1,
            Some("email delivery not configured".into()),
            None
        ))
    );

    let flaky = Recorder::failing(vec![
        SendFailure::Transient("sendgrid 503".into());
        receipts_max_attempts()
    ]);
    let session = settle(app.clone(), state.clone(), buyer.clone()).await;
    for attempt in 1..receipts_max_attempts() as i32 {
        let summary = pass(&pool, &sender(flaky.clone()), &state).await;
        assert_eq!(summary.retried, 1, "attempt {attempt}");
        let (status, email, attempts, error, _) = receipt(&pool, &session).await.unwrap();
        assert_eq!(
            (
                status.as_str(),
                email.as_deref(),
                attempts,
                error.as_deref()
            ),
            (
                "pending",
                Some("buyer@example.org"),
                attempt,
                Some("sendgrid 503")
            )
        );
        let waits: bool = sqlx::query_scalar(
            "SELECT next_attempt_at > timezone('UTC', now()) + interval '20 seconds' FROM credits.purchase_receipts WHERE status = 'pending'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(waits, "a transient failure backs off");
        make_due(&pool).await;
    }
    let summary = pass(&pool, &sender(flaky.clone()), &state).await;
    assert_eq!(summary.failed, 1);
    assert_eq!(
        receipt(&pool, &session).await,
        Some((
            "failed".into(),
            None,
            5,
            Some("sendgrid 503 (after 5 attempts)".into()),
            None
        ))
    );

    let refused = Recorder::failing(vec![SendFailure::Permanent("sendgrid 400".into())]);
    let session = settle(app.clone(), state.clone(), buyer.clone()).await;
    assert_eq!(pass(&pool, &sender(refused), &state).await.failed, 1);
    assert_eq!(
        receipt(&pool, &session).await,
        Some(("failed".into(), None, 1, Some("sendgrid 400".into()), None))
    );
}

fn receipts_max_attempts() -> usize {
    crate::subscriptions::delivery::MAX_ATTEMPTS as usize
}

/// A purchase refunded before its receipt went out gets none: the email says
/// its Credits are ready.
#[tokio::test]
async fn a_reversed_purchase_skips_its_receipt() {
    let Some(db) = OwnDatabase::create("receipt_rev").await else {
        return;
    };
    let pool = db.pool.clone();
    let mailer = Recorder::configured();
    let state = receipts_state(&pool, None, mailer.clone());
    let app = app(state.clone());
    let buyer = scratch_wallet().address;
    let (_, session) = session_order_fixture(&state, &buyer).await;
    let pi = unique("pi_receipt");
    let paid = session_event(
        "checkout.session.completed",
        &session,
        &pi,
        true,
        Some("buyer@example.org"),
    );
    assert_eq!(deliver(&app, &paid).await.1["detail"], "processed");
    let refund = json!({ "id": unique("evt_receipt_refund"), "type": "charge.refunded", "data": { "object": { "payment_intent": pi, "amount_refunded": 500 } } }).to_string();
    assert_eq!(deliver(&app, &refund).await.1["detail"], "processed");

    let summary = pass(&pool, state.receipts.get().unwrap(), &state).await;
    assert_eq!((summary.sent, summary.skipped), (0, 1));
    assert!(mailer.sent().is_empty());
    assert_eq!(
        receipt(&pool, &session).await.map(|r| (r.0, r.1, r.3)),
        Some((
            "skipped".into(),
            None,
            Some("the purchase is refunded before its receipt went out".into())
        ))
    );
}

/// An outcome recorded for a claim whose lease ended and whose row was
/// claimed again changes nothing: the newer claim owns the row.
#[tokio::test]
async fn a_stale_claim_records_nothing() {
    let Some(db) = OwnDatabase::create("receipt_lease").await else {
        return;
    };
    let pool = db.pool.clone();
    let state = receipts_state(&pool, None, Recorder::configured());
    let app = app(state.clone());
    let buyer = scratch_wallet().address;
    let (_, session) = session_order_fixture(&state, &buyer).await;
    let paid = session_event(
        "checkout.session.completed",
        &session,
        &unique("pi_receipt"),
        true,
        Some("buyer@example.org"),
    );
    assert_eq!(deliver(&app, &paid).await.1["detail"], "processed");

    let now = crate::shared::now_utc();
    let first = receipts::claim(&pool, now, 10).await.unwrap();
    assert_eq!(first.len(), 1);
    assert!(
        receipts::claim(&pool, now, 10).await.unwrap().is_empty(),
        "a leased row is not claimed again"
    );
    make_due(&pool).await;
    let second = receipts::claim(&pool, now, 10).await.unwrap();
    assert_eq!(second[0].attempts, 2);
    receipts::record(&pool, &first[0], now, &Outcome::Sent(Some("stale".into())))
        .await
        .unwrap();
    assert_eq!(
        receipt(&pool, &session).await.map(|r| (r.0, r.2)),
        Some(("pending".into(), 2))
    );
    receipts::record(&pool, &second[0], now, &Outcome::Sent(Some("fresh".into())))
        .await
        .unwrap();
    assert_eq!(
        receipt(&pool, &session).await,
        Some(("sent".into(), None, 2, None, Some("fresh".into())))
    );
}

/// The receipt greets by a claimed name only, escapes every interpolated
/// value, and groups the credits as the design's "100,000".
#[test]
fn the_receipt_renders_the_buyer_and_escapes_every_value() {
    let links = Links::new(
        "https://shop.example.org/shop?a=1&b=2",
        Some("https://cdn.example.org/email/"),
        "https://account.example.org/?x=\"y\"",
    );
    let named = receipts::render_purchase_receipt(Some("<b>Ana</b>"), "100000", &links);
    assert_eq!(named.subject, "Your purchase was successful");
    assert!(
        named
            .html
            .contains(r#"Hey there <span style="color:#FF2D55;">&lt;b&gt;Ana&lt;/b&gt;</span>, your purchase was successful!"#)
    );
    assert!(!named.html.contains("<b>Ana</b>"));
    assert!(
        named
            .html
            .contains(r#"href="https://shop.example.org/shop?a=1&amp;b=2""#)
    );
    assert!(
        named
            .html
            .contains(r#"href="https://account.example.org/?x=&quot;y&quot;""#)
    );
    assert!(
        named
            .html
            .contains(r#"src="https://cdn.example.org/email/logo.png""#)
    );
    assert!(named.html.contains(">100,000</td>"));
    assert!(
        named
            .html
            .contains("Your 100,000 Credits are ready to spend.")
    );
    assert!(
        named.text.starts_with(
            "Hey there <b>Ana</b>, your purchase was successful!\n\n100,000 CREDITS\n\n"
        )
    );
    assert!(
        named
            .text
            .contains("Go shopping: https://shop.example.org/shop?a=1&b=2\n")
    );

    let unnamed = receipts::render_purchase_receipt(None, "50", &links);
    assert!(
        unnamed
            .html
            .contains("Hey there, your purchase was successful!")
    );
    assert!(
        unnamed
            .text
            .starts_with("Your purchase was successful!\n\n50 CREDITS")
    );

    for (raw, shown) in [
        ("0", "0"),
        ("7", "7"),
        ("999", "999"),
        ("1000", "1,000"),
        ("1234567", "1,234,567"),
        ("500.000", "500"),
        ("1500.50", "1,500.5"),
    ] {
        assert_eq!(receipts::format_credits(raw), shown, "{raw}");
    }
    assert_eq!(
        Links::new(
            "https://shop.example.org/shop/",
            Some(" "),
            "https://a.example.org"
        )
        .assets_url,
        "https://shop.example.org/shop/email"
    );
}

/// A claimed name greets the buyer; a profile read slower than the deadline
/// sends the receipt without one.
#[tokio::test(start_paused = true)]
async fn a_slow_name_read_never_holds_the_receipt() {
    let mailer = Recorder::configured();
    let named = Sender {
        mailer: mailer.clone(),
        names: Some(Arc::new(Names(Some("Ana"), Duration::from_millis(10)))),
        links: links(),
    };
    named
        .send("buyer@example.org", "0xabc", "50", &json!({}))
        .await
        .unwrap();
    assert!(mailer.sent()[0].text.starts_with("Hey there Ana, your"));

    let slow = Sender {
        mailer: mailer.clone(),
        names: Some(Arc::new(Names(
            Some("Late"),
            receipts::PROFILE_TIMEOUT + Duration::from_secs(1),
        ))),
        links: links(),
    };
    slow.send("buyer@example.org", "0xabc", "50", &json!({}))
        .await
        .unwrap();
    assert!(
        mailer.sent()[1]
            .text
            .starts_with("Your purchase was successful!")
    );
}

/// The operator's test send renders the receipt for a wallet with no
/// purchase, writes no receipt row, audits without the recipient, and says
/// what happened to it.
#[tokio::test]
async fn the_operator_sends_a_test_receipt() {
    let Some(db) = OwnDatabase::create("receipt_admin").await else {
        return;
    };
    let pool = db.pool.clone();
    let mailer = Recorder::configured();
    let state = receipts_state(&pool, None, mailer.clone());
    let app = app(state.clone());
    let wallet = scratch_wallet().address;
    let mut bearer = HeaderMap::new();
    bearer.insert(
        header::AUTHORIZATION,
        format!("Bearer {ADMIN_TOKEN}").parse().unwrap(),
    );

    let (status, _, _) = send(
        &app,
        "POST",
        "/credits/admin/test-purchase-email",
        HeaderMap::new(),
        Some(json!({ "address": wallet, "email": "ops@example.org" })),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert!(mailer.sent().is_empty());

    let (status, _, v) = send(
        &app,
        "POST",
        "/credits/admin/test-purchase-email",
        bearer.clone(),
        Some(json!({ "address": wallet, "email": "ops@example.org" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{v}");
    assert_eq!(
        v,
        json!({ "address": wallet, "credits": 500, "status": "sent", "messageId": "msg-1" })
    );
    assert_eq!(mailer.sent()[0].to, "ops@example.org");
    assert!(mailer.sent()[0].text.contains("500 CREDITS"));

    for credits in [json!(0), json!(-5), json!(2.5), json!("500")] {
        let (status, _, v) = send(
            &app,
            "POST",
            "/credits/admin/test-purchase-email",
            bearer.clone(),
            Some(json!({ "address": wallet, "credits": credits })),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{credits}: {v}");
    }
    let (status, _, v) = send(
        &app,
        "POST",
        "/credits/admin/test-purchase-email",
        bearer.clone(),
        Some(json!({ "address": "not-a-wallet", "email": "ops@example.org" })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{v}");

    let (status, _, v) = send(
        &app,
        "POST",
        "/credits/admin/test-purchase-email",
        bearer.clone(),
        Some(json!({ "address": wallet, "credits": 25 })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{v}");
    assert_eq!(
        v,
        json!({ "address": wallet, "credits": 25, "status": "skipped", "reason": "no recipient" })
    );
    assert_eq!(mailer.sent().len(), 1);

    assert_eq!(receipts_count(&pool).await, 0);
    let audits: Vec<Value> = sqlx::query_scalar(
        "SELECT detail FROM ops.admin_audit WHERE domain = 'credits' AND action = 'test_purchase_email' AND target = $1 ORDER BY id",
    )
    .bind(&wallet)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        audits,
        vec![
            json!({ "credits": 500, "recipient": "given" }),
            json!({ "credits": 25, "recipient": "account" })
        ]
    );

    let unset: SharedState = Arc::new(State::with_sources(
        pool.clone(),
        Outbound::for_tests(),
        state.settings.clone(),
        Arc::new(FakeMarket::default()),
        Arc::new(MockBroker::configured()),
        None,
    ));
    let (status, _, v) = send(
        &super::tests::app(unset),
        "POST",
        "/credits/admin/test-purchase-email",
        bearer.clone(),
        Some(json!({ "address": wallet, "email": "ops@example.org" })),
    )
    .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{v}");

    let mut read_only = state.settings.clone();
    read_only.writes_enabled = false;
    let gated: SharedState = Arc::new(State::with_sources(
        pool.clone(),
        Outbound::for_tests(),
        read_only,
        Arc::new(FakeMarket::default()),
        Arc::new(MockBroker::configured()),
        None,
    ));
    assert!(gated.receipts.set(Arc::new(sender(mailer.clone()))).is_ok());
    let (status, _, v) = send(
        &super::tests::app(gated),
        "POST",
        "/credits/admin/test-purchase-email",
        bearer,
        Some(json!({ "address": wallet, "email": "ops@example.org" })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::SERVICE_UNAVAILABLE,
        "behind the write gate: {v}"
    );
    assert!(!v.to_string().contains("ops@example.org"));
    assert_eq!(mailer.sent().len(), 1, "a gated node sends nothing");
}

/// Signals when the name lookup starts and holds it until released: the slow
/// lookup a reversal can land during.
struct HeldName {
    entered: tokio::sync::Notify,
    release: tokio::sync::Notify,
}

#[async_trait::async_trait]
impl BuyerNames for HeldName {
    async fn claimed_name(&self, _address: &str) -> Option<String> {
        self.entered.notify_one();
        self.release.notified().await;
        None
    }
}

/// A full refund committed after the claim, while the lookups run, stops
/// the receipt: the dispatch check reads the purchase again right before the
/// provider call.
#[tokio::test]
async fn a_reversal_during_the_lookups_stops_the_send() {
    let Some(db) = OwnDatabase::create("receipt_race").await else {
        return;
    };
    let pool = db.pool.clone();
    let mailer = Recorder::configured();
    let state = receipts_state(&pool, None, mailer.clone());
    let app = app(state.clone());
    let buyer = scratch_wallet().address;
    let (_, session) = session_order_fixture(&state, &buyer).await;
    let pi = unique("pi_receipt");
    let paid = session_event(
        "checkout.session.completed",
        &session,
        &pi,
        true,
        Some("buyer@example.org"),
    );
    assert_eq!(deliver(&app, &paid).await.1["detail"], "processed");

    let held = Arc::new(HeldName {
        entered: tokio::sync::Notify::new(),
        release: tokio::sync::Notify::new(),
    });
    let sender = Arc::new(Sender {
        mailer: mailer.clone(),
        names: Some(held.clone()),
        links: links(),
    });
    let pass = tokio::spawn({
        let (pool, sender) = (pool.clone(), sender.clone());
        async move { receipts::run_pass(&pool, &sender, None).await.unwrap() }
    });
    held.entered.notified().await;
    let refund = json!({ "id": unique("evt_receipt_refund"), "type": "charge.refunded", "data": { "object": { "payment_intent": pi, "amount_refunded": 500 } } }).to_string();
    assert_eq!(deliver(&app, &refund).await.1["detail"], "processed");
    held.release.notify_one();
    let summary = pass.await.unwrap();
    assert_eq!((summary.claimed, summary.sent, summary.skipped), (1, 0, 1));
    assert!(
        mailer.sent().is_empty(),
        "no success email for a refunded purchase"
    );
    assert_eq!(
        receipt(&pool, &session).await.map(|r| (r.0, r.3)),
        Some((
            "skipped".into(),
            Some("the purchase is refunded before its receipt went out".into())
        ))
    );
}

/// Log lines written while a test runs, captured on its thread.
#[derive(Clone, Default)]
struct Logs(Arc<Mutex<Vec<u8>>>);

impl std::io::Write for Logs {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for Logs {
    type Writer = Logs;

    fn make_writer(&'a self) -> Self::Writer {
        self.clone()
    }
}

impl Logs {
    fn text(&self) -> String {
        String::from_utf8_lossy(&self.0.lock().unwrap()).into_owned()
    }
}

/// A Stripe that refuses every session read with an error naming an address.
async fn refusing_stripe() -> String {
    let app = Router::new().route(
        "/v1/checkout/sessions/{id}",
        axum::routing::get(|| async {
            (
                StatusCode::BAD_REQUEST,
                axum::Json(json!({ "error": { "type": "invalid_request_error", "code": "resource_missing", "param": "id", "message": "No such session for stripe-side@example.org" } })),
            )
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    url
}

/// A provider refusal that echoes the recipient, and a Stripe error that
/// names an address, leave no address in a log line, a stored error or a
/// response, the account email the operator never typed included.
#[tokio::test]
async fn an_echoed_recipient_reaches_no_log_row_or_response() {
    let logs = Logs::default();
    let _guard = tracing::subscriber::set_default(
        tracing_subscriber::fmt()
            .with_writer(logs.clone())
            .with_ansi(false)
            .with_max_level(tracing::Level::TRACE)
            .finish(),
    );
    let Some(db) = OwnDatabase::create("receipt_echo").await else {
        return;
    };
    let pool = db.pool.clone();
    let echo = |to: &str| {
        SendFailure::Permanent(format!(
            "sendgrid 400 Bad Request: {to} is not a valid recipient"
        ))
    };
    let mailer = Recorder::failing(vec![
        echo("account@example.org"),
        echo("account@example.org"),
        echo("given@example.org"),
    ]);
    let state = receipts_state(&pool, Some(refusing_stripe().await), mailer.clone());
    let app = app(state.clone());
    let buyer = scratch_wallet().address;
    // Bound, not inlined: sqlx logs statements, and the check below is about
    // the node's own lines.
    sqlx::query("INSERT INTO social.subscriptions (address, email) VALUES ($1, $2)")
        .bind(&buyer)
        .bind("account@example.org")
        .execute(&pool)
        .await
        .unwrap();

    let session = settle_by_intent(&app, &state, &buyer).await;
    let summary = pass(&pool, state.receipts.get().unwrap(), &state).await;
    assert_eq!(summary.failed, 1);
    assert_eq!(
        receipt(&pool, &session).await,
        Some((
            "failed".into(),
            None,
            1,
            Some("sendgrid 400 Bad Request: <address> is not a valid recipient".into()),
            None
        ))
    );

    let mut bearer = HeaderMap::new();
    bearer.insert(
        header::AUTHORIZATION,
        format!("Bearer {ADMIN_TOKEN}").parse().unwrap(),
    );
    for body in [
        json!({ "address": buyer }),
        json!({ "address": buyer, "email": "given@example.org" }),
    ] {
        let (status, _, v) = send(
            &app,
            "POST",
            "/credits/admin/test-purchase-email",
            bearer.clone(),
            Some(body),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_GATEWAY, "{v}");
        assert!(!v.to_string().contains('@'), "{v}");
        assert!(v.to_string().contains("<address>"), "{v}");
    }

    // A thread-scoped capture can miss a callsite another test thread hits at
    // the same time (the worker's failure line logs the stored error checked
    // above), so the presence check is on the Stripe read only this test
    // reaches; nothing captured may hold an address.
    let text = logs.text();
    assert!(
        text.contains("type=invalid_request_error code=resource_missing param=id"),
        "{text}"
    );
    assert!(!text.contains("@example.org"), "{text}");
}

/// The test route reads its body by hand: a missing or non-string wallet, a
/// non-string email or a body that is not an object is upstream's 400 on
/// both mounts, never a 422.
#[tokio::test]
async fn a_malformed_test_send_is_a_400_on_both_mounts() {
    let Some(db) = OwnDatabase::create("receipt_body").await else {
        return;
    };
    let pool = db.pool.clone();
    let mailer = Recorder::configured();
    let state = receipts_state(&pool, None, mailer.clone());
    let credits_mount = app(state.clone());
    let shop_mount = Router::new().nest("/shop-api", super::shop_router(state.clone()));
    let wallet = scratch_wallet().address;
    let mut bearer = HeaderMap::new();
    bearer.insert(
        header::AUTHORIZATION,
        format!("Bearer {ADMIN_TOKEN}").parse().unwrap(),
    );
    for (mount, path) in [
        (&credits_mount, "/credits/admin/test-purchase-email"),
        (&shop_mount, "/shop-api/admin/credits/test-purchase-email"),
    ] {
        for body in [
            json!({}),
            json!({ "address": 5 }),
            json!({ "address": null }),
            json!({ "address": wallet, "email": 5 }),
            json!([wallet]),
        ] {
            let (status, _, v) =
                send(mount, "POST", path, bearer.clone(), Some(body.clone())).await;
            assert_eq!(status, StatusCode::BAD_REQUEST, "{path} {body}: {v}");
        }
        let (status, _, v) = send(
            mount,
            "POST",
            path,
            bearer.clone(),
            Some(json!({ "address": wallet })),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{path}: {v}");
        assert_eq!(v["reason"], "no recipient", "{path}: {v}");
    }
    assert!(mailer.sent().is_empty());
}

#[test]
fn every_address_in_a_failure_is_redacted() {
    for (raw, shown) in [
        ("sendgrid 400: buyer@example.org", "sendgrid 400: <address>"),
        (
            "to <Buyer.Name+x@sub.example.org>, cc a@b",
            "to <<address>>, cc <address>",
        ),
        ("sendgrid 503", "sendgrid 503"),
        ("", ""),
    ] {
        assert_eq!(receipts::redact_addresses(raw), shown, "{raw}");
    }
}

#[test]
fn a_stripe_error_keeps_only_its_kind() {
    assert_eq!(
        super::stripe::stripe_error(
            r#"{"error":{"type":"invalid_request_error","code":"resource_missing","param":"customer_email","message":"No such customer: buyer@example.org"}}"#
        ),
        "type=invalid_request_error code=resource_missing param=customer_email"
    );
    for body in [
        "buyer@example.org",
        "",
        r#"{"error":{"message":"buyer@example.org"}}"#,
        r#"{"error":{"type":"buyer@example.org"}}"#,
    ] {
        assert_eq!(super::stripe::stripe_error(body), "unclassified", "{body}");
    }
}
