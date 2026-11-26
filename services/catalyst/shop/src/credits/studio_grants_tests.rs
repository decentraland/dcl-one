//! Studio grants over the operator routes (`handlers::studio_operators`,
//! `studio_grants.rs`), and the gifts they leave in a player's credit orders.

use axum::Router;
use axum::http::StatusCode;
use serde_json::{Value, json};
use sqlx::PgPool;

use super::tests::{
    OwnDatabase, Wallet, admin_bearer, app, call, create_test_studio, grant_state, scratch_wallet,
    send, signed_headers,
};

/// Adds `address` as an operator of `studio` over the admin route.
async fn add_operator(app: &Router, studio: &str, address: &str) {
    let (status, _, v) = send(
        app,
        "POST",
        &format!("/credits/admin/studios/{studio}/operators"),
        admin_bearer(),
        Some(json!({ "address": address })),
    )
    .await;
    assert!(status.is_success(), "{v}");
}

/// Changes `studio`'s budget by `credits` over the admin route.
async fn budget(app: &Router, studio: &str, credits: i64, key: &str) -> (StatusCode, Value) {
    let kind = if credits > 0 { "deposit" } else { "adjustment" };
    let (status, _, v) = send(
        app,
        "POST",
        &format!("/credits/admin/studios/{studio}/budget"),
        admin_bearer(),
        Some(json!({ "kind": kind, "credits": credits, "note": "test", "idempotencyKey": key })),
    )
    .await;
    (status, v)
}

/// A studio named `name` with `credits` of budget and `operator` acting for it.
async fn funded_studio(app: &Router, name: &str, credits: i64, operator: &Wallet) -> String {
    let id = create_test_studio(app, name).await;
    let (status, v) = budget(app, &id, credits, &format!("fund-{id}")).await;
    assert_eq!(status, StatusCode::CREATED, "{v}");
    add_operator(app, &id, &operator.address).await;
    id
}

/// `operator` grants `credits` to `recipient` out of `studio`'s budget.
async fn grant(
    app: &Router,
    operator: &Wallet,
    studio: &str,
    recipient: &str,
    credits: i64,
    key: &str,
) -> (StatusCode, Value) {
    call(
        app,
        operator,
        "post",
        &format!("/credits/operator/studios/{studio}/grants"),
        &[],
        Some(json!({
            "address": recipient,
            "credits": credits,
            "reason": "Event winner",
            "idempotencyKey": key,
        })),
    )
    .await
}

/// What a grant leaves behind: ledger rows, budget entries, audit rows and
/// gift notices, every studio together.
async fn written(pool: &PgPool) -> (i64, i64, i64, i64) {
    sqlx::query_as(
        "SELECT (SELECT count(*) FROM credits.ledger), \
                (SELECT count(*) FROM credits.studio_budget_entries WHERE kind = 'grant'), \
                (SELECT count(*) FROM ops.admin_audit WHERE action = 'grant'), \
                (SELECT count(*) FROM social.notifications WHERE type = 'credits_on_demand_granted')",
    )
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn balance_cents(pool: &PgPool, studio: &str) -> i64 {
    sqlx::query_scalar(
        "SELECT COALESCE(SUM(usd_cents), 0)::bigint FROM credits.studio_budget_entries WHERE studio_id = $1::uuid",
    )
    .bind(studio)
    .fetch_one(pool)
    .await
    .unwrap()
}

/// An operator grants out of the budget: the credit, the audit row, the
/// notice and the budget entry naming the credit land together; the operator
/// routes show the studio and the grant; the player's credit orders list it
/// as a gift from the studio.
#[tokio::test]
async fn an_operator_grants_out_of_the_studio_budget() {
    let Some(db) = OwnDatabase::create("studio_grant").await else {
        return;
    };
    let state = grant_state(db.pool.clone(), Some(5_000));
    let app = app(state.clone());
    let operator = scratch_wallet();
    let player = scratch_wallet();
    let stranger = scratch_wallet();
    let studio = funded_studio(&app, "Acme", 100, &operator).await;

    let (status, v) = call(
        &app,
        &operator,
        "get",
        "/credits/operator/studios",
        &[],
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{v}");
    assert_eq!(
        v,
        json!({
            "studios": [{
                "id": studio, "name": "Acme", "status": "active",
                "balanceCents": 1000, "grantedCents": 0, "grantCount": 0,
            }],
            "grantLimits": { "maxGrantCents": 5000 },
        })
    );
    let (status, v) = call(
        &app,
        &stranger,
        "get",
        "/credits/operator/studios",
        &[],
        None,
    )
    .await;
    assert_eq!((status, v["studios"].clone()), (StatusCode::OK, json!([])));

    let (status, v) = grant(&app, &operator, &studio, &player.address, 30, "grant-0001").await;
    assert_eq!(status, StatusCode::CREATED, "{v}");
    let credit_id = v["creditId"].as_str().unwrap().to_string();
    assert_eq!(
        v,
        json!({
            "creditId": credit_id, "studioId": studio, "address": player.address,
            "denomination": "USD", "credits": 30, "usdCents": 300,
            "balanceCents": 700, "replayed": false,
        })
    );
    let ledger_id: i64 = credit_id.parse().unwrap();
    let (address, kind, bucket, amount, actor, detail): (String, String, String, String, String, Value) =
        sqlx::query_as(
            "SELECT l.address, l.kind, l.bucket, l.amount::text, a.actor, a.detail \
             FROM credits.ledger l JOIN ops.admin_audit a ON a.id = l.admin_audit_id WHERE l.id = $1",
        )
        .bind(ledger_id)
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_eq!(
        (
            address.as_str(),
            kind.as_str(),
            bucket.as_str(),
            amount.as_str()
        ),
        (player.address.as_str(), "grant", "paid", "30")
    );
    assert_eq!(actor, operator.address, "the operator is the granter");
    assert_eq!(detail["studioId"], studio.as_str());
    assert_eq!(detail["studioName"], "Acme");
    assert_eq!(detail["reason"], "Event winner");
    let (entry_kind, cents, note, created_by): (String, i64, Option<String>, String) =
        sqlx::query_as(
            "SELECT kind, usd_cents, note, created_by FROM credits.studio_budget_entries WHERE ledger_id = $1",
        )
        .bind(ledger_id)
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_eq!(
        (
            entry_kind.as_str(),
            cents,
            note.as_deref(),
            created_by.as_str()
        ),
        (
            "grant",
            -300,
            Some("Event winner"),
            operator.address.as_str()
        )
    );
    let available: String =
        sqlx::query_scalar("SELECT available::text FROM credits.wallets WHERE address = $1")
            .bind(&player.address)
            .fetch_one(&db.pool)
            .await
            .unwrap();
    assert_eq!(available, "30");
    let title: String = sqlx::query_scalar(
        "SELECT metadata->>'title' FROM social.notifications WHERE address = $1 AND type = 'credits_on_demand_granted'",
    )
    .bind(&player.address)
    .fetch_one(&db.pool)
    .await
    .unwrap();
    assert_eq!(title, "A gift from Acme");

    let path = format!("/credits/operator/studios/{studio}");
    let (status, v) = call(&app, &operator, "get", &path, &[], None).await;
    assert_eq!(status, StatusCode::OK, "{v}");
    assert_eq!(v["studio"]["balanceCents"], 700);
    assert_eq!(v["studio"]["grantedCents"], 300);
    assert_eq!(v["studio"]["grantCount"], 1);
    assert_eq!(v["page"], json!({ "limit": 50, "offset": 0, "total": 1 }));
    assert_eq!(v["grants"][0]["creditId"], credit_id.as_str());
    assert_eq!(v["grants"][0]["recipient"], player.address.as_str());
    assert_eq!(v["grants"][0]["usdCents"], 300);
    assert_eq!(v["grants"][0]["reason"], "Event winner");
    assert_eq!(v["grants"][0]["grantedBy"], operator.address.as_str());
    assert_eq!(v["grantLimits"]["maxGrantCents"], 5000);

    // A wallet that does not act for the studio is told it does not exist,
    // for every route and even with a key that granted.
    let (status, v) = call(&app, &stranger, "get", &path, &[], None).await;
    assert_eq!(
        (status, v["code"].clone()),
        (StatusCode::NOT_FOUND, json!("STUDIO_NOT_FOUND"))
    );
    let (status, v) = grant(&app, &stranger, &studio, &player.address, 30, "grant-0001").await;
    assert_eq!(
        (status, v["code"].clone()),
        (StatusCode::NOT_FOUND, json!("STUDIO_NOT_FOUND"))
    );
    let missing = "6f9619ff-8b86-d011-b42d-00c04fc964ff";
    let (status, _) = grant(&app, &operator, missing, &player.address, 1, "grant-0002").await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    // The gifts: the studio's, an admin grant with no studio, and a migrated
    // studio grant (an entry naming an imported ledger row). A purchase, which
    // the node writes with an audit row like a grant, and a migrated row no
    // entry names are not gifts.
    state
        .credits
        .admin_grant_credits(super::admin_store::GrantRequest {
            address: &player.address,
            amount: "7",
            kind: "purchase",
            reason: Some("pack"),
            actor: None,
            idempotency_key: None,
            detail: &json!({}),
            studio_name: None,
        })
        .await
        .unwrap();
    let (status, _, v) = send(
        &app,
        "POST",
        "/credits/admin/credits/grant",
        admin_bearer(),
        Some(json!({ "address": player.address, "amount": "5", "reason": "support" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{v}");
    sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
        "INSERT INTO credits.ledger (address, kind, bucket, amount, tx_ref, created_at) \
         VALUES ('{p}', 'grant', 'paid', 2, 'credits-server:credit:old-iap', timezone('UTC', now()) - interval '3 days'), \
                ('{p}', 'grant', 'paid', 4, 'credits-server:credit:old-gift', timezone('UTC', now()) - interval '2 days'); \
         INSERT INTO credits.studio_budget_entries (studio_id, usd_cents, kind, ledger_id, note, created_by, created_at) \
         SELECT '{studio}'::uuid, -40, 'grant', id, 'Imported', '{o}', created_at FROM credits.ledger WHERE tx_ref = 'credits-server:credit:old-gift'",
        p = player.address,
        o = operator.address,
    )))
    .execute(&db.pool)
    .await
    .unwrap();
    let orders = format!("/credits/users/{}/credit-orders", player.address);
    let (status, v) = call(&app, &player, "get", &orders, &[], None).await;
    assert_eq!(status, StatusCode::OK, "{v}");
    let gifts = &v["gifts"];
    assert_eq!(gifts["total"], 3, "{gifts}");
    let listed: Vec<(i64, Value)> = gifts["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|g| (g["usdCents"].as_i64().unwrap(), g["studioName"].clone()))
        .collect();
    assert_eq!(
        listed,
        vec![(50, json!(null)), (300, json!("Acme")), (40, json!("Acme"))],
        "newest first: {gifts}"
    );
    assert_eq!(gifts["items"][1]["id"], credit_id.as_str());
    assert_eq!(gifts["items"][1]["credits"], 30);
    assert!(gifts["items"][1]["createdAt"].as_i64().unwrap() > 0);
    // The signature covers the path; the page is the query.
    let headers = signed_headers(&player, "get", &orders).await;
    let paged = format!("{orders}?giftsLimit=1&giftsOffset=1");
    let (status, _, v) = send(&app, "GET", &paged, headers, None).await;
    assert_eq!(status, StatusCode::OK, "{v}");
    assert_eq!(v["gifts"]["total"], 3);
    assert_eq!(v["gifts"]["items"].as_array().unwrap().len(), 1);
    assert_eq!(v["gifts"]["items"][0]["usdCents"], 300);
    db.drop().await;
}

/// Every refusal writes nothing, in credits-server's order: the studio's own
/// refusals come before the player's flag, so a paused or empty studio cannot
/// learn which wallets carry one.
#[tokio::test]
async fn a_refused_studio_grant_writes_nothing() {
    let Some(db) = OwnDatabase::create("studio_grant_refused").await else {
        return;
    };
    let app = app(grant_state(db.pool.clone(), Some(500)));
    let operator = scratch_wallet();
    let other_operator = scratch_wallet();
    let player = scratch_wallet();
    let flagged = scratch_wallet();
    let studio = funded_studio(&app, "Acme", 30, &operator).await;
    add_operator(&app, &studio, &other_operator.address).await;
    sqlx::query("INSERT INTO credits.wallet_flags (address, source) VALUES ($1, 'test')")
        .bind(&flagged.address)
        .execute(&db.pool)
        .await
        .unwrap();
    let before = written(&db.pool).await;
    let refused = |status: StatusCode, v: &Value, code: &str| {
        assert_eq!((status, v["code"].as_str()), (status, Some(code)), "{v}");
    };

    for (body, why) in [
        (
            json!({ "address": "nope", "credits": 1, "reason": "r", "idempotencyKey": "key-00001" }),
            "address",
        ),
        (
            json!({ "address": player.address, "credits": 0, "reason": "r", "idempotencyKey": "key-00001" }),
            "credits",
        ),
        (
            json!({ "address": player.address, "credits": 1.5, "reason": "r", "idempotencyKey": "key-00001" }),
            "credits",
        ),
        (
            json!({ "address": player.address, "credits": 1, "reason": " ", "idempotencyKey": "key-00001" }),
            "reason",
        ),
        (
            json!({ "address": player.address, "credits": 1, "reason": "r", "annotation": 1, "idempotencyKey": "key-00001" }),
            "annotation",
        ),
        (
            json!({ "address": player.address, "credits": 1, "reason": "r", "idempotencyKey": "short" }),
            "idempotencyKey",
        ),
    ] {
        let (status, v) = call(
            &app,
            &operator,
            "post",
            &format!("/credits/operator/studios/{studio}/grants"),
            &[],
            Some(body),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{why}: {v}");
        refused(status, &v, "INVALID_REQUEST");
    }
    let (status, v) = grant(
        &app,
        &operator,
        &studio,
        &other_operator.address,
        1,
        "key-00002",
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    refused(status, &v, "STUDIO_OPERATOR");
    let (status, v) = grant(&app, &operator, &studio, &player.address, 51, "key-00003").await;
    assert_eq!(status, StatusCode::CONFLICT);
    refused(status, &v, "GRANT_LIMIT_EXCEEDED");
    let (status, v) = grant(&app, &operator, &studio, &player.address, 31, "key-00004").await;
    assert_eq!(status, StatusCode::CONFLICT);
    refused(status, &v, "STUDIO_BUDGET_EXCEEDED");
    // The flag is judged last: an empty budget is told first.
    let (status, v) = grant(&app, &operator, &studio, &flagged.address, 31, "key-00005").await;
    refused(status, &v, "STUDIO_BUDGET_EXCEEDED");
    let (status, v) = grant(&app, &operator, &studio, &flagged.address, 1, "key-00006").await;
    assert_eq!(status, StatusCode::CONFLICT);
    refused(status, &v, "RECIPIENT_NOT_ELIGIBLE");
    assert_eq!(
        v["error"],
        "This account cannot receive Credits from a studio"
    );

    let (status, _, v) = send(
        &app,
        "PATCH",
        &format!("/credits/admin/studios/{studio}"),
        admin_bearer(),
        Some(json!({ "status": "paused" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{v}");
    let (status, v) = grant(&app, &operator, &studio, &player.address, 1, "key-00007").await;
    assert_eq!(status, StatusCode::CONFLICT);
    refused(status, &v, "STUDIO_PAUSED");
    let (status, v) = grant(&app, &operator, &studio, &flagged.address, 1, "key-00008").await;
    refused(status, &v, "STUDIO_PAUSED");
    // A paused studio stays visible to its operators.
    let (_, v) = call(
        &app,
        &operator,
        "get",
        "/credits/operator/studios",
        &[],
        None,
    )
    .await;
    assert_eq!(v["studios"][0]["status"], "paused");
    assert_eq!(v["studios"][0]["balanceCents"], 300);

    // No ceiling configured refuses every new grant.
    let unset = super::tests::app(grant_state(db.pool.clone(), None));
    let (status, v) = grant(&unset, &operator, &studio, &player.address, 1, "key-00009").await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    refused(status, &v, "GRANTS_NOT_CONFIGURED");
    let (_, v) = call(
        &unset,
        &operator,
        "get",
        "/credits/operator/studios",
        &[],
        None,
    )
    .await;
    assert_eq!(v["grantLimits"], json!({ "maxGrantCents": null }));

    assert_eq!(written(&db.pool).await, before, "a refusal writes nothing");
    assert_eq!(balance_cents(&db.pool, &studio).await, 300);
    db.drop().await;
}

/// A repeat of the key with the same request answers the grant it made, even
/// once the studio is paused, the player flagged and the ceiling unset; the
/// key is refused for any other request, a budget change's included.
#[tokio::test]
async fn a_studio_grant_replays_after_the_studio_changes() {
    let Some(db) = OwnDatabase::create("studio_grant_replay").await else {
        return;
    };
    let app = app(grant_state(db.pool.clone(), Some(500)));
    let operator = scratch_wallet();
    let player = scratch_wallet();
    let studio = funded_studio(&app, "Acme", 30, &operator).await;
    let (status, first) = grant(&app, &operator, &studio, &player.address, 10, "same-key-1").await;
    assert_eq!(status, StatusCode::CREATED, "{first}");
    let after_grant = written(&db.pool).await;

    // A budget change's key is not a grant's, and the reverse.
    let (status, v) = budget(&app, &studio, 5, "same-key-1").await;
    assert_eq!(
        (status, v["code"].clone()),
        (StatusCode::CONFLICT, json!("IDEMPOTENCY_KEY_REUSED"))
    );
    let (status, v) = budget(&app, &studio, 5, "budget-key-1").await;
    assert_eq!(status, StatusCode::CREATED, "{v}");
    let (status, v) = grant(&app, &operator, &studio, &player.address, 5, "budget-key-1").await;
    assert_eq!(
        (status, v["code"].clone()),
        (StatusCode::CONFLICT, json!("IDEMPOTENCY_KEY_REUSED"))
    );

    sqlx::query("INSERT INTO credits.wallet_flags (address, source) VALUES ($1, 'test')")
        .bind(&player.address)
        .execute(&db.pool)
        .await
        .unwrap();
    let (status, _, _) = send(
        &app,
        "PATCH",
        &format!("/credits/admin/studios/{studio}"),
        admin_bearer(),
        Some(json!({ "status": "paused" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let unset = super::tests::app(grant_state(db.pool.clone(), None));
    let (status, again) = grant(
        &unset,
        &operator,
        &studio,
        &player.address,
        10,
        "same-key-1",
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{again}");
    assert_eq!(again["replayed"], true);
    assert_eq!(again["creditId"], first["creditId"]);
    assert_eq!(again["balanceCents"], 250, "the balance as it stands");
    // Another request under the same key is refused.
    let (status, v) = grant(
        &unset,
        &operator,
        &studio,
        &player.address,
        11,
        "same-key-1",
    )
    .await;
    assert_eq!(
        (status, v["code"].clone()),
        (StatusCode::CONFLICT, json!("IDEMPOTENCY_KEY_REUSED"))
    );
    let (ledger, entries, audits, notices) = written(&db.pool).await;
    assert_eq!(
        (ledger, entries, audits, notices),
        after_grant,
        "a replay writes nothing and notifies no one again"
    );
    db.drop().await;
}

/// Two grants racing for the last of a budget run one after the other, even
/// when every session defaults to REPEATABLE READ: one is made, the other is
/// refused, and the balance never goes below zero.
#[tokio::test]
async fn two_grants_racing_for_the_last_budget_never_overdraw() {
    let Some(db) = OwnDatabase::create("studio_grant_race").await else {
        return;
    };
    sqlx::raw_sql(
        "DO $$ BEGIN EXECUTE format('ALTER DATABASE %I SET default_transaction_isolation = %L', \
         current_database(), 'repeatable read'); END $$",
    )
    .execute(&db.pool)
    .await
    .unwrap();
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(6)
        .connect_with((*db.pool.connect_options()).clone())
        .await
        .unwrap();
    let level: String = sqlx::query_scalar("SHOW transaction_isolation")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(level, "repeatable read");
    let app = app(grant_state(pool.clone(), Some(10_000)));
    let operator = scratch_wallet();
    let studio = funded_studio(&app, "Racers", 10, &operator).await;
    for round in 0..6 {
        let (a, b) = (scratch_wallet(), scratch_wallet());
        let (key_a, key_b) = (format!("race-a-{round:04}"), format!("race-b-{round:04}"));
        let ((sa, va), (sb, vb)) = tokio::join!(
            grant(&app, &operator, &studio, &a.address, 10, &key_a),
            grant(&app, &operator, &studio, &b.address, 10, &key_b)
        );
        let mut outcomes = [(sa, va["code"].clone()), (sb, vb["code"].clone())];
        outcomes.sort_by_key(|(s, _)| s.as_u16());
        assert_eq!(
            outcomes,
            [
                (StatusCode::CREATED, Value::Null),
                (StatusCode::CONFLICT, json!("STUDIO_BUDGET_EXCEEDED"))
            ],
            "round {round}: {va} {vb}"
        );
        assert_eq!(balance_cents(&pool, &studio).await, 0, "round {round}");
        let (status, v) = budget(&app, &studio, 10, &format!("refill-{round:04}")).await;
        assert_eq!(status, StatusCode::CREATED, "{v}");
    }
    let grants: i64 =
        sqlx::query_scalar("SELECT count(*) FROM credits.ledger WHERE kind = 'grant'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(grants, 6);
    pool.close().await;
    db.drop().await;
}

/// Holds `studio`'s row lock in a transaction, starts a grant, waits until it
/// queues behind the lock, applies `change` and commits: the grant then reads
/// the studio as the change left it.
async fn grant_behind(
    pool: &PgPool,
    app: &Router,
    operator: &Wallet,
    studio: &str,
    recipient: &str,
    key: &str,
    change: &str,
) -> (StatusCode, Value) {
    let mut holder = pool.begin().await.unwrap();
    sqlx::query("SELECT 1 FROM credits.studios WHERE id = $1::uuid FOR UPDATE")
        .bind(studio)
        .execute(&mut *holder)
        .await
        .unwrap();
    let request = {
        let (app, operator, studio, recipient, key) = (
            app.clone(),
            operator.clone(),
            studio.to_string(),
            recipient.to_string(),
            key.to_string(),
        );
        tokio::spawn(async move { grant(&app, &operator, &studio, &recipient, 1, &key).await })
    };
    // The grant has passed its first checks and waits for the row.
    let mut waited = false;
    for _ in 0..200 {
        let waiting: bool = sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM pg_stat_activity WHERE datname = current_database() \
             AND wait_event_type = 'Lock' AND query LIKE '%FROM credits.studios WHERE id = $1::uuid FOR UPDATE%')",
        )
        .fetch_one(pool)
        .await
        .unwrap();
        if waiting {
            waited = true;
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }
    sqlx::raw_sql(sqlx::AssertSqlSafe(change.to_string()))
        .execute(&mut *holder)
        .await
        .unwrap();
    holder.commit().await.unwrap();
    let outcome = request.await.unwrap();
    assert!(
        waited,
        "the grant queued behind the studio's lock: {outcome:?}"
    );
    outcome
}

/// A grant reads the operators, the status and the balance under the
/// studio's lock: an operator removed, a pause or an adjustment that commits
/// while the grant waits for the lock is what the grant judges, even when
/// every session defaults to REPEATABLE READ.
#[tokio::test]
async fn a_grant_judges_what_commits_while_it_waits_for_the_studio() {
    let Some(db) = OwnDatabase::create("studio_grant_wait").await else {
        return;
    };
    sqlx::raw_sql(
        "DO $$ BEGIN EXECUTE format('ALTER DATABASE %I SET default_transaction_isolation = %L', \
         current_database(), 'repeatable read'); END $$",
    )
    .execute(&db.pool)
    .await
    .unwrap();
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(6)
        .connect_with((*db.pool.connect_options()).clone())
        .await
        .unwrap();
    let app = app(grant_state(pool.clone(), Some(10_000)));
    let operator = scratch_wallet();
    let player = scratch_wallet();
    let studio = funded_studio(&app, "Waiters", 10, &operator).await;
    let before = written(&pool).await;

    let removed = format!(
        "DELETE FROM credits.studio_operators WHERE studio_id = '{studio}'::uuid AND address = '{}'",
        operator.address
    );
    let (status, v) = grant_behind(
        &pool,
        &app,
        &operator,
        &studio,
        &player.address,
        "wait-key-01",
        &removed,
    )
    .await;
    assert_eq!(
        (status, v["code"].clone()),
        (StatusCode::NOT_FOUND, json!("STUDIO_NOT_FOUND")),
        "{v}"
    );
    add_operator(&app, &studio, &operator.address).await;

    let paused =
        format!("UPDATE credits.studios SET status = 'paused' WHERE id = '{studio}'::uuid");
    let (status, v) = grant_behind(
        &pool,
        &app,
        &operator,
        &studio,
        &player.address,
        "wait-key-02",
        &paused,
    )
    .await;
    assert_eq!(
        (status, v["code"].clone()),
        (StatusCode::CONFLICT, json!("STUDIO_PAUSED")),
        "{v}"
    );
    sqlx::query("UPDATE credits.studios SET status = 'active' WHERE id = $1::uuid")
        .bind(&studio)
        .execute(&pool)
        .await
        .unwrap();

    let emptied = format!(
        "INSERT INTO credits.studio_budget_entries (studio_id, usd_cents, kind, note, created_by) \
         VALUES ('{studio}'::uuid, -100, 'adjustment', 'emptied', 'test')"
    );
    let (status, v) = grant_behind(
        &pool,
        &app,
        &operator,
        &studio,
        &player.address,
        "wait-key-03",
        &emptied,
    )
    .await;
    assert_eq!(
        (status, v["code"].clone()),
        (StatusCode::CONFLICT, json!("STUDIO_BUDGET_EXCEEDED")),
        "{v}"
    );
    assert_eq!(balance_cents(&pool, &studio).await, 0);
    assert_eq!(written(&pool).await, before, "nothing was granted");
    pool.close().await;
    db.drop().await;
}

/// An admin grant is Decentraland's: naming a studio is refused, on the
/// credits route as on the console's.
#[tokio::test]
async fn an_admin_grant_refuses_a_studio_name() {
    let Some(db) = OwnDatabase::create("studio_grant_admin").await else {
        return;
    };
    let app = app(grant_state(db.pool.clone(), Some(500)));
    let player = scratch_wallet();
    let before = written(&db.pool).await;
    let (status, _, v) = send(
        &app,
        "POST",
        "/credits/admin/credits/grant",
        admin_bearer(),
        Some(json!({ "address": player.address, "amount": "5", "studioName": "Acme" })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{v}");
    assert!(v.to_string().contains("/operator/studios/"), "{v}");
    assert_eq!(written(&db.pool).await, before);
    let (status, _, v) = send(
        &app,
        "POST",
        "/credits/admin/credits/grant",
        admin_bearer(),
        Some(json!({ "address": player.address, "amount": "5", "studioName": null })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{v}");
    db.drop().await;
}

/// Each operator wallet is paced: past the window's grants, a request is
/// answered 429 before its body is read, and another operator is not slowed.
#[tokio::test]
async fn an_operator_is_paced() {
    let Some(db) = OwnDatabase::create("studio_grant_pace").await else {
        return;
    };
    let app = app(grant_state(db.pool.clone(), Some(500)));
    let operator = scratch_wallet();
    let other = scratch_wallet();
    let studio = funded_studio(&app, "Pace", 10, &operator).await;
    let path = format!("/credits/operator/studios/{studio}/grants");
    let limit = super::handlers::studio_operators::GRANTS_PER_WINDOW;
    for _ in 0..limit {
        let (status, _) = call(&app, &operator, "post", &path, &[], Some(json!({}))).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }
    let (status, v) = call(&app, &operator, "post", &path, &[], Some(json!({}))).await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS, "{v}");
    assert_eq!(v["retryAfterSeconds"], 60);
    let (status, _) = call(&app, &other, "post", &path, &[], Some(json!({}))).await;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "another wallet is not slowed"
    );
    db.drop().await;
}

/// Two identical grants sent at once both pass the early replay read; the
/// one that takes the studio's lock second finds the first's entry and
/// answers it as a replay: one grant, one credit, never two.
#[tokio::test]
async fn two_identical_grants_racing_make_one_grant() {
    let Some(db) = OwnDatabase::create("studio_grant_twin").await else {
        return;
    };
    let app = app(grant_state(db.pool.clone(), Some(500)));
    let operator = scratch_wallet();
    let studio = funded_studio(&app, "Twins", 50, &operator).await;
    for round in 0..6 {
        let player = scratch_wallet();
        let key = format!("twin-key-{round:04}");
        let ((sa, va), (sb, vb)) = tokio::join!(
            grant(&app, &operator, &studio, &player.address, 5, &key),
            grant(&app, &operator, &studio, &player.address, 5, &key)
        );
        let mut statuses = [sa, sb];
        statuses.sort();
        assert_eq!(
            statuses,
            [StatusCode::OK, StatusCode::CREATED],
            "round {round}: {va} {vb}"
        );
        assert_eq!(va["creditId"], vb["creditId"], "round {round}");
        let credits: i64 =
            sqlx::query_scalar("SELECT count(*) FROM credits.ledger WHERE address = $1")
                .bind(&player.address)
                .fetch_one(&db.pool)
                .await
                .unwrap();
        assert_eq!(credits, 1, "round {round}");
    }
    assert_eq!(balance_cents(&db.pool, &studio).await, 500 - 6 * 50);
    db.drop().await;
}
