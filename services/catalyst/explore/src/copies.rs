//! The descriptive rule (`shared::as_of`) over the three mirrored
//! catalogues: a read of places, worlds or events answers the copy its
//! mirror's last complete pass made, stamped `as_of`, and the unavailable
//! envelope while a configured mirror has never completed one. Rows no
//! mirror fed ([`origin`]: the places this node derives from its content,
//! the worlds it hosts, the events written into it) are live: a read of one
//! of them carries no stamp and never answers 503, and a list answers 503
//! only while the catalogue holds none of them, and otherwise lists only
//! them until a pass completes ([`OwnRowsOnly`]).

use std::time::Duration;

use axum::body::{Body, to_bytes};
use axum::extract::{Request, State};
use axum::http::Method;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use chrono::NaiveDateTime;

use crate::events::mirror::EVENTS_MIRROR;
use crate::places::mirror::{PLACES_MIRROR, WORLDS_MIRROR};
use crate::shared::as_of::{self, Freshness};
use crate::shared::cache::TtlMap;
use crate::{Settings, SharedState};

/// How long a pass marker is reused before it is read again.
pub const MARKER_TTL: Duration = Duration::from_secs(10);

/// `explore.mirror_state.last_complete_pass_at` per mirror.
pub type Markers = TtlMap<&'static str, Option<NaiveDateTime>>;

/// Whether a mirror's catalogue holds rows of this node's own ([`origin::any`]).
pub type LocalRows = TtlMap<&'static str, bool>;

/// The mirrors a list served live before their first complete pass may
/// read only this node's own rows of: what an interrupted first pass
/// upserted waits for a complete one. Set by [`gate`] for the request it
/// serves; [`origin::place_rows_sql`] and [`origin::event_rows_sql`] read it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OwnRowsOnly {
    places: bool,
    worlds: bool,
    events: bool,
}

impl OwnRowsOnly {
    fn insert(&mut self, mirror: &str) {
        match mirror {
            PLACES_MIRROR => self.places = true,
            WORLDS_MIRROR => self.worlds = true,
            EVENTS_MIRROR => self.events = true,
            _ => {}
        }
    }

    /// The request's restriction; none outside a gated request.
    fn current() -> Self {
        OWN_ROWS_ONLY.try_with(|only| *only).unwrap_or_default()
    }
}

tokio::task_local! {
    static OWN_ROWS_ONLY: OwnRowsOnly;
}

const PLACES: &[&str] = &[PLACES_MIRROR];
const WORLDS: &[&str] = &[WORLDS_MIRROR];
const DESTINATIONS: &[&str] = &[PLACES_MIRROR, WORLDS_MIRROR];
const EVENTS: &[&str] = &[EVENTS_MIRROR];

/// The catalogues a request reads, by the path the router sees (the prefix
/// of a nest stripped); empty for writes and for what no mirror feeds.
pub fn catalogues(method: &Method, path: &str) -> &'static [&'static str] {
    let segments: Vec<&str> = path.trim_matches('/').split('/').collect();
    let read_over_post = matches!(
        segments.as_slice(),
        ["api", "places"]
            | ["api", "places", "status"]
            | ["api", "destinations"]
            | ["api", "events", "search"]
    );
    if !as_of::is_read(method) && !(*method == Method::POST && read_over_post) {
        return &[];
    }
    match segments.as_slice() {
        ["api", "places"]
        | ["api", "places", _]
        | ["api", "places", _, "categories"]
        | ["api", "map"]
        | ["api", "map", "places"] => PLACES,
        ["api", "worlds"]
        | ["api", "worlds", _]
        | ["api", "worlds", _, "scenes"]
        | ["api", "world_names"] => WORLDS,
        ["api", "destinations"] | ["v1", "destinations"] | ["v1", "destinations", _] => {
            DESTINATIONS
        }
        ["api", "events", "attending" | "moderation" | "categories"] => &[],
        ["api", "events"]
        | ["api", "events", _]
        | ["v1", "events"]
        | ["v1", "events", _]
        | ["v1", "destinations", _, "events"] => EVENTS,
        _ => &[],
    }
}

/// The one object a read names, whose origin decides its freshness.
#[derive(Debug, PartialEq, Eq)]
enum Object<'a> {
    Place(&'a str),
    World(&'a str),
    WorldScenes(&'a str),
    Destination(&'a str),
    Event(&'a str),
}

fn object(path: &str) -> Option<Object<'_>> {
    let segments: Vec<&str> = path.trim_matches('/').split('/').collect();
    match segments.as_slice() {
        ["api", "places", id] | ["api", "places", id, "categories"] if *id != "status" => {
            Some(Object::Place(id))
        }
        ["api", "worlds", name] => Some(Object::World(name)),
        ["api", "worlds", name, "scenes"] => Some(Object::WorldScenes(name)),
        ["v1", "destinations", id] => Some(Object::Destination(id)),
        ["api", "events", id] | ["v1", "events", id] if *id != "search" => Some(Object::Event(id)),
        _ => None,
    }
}

/// Whether a row is this node's own rather than a mirror's: a place derived
/// from its content (either `places::derive` scope) or `origin = 'local'`, a
/// world it hosts, an event created here (`origin = 'local'`, or the create
/// overlay an older binary left, which the events mirror reclaims).
pub mod origin {
    use sqlx::PgPool;

    use crate::events::mirror::EVENTS_MIRROR;
    use crate::places::mirror::{PLACES_MIRROR, WORLDS_MIRROR};

    /// A place of `explore.place` this node derived from its content.
    macro_rules! local_place {
        () => {
            "(origin = 'local' OR raw->>'source' = 'content')"
        };
    }

    /// An event of `explore.event` created on this node.
    macro_rules! local_event {
        () => {
            "(origin = 'local' OR EXISTS (SELECT 1 FROM explore.events_local l \
              WHERE l.id = explore.event.id \
                AND (l.signed_payload ? 'create' OR l.signed_at = explore.event.fetched_at)))"
        };
    }

    /// A place this node derived from its content or holds in its local leg.
    pub async fn place(pool: &PgPool, id: &str) -> Result<bool, sqlx::Error> {
        if let Ok(identity) = uuid::Uuid::parse_str(id) {
            let local: Option<bool> = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
                "SELECT EXISTS (SELECT 1 {} WHERE p.identity = known.identity AND {}) FROM explore.world_scene_place known WHERE known.identity = $1",
                crate::places::scene_identity::BOUND_WORLD_SCENE_FROM,
                crate::places::scene_identity::LOCAL_WORLD_SCENE
            ))).bind(identity).fetch_optional(pool).await?;
            if let Some(local) = local {
                return Ok(local);
            }
        }
        sqlx::query_scalar(concat!(
            "SELECT EXISTS (SELECT 1 FROM explore.place WHERE id = $1 AND ",
            local_place!(),
            ") OR EXISTS (SELECT 1 FROM explore.place_world_local WHERE id = $1)"
        ))
        .bind(id)
        .fetch_one(pool)
        .await
    }

    /// A world this node hosts: its local leg, or published here (no
    /// `mirror_source`).
    pub async fn world(pool: &PgPool, name: &str) -> Result<bool, sqlx::Error> {
        sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM explore.place_world_local \
                            WHERE world IS TRUE AND (id = $1 OR lower(world_name) = lower($1))) \
                 OR EXISTS (SELECT 1 FROM worlds.worlds WHERE name = lower($1) AND mirror_source IS NULL)",
        )
        .bind(name.trim().to_lowercase())
        .fetch_one(pool)
        .await
    }

    pub async fn world_scenes(pool: &PgPool, name: &str) -> Result<bool, sqlx::Error> {
        let sql = format!(
            "SELECT EXISTS (SELECT 1 FROM worlds.worlds WHERE name = lower($1) AND mirror_source IS NULL) AND NOT EXISTS (SELECT 1 FROM worlds.world_scenes deployed WHERE deployed.world_name = lower($1) AND NOT EXISTS (SELECT 1 {} WHERE p.world_name = deployed.world_name AND p.entity_id = deployed.entity_id AND {}))",
            crate::places::scene_identity::BOUND_WORLD_SCENE_FROM,
            crate::places::scene_identity::LOCAL_WORLD_SCENE
        );
        sqlx::query_scalar(sqlx::AssertSqlSafe(sql))
            .bind(name.trim().to_lowercase())
            .fetch_one(pool)
            .await
    }

    /// An event created on this node; a local edit of a mirrored event
    /// leaves it the mirror's.
    pub async fn event(pool: &PgPool, id: &str) -> Result<bool, sqlx::Error> {
        sqlx::query_scalar(concat!(
            "SELECT EXISTS (SELECT 1 FROM explore.event WHERE id = $1 AND ",
            local_event!(),
            ")"
        ))
        .bind(id)
        .fetch_one(pool)
        .await
    }

    /// Whether `mirror`'s catalogue holds any row of this node's own.
    pub async fn any(pool: &PgPool, mirror: &str) -> Result<bool, sqlx::Error> {
        let sql = match mirror {
            PLACES_MIRROR => format!(
                "SELECT EXISTS (SELECT 1 FROM explore.place WHERE {}) OR EXISTS (SELECT 1 FROM explore.place_world_local WHERE world IS FALSE) OR EXISTS (SELECT 1 {} WHERE {})",
                local_place!(), crate::places::scene_identity::BOUND_WORLD_SCENE_FROM,
                crate::places::scene_identity::LOCAL_WORLD_SCENE
            ),
            WORLDS_MIRROR => "SELECT EXISTS (SELECT 1 FROM explore.place_world_local WHERE world IS TRUE) OR EXISTS (SELECT 1 FROM worlds.worlds WHERE mirror_source IS NULL)".to_string(),
            EVENTS_MIRROR => concat!("SELECT EXISTS (SELECT 1 FROM explore.event WHERE ", local_event!(), ")").to_string(),
            _ => return Ok(false),
        };
        sqlx::query_scalar(sqlx::AssertSqlSafe(sql))
            .fetch_one(pool)
            .await
    }

    /// The markers above as predicates over a row of `explore.place_indexed`
    /// (or either of its legs), one per mirror the request's list is
    /// restricted to ([`super::OwnRowsOnly`]); none otherwise.
    pub fn place_rows_sql() -> Vec<&'static str> {
        let only = super::OwnRowsOnly::current();
        let mut out = Vec::new();
        if only.places {
            out.push(concat!(
                "(world IS TRUE OR id IN (SELECT id FROM explore.place WHERE ",
                local_place!(),
                ") OR id IN (SELECT id FROM explore.place_world_local))"
            ));
        }
        if only.worlds {
            out.push(
                "(world IS FALSE OR id IN (SELECT id FROM explore.place_world_local) \
                  OR lower(world_name) IN (SELECT name FROM worlds.worlds WHERE mirror_source IS NULL))",
            );
        }
        out
    }

    /// Canonical scene rows are fed by deployments, not the catalogue mirror.
    /// While a mixed batch is restricted to local rows, do not expose a
    /// partially mirrored scene through this separate read surface.
    pub fn world_scenes_local_only() -> bool {
        super::OwnRowsOnly::current().worlds
    }

    pub fn world_scene_rows_sql() -> &'static str {
        let only = super::OwnRowsOnly::current();
        if only.worlds { " AND local" } else { "" }
    }

    /// [`event`] as a predicate over a row of `explore.event`, while the
    /// request's list is restricted to this node's own rows.
    pub fn event_rows_sql() -> Option<&'static str> {
        super::OwnRowsOnly::current()
            .events
            .then_some(local_event!())
    }
}

/// Whether the object a read names is this node's own.
async fn local_object(state: &SharedState, object: &Object<'_>) -> Result<bool, sqlx::Error> {
    let pool = &state.pool;
    match *object {
        Object::Place(id) => origin::place(pool, id).await,
        Object::World(name) => origin::world(pool, name).await,
        Object::WorldScenes(name) => origin::world_scenes(pool, name).await,
        Object::Destination(id) => {
            Ok(origin::place(pool, id).await? || origin::world(pool, id).await?)
        }
        Object::Event(id) => origin::event(pool, id).await,
    }
}

/// Whether this node mirrors `mirror` now, the variable naming its
/// upstream, and its interval.
fn configured(settings: &Settings, mirror: &str) -> (bool, &'static str, Duration) {
    match mirror {
        PLACES_MIRROR => (
            settings.places_upstream.is_some(),
            "PLACES_UPSTREAM_URL",
            settings.places_interval,
        ),
        WORLDS_MIRROR => (
            settings.worlds_upstream.is_some(),
            "WORLDS_UPSTREAM_URL",
            settings.worlds_interval,
        ),
        _ => (
            settings.events_upstream.is_some(),
            "EVENTS_UPSTREAM_URL",
            settings.events_interval,
        ),
    }
}

/// What [`freshness`] found.
struct Found {
    freshness: Freshness,
    /// The first mirror with no copy, and its variable.
    missing: Option<(&'static str, &'static str)>,
    /// How long after `as_of` the answer is stale: until the first of its
    /// copies outgrows its own budget (`as_of::stale_after` of its interval).
    stale_after: Duration,
    /// The mirrors whose rows the list may not serve yet.
    own_rows_only: OwnRowsOnly,
}

/// The middleware `router` layers over every explore route.
pub async fn gate(State(state): State<SharedState>, request: Request, next: Next) -> Response {
    let mut request = request;
    let mut mirrors = catalogues(request.method(), request.uri().path());
    let path = urlencoding::decode(request.uri().path())
        .unwrap_or_else(|_| request.uri().path().into())
        .into_owned();
    let is_batch = request.method() == Method::POST
        && matches!(
            path.trim_end_matches('/'),
            "/api/places" | "/api/places/status" | "/api/destinations"
        );
    if is_batch {
        let (parts, body) = request.into_parts();
        let bytes = match to_bytes(body, 2 * 1024 * 1024).await {
            Ok(bytes) => bytes,
            Err(_) => return axum::http::StatusCode::PAYLOAD_TOO_LARGE.into_response(),
        };
        request = Request::from_parts(parts, Body::from(bytes.clone()));
        let parsed = serde_json::from_slice::<serde_json::Value>(&bytes).ok();
        let destinations = path == "/api/destinations";
        let ids_value = if destinations {
            parsed
                .as_ref()
                .and_then(|v| v.get("ids").or_else(|| v.is_array().then_some(v)))
        } else {
            parsed.as_ref()
        };
        let Some(values) = ids_value.and_then(|v| v.as_array()).filter(|v| {
            if destinations {
                v.len() <= crate::places::destinations::MAX_BATCH_ITEMS
            } else {
                v.iter().filter(|value| value.is_string()).count()
                    <= crate::places::handlers::MAX_BATCH_ITEMS
            }
        }) else {
            if destinations && parsed.as_ref().is_some_and(|v| v.is_object()) {
                // Position/world-name selectors still use both catalogue legs.
                return gate_with_freshness(state, request, next, DESTINATIONS).await;
            }
            return gate_with_freshness(state, request, next, mirrors).await;
        };
        let ids: Vec<String> = values
            .iter()
            .filter_map(|v| v.as_str().map(str::to_string))
            .collect();
        let other_selectors = destinations
            && parsed.as_ref().is_some_and(|v| {
                ["positions", "world_names"].iter().any(|key| {
                    v.get(key)
                        .and_then(|v| v.as_array())
                        .is_some_and(|a| !a.is_empty())
                })
            });
        if let Ok((catalogue, scenes)) = state.places.partition_scene_ids(&ids).await {
            if !scenes.is_empty() {
                // A retired/unbound canonical ID carries only a local
                // tombstone; it cannot expose any mirrored scene metadata.
                let bound = state
                    .places
                    .has_bound_scene_ids(&scenes)
                    .await
                    .unwrap_or(true);
                mirrors = match (catalogue.is_empty() && !other_selectors, bound) {
                    (true, false) => &[],
                    (true, true) => WORLDS,
                    (false, true) => DESTINATIONS,
                    (false, false) => {
                        if destinations {
                            DESTINATIONS
                        } else {
                            PLACES
                        }
                    }
                };
            }
        }
    } else if let Some(Object::Place(id) | Object::Destination(id)) = object(&path) {
        if let Ok((_, scenes)) = state.places.partition_scene_ids(&[id.to_string()]).await {
            if !scenes.is_empty() {
                mirrors = if state
                    .places
                    .has_bound_scene_ids(&scenes)
                    .await
                    .unwrap_or(true)
                {
                    WORLDS
                } else {
                    &[]
                };
            }
        }
    } else if request.method() == Method::GET
        && matches!(
            path.as_str(),
            "/api/places" | "/api/destinations" | "/v1/destinations"
        )
        && request.uri().query().is_some_and(|query| {
            url::form_urlencoded::parse(query.as_bytes()).any(|(key, value)| {
                key == "only_favorites" && matches!(value.as_ref(), "true" | "1")
            })
        })
    {
        mirrors = DESTINATIONS;
    }
    gate_with_freshness(state, request, next, mirrors).await
}

async fn gate_with_freshness(
    state: SharedState,
    request: Request,
    next: Next,
    mirrors: &'static [&'static str],
) -> Response {
    let decoded =
        urlencoding::decode(request.uri().path()).unwrap_or_else(|_| request.uri().path().into());
    let object = object(&decoded).filter(|_| !mirrors.is_empty());
    match freshness(&state, mirrors, object.as_ref()).await {
        Ok(Found {
            freshness,
            missing,
            stale_after,
            own_rows_only,
        }) => {
            let served = as_of::serve(freshness, stale_after, request, next, || {
                let (mirror, var) = missing.unwrap_or((PLACES_MIRROR, "PLACES_UPSTREAM_URL"));
                as_of::no_copy(
                    &format!("{mirror}-mirror"),
                    var,
                    &format!(
                        "the {mirror} mirror has not completed a pass into this database yet, so there is no copy to serve; /api/status shows its progress"
                    ),
                )
            });
            OWN_ROWS_ONLY.scope(own_rows_only, served).await
        }
        Err(err) => {
            // The markers say how old the copy is; they are not the data:
            // serve without a stamp.
            tracing::warn!(%err, "explore: reading the copy's markers failed");
            next.run(request).await
        }
    }
}

/// The freshness of a read over `mirrors`, and the first mirror with no
/// copy. An object of this node's own is live; a list over a mirror with
/// no copy is live while the catalogue holds rows of this node's own, and
/// lists only those.
async fn freshness(
    state: &SharedState,
    mirrors: &[&'static str],
    object: Option<&Object<'_>>,
) -> Result<Found, sqlx::Error> {
    let mut freshness = Freshness::Live;
    let mut missing = None;
    let mut stale_at: Option<NaiveDateTime> = None;
    let mut own_rows_only = OwnRowsOnly::default();
    for &mirror in mirrors {
        let pool = state.pool.clone();
        let last = state
            .pass_markers
            .get_or_fetch(mirror, || async move {
                crate::mirror_state::last_complete_pass(&pool, mirror).await
            })
            .await?;
        let (on, var, interval) = configured(&state.settings, mirror);
        if let Some(last) = last {
            let budget = chrono::Duration::from_std(as_of::stale_after(interval))
                .unwrap_or(chrono::Duration::MAX);
            let at = last
                .checked_add_signed(budget)
                .unwrap_or(NaiveDateTime::MAX);
            stale_at = Some(stale_at.map_or(at, |seen| seen.min(at)));
        }
        let mut this = Freshness::of(on, last);
        if this == Freshness::NoCopy && object.is_none() {
            let pool = state.pool.clone();
            let local = state
                .local_rows
                .get_or_fetch(mirror, || async move { origin::any(&pool, mirror).await })
                .await?;
            if local {
                this = Freshness::Live;
                own_rows_only.insert(mirror);
            }
        }
        if this == Freshness::NoCopy && missing.is_none() {
            missing = Some((mirror, var));
        }
        freshness = freshness.and(this);
    }
    if freshness != Freshness::Live
        && let Some(object) = object
        && local_object(state, object).await?
    {
        freshness = Freshness::Live;
        missing = None;
    }
    if freshness == Freshness::NoCopy
        && let Some(Object::WorldScenes(name)) = object
    {
        let has_local: bool = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
            "SELECT EXISTS (SELECT 1 {} WHERE p.world_name = lower($1) AND {})",
            crate::places::scene_identity::BOUND_WORLD_SCENE_FROM,
            crate::places::scene_identity::LOCAL_WORLD_SCENE
        )))
        .bind(name.trim().to_lowercase())
        .fetch_one(&state.pool)
        .await?;
        if has_local {
            freshness = Freshness::Live;
            missing = None;
            own_rows_only.insert(WORLDS_MIRROR);
        }
    }
    let stale_after = match (freshness, stale_at) {
        (Freshness::AsOf(at), Some(stale_at)) => {
            (stale_at.and_utc() - at).to_std().unwrap_or_default()
        }
        _ => Duration::ZERO,
    };
    Ok(Found {
        freshness,
        missing,
        stale_after,
        own_rows_only,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_of_the_mirrored_catalogues_and_nothing_else() {
        let get = Method::GET;
        let post = Method::POST;
        for (method, path, want) in [
            (&get, "/api/places", PLACES),
            (&post, "/api/places", PLACES),
            (&post, "/api/places/status", PLACES),
            (&get, "/api/places/abc", PLACES),
            (&get, "/api/places/abc/categories", PLACES),
            (&get, "/api/map/places", PLACES),
            (&get, "/api/worlds/x.dcl.eth", WORLDS),
            (&get, "/api/worlds/x.dcl.eth/scenes", WORLDS),
            (&get, "/api/world_names", WORLDS),
            (&get, "/api/destinations", DESTINATIONS),
            (&post, "/api/destinations", DESTINATIONS),
            (&get, "/v1/destinations/abc", DESTINATIONS),
            (&get, "/api/events", EVENTS),
            (&post, "/api/events/search", EVENTS),
            (&get, "/api/events/abc", EVENTS),
            (&get, "/v1/events/abc", EVENTS),
            (&get, "/v1/destinations/abc/events", EVENTS),
        ] {
            assert_eq!(catalogues(method, path), want, "{method} {path}");
        }
        for (method, path) in [
            (&post, "/api/events"),
            (&Method::PATCH, "/api/events/abc"),
            (&Method::PATCH, "/api/places/abc/likes"),
            (&Method::PUT, "/api/places/abc/rating"),
            (&get, "/api/events/attending"),
            (&get, "/api/events/abc/attendees"),
            (&get, "/api/schedules"),
            (&get, "/api/pois"),
            (&get, "/api/status"),
            (&get, "/api/categories"),
            (&get, "/events/sitemap.xml"),
        ] {
            assert!(catalogues(method, path).is_empty(), "{method} {path}");
        }
    }

    #[test]
    fn the_object_a_read_names() {
        for (path, want) in [
            ("/api/places/abc", Some(Object::Place("abc"))),
            ("/api/places/abc/categories", Some(Object::Place("abc"))),
            ("/api/worlds/x.dcl.eth", Some(Object::World("x.dcl.eth"))),
            ("/v1/destinations/abc", Some(Object::Destination("abc"))),
            ("/api/events/abc", Some(Object::Event("abc"))),
            ("/v1/events/abc", Some(Object::Event("abc"))),
            ("/api/places", None),
            ("/api/places/status", None),
            ("/api/events/search", None),
            ("/v1/destinations/abc/events", None),
            ("/api/worlds", None),
        ] {
            assert_eq!(object(path), want, "{path}");
        }
    }
}
