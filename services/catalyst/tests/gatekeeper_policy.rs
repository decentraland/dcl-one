//! The gatekeeper's tests that need the SQL-backed worlds permission core
//! and room resolver (`worlds::policy::Policy`, `worlds::rooms::SceneResolver`),
//! which `catalyst-comms` does not link: scene ownership and admin decisions,
//! world access and scene resolution, the stream key of an administered
//! place, the world comms mints and the relay authority's room policy. They
//! moved here from the gatekeeper's unit tests with their names and
//! assertions, and run against the lib's scratch database
//! (`CATALYST_TEST_DATABASE_URL`; skipped without it) with uuid-scoped rows.

/// From the gatekeeper's contract tests (`gatekeeper/tests.rs`): scene
/// ownership and admin decisions, world access and scene resolution.
mod tests {
    use std::sync::Arc;

    use axum::Json;
    use axum::extract::{Query, State};
    use axum::http::StatusCode;
    use sqlx::PgPool;
    use uuid::Uuid;

    use catalyst::comms::Address;
    use catalyst::comms::gatekeeper::admission::scene::{self, PlaceLookup};
    use catalyst::comms::gatekeeper::{
        CommsState, Moderation, Settings, SharedState, admission, moderation,
    };
    use catalyst::http::Outbound;
    use catalyst::shared::test_keys::{KEY, address_of, signed};
    use catalyst::worlds::policy::{PersonalWorldsPolicy, Policy};
    use catalyst::worlds::rooms::SceneResolver;

    fn wallet() -> Address {
        Address::parse(&format!("0x00000000{}", Uuid::new_v4().simple())).unwrap()
    }

    fn tag() -> String {
        Uuid::new_v4().simple().to_string()[..8].to_string()
    }

    async fn scratch() -> Option<PgPool> {
        catalyst::db::scratch::pool(8).await
    }

    /// The `OriginalUri` of a handler called in process: the route at the root.
    fn at(route: &str) -> axum::extract::OriginalUri {
        axum::extract::OriginalUri(route.parse().unwrap())
    }

    /// The gatekeeper over the SQL-backed permission core and room resolver,
    /// as the old `CommsState::new` built them (no personal worlds, no
    /// marketplace).
    fn state_with(pool: PgPool, settings: Settings) -> SharedState {
        Arc::new(CommsState::new(
            pool.clone(),
            Outbound::for_tests(),
            settings,
            None,
            None,
            Arc::new(Policy::new(
                pool.clone(),
                PersonalWorldsPolicy::disabled(),
                None,
            )),
            Arc::new(SceneResolver::new(pool)),
            Arc::new(catalyst::comms::presence::Presence::new()),
            Arc::new(catalyst::comms::interfaces::test_support::StubDirectory::default()),
        ))
    }

    async fn seed_place(pool: &PgPool, id: &str, raw: serde_json::Value, base_position: &str) {
        sqlx::query(
            "INSERT INTO explore.place (id, base_position, raw) VALUES ($1, $2, $3::jsonb)",
        )
        .bind(id)
        .bind(base_position)
        .bind(raw)
        .execute(pool)
        .await
        .unwrap();
    }

    async fn seed_world_scene(pool: &PgPool, world: &str, entity_id: &str, parcels: &[&str]) {
        sqlx::query("INSERT INTO worlds.worlds (name) VALUES ($1) ON CONFLICT DO NOTHING")
            .bind(world)
            .execute(pool)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO worlds.world_scenes (world_name, entity_id, deployment_auth_chain, entity, deployer, parcels, size, mirror_source, created_at) \
             VALUES ($1, $2, '[]'::json, '{}'::jsonb, $3, $4, 1, NULL, timezone('UTC', now()) + ($5 || ' seconds')::interval)",
        )
        .bind(world)
        .bind(entity_id)
        .bind(catalyst::shared::identity::ZERO_ADDRESS)
        .bind(parcels.iter().map(|p| p.to_string()).collect::<Vec<_>>())
        .bind(entity_id.len().to_string())
        .execute(pool)
        .await
        .unwrap();
    }

    fn meta(realm: &str, scene: &str, device: Option<&str>) -> String {
        let mut m =
            serde_json::json!({ "signer": "dcl:explorer", "realmName": realm, "sceneId": scene });
        if let Some(d) = device {
            m["deviceIdentifier"] = serde_json::json!(d);
        }
        m.to_string()
    }

    fn writes_on() -> Option<axum::Extension<catalyst::comms::writes::Switch>> {
        Some(axum::Extension(catalyst::comms::writes::Switch(true)))
    }

    /// A marketplace subgraph on the loopback answering `parcelsAt` from a fixed
    /// parcel list (any other document gets an empty page), or, when `fail` is
    /// set, HTTP 500 with `Retry-After: 0`, so the reader spends its retries
    /// without waiting through the backoff ladder.
    async fn fake_land_subgraph(
        parcels: serde_json::Value,
        fail: Arc<std::sync::atomic::AtomicBool>,
    ) -> String {
        use axum::Router;
        use axum::response::IntoResponse;
        use axum::routing::post;
        async fn answer(
            State((parcels, fail)): State<(
                Arc<serde_json::Value>,
                Arc<std::sync::atomic::AtomicBool>,
            )>,
            Json(body): Json<serde_json::Value>,
        ) -> axum::response::Response {
            if fail.load(std::sync::atomic::Ordering::Relaxed) {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    [(axum::http::header::RETRY_AFTER, "0")],
                    "boom",
                )
                    .into_response();
            }
            let query = body["query"].as_str().unwrap_or("");
            let nfts = if query.contains("parcelsAt") {
                let xs: Vec<String> = body["variables"]["xs"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_str().unwrap().to_string())
                    .collect();
                let ys: Vec<String> = body["variables"]["ys"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_str().unwrap().to_string())
                    .collect();
                parcels
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter(|p| {
                        xs.contains(&p["searchParcelX"].as_str().unwrap().to_string())
                            && ys.contains(&p["searchParcelY"].as_str().unwrap().to_string())
                    })
                    .cloned()
                    .collect::<Vec<_>>()
            } else {
                Vec::new()
            };
            Json(serde_json::json!({ "data": { "nfts": nfts } })).into_response()
        }
        let app = Router::new()
            .route("/subgraph", post(answer))
            .with_state((Arc::new(parcels), fail));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        format!("http://{addr}/subgraph")
    }

    fn state_with_marketplace(pool: PgPool, subgraph: Option<String>) -> SharedState {
        let settings = catalyst::chain_index::IndexSettings {
            marketplace_subgraph_url: subgraph,
            ..catalyst::chain_index::IndexSettings::for_tests()
        };
        let market = catalyst::chain_index::ChainIndex::new(Outbound::for_tests(), settings);
        market.allow_loopback_for_tests();
        let market = Arc::new(market);
        Arc::new(CommsState::new(
            pool.clone(),
            Outbound::for_tests(),
            Settings::for_tests(),
            Some(market.clone()),
            None,
            Arc::new(Policy::new(
                pool.clone(),
                PersonalWorldsPolicy::disabled(),
                Some(market),
            )),
            Arc::new(SceneResolver::new(pool)),
            Arc::new(catalyst::comms::presence::Presence::new()),
            Arc::new(catalyst::comms::interfaces::test_support::StubDirectory::default()),
        ))
    }

    /// Genesis City moderation: a parcel owner and an estate owner are scene
    /// owners without an admin row (so they can add the first admin, ban and
    /// unban), a bystander is refused, a reader that cannot be asked is 503.
    #[tokio::test]
    async fn a_land_owner_moderates_a_genesis_scene_without_an_admin_row() {
        let Some(pool) = scratch().await else { return };
        let (parcel_owner, estate_owner, bystander, first_admin) =
            (wallet(), wallet(), wallet(), wallet());
        let fail = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let subgraph = fake_land_subgraph(
            serde_json::json!([
                { "id": "0xland-a", "searchParcelX": "10", "searchParcelY": "20", "owner": { "id": parcel_owner.as_str().to_uppercase() }, "parcel": { "estate": null } },
                { "id": "0xland-b", "searchParcelX": "10", "searchParcelY": "21", "owner": { "id": "0xestatecontract" }, "parcel": { "estate": { "owner": { "id": estate_owner.as_str() } } } },
                { "id": "0xland-c", "searchParcelX": "11", "searchParcelY": "20", "owner": { "id": bystander.as_str() }, "parcel": { "estate": null } }
            ]),
            fail.clone(),
        )
        .await;
        let state = state_with_marketplace(pool.clone(), Some(subgraph));
        let place_id = format!("genesis-{}", tag());
        // The place covers (10,20) and (10,21); (11,20) is the bystander's parcel next door.
        seed_place(
            &pool,
            &place_id,
            serde_json::json!({ "positions": ["10,20", "10,21"] }),
            "10,20",
        )
        .await;
        let place = PlaceLookup::new(&state, &place_id);
        assert!(
            scene::scene_owner_or_admin_decision(&state, &place, &parcel_owner)
                .await
                .is_allowed(),
            "the parcel owner"
        );
        assert!(
            scene::scene_owner_or_admin_decision(&state, &place, &estate_owner)
                .await
                .is_allowed(),
            "the estate owner"
        );
        let refused = scene::scene_owner_or_admin_decision(&state, &place, &bystander)
            .await
            .into_result()
            .unwrap_err();
        assert_eq!(
            (refused.http_status(), refused.to_string().as_str()),
            (
                403,
                "refused: signer is not an owner or admin of this scene"
            )
        );
        // The owner adds the first admin, who then holds the same authority.
        state
            .moderation
            .add_admin(&place_id, &first_admin, &parcel_owner)
            .await
            .unwrap();
        assert!(
            scene::scene_owner_or_admin_decision(&state, &place, &first_admin)
                .await
                .is_allowed()
        );
        let err = scene::ensure_target_not_protected(&state, &place, &estate_owner)
            .await
            .expect_err("an owner cannot be banned");
        assert_eq!(err.code, 400);
        // A reader that cannot be asked is a 503, never a refusal (the cache
        // holds the answered set; a new place misses it).
        fail.store(true, std::sync::atomic::Ordering::Relaxed);
        let other_place = format!("genesis-{}", tag());
        seed_place(
            &pool,
            &other_place,
            serde_json::json!({ "positions": ["50,50"] }),
            "50,50",
        )
        .await;
        let other = PlaceLookup::new(&state, &other_place);
        let undetermined = scene::scene_owner_or_admin_decision(&state, &other, &bystander)
            .await
            .into_result()
            .unwrap_err();
        assert_eq!(undetermined.http_status(), 503, "{undetermined}");
        // No marketplace subgraph configured: an explicit refusal, as for worlds.
        let unconfigured = state_with_marketplace(pool.clone(), None);
        let place = PlaceLookup::new(&unconfigured, &place_id);
        assert!(
            !scene::scene_owner_or_admin_decision(&unconfigured, &place, &parcel_owner)
                .await
                .is_allowed()
        );
    }

    #[tokio::test]
    async fn scene_ban_rejects_admin_target_and_admin_decisions_are_explicit() {
        let Some(pool) = scratch().await else { return };
        let state = state_with(pool.clone(), Settings::for_tests());
        let place_id = format!("place-{}", tag());
        let (admin, owner, bystander) = (wallet(), wallet(), wallet());
        state
            .moderation
            .add_admin(&place_id, &admin, &owner)
            .await
            .unwrap();
        let place = PlaceLookup::new(&state, &place_id);
        let err = scene::ensure_target_not_protected(&state, &place, &admin)
            .await
            .expect_err("banning a scene admin must be rejected");
        assert_eq!(
            (err.code, err.message.as_str()),
            (400, "Cannot ban this address")
        );
        scene::ensure_target_not_protected(&state, &place, &bystander)
            .await
            .expect("a regular user remains bannable");
        assert!(
            scene::scene_owner_or_admin_decision(&state, &place, &admin)
                .await
                .is_allowed()
        );
        let refused = scene::scene_owner_or_admin_decision(&state, &place, &bystander)
            .await
            .into_result()
            .unwrap_err();
        assert_eq!(
            (refused.http_status(), refused.to_string().as_str()),
            (
                403,
                "refused: signer is not an owner or admin of this scene"
            )
        );
        let listed = state
            .moderation
            .list_active_admins(&place_id, None)
            .await
            .unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].admin, admin.as_str());
        assert_eq!(listed[0].added_by, owner.as_str());
        assert_eq!(
            state
                .moderation
                .remove_admin(&place_id, &admin)
                .await
                .unwrap(),
            1
        );
        assert!(
            state
                .moderation
                .list_active_admins(&place_id, None)
                .await
                .unwrap()
                .is_empty()
        );
        assert!(
            !scene::scene_owner_or_admin_decision(&state, &place, &admin)
                .await
                .is_allowed(),
            "a removed admin holds nothing"
        );
        state
            .moderation
            .add_admin(&place_id, &admin, &owner)
            .await
            .unwrap();
        assert_eq!(
            state
                .moderation
                .list_active_admins(&place_id, Some(&admin))
                .await
                .unwrap()
                .len(),
            1,
            "re-adding flips the same row back"
        );
        // A dead store is only undetermined once the pool gives up connecting. The
        // paused clock runs its connect retries and acquire timeout without waiting
        // through them; the one socket left is the refused one.
        tokio::time::pause();
        let down = Moderation::new(
            PgPool::connect_lazy("postgres://postgres@127.0.0.1:1/postgres").unwrap(),
        );
        let undetermined = down
            .admin_decision(&place_id, &admin)
            .await
            .into_result()
            .unwrap_err();
        assert_eq!(
            undetermined.http_status(),
            503,
            "a dead store is undetermined, never a denial"
        );
    }

    #[tokio::test]
    async fn a_world_realm_resolves_its_scene_from_the_active_deployment_and_needs_world_access() {
        let Some(pool) = scratch().await else { return };
        let state = state_with(pool.clone(), Settings::for_tests());
        let world = format!("{}.dcl.eth", tag());
        seed_world_scene(&pool, &world, "bafkreiolder", &["0,0"]).await;
        seed_world_scene(&pool, &world, "bafkreinewest12", &["0,0", "1,0"]).await;
        assert_eq!(
            state
                .scenes
                .world_active_scene(
                    &catalyst::comms::world_identity::WorldName::parse(&world.to_uppercase())
                        .unwrap()
                )
                .await
                .unwrap()
                .as_ref()
                .map(|dep| dep.scene_id.as_str()),
            Some("bafkreinewest12"),
            "the newest scene, as /about advertises"
        );
        let realm = catalyst::comms::world_identity::RealmId::parse(&world).unwrap();
        let parcel_request = |x, y| catalyst::comms::rooms::RoomRequest::Parcel {
            realm: &realm,
            parcel: catalyst::comms::world_identity::Parcel { x, y },
        };
        assert_eq!(
            state
                .scenes
                .resolve(parcel_request(1, 0))
                .await
                .unwrap()
                .scene_id
                .unwrap()
                .as_str(),
            "bafkreinewest12"
        );
        assert!(matches!(
            state.scenes.resolve(parcel_request(9, 9)).await,
            Err(catalyst::comms::rooms::ResolveError::NoSceneAtParcel { .. })
        ));
        assert_eq!(
            state
                .scenes
                .world_active_scene(
                    &catalyst::comms::world_identity::WorldName::parse("nobody.dcl.eth").unwrap()
                )
                .await
                .unwrap(),
            None
        );
        assert_eq!(
            scene::resolve_world_scene_id(&state, &world, &world)
                .await
                .unwrap(),
            "bafkreinewest12"
        );
        assert_eq!(
            scene::resolve_world_scene_id(&state, &world, "bafkreiolder")
                .await
                .unwrap(),
            "bafkreiolder",
            "an explicit deployed scene keeps its own room"
        );
        assert_eq!(
            scene::resolve_world_scene_id(&state, &world, "bafkreiexplicit")
                .await
                .unwrap_err()
                .code,
            404,
            "a scene not deployed in the world has no room"
        );
        assert_eq!(
            scene::resolve_world_scene_id(&state, "main", "bafkreiexplicit")
                .await
                .unwrap(),
            "bafkreiexplicit",
            "a shared realm's id is taken as given"
        );
        assert_eq!(
            scene::resolve_world_scene_id(&state, "nobody.dcl.eth", "nobody.dcl.eth")
                .await
                .unwrap_err()
                .code,
            400
        );
        assert_eq!(
            scene::resolve_listing_place_id(
                &state,
                None,
                &serde_json::json!({ "realmName": world })
            )
            .await
            .unwrap(),
            "bafkreinewest12"
        );

        // World access is the permission core's answer (the old HTTP hop to the
        // worlds service is gone): an unrestricted world admits, an allow-list
        // world admits its listed wallets only, a shared-secret world reads the
        // secret from the signed metadata.
        let headers = signed(
            KEY,
            "post",
            "/get-scene-adapter",
            &meta(&world, &world, None),
        )
        .await;
        let Json(resp) = admission::handlers::get_scene_adapter(
            State(state.clone()),
            at("/get-scene-adapter"),
            headers.clone(),
            writes_on(),
            axum::body::Bytes::new(),
        )
        .await
        .expect("an unrestricted world admits");
        let claims = catalyst::shared::livekit::decode_jwt_payload(
            resp.adapter.split("access_token=").nth(1).unwrap(),
        );
        assert_eq!(
            claims["video"]["room"],
            catalyst::comms::rooms::worlds_service_scene_room(
                &catalyst::comms::world_identity::WorldName::parse(&world).unwrap(),
                &catalyst::comms::world_identity::SceneId::parse("bafkreinewest12").unwrap()
            )
        );
        sqlx::query("UPDATE worlds.worlds SET access_type = 'allow-list' WHERE name = $1")
            .bind(&world)
            .execute(&pool)
            .await
            .unwrap();
        let err = admission::handlers::get_scene_adapter(
            State(state.clone()),
            at("/get-scene-adapter"),
            headers.clone(),
            writes_on(),
            axum::body::Bytes::new(),
        )
        .await
        .unwrap_err();
        assert_eq!(
            (err.code, err.message.as_str()),
            (
                401,
                "Access denied, you are not authorized to access this world"
            )
        );
        let me = Address::parse(&address_of(KEY)).unwrap();
        sqlx::query("INSERT INTO worlds.world_permissions (world_name, permission, address) VALUES ($1, 'access', $2)").bind(&world).bind(&me).execute(&pool).await.unwrap();
        assert!(
            admission::handlers::get_scene_adapter(
                State(state.clone()),
                at("/get-scene-adapter"),
                headers,
                writes_on(),
                axum::body::Bytes::new()
            )
            .await
            .is_ok(),
            "allow-listed now"
        );
        let hash = bcrypt::hash("open sesame", 4).unwrap();
        sqlx::query("UPDATE worlds.worlds SET access_type = 'shared-secret', access_secret_hash = $2 WHERE name = $1").bind(&world).bind(&hash).execute(&pool).await.unwrap();
        let without = signed(
            KEY,
            "post",
            "/get-scene-adapter",
            &meta(&world, &world, None),
        )
        .await;
        assert_eq!(
            admission::handlers::get_scene_adapter(
                State(state.clone()),
                at("/get-scene-adapter"),
                without,
                writes_on(),
                axum::body::Bytes::new()
            )
            .await
            .unwrap_err()
            .code,
            401
        );
        let mut with_secret: serde_json::Value =
            serde_json::from_str(&meta(&world, &world, None)).unwrap();
        with_secret["secret"] = serde_json::json!("open sesame");
        let with_secret = signed(KEY, "post", "/get-scene-adapter", &with_secret.to_string()).await;
        assert!(
            admission::handlers::get_scene_adapter(
                State(state),
                at("/get-scene-adapter"),
                with_secret,
                writes_on(),
                axum::body::Bytes::new()
            )
            .await
            .is_ok(),
            "the signed secret admits"
        );
    }

    #[tokio::test]
    async fn a_world_scene_ban_survives_a_redeploy_at_the_same_base() {
        use axum::http::Request;
        use tower::ServiceExt;
        let Some(pool) = scratch().await else { return };
        let state = state_with(pool.clone(), Settings::for_tests());
        let world = format!("{}.dcl.eth", tag());
        let (first, second) = (
            format!("bafkreifirst{}", tag()),
            format!("bafkreisecond{}", tag()),
        );
        seed_world_scene(&pool, &world, &first, &["10,20", "11,20"]).await;
        let owner = Address::parse(&address_of(KEY)).unwrap();
        sqlx::query("UPDATE worlds.worlds SET owner = $2 WHERE name = $1")
            .bind(&world)
            .bind(&owner)
            .execute(&pool)
            .await
            .unwrap();
        let banned_key = catalyst::shared::test_keys::random_key();
        let banned = Address::parse(&address_of(&banned_key)).unwrap();
        let scene_meta = |scene: &str| {
            serde_json::json!({ "signer": "decentraland-kernel-scene", "realmName": world, "sceneId": scene, "parcel": "10,20" }).to_string()
        };
        let target = serde_json::json!({ "banned_address": banned.as_str() }).to_string();
        moderation::handlers::ban_user(
            State(state.clone()),
            at("/scene-bans"),
            signed(KEY, "post", "/scene-bans", &scene_meta(&first)).await,
            Query(Default::default()),
            axum::body::Bytes::from(target.clone()),
        )
        .await
        .map(|_| ())
        .expect("the owner bans");
        let stored: String =
            sqlx::query_scalar("SELECT place_id FROM social.scene_bans WHERE banned_address = $1")
                .bind(&banned)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(stored, format!("{world}:10,20"), "world + base parcel");

        sqlx::query("DELETE FROM worlds.world_scenes WHERE world_name = $1")
            .bind(&world)
            .execute(&pool)
            .await
            .unwrap();
        seed_world_scene(&pool, &world, &second, &["10,20"]).await;
        // What the replacing deploy keeps of the old scene.
        sqlx::query(
            "INSERT INTO worlds.replaced_world_scenes (world_name, entity_id, parcels, declared_base, replaced_at) \
             VALUES ($1, $2, ARRAY['10,20', '11,20'], NULL, timezone('UTC', now()))",
        )
        .bind(&world)
        .bind(&first)
        .execute(&pool)
        .await
        .unwrap();
        let mint_at = |key: String, scene: String| {
            let app = admission::world_mint::router(state.clone());
            let path = format!("/worlds/{world}/scenes/{scene}/comms");
            async move {
                let mut req = Request::post(&path)
                    .body(axum::body::Body::empty())
                    .unwrap();
                req.headers_mut()
                    .extend(signed(&key, "post", &path, "{}").await);
                let resp = app.oneshot(req).await.unwrap();
                let status = resp.status();
                let body = axum::body::to_bytes(resp.into_body(), 1 << 20)
                    .await
                    .unwrap();
                (status, String::from_utf8_lossy(&body).into_owned())
            }
        };
        let mint = |key: String| mint_at(key, second.clone());
        let (status, body) = mint(banned_key.clone()).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{body}");
        assert!(body.contains("You are banned from world"), "{body}");
        let (status, body) = mint_at(banned_key.clone(), first.clone()).await;
        assert_eq!(
            status,
            StatusCode::UNAUTHORIZED,
            "the replaced scene's room keeps the ban: {body}"
        );

        moderation::handlers::unban_user(
            State(state.clone()),
            at("/scene-bans"),
            signed(KEY, "delete", "/scene-bans", &scene_meta(&second)).await,
            Query(Default::default()),
            axum::body::Bytes::from(target),
        )
        .await
        .map(|_| ())
        .expect("the redeployed scene lifts the same ban");
        state.presence.complete_empty_fixture();
        let (status, body) = mint(banned_key.clone()).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        state.presence.complete_empty_fixture();
        let (status, body) = mint_at(banned_key.clone(), first.clone()).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        let adapter: serde_json::Value = serde_json::from_str(&body).unwrap();
        let claims = catalyst::shared::livekit::decode_jwt_payload(
            adapter["fixedAdapter"]
                .as_str()
                .unwrap()
                .split("access_token=")
                .nth(1)
                .unwrap(),
        );
        assert_eq!(
            claims["video"]["room"],
            catalyst::comms::rooms::worlds_service_scene_room(
                &catalyst::comms::world_identity::WorldName::parse(&world).unwrap(),
                &catalyst::comms::world_identity::SceneId::parse(&first).unwrap()
            ),
            "the replaced scene's own room"
        );
        sqlx::query("UPDATE worlds.replaced_world_scenes SET replaced_at = replaced_at - interval '8 days' WHERE entity_id = $1")
            .bind(&first)
            .execute(&pool)
            .await
            .unwrap();
        let (status, body) = mint_at(banned_key, first.clone()).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "past the window: {body}");
    }

    /// A ban on the scene that replaced another reaches the replaced scene's
    /// room: its mint is refused even though the base moved, and the banned
    /// wallet is kicked from the replaced room as well as the current one;
    /// the unban lifts the metadata ban from both.
    #[tokio::test]
    async fn a_ban_on_the_replacing_scene_reaches_the_replaced_scene_room() {
        use axum::http::Request;
        use catalyst::comms::world_identity::{SceneId, WorldName};
        use tower::ServiceExt;
        let Some(pool) = scratch().await else { return };
        let removed = Arc::new(std::sync::Mutex::new(Vec::<(String, String)>::new()));
        let metadata = Arc::new(std::sync::Mutex::new(std::collections::HashMap::<
            String,
            String,
        >::new()));
        let world = format!("{}.dcl.eth", tag());
        let (first, second, other) = (
            format!("bafkreifirst{}", tag()),
            format!("bafkreisecond{}", tag()),
            format!("bafkreiother{}", tag()),
        );
        let world_name = WorldName::parse(&world).unwrap();
        let room_of = |scene: &str| {
            catalyst::comms::rooms::worlds_service_scene_room(
                &world_name,
                &SceneId::parse(scene).unwrap(),
            )
        };
        let listed: Vec<String> = [&first, &second, &other]
            .iter()
            .map(|s| room_of(s))
            .collect();
        let capture = removed.clone();
        let stored = metadata.clone();
        let livekit = axum::Router::new().route(
            "/twirp/livekit.RoomService/{method}",
            axum::routing::post(
                move |axum::extract::Path(method): axum::extract::Path<String>,
                      Json(body): Json<serde_json::Value>| {
                    let (capture, listed, stored) =
                        (capture.clone(), listed.clone(), stored.clone());
                    async move {
                        match method.as_str() {
                            "ListRooms" => {
                                let names: Vec<String> = match body["names"].as_array() {
                                    Some(names) if !names.is_empty() => names
                                        .iter()
                                        .filter_map(|n| n.as_str().map(str::to_owned))
                                        .collect(),
                                    _ => listed,
                                };
                                let stored = stored.lock().unwrap();
                                let meta = |n: &String| stored.get(n).cloned().unwrap_or_else(|| "{}".to_owned());
                                Json(serde_json::json!({"rooms": names.iter().map(|n| serde_json::json!({"name": n, "sid": format!("RM_{n}"), "metadata": meta(n)})).collect::<Vec<_>>()}))
                            }
                            "UpdateRoomMetadata" => {
                                stored.lock().unwrap().insert(
                                    body["room"].as_str().unwrap_or_default().to_owned(),
                                    body["metadata"].as_str().unwrap_or_default().to_owned(),
                                );
                                Json(serde_json::json!({}))
                            }
                            "RemoveParticipant" => {
                                capture.lock().unwrap().push((
                                    body["room"].as_str().unwrap_or_default().to_owned(),
                                    body["identity"].as_str().unwrap_or_default().to_owned(),
                                ));
                                Json(serde_json::json!({}))
                            }
                            _ => Json(serde_json::json!({})),
                        }
                    }
                },
            ),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let mut settings = Settings::for_tests();
        settings.livekit.api_url = format!("http://{}", listener.local_addr().unwrap());
        settings.livekit.configured = true;
        let server = tokio::spawn(async move { axum::serve(listener, livekit).await.unwrap() });
        let state = state_with(pool.clone(), settings);

        // The redeploy moved the base: 10,20 (first) to 11,20 (second).
        seed_world_scene(&pool, &world, &second, &["11,20"]).await;
        seed_world_scene(&pool, &world, &other, &["50,50"]).await;
        sqlx::query(
            "INSERT INTO worlds.replaced_world_scenes (world_name, entity_id, parcels, declared_base, replaced_at) \
             VALUES ($1, $2, ARRAY['10,20', '11,20'], NULL, timezone('UTC', now()))",
        )
        .bind(&world)
        .bind(&first)
        .execute(&pool)
        .await
        .unwrap();
        let owner = Address::parse(&address_of(KEY)).unwrap();
        sqlx::query("UPDATE worlds.worlds SET owner = $2 WHERE name = $1")
            .bind(&world)
            .bind(&owner)
            .execute(&pool)
            .await
            .unwrap();
        let banned_key = catalyst::shared::test_keys::random_key();
        let banned = Address::parse(&address_of(&banned_key)).unwrap();
        let scene_meta = serde_json::json!({ "signer": "decentraland-kernel-scene", "realmName": world, "sceneId": second, "parcel": "11,20" }).to_string();
        moderation::handlers::ban_user(
            State(state.clone()),
            at("/scene-bans"),
            signed(KEY, "post", "/scene-bans", &scene_meta).await,
            Query(Default::default()),
            axum::body::Bytes::from(
                serde_json::json!({ "banned_address": banned.as_str() }).to_string(),
            ),
        )
        .await
        .map(|_| ())
        .expect("the owner bans on the replacing scene");
        let mut kicked: Vec<String> = removed
            .lock()
            .unwrap()
            .iter()
            .filter(|(_, identity)| identity == banned.as_str())
            .map(|(room, _)| room.clone())
            .collect();
        kicked.sort();
        kicked.dedup();
        let mut expected = vec![
            catalyst::comms::rooms::worlds_service_world_room(&world_name),
            room_of(&first),
            room_of(&second),
        ];
        expected.sort();
        assert_eq!(kicked, expected, "never the unrelated scene's room");

        let path = format!("/worlds/{world}/scenes/{first}/comms");
        let mut req = Request::post(&path)
            .body(axum::body::Body::empty())
            .unwrap();
        req.headers_mut()
            .extend(signed(&banned_key, "post", &path, "{}").await);
        let resp = admission::world_mint::router(state.clone())
            .oneshot(req)
            .await
            .unwrap();
        assert_eq!(
            resp.status(),
            StatusCode::UNAUTHORIZED,
            "the successor's ban reaches the replaced room"
        );
        let banned_in = |room: String| {
            metadata.lock().unwrap().get(&room).is_some_and(|m| {
                serde_json::from_str::<serde_json::Value>(m).unwrap()["bannedAddresses"]
                    .as_array()
                    .is_some_and(|a| a.iter().any(|v| v == banned.as_str()))
            })
        };
        assert!(
            banned_in(room_of(&first)) && banned_in(room_of(&second)),
            "the ban lists the address in both rooms"
        );

        moderation::handlers::unban_user(
            State(state.clone()),
            at("/scene-bans"),
            signed(KEY, "delete", "/scene-bans", &scene_meta).await,
            Query(Default::default()),
            axum::body::Bytes::from(
                serde_json::json!({ "banned_address": banned.as_str() }).to_string(),
            ),
        )
        .await
        .map(|_| ())
        .expect("the owner unbans on the replacing scene");
        assert!(
            !banned_in(room_of(&first)) && !banned_in(room_of(&second)),
            "the unban lifts it from the replaced room too"
        );
        server.abort();
    }

    #[tokio::test]
    async fn scene_admin_routes_write_the_relations_under_the_scene_signer_gate() {
        let Some(pool) = scratch().await else { return };
        let state = state_with(pool.clone(), Settings::for_tests());
        let place_id = format!("ToolkitScene{}", tag());
        let world = format!("toolkit-{}.eth", tag());
        seed_world_scene(&pool, &world, &place_id, &["10,20"]).await;
        let owner = Address::parse(&address_of(KEY)).unwrap();
        let new_admin = wallet();
        let metadata = serde_json::json!({ "signer": "decentraland-kernel-scene", "realmName": world, "sceneId": place_id, "parcel": "10,20" }).to_string();
        let body = serde_json::json!({ "place_id": place_id, "admin": new_admin.as_str() });

        let headers = signed(KEY, "post", "/scene-admin", &metadata).await;
        let err = moderation::handlers::add_admin(
            State(state.clone()),
            at("/scene-admin"),
            headers.clone(),
            Query(Default::default()),
            axum::body::Bytes::from(body.to_string()),
        )
        .await
        .map(|_| ())
        .unwrap_err();
        assert_eq!(
            (err.code, err.message.as_str()),
            (403, "signer is not an owner or admin of this scene"),
            "no stored owner or names reader: only explicit admins moderate"
        );

        state
            .moderation
            .add_admin(&place_id, &owner, &owner)
            .await
            .unwrap();
        moderation::handlers::add_admin(
            State(state.clone()),
            at("/scene-admin"),
            headers,
            Query(Default::default()),
            axum::body::Bytes::from(body.to_string()),
        )
        .await
        .expect("an explicit admin may add another");
        let explorer_signed = signed(
            KEY,
            "post",
            "/scene-admin",
            &serde_json::json!({ "signer": "dcl:explorer" }).to_string(),
        )
        .await;
        let err = moderation::handlers::add_admin(
            State(state.clone()),
            at("/scene-admin"),
            explorer_signed,
            Query(Default::default()),
            axum::body::Bytes::from(serde_json::json!({ "admin": new_admin.as_str() }).to_string()),
        )
        .await
        .map(|_| ())
        .unwrap_err();
        assert_eq!(err.code, 400, "the admin routes are scene-signed only");

        let list_headers = signed(KEY, "get", "/scene-admin", &metadata).await;
        let Json(list) = moderation::handlers::list_admins(
            State(state.clone()),
            at("/scene-admin"),
            list_headers,
            Query(moderation::handlers::PlaceQuery {
                place_id: Some(place_id.clone()),
                admin: None,
            }),
        )
        .await
        .unwrap();
        let admins: Vec<&str> = list
            .as_array()
            .unwrap()
            .iter()
            .map(|e| e["admin"].as_str().unwrap())
            .collect();
        assert_eq!(admins.len(), 2);
        assert!(admins.contains(&owner.as_str()) && admins.contains(&new_admin.as_str()));
        assert!(list[0]["canBeRemoved"].as_bool().unwrap());

        let remove_headers = signed(KEY, "delete", "/scene-admin", &metadata).await;
        moderation::handlers::remove_admin(
            State(state.clone()),
            at("/scene-admin"),
            remove_headers,
            Query(std::collections::BTreeMap::from([
                ("place_id".into(), place_id.clone()),
                ("admin".into(), new_admin.as_str().into()),
            ])),
            axum::body::Bytes::new(),
        )
        .await
        .unwrap();
        assert_eq!(
            state
                .moderation
                .list_active_admins(&place_id, None)
                .await
                .unwrap()
                .len(),
            1
        );

        let banned = wallet();
        let ban_headers = signed(KEY, "post", "/scene-bans", &metadata).await;
        moderation::handlers::ban_user(
            State(state.clone()),
            at("/scene-bans"),
            ban_headers,
            Query(Default::default()),
            axum::body::Bytes::from(
                serde_json::json!({"banned_address":banned.as_str().to_uppercase()}).to_string(),
            ),
        )
        .await
        .unwrap();
        assert!(
            state
                .moderation
                .is_scene_banned(&format!("{world}:10,20"), &banned)
                .await
                .unwrap()
        );
        let self_ban = signed(KEY, "post", "/scene-bans", &metadata).await;
        let err = moderation::handlers::ban_user(
            State(state.clone()),
            at("/scene-bans"),
            self_ban,
            Query(Default::default()),
            axum::body::Bytes::from(
                serde_json::json!({"banned_address":owner.as_str()}).to_string(),
            ),
        )
        .await
        .map(|_| ())
        .unwrap_err();
        assert_eq!(
            (err.code, err.message.as_str()),
            (400, "Cannot ban this address")
        );
        let list_headers = signed(KEY, "get", "/scene-bans", &metadata).await;
        let Json(page) = moderation::handlers::list_bans(
            State(state.clone()),
            at("/scene-bans"),
            list_headers,
            Query(moderation::handlers::BanQuery {
                place_id: Some(place_id.clone()),
                banned_address: None,
                limit: None,
                offset: None,
            }),
        )
        .await
        .unwrap();
        assert_eq!(
            (
                page.total,
                page.page,
                page.results[0]["bannedAddress"].as_str().unwrap()
            ),
            (1, 1, banned.as_str())
        );
        let unban_headers = signed(KEY, "delete", "/scene-bans", &metadata).await;
        moderation::handlers::unban_user(
            State(state.clone()),
            at("/scene-bans"),
            unban_headers,
            Query(std::collections::BTreeMap::from([
                ("place_id".into(), place_id.clone()),
                ("banned_address".into(), banned.as_str().into()),
            ])),
            axum::body::Bytes::new(),
        )
        .await
        .unwrap();
        assert!(
            !state
                .moderation
                .is_scene_banned(&format!("{world}:10,20"), &banned)
                .await
                .unwrap()
        );
    }
}

/// From `gatekeeper/streaming.rs`: the stream key lifecycle over a place
/// the signer administers (`Policy::is_admin` over `social.scene_admins`).
mod stream_access {
    use std::sync::Arc;

    use axum::Json;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use serde_json::Value;
    use tower::ServiceExt;
    use uuid::Uuid;

    use catalyst::comms::Address;
    use catalyst::comms::gatekeeper::auth::SCENE_SIGNER;
    use catalyst::comms::gatekeeper::moderation::store::{
        CreateBan, PLATFORM_BANNED_MSG, UpsertPlayerConnection,
    };
    use catalyst::comms::gatekeeper::streaming::{
        FOUR_DAYS_MS, NOT_AUTHORIZED_TO_STREAM_MSG, PATH, STREAM_ACCESS_NOT_FOUND_MSG,
    };
    use catalyst::comms::gatekeeper::{CommsState, LiveKit, Settings, SharedState, router};
    use catalyst::http::Outbound;
    use catalyst::shared::test_keys::{address_of, random_key, signed};
    use catalyst::worlds::policy::{PersonalWorldsPolicy, Policy};
    use catalyst::worlds::rooms::SceneResolver;

    /// A stub LiveKit Ingress service: `ListIngress` answers empty,
    /// `CreateIngress` mints `IN_<n>`/`sk_<n>`, `DeleteIngress` records the id.
    async fn ingress_stub() -> (String, Arc<std::sync::Mutex<Vec<String>>>) {
        use axum::routing::post;
        let log = Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
        let counter = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let app = axum::Router::new()
            .route("/twirp/livekit.Ingress/ListIngress", post(|| async { Json(serde_json::json!({ "items": [] })) }))
            .route(
                "/twirp/livekit.Ingress/CreateIngress",
                post({
                    let counter = counter.clone();
                    move || async move {
                        let n = counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
                        Json(serde_json::json!({ "ingressId": format!("IN_{n}"), "url": "rtmp://ingest.local/live", "streamKey": format!("sk_{n}") }))
                    }
                }),
            )
            .route(
                "/twirp/livekit.Ingress/DeleteIngress",
                post({
                    let log = log.clone();
                    move |Json(body): Json<Value>| async move {
                        log.lock().unwrap().push(body["ingressId"].as_str().unwrap_or_default().to_string());
                        Json(serde_json::json!({}))
                    }
                }),
            );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        (format!("http://{addr}"), log)
    }

    async fn call(
        app: &axum::Router,
        key: &str,
        method: &str,
        metadata: &str,
    ) -> (StatusCode, Value) {
        let headers = signed(
            key,
            &method.to_ascii_lowercase(),
            &format!("/comms-gatekeeper{PATH}"),
            metadata,
        )
        .await;
        let mut req = Request::builder()
            .method(method)
            .uri(format!("/comms-gatekeeper{PATH}"));
        for (k, v) in headers.iter() {
            req = req.header(k, v);
        }
        let resp = app
            .clone()
            .oneshot(req.body(Body::empty()).unwrap())
            .await
            .unwrap();
        let status = resp.status();
        let bytes = axum::body::to_bytes(resp.into_body(), 1 << 20)
            .await
            .unwrap();
        (
            status,
            serde_json::from_slice(&bytes).unwrap_or(Value::Null),
        )
    }

    /// The lifecycle over a Genesis place the signer administers: add mints
    /// once, list reads it, reset replaces the ingress, the webhook flips
    /// `streaming`, remove deletes it; a stranger is refused; a platform
    /// ban refuses list/add/reset before anything is minted, never remove.
    #[tokio::test]
    async fn a_scene_admin_mints_lists_resets_and_removes_the_stream_key() {
        let Some(pool) = catalyst::db::scratch::pool(4).await else {
            return;
        };
        let (host, deleted) = ingress_stub().await;
        let settings = Settings {
            livekit: LiveKit {
                api_url: host,
                ..Settings::for_tests().livekit
            },
            ..Settings::for_tests()
        };
        let state: SharedState = Arc::new(CommsState::new(
            pool.clone(),
            Outbound::for_tests(),
            settings,
            None,
            None,
            Arc::new(Policy::new(
                pool.clone(),
                PersonalWorldsPolicy::disabled(),
                None,
            )),
            Arc::new(SceneResolver::new(pool.clone())),
            Arc::new(catalyst::comms::presence::Presence::new()),
            Arc::new(catalyst::comms::interfaces::test_support::StubDirectory::default()),
        ));
        let app = axum::Router::new().nest("/comms-gatekeeper", router(state.clone()));
        let place_id = Uuid::new_v4().to_string();
        let parcel = format!("stream-test-{}", Uuid::new_v4().simple());
        let admin_key = random_key();
        let admin = Address::parse(&address_of(&admin_key)).unwrap();
        // The place has an owner, as every catalogue row does: the reset and revoke
        // notifications resolve their recipient from it.
        sqlx::query(
            "INSERT INTO explore.place (id, base_position, raw) VALUES ($1, $2, $3::jsonb)",
        )
        .bind(&place_id)
        .bind(&parcel)
        .bind(serde_json::json!({ "world": false, "positions": [parcel], "owner": address_of(&admin_key) }))
        .execute(&pool)
        .await
        .unwrap();
        state
            .moderation
            .add_admin(&place_id, &admin, &admin)
            .await
            .unwrap();
        let metadata = serde_json::json!({ "signer": SCENE_SIGNER, "realmName": "main", "parcel": parcel, "sceneId": "bafkreistreamscene" }).to_string();

        let (status, body) = call(&app, &admin_key, "GET", &metadata).await;
        assert_eq!(
            (status, body["error"].as_str()),
            (StatusCode::NOT_FOUND, Some(STREAM_ACCESS_NOT_FOUND_MSG))
        );
        let (status, minted) = call(&app, &admin_key, "POST", &metadata).await;
        assert_eq!(status, StatusCode::OK, "{minted}");
        assert_eq!(
            (
                minted["streaming_url"].as_str(),
                minted["streaming_key"].as_str()
            ),
            (Some("rtmp://ingest.local/live"), Some("sk_1"))
        );
        assert_eq!(
            minted["ends_at"].as_i64().unwrap() - minted["created_at"].as_i64().unwrap(),
            FOUR_DAYS_MS
        );
        let (_, again) = call(&app, &admin_key, "POST", &metadata).await;
        assert_eq!(
            again, minted,
            "an active credential is answered, not re-minted"
        );
        let (status, listed) = call(&app, &admin_key, "GET", &metadata).await;
        assert_eq!((status, &listed), (StatusCode::OK, &minted));
        let room: String = sqlx::query_scalar(
            "SELECT room_id FROM social.scene_stream_access WHERE place_id = $1 AND active",
        )
        .bind(&place_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(room, "scene:main:bafkreistreamscene");

        let event = serde_json::to_vec(&serde_json::json!({
            "id": Uuid::new_v4().to_string(), "createdAt": chrono::Utc::now().timestamp(),
            "event": "ingress_started", "ingressInfo": { "ingressId": "IN_1" }
        }))
        .unwrap();
        let signature = catalyst::comms::gatekeeper::webhook::test_support::webhook_jwt(
            &event,
            &state.livekit.api_key,
            &state.livekit.api_secret,
        );
        let response = app
            .clone()
            .oneshot(
                Request::post("/comms-gatekeeper/livekit-webhook")
                    .header("authorization", signature)
                    .body(Body::from(event))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let streaming: bool = sqlx::query_scalar(
            "SELECT streaming FROM social.scene_stream_access WHERE place_id = $1 AND active",
        )
        .bind(&place_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(streaming);

        let (status, reset) = call(&app, &admin_key, "PUT", &metadata).await;
        assert_eq!(status, StatusCode::OK, "{reset}");
        assert_eq!(reset["streaming_key"], "sk_2");
        assert_eq!(
            deleted.lock().unwrap().as_slice(),
            ["IN_1".to_string()],
            "the reset deletes the old ingress"
        );
        let rows: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM social.scene_stream_access WHERE place_id = $1",
        )
        .bind(&place_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(rows, 2, "the replaced credential is kept, inactive");

        let stranger = random_key();
        let (status, body) = call(&app, &stranger, "POST", &metadata).await;
        assert_eq!(
            (status, body["error"].as_str()),
            (StatusCode::FORBIDDEN, Some(NOT_AUTHORIZED_TO_STREAM_MSG))
        );

        let device = format!("stream-device-{}", Uuid::new_v4().simple());
        state
            .moderation
            .upsert_connection(UpsertPlayerConnection {
                address: admin.clone(),
                ip_address: None,
                device_id: Some(device.clone()),
            })
            .await
            .unwrap();
        state
            .moderation
            .create_ban(CreateBan {
                banned_address: admin.clone(),
                banned_by: "0xmoderator".into(),
                reason: "abuse".into(),
                custom_message: None,
                banned_device_id: Some(device),
                duration_ms: None,
            })
            .await
            .unwrap();
        for method in ["GET", "POST", "PUT"] {
            let (status, body) = call(&app, &admin_key, method, &metadata).await;
            assert_eq!(
                (status, body["error"].as_str()),
                (StatusCode::FORBIDDEN, Some(PLATFORM_BANNED_MSG)),
                "{method}"
            );
        }
        let (status, _) = call(&app, &admin_key, "DELETE", &metadata).await;
        assert_eq!(
            status,
            StatusCode::NO_CONTENT,
            "remove is ungated, as upstream"
        );
        assert_eq!(
            deleted.lock().unwrap().as_slice(),
            ["IN_1".to_string(), "IN_2".to_string()]
        );
        let (status, _) = call(&app, &admin_key, "DELETE", &metadata).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }
}

/// From `gatekeeper/admission/world_mint.rs`: the world comms mints decide through the
/// permission core (`worlds.*` rows, blocked wallets, shared secrets).
mod world_mint {
    use std::sync::Arc;

    use axum::Router;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;
    use uuid::Uuid;

    use catalyst::comms::gatekeeper::admission::world_mint::router;
    use catalyst::comms::gatekeeper::{CommsState, Settings};
    use catalyst::comms::presence::Presence;
    use catalyst::comms::rooms::worlds_service_world_room;
    use catalyst::comms::world_identity::WorldName;
    use catalyst::http::Outbound;
    use catalyst::shared::signed_fetch::ORIGINAL_PATH_HEADER;
    use catalyst::shared::test_keys::{address_of, random_key, signed};
    use catalyst::worlds::policy::{PersonalWorldsPolicy, Policy};
    use catalyst::worlds::rooms::SceneResolver;

    /// The mints over the SQL-backed permission core and room resolver, the
    /// capacity read from `presence`.
    fn mint_state(pool: &sqlx::PgPool, presence: Arc<Presence>) -> CommsState {
        CommsState::new(
            pool.clone(),
            Outbound::for_tests(),
            Settings::for_tests(),
            None,
            None,
            Arc::new(Policy::new(
                pool.clone(),
                PersonalWorldsPolicy::disabled(),
                None,
            )),
            Arc::new(SceneResolver::new(pool.clone())),
            presence,
            Arc::new(catalyst::comms::interfaces::test_support::StubDirectory::default()),
        )
    }

    fn world() -> WorldName {
        WorldName::parse(&format!(
            "m{}.dcl.eth",
            &Uuid::new_v4().simple().to_string()[..10]
        ))
        .unwrap()
    }

    async fn post(
        app: &Router,
        key: &str,
        path: &str,
        metadata: &str,
    ) -> (StatusCode, serde_json::Value, Option<String>) {
        let mut req = Request::post(path).body(Body::empty()).unwrap();
        req.headers_mut()
            .extend(signed(key, "post", path, metadata).await);
        let resp = app.clone().oneshot(req).await.unwrap();
        let status = resp.status();
        let retry = resp
            .headers()
            .get("retry-after")
            .and_then(|v| v.to_str().ok())
            .map(str::to_string);
        let bytes = axum::body::to_bytes(resp.into_body(), 1 << 20)
            .await
            .unwrap();
        (
            status,
            serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null),
            retry,
        )
    }

    fn room_of(adapter: &str) -> String {
        let token = adapter.split("access_token=").nth(1).unwrap_or_default();
        let claims = token.split('.').nth(1).unwrap();
        use base64::Engine as _;
        let claims: serde_json::Value = serde_json::from_slice(
            &base64::engine::general_purpose::URL_SAFE_NO_PAD
                .decode(claims)
                .unwrap(),
        )
        .unwrap();
        claims["video"]["room"].as_str().unwrap().to_string()
    }

    #[tokio::test]
    async fn the_world_mints_decide_through_the_permission_core_and_throttle_secrets() {
        let Some(pool) = catalyst::db::scratch::pool(4).await else {
            return;
        };
        let state = Arc::new(
            mint_state(&pool, Arc::new(Presence::new())).with_shared_secret_max_attempts(2),
        );
        let app = Router::new()
            .merge(router(state.clone()))
            .nest("/worlds-content-server", router(state.clone()));
        let visitor = random_key();

        let open = world();
        // A lobby is canonical only for a world the catalogue knows (dd6caa7c), so the open
        // world gets its row, as the listed and secret worlds below do.
        sqlx::query("INSERT INTO worlds.worlds (name) VALUES ($1)")
            .bind(open.key())
            .execute(&pool)
            .await
            .unwrap();
        // Permission/catalogue integration uses an explicit fresh complete
        // occupancy fixture; unknown source state correctly fails admission.
        state.presence.complete_empty_fixture();
        let (status, body, _) = post(
            &app,
            &visitor,
            &format!("/worlds/{}/comms", open.display()),
            "{}",
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        let adapter = body["fixedAdapter"].as_str().unwrap();
        assert!(adapter.starts_with("livekit:"), "{adapter}");
        assert_eq!(room_of(adapter), format!("world-{}", open.key()));
        // `/get-comms-adapter` requires the explorer's signed handshake, as upstream does.
        let handshake = r#"{"signer":"dcl:explorer","intent":"dcl:explorer:comms-handshake"}"#;
        state.presence.complete_empty_fixture();
        let (status, body, _) = post(
            &app,
            &visitor,
            &format!(
                "/worlds-content-server/get-comms-adapter/world-{}",
                open.display()
            ),
            handshake,
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        let (status, body, _) = post(&app, &visitor, "/get-comms-adapter/scene-x", handshake).await;
        assert_eq!(
            (status, body["message"].as_str().or(body["error"].as_str())),
            (StatusCode::BAD_REQUEST, Some("Invalid room id requested."))
        );
        let (status, _, _) = post(
            &app,
            &visitor,
            &format!("/worlds/{}/scenes/bafymissing/comms", open.display()),
            "{}",
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);

        let unknown = world();
        let disabled = world();
        sqlx::query(
            "INSERT INTO worlds.worlds (name, blocked_since) VALUES ($1, timezone('UTC', now()))",
        )
        .bind(disabled.key())
        .execute(&pool)
        .await
        .unwrap();
        for invalid in [&unknown, &disabled] {
            let (status, body, _) = post(
                &app,
                &visitor,
                &format!("/worlds/{}/comms", invalid.display()),
                "{}",
            )
            .await;
            assert_eq!(
                (status, body["message"].as_str()),
                (
                    StatusCode::NOT_FOUND,
                    Some(
                        format!("World \"{}\" is invalid or blocked.", invalid.display()).as_str()
                    )
                )
            );
            let (status, body, _) = post(
                &app,
                &visitor,
                &format!("/get-comms-adapter/world-{}", invalid.display()),
                handshake,
            )
            .await;
            assert_eq!(
                (status, body["message"].as_str()),
                (
                    StatusCode::NOT_FOUND,
                    Some(format!("World \"{}\" was not found.", invalid.display()).as_str())
                )
            );
        }

        let listed = world();
        sqlx::query("INSERT INTO worlds.worlds (name, access_type) VALUES ($1, 'allow-list')")
            .bind(listed.key())
            .execute(&pool)
            .await
            .unwrap();
        let (status, _, _) = post(
            &app,
            &visitor,
            &format!("/worlds/{}/comms", listed.display()),
            "{}",
        )
        .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "not on the allow-list");

        // A member of a listed community is admitted to the lobby and the scene.
        let community = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO social.communities (id, name, owner_address) VALUES ($1, 'c', $2)",
        )
        .bind(community)
        .bind(address_of(&random_key()))
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query("INSERT INTO social.community_members (community_id, member_address, role) VALUES ($1, $2, 'member')")
            .bind(community)
            .bind(address_of(&visitor))
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO worlds.world_access_communities (world_name, community_id) VALUES ($1, $2)")
            .bind(listed.key())
            .bind(community)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO worlds.world_scenes (world_name, entity_id, deployment_auth_chain, entity, deployer, parcels, size) \
             VALUES ($1, 'bafylisted', '[]'::json, '{}'::jsonb, $2, ARRAY['0,0'], 1)",
        )
        .bind(listed.key())
        .bind(catalyst::shared::identity::ZERO_ADDRESS)
        .execute(&pool)
        .await
        .unwrap();
        for path in [
            format!("/worlds/{}/comms", listed.display()),
            format!("/worlds/{}/scenes/bafylisted/comms", listed.display()),
        ] {
            state.presence.complete_empty_fixture();
            let (status, body, _) = post(&app, &visitor, &path, "{}").await;
            assert_eq!(status, StatusCode::OK, "{path}: {body}");
            let (status, _, _) = post(&app, &random_key(), &path, "{}").await;
            assert_eq!(status, StatusCode::UNAUTHORIZED, "{path}: a non-member");
        }

        let secret = world();
        let hash = bcrypt::hash("open sesame", 4).unwrap();
        sqlx::query("INSERT INTO worlds.worlds (name, access_type, access_secret_hash) VALUES ($1, 'shared-secret', $2)").bind(secret.key()).bind(&hash).execute(&pool).await.unwrap();
        let path = format!("/worlds/{}/comms", secret.display());
        for attempt in 0..2 {
            let (status, _, _) = post(&app, &visitor, &path, r#"{"secret":"wrong"}"#).await;
            assert_eq!(status, StatusCode::FORBIDDEN, "attempt {attempt}");
        }
        let (status, _, retry) = post(&app, &visitor, &path, r#"{"secret":"wrong"}"#).await;
        assert_eq!(
            (status, retry.as_deref()),
            (StatusCode::TOO_MANY_REQUESTS, Some("60")),
            "the third wrong secret"
        );
        let (status, _, _) = post(&app, &visitor, &path, r#"{"secret":"open sesame"}"#).await;
        assert_eq!(
            status,
            StatusCode::TOO_MANY_REQUESTS,
            "throttled even with the right secret"
        );
        let other = random_key();
        state.presence.complete_empty_fixture();
        let (status, body, _) = post(&app, &other, &path, r#"{"secret":"open sesame"}"#).await;
        assert_eq!(status, StatusCode::OK, "{body}");

        sqlx::query("INSERT INTO worlds.blocked_wallets (wallet) VALUES ($1)")
            .bind(address_of(&visitor))
            .execute(&pool)
            .await
            .unwrap();
        let (status, body, _) = post(
            &app,
            &visitor,
            &format!("/worlds/{}/comms", open.display()),
            "{}",
        )
        .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{body}");

        let crowd = Arc::new(Presence::new());
        for wallet in ["0x01", "0x02"] {
            crowd.peer_joined(wallet, open.key(), &worlds_service_world_room(&open), false);
        }
        let crowded = Arc::new(mint_state(&pool, crowd).with_max_users_per_world(2));
        let (status, _, _) = post(
            &router(crowded),
            &other,
            &format!("/worlds/{}/comms", open.display()),
            "{}",
        )
        .await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "a full world");

        let unsigned = app
            .clone()
            .oneshot(
                Request::post(format!("/worlds/{}/comms", open.display()))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert!(
            unsigned.status() == StatusCode::BAD_REQUEST
                || unsigned.status() == StatusCode::UNAUTHORIZED,
            "{}",
            unsigned.status()
        );
    }

    const NEST: &str = "/worlds-content-server";

    /// `route` on the `worlds-content-server.<domain>` host: nested by the
    /// gateway, the client's path forwarded as `x-original-path`.
    async fn post_via_gateway(
        app: &Router,
        key: &str,
        route: &str,
        signed_path: &str,
        forwarded: Option<&str>,
        metadata: &str,
    ) -> StatusCode {
        let mut req = Request::post(format!("{NEST}{route}"))
            .body(Body::empty())
            .unwrap();
        req.headers_mut()
            .extend(signed(key, "post", signed_path, metadata).await);
        if let Some(path) = forwarded {
            req.headers_mut()
                .insert(ORIGINAL_PATH_HEADER, path.parse().unwrap());
        }
        app.clone().oneshot(req).await.unwrap().status()
    }

    #[tokio::test]
    async fn the_mints_verify_the_bare_path_through_the_gateway_nest() {
        let Some(pool) = catalyst::db::scratch::pool(4).await else {
            return;
        };
        let state = Arc::new(mint_state(&pool, Arc::new(Presence::new())));
        let app = Router::new().nest(NEST, router(state.clone()));
        let visitor = random_key();
        let open = world();
        let scene = format!("bafkreinest{}", &Uuid::new_v4().simple().to_string()[..8]);
        sqlx::query("INSERT INTO worlds.worlds (name) VALUES ($1)")
            .bind(open.key())
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO worlds.world_scenes (world_name, entity_id, deployment_auth_chain, entity, deployer, parcels, size, mirror_source) \
             VALUES ($1, $2, '[]'::json, '{}'::jsonb, $3, ARRAY['0,0'], 1, NULL)",
        )
        .bind(open.key())
        .bind(&scene)
        .bind(catalyst::shared::identity::ZERO_ADDRESS)
        .execute(&pool)
        .await
        .unwrap();

        let handshake = r#"{"signer":"dcl:explorer","intent":"dcl:explorer:comms-handshake"}"#;
        let lobby = format!("/worlds/{}/comms", open.display());
        let scene_comms = format!("/worlds/{}/scenes/{scene}/comms", open.display());
        let adapter = format!("/get-comms-adapter/world-{}", open.display());
        for (route, other) in [
            (&lobby, &scene_comms),
            (&scene_comms, &lobby),
            (&adapter, &lobby),
        ] {
            let prefixed = format!("{NEST}{route}");
            for (signed_path, forwarded, status) in [
                (route.as_str(), Some(route.as_str()), StatusCode::OK),
                (prefixed.as_str(), None, StatusCode::OK),
                (
                    other.as_str(),
                    Some(other.as_str()),
                    StatusCode::UNAUTHORIZED,
                ),
                (route.as_str(), None, StatusCode::UNAUTHORIZED),
            ] {
                state.presence.complete_empty_fixture();
                let got =
                    post_via_gateway(&app, &visitor, route, signed_path, forwarded, handshake)
                        .await;
                assert_eq!(got, status, "{route} signed over {signed_path}");
            }
        }
    }
}

/// From `gatekeeper/admission/relay_authority/policy.rs`: the relay authority's rooms
/// decided by the permission core and the realm-lane fence.
mod relay_authority_policy {
    use std::sync::Arc;

    use async_trait::async_trait;
    use sqlx::PgPool;
    use uuid::Uuid;

    use catalyst::comms::Address;
    use catalyst::comms::gatekeeper::admission::cluster::fence::{
        FenceError, RealmAssignmentReader, RealmAssignmentSnapshot,
    };
    use catalyst::comms::gatekeeper::admission::relay_authority::RelayPolicy;
    use catalyst::comms::gatekeeper::admission::relay_authority::policy::CurrentRoomPolicy;
    use catalyst::comms::gatekeeper::admission::relay_authority::proof::{
        RoomClaims, RoomGrant, VerifiedRoom,
    };
    use catalyst::comms::protocol::room_authority::denied::Reason as DenialReason;
    use catalyst::worlds::policy::{AccessType, PersonalWorldsPolicy, Policy};
    use catalyst::worlds::rooms::SceneResolver;

    fn room(sub: &str, wallet: &str, room: &str, metadata: &str) -> VerifiedRoom {
        VerifiedRoom {
            claims: RoomClaims {
                iss: "key".into(),
                sub: sub.into(),
                exp: u64::MAX,
                nbf: 0,
                video: RoomGrant {
                    room: room.into(),
                    room_join: true,
                    can_publish_data: true,
                },
                metadata: metadata.into(),
            },
            wallet: wallet.into(),
            session: wallet.into(),
            is_guest: true,
            header_payload: String::new(),
            nonce: [0; 32],
            proof: Vec::new(),
        }
    }

    fn wallet() -> Address {
        Address::parse(&format!("0x00000000{}", Uuid::new_v4().simple())).unwrap()
    }

    async fn seed_world(pool: &PgPool, world: &str, access: AccessType, scene: &str) {
        sqlx::query(
            "INSERT INTO worlds.worlds (name, access_type) VALUES ($1, $2) ON CONFLICT DO NOTHING",
        )
        .bind(world)
        .bind(access.as_str())
        .execute(pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO worlds.world_scenes (world_name, entity_id, deployment_auth_chain, entity, deployer, parcels, size) \
             VALUES ($1, $2, '[]'::json, '{}'::jsonb, $3, '{\"0,0\"}', 1)",
        )
        .bind(world)
        .bind(scene)
        .bind(catalyst::shared::identity::ZERO_ADDRESS)
        .execute(pool)
        .await
        .unwrap();
    }

    fn island_owner(wallet: &str, revision: u64) -> String {
        serde_json::json!({ "catalyrstIsland": {
            "version": 2, "wallet": wallet, "session": wallet, "audience": "aud", "lane": "realm",
            "ownerEpoch": 7, "assignmentRevision": revision, "fencingToken": 11,
        }})
        .to_string()
    }

    struct FixedFence(Result<RealmAssignmentSnapshot, FenceError>);

    #[async_trait]
    impl RealmAssignmentReader for FixedFence {
        async fn current_realm_assignment(
            &self,
            _wallet: &str,
            _session: &str,
        ) -> Result<RealmAssignmentSnapshot, FenceError> {
            self.0.clone()
        }
    }

    /// An island room is granted only to the claims naming the realm lane's
    /// current owner, fence and island, and only while the wallet is not
    /// platform-banned; a moved row, another island or a fence that cannot
    /// be read is refused.
    #[tokio::test]
    async fn island_rooms_follow_the_realm_lane_fence() {
        let Some(pool) = catalyst::db::scratch::pool(2).await else {
            return;
        };
        let policy = Arc::new(Policy::new(
            pool.clone(),
            PersonalWorldsPolicy::disabled(),
            None,
        ));
        let player = wallet();
        let current = RealmAssignmentSnapshot {
            audience: "aud".into(),
            wallet: player.as_str().into(),
            lane: "realm".into(),
            authority_incarnation: "i".into(),
            owner_session: player.as_str().into(),
            owner_epoch: 7,
            assignment_revision: 3,
            fencing_token: 11,
            island_id: "island-abc".into(),
            pulse_admission: None,
        };
        let relay = |fence: Result<RealmAssignmentSnapshot, FenceError>| {
            CurrentRoomPolicy::new(
                Arc::new(SceneResolver::new(pool.clone())),
                policy.clone(),
                Arc::new(catalyst::comms::gatekeeper::moderation::Moderation::new(
                    pool.clone(),
                )),
                None,
            )
            .with_assignments(Some(Arc::new(FixedFence(fence))))
        };
        let owner = island_owner(player.as_str(), 3);
        assert_eq!(
            relay(Ok(current.clone()))
                .authorize(&room(
                    player.as_str(),
                    player.as_str(),
                    "island-abc",
                    &owner
                ))
                .await,
            Ok(())
        );
        assert_eq!(
            relay(Ok(current.clone()))
                .authorize(&room(
                    player.as_str(),
                    player.as_str(),
                    "island-other",
                    &owner
                ))
                .await,
            Err(DenialReason::NotAuthorized)
        );
        let stale = island_owner(player.as_str(), 2);
        assert_eq!(
            relay(Ok(current.clone()))
                .authorize(&room(
                    player.as_str(),
                    player.as_str(),
                    "island-abc",
                    &stale
                ))
                .await,
            Err(DenialReason::NotAuthorized)
        );
        assert_eq!(
            relay(Err(FenceError::Conflict))
                .authorize(&room(
                    player.as_str(),
                    player.as_str(),
                    "island-abc",
                    &owner
                ))
                .await,
            Err(DenialReason::NotAuthorized)
        );
        assert_eq!(
            relay(Err(FenceError::Unavailable))
                .authorize(&room(
                    player.as_str(),
                    player.as_str(),
                    "island-abc",
                    &owner
                ))
                .await,
            Err(DenialReason::Unavailable)
        );
        sqlx::query("INSERT INTO worlds.blocked_wallets (wallet) VALUES ($1)")
            .bind(&player)
            .execute(&pool)
            .await
            .unwrap();
        assert_eq!(
            relay(Ok(current))
                .authorize(&room(
                    player.as_str(),
                    player.as_str(),
                    "island-abc",
                    &owner
                ))
                .await,
            Err(DenialReason::NotAuthorized),
            "the platform-ban gate follows the fence"
        );
    }

    /// A Genesis scene room admits a clean wallet and refuses a banned one; a
    /// world room follows the world's access setting over its active scene;
    /// an island room without a fence is unavailable; the authoritative
    /// server needs the configured wallet in its claims.
    #[tokio::test]
    async fn rooms_are_decided_by_the_permission_core() {
        let Some(pool) = catalyst::db::scratch::pool(4).await else {
            return;
        };
        let authoritative = wallet();
        let policy = Arc::new(Policy::new(
            pool.clone(),
            PersonalWorldsPolicy::disabled(),
            None,
        ));
        let relay = CurrentRoomPolicy::new(
            Arc::new(SceneResolver::new(pool.clone())),
            policy.clone(),
            Arc::new(catalyst::comms::gatekeeper::moderation::Moderation::new(
                pool.clone(),
            )),
            Some(authoritative.clone()),
        );
        let (player, banned, other) = (wallet(), wallet(), wallet());
        let scene = format!("bafkrei{}", Uuid::new_v4().simple());
        assert_eq!(
            relay
                .authorize(&room(
                    player.as_str(),
                    player.as_str(),
                    &format!("scene:main:{scene}"),
                    ""
                ))
                .await,
            Ok(())
        );
        sqlx::query("INSERT INTO worlds.blocked_wallets (wallet) VALUES ($1)")
            .bind(&banned)
            .execute(&pool)
            .await
            .unwrap();
        assert_eq!(
            relay
                .authorize(&room(
                    banned.as_str(),
                    banned.as_str(),
                    &format!("scene:main:{scene}"),
                    ""
                ))
                .await,
            Err(DenialReason::NotAuthorized)
        );
        assert_eq!(
            relay
                .authorize(&room(player.as_str(), player.as_str(), "island-abc", ""))
                .await,
            Err(DenialReason::NotAuthorized),
            "an island room needs owner claims"
        );
        let owner = island_owner(player.as_str(), 3);
        assert_eq!(
            relay
                .authorize(&room(
                    player.as_str(),
                    player.as_str(),
                    "island-abc",
                    &owner
                ))
                .await,
            Err(DenialReason::Unavailable),
            "no fence, no island grant"
        );

        let open = format!("{}.dcl.eth", &Uuid::new_v4().simple().to_string()[..10]);
        seed_world(&pool, &open, AccessType::Unrestricted, &scene).await;
        assert_eq!(
            relay
                .authorize(&room(
                    player.as_str(),
                    player.as_str(),
                    &format!("world-{open}"),
                    ""
                ))
                .await,
            Ok(())
        );
        assert_eq!(
            relay
                .authorize(&room(
                    player.as_str(),
                    player.as_str(),
                    &catalyst::comms::rooms::worlds_service_scene_room(
                        &catalyst::comms::world_identity::WorldName::parse(&open).unwrap(),
                        &catalyst::comms::world_identity::SceneId::parse(&scene).unwrap()
                    ),
                    ""
                ))
                .await,
            Ok(())
        );
        assert_eq!(
            relay
                .authorize(&room(
                    player.as_str(),
                    player.as_str(),
                    &catalyst::comms::rooms::worlds_service_scene_room(
                        &catalyst::comms::world_identity::WorldName::parse(&open).unwrap(),
                        &catalyst::comms::world_identity::SceneId::parse("bafkreinotdeployed")
                            .unwrap()
                    ),
                    ""
                ))
                .await,
            Err(DenialReason::NotAuthorized),
            "a scene room exists only for a scene deployed in the world"
        );
        let listed = format!("{}.dcl.eth", &Uuid::new_v4().simple().to_string()[..10]);
        seed_world(&pool, &listed, AccessType::AllowList, &scene).await;
        assert_eq!(
            relay
                .authorize(&room(
                    player.as_str(),
                    player.as_str(),
                    &format!("world-{listed}"),
                    ""
                ))
                .await,
            Err(DenialReason::NotAuthorized)
        );
        sqlx::query("INSERT INTO worlds.world_permissions (world_name, permission, address) VALUES ($1, 'access', $2)")
            .bind(&listed)
            .bind(&player)
            .execute(&pool)
            .await
            .unwrap();
        assert_eq!(
            relay
                .authorize(&room(
                    player.as_str(),
                    player.as_str(),
                    &format!("world-{listed}"),
                    ""
                ))
                .await,
            Ok(())
        );
        let empty = format!("{}.dcl.eth", &Uuid::new_v4().simple().to_string()[..10]);
        assert_eq!(
            relay
                .authorize(&room(
                    player.as_str(),
                    player.as_str(),
                    &format!("world-{empty}"),
                    ""
                ))
                .await,
            Err(DenialReason::NotAuthorized),
            "a world without a scene has no room to admit into"
        );

        // The authoritative server is a principal like any other for the
        // access setting (the old policy ran the same check on it): admitted
        // to the unrestricted world, refused on the allow-list it is not on.
        let server_meta = serde_json::json!({ "catalyrstAuthoritativeServer": { "wallet": authoritative.as_str() } }).to_string();
        assert_eq!(
            relay
                .authorize(&room(
                    "authoritative-server",
                    authoritative.as_str(),
                    &catalyst::comms::rooms::worlds_service_scene_room(
                        &catalyst::comms::world_identity::WorldName::parse(&open).unwrap(),
                        &catalyst::comms::world_identity::SceneId::parse(&scene).unwrap()
                    ),
                    &server_meta
                ))
                .await,
            Ok(())
        );
        assert_eq!(
            relay
                .authorize(&room(
                    "authoritative-server",
                    authoritative.as_str(),
                    &catalyst::comms::rooms::worlds_service_scene_room(
                        &catalyst::comms::world_identity::WorldName::parse(&listed).unwrap(),
                        &catalyst::comms::world_identity::SceneId::parse(&scene).unwrap()
                    ),
                    &server_meta
                ))
                .await,
            Err(DenialReason::NotAuthorized)
        );
        let wrong_meta =
            serde_json::json!({ "catalyrstAuthoritativeServer": { "wallet": other.as_str() } })
                .to_string();
        assert_eq!(
            relay
                .authorize(&room(
                    "authoritative-server",
                    other.as_str(),
                    &catalyst::comms::rooms::worlds_service_scene_room(
                        &catalyst::comms::world_identity::WorldName::parse(&listed).unwrap(),
                        &catalyst::comms::world_identity::SceneId::parse(&scene).unwrap()
                    ),
                    &wrong_meta
                ))
                .await,
            Err(DenialReason::NotAuthorized)
        );
        assert_eq!(
            relay
                .authorize(&room(
                    "authoritative-server",
                    authoritative.as_str(),
                    "island-abc",
                    &server_meta
                ))
                .await,
            Err(DenialReason::NotAuthorized)
        );
        assert_eq!(
            relay
                .authorize(&room(
                    player.as_str(),
                    player.as_str(),
                    &format!("scene:main:{scene}"),
                    &server_meta
                ))
                .await,
            Err(DenialReason::NotAuthorized),
            "a player claiming the server metadata is refused"
        );
    }
}
