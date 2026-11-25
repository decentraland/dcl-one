//! Root HTTP server routes.
use super::{settings::write_switches, states::OpsStates};
use crate::{
    builder, camera_reel, comms, config::Config, content, explore, land_authz, ops, presence,
    price, shared, shop, social, worlds,
};
use dcl_metrics::http::component;
use std::sync::Arc;

/// The old transactions-server surface: the relay (`economy`) plus the
/// Shop's checkout and treasury routes over the same state, at the root
/// and under `/transactions-api`.
pub(super) fn economy_routes(state: &Arc<shop::evm::State>) -> axum::Router {
    shop::evm::router(state.clone())
        .merge(shop::checkout::router(state.clone()))
        .merge(shop::treasury::router(state.clone()))
}

/// The Shop's signed-write mounts: the canonical tree under `/shop-api`, the
/// relay with checkout and treasury ([`economy_routes`]) at the root and under
/// `/transactions-api` (with the old health routes there only), and the
/// rentals store at the root and under `/signatures-api`.
pub(super) fn shop_routes(states: shop::States) -> axum::Router {
    let economy = states.economy.clone();
    let signatures = states.signatures.clone();
    axum::Router::new()
        .nest("/shop-api", shop::router(states))
        .merge(economy_routes(&economy))
        .nest(
            "/transactions-api",
            economy_routes(&economy).merge(shop::evm::health_router(economy)),
        )
        .merge(shop::rentals::router(signatures.clone()))
        .nest("/signatures-api", shop::rentals::router(signatures))
}

/// The live presence's two routes where the worlds core routers held them:
/// at the root and under `/worlds-content-server`, each behind the worlds
/// writes gate (`WORLDS_WRITES_ENABLED`) as those routers were, applied
/// before nesting so the gate sees the route-local path.
pub(crate) fn live_presence_routes(
    presence: Arc<comms::presence::Presence>,
    writes_enabled: bool,
) -> axum::Router {
    axum::Router::new()
        .merge(worlds::writes::guard(
            comms::presence::router(presence.clone()),
            writes_enabled,
        ))
        .nest(
            "/worlds-content-server",
            worlds::writes::guard(comms::presence::router(presence), writes_enabled),
        )
}

/// Catalyst's own half of `/asset-bundle-registry` (profiles); the Registry
/// gateway's reads join it under the same prefix through `explore::api::router`.
pub(super) fn registry_profiles(market: shop::market::SharedState) -> axum::Router {
    axum::Router::new().nest(
        "/asset-bundle-registry",
        shop::market::profiles::registry::router(market),
    )
}

pub(super) struct Routes<'a> {
    pub(super) content: Arc<content::ContentState>,
    pub(super) market: shop::market::SharedState,
    pub(super) stage5_ops: OpsStates,
    pub(super) economy_state: Arc<shop::evm::State>,
    pub(super) signatures: Arc<shop::rentals::State>,
    pub(super) outfits: Arc<shop::outfits::State>,
    pub(super) subscriptions: Arc<shop::subscriptions::State>,
    pub(super) mana: Arc<shop::mana::State>,
    pub(super) land: Arc<land_authz::LandAuthzState>,
    pub(super) price: Arc<price::State>,
    pub(super) presence: presence::SharedState,
    pub(super) explore: explore::SharedState,
    pub(super) worlds: worlds::SharedState,
    pub(super) stage5: &'a worlds::Stage5,
    pub(super) live_presence: Arc<comms::presence::Presence>,
    pub(super) explorer_api: explore::api::Shared,
    pub(super) archipelago_state: comms::archipelago::AppState,
    pub(super) gatekeeper: &'a comms::gatekeeper::SharedState,
    pub(super) social: &'a social::States,
    pub(super) builder: builder::SharedState,
    pub(super) camera_reel: camera_reel::SharedState,
    pub(super) scene_proxy: crate::scene_proxy::Shared,
    pub(super) ops_admin: ops::admin::SharedState,
    pub(super) snapshot_gen: Option<Arc<content::snapshot_gen::SnapshotGenerator>>,
}

pub(super) fn build_routes(
    config: &Config,
    statistics_settings: comms::stats::http::Settings,
    primary: bool,
    routes: Routes<'_>,
) -> axum::Router {
    let Routes {
        content,
        market,
        stage5_ops,
        economy_state,
        signatures,
        outfits,
        subscriptions,
        mana,
        land,
        price,
        presence,
        explore,
        worlds,
        stage5,
        live_presence,
        explorer_api,
        archipelago_state,
        gatekeeper,
        social,
        builder,
        camera_reel,
        scene_proxy,
        ops_admin,
        snapshot_gen,
    } = routes;
    // The old gateway's location table (catalyrst/nixos/web-gateway.nix): the
    // content routes and /lambdas at the root, every other service under the
    // prefix nginx used to strip; the same routes also at the paths their own
    // vhost served, so a per-vhost proxy needs no rewrite. Anything else falls
    // through to the content 404. The canonical Shop tree (`shop::router`:
    // the market surface, the credits resources, the rentals store, the
    // MANA top-up reads, the operator routes) is under /shop-api; the
    // market surface alone answers again under /marketplace-api, its legacy
    // alias: its /v1/items and /v1/collections collide with the builder's at
    // the root, and the lambdas stay at the root. The economy routes answer at the root
    // (the old web tier's /v1/transactions location) and under
    // /transactions-api (the gateway's strip prefix, with the old /health and
    // /ping there only); the price feed at the root (/api/v3/simple/price,
    // as web.nix served it) and under /price; the rentals store at the root
    // and under /signatures-api; the LAND routes at their /lambdas paths; the
    // presence archive under /presence, the paths the sites call. The
    // connector answers at the root and under /archipelago-ea-stats; its
    // socket routes only on the primary.
    let connector_lifecycle = archipelago_state.clone();
    let archipelago = crate::comms::archipelago::mounts(
        archipelago_state,
        primary,
        statistics_settings,
        content.denylist.clone(),
    );
    let statistics = archipelago.statistics.clone();
    let blobs = content.clone();
    let report_storage = blobs.storage.clone();
    let max_deploy_body = content::create_entity::max_deployment_body_bytes(&content.settings);
    let app = component(content::build_router(content), "content")
        .merge(component(shop::market::router(market.clone()), "shop"))
        .merge(component(
            shop_routes(shop::States {
                market: market.clone(),
                credits: stage5_ops.credits.clone(),
                economy: economy_state,
                signatures,
                outfits,
                subscriptions,
                mana,
            }),
            "shop",
        ))
        .merge(component(
            shop::campaigns::builder_alias_router(market.clone()),
            "shop",
        ))
        // events-notifier.<domain> reaches /events-notifier (gateway subdomain rule).
        .nest(
            "/events-notifier",
            component(shop::market::tips::router(market.clone()), "shop"),
        )
        .merge(component(registry_profiles(market.clone()), "shop"))
        .nest(
            "/marketplace-api",
            component(shop::market::market_router(market), "shop"),
        )
        .merge(component(land_authz::router(land), "land-authz"))
        .merge(component(shop::evm::contracts_document_router(), "shop"))
        .merge(component(price::router(price.clone()), "price"))
        .nest("/price", component(price::router(price), "price"))
        .nest(
            "/presence",
            component(presence::router(presence), "presence"),
        )
        .merge(component(explore::router(explore.clone()), "explore"));
    let app = explore::NESTS.into_iter().fold(app, |app, nest| {
        app.nest(nest, component(explore::router(explore.clone()), "explore"))
    });
    let app = app
        .merge(component(
            worlds::app_router(worlds, blobs, stage5),
            "worlds",
        ))
        .merge(component(
            live_presence_routes(live_presence, stage5.writes_enabled),
            "comms",
        ))
        .merge(component(
            if primary {
                comms::archipelago::socket_router(connector_lifecycle.clone())
            } else {
                axum::Router::new()
            },
            "comms",
        ))
        .merge(component(explore::api::router(explorer_api), "explore"))
        // The scene workers' routing map and proxy (`scene_proxy`).
        .nest(
            "/_scene",
            component(crate::scene_proxy::router(scene_proxy), "scene-proxy"),
        )
        // The gatekeeper's mounts, where `social::router` held them: the Pulse
        // room authority, the message router's token route, the world comms mints and the LiveKit webhook (the
        // worlds content server's root path) at the root, the mints and the
        // webhook under `/worlds-content-server`, and the gatekeeper's routes
        // with social's MLS routes under `/comms-gatekeeper` behind the
        // social writes switch.
        .merge(component(
            comms::gatekeeper::room_authority_router(gatekeeper),
            "comms",
        ))
        .merge(component(
            comms::gatekeeper::message_router_token_router(gatekeeper),
            "comms",
        ))
        .merge(component(
            comms::gatekeeper::admission::world_mint::router(gatekeeper.clone()),
            "comms",
        ))
        .merge(component(
            social::writes::guard(
                comms::gatekeeper::webhook_router(gatekeeper.clone()),
                config.writes.social_writes_enabled,
            ),
            "comms",
        ))
        .nest(
            "/worlds-content-server",
            component(
                comms::gatekeeper::admission::world_mint::router(gatekeeper.clone()).merge(
                    social::writes::guard(
                        comms::gatekeeper::webhook_router(gatekeeper.clone()),
                        config.writes.social_writes_enabled,
                    ),
                ),
                "comms",
            ),
        )
        .nest(
            "/comms-gatekeeper",
            component(
                social::writes::guard(
                    comms::gatekeeper::router(gatekeeper.clone())
                        .merge(social::messaging::router(social.messaging.clone())),
                    config.writes.social_writes_enabled,
                ),
                "comms",
            ),
        )
        .merge(component(
            social::router(social, config.writes.social_writes_enabled),
            "social",
        ))
        .merge(component(builder::root_router(builder.clone()), "builder"))
        .nest(
            "/builder-api",
            component(builder::router(builder), "builder"),
        )
        .nest(
            "/camera-reel-service",
            component(camera_reel::router(camera_reel), "camera-reel"),
        )
        // The Cast presenter under upstream's host label (cast-presenter-service);
        // its sessions live in this process and their bots in `cast-bot` processes.
        .nest(
            "/cast-presenter-service",
            component(
                comms::cast_presenter::router(comms::cast_presenter::presenter(
                    gatekeeper.clone(),
                    config.social.cast_bot_bin.clone(),
                    comms::cast_presenter::LiveKitTarget::from_livekit(&gatekeeper.livekit),
                    config.social.max_concurrent_presentations,
                    Config::non_empty(&config.social.cast_presenter_public_url),
                )),
                "comms",
            ),
        )
        // The player report intake (`gatekeeper::reports`). Reports and their
        // evidence are social rows and blobs, so the social writes switch gates them.
        .nest(
            "/report-user",
            component(
                social::writes::guard(
                    comms::gatekeeper::reports::router(gatekeeper.clone(), report_storage),
                    config.writes.social_writes_enabled,
                ),
                "comms",
            ),
        );
    // The operator plane under the old gateway's prefixes: credits under
    // /credits only (its /users, /cart and /checkout would collide at the root; the
    // handlers verify a signed fetch over the request path or a matching
    // x-original-path), the relay under /rpc (rpc.<domain>/x -> /rpc/x),
    // the imposter zips under /bvimposters (the prefix baked into the explorer
    // wasm), the console API at the root (/admin/auth, /admin/api/*).
    let app = app
        .nest(
            "/credits",
            component(shop::credits::router(stage5_ops.credits), "shop"),
        )
        .nest("/rpc", component(ops::rpc::router(stage5_ops.rpc), "ops"))
        .nest(
            "/bvimposters",
            component(explore::imposters::router(stage5_ops.imposters), "explore"),
        )
        .merge(component(
            ops::admin::content::router(ops_admin.clone()),
            "ops",
        ))
        .merge(component(
            ops::admin::snapshots::router(ops_admin.clone(), snapshot_gen),
            "ops",
        ))
        .merge(component(ops::admin::router(ops_admin), "ops"))
        .merge(component(
            shared::writes::status_router(write_switches(config)),
            "shared",
        ));
    // A request a gate, an auth check or a handler answers before reading
    // its body is drained, not reset: nginx is still writing the upload
    // after the answer, and a reset fails that write, answers 502 and counts
    // the process down. Up to the largest body a route admits (a deployment,
    // `MAX_UPLOAD_TOTAL_SIZE` plus framing): the drain discards each chunk
    // as it arrives, so the bound costs bandwidth, never memory or disk. A
    // content deploy streams through nginx unbuffered, so its drain runs at
    // the client's pace; it ends at `DRAIN_DEADLINE` (300 s) whatever is left.
    // Forwarding headers name the client only from a loopback or listed proxy.
    let app =
        shared::client_ip::pin_client_headers(app, config.node.http_trusted_proxy_cidrs.clone());
    let app = shared::drain::drain_unread(
        app,
        max_deploy_body.max(worlds::core::deploy::MAX_UPLOAD_WIRE_SIZE_BYTES) as u64,
    );
    // Upstream's router under /content/: static segments matched whatever
    // their case, and its `Allow` list on a 405 (also under
    // /worlds-content-server/). Other paths pass untouched.
    let app = content::routing::upstream_routing(app);
    client_facing(
        app,
        statistics,
        archipelago.stats,
        archipelago.connector,
        config,
    )
}

/// The router the listener serves: the archipelago socket's stats and
/// connector hosts beside the shared app (`mount_services`), and every request
/// on any of them, those hosts' socket aliases included, inside its client
/// source, which the shared EIP-1654 validator charges its contract-wallet
/// checks to (`shared::eip1654_budget`).
fn client_facing(
    app: axum::Router,
    statistics: comms::stats::http::State,
    stats: axum::Router,
    connector: axum::Router,
    config: &Config,
) -> axum::Router {
    let app = comms::stats::http::mount_services(app, statistics, stats, connector);
    shared::eip1654_budget::scope_client_source(app, config.node.http_trusted_proxy_cidrs.clone())
}

#[cfg(test)]
mod client_source_tests {
    use std::sync::Arc;

    use axum::body::Body;
    use axum::extract::ConnectInfo;
    use axum::http::Request;
    use axum::routing::get;
    use tower::ServiceExt;

    use crate::comms::stats;
    use crate::shared::eip1654_budget::current_source;

    struct Hooks;

    #[async_trait::async_trait]
    impl stats::http::Hooks for Hooks {
        async fn probe(&self, _: bool) -> stats::http::Lifecycle {
            Default::default()
        }
        async fn diagnostics(&self) -> Result<String, stats::content::Error> {
            Ok(String::new())
        }
    }

    fn probe() -> axum::Router {
        axum::Router::new().route(
            "/probe",
            get(|| async { current_source().unwrap_or_else(|| "-".into()) }),
        )
    }

    /// A contract wallet that refuses every signature, counting the calls.
    #[derive(Default)]
    struct Refusing(std::sync::atomic::AtomicUsize);

    #[async_trait::async_trait]
    impl crate::auth::eip1654::Eip1654Validator for Refusing {
        async fn validate_signature(
            &self,
            _: &str,
            _: &[u8],
            _: &[u8],
        ) -> Result<bool, crate::auth::AuthError> {
            self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(false)
        }
    }

    /// [`probe`] and `/check`, one contract-wallet check through `validator`:
    /// 401 when the contract refuses, 429 when the caller's budget does.
    fn shared_app(validator: Arc<dyn crate::auth::eip1654::Eip1654Validator>) -> axum::Router {
        probe().route(
            "/check",
            get(move || {
                let validator = validator.clone();
                async move {
                    let wallet = "0x00000000000000000000000000000000000c0de1";
                    match validator.validate_signature(wallet, &[0; 32], &[0]).await {
                        Ok(_) => axum::http::StatusCode::UNAUTHORIZED,
                        Err(crate::auth::AuthError::Eip1654RateLimited) => {
                            axum::http::StatusCode::TOO_MANY_REQUESTS
                        }
                        Err(_) => axum::http::StatusCode::SERVICE_UNAVAILABLE,
                    }
                }
            }),
        )
    }

    /// Every mount the listener serves (`client_facing`, the router
    /// `build_routes` returns) names the request's client source: the shared
    /// app and the archipelago socket's stats and connector hosts, which sit
    /// outside the shared router's middleware. A trusted proxy's `X-Real-IP`
    /// names its client; an untrusted internal peer names none. A
    /// contract-wallet check through the node's budgeted validator is charged
    /// to its caller: one client behind the proxy spends its budget, the next
    /// one still gets its check.
    #[tokio::test]
    async fn every_mount_runs_inside_the_client_source() {
        let pool = sqlx::postgres::PgPoolOptions::new()
            .connect_lazy("postgres://nobody@127.0.0.1:1/nowhere")
            .unwrap();
        let statistics = stats::http::State::new(
            Arc::new(stats::Reader::default()),
            Arc::new(stats::content::Content {
                pool,
                denylist: Arc::new(crate::content::Denylist::default()),
            }),
            Arc::new(Hooks),
            Default::default(),
        );
        let config = crate::config::Config::parse_env_for_tests([
            ("DATABASE_URL", "postgres:///catalyst"),
            ("HTTP_TRUSTED_PROXY_CIDRS", "10.40.0.0/18"),
        ])
        .unwrap();
        let contract = Arc::new(Refusing::default());
        let validator = Arc::new(crate::shared::eip1654_budget::SourceBudgeted::new(
            contract.clone(),
        ));
        let app =
            super::client_facing(shared_app(validator), statistics, probe(), probe(), &config);
        let source = |path: &'static str, peer: &'static str, real_ip: Option<&'static str>| {
            let app = app.clone();
            async move {
                let mut request = Request::get(path);
                if let Some(ip) = real_ip {
                    request = request.header("x-real-ip", ip);
                }
                let mut request = request.body(Body::empty()).unwrap();
                request
                    .extensions_mut()
                    .insert(ConnectInfo(peer.parse::<std::net::SocketAddr>().unwrap()));
                let response = app.oneshot(request).await.unwrap();
                let body = axum::body::to_bytes(response.into_body(), 1024)
                    .await
                    .unwrap();
                String::from_utf8(body.to_vec()).unwrap()
            }
        };
        for path in [
            "/probe",
            "/archipelago-ea-stats/probe",
            "/archipelago-ea-ws-connector/probe",
        ] {
            assert_eq!(
                source(path, "203.0.113.9:1", None).await,
                "203.0.113.9",
                "{path}"
            );
            assert_eq!(
                source(path, "10.40.1.2:1", Some("198.51.100.4")).await,
                "198.51.100.4",
                "{path}: a trusted proxy's client"
            );
            assert_eq!(source(path, "10.9.0.2:1", None).await, "-", "{path}");
        }

        let check = |real_ip: &'static str| {
            let app = app.clone();
            async move {
                let mut request = Request::get("/check")
                    .header("x-real-ip", real_ip)
                    .body(Body::empty())
                    .unwrap();
                request.extensions_mut().insert(ConnectInfo(
                    "10.40.1.2:1".parse::<std::net::SocketAddr>().unwrap(),
                ));
                app.oneshot(request).await.unwrap().status()
            }
        };
        let started = std::time::Instant::now();
        let mut refused = 0;
        loop {
            match check("203.0.113.40").await.as_u16() {
                401 => refused += 1,
                429 => break,
                other => panic!("unexpected {other}"),
            }
            assert!(refused <= 2 * crate::shared::limits::PREAUTH_PER_MINUTE);
        }
        let spent = contract.0.load(std::sync::atomic::Ordering::SeqCst);
        let admitted = crate::shared::eip1654_budget::admitted_within(started.elapsed());
        assert!(
            admitted.contains(&spent),
            "the caller's budget, and no call past it: {spent}, {admitted:?}"
        );
        assert_eq!(check("203.0.113.41").await, 401, "another caller's budget");
        assert_eq!(
            contract.0.load(std::sync::atomic::Ordering::SeqCst),
            spent + 1
        );
    }
}
