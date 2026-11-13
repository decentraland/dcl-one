//! Bounded migration listener. This router starts no domain jobs or providers.
//! Its pilot read is internal, not a replacement for the public Places API.
use crate::{
    migration_replay::{ReplayReceipt, ReplayRequest},
    shared::errors::ApiError,
};
use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, Path, Query, Request, State},
    http::{HeaderMap, StatusCode},
    middleware::{self, Next},
    response::Response,
    routing::{get, post},
};
use hmac::{Hmac, KeyInit, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use sqlx::{PgConnection, PgPool, Row};
use uuid::Uuid;

#[derive(Clone)]
pub struct Follower {
    pool: PgPool,
    stream: String,
    generation: Uuid,
    max_age_seconds: i64,
    token_tag: Vec<u8>,
}
impl Follower {
    pub fn new(
        pool: PgPool,
        stream: String,
        generation: Uuid,
        max_age_seconds: i64,
        token: &str,
    ) -> anyhow::Result<Self> {
        anyhow::ensure!(
            !stream.is_empty()
                && stream.len() <= 64
                && stream
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_'),
            "invalid follower stream"
        );
        anyhow::ensure!(
            !generation.is_nil() && (1..=3600).contains(&max_age_seconds),
            "invalid follower generation or freshness bound"
        );
        anyhow::ensure!(
            (32..=4096).contains(&token.len()),
            "follower token must be 32 to 4096 bytes"
        );
        Ok(Self {
            pool,
            stream,
            generation,
            max_age_seconds,
            token_tag: tag(token),
        })
    }
}
fn tag(token: &str) -> Vec<u8> {
    let mut mac =
        Hmac::<Sha256>::new_from_slice(token.as_bytes()).expect("HMAC accepts any key length");
    mac.update(b"dcl-one migration follower bearer v1");
    mac.finalize().into_bytes().to_vec()
}
fn authorized(headers: &HeaderMap, state: &Follower) -> bool {
    if headers.get_all("authorization").iter().count() != 1 {
        return false;
    }
    let Some(token) = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
    else {
        return false;
    };
    if !(32..=4096).contains(&token.len()) {
        return false;
    }
    let mut mac =
        Hmac::<Sha256>::new_from_slice(token.as_bytes()).expect("HMAC accepts any key length");
    mac.update(b"dcl-one migration follower bearer v1");
    mac.verify_slice(&state.token_tag).is_ok()
}
async fn authenticate(
    State(state): State<Follower>,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    if !authorized(request.headers(), &state) {
        return Err(StatusCode::UNAUTHORIZED);
    }
    Ok(next.run(request).await)
}
pub fn router(state: Follower) -> Router {
    let private = Router::new()
        .route("/migration/places/{id}/favorites", get(favorites))
        .route("/internal/migration/favorites/replay", post(replay))
        .route_layer(middleware::from_fn_with_state(state.clone(), authenticate));
    Router::new()
        .route(
            "/healthz",
            get(|| async { Json(serde_json::json!({"healthy":true,"mode":"migration-follower"})) }),
        )
        .route("/readyz", get(ready))
        .merge(private)
        .layer(DefaultBodyLimit::max(16 * 1024))
        .with_state(state)
}
#[derive(Serialize)]
pub struct Readiness {
    ready: bool,
    applied_position: Option<i64>,
    captured_position: Option<i64>,
    observed_position: Option<i64>,
    capture_age_seconds: Option<f64>,
}
async fn readiness(
    connection: &mut PgConnection,
    state: &Follower,
) -> Result<Readiness, sqlx::Error> {
    let row = sqlx::query("SELECT s.generation = $2 AND NOT s.paused AS admitted, (s.applied_lsn - '0/0'::pg_lsn)::bigint AS applied, c.captured_position, c.observed_position, EXTRACT(EPOCH FROM (clock_timestamp() - c.last_contact_at))::float8 AS age, EXISTS (SELECT 1 FROM migration_sync.inbox i WHERE i.stream=s.name AND i.status='blocked') AS blocked FROM migration_sync.streams s LEFT JOIN migration_sync.capture_sources c ON c.stream=s.name AND c.generation=s.generation WHERE s.name=$1")
        .bind(&state.stream).bind(state.generation).fetch_optional(connection).await?;
    let mut result = Readiness {
        ready: false,
        applied_position: None,
        captured_position: None,
        observed_position: None,
        capture_age_seconds: None,
    };
    if let Some(row) = row {
        result.applied_position = row.try_get("applied")?;
        result.captured_position = row.try_get("captured_position")?;
        result.observed_position = row.try_get("observed_position")?;
        result.capture_age_seconds = row.try_get("age")?;
        result.ready = row.try_get::<bool, _>("admitted")?
            && !row.try_get::<bool, _>("blocked")?
            && result.applied_position.is_some()
            && result.applied_position == result.captured_position
            && result.captured_position == result.observed_position
            && result
                .capture_age_seconds
                .is_some_and(|age| age >= 0.0 && age <= state.max_age_seconds as f64);
    }
    Ok(result)
}
async fn ready(State(state): State<Follower>) -> (StatusCode, Json<Readiness>) {
    let result = async {
        let mut connection = state.pool.acquire().await?;
        readiness(&mut connection, &state).await
    }
    .await;
    let status = result.unwrap_or(Readiness {
        ready: false,
        applied_position: None,
        captured_position: None,
        observed_position: None,
        capture_age_seconds: None,
    });
    (
        if status.ready {
            StatusCode::OK
        } else {
            StatusCode::SERVICE_UNAVAILABLE
        },
        Json(status),
    )
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FavoriteQuery {
    user: String,
}
async fn favorites(
    State(state): State<Follower>,
    Path(id): Path<String>,
    Query(query): Query<FavoriteQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    if !Uuid::parse_str(&id).is_ok_and(|value| value.to_string() == id)
        || query.user.len() != 42
        || !query.user.starts_with("0x")
        || !query.user[2..]
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(ApiError::bad_request("canonical place and wallet required"));
    }
    let mut tx = state.pool.begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .execute(&mut *tx)
        .await?;
    if !readiness(&mut tx, &state).await?.ready {
        return Err(ApiError::service_unavailable("follower not caught up"));
    }
    let row = sqlx::query("SELECT EXISTS (SELECT 1 FROM explore.user_favorites f WHERE f.entity_id=p.id AND f.\"user\"=$2) AS favorite, (SELECT COUNT(*) FROM explore.user_favorites f WHERE f.entity_id=p.id) AS favorites FROM explore.place p WHERE p.id=$1 AND NOT p.world AND p.origin='local' AND EXISTS (SELECT 1 FROM migration_sync.capture_scope scope WHERE scope.stream=$3 AND scope.generation=$4 AND scope.entity_id=p.id::uuid)")
        .bind(&id).bind(&query.user).bind(&state.stream).bind(state.generation).fetch_optional(&mut *tx).await?.ok_or_else(|| ApiError::not_found("place not found"))?;
    let result = serde_json::json!({"id":id,"user_favorite":row.try_get::<bool,_>("favorite")?,"favorites":row.try_get::<i64,_>("favorites")?});
    tx.commit().await?;
    Ok(Json(result))
}
async fn replay(
    State(state): State<Follower>,
    Json(request): Json<ReplayRequest>,
) -> Result<Json<ReplayReceipt>, ApiError> {
    if request.stream != state.stream || request.generation != state.generation {
        return Err(ApiError::conflict("follower stream generation mismatch"));
    }
    crate::migration_replay::replay(&state.pool, &request)
        .await
        .map(Json)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::Body, http::Request};
    use tower::ServiceExt;
    fn app() -> Router {
        router(
            Follower::new(
                sqlx::postgres::PgPoolOptions::new()
                    .connect_lazy("postgres://localhost/follower_no_connection")
                    .unwrap(),
                "places".into(),
                Uuid::new_v4(),
                30,
                "01234567890123456789012345678901",
            )
            .unwrap(),
        )
    }
    #[tokio::test]
    async fn health_and_unknown_mutations_need_no_database() {
        for (method, path, expected) in [
            ("GET", "/healthz", 200),
            ("PATCH", "/api/places/example/favorites", 404),
            ("POST", "/credits", 404),
            ("GET", "/ws", 404),
            ("POST", "/healthz", 405),
        ] {
            let response = app()
                .oneshot(
                    Request::builder()
                        .method(method)
                        .uri(path)
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status().as_u16(), expected);
        }
    }
    #[tokio::test]
    async fn internal_auth_precedes_json_and_database() {
        for token in [
            None,
            Some("Bearer wrong"),
            Some("Bearer 01234567890123456789012345678900"),
        ] {
            let mut request = Request::builder()
                .method("POST")
                .uri("/internal/migration/favorites/replay");
            if let Some(token) = token {
                request = request.header("authorization", token);
            }
            assert_eq!(
                app()
                    .oneshot(request.body(Body::from("invalid JSON")).unwrap())
                    .await
                    .unwrap()
                    .status(),
                StatusCode::UNAUTHORIZED
            );
        }
    }
    #[tokio::test]
    async fn authenticated_replay_rejects_other_generation_without_database() {
        let body =
            serde_json::json!({"stream":"places","generation":Uuid::new_v4(),"commit_lsn":"0/1"});
        let response = app()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/internal/migration/favorites/replay")
                    .header("authorization", "Bearer 01234567890123456789012345678901")
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CONFLICT);
    }

    #[tokio::test]
    async fn duplicate_bearer_headers_are_refused() {
        let response = app()
            .oneshot(
                Request::builder()
                    .uri("/migration/places/example/favorites")
                    .header("authorization", "Bearer 01234567890123456789012345678901")
                    .header("authorization", "Bearer 01234567890123456789012345678901")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn readiness_requires_fresh_capture_and_equal_positions() {
        let Some(pool) = crate::db::scratch::isolated_pool(2).await else {
            return;
        };
        let generation = Uuid::new_v4();
        let state = Follower::new(
            pool.clone(),
            "places".into(),
            generation,
            30,
            "01234567890123456789012345678901",
        )
        .unwrap();
        let mut connection = pool.acquire().await.unwrap();
        assert!(!readiness(&mut connection, &state).await.unwrap().ready);
        sqlx::query("INSERT INTO migration_sync.streams(name,source,generation,baseline_lsn,applied_lsn,paused) VALUES ('places','places',$1,'0/0','0/0',false)")
            .bind(generation).execute(&mut *connection).await.unwrap();
        assert!(!readiness(&mut connection, &state).await.unwrap().ready);
        sqlx::query("INSERT INTO migration_sync.capture_sources(stream,generation,adapter,captured_position,observed_position,last_contact_at) VALUES ('places',$1,'places_outbox_v1',0,0,now())")
            .bind(generation).execute(&mut *connection).await.unwrap();
        assert!(readiness(&mut connection, &state).await.unwrap().ready);
        for change in [
            "UPDATE migration_sync.capture_sources SET observed_position=1",
            "UPDATE migration_sync.capture_sources SET captured_position=1",
            "UPDATE migration_sync.streams SET applied_lsn='0/1', paused=true",
            "UPDATE migration_sync.streams SET paused=false; UPDATE migration_sync.capture_sources SET last_contact_at=now()-interval '31 seconds'",
        ] {
            sqlx::raw_sql(change)
                .execute(&mut *connection)
                .await
                .unwrap();
            assert!(!readiness(&mut connection, &state).await.unwrap().ready);
        }
        sqlx::query("UPDATE migration_sync.capture_sources SET last_contact_at=now()")
            .execute(&mut *connection)
            .await
            .unwrap();
        assert!(readiness(&mut connection, &state).await.unwrap().ready);
        // Even a valid local Genesis row must belong to the captured pilot scope.
        let id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO explore.place(id,base_position,raw,origin) VALUES ($1,'0,0','{}','local')",
        )
        .bind(id.to_string())
        .execute(&mut *connection)
        .await
        .unwrap();
        let path = format!(
            "/migration/places/{id}/favorites?user=0x0000000000000000000000000000000000000001"
        );
        let response = router(state.clone())
            .oneshot(
                Request::builder()
                    .uri(&path)
                    .header("authorization", "Bearer 01234567890123456789012345678901")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        sqlx::query("INSERT INTO migration_sync.capture_scope(stream,generation,entity_id) VALUES ('places',$1,$2)")
            .bind(generation).bind(id).execute(&mut *connection).await.unwrap();
        let response = router(state.clone())
            .oneshot(
                Request::builder()
                    .uri(&path)
                    .header("authorization", "Bearer 01234567890123456789012345678901")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let other = Follower::new(
            pool.clone(),
            "places".into(),
            Uuid::new_v4(),
            30,
            "01234567890123456789012345678901",
        )
        .unwrap();
        assert!(!readiness(&mut connection, &other).await.unwrap().ready);
    }
    #[tokio::test]
    async fn authenticated_replay_body_is_bounded() {
        let response = app()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/internal/migration/favorites/replay")
                    .header("authorization", "Bearer 01234567890123456789012345678901")
                    .header("content-type", "application/json")
                    .body(Body::from("x".repeat(16 * 1024 + 1)))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
    }
}
