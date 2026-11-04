//! The descriptive rule (`shared::as_of`) over the mirrored communities: a
//! listing answers the copy the mirror's last complete pass made, stamped
//! `as_of`, and the unavailable envelope while a configured mirror has never
//! completed one; a mirrored community's own page carries its source's
//! `as_of`. Local communities are this node's own and live.

use std::time::Duration;

use axum::extract::{Request, State};
use axum::middleware::Next;
use axum::response::Response;
use chrono::NaiveDateTime;
use sqlx::PgPool;
use uuid::Uuid;

use super::SharedState;
use crate::shared::as_of::{self, Freshness};
use crate::shared::cache::TtlMap;

/// How long a pass marker is reused before it is read again.
pub const MARKER_TTL: Duration = Duration::from_secs(10);

/// `social.community_mirror_state.last_complete_pass_at` by source (`None`:
/// the newest over every source).
pub type Markers = TtlMap<Option<String>, Option<NaiveDateTime>>;

/// A community's mirror source (`None`: the node's own), reused as the
/// markers are, so a community page costs no lookup of its own per read.
pub type Sources = TtlMap<Uuid, Option<String>>;
pub const SOURCES_CAP: usize = 4096;

const NO_COPY: &str = "the communities mirror has not completed a pass into this database yet, so there is no copy of the upstream listing to serve";

#[derive(Debug, PartialEq, Eq)]
enum Read {
    List,
    One(Uuid),
}

/// The community read a request is, by the path the router sees.
fn read(request: &Request) -> Option<Read> {
    if !as_of::is_read(request.method()) {
        return None;
    }
    let segments: Vec<&str> = request.uri().path().trim_matches('/').split('/').collect();
    match segments.as_slice() {
        ["v1" | "v2", "communities"] => Some(Read::List),
        ["v1" | "v2", "communities", id] => Uuid::parse_str(id).ok().map(Read::One),
        _ => None,
    }
}

async fn last_pass(pool: &PgPool, source: Option<&str>) -> sqlx::Result<Option<NaiveDateTime>> {
    match source {
        Some(source) => Ok(sqlx::query_scalar(
            "SELECT last_complete_pass_at FROM social.community_mirror_state WHERE source_url = $1",
        )
        .bind(source)
        .fetch_optional(pool)
        .await?
        .flatten()),
        None => {
            sqlx::query_scalar(
                "SELECT max(last_complete_pass_at) FROM social.community_mirror_state",
            )
            .fetch_one(pool)
            .await
        }
    }
}

async fn marker(
    state: &SharedState,
    source: Option<String>,
) -> sqlx::Result<Option<NaiveDateTime>> {
    let pool = state.pool.clone();
    state
        .pass_markers
        .get_or_fetch(source.clone(), || async move {
            last_pass(&pool, source.as_deref()).await
        })
        .await
}

async fn freshness(state: &SharedState, read: Read) -> sqlx::Result<Freshness> {
    let configured = state.mirror_source.get().cloned();
    Ok(match read {
        Read::List => Freshness::of(configured.is_some(), marker(state, configured).await?),
        Read::One(id) => {
            let pool = state.pool.clone();
            let source = state
                .sources
                .get_or_fetch(id, || async move {
                    sqlx::query_scalar(
                        "SELECT source_url FROM social.community_mirror WHERE community_id = $1",
                    )
                    .bind(id)
                    .fetch_optional(&pool)
                    .await
                })
                .await?;
            match source {
                Some(source) => Freshness::of(false, marker(state, Some(source)).await?),
                None => Freshness::Live,
            }
        }
    })
}

/// The middleware `router` layers over every community route.
pub async fn gate(State(state): State<SharedState>, request: Request, next: Next) -> Response {
    let Some(read) = read(&request) else {
        return next.run(request).await;
    };
    let freshness = match freshness(&state, read).await {
        Ok(f) => f,
        Err(err) => {
            // The marker is a stamp, not the data: serve without it.
            tracing::warn!(%err, "communities: reading the mirror's pass marker failed");
            return next.run(request).await;
        }
    };
    let stale_after = as_of::stale_after(state.settings.mirror_interval);
    as_of::serve(freshness, stale_after, request, next, || {
        as_of::no_copy("communities-mirror", "COMMUNITIES_UPSTREAM_URL", NO_COPY)
    })
    .await
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use axum::Router;
    use axum::body::Body;
    use axum::http::{Method, StatusCode};
    use serde_json::Value;
    use tower::ServiceExt;

    use super::*;
    use crate::communities::{Settings, State as CommunitiesState, router};
    use crate::http::Outbound;

    #[test]
    fn listings_and_community_pages_are_the_reads() {
        let id = Uuid::new_v4();
        let req = |m: Method, p: String| {
            Request::builder()
                .method(m)
                .uri(p)
                .body(Body::empty())
                .unwrap()
        };
        assert_eq!(
            read(&req(Method::GET, "/v1/communities".into())),
            Some(Read::List)
        );
        assert_eq!(
            read(&req(Method::GET, "/v2/communities?search=x".into())),
            Some(Read::List)
        );
        assert_eq!(
            read(&req(Method::GET, format!("/v1/communities/{id}"))),
            Some(Read::One(id))
        );
        assert_eq!(read(&req(Method::POST, "/v1/communities".into())), None);
        assert_eq!(
            read(&req(Method::GET, format!("/v1/communities/{id}/members"))),
            None
        );
        assert_eq!(
            read(&req(Method::GET, "/v1/communities/not-a-uuid".into())),
            None
        );
    }

    async fn get(state: Arc<CommunitiesState>, uri: &str) -> (StatusCode, Value) {
        let app = Router::new()
            .merge(router(state.clone()))
            .nest("/social-api", router(state));
        let res = app
            .oneshot(Request::get(uri).body(Body::empty()).unwrap())
            .await
            .unwrap();
        let status = res.status();
        let bytes = axum::body::to_bytes(res.into_body(), 1 << 20)
            .await
            .unwrap();
        (
            status,
            serde_json::from_slice(&bytes).unwrap_or(Value::Null),
        )
    }

    /// A configured mirror with no complete pass has no copy of the upstream
    /// listing (503, not the local rows as if they were the catalogue); once
    /// a pass completes the listing and a mirrored community carry its time
    /// as `as_of`, while a local community stays live.
    #[tokio::test]
    async fn the_mirrored_listing_serves_its_copy_with_as_of_and_503_before_its_first_pass() {
        let Some(pool) = crate::db::scratch::pool(4).await else {
            return;
        };
        let storage_dir = std::env::temp_dir().join(format!("catalyst-copies-{}", Uuid::new_v4()));
        let storage = Arc::new(
            crate::storage::Storage::from_config(&storage_dir)
                .await
                .unwrap(),
        );
        let source = format!("http://upstream-{}.test", Uuid::new_v4().simple());
        let state = || {
            let state = Arc::new(CommunitiesState::new(
                pool.clone(),
                storage.clone(),
                Settings::for_tests(),
                None,
                &Outbound::for_tests(),
                CommunitiesState::explore_for_tests(&pool),
            ));
            state.set_mirror_source(&source);
            state
        };
        let (mirrored, local) = (Uuid::new_v4(), Uuid::new_v4());
        let owner = format!("0x{}", "ab".repeat(20));
        for (id, name) in [(mirrored, "mirrored one"), (local, "local one")] {
            sqlx::query(
                "INSERT INTO social.communities (id, name, owner_address) VALUES ($1, $2, $3)",
            )
            .bind(id)
            .bind(format!("{name} {id}"))
            .bind(&owner)
            .execute(&pool)
            .await
            .unwrap();
        }
        sqlx::query(
            "INSERT INTO social.community_mirror (community_id, source_url, fetched_at) VALUES ($1, $2, timezone('UTC', now()))",
        )
        .bind(mirrored)
        .bind(&source)
        .execute(&pool)
        .await
        .unwrap();

        for uri in ["/v1/communities", "/social-api/v2/communities"] {
            let (status, body) = get(state(), uri).await;
            assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{uri}: {body}");
            assert_eq!(body["source"], "communities-mirror");
            assert_eq!(body["configure"], "COMMUNITIES_UPSTREAM_URL");
        }

        sqlx::query(
            "INSERT INTO social.community_mirror_state (source_url, last_complete_pass_at, updated_at) \
             VALUES ($1, '2026-03-04 05:06:07.5', '2026-03-04 05:06:07.5')",
        )
        .bind(&source)
        .execute(&pool)
        .await
        .unwrap();
        for uri in [
            "/v1/communities".to_string(),
            "/social-api/v2/communities".to_string(),
            format!("/v1/communities/{mirrored}"),
        ] {
            let (status, body) = get(state(), &uri).await;
            assert_eq!(status, StatusCode::OK, "{uri}: {body}");
            assert_eq!(body["as_of"], "2026-03-04T05:06:07.500Z", "{uri}");
            assert_eq!(body["stale_after_secs"], 7200, "{uri}");
        }
        let (status, body) = get(state(), &format!("/v1/communities/{local}")).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert!(
            body.get("as_of").is_none(),
            "a local community is live: {body}"
        );

        // A community's source is read once per marker TTL, not per GET:
        // with its mirror row gone, a state that already read it still
        // stamps the page, and a fresh one sees a local community.
        let warm = state();
        let page = format!("/v1/communities/{mirrored}");
        let (_, body) = get(warm.clone(), &page).await;
        assert_eq!(body["as_of"], "2026-03-04T05:06:07.500Z");
        sqlx::query("DELETE FROM social.community_mirror WHERE community_id = $1")
            .bind(mirrored)
            .execute(&pool)
            .await
            .unwrap();
        let (_, body) = get(warm, &page).await;
        assert_eq!(body["as_of"], "2026-03-04T05:06:07.500Z", "cached: {body}");
        let (_, body) = get(state(), &page).await;
        assert!(body.get("as_of").is_none(), "uncached: {body}");

        sqlx::query("DELETE FROM social.communities WHERE id = ANY($1)")
            .bind(vec![mirrored, local])
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("DELETE FROM social.community_mirror_state WHERE source_url = $1")
            .bind(&source)
            .execute(&pool)
            .await
            .unwrap();
    }
}
