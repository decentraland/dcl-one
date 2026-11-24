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
    prepared: crate::metrics_matcher::PreparedStats,
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
    let (statistics_settings, statistics_matcher) = prepared.into_parts();
    let archipelago = crate::comms::archipelago::mounts(
        archipelago_state,
        primary,
        statistics_settings,
        statistics_matcher,
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
    comms::stats::http::mount_services(app, statistics, archipelago.stats, archipelago.connector)
}
