//! `catalyst camera-reel import` end to end: a scratch source database with
//! camera-reel-service's `images` table as its migrations leave it
//! (@ 3e8bd0b `migrations/`, the table's DDL in their order), a fake bucket
//! on the loopback, two passes (the second changes nothing), a refresh, a
//! moderated row left alone, a photo uploaded here left alone, a prune, and
//! the legacy URLs served from the imported rows.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use axum::extract::{Path, State as AxumState};
use axum::http::{StatusCode, header};
use axum::response::IntoResponse;
use serde_json::{Value, json};
use sqlx::PgPool;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use uuid::Uuid;

use super::{API_URL, PREFIX, Scratch, png, random_address, scratch, send};
use crate::auth::hash_bytes_v1;
use crate::import::{ImportOptions, ImportReport, run};
use crate::{Settings, State, router};

/// The `images` DDL of camera-reel-service's migrations, in order
/// (initial schema, thumbnail url, public flag).
const UPSTREAM_IMAGES_DDL: &str = "
CREATE TABLE IF NOT EXISTS images (
    id UUID PRIMARY KEY,
    user_address TEXT NOT NULL,
    url TEXT NOT NULL,
    metadata JSONB NOT NULL,
    created_at TIMESTAMP DEFAULT now()
);
CREATE INDEX IF NOT EXISTS images_user_address_idx ON images (user_address);
ALTER TABLE images ADD COLUMN thumbnail_url TEXT NOT NULL DEFAULT '';
ALTER TABLE images ADD COLUMN is_public BOOLEAN NOT NULL DEFAULT FALSE;
";

const OLD_API: &str = "https://camera-reel-service.example/api/images";

struct Source {
    admin: PgPool,
    name: String,
    pool: PgPool,
}

/// A database of its own, named so the scratch sweep drops it if this
/// process dies before `drop`.
async fn source_database() -> Option<Source> {
    let base = std::env::var("CATALYST_TEST_DATABASE_URL").ok()?;
    let opts: PgConnectOptions = base.parse().unwrap();
    let admin = PgPoolOptions::new()
        .max_connections(1)
        .connect_with(opts.clone().database("postgres"))
        .await
        .unwrap();
    let name = format!(
        "catalyst_test_lib_{}_{}",
        std::process::id(),
        &Uuid::new_v4().simple().to_string()[..8]
    );
    sqlx::query(sqlx::AssertSqlSafe(format!("CREATE DATABASE \"{name}\"")))
        .execute(&admin)
        .await
        .unwrap();
    let pool = PgPoolOptions::new()
        .max_connections(2)
        .connect_with(opts.database(&name))
        .await
        .unwrap();
    sqlx::raw_sql(UPSTREAM_IMAGES_DDL)
        .execute(&pool)
        .await
        .unwrap();
    Some(Source { admin, name, pool })
}

impl Source {
    async fn drop(self) {
        self.pool.close().await;
        sqlx::query(sqlx::AssertSqlSafe(format!(
            "DROP DATABASE \"{}\" WITH (FORCE)",
            self.name
        )))
        .execute(&self.admin)
        .await
        .unwrap();
    }

    #[allow(clippy::too_many_arguments)]
    async fn insert(
        &self,
        id: Uuid,
        user_address: &str,
        url: &str,
        thumbnail_url: &str,
        metadata: &Value,
        created_at: Option<&str>,
        is_public: bool,
    ) {
        sqlx::query(
            "INSERT INTO images (id, user_address, url, thumbnail_url, metadata, created_at, is_public) \
             VALUES ($1, $2, $3, $4, $5, $6::timestamp, $7)",
        )
        .bind(id)
        .bind(user_address)
        .bind(url)
        .bind(thumbnail_url)
        .bind(metadata)
        .bind(created_at)
        .bind(is_public)
        .execute(&self.pool)
        .await
        .unwrap();
    }
}

#[derive(Clone, Default)]
struct Bucket {
    objects: Arc<Mutex<HashMap<String, Vec<u8>>>>,
    /// Keys that answer 503 this many more times before their object.
    flaky: Arc<Mutex<HashMap<String, u32>>>,
    requests: Arc<Mutex<Vec<String>>>,
}

async fn object(
    AxumState(bucket): AxumState<Bucket>,
    Path(key): Path<String>,
) -> impl IntoResponse {
    bucket.requests.lock().unwrap().push(key.clone());
    if let Some(left) = bucket.flaky.lock().unwrap().get_mut(&key)
        && *left > 0
    {
        *left -= 1;
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    }
    match bucket.objects.lock().unwrap().get(&key) {
        Some(bytes) => ([(header::CONTENT_TYPE, "image/png")], bytes.clone()).into_response(),
        // A public S3 bucket answers 403 for a key it does not hold.
        None => StatusCode::FORBIDDEN.into_response(),
    }
}

async fn serve_bucket(bucket: Bucket) -> url::Url {
    let app = axum::Router::new()
        .route("/bucket/{key}", axum::routing::get(object))
        .with_state(bucket);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    format!("http://{addr}/bucket/").parse().unwrap()
}

fn document(owner: &str, place: &str) -> Value {
    json!({
        "userName": "imported",
        "userAddress": owner,
        "dateTime": "1700000000",
        "realm": "main",
        "scene": { "name": "Genesis Plaza", "location": { "x": "0", "y": "0" } },
        "visiblePeople": [],
        "placeId": place
    })
}

async fn import(
    source: &Source,
    s: &Scratch,
    bucket_url: &url::Url,
    dry_run: bool,
    prune: bool,
) -> ImportReport {
    run(
        &source.pool,
        s.db().pool(),
        &s.state.storage,
        &dcl_http::Outbound::for_tests(),
        &ImportOptions {
            bucket_url: bucket_url.clone(),
            batch: 2,
            concurrency: 3,
            dry_run,
            prune,
        },
    )
    .await
    .unwrap()
}

#[derive(Debug, PartialEq, sqlx::FromRow)]
struct Row {
    user_address: String,
    image_hash: String,
    thumbnail_hash: String,
    is_public: bool,
    review_status: String,
    ranking_published: bool,
    metadata: Value,
    created_at: chrono::NaiveDateTime,
}

async fn row(pool: &PgPool, id: Uuid) -> Option<Row> {
    sqlx::query_as(
        "SELECT user_address, image_hash, thumbnail_hash, is_public, review_status, \
                ranking_published, metadata, created_at \
         FROM camera_reel.images WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .unwrap()
}

fn skipped(report: &ImportReport) -> Vec<(Uuid, &'static str)> {
    let mut out: Vec<_> = report
        .skips
        .iter()
        .map(|s| (s.id, s.reason.as_str()))
        .collect();
    out.sort();
    out
}

#[tokio::test]
async fn the_import_copies_the_old_service_and_keeps_its_urls() {
    let Some(mut s) = scratch(Settings::new(API_URL)).await else {
        return;
    };
    let source = source_database().await.unwrap();
    let bucket = Bucket::default();
    let bucket_url = serve_bucket(bucket.clone()).await;
    let put = |key: String, bytes: &[u8]| {
        bucket.objects.lock().unwrap().insert(key, bytes.to_vec());
    };

    // The owner as the old service stored it: a checksummed address
    // (lowercased on import) whose document names it in lowercase.
    let checksummed = s
        .owner(&random_address())
        .to_uppercase()
        .replacen("0X", "0x", 1);
    let owner = checksummed.to_lowercase();
    let place = Uuid::new_v4().to_string();
    let ids: Vec<Uuid> = (0..10).map(|_| Uuid::new_v4()).collect();
    let [
        full,
        no_thumb_url,
        thumb_missing,
        image_missing,
        wrong_owner,
        not_object,
        flaky,
        undated,
        bad_shape,
        uploaded_here,
    ] = ids[..].try_into().unwrap();
    let image_full = png(64, 48, 1);
    let thumb_full = png(32, 24, 2);
    let image_b = png(1280, 720, 3);
    let image_c = png(1280, 720, 4);
    let image_g = png(16, 16, 5);
    let thumb_g = png(8, 8, 6);
    put(format!("{full}-shot.png"), &image_full);
    put(format!("{full}-thumbnail-shot.png"), &thumb_full);
    put(format!("{no_thumb_url}-shot.png"), &image_b);
    put(format!("{thumb_missing}-shot.png"), &image_c);
    put(format!("{flaky}-a b.png"), &image_g);
    put(format!("{flaky}-thumbnail-a b.png"), &thumb_g);
    bucket
        .flaky
        .lock()
        .unwrap()
        .insert(format!("{flaky}-a b.png"), 1);
    let urls = |id: Uuid, file: &str| {
        (
            format!("{OLD_API}/{id}-{file}"),
            format!("{OLD_API}/{id}-thumbnail-{file}"),
        )
    };
    let doc = document(&owner, &place);
    let at = |n: u32| format!("2024-01-0{n} 10:00:00.123456");
    let (u, t) = urls(full, "shot.png");
    source
        .insert(full, &checksummed, &u, &t, &doc, Some(&at(1)), true)
        .await;
    let (u, _) = urls(no_thumb_url, "shot.png");
    source
        .insert(no_thumb_url, &owner, &u, "", &doc, Some(&at(2)), true)
        .await;
    let (u, t) = urls(thumb_missing, "shot.png");
    source
        .insert(thumb_missing, &owner, &u, &t, &doc, Some(&at(2)), false)
        .await;
    let (u, t) = urls(image_missing, "shot.png");
    source
        .insert(image_missing, &owner, &u, &t, &doc, Some(&at(3)), true)
        .await;
    let (u, t) = urls(wrong_owner, "shot.png");
    source
        .insert(
            wrong_owner,
            &owner,
            &u,
            &t,
            &document(&random_address(), &place),
            Some(&at(3)),
            true,
        )
        .await;
    let (u, t) = urls(not_object, "shot.png");
    source
        .insert(not_object, &owner, &u, &t, &json!([]), Some(&at(4)), true)
        .await;
    let (u, t) = urls(flaky, "a b.png");
    source
        .insert(flaky, &owner, &u, &t, &doc, Some(&at(5)), true)
        .await;
    let (u, t) = urls(undated, "shot.png");
    source
        .insert(undated, &owner, &u, &t, &doc, None, true)
        .await;
    let (u, t) = urls(bad_shape, "shot.png");
    source
        .insert(
            bad_shape,
            &owner,
            &u,
            &t,
            &json!({ "userAddress": owner }),
            Some(&at(6)),
            true,
        )
        .await;

    let expected_skips = {
        let mut v = vec![
            (image_missing, "image_missing"),
            (wrong_owner, "owner_mismatch"),
            (not_object, "metadata_not_object"),
            (undated, "created_at_missing"),
            (bad_shape, "metadata_shape"),
        ];
        v.sort();
        v
    };

    // A dry run fetches and checks, and writes nothing.
    let dry = import(&source, &s, &bucket_url, true, false).await;
    assert_eq!(
        (dry.read, dry.imported, dry.thumbnails_generated),
        (9, 4, 2)
    );
    assert_eq!(skipped(&dry), expected_skips);
    for id in &ids {
        assert!(row(s.db().pool(), *id).await.is_none());
    }
    bucket
        .flaky
        .lock()
        .unwrap()
        .insert(format!("{flaky}-a b.png"), 1);

    let first = import(&source, &s, &bucket_url, false, false).await;
    assert_eq!(
        (first.read, first.imported, first.updated, first.unchanged),
        (9, 4, 0, 0),
        "{first:#?}"
    );
    assert_eq!(first.thumbnails_generated, 2);
    assert_eq!(skipped(&first), expected_skips);
    assert_eq!(first.skipped["image_missing"], 1);
    assert!(first.bytes_fetched > 0 && first.bytes_stored > 0);

    let pool = s.db().pool().clone();
    let a = row(&pool, full).await.unwrap();
    assert_eq!(
        a,
        Row {
            user_address: owner.clone(),
            image_hash: hash_bytes_v1(&image_full),
            thumbnail_hash: hash_bytes_v1(&thumb_full),
            is_public: true,
            review_status: "ok".into(),
            ranking_published: true,
            metadata: doc.clone(),
            created_at: chrono::NaiveDateTime::parse_from_str(&at(1), "%Y-%m-%d %H:%M:%S%.f")
                .unwrap(),
        }
    );
    for (id, image) in [(no_thumb_url, &image_b), (thumb_missing, &image_c)] {
        let r = row(&pool, id).await.unwrap();
        assert_eq!(r.image_hash, hash_bytes_v1(image));
        let generated = crate::handlers::images::thumbnail_of(image).unwrap();
        assert_eq!(r.thumbnail_hash, hash_bytes_v1(&generated));
        let thumb = image::load_from_memory(&generated).unwrap();
        assert_eq!((thumb.width(), thumb.height()), (640, 360));
    }
    assert!(!row(&pool, thumb_missing).await.unwrap().is_public);
    let g = row(&pool, flaky).await.unwrap();
    assert_eq!(g.thumbnail_hash, hash_bytes_v1(&thumb_g));
    for id in [full, no_thumb_url, thumb_missing, flaky] {
        let r = row(&pool, id).await.unwrap();
        for hash in [&r.image_hash, &r.thumbnail_hash] {
            assert!(s.state.storage.exist(hash).await.unwrap(), "{id} {hash}");
        }
    }
    for id in [image_missing, wrong_owner, not_object, undated, bad_shape] {
        assert!(row(&pool, id).await.is_none(), "{id}");
    }

    // An import is not an upload: no photo event, no ranking count, nothing
    // for the ranking repair to publish.
    let events: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM camera_reel.photo_events WHERE photo_id = ANY($1)",
    )
    .bind(&ids)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(events, 0);
    let receipts: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM social.community_ranking_events WHERE event_key = ANY($1)",
    )
    .bind(ids.iter().map(Uuid::to_string).collect::<Vec<_>>())
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(receipts, 0);
    let pending = s.db().pending_rankings(Uuid::nil()).await.unwrap();
    assert!(pending.iter().all(|(id, ..)| !ids.contains(id)));

    // The second pass fetches nothing new and changes nothing.
    let fetched_before = bucket.requests.lock().unwrap().len();
    let second = import(&source, &s, &bucket_url, false, false).await;
    assert_eq!(
        (
            second.read,
            second.imported,
            second.updated,
            second.unchanged
        ),
        (9, 0, 0, 4),
        "{second:#?}"
    );
    assert_eq!(skipped(&second), expected_skips);
    assert_eq!(second.bytes_stored, 0);
    // Only the rows that still fail are asked for again.
    let asked: Vec<String> = bucket.requests.lock().unwrap()[fetched_before..].to_vec();
    assert!(
        asked
            .iter()
            .all(|k| k.starts_with(&image_missing.to_string())),
        "{asked:?}"
    );
    assert_eq!(row(&pool, full).await.unwrap(), a);

    // The legacy URLs resolve to the imported blobs, by the hash route's rules.
    let app = s.app();
    let get = |key: String| {
        let app = app.clone();
        async move {
            send(
                app,
                "GET",
                &format!("{PREFIX}/api/images/{key}"),
                vec![],
                None,
            )
            .await
        }
    };
    let (status, headers, body) = get(format!("{full}-shot.png")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers[header::CONTENT_TYPE], "image/png");
    assert_eq!(body, image_full);
    let (status, _, body) = get(format!("{full}-thumbnail-shot.png")).await;
    assert_eq!((status, body), (StatusCode::OK, thumb_full.clone()));
    let (status, _, body) = get(format!("{flaky}-a%20b.png")).await;
    assert_eq!((status, body), (StatusCode::OK, image_g.clone()));
    // The file part is not stored, so any file after the id resolves.
    let (status, _, body) = get(format!("{full}-1790074770.jpg")).await;
    assert_eq!((status, body), (StatusCode::OK, image_full.clone()));
    let (status, _, body) = get(a.image_hash.clone()).await;
    assert_eq!((status, body), (StatusCode::OK, image_full.clone()));
    for missing in [
        format!("{image_missing}-shot.png"),
        format!("{}-shot.png", Uuid::new_v4()),
        full.to_string(),
    ] {
        let (status, _, body) = get(missing.clone()).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{missing}");
        assert_eq!(
            serde_json::from_slice::<Value>(&body).unwrap(),
            json!({ "message": "image not found" })
        );
    }
    // With the store's bucket configured, the redirect names the hash.
    let mut with_bucket = Settings::new(API_URL);
    with_bucket.bucket_url = Some("https://blobs.example/store/".into());
    let redirecting = axum::Router::new().nest(
        PREFIX,
        router(Arc::new(State::new(
            pool.clone(),
            s.state.storage.clone(),
            with_bucket,
            None,
        ))),
    );
    let (status, headers, _) = send(
        redirecting,
        "GET",
        &format!("{PREFIX}/api/images/{full}-thumbnail-shot.png"),
        vec![],
        None,
    )
    .await;
    assert_eq!(status, StatusCode::TEMPORARY_REDIRECT);
    assert_eq!(
        headers[header::LOCATION],
        format!("https://blobs.example/store/{}", a.thumbnail_hash).as_str()
    );

    // The source stays the authority for visibility and the document of an
    // unmoderated imported row; a moderated one and a photo uploaded here
    // are not the source's to change.
    let moved = Uuid::new_v4().to_string();
    sqlx::query("UPDATE images SET is_public = false, metadata = $2 WHERE id = $1")
        .bind(full)
        .bind(document(&owner, &moved))
        .execute(&source.pool)
        .await
        .unwrap();
    sqlx::query("UPDATE images SET is_public = false WHERE id = $1")
        .bind(no_thumb_url)
        .execute(&source.pool)
        .await
        .unwrap();
    sqlx::query("UPDATE camera_reel.images SET review_status = 'rejected' WHERE id = $1")
        .bind(no_thumb_url)
        .execute(&pool)
        .await
        .unwrap();
    let mut here = super::image(
        &owner,
        &s.hash(&hash_bytes_v1(b"here")),
        &hash_bytes_v1(b"here-t"),
        true,
        &place,
    );
    here.id = uploaded_here;
    s.db().insert_image(&here).await.unwrap();
    let (u, t) = urls(uploaded_here, "shot.png");
    source
        .insert(uploaded_here, &owner, &u, &t, &doc, Some(&at(7)), false)
        .await;

    let third = import(&source, &s, &bucket_url, false, false).await;
    assert_eq!(
        (
            third.read,
            third.imported,
            third.updated,
            third.unchanged,
            third.kept_moderated
        ),
        (10, 0, 1, 2, 1),
        "{third:#?}"
    );
    let a3 = row(&pool, full).await.unwrap();
    assert!(!a3.is_public);
    assert_eq!(a3.metadata["placeId"], moved.as_str());
    let b3 = row(&pool, no_thumb_url).await.unwrap();
    assert_eq!(
        (b3.is_public, b3.review_status.as_str()),
        (true, "rejected")
    );
    assert!(row(&pool, uploaded_here).await.unwrap().is_public);
    assert!(skipped(&third).contains(&(uploaded_here, "local_photo")));
    let events: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM camera_reel.photo_events WHERE photo_id = ANY($1)",
    )
    .bind(&ids)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(events, 0, "a refresh records no privacy event");

    // A photo the source deleted goes with --prune; one uploaded here stays.
    sqlx::query("DELETE FROM images WHERE id = ANY($1)")
        .bind(vec![thumb_missing, uploaded_here])
        .execute(&source.pool)
        .await
        .unwrap();
    let dry_prune = import(&source, &s, &bucket_url, true, true).await;
    assert_eq!(dry_prune.pruned, Some(1));
    assert!(row(&pool, thumb_missing).await.is_some());
    let c = row(&pool, thumb_missing).await.unwrap();
    s.hash(&c.image_hash);
    s.hash(&c.thumbnail_hash);
    let pruned = import(&source, &s, &bucket_url, false, true).await;
    assert_eq!(pruned.pruned, Some(1));
    assert!(row(&pool, thumb_missing).await.is_none());
    assert!(row(&pool, uploaded_here).await.is_some());
    assert_eq!(s.withheld(&c.image_hash).await.as_deref(), Some("deleted"));
    let provenance: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM camera_reel.imported_images WHERE id = ANY($1)")
            .bind(&ids)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(provenance, 3, "full, no_thumb_url and flaky stay imported");

    for r in [&a, &g] {
        s.hash(&r.image_hash);
        s.hash(&r.thumbnail_hash);
    }
    source.drop().await;
    s.done().await;
}
