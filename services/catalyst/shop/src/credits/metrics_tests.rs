//! The credits families (`metrics.rs`): each counter moves once its
//! transaction commits and never for a rollback or a redelivery; each gauge
//! reads what one pass found. Every test records into its own local recorder
//! and reads the rendered exposition, as Prometheus would.

use chrono::NaiveDateTime;
use serde_json::json;
use sqlx::PgPool;

use super::authorize::{LIVE_STATUSES, NewAuthorization};
use super::metrics::{self, Snapshot};
use super::packs::PackRow;
use super::receipts;
use super::receipts_tests::{Recorder, intent_succeeded, receipts_state, sender, session_event};
use super::tests::{
    OwnDatabase, app, deliver, scratch_wallet, seed_wallet, session_order_fixture, unique,
};
use super::usage_tests::{auth_status, authorization, one_used_credit_index, salt, usage_settings};
use super::{RawSettings, Settings, SharedState, State};
use crate::http::Outbound;
use crate::shared::notifications::SendFailure;

/// A recorder with the credits families declared, and its exposition.
fn recorder() -> (
    dcl_metrics::PrometheusRecorder,
    dcl_metrics::PrometheusHandle,
) {
    // The registry's own recorder: it attaches each family's help, as serve's does.
    let recorder = dcl_metrics::Registry::new(&[metrics::FAMILIES])
        .unwrap()
        .build_recorder(dcl_metrics::PrometheusBuilder::new())
        .unwrap();
    let handle = recorder.handle();
    (recorder, handle)
}

/// The value of one rendered series (`name{labels}`), 0 when absent.
fn value(handle: &dcl_metrics::PrometheusHandle, series: &str) -> f64 {
    handle
        .render()
        .lines()
        .find_map(|line| {
            let rest = line.strip_prefix(series)?;
            rest.starts_with(' ').then(|| rest.trim().parse().unwrap())
        })
        .unwrap_or(0.0)
}

fn paid_session(session: &str, pi: &str, cents: i64, currency: &str) -> String {
    json!({ "id": unique("evt_metrics"), "type": "checkout.session.completed", "data": { "object": {
        "id": session, "payment_status": "paid", "amount_total": cents, "currency": currency, "payment_intent": pi,
    } } })
    .to_string()
}

fn refunded(pi: &str, cents: i64) -> String {
    json!({ "id": unique("evt_metrics_refund"), "type": "charge.refunded", "data": { "object": { "payment_intent": pi, "amount_refunded": cents } } }).to_string()
}

fn disputed(pi: &str) -> String {
    json!({ "id": unique("evt_metrics_dispute"), "type": "charge.dispute.funds_withdrawn", "data": { "object": { "payment_intent": pi, "status": "lost" } } }).to_string()
}

/// Every settled-payment outcome is counted once per processed event, the
/// granted cents with it; a redelivery counts nothing, and neither does a
/// grant whose transaction fails to commit. Each reversal that charged
/// credits back is counted by kind, with `shortfall` when the buyer had
/// already spent them, including one recorded before its grant.
#[tokio::test]
async fn the_webhook_counts_purchases_and_reversals_once_they_commit() {
    let Some(db) = OwnDatabase::create("metrics_webhook").await else {
        return;
    };
    let pool = db.pool.clone();
    let (recorder, handle) = recorder();
    let _local = dcl_metrics::set_default_local_recorder(&recorder);
    let state = receipts_state(&pool, None, Recorder::configured());
    let app = app(state.clone());
    let buyer = scratch_wallet().address;
    let outcome = |o: &str| format!("credits_purchases_total{{outcome=\"{o}\"}}");
    let cents = "credits_purchase_usd_cents_total";

    // Granted, redelivered, then the purchase's other settled event.
    let (_, session) = session_order_fixture(&state, &buyer).await;
    let pi = unique("pi_metrics");
    let paid = paid_session(&session, &pi, 500, "usd");
    assert_eq!(deliver(&app, &paid).await.1["detail"], "processed");
    assert_eq!(deliver(&app, &paid).await.1["detail"], "already processed");
    assert_eq!(
        deliver(&app, &intent_succeeded(&pi)).await.1["detail"],
        "processed"
    );
    assert_eq!(value(&handle, &outcome("granted")), 1.0);
    assert_eq!(value(&handle, &outcome("already_settled")), 1.0);
    assert_eq!(value(&handle, cents), 500.0);

    // The refusals: amount, currency, a failed payment, no purchase.
    let (_, short) = session_order_fixture(&state, &buyer).await;
    deliver(
        &app,
        &paid_session(&short, &unique("pi_metrics"), 400, "usd"),
    )
    .await;
    let (_, euros) = session_order_fixture(&state, &buyer).await;
    deliver(
        &app,
        &paid_session(&euros, &unique("pi_metrics"), 500, "eur"),
    )
    .await;
    let (_, failed) = session_order_fixture(&state, &buyer).await;
    let failure = json!({ "id": unique("evt_metrics_failed"), "type": "checkout.session.async_payment_failed", "data": { "object": { "id": failed } } }).to_string();
    deliver(&app, &failure).await;
    deliver(
        &app,
        &paid_session(&failed, &unique("pi_metrics"), 500, "usd"),
    )
    .await;
    deliver(
        &app,
        &paid_session(&unique("cs_unknown"), &unique("pi_metrics"), 500, "usd"),
    )
    .await;
    for o in [
        "amount_mismatch",
        "currency_mismatch",
        "payment_failed",
        "no_purchase",
    ] {
        assert_eq!(value(&handle, &outcome(o)), 1.0, "{o}");
    }
    assert_eq!(value(&handle, cents), 500.0, "only grants count cents");

    // A grant whose commit fails counts nothing; its redelivery counts once.
    sqlx::raw_sql(
        "CREATE FUNCTION public.refuse_at_commit() RETURNS trigger LANGUAGE plpgsql AS \
         $$ BEGIN RAISE EXCEPTION 'refused at commit'; END $$; \
         CREATE CONSTRAINT TRIGGER refuse_grant AFTER INSERT ON credits.ledger \
         DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION public.refuse_at_commit();",
    )
    .execute(&pool)
    .await
    .unwrap();
    let (_, rolled) = session_order_fixture(&state, &buyer).await;
    let rolled_pi = unique("pi_metrics");
    let rolled_paid = paid_session(&rolled, &rolled_pi, 500, "usd");
    assert!(deliver(&app, &rolled_paid).await.0.is_server_error());
    assert_eq!(
        value(&handle, &outcome("granted")),
        1.0,
        "a rolled-back grant counts nothing"
    );
    assert_eq!(value(&handle, cents), 500.0);
    sqlx::query("DROP TRIGGER refuse_grant ON credits.ledger")
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(deliver(&app, &rolled_paid).await.1["detail"], "processed");
    assert_eq!(value(&handle, &outcome("granted")), 2.0);
    assert_eq!(value(&handle, cents), 1000.0);

    let reversals = |kind: &str, shortfall: bool| {
        format!("credits_purchase_reversals_total{{kind=\"{kind}\",shortfall=\"{shortfall}\"}}")
    };
    // A partial refund of a granted purchase.
    assert_eq!(
        deliver(&app, &refunded(&pi, 250)).await.1["detail"],
        "processed"
    );
    assert_eq!(value(&handle, &reversals("refund", false)), 1.0);
    // A dispute after the buyer spent most of the credits: a shortfall.
    state
        .credits
        .admin_revoke_credits(&buyer, "70", Some("spent"), Some("test"), &json!({}))
        .await
        .unwrap();
    assert_eq!(
        deliver(&app, &disputed(&rolled_pi)).await.1["detail"],
        "processed"
    );
    assert_eq!(value(&handle, &reversals("dispute", true)), 1.0);
    // A refund that reached the webhook before its purchase was granted is
    // applied, and counted, with the grant.
    let (_, early) = session_order_fixture(&state, &buyer).await;
    let early_pi = unique("pi_metrics");
    let unpaid = session_event("checkout.session.completed", &early, &early_pi, false, None);
    deliver(&app, &unpaid).await;
    deliver(&app, &refunded(&early_pi, 500)).await;
    assert_eq!(
        value(&handle, &reversals("refund", false)),
        1.0,
        "no paid purchase yet"
    );
    deliver(&app, &paid_session(&early, &early_pi, 500, "usd")).await;
    assert_eq!(value(&handle, &reversals("refund", false)), 2.0);
    assert_eq!(value(&handle, &outcome("granted")), 3.0);
    // A redelivered reversal counts nothing.
    let again = refunded(&pi, 250);
    deliver(&app, &again).await;
    assert_eq!(
        value(&handle, &reversals("refund", false)),
        2.0,
        "no new reversed amount"
    );
    // A pack priced in another currency is granted, and adds no USD cents.
    let usd = value(&handle, cents);
    let euro_pack = PackRow {
        sku: "euro-pack".into(),
        title: "Euro".into(),
        credits: "50".into(),
        price_cents: 500,
        currency: "eur".into(),
        sort_order: 0,
    };
    let euro_session = unique("cs");
    state
        .credits
        .insert_pending_order(
            &unique("ord"),
            &buyer,
            &euro_pack.order(1).unwrap(),
            &euro_session,
        )
        .await
        .unwrap();
    let euro = paid_session(&euro_session, &unique("pi_metrics"), 500, "eur");
    assert_eq!(deliver(&app, &euro).await.1["detail"], "processed");
    assert_eq!(value(&handle, &outcome("granted")), 4.0);
    assert_eq!(value(&handle, cents), usd, "euro cents are not USD cents");
    db.drop().await;
}

/// A receipt counts once it reaches its terminal status; a retry counts
/// nothing.
#[tokio::test]
async fn the_receipt_job_counts_terminal_receipts() {
    let Some(db) = OwnDatabase::create("metrics_receipts").await else {
        return;
    };
    let pool = db.pool.clone();
    let (recorder, handle) = recorder();
    let _local = dcl_metrics::set_default_local_recorder(&recorder);
    let state = receipts_state(&pool, None, Recorder::configured());
    let app = app(state.clone());
    let buyer = scratch_wallet().address;
    let settle = || async {
        let (_, session) = session_order_fixture(&state, &buyer).await;
        let paid = session_event(
            "checkout.session.completed",
            &session,
            &unique("pi_metrics"),
            true,
            Some("buyer@example.org"),
        );
        assert_eq!(deliver(&app, &paid).await.1["detail"], "processed");
    };
    let status = |s: &str| format!("credits_purchase_receipts_total{{status=\"{s}\"}}");

    settle().await;
    receipts::run_pass(&pool, &sender(Recorder::configured()), None)
        .await
        .unwrap();
    assert_eq!(value(&handle, &status("sent")), 1.0);

    settle().await;
    receipts::run_pass(
        &pool,
        &sender(std::sync::Arc::new(Recorder::default())),
        None,
    )
    .await
    .unwrap();
    assert_eq!(value(&handle, &status("skipped")), 1.0);

    settle().await;
    let flaky = Recorder::failing(vec![SendFailure::Transient("sendgrid 503".into())]);
    receipts::run_pass(&pool, &sender(flaky), None)
        .await
        .unwrap();
    for s in ["sent", "skipped", "failed"] {
        assert_eq!(
            value(&handle, &status(s)),
            if s == "failed" { 0.0 } else { 1.0 },
            "a retry counts nothing: {s}"
        );
    }
    sqlx::query("UPDATE credits.purchase_receipts SET next_attempt_at = timezone('UTC', now()) - interval '1 second' WHERE status = 'pending'")
        .execute(&pool)
        .await
        .unwrap();
    let refused = Recorder::failing(vec![SendFailure::Permanent(
        "sendgrid 400: fields x".into(),
    )]);
    receipts::run_pass(&pool, &sender(refused), None)
        .await
        .unwrap();
    assert_eq!(value(&handle, &status("failed")), 1.0);
    db.drop().await;
}

/// Each pledge transition counts once: a reservation and an insert
/// (`authorized`), a cancel (`retired`), and both expiry sweeps (the
/// credits' own and the outbox tick's), which turn a retired pledge
/// `released` and a live one `expired`.
#[tokio::test]
async fn every_pledge_transition_is_counted() {
    let Some(db) = OwnDatabase::create("metrics_pledges").await else {
        return;
    };
    let pool = db.pool.clone();
    let (recorder, handle) = recorder();
    let _local = dcl_metrics::set_default_local_recorder(&recorder);
    let state = receipts_state(&pool, None, Recorder::configured());
    let wallet = scratch_wallet().address;
    seed_wallet(&pool, &wallet, "0", "100").await;
    let pledge = |o: &str| format!("credits_pledges_total{{outcome=\"{o}\"}}");
    let later = super::store::now() + chrono::Duration::minutes(10);
    let ids: Vec<String> = (0..4).map(|_| salt()).collect();
    let key = unique("idem");
    let reserved = state
        .credits
        .reserve_authorization(&new_pledge(&ids[0], &wallet, later), &key)
        .await
        .unwrap();
    assert!(!reserved.replayed);
    for id in &ids[1..] {
        state
            .credits
            .insert_authorization(&new_pledge(id, &wallet, later))
            .await
            .unwrap();
    }
    assert_eq!(value(&handle, &pledge("authorized")), 4.0);
    let replay = state
        .credits
        .reserve_authorization(&new_pledge(&ids[0], &wallet, later), &key)
        .await
        .unwrap();
    assert!(replay.replayed);
    assert_eq!(
        value(&handle, &pledge("authorized")),
        4.0,
        "a replayed reservation counts nothing"
    );

    let retired = state
        .credits
        .retire_intents(&[ids[0].to_string(), ids[1].to_string()], &wallet)
        .await
        .unwrap();
    assert_eq!(retired.retired, 2);
    assert_eq!(value(&handle, &pledge("retired")), 2.0);

    // Past their expiry, with no CreditUsed index to wait for.
    sqlx::query("UPDATE credits.authorizations SET expires_at = timezone('UTC', now()) - interval '1 minute' WHERE id = ANY($1)")
        .bind(vec![ids[0].to_string(), ids[2].to_string()])
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        state.credits.expire_stale_authorizations().await.unwrap(),
        2
    );
    assert_eq!(value(&handle, &pledge("released")), 1.0);
    assert_eq!(value(&handle, &pledge("expired")), 1.0);
    sqlx::query("UPDATE credits.authorizations SET expires_at = timezone('UTC', now()) - interval '1 minute' WHERE id = ANY($1)")
        .bind(vec![ids[1].to_string(), ids[3].to_string()])
        .execute(&pool)
        .await
        .unwrap();
    state.outbox_worker().run_once().await.unwrap();
    assert_eq!(value(&handle, &pledge("released")), 2.0);
    assert_eq!(value(&handle, &pledge("expired")), 2.0);
    assert_eq!(auth_status(&pool, &ids[3]).await, "expired");
    db.drop().await;
}

fn settlement(kind: &str, recovered: bool) -> String {
    format!("credits_settlements_total{{kind=\"{kind}\",recovered=\"{recovered}\"}}")
}

/// A used credit settles once: its settlement and its pledge's move to
/// `consumed` are counted after the commit, and a second pass counts
/// nothing. The pass also sets the index's readiness gauges.
#[tokio::test]
async fn a_settled_use_counts_once() {
    let Some(db) = OwnDatabase::create("metrics_settle").await else {
        return;
    };
    let pool = db.pool.clone();
    let (recorder, handle) = recorder();
    let _local = dcl_metrics::set_default_local_recorder(&recorder);
    let wallet = scratch_wallet().address;
    seed_wallet(&pool, &wallet, "0", "100").await;
    let pledge = salt();
    let later = super::store::now() + chrono::Duration::minutes(10);
    authorization(&pool, &pledge, &wallet, 600, "authorized", later).await;
    let index = one_used_credit_index(&pool, &wallet, &pledge).await;
    index.run_pass(None, true).await.unwrap();
    assert_eq!(value(&handle, &settlement("purchase", false)), 1.0);
    assert_eq!(
        value(&handle, "credits_pledges_total{outcome=\"consumed\"}"),
        1.0
    );
    index.run_pass(None, true).await.unwrap();
    assert_eq!(
        value(&handle, &settlement("purchase", false)),
        1.0,
        "settled once"
    );
    assert_eq!(
        Snapshot::read(&state_credits(&pool))
            .await
            .unwrap()
            .usages_unpaid,
        0,
        "a settled use is not unpaid"
    );
    assert_eq!(
        value(&handle, "credits_pledges_total{outcome=\"consumed\"}"),
        1.0
    );
    // The pass published a fresh fold: the lane is open, and the lag is the
    // age of the folded block (the stub's finalized block is ~16,000 s old).
    let now = chrono::Utc::now();
    index.record_readiness(now).await;
    assert_eq!(value(&handle, "credits_usage_index_ready"), 1.0);
    let lag = value(&handle, "credits_usage_index_lag_seconds");
    assert!(lag > 15_000.0 && lag < 17_000.0, "{lag}");
    // A day on, that fold is stale: the lane is closed.
    index
        .record_readiness(now + chrono::Duration::days(1))
        .await;
    assert_eq!(value(&handle, "credits_usage_index_ready"), 0.0);
    db.drop().await;
}

/// A use whose pledge had expired before it settled is `recovered`.
#[tokio::test]
async fn a_use_after_its_hold_ended_is_recovered() {
    let Some(db) = OwnDatabase::create("metrics_recovered").await else {
        return;
    };
    let pool = db.pool.clone();
    let (recorder, handle) = recorder();
    let _local = dcl_metrics::set_default_local_recorder(&recorder);
    let wallet = scratch_wallet().address;
    seed_wallet(&pool, &wallet, "0", "100").await;
    let pledge = salt();
    let later = super::store::now() + chrono::Duration::minutes(10);
    authorization(&pool, &pledge, &wallet, 600, "expired", later).await;
    one_used_credit_index(&pool, &wallet, &pledge)
        .await
        .run_pass(None, true)
        .await
        .unwrap();
    assert_eq!(value(&handle, &settlement("purchase", true)), 1.0);
    assert_eq!(value(&handle, &settlement("purchase", false)), 0.0);
    db.drop().await;
}

/// A use the wallet cannot cover (another live pledge holds the rest) is
/// not settled and not counted; the snapshot reads it as unpaid, the
/// double-spend gauge.
#[tokio::test]
async fn an_uncovered_use_is_unpaid() {
    let Some(db) = OwnDatabase::create("metrics_unpaid").await else {
        return;
    };
    let pool = db.pool.clone();
    let (recorder, handle) = recorder();
    let _local = dcl_metrics::set_default_local_recorder(&recorder);
    let wallet = scratch_wallet().address;
    seed_wallet(&pool, &wallet, "0", "100").await;
    let (uncovered, other) = (salt(), salt());
    let later = super::store::now() + chrono::Duration::minutes(10);
    authorization(&pool, &uncovered, &wallet, 600, "authorized", later).await;
    authorization(&pool, &other, &wallet, 500, "authorized", later).await;
    let out = one_used_credit_index(&pool, &wallet, &uncovered)
        .await
        .run_pass(None, true)
        .await
        .unwrap();
    assert_eq!(out.failed, 1, "{out:?}");
    assert_eq!(value(&handle, &settlement("purchase", false)), 0.0);
    let snapshot = Snapshot::read(&state_credits(&pool)).await.unwrap();
    assert_eq!(snapshot.usages_unpaid, 1);
    assert_eq!(snapshot.pledges_live, 2);
    snapshot.record();
    assert_eq!(value(&handle, "credits_usages_unpaid"), 1.0);
    db.drop().await;
}

fn new_pledge<'a>(
    id: &'a str,
    address: &'a str,
    expires_at: NaiveDateTime,
) -> NewAuthorization<'a> {
    NewAuthorization {
        id,
        address,
        usd_cents: 100,
        amount_wei: "1",
        trade_id: None,
        contract_address: None,
        item_id: None,
        source: None,
        expires_at,
    }
}

fn state_credits(pool: &PgPool) -> super::store::Credits {
    super::store::Credits::new(pool.clone())
}

/// The snapshot counts live pledges and their cents by the live statuses,
/// and pending or sent revocations; its gauges render what it read.
#[tokio::test]
async fn the_snapshot_reads_live_pledges_and_pending_revocations() {
    let Some(db) = OwnDatabase::create("metrics_snapshot").await else {
        return;
    };
    let pool = db.pool.clone();
    let (recorder, handle) = recorder();
    let _local = dcl_metrics::set_default_local_recorder(&recorder);
    let wallet = scratch_wallet().address;
    let later = super::store::now() + chrono::Duration::minutes(10);
    for (cents, status) in [
        (100, "authorized"),
        (250, "retired"),
        (400, "released"),
        (800, "consumed"),
    ] {
        authorization(&pool, &salt(), &wallet, cents, status, later).await;
    }
    for status in ["pending", "sent", "confirmed", "failed"] {
        sqlx::query("INSERT INTO credits.revocations (credit_hash, authorization_id, chain_id, contract, reason, status) VALUES ($1, $2, 137, $3, 'consumed', $4)")
            .bind(salt())
            .bind(salt())
            .bind(format!("0x{}", "a".repeat(40)))
            .bind(status)
            .execute(&pool)
            .await
            .unwrap();
    }
    let snapshot = Snapshot::read(&state_credits(&pool)).await.unwrap();
    assert_eq!(
        snapshot,
        Snapshot {
            pledges_live: 2,
            pledged_usd_cents: 350,
            usages_unpaid: 0,
            revocations_pending: 2
        }
    );
    snapshot.record();
    assert_eq!(value(&handle, "credits_pledges_live"), 2.0);
    assert_eq!(value(&handle, "credits_pledged_usd_cents"), 350.0);
    assert_eq!(value(&handle, "credits_revocations_pending"), 2.0);
    db.drop().await;
}

/// The families are valid for a registry, render with their help, and the
/// snapshot's live statuses are `authorize::LIVE_STATUSES`.
#[test]
fn the_families_register_and_render() {
    let (recorder, handle) = recorder();
    let _local = dcl_metrics::set_default_local_recorder(&recorder);
    metrics::revocations(1, 2, 3);
    metrics::usage_index(true, Some(12.5));
    metrics::settlement("remainder", false);
    let rendered = handle.render();
    for line in [
        "credits_revocations_total{status=\"sent\"} 1",
        "credits_revocations_total{status=\"confirmed\"} 2",
        "credits_revocations_total{status=\"failed\"} 3",
        "credits_usage_index_ready 1",
        "credits_usage_index_lag_seconds 12.5",
        "credits_settlements_total{kind=\"remainder\",recovered=\"false\"} 1",
        "# HELP credits_revocations_total",
    ] {
        assert!(rendered.contains(line), "missing {line}:\n{rendered}");
    }
    assert_eq!(
        metrics::SNAPSHOT_SQL
            .matches(&format!("status IN {LIVE_STATUSES}"))
            .count(),
        2
    );
}

/// Polls until `series` reads `want`.
async fn wait_for(handle: &dcl_metrics::PrometheusHandle, series: &str, want: f64) {
    for _ in 0..250 {
        if value(handle, series) == want {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    panic!("{series} never read {want}:\n{}", handle.render());
}

/// A job's gauges read 0 once the lease they were set under ends (here the
/// process stops its jobs), so a serve replica that no longer runs the job
/// never keeps exporting what it last saw: the settlement job's snapshot,
/// and the usage index's readiness over a fresh published fold.
#[tokio::test]
async fn a_job_clears_its_gauges_when_its_lease_ends() {
    let Some(db) = OwnDatabase::create("metrics_lease").await else {
        return;
    };
    let pool = db.pool.clone();
    let (recorder, handle) = recorder();
    let _local = dcl_metrics::set_default_local_recorder(&recorder);
    let later = super::store::now() + chrono::Duration::minutes(10);
    authorization(
        &pool,
        &salt(),
        &scratch_wallet().address,
        100,
        "authorized",
        later,
    )
    .await;
    let jobs = crate::jobs::Jobs::new().with_first_pass_now(true);
    super::settlement::spawn_settlement_job(
        &jobs,
        state_credits(&pool),
        None,
        true,
        std::time::Duration::from_secs(3600),
    );
    wait_for(&handle, "credits_pledges_live", 1.0).await;
    jobs.shutdown(std::time::Duration::from_secs(5)).await;
    wait_for(&handle, "credits_pledges_live", 0.0).await;

    let contract = "0x00000000000000000000000000000000000000aa";
    sqlx::query(
        "INSERT INTO credits.usage_index_cursor (chain_id, contract, last_block, folded_block, folded_block_time, folded_at) \
         VALUES (137, $1, 100, 100, timezone('UTC', now()) - interval '60 seconds', timezone('UTC', now()))",
    )
    .bind(contract)
    .execute(&pool)
    .await
    .unwrap();
    let mut settings = Settings::parse(RawSettings::default()).unwrap();
    settings.usage = usage_settings("http://127.0.0.1:9", 137, contract, 1);
    let state: SharedState = std::sync::Arc::new(State::with_sources(
        pool.clone(),
        Outbound::for_tests(),
        settings,
        std::sync::Arc::new(super::tests::FakeMarket::default()),
        std::sync::Arc::new(super::tests::MockBroker::configured()),
        None,
    ));
    let jobs = crate::jobs::Jobs::new().with_first_pass_now(true);
    state.spawn_jobs(&jobs);
    wait_for(&handle, "credits_usage_index_ready", 1.0).await;
    assert!(value(&handle, "credits_usage_index_lag_seconds") >= 60.0);
    jobs.shutdown(std::time::Duration::from_secs(5)).await;
    wait_for(&handle, "credits_usage_index_ready", 0.0).await;
    assert_eq!(value(&handle, "credits_usage_index_lag_seconds"), 0.0);
    db.drop().await;
}
