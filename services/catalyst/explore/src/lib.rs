//! The explore domain: places, the worlds catalogue, events, the map and the
//! explorer-facing `/api/destinations` feed, over the `explore.*` schema.
//!
//! Reads are served from the mirrored catalogues (`explore.place`,
//! `explore.event`) plus the local overlays; the mirrors run as periodic jobs
//! ([`ExploreState::spawn_jobs`]) against explicit upstream endpoints
//! ([`MirrorSource`], which refuses a source that is this node or a private
//! address), each persisting its cursor and pass markers so a restart resumes
//! and `GET /api/status` reports how fresh each catalogue is. Writes (favorites,
//! likes, RSVPs, event/schedule creation, reports) arrive with stage 5.

// The crates this one builds on, under their module names (AGENTS.md).
pub(crate) use catalyst_config::config;
pub(crate) use catalyst_content as content;
#[cfg(test)]
pub(crate) use catalyst_db as db;
pub(crate) use catalyst_jobs as jobs;
pub(crate) use catalyst_land_authz as land_authz;
pub(crate) use catalyst_shared as shared;
pub(crate) use catalyst_storage as storage;
pub(crate) use dcl_auth as auth;
pub(crate) use dcl_http as http;

pub mod api;
pub mod copies;
pub mod events;
pub mod fed;
pub mod filecache;
pub mod imposters;
pub mod mirror_state;
pub mod opensea;
pub mod places;
pub mod sanitize;

use std::sync::Arc;

use crate::shared::signed_fetch;
use std::time::Duration;

use anyhow::{Context, bail};
use axum::Json;
use axum::Router;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::routing::{get, patch, post, put};
use serde::Serialize;
use serde_json::{Value, json};
use sqlx::PgPool;

use crate::auth::eip1654::Eip1654Validator;
use crate::jobs::{Jobs, PeriodicCfg};
use crate::shared::errors::ApiError;
use crate::shared::openapi;
use events::EventsComponent;
use events::schedules::SchedulesComponent;
use places::PlacesComponent;

/// An upstream catalogue endpoint a mirror reads from. Built once at startup:
/// the URL must be http(s) to a publicly routable host (the SSRF guard of
/// [`crate::http`]), and must not name this node itself, which would make the
/// mirror read its own copy back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MirrorSource {
    base: String,
}

impl MirrorSource {
    /// `local_urls`: every URL this node answers at (PUBLIC_URL, the bind
    /// address); a source whose host is one of them is a loop.
    pub fn checked(url: &str, local_urls: &[&str]) -> anyhow::Result<Self> {
        let trimmed = url.trim().trim_end_matches('/');
        let parsed = url::Url::parse(trimmed).context("mirror source is not a URL (the value is not echoed: a hosted endpoint may carry a token)")?;
        let Some(host) = parsed.host_str() else {
            bail!("mirror source has no host");
        };
        if !crate::http::is_safe_http_url(trimmed) {
            bail!(
                "mirror source host {host} is not a public http(s) endpoint; a mirror reads an \
                 explicit upstream, never a local or private address"
            );
        }
        for local in local_urls {
            if let Ok(local) = url::Url::parse(local)
                && local
                    .host_str()
                    .is_some_and(|h| h.eq_ignore_ascii_case(host))
                && local.port_or_known_default() == parsed.port_or_known_default()
            {
                bail!(
                    "mirror source {trimmed:?} is this node itself ({local}); refusing a self-referential mirror"
                );
            }
        }
        Ok(Self {
            base: trimmed.to_string(),
        })
    }

    /// No guard: for tests against a stub upstream on the loopback.
    pub fn unchecked(url: &str) -> Self {
        Self {
            base: url.trim().trim_end_matches('/').to_string(),
        }
    }

    pub fn base(&self) -> &str {
        &self.base
    }
}

/// The subset of the process configuration the explore domain reads. The old
/// env names: PLACES_MIRROR_UPSTREAM/PLACES_UPSTREAM_URL/PLACES_MIRROR_INTERVAL_SECS,
/// WORLDS_MIRROR_UPSTREAM/WORLDS_UPSTREAM_URL/WORLDS_MIRROR_INTERVAL_SECS,
/// EVENTS_MIRROR_UPSTREAM/EVENTS_UPSTREAM_URL/EVENTS_MIRROR_INTERVAL_SECS,
/// CATALYRST_EVENTS_ADMIN_TOKEN, CONTENT_PUBLIC_URL and HTTP_BASE_URL (the
/// asset rewrite domain, derived from PUBLIC_URL here).
#[derive(Clone, Debug)]
pub struct Settings {
    /// The node's content base URL; thumbnails from it pass the image filter.
    pub content_public_url: String,
    /// The deployment's own domain, swapped into mirrored upstream poster
    /// URLs; `None` (a decentraland.org node, or no PUBLIC_URL) keeps them.
    pub asset_rewrite_domain: Option<String>,
    /// Bearer for the events moderation routes; unset refuses them.
    pub events_admin_token: Option<String>,
    pub places_upstream: Option<MirrorSource>,
    pub places_interval: Duration,
    pub worlds_upstream: Option<MirrorSource>,
    pub worlds_interval: Duration,
    pub events_upstream: Option<MirrorSource>,
    pub events_interval: Duration,
    /// Which scenes the `places-derive` job turns into place rows
    /// (`places::derive`); `None` attaches no job (tests).
    pub places_derive: Option<places::derive::DeriveScope>,
    pub places_derive_interval: Duration,
    /// `DATA_WRITES_ENABLED`: the users' own writes (favorites, likes, RSVPs,
    /// their events, place reports) are local authority behind the data
    /// switch; off, each answers 503 with `Retry-After` and the reads serve.
    pub user_writes_enabled: bool,
    /// `OPS_WRITES_ENABLED`: the places admin routes (moderation, rankings,
    /// POIs, the reports queue's decisions) are operator writes.
    pub operator_writes_enabled: bool,
    /// `PLACES_ADMIN_AUTH_TOKEN`: the places admin bearer; unset refuses it.
    pub places_admin_token: Option<String>,
    /// `DATA_TEAM_AUTH_TOKEN`: the automated-ranking bearer.
    pub data_team_token: Option<String>,
    /// `PLACES_ADMIN_ADDRESSES`: wallets whose signed fetch may moderate.
    pub places_admin_addresses: Vec<String>,
    /// `EVENTS_BASE_URL`: the events site the sitemaps point at.
    pub events_base_url: String,
    /// This node's realm name, the `serverName` a place's `realms_detail`
    /// carries.
    pub realm_name: String,
}

impl Settings {
    /// The old `asset_rewrite_domain` rule: the public host unless it is
    /// decentraland.org or a subdomain. Only a domain can carry the
    /// `events-assets-*` subdomain, so an IP address, `localhost` or another
    /// single-label host (a local development stack) keeps the upstream URLs.
    pub fn asset_rewrite_domain_for(public_url: Option<&str>) -> Option<String> {
        public_url
            .and_then(|u| url::Url::parse(u).ok())
            .and_then(|u| match u.host() {
                Some(url::Host::Domain(h)) => Some(h.to_string()),
                _ => None,
            })
            .filter(|h| h.contains('.'))
            .filter(|h| h != "decentraland.org" && !h.ends_with(".decentraland.org"))
    }

    pub fn for_tests() -> Self {
        Self {
            content_public_url: "http://localhost/content/".to_string(),
            asset_rewrite_domain: None,
            events_admin_token: None,
            places_upstream: None,
            places_interval: Duration::from_secs(3600),
            worlds_upstream: None,
            worlds_interval: Duration::from_secs(3600),
            events_upstream: None,
            events_interval: Duration::from_secs(3600),
            places_derive: None,
            places_derive_interval: places::derive::DEFAULT_INTERVAL,
            user_writes_enabled: false,
            operator_writes_enabled: false,
            places_admin_token: None,
            data_team_token: None,
            places_admin_addresses: Vec::new(),
            events_base_url: "https://events.decentraland.org".to_string(),
            realm_name: "main".to_string(),
        }
    }
}

/// The 503 message of an operator write while `OPS_WRITES_ENABLED` is off.
pub const OPERATOR_WRITES_DISABLED: &str = "operator writes are disabled on this node (OPS_WRITES_ENABLED=false): a backup restore has to be demonstrated before this node accepts operator writes";

/// The 503 message of a user write while `DATA_WRITES_ENABLED` is off.
pub const USER_WRITES_DISABLED: &str = "explore writes are disabled on this node (DATA_WRITES_ENABLED=false): a backup restore has to be demonstrated before this node accepts user writes";

pub struct ExploreState {
    pub pool: PgPool,
    pub settings: Settings,
    pub places: PlacesComponent,
    pub events: EventsComponent,
    pub schedules: SchedulesComponent,
    /// Contract-wallet (EIP-1654) signature check for signed-fetch chains;
    /// `None` reads such chains as anonymous on the optional path.
    pub eip1654: Option<Arc<dyn Eip1654Validator>>,
    /// The ONE blob store, for the event poster uploads; `None` answers them 503.
    pub storage: Option<Arc<crate::storage::Storage>>,
    /// The destinations' live headcounts and connected wallets
    /// (`places::live`; comms attaches its rooms once it is up).
    pub live: Arc<places::live::Live>,
    /// The mirrors' pass markers, the `as_of` of their reads (`copies`).
    pub pass_markers: copies::Markers,
    /// Whether each catalogue holds rows of this node's own (`copies`).
    pub local_rows: copies::LocalRows,
}

pub type SharedState = Arc<ExploreState>;

impl ExploreState {
    pub fn new(
        pool: PgPool,
        settings: Settings,
        eip1654: Option<Arc<dyn Eip1654Validator>>,
    ) -> Self {
        Self {
            places: PlacesComponent::new(pool.clone(), &settings.content_public_url)
                .with_worlds_mirror(settings.worlds_upstream.is_some()),
            events: EventsComponent::new(pool.clone(), settings.asset_rewrite_domain.clone()),
            schedules: SchedulesComponent::new(pool.clone()),
            live: Arc::new(places::live::Live::new(pool.clone())),
            pass_markers: copies::Markers::new(copies::MARKER_TTL),
            local_rows: copies::LocalRows::new(copies::MARKER_TTL),
            pool,
            settings,
            eip1654,
            storage: None,
        }
    }

    /// The blob store the poster uploads land in.
    pub fn with_storage(mut self, storage: Arc<crate::storage::Storage>) -> Self {
        self.storage = Some(storage);
        self
    }

    /// A user write may proceed: `DATA_WRITES_ENABLED` is on and the
    /// restore stamps vouch for this node.
    pub fn user_writes(&self) -> Result<(), ApiError> {
        match crate::shared::writes::refusal(self.settings.user_writes_enabled) {
            None => Ok(()),
            Some(r) => Err(ApiError::WritesDisabled(
                r.message("explore", USER_WRITES_DISABLED),
            )),
        }
    }

    /// An operator write may proceed: `OPS_WRITES_ENABLED` is on and the
    /// restore stamps vouch for this node.
    pub fn operator_writes(&self) -> Result<(), ApiError> {
        match crate::shared::writes::ops_refusal(self.settings.operator_writes_enabled) {
            None => Ok(()),
            Some(r) => Err(ApiError::WritesDisabled(
                r.message("operator", OPERATOR_WRITES_DISABLED),
            )),
        }
    }

    /// The lowercased signer a write is attributed to: a verified signed
    /// fetch over `path` (or the `x-original-path` a prefix-stripping proxy
    /// forwards), else the old routes' 401 "Invalid authentication".
    pub async fn signer(
        &self,
        headers: &HeaderMap,
        method: &str,
        path: &str,
    ) -> Result<String, ApiError> {
        signed_fetch::require_signer(
            headers,
            method,
            path,
            signed_fetch::PayloadPolicy::V6ThenLegacy,
            self.eip1654.as_deref(),
        )
        .await
        .map(|s| s.into_string())
        .map_err(|e| {
            tracing::debug!(error = %e, "signed-fetch verification failed");
            ApiError::unauthorized("Invalid authentication")
        })
    }

    /// The lowercased signer of a signed-fetch request, or `None` for an
    /// anonymous or unverifiable one.
    pub async fn viewer(&self, headers: &HeaderMap, method: &str, path: &str) -> Option<String> {
        signed_fetch::optional_signer(
            headers,
            method,
            path,
            signed_fetch::PayloadPolicy::V6ThenLegacy,
            self.eip1654.as_deref(),
        )
        .await
        .map(|s| s.into_string())
    }

    /// Attaches the configured mirrors as periodic jobs: the first pass right
    /// away (jittered), then every interval, doubling the wait after a failed
    /// pass up to an hour.
    pub fn spawn_jobs(self: &Arc<Self>, jobs: &Jobs, client: crate::http::Outbound) {
        let cfg = |interval: Duration| {
            PeriodicCfg::every(interval)
                .with_jitter(Duration::from_secs(30))
                .with_error_backoff(Duration::from_secs(3600))
        };
        {
            let pool = self.pool.clone();
            let cfg = PeriodicCfg::every(Duration::from_secs(300))
                .with_jitter(Duration::from_secs(30))
                .with_error_backoff(Duration::from_secs(900));
            jobs.spawn_periodic_leased("explore-nonce-sweep", cfg, move |_ctx| {
                let pool = pool.clone();
                async move {
                    catalyst_fed::sweep_nonces(&pool, fed::NONCES, chrono::Utc::now().timestamp())
                        .await?;
                    Ok(())
                }
            });
        }
        if let Some(scope) = self.settings.places_derive {
            places::derive::spawn_job(
                jobs,
                self.pool.clone(),
                scope,
                self.settings.places_derive_interval,
                self.settings.content_public_url.clone(),
                self.storage.clone(),
            );
        }
        if let Some(source) = self.settings.places_upstream.clone() {
            let (client, pool, interval) = (
                client.clone(),
                self.pool.clone(),
                self.settings.places_interval,
            );
            tracing::info!(
                upstream = source.base(),
                interval_secs = interval.as_secs(),
                "place catalogue: mirroring from upstream"
            );
            jobs.spawn_periodic_leased("places-mirror", cfg(interval), move |ctx| {
                let (client, pool, source) = (client.clone(), pool.clone(), source.clone());
                async move {
                    let out = places::mirror::run_places_pass(
                        &client,
                        &pool,
                        &source,
                        interval,
                        &ctx.cancel,
                    )
                    .await?;
                    ctx.progress(format!(
                        "upserted {} retired {} swept {} complete {} resumed_from {:?}",
                        out.upserted, out.retired, out.swept, out.complete, out.resumed_from
                    ));
                    tracing::info!(
                        upserted = out.upserted,
                        retired = out.retired,
                        swept = out.swept,
                        complete = out.complete,
                        cancelled = out.cancelled,
                        "place catalogue mirrored from upstream"
                    );
                    Ok(())
                }
            });
        }
        if let Some(source) = self.settings.worlds_upstream.clone() {
            let (client, pool, interval) = (
                client.clone(),
                self.pool.clone(),
                self.settings.worlds_interval,
            );
            tracing::info!(
                upstream = source.base(),
                interval_secs = interval.as_secs(),
                "world catalogue: mirroring from upstream"
            );
            jobs.spawn_periodic_leased("worlds-mirror", cfg(interval), move |ctx| {
                let (client, pool, source) = (client.clone(), pool.clone(), source.clone());
                async move {
                    let out = places::mirror::run_worlds_pass(
                        &client,
                        &pool,
                        &source,
                        interval,
                        &ctx.cancel,
                    )
                    .await?;
                    ctx.progress(format!(
                        "upserted {} skipped {} failed_pages {} swept {} complete {}",
                        out.upserted, out.skipped_rows, out.failed_pages, out.swept, out.complete
                    ));
                    tracing::info!(
                        upserted = out.upserted,
                        skipped_rows = out.skipped_rows,
                        failed_rows = out.failed_rows,
                        failed_pages = out.failed_pages,
                        swept = out.swept,
                        complete = out.complete,
                        cancelled = out.cancelled,
                        "world catalogue mirrored from upstream"
                    );
                    if !out.complete && !out.cancelled {
                        anyhow::bail!(
                            "worlds pass incomplete: {} failed pages, {} failed rows",
                            out.failed_pages,
                            out.failed_rows
                        );
                    }
                    Ok(())
                }
            });
        }
        if let Some(source) = self.settings.events_upstream.clone() {
            let (client, pool, interval) = (
                client.clone(),
                self.pool.clone(),
                self.settings.events_interval,
            );
            tracing::info!(
                upstream = source.base(),
                interval_secs = interval.as_secs(),
                "event catalogue: mirroring from upstream"
            );
            jobs.spawn_periodic_leased("events-mirror", cfg(interval), move |ctx| {
                let (client, pool, source) = (client.clone(), pool.clone(), source.clone());
                async move {
                    let out =
                        events::mirror::run_pass(&client, &pool, &source, interval, &ctx.cancel)
                            .await?;
                    ctx.progress(format!(
                        "upserted {} failed_pages {} swept {} unlisted gone {} complete {}",
                        out.upserted, out.failed_pages, out.swept, out.unlisted_gone, out.complete
                    ));
                    tracing::info!(
                        upserted = out.upserted,
                        skipped_rows = out.skipped_rows,
                        failed_rows = out.failed_rows,
                        failed_pages = out.failed_pages,
                        swept = out.swept,
                        unlisted_refreshed = out.unlisted_refreshed,
                        unlisted_gone = out.unlisted_gone,
                        unlisted_unconfirmed = out.unlisted_unconfirmed,
                        complete = out.complete,
                        cancelled = out.cancelled,
                        "event catalogue mirrored from upstream"
                    );
                    if !out.complete && !out.cancelled {
                        anyhow::bail!(
                            "events pass incomplete: {} failed pages, {} failed rows",
                            out.failed_pages,
                            out.failed_rows
                        );
                    }
                    Ok(())
                }
            });
        }
    }
}

/// `GET /openapi.json`: the events document under `/events`, the places one elsewhere.
async fn openapi_json(
    axum::extract::OriginalUri(uri): axum::extract::OriginalUri,
) -> axum::response::Response {
    if uri.path().starts_with("/events/") {
        openapi::document(openapi::EVENTS)
    } else {
        openapi::document(openapi::PLACES)
    }
}

#[derive(Debug, Serialize)]
struct CatalogueCounts {
    places: i64,
    worlds: i64,
    local_worlds: i64,
    events: i64,
    local_events: i64,
    /// The newest `fetched_at` on the mirrored rows, per catalogue.
    places_fetched_at: Option<chrono::DateTime<chrono::Utc>>,
    events_fetched_at: Option<chrono::DateTime<chrono::Utc>>,
}

async fn catalogue_counts(pool: &PgPool) -> Result<CatalogueCounts, ApiError> {
    let (places, worlds, local_worlds, events, local_events, places_fetched_at, events_fetched_at): (
        i64,
        i64,
        i64,
        i64,
        i64,
        Option<chrono::NaiveDateTime>,
        Option<chrono::NaiveDateTime>,
    ) = sqlx::query_as(
        "SELECT (SELECT count(*) FROM explore.place WHERE world IS FALSE), \
                (SELECT count(*) FROM explore.place WHERE world IS TRUE), \
                (SELECT count(*) FROM explore.place_world_local), \
                (SELECT count(*) FROM explore.event), \
                (SELECT count(*) FROM explore.event WHERE origin = 'local'), \
                (SELECT max(fetched_at) FROM explore.place WHERE origin = 'upstream'), \
                (SELECT max(fetched_at) FROM explore.event WHERE origin = 'upstream')",
    )
    .fetch_one(pool)
    .await?;
    Ok(CatalogueCounts {
        places,
        worlds,
        local_worlds,
        events,
        local_events,
        places_fetched_at: places_fetched_at.map(|t| t.and_utc()),
        events_fetched_at: events_fetched_at.map(|t| t.and_utc()),
    })
}

/// `GET /api/status`: the old places status envelope plus, honestly, what the
/// mirrors have done: each configured mirror's last complete pass and its lag
/// (or `null` when none finished yet), an in-flight cursor, the last error,
/// and the row counts behind the routes. A node that mirrors nothing says so.
async fn status(State(state): State<SharedState>) -> Result<Json<Value>, ApiError> {
    let mirrors = mirror_state::status(&state.pool).await?;
    let counts = catalogue_counts(&state.pool).await?;
    let configured = json!({
        "places": state.settings.places_upstream.as_ref().map(MirrorSource::base),
        "worlds": state.settings.worlds_upstream.as_ref().map(MirrorSource::base),
        "events": state.settings.events_upstream.as_ref().map(MirrorSource::base),
    });
    Ok(Json(json!({
        "ok": true,
        "data": {
            "image": concat!("catalyst/", env!("CARGO_PKG_VERSION")),
            "timestamp": chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            "version": env!("CARGO_PKG_VERSION"),
            "mirrors": mirrors,
            "configured_mirrors": configured,
            "catalogue": counts,
        }
    })))
}

/// The explore routes, with the content server's CORS/security headers. Mount
/// beside the content router: `content::build_router(content).merge(explore::router(explore))`.
pub fn router(state: SharedState) -> Router {
    use events::handlers as ev;
    use events::profiles as evp;
    use events::writes as evw;
    use places::admin as pa;
    use places::destinations;
    use places::handlers as pl;
    use places::interactions as pi;
    use places::reports as pr;
    use places::v1 as pv1;
    Router::new()
        .route(
            "/api/places",
            get(pl::get_place_list).post(pl::post_place_list_by_id),
        )
        .route("/api/places/status", post(pl::post_place_status_list_by_id))
        .route("/api/places/{place_id}", get(pl::get_place))
        .route(
            "/api/places/{place_id}/categories",
            get(pl::get_place_categories),
        )
        .route(
            "/api/places/{place_id}/favorites",
            patch(pi::patch_place_favorites),
        )
        .route("/api/places/{place_id}/likes", patch(pi::patch_place_likes))
        .route("/api/places/{place_id}/rating", put(pa::put_place_rating))
        .route("/api/places/{place_id}/ranking", put(pa::put_place_ranking))
        .route(
            "/api/places/{place_id}/ranking-exclusion",
            put(pa::put_place_ranking_exclusion).delete(pa::delete_place_ranking_exclusion),
        )
        .route(
            "/api/places/{place_id}/highlight",
            put(pa::put_place_highlight),
        )
        .route(
            "/api/places/{place_id}/disable",
            put(pa::put_place_disable).patch(pa::patch_place_disable),
        )
        .route(
            "/api/places/{place_id}/featured",
            put(pa::put_place_featured).delete(pa::delete_place_featured),
        )
        .route("/api/categories", get(pl::get_categories))
        .route("/api/map", get(pl::get_map_places))
        .route("/api/map/places", get(pl::get_all_places_list))
        .route(
            "/api/destinations",
            get(destinations::get_destinations_list)
                .post(destinations::post_destinations_list_by_id),
        )
        .route("/api/worlds", get(pl::get_world_list))
        .route("/api/worlds/{world_id}", get(pl::get_world))
        .route(
            "/api/worlds/{world_id}/scenes",
            get(places::world_scenes::get_world_scene_places),
        )
        .route(
            "/api/worlds/{world_id}/favorites",
            patch(pi::patch_world_favorites),
        )
        .route("/api/worlds/{world_id}/likes", patch(pi::patch_world_likes))
        .route("/api/worlds/{world_id}/rating", put(pa::put_world_rating))
        .route("/api/worlds/{world_id}/ranking", put(pa::put_world_ranking))
        .route(
            "/api/worlds/{world_id}/ranking-exclusion",
            put(pa::put_world_ranking_exclusion).delete(pa::delete_world_ranking_exclusion),
        )
        .route(
            "/api/worlds/{world_id}/highlight",
            put(pa::put_world_highlight),
        )
        .route(
            "/api/worlds/{world_id}/featured",
            put(pa::put_world_featured).delete(pa::delete_world_featured),
        )
        .route(
            "/api/destinations/ranking",
            put(pa::put_destinations_ranking),
        )
        .route("/places/place", get(places::social::inject_place_metadata))
        .route("/places/world", get(places::social::inject_world_metadata))
        .route("/api/pois", get(pa::get_pois).post(pa::post_poi))
        .route(
            "/api/pois/{position}",
            patch(pa::patch_poi).delete(pa::delete_poi),
        )
        .route("/api/reports", get(pr::get_reports))
        .route("/api/reports/{id}", patch(pr::patch_report))
        .route("/api/report", post(pr::post_report))
        .route("/api/report/upload/{filename}", put(pr::put_report_upload))
        .route("/v1/destinations", get(pv1::get_v1_destinations_list))
        .route("/v1/destinations/{id}", get(pv1::get_v1_destination))
        .route(
            "/v1/destinations/{id}/favorites",
            put(pi::put_v1_destination_favorites).delete(pi::delete_v1_destination_favorites),
        )
        .route(
            "/v1/destinations/{id}/likes",
            put(pi::put_v1_destination_likes).delete(pi::delete_v1_destination_likes),
        )
        .route("/v1/categories", get(pv1::get_v1_categories))
        .route("/api/world_names", get(pl::get_world_names_list))
        .route(
            "/api/events",
            get(ev::get_event_list).post(evw::create_event),
        )
        .route("/api/events/search", post(ev::post_event_search))
        .route("/api/events/attending", get(ev::get_attending_event_list))
        .route("/api/events/moderation", get(ev::get_moderation_list))
        .route("/api/events/categories", get(ev::get_event_category_list))
        .route(
            "/api/events/{event_id}",
            get(ev::get_event)
                .patch(evw::patch_event)
                .delete(evw::delete_event),
        )
        .route(
            "/api/events/{event_id}/attendees",
            get(ev::get_event_attendees)
                .post(evw::create_event_attendee)
                .delete(evw::delete_event_attendee),
        )
        .route(
            "/api/schedules",
            get(ev::get_schedule_list).post(evp::create_schedule),
        )
        .route(
            "/api/schedules/{schedule_id}",
            get(ev::get_schedule_by_id).patch(evp::patch_schedule),
        )
        .route("/api/profiles/settings", get(evp::list_profile_settings))
        .route(
            "/api/profiles/me/settings",
            get(evp::get_my_profile_settings).patch(evp::update_my_profile_settings),
        )
        .route(
            "/api/profiles/{profile_id}/settings",
            get(evp::get_profile_settings).patch(evp::update_profile_settings),
        )
        .route(
            "/api/profiles/subscriptions",
            get(evp::profile_subscriptions)
                .post(evp::profile_subscriptions)
                .delete(evp::profile_subscriptions),
        )
        .route("/api/poster", post(events::posters::upload_poster))
        .route("/assets/event-default.webp", get(events::posters::default_image))
        .route(
            "/api/poster-vertical",
            post(events::posters::upload_poster_vertical),
        )
        .route("/v1/events", get(ev::get_event_list))
        .route("/v1/events/{event_id}", get(events::v1::get_v1_event))
        .route(
            "/v1/destinations/{id}/events",
            get(events::v1::get_v1_destination_events),
        )
        .route("/events/sitemap.xml", get(events::sitemap::sitemap_index))
        .route(
            "/events/sitemap.static.xml",
            get(events::sitemap::sitemap_static),
        )
        .route(
            "/events/sitemap.events.xml",
            get(events::sitemap::sitemap_events),
        )
        .route(
            "/events/sitemap.schedules.xml",
            get(events::sitemap::sitemap_schedules),
        )
        .route("/federation/places/snapshot", get(fed::places_snapshot))
        .route("/federation/places/changes", get(fed::places_changes))
        .route("/federation/v1/events/feed", get(fed::events_feed))
        .route(
            "/federation/v1/events/{event_id}/attendance",
            get(fed::events_attendance),
        )
        .route("/api/status", get(status))
        .route("/openapi.json", get(openapi_json))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            copies::gate,
        ))
        .layer(axum::middleware::from_fn(
            crate::shared::cors::cors_middleware,
        ))
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_mirror_source_must_be_a_public_upstream_that_is_not_this_node() {
        let ok = MirrorSource::checked(
            "https://places.decentraland.org/",
            &["https://peer.example.org", "http://127.0.0.1:5210"],
        )
        .unwrap();
        assert_eq!(ok.base(), "https://places.decentraland.org");
        for bad in [
            "http://127.0.0.1:5143",
            "http://localhost:5143",
            "http://10.0.0.1/",
            "http://169.254.169.254/",
            "ftp://places.decentraland.org",
            "not a url",
            "",
        ] {
            assert!(
                MirrorSource::checked(bad, &[]).is_err(),
                "{bad:?} must be refused"
            );
        }
        let err = MirrorSource::checked(
            "https://peer.example.org/",
            &["https://peer.example.org:443/content"],
        )
        .unwrap_err()
        .to_string();
        assert!(err.contains("self-referential"), "{err}");
        assert!(
            MirrorSource::checked(
                "https://peer.example.org:8443/",
                &["https://peer.example.org/"]
            )
            .is_ok()
        );
        assert_eq!(
            MirrorSource::unchecked("http://127.0.0.1:9/").base(),
            "http://127.0.0.1:9"
        );
    }

    #[test]
    fn the_asset_rewrite_domain_is_the_public_host_unless_it_is_decentraland() {
        assert_eq!(
            Settings::asset_rewrite_domain_for(Some("https://interconnected.online/")).as_deref(),
            Some("interconnected.online")
        );
        assert_eq!(
            Settings::asset_rewrite_domain_for(Some("https://events.decentraland.org")),
            None
        );
        assert_eq!(
            Settings::asset_rewrite_domain_for(Some("https://decentraland.org")),
            None
        );
        assert_eq!(Settings::asset_rewrite_domain_for(None), None);
        for local in [
            "http://127.0.0.1:5212",
            "http://localhost:5212/",
            "http://[::1]:5212",
            "http://devbox:5212",
        ] {
            assert_eq!(
                Settings::asset_rewrite_domain_for(Some(local)),
                None,
                "{local}"
            );
        }
        assert_eq!(Settings::asset_rewrite_domain_for(Some("garbage")), None);
    }

    /// The explore writes' two switches wait on the restore stamps too: a
    /// stale drill stamp answers the 503 naming the stamp and its age.
    #[tokio::test]
    async fn a_stale_restore_stamp_holds_explore_writes_and_names_it() {
        use crate::shared::writes::restore::{
            Stamp,
            lab::{DAY, StampLab},
            scoped,
        };
        let pool = sqlx::postgres::PgPoolOptions::new()
            .connect_lazy("postgres://nobody@127.0.0.1:1/nowhere")
            .unwrap();
        let state = ExploreState::new(
            pool,
            Settings {
                user_writes_enabled: true,
                operator_writes_enabled: true,
                ..Settings::for_tests()
            },
            None,
        );
        let lab = StampLab::fresh();
        scoped(lab.stamps.clone(), async {
            assert!(state.user_writes().is_ok());
            assert!(state.operator_writes().is_ok());
            lab.write_aged(Stamp::Drill, 8 * DAY + DAY / 2);
            let held = |r: Result<(), ApiError>| match r {
                Err(ApiError::WritesDisabled(m)) => m.into_owned(),
                other => panic!("{other:?}"),
            };
            assert_eq!(
                held(state.user_writes()),
                "explore writes are held on this node: the last passing restore drill (drill.json) is 8 d 12 h old, past WRITES_DRILL_MAX_AGE_DAYS=8"
            );
            assert!(
                held(state.operator_writes()).starts_with("operator writes are held on this node: ")
            );
            let res = axum::response::IntoResponse::into_response(state.user_writes().unwrap_err());
            assert_eq!(res.status(), axum::http::StatusCode::SERVICE_UNAVAILABLE);
            assert_eq!(res.headers()[axum::http::header::RETRY_AFTER], "3600");
        })
        .await;
    }
}
