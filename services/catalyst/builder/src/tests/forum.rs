//! The forum routes against a loopback Discourse: what the node sends, what
//! it stores, what it answers, and that a collection gets one topic however
//! its requests interleave with each other and with the forum's failures.

use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use chrono::Utc;
use serde_json::{Value, json};
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

use super::support::{
    FakeCommittee, committee_index, on_chain, scratch_pool, temp_storage, wallet,
};
use crate::curation::forum::fake::{API_USERNAME, FakeForum};
use crate::shared::signed_fetch::test_support::signed_headers_by;

const OWNER_KEY: &str = super::support::OWNER_KEY;
const STRANGER_KEY: &str = "0x2222222222222222222222222222222222222222222222222222222222222222";
const COMMITTEE_KEY: &str = "0x3333333333333333333333333333333333333333333333333333333333333333";
const NAMED_KEY: &str = "0x7777777777777777777777777777777777777777777777777777777777777777";
const PROVIDER: &str = "urn:decentraland:matic:collections-thirdparty:forum";

fn address(key: &str) -> String {
    format!("{:#x}", wallet(key).address())
}

struct Env {
    pool: PgPool,
    app: Router,
    state: crate::SharedState,
    forum: FakeForum,
    origin: String,
}

async fn env_with(pool: PgPool, forum_on: bool) -> Env {
    let committee = FakeCommittee::with(&[&address(COMMITTEE_KEY)]);
    let forum = FakeForum::default();
    let origin = forum.start().await;
    let mut state = crate::State::new(
        pool.clone(),
        temp_storage().await,
        Arc::default(),
        dcl_http::Outbound::for_tests(),
        Some(committee_index(&committee).await),
        None,
        crate::Settings {
            writes_enabled: true,
            forum: forum_on.then(|| FakeForum::forum(&origin)),
            links: crate::curation::slack::Links {
                creators_url: Some("https://creators.example.org".into()),
                builder_url: None,
            },
            builder_server_url: "https://builder-api.example.org".into(),
            ..crate::Settings::default()
        },
    )
    .unwrap();
    state.guarded = dcl_http::Outbound::for_tests();
    let state = Arc::new(state);
    let app = Router::new()
        .merge(crate::root_router(state.clone()))
        .nest("/builder-api", crate::router(state.clone()));
    Env {
        pool,
        app,
        state,
        forum,
        origin,
    }
}

async fn env() -> Option<Env> {
    Some(env_with(scratch_pool(4).await?, true).await)
}

async fn send(app: &Router, key: Option<&str>, path: &str, body: Value) -> (StatusCode, Value) {
    let mut request = Request::builder()
        .method("POST")
        .uri(path)
        .header("content-type", "application/json");
    if let Some(key) = key {
        let now = Utc::now().timestamp_millis().to_string();
        for (name, value) in signed_headers_by(&wallet(key), "post", path, "{}", true, &now)
            .await
            .iter()
        {
            request = request.header(name, value);
        }
    }
    let response = app
        .clone()
        .oneshot(request.body(Body::from(body.to_string())).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), 1 << 20)
        .await
        .unwrap();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

async fn standard(pool: &PgPool, name: &str, published: bool) -> (Uuid, Option<String>) {
    standard_of(pool, OWNER_KEY, name, published).await
}

async fn standard_of(
    pool: &PgPool,
    key: &str,
    name: &str,
    published: bool,
) -> (Uuid, Option<String>) {
    let owner = address(key);
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO builder.collections (id, name, eth_address) VALUES ($1, $2, $3)")
        .bind(id)
        .bind(format!("{name} {}", &id.simple().to_string()[..8]))
        .bind(&owner)
        .execute(pool)
        .await
        .unwrap();
    for (n, item) in ["Red hat", "Blue hat"].into_iter().enumerate() {
        let item_id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO builder.items (id, collection_id, eth_address, name, description, type, rarity, data, thumbnail, created_at) \
             VALUES ($1, $2, $3, $4, 'A *hat*', 'wearable', 'common', '{\"category\":\"hat\"}', 'thumbnail.png', \
               timezone('UTC', now()) + $5 * interval '1 second')",
        )
        .bind(item_id)
        .bind(id)
        .bind(&owner)
        .bind(item)
        .bind(n as i32)
        .execute(pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO builder.item_contents (item_id, file, hash, named_by) VALUES ($1, 'thumbnail.png', $2, $3)",
        )
        .bind(item_id)
        .bind(format!("bafkreithumb{n}"))
        .bind(&owner)
        .execute(pool)
        .await
        .unwrap();
    }
    let contract = if published {
        Some(on_chain(pool, id, false).await)
    } else {
        None
    };
    (id, contract)
}

async fn third_party(pool: &PgPool, published: bool) -> Uuid {
    sqlx::query(
        "INSERT INTO builder.virtual_third_parties (id, managers, raw_metadata) VALUES ($1, $2, 'tp:1:Forum:') \
         ON CONFLICT (id) DO NOTHING",
    )
    .bind(PROVIDER)
    .bind(vec![address(OWNER_KEY)])
    .execute(pool)
    .await
    .unwrap();
    let id = Uuid::new_v4();
    let urn = format!("{PROVIDER}:forum-{}", id.simple());
    sqlx::query(
        "INSERT INTO builder.collections (id, name, eth_address, third_party_id, urn_suffix) VALUES ($1, 'Linked hats', $2, $3, $4)",
    )
    .bind(id)
    .bind(address(OWNER_KEY))
    .bind(PROVIDER)
    .bind(&urn)
    .execute(pool)
    .await
    .unwrap();
    let item = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO builder.items (id, collection_id, eth_address, name, type, urn_suffix) VALUES ($1, $2, $3, 'Linked hat', 'wearable', $4)",
    )
    .bind(item)
    .bind(id)
    .bind(address(OWNER_KEY))
    .bind(format!("{urn}:{}", item.simple()))
    .execute(pool)
    .await
    .unwrap();
    if published {
        sqlx::query(
            "INSERT INTO builder.item_curations (id, item_id, status, content_hash) VALUES ($1, $2, 'pending', $3)",
        )
        .bind(Uuid::new_v4())
        .bind(item)
        .bind("ab".repeat(32))
        .execute(pool)
        .await
        .unwrap();
    }
    id
}

async fn stored(pool: &PgPool, id: Uuid) -> (Option<String>, Option<i32>, bool) {
    sqlx::query_as(
        "SELECT forum_link, forum_id, forum_claimed_at IS NOT NULL FROM builder.collections WHERE id = $1",
    )
    .bind(id)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn age_claim(pool: &PgPool, id: Uuid) {
    sqlx::query(
        "UPDATE builder.collections SET forum_claimed_at = forum_claimed_at - interval '61 seconds' WHERE id = $1",
    )
    .bind(id)
    .execute(pool)
    .await
    .unwrap();
}

async fn at_forum(forum: &FakeForum, calls: usize) {
    for _ in 0..500 {
        if forum.calls().len() >= calls {
            return;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("the forum was not called");
}

fn creates(forum: &FakeForum) -> usize {
    forum
        .calls()
        .iter()
        .filter(|c| c["method"] == "POST" && c["body"].get("title").is_some())
        .count()
}

#[tokio::test]
async fn a_published_collection_gets_one_topic_in_the_nodes_words() {
    let Some(e) = env().await else {
        return;
    };
    let (id, _) = standard(&e.pool, "Hats", true).await;
    let name: String = sqlx::query_scalar("SELECT name FROM builder.collections WHERE id = $1")
        .bind(id)
        .fetch_one(&e.pool)
        .await
        .unwrap();
    let path = format!("/v1/collections/{id}/post");
    let hostile = json!({ "forumPost": {
        "title": "attacker title", "raw": "attacker body", "topic_id": 1, "category": 99, "archetype": "private_message"
    }});
    let (status, body) = send(&e.app, Some(OWNER_KEY), &path, hostile).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let title = format!("Collection '{name}' created by 0x19e7...ff2a is ready for review!");
    let link = format!(
        "{}/t/{}/1001",
        e.origin,
        crate::curation::forum::fake::slug(&title)
    );
    assert_eq!(body, json!({ "ok": true, "data": link }));
    assert_eq!(
        stored(&e.pool, id).await,
        (Some(link.clone()), Some(5001), false)
    );
    let calls = e.forum.take();
    assert_eq!(calls.len(), 1, "{calls:#?}");
    let editor = format!("https://creators.example.org/collections/editor?collection={id}");
    let items: Vec<(Uuid,)> =
        sqlx::query_as("SELECT id FROM builder.items WHERE collection_id = $1 ORDER BY created_at")
            .bind(id)
            .fetch_all(&e.pool)
            .await
            .unwrap();
    let item = |n: usize, label: &str| {
        format!(
            "**{label}**\n      - Description: A \\*hat\\*\n- Rarity: common\n- Category: hat\n      \
             ![](https://builder-api.example.org/v1/storage/contents/bafkreithumb{n})\n      \
             [Link to editor]({editor}&item={}&reviewing=true)",
            items[n].0
        )
    };
    assert_eq!(
        calls[0],
        json!({
            "method": "POST", "path": "/posts.json", "username": null,
            "body": {
                "title": title,
                "raw": format!(
                    "# {}\n\n  [View entire collection]({editor}&reviewing=true)\n\n  ## Wearables\n\n  {}\n\n{}",
                    crate::curation::forum_text::sanitize(&name),
                    item(0, "Red hat"),
                    item(1, "Blue hat")
                ),
                "category": "12",
            }
        })
    );
    let (status, body) = send(
        &e.app,
        Some(OWNER_KEY),
        &format!("/builder-api{path}"),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(
        body,
        json!({ "ok": false, "error": "Forum post already exists", "data": { "id": id, "forum_link": link } })
    );
    assert!(
        e.forum.calls().is_empty(),
        "a collection with a topic never calls out"
    );
}

#[tokio::test]
async fn a_title_names_the_creators_claimed_name_and_a_taken_title_gets_the_contract() {
    let Some(e) = env().await else {
        return;
    };
    let (first, _) = standard_of(&e.pool, NAMED_KEY, "Twins 🎩✨", true).await;
    let (second, contract) = standard_of(&e.pool, NAMED_KEY, "Twins 🎩✨", true).await;
    let name: String = sqlx::query_scalar("SELECT name FROM builder.collections WHERE id = $1")
        .bind(first)
        .fetch_one(&e.pool)
        .await
        .unwrap();
    sqlx::query("UPDATE builder.collections SET name = $2 WHERE id = $1")
        .bind(second)
        .bind(&name)
        .execute(&e.pool)
        .await
        .unwrap();
    let owner = address(NAMED_KEY);
    let entity = format!("bafkrei{}", Uuid::new_v4().simple());
    sqlx::query(
        "INSERT INTO content.deployments (deployer_address, version, entity_type, entity_id, entity_metadata, \
           entity_timestamp, entity_pointers, local_timestamp, auth_chain) \
         VALUES ($1, 'v3', 'profile', $2, $3, to_timestamp(1451606400), ARRAY[$1], now(), '[]'::json)",
    )
    .bind(&owner)
    .bind(&entity)
    .bind(json!({ "v": { "avatars": [{ "name": "Hatter", "hasClaimedName": true }] } }))
    .execute(&e.pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO content.active_pointers (pointer, entity_id, entity_type) VALUES ($1, $2, 'profile') \
         ON CONFLICT (pointer) DO UPDATE SET entity_id = EXCLUDED.entity_id",
    )
    .bind(&owner)
    .bind(&entity)
    .execute(&e.pool)
    .await
    .unwrap();
    let title = format!(
        "Collection '{}' created by Hatter is ready for review!",
        name.replace("🎩✨", "")
    );
    for id in [first, second] {
        let (status, body) = send(
            &e.app,
            Some(NAMED_KEY),
            &format!("/v1/collections/{id}/post"),
            json!({}),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
    }
    sqlx::query("DELETE FROM content.active_pointers WHERE pointer = $1")
        .bind(&owner)
        .execute(&e.pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM content.deployments WHERE entity_id = $1")
        .bind(&entity)
        .execute(&e.pool)
        .await
        .unwrap();
    let titles: Vec<Value> = e
        .forum
        .calls()
        .iter()
        .map(|c| c["body"]["title"].clone())
        .collect();
    let contract = contract.unwrap();
    let shortened = format!("{}...{}", &contract[..6], &contract[38..]);
    assert_eq!(
        titles,
        vec![
            json!(title),
            json!(title),
            json!(format!("{title} {shortened}"))
        ]
    );
    assert_eq!(e.forum.topics(), 2);
}

#[tokio::test]
async fn a_third_party_collection_is_posted_under_its_provider_before_any_item_is_submitted() {
    let Some(e) = env().await else {
        return;
    };
    for submitted in [false, true] {
        let id = third_party(&e.pool, submitted).await;
        sqlx::query("UPDATE builder.collections SET name = $2 WHERE id = $1")
            .bind(id)
            .bind(format!("Linked hats {submitted}"))
            .execute(&e.pool)
            .await
            .unwrap();
        let (status, body) = send(
            &e.app,
            Some(OWNER_KEY),
            &format!("/v1/collections/{id}/post"),
            json!({}),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "submitted {submitted}: {body}");
        let calls = e.forum.take();
        assert_eq!(calls.len(), 1, "{calls:#?}");
        assert_eq!(
            calls[0]["body"]["title"],
            format!("Third Party collection Linked hats {submitted} with URN: {PROVIDER}")
        );
        let (link, post, claimed) = stored(&e.pool, id).await;
        assert_eq!(link.as_deref(), body["data"].as_str());
        assert!(post.is_some() && !claimed);
    }
    let draft = third_party(&e.pool, false).await;
    let (status, body) = send(
        &e.app,
        Some(STRANGER_KEY),
        &format!("/v1/collections/{draft}/post"),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "{body}");
    assert!(e.forum.calls().is_empty());
}

#[tokio::test]
async fn without_a_forum_an_admitted_post_or_reply_is_unavailable_and_nothing_is_claimed() {
    let Some(pool) = scratch_pool(4).await else {
        return;
    };
    let e = env_with(pool, false).await;
    let (id, _) = standard(&e.pool, "Unforumed", true).await;
    let (status, body) = send(
        &e.app,
        Some(OWNER_KEY),
        &format!("/v1/collections/{id}/post"),
        json!({}),
    )
    .await;
    assert_eq!(
        (status, body["error"].clone()),
        (
            StatusCode::SERVICE_UNAVAILABLE,
            json!("The forum is not available. Try again later.")
        )
    );
    assert_eq!(stored(&e.pool, id).await, (None, None, false));
    let (status, _) = send(
        &e.app,
        Some(STRANGER_KEY),
        &format!("/v1/collections/{id}/post"),
        json!({}),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "a refusal comes before the forum"
    );
    sqlx::query("UPDATE builder.collections SET forum_link = 'https://forum.example.test/t/x/1', forum_id = 1 WHERE id = $1")
        .bind(id)
        .execute(&e.pool)
        .await
        .unwrap();
    let (status, _) = send(
        &e.app,
        Some(COMMITTEE_KEY),
        &format!("/v1/collections/{id}/curation/post"),
        json!({ "forumPost": { "raw": "hi" } }),
    )
    .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert!(e.forum.calls().is_empty());
}

#[tokio::test]
async fn two_requests_at_once_make_one_topic() {
    let Some(e) = env().await else {
        return;
    };
    let (id, _) = standard(&e.pool, "Race", true).await;
    let path = format!("/v1/collections/{id}/post");
    e.forum.hold();
    let first = tokio::spawn({
        let (app, path) = (e.app.clone(), path.clone());
        async move { send(&app, Some(OWNER_KEY), &path, json!({})).await }
    });
    at_forum(&e.forum, 1).await;
    let (status, body) = send(&e.app, Some(OWNER_KEY), &path, json!({})).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{body}");
    assert_eq!(
        body["error"],
        "The forum post is being created. Try again later."
    );
    e.forum.release();
    let (status, body) = first.await.unwrap();
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, again) = send(&e.app, Some(OWNER_KEY), &path, json!({})).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(again["data"]["forum_link"], body["data"]);
    assert_eq!((e.forum.topics(), creates(&e.forum)), (1, 1));
}

#[tokio::test]
async fn simultaneous_claims_decide_under_the_collection_lock() {
    let Some(pool) = crate::db::scratch::isolated_pool(4).await else {
        return;
    };
    sqlx::query("INSERT INTO shop.mirror_state (mirror, network, source_url, last_complete_pass_at, pass_started_at) VALUES ('collections', 'matic', 'https://subgraph.example', timezone('UTC', now()), timezone('UTC', now()))")
        .execute(&pool).await.unwrap();
    crate::tests::support::publish_registry_index(&mut pool.acquire().await.unwrap(), false).await;
    let e = env_with(pool, true).await;
    let (id, _) = standard(&e.pool, "Simultaneous", true).await;
    sqlx::raw_sql("CREATE FUNCTION builder.delay_forum_claim() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN PERFORM pg_sleep(0.2); RETURN NEW; END $$; CREATE TRIGGER delay_forum_claim BEFORE UPDATE OF forum_claimed_at ON builder.collections FOR EACH ROW EXECUTE FUNCTION builder.delay_forum_claim();")
        .execute(&e.pool).await.unwrap();
    let path = format!("/v1/collections/{id}/post");
    let (a, b) = tokio::join!(
        send(&e.app, Some(OWNER_KEY), &path, json!({})),
        send(&e.app, Some(OWNER_KEY), &path, json!({})),
    );
    assert!(
        a.0 == StatusCode::OK || b.0 == StatusCode::OK,
        "{a:?} {b:?}"
    );
    assert_eq!(creates(&e.forum), 1, "{a:?} {b:?}");
}

#[tokio::test]
async fn a_forum_claim_rechecks_the_managers_current_access() {
    let Some(e) = env().await else {
        return;
    };
    let (id, contract) = standard(&e.pool, "Rechecked", true).await;
    let path = format!("/v1/collections/{id}/post");
    let now = Utc::now().timestamp_millis().to_string();
    let headers = signed_headers_by(&wallet(OWNER_KEY), "post", &path, "{}", true, &now).await;
    let signer = crate::auth::require_signer(
        &headers,
        "post",
        &crate::auth::SignedPath::local(&path),
        None,
    )
    .await
    .unwrap();
    let grant = crate::access::manage_collection(&e.state, &signer, &id.to_string())
        .await
        .unwrap();
    sqlx::query("UPDATE builder.collections SET eth_address = $2 WHERE id = $1")
        .bind(id)
        .bind(address(STRANGER_KEY))
        .execute(&e.pool)
        .await
        .unwrap();
    sqlx::query("UPDATE shop.collections SET creator = $2, owner = $2 WHERE network = 'matic' AND contract_address = $1")
        .bind(contract).bind(address(STRANGER_KEY)).execute(&e.pool).await.unwrap();
    let error = crate::curation::forum_posts::post(&e.state, &grant)
        .await
        .unwrap_err();
    assert_eq!(error.status(), 401);
    assert!(e.forum.calls().is_empty());
}

#[tokio::test]
async fn a_post_the_forum_accepted_but_the_node_never_stored_is_made_again_once_its_claim_expires()
{
    let Some(e) = env().await else {
        return;
    };
    let (id, contract) = standard(&e.pool, "Lost", true).await;
    let path = format!("/v1/collections/{id}/post");
    e.forum.lose(1);
    let (status, body) = send(&e.app, Some(OWNER_KEY), &path, json!({})).await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR, "{body}");
    assert_eq!(stored(&e.pool, id).await, (None, None, true));
    assert_eq!(e.forum.topics(), 1, "the forum made the topic");
    let (status, _) = send(&e.app, Some(OWNER_KEY), &path, json!({})).await;
    assert_eq!(
        status,
        StatusCode::SERVICE_UNAVAILABLE,
        "a fresh claim holds"
    );
    assert_eq!(e.forum.calls().len(), 1, "a held claim calls nothing");
    age_claim(&e.pool, id).await;
    let (status, body) = send(&e.app, Some(OWNER_KEY), &path, json!({})).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let link = body["data"].as_str().unwrap().to_string();
    assert!(link.ends_with("/1002"), "{link}");
    assert_eq!(stored(&e.pool, id).await, (Some(link), Some(5002), false));
    let calls = e.forum.calls();
    let paths: Vec<&Value> = calls.iter().map(|c| &c["path"]).collect();
    assert_eq!(paths, vec!["/posts.json"; 3], "{calls:#?}");
    let shortened = crate::curation::slack::shorten(&contract.unwrap());
    assert!(
        calls[2]["body"]["title"]
            .as_str()
            .unwrap()
            .ends_with(&format!(" {shortened}")),
        "the second topic takes the contract in its title: {calls:#?}"
    );
    assert!(calls.iter().all(|c| c["body"].get("external_id").is_none()));
    assert_eq!(e.forum.topics(), 2, "the forum holds two topics");
}

#[tokio::test]
async fn a_third_party_post_the_node_never_stored_is_refused_again_by_its_taken_title() {
    let Some(e) = env().await else {
        return;
    };
    let id = third_party(&e.pool, false).await;
    let path = format!("/v1/collections/{id}/post");
    e.forum.lose(1);
    let (status, _) = send(&e.app, Some(OWNER_KEY), &path, json!({})).await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    age_claim(&e.pool, id).await;
    let (status, body) = send(&e.app, Some(OWNER_KEY), &path, json!({})).await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR, "{body}");
    assert_eq!(body["error"], "Error creating forum post");
    assert_eq!((e.forum.topics(), creates(&e.forum)), (1, 2));
    assert_eq!(stored(&e.pool, id).await, (None, None, true));
}

#[tokio::test]
async fn a_refused_post_is_made_once_its_claim_expires_and_the_forums_words_stay_out() {
    let Some(e) = env().await else {
        return;
    };
    let (id, _) = standard(&e.pool, "Refused", true).await;
    let path = format!("/v1/collections/{id}/post");
    e.forum.refuse(1);
    let (status, body) = send(&e.app, Some(OWNER_KEY), &path, json!({})).await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(body["error"], "Error creating forum post");
    assert!(!body.to_string().contains("Something went wrong"), "{body}");
    let (status, _) = send(&e.app, Some(OWNER_KEY), &path, json!({})).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    age_claim(&e.pool, id).await;
    let (status, body) = send(&e.app, Some(OWNER_KEY), &path, json!({})).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!((e.forum.topics(), creates(&e.forum)), (1, 2));
}

#[tokio::test]
async fn an_attempt_overtaken_after_its_claim_lapsed_keeps_the_first_stored_link() {
    let Some(e) = env().await else {
        return;
    };
    let (id, _) = standard(&e.pool, "Slow", true).await;
    let path = format!("/v1/collections/{id}/post");
    e.forum.hold();
    let first = tokio::spawn({
        let (app, path) = (e.app.clone(), path.clone());
        async move { send(&app, Some(OWNER_KEY), &path, json!({})).await }
    });
    at_forum(&e.forum, 1).await;
    age_claim(&e.pool, id).await;
    let second = tokio::spawn({
        let (app, path) = (e.app.clone(), path.clone());
        async move { send(&app, Some(OWNER_KEY), &path, json!({})).await }
    });
    at_forum(&e.forum, 2).await;
    e.forum.release();
    let answers = [first.await.unwrap(), second.await.unwrap()];
    let made: Vec<&Value> = answers
        .iter()
        .filter(|(status, _)| *status == StatusCode::OK)
        .map(|(_, body)| &body["data"])
        .collect();
    assert_eq!(e.forum.topics(), 2, "{answers:?}");
    let (link, _, claimed) = stored(&e.pool, id).await;
    assert!(!claimed);
    assert!(
        !made.is_empty() && made.iter().all(|m| m.as_str() == link.as_deref()),
        "{answers:?}"
    );
}

#[tokio::test]
async fn an_attempt_whose_store_waits_on_another_stored_link_answers_the_stored_one() {
    let Some(e) = env().await else {
        return;
    };
    let (id, _) = standard(&e.pool, "Raced", true).await;
    e.forum.hold();
    let attempt = tokio::spawn({
        let (app, path) = (e.app.clone(), format!("/v1/collections/{id}/post"));
        async move { send(&app, Some(OWNER_KEY), &path, json!({})).await }
    });
    at_forum(&e.forum, 1).await;
    let other = format!("{}/t/other/9", e.origin);
    let mut competing = e.pool.begin().await.unwrap();
    sqlx::query(
        "UPDATE builder.collections SET forum_link = $2, forum_id = 9, forum_claimed_at = NULL WHERE id = $1",
    )
    .bind(id)
    .bind(&other)
    .execute(&mut *competing)
    .await
    .unwrap();
    e.forum.release();
    let mut waiting = false;
    for _ in 0..500 {
        waiting = sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM pg_stat_activity WHERE datname = current_database() \
             AND wait_event_type = 'Lock' AND query LIKE '%UPDATE builder.collections SET forum_link%')",
        )
        .fetch_one(&e.pool)
        .await
        .unwrap();
        if waiting {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert!(waiting, "the attempt's store never waited on the row");
    competing.commit().await.unwrap();
    let (status, body) = attempt.await.unwrap();
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["data"], json!(other));
    assert_eq!(stored(&e.pool, id).await, (Some(other), Some(9), false));
}

#[tokio::test]
async fn no_connection_is_held_while_the_forum_answers() {
    let Some(pool) = crate::db::scratch::isolated_pool(1).await else {
        return;
    };
    sqlx::query(
        "INSERT INTO shop.mirror_state (mirror, network, source_url, last_complete_pass_at, pass_started_at) \
         VALUES ('collections', 'matic', 'https://subgraph.example', timezone('UTC', now()), timezone('UTC', now()))",
    )
    .execute(&pool)
    .await
    .unwrap();
    crate::tests::support::publish_registry_index(&mut pool.acquire().await.unwrap(), false).await;
    let e = env_with(pool, true).await;
    let (id, _) = standard(&e.pool, "Pooled", true).await;
    e.forum.hold();
    let first = tokio::spawn({
        let (app, path) = (e.app.clone(), format!("/v1/collections/{id}/post"));
        async move { send(&app, Some(OWNER_KEY), &path, json!({})).await }
    });
    at_forum(&e.forum, 1).await;
    let free = tokio::time::timeout(
        Duration::from_secs(5),
        sqlx::query_scalar::<_, i32>("SELECT 1").fetch_one(&e.pool),
    )
    .await;
    e.forum.release();
    assert!(
        matches!(free, Ok(Ok(1))),
        "the only connection was held: {free:?}"
    );
    assert_eq!(first.await.unwrap().0, StatusCode::OK);
}

async fn posted(e: &Env) -> Uuid {
    let (id, _) = standard(&e.pool, "Replied", true).await;
    let (status, _) = send(
        &e.app,
        Some(OWNER_KEY),
        &format!("/v1/collections/{id}/post"),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    e.forum.take();
    id
}

#[tokio::test]
async fn a_committee_reply_goes_to_the_collections_topic_and_mentions_nobody() {
    let Some(e) = env().await else {
        return;
    };
    let id = posted(&e).await;
    let path = format!("/builder-api/v1/collections/{id}/curation/post");
    let (status, body) = send(
        &e.app,
        Some(COMMITTEE_KEY),
        &path,
        json!({ "forumPost": { "topic_id": 999, "raw": "The collection has been assigned to @admins" } }),
    )
    .await;
    assert_eq!((status, body), (StatusCode::OK, json!({ "ok": true })));
    assert_eq!(
        e.forum.take(),
        vec![
            json!({ "method": "GET", "path": "/posts/5001.json", "username": API_USERNAME, "body": null }),
            json!({ "method": "POST", "path": "/posts.json", "username": null,
                "body": { "topic_id": 1001, "raw": "The collection has been assigned to @\u{200b}admins" } }),
        ]
    );
}

#[tokio::test]
async fn a_reply_is_refused_in_builder_servers_order_before_the_forum_is_asked() {
    let Some(e) = env().await else {
        return;
    };
    let id = posted(&e).await;
    let (bare, _) = standard(&e.pool, "Bare", true).await;
    let reply = |raw: Value| json!({ "forumPost": { "raw": raw } });
    let path = |id: &str| format!("/v1/collections/{id}/curation/post");
    let stranger = address(STRANGER_KEY);
    for (key, target, body, status, error, data) in [
        (
            None,
            id.to_string(),
            reply(json!("x")),
            401,
            "Unauthenticated",
            None,
        ),
        (
            Some(OWNER_KEY),
            id.to_string(),
            reply(json!("x")),
            401,
            "Unauthorized",
            Some(json!({ "id": id, "ethAddress": address(OWNER_KEY) })),
        ),
        (
            Some(STRANGER_KEY),
            "not-a-uuid".into(),
            reply(json!("x")),
            401,
            "Unauthorized",
            Some(json!({ "id": "not-a-uuid", "ethAddress": stranger })),
        ),
        (
            Some(COMMITTEE_KEY),
            Uuid::new_v4().to_string(),
            reply(json!("x")),
            404,
            "Collection not found",
            None,
        ),
        (
            Some(COMMITTEE_KEY),
            bare.to_string(),
            json!({}),
            409,
            "The collection does not have a forum post yet",
            Some(json!({ "id": bare })),
        ),
        (
            Some(COMMITTEE_KEY),
            id.to_string(),
            json!({}),
            400,
            "Invalid forum post",
            Some(json!({ "id": id })),
        ),
        (
            Some(COMMITTEE_KEY),
            id.to_string(),
            reply(json!("")),
            400,
            "Invalid forum post",
            None,
        ),
        (
            Some(COMMITTEE_KEY),
            id.to_string(),
            reply(json!(7)),
            400,
            "Invalid forum post",
            None,
        ),
        (
            Some(COMMITTEE_KEY),
            id.to_string(),
            reply(json!("x".repeat(10_001))),
            400,
            "Invalid forum post",
            None,
        ),
        (
            Some(COMMITTEE_KEY),
            id.to_string(),
            json!({ "forumPost": { "raw": "x" }, "category": 1 }),
            400,
            "Invalid forum post",
            None,
        ),
        (
            Some(COMMITTEE_KEY),
            id.to_string(),
            json!({ "forumPost": { "raw": "x", "category": 1 } }),
            400,
            "Invalid forum post",
            None,
        ),
    ] {
        let (got, body) = send(&e.app, key, &path(&target), body).await;
        assert_eq!(
            (got.as_u16(), body["error"].as_str()),
            (status, Some(error)),
            "{target} {body}"
        );
        if let Some(data) = data {
            assert_eq!(body["data"], data);
        }
    }
    assert!(e.forum.calls().is_empty(), "{:#?}", e.forum.calls());
}

#[tokio::test]
async fn a_reply_the_forum_refuses_answers_ok_false_without_the_forums_words() {
    let Some(e) = env().await else {
        return;
    };
    let id = posted(&e).await;
    e.forum.refuse(1);
    let (status, body) = send(
        &e.app,
        Some(COMMITTEE_KEY),
        &format!("/v1/collections/{id}/curation/post"),
        json!({ "forumPost": { "raw": "unassigned." } }),
    )
    .await;
    assert_eq!(
        (status, body),
        (
            StatusCode::OK,
            json!({ "ok": false, "error": "Error creating forum post", "data": { "id": id } })
        )
    );
    sqlx::query("UPDATE builder.collections SET forum_id = 4999 WHERE id = $1")
        .bind(id)
        .execute(&e.pool)
        .await
        .unwrap();
    let (status, body) = send(
        &e.app,
        Some(COMMITTEE_KEY),
        &format!("/v1/collections/{id}/curation/post"),
        json!({ "forumPost": { "raw": "unassigned." } }),
    )
    .await;
    assert_eq!(
        (status, body["ok"].clone()),
        (StatusCode::OK, json!(false)),
        "an unknown post: {body}"
    );
}

#[tokio::test]
async fn a_forum_redirect_is_not_followed_with_its_credentials() {
    use axum::response::IntoResponse;
    use axum::routing::post;
    use std::sync::atomic::{AtomicUsize, Ordering};

    let calls = Arc::new(AtomicUsize::new(0));
    let observed = calls.clone();
    let app = Router::new()
        .route(
            "/posts.json",
            post(|| async {
                (StatusCode::TEMPORARY_REDIRECT, [("location", "/elsewhere")]).into_response()
            }),
        )
        .route(
            "/elsewhere",
            post(move || {
                observed.fetch_add(1, Ordering::SeqCst);
                async { axum::Json(json!({"id":1,"topic_id":2,"topic_slug":"hats"})) }
            }),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let forum = FakeForum::forum(&format!("http://{}", listener.local_addr().unwrap()));
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let client = dcl_http::Outbound::new(dcl_http::HttpClientCfg::default().following_redirects(2))
        .unwrap()
        .allowing_private_hosts();
    let result = forum.create(&client, "Hats", "body").await;
    server.abort();
    assert_eq!(
        result,
        Err(crate::curation::forum::Failure::Refused(
            StatusCode::TEMPORARY_REDIRECT
        ))
    );
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn a_forum_call_times_out_before_twelve_seconds() {
    use axum::routing::post;

    let app = Router::new().route(
        "/posts.json",
        post(|| async {
            tokio::time::sleep(Duration::from_secs(60)).await;
            axum::Json(json!({"id":1,"topic_id":2,"topic_slug":"hats"}))
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let forum = FakeForum::forum(&format!("http://{}", listener.local_addr().unwrap()));
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let client = dcl_http::Outbound::for_tests();
    let result = tokio::time::timeout(
        Duration::from_secs(12),
        forum.create(&client, "Hats", "body"),
    )
    .await;
    server.abort();
    assert!(
        matches!(
            result,
            Ok(Err(crate::curation::forum::Failure::Unreachable))
        ),
        "{result:?}"
    );
}

#[tokio::test]
async fn a_chunked_forum_answer_is_bounded_without_a_content_length() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let forum = FakeForum::forum(&format!("http://{}", listener.local_addr().unwrap()));
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut request = [0; 8192];
        let read = stream.read(&mut request).await.unwrap();
        assert!(read > 0);
        let body = json!({"id":1,"topic_id":2,"topic_slug":"hats","padding":" ".repeat(1024*1024)})
            .to_string();
        stream
            .write_all(
                b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n",
            )
            .await
            .unwrap();
        for chunk in body.as_bytes().chunks(32 * 1024) {
            if stream
                .write_all(format!("{:x}\r\n", chunk.len()).as_bytes())
                .await
                .is_err()
                || stream.write_all(chunk).await.is_err()
                || stream.write_all(b"\r\n").await.is_err()
            {
                return;
            }
        }
        let _ = stream.write_all(b"0\r\n\r\n").await;
    });
    let result = forum
        .create(&dcl_http::Outbound::for_tests(), "Hats", "body")
        .await;
    server.abort();
    assert_eq!(result, Err(crate::curation::forum::Failure::Invalid));
}
