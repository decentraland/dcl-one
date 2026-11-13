use super::*;

#[tokio::test]
async fn image_urls_redirect_without_a_database_or_existing_object() {
    let pool = sqlx::postgres::PgPoolOptions::new()
        .connect_lazy("postgres://localhost/camera_redirect_test")
        .unwrap();
    pool.close().await;
    let directory = std::env::temp_dir().join(format!("camera-redirect-{}", Uuid::new_v4()));
    let storage = Arc::new(Storage::from_config(&directory).await.unwrap());
    for bucket in [
        None,
        Some("https://bucket.example"),
        Some("https://bucket.example/"),
    ] {
        let mut settings = Settings::new(API_URL);
        settings.bucket_url = bucket.map(str::to_owned);
        let state = Arc::new(State::new(pool.clone(), storage.clone(), settings, None));
        let app = Router::new().nest(PREFIX, router(state));
        for name in [
            "absent",
            "00000000-0000-0000-0000-000000000000",
            "00000000-0000-0000-0000-000000000000-shot.png",
        ] {
            let (status, headers, body) = send(
                app.clone(),
                "GET",
                &format!("{PREFIX}/api/images/{name}"),
                vec![],
                None,
            )
            .await;
            assert_eq!(status, StatusCode::TEMPORARY_REDIRECT);
            assert!(body.is_empty());
            let target = match bucket {
                Some(bucket) => format!("{bucket}/{name}"),
                None => format!("{API_URL}/api/files/{name}"),
            };
            assert_eq!(headers[header::LOCATION], target);
        }
    }
    tokio::fs::remove_dir_all(directory).await.unwrap();
}

async fn follow_photo(app: Router, url: &str) -> (StatusCode, Vec<u8>) {
    let path = url.strip_prefix("http://127.0.0.1:5149").unwrap();
    let (status, headers, body) = send(app.clone(), "GET", path, vec![], None).await;
    assert_eq!(status, StatusCode::TEMPORARY_REDIRECT);
    assert!(body.is_empty());
    let target = headers[header::LOCATION]
        .to_str()
        .unwrap()
        .strip_prefix("http://127.0.0.1:5149")
        .unwrap();
    let (status, _, body) = send(app, "GET", target, vec![], None).await;
    (status, body)
}

#[tokio::test]
async fn uploaded_filename_aliases_follow_to_immutable_bytes_and_obey_moderation() {
    let mut settings = Settings::new(API_URL);
    settings.writes_enabled = true;
    settings.admin_token = Some("moderator-secret".to_string());
    let Some(mut s) = scratch(settings).await else {
        return;
    };
    let owner_wallet = wallet();
    let owner = s.owner(&owner_wallet.address);
    let bytes = png(1280, 720, 19);
    let (mut body, content_type) =
        multipart(&bytes, "image/png", &metadata_json(&owner, ""), Some(false));
    let start = body
        .windows(b"shot.png".len())
        .position(|bytes| bytes == b"shot.png")
        .unwrap();
    body.splice(
        start..start + b"shot.png".len(),
        b"photo-123.png".iter().copied(),
    );
    let path = format!("{PREFIX}/api/images");
    let (status, _, body) = send(
        s.app(),
        "POST",
        &path,
        signed_headers(&owner_wallet, "post", &path, true).await,
        Some((body, &content_type)),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let uploaded = json_of(&body);
    let id = uploaded["image"]["id"].as_str().unwrap();
    let image_url = uploaded["image"]["url"].as_str().unwrap();
    let thumb_url = uploaded["image"]["thumbnailUrl"].as_str().unwrap();
    assert_eq!(
        image_url,
        format!("{API_URL}/api/images/{id}-photo-123.png")
    );
    assert_eq!(
        thumb_url,
        format!("{API_URL}/api/images/{id}-thumbnail-photo-123.png")
    );
    let stored = s.db().get_image(id).await.unwrap();
    s.hash(&stored.image_hash);
    s.hash(&stored.thumbnail_hash);
    assert_eq!(stored.url, image_url);
    assert_eq!(stored.thumbnail_url, thumb_url);
    let filename: Option<String> =
        sqlx::query_scalar("SELECT original_filename FROM camera_reel.images WHERE id=$1")
            .bind(stored.id)
            .fetch_one(s.db().pool())
            .await
            .unwrap();
    assert_eq!(filename.as_deref(), Some("photo-123.png"));
    assert_eq!(
        follow_photo(s.app(), image_url).await,
        (StatusCode::OK, bytes.clone())
    );
    let (status, thumbnail) = follow_photo(s.app(), thumb_url).await;
    assert_eq!(status, StatusCode::OK);
    let decoded = image::load_from_memory(&thumbnail).unwrap();
    assert_eq!((decoded.width(), decoded.height()), (640, 360));
    let legacy_url = format!("{API_URL}/api/images/{}", stored.image_hash);
    assert_eq!(
        follow_photo(s.app(), &legacy_url).await,
        (StatusCode::OK, bytes.clone())
    );

    // An altered basename is not an alias, and an unrelated blob in shared
    // storage cannot be reached by inventing a camera URL for its hash.
    let forged = format!("{API_URL}/api/images/{id}-other.png");
    assert_eq!(
        follow_photo(s.app(), &forged).await.0,
        StatusCode::NOT_FOUND
    );
    let unrelated = hash_bytes_v1(b"unrelated content");
    s.state
        .storage
        .store(&unrelated, bytes::Bytes::from_static(b"unrelated content"))
        .await
        .unwrap();
    assert_eq!(
        follow_photo(s.app(), &format!("{API_URL}/api/images/{unrelated}"))
            .await
            .0,
        StatusCode::NOT_FOUND
    );

    let review_path = format!("{PREFIX}/admin/images/{id}/review");
    let admin = vec![(
        "authorization".to_string(),
        "Bearer moderator-secret".to_string(),
    )];
    for verdict in ["rejected", "ok"] {
        let review = json!({"reviewStatus":verdict}).to_string().into_bytes();
        let (status, _, _) = send(
            s.app(),
            "PATCH",
            &review_path,
            admin.clone(),
            Some((review, "application/json")),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let expected = if verdict == "rejected" {
            StatusCode::NOT_FOUND
        } else {
            StatusCode::OK
        };
        assert_eq!(follow_photo(s.app(), image_url).await.0, expected);
        assert_eq!(follow_photo(s.app(), thumb_url).await.0, expected);
    }
    // An operator denylist entry applies after resolving a filename to its hash.
    s.state.denylist.add(&stored.image_hash);
    assert_eq!(
        follow_photo(s.app(), image_url).await.0,
        StatusCode::NOT_FOUND
    );
    s.state.denylist.remove(&stored.image_hash);
    let delete = format!("{PREFIX}/api/images/{id}");
    let (status, _, _) = send(
        s.app(),
        "DELETE",
        &delete,
        signed_headers(&owner_wallet, "delete", &delete, true).await,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        follow_photo(s.app(), image_url).await.0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        follow_photo(s.app(), thumb_url).await.0,
        StatusCode::NOT_FOUND
    );
    assert!(s.state.storage.exist(&stored.image_hash).await.unwrap());
    s.done().await;
}

#[tokio::test]
async fn filename_migration_preserves_existing_hash_urls_and_can_be_reapplied() {
    let Some(pool) = catalyst_db::scratch::isolated_pool(4).await else {
        return;
    };
    // This database belongs only to this test. Model the pre-migration table
    // before inserting the photo, so the upgrade must preserve an existing row.
    sqlx::query("ALTER TABLE camera_reel.images DROP COLUMN original_filename")
        .execute(&pool)
        .await
        .unwrap();
    let directory = std::env::temp_dir().join(format!("camera-migration-{}", Uuid::new_v4()));
    let storage = Arc::new(Storage::from_config(&directory).await.unwrap());
    let state = Arc::new(State::new(pool, storage, Settings::new(API_URL), None));
    let mut s = Scratch {
        state,
        owners: Vec::new(),
        hashes: Vec::new(),
        dir: directory,
    };
    let owner = s.owner(&random_address());
    let bytes = png(32, 32, 29);
    let hash = s.hash(&hash_bytes_v1(&bytes));
    s.state
        .storage
        .store(&hash, bytes.clone().into())
        .await
        .unwrap();
    let old = image(&owner, &hash, &hash, true, "");
    s.db().insert_image(&old).await.unwrap();
    const MIGRATION: &str = include_str!(concat!(
        env!("DCL_ONE_ROOT"),
        "/schemas/migrations/0040_camera_reel_original_filename.sql"
    ));
    sqlx::raw_sql(MIGRATION)
        .execute(s.db().pool())
        .await
        .unwrap();
    let upgraded = s.db().get_image(&old.id.to_string()).await.unwrap();
    let old_url = format!("{API_URL}/api/images/{hash}");
    assert_eq!(upgraded.url, old_url);
    assert_eq!(upgraded.thumbnail_url, old_url);
    assert_eq!(
        follow_photo(s.app(), &old_url).await,
        (StatusCode::OK, bytes.clone())
    );
    let filename: Option<String> =
        sqlx::query_scalar("SELECT original_filename FROM camera_reel.images WHERE id=$1")
            .bind(old.id)
            .fetch_one(s.db().pool())
            .await
            .unwrap();
    assert!(filename.is_none());

    let named = image(&owner, &hash, &hash, true, "");
    assert_eq!(
        s.db()
            .insert_image_within_limit_named(&named, 500, Some("new.png"))
            .await
            .unwrap(),
        Some(2)
    );
    sqlx::raw_sql(MIGRATION)
        .execute(s.db().pool())
        .await
        .unwrap();
    assert_eq!(
        s.db().get_image(&old.id.to_string()).await.unwrap().url,
        old_url
    );
    let new_url = format!("{API_URL}/api/images/{}-new.png", named.id);
    assert_eq!(
        s.db().get_image(&named.id.to_string()).await.unwrap().url,
        new_url
    );
    assert_eq!(
        follow_photo(s.app(), &new_url).await,
        (StatusCode::OK, bytes)
    );
    assert_eq!(s.count(&owner).await, 2);
    s.done().await;
}

#[tokio::test]
async fn filename_urls_escape_reserved_bytes_once_for_upload_gallery_and_redirects() {
    let mut settings = Settings::new(API_URL);
    settings.writes_enabled = true;
    let Some(mut s) = scratch(settings).await else {
        return;
    };
    let owner_wallet = wallet();
    let owner = s.owner(&owner_wallet.address);
    let bytes = png(12, 8, 117);
    for (filename, encoded) in [
        ("shot#part.png", "shot%23part.png"),
        ("shot?size.png", "shot%3Fsize.png"),
        ("shot%2Fname.png", "shot%252Fname.png"),
        ("shot name.png", "shot%20name.png"),
        ("café.png", "caf%C3%A9.png"),
    ] {
        let (mut body, content_type) =
            multipart(&bytes, "image/png", &metadata_json(&owner, ""), Some(true));
        let start = body
            .windows(b"shot.png".len())
            .position(|bytes| bytes == b"shot.png")
            .unwrap();
        body.splice(
            start..start + b"shot.png".len(),
            filename.as_bytes().iter().copied(),
        );
        let path = format!("{PREFIX}/api/images");
        let (status, _, body) = send(
            s.app(),
            "POST",
            &path,
            signed_headers(&owner_wallet, "post", &path, true).await,
            Some((body, &content_type)),
        )
        .await;
        assert_eq!(
            status,
            StatusCode::OK,
            "{filename}: {}",
            String::from_utf8_lossy(&body)
        );
        let uploaded = json_of(&body);
        let id = uploaded["image"]["id"].as_str().unwrap();
        let url = uploaded["image"]["url"].as_str().unwrap();
        assert_eq!(url, format!("{API_URL}/api/images/{id}-{encoded}"));
        let stored = s.db().get_image(id).await.unwrap();
        s.hash(&stored.image_hash);
        s.hash(&stored.thumbnail_hash);
        assert_eq!(stored.url, url);
        let page = s
            .db()
            .get_user_images_page(&owner, &Page::Offset(0), 100, false)
            .await
            .unwrap();
        assert_eq!(
            page.images
                .iter()
                .find(|image| image.id.to_string() == id)
                .unwrap()
                .url,
            url
        );
        assert_eq!(
            follow_photo(s.app(), url).await,
            (StatusCode::OK, bytes.clone())
        );
    }
    s.done().await;
}

#[tokio::test]
async fn configured_bucket_redirects_local_aliases_to_stored_hash_keys() {
    let mut settings = Settings::new(API_URL);
    settings.writes_enabled = true;
    settings.bucket_url = Some("https://bucket.example".into());
    let Some(mut s) = scratch(settings).await else {
        return;
    };
    let owner_wallet = wallet();
    let owner = s.owner(&owner_wallet.address);
    let bytes = png(12, 8, 118);
    let (body, content_type) =
        multipart(&bytes, "image/png", &metadata_json(&owner, ""), Some(false));
    let path = format!("{PREFIX}/api/images");
    let (status, _, body) = send(
        s.app(),
        "POST",
        &path,
        signed_headers(&owner_wallet, "post", &path, true).await,
        Some((body, &content_type)),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let uploaded = json_of(&body);
    let id = uploaded["image"]["id"].as_str().unwrap();
    let stored = s.db().get_image(id).await.unwrap();
    s.hash(&stored.image_hash);
    s.hash(&stored.thumbnail_hash);
    for (url, hash) in [
        (&stored.url, &stored.image_hash),
        (&stored.thumbnail_url, &stored.thumbnail_hash),
    ] {
        let path = url.strip_prefix("http://127.0.0.1:5149").unwrap();
        let (status, headers, _) = send(s.app(), "GET", path, vec![], None).await;
        assert_eq!(status, StatusCode::TEMPORARY_REDIRECT);
        assert_eq!(
            headers[header::LOCATION],
            format!("https://bucket.example/{hash}")
        );
        assert!(s.state.storage.exist(hash).await.unwrap());
    }
    let (status, headers, _) = send(
        s.app(),
        "GET",
        &format!("{PREFIX}/api/images/absent%23file.png"),
        vec![],
        None,
    )
    .await;
    assert_eq!(status, StatusCode::TEMPORARY_REDIRECT);
    assert_eq!(
        headers[header::LOCATION],
        "https://bucket.example/absent%23file.png"
    );
    s.done().await;
}
