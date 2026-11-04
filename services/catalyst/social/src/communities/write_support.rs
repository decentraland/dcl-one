//! Shared pieces of the client write paths: the multipart create/update
//! body, the standing loads that render a fault as a 500 (never as "not a
//! member"), the ban pre-check that renders a fault as a 500 (never as "not
//! banned"), and the write authority proven against `community_members`.
//! Handlers apply the ban checks with their route-specific refusal contracts.

use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::Duration;

use axum::body::Bytes;
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};

use crate::{
    Address, AuthorityNotEstablished, CommunityId, CommunityPermission, CommunityStanding,
};

use super::State;
use super::http::{err_json, status_and_message_for_refusal};
use super::standing::{Standing, load_standing_from_database, load_standing_pair_from_database};

pub type Refusal = Box<Response>;

/// Community membership mutations serialize across source-defined independent
/// commits, including bulk acceptance. Keep the key stable across every writer.
pub(super) fn membership_lock(community: CommunityId) -> sqlx::postgres::PgAdvisoryLock {
    sqlx::postgres::PgAdvisoryLock::new(format!("social-community-membership:{community}"))
}

/// Own the existing pooled connection while acquisition can still be cancelled.
/// SQLx's native guard queues an unlock, but returning a cancelled blocked query
/// to the pool also waits for that query. Close only unsuccessful acquisitions.
pub(super) struct MembershipConnection {
    connection: sqlx::pool::PoolConnection<sqlx::Postgres>,
    acquiring: Arc<AtomicBool>,
}

impl AsRef<sqlx::PgConnection> for MembershipConnection {
    fn as_ref(&self) -> &sqlx::PgConnection {
        &self.connection
    }
}

impl AsMut<sqlx::PgConnection> for MembershipConnection {
    fn as_mut(&mut self) -> &mut sqlx::PgConnection {
        &mut self.connection
    }
}

impl Drop for MembershipConnection {
    fn drop(&mut self) {
        if self.acquiring.load(Ordering::Relaxed) {
            self.connection.close_on_drop();
        }
    }
}

/// Bound checkout and acquisition to one second. The short acquisition-only
/// transaction scopes PostgreSQL's lock timeout; its COMMIT restores autocommit
/// before any source-defined independent writes. On every error or cancellation
/// the armed owner closes the connection, including an unfinished BEGIN/COMMIT.
/// Successful guards retain SQLx's native queued unlock and same backend reuse.
pub(super) async fn acquire_membership(
    pool: &sqlx::PgPool,
    community: CommunityId,
) -> sqlx::Result<sqlx::postgres::PgAdvisoryLockGuard<MembershipConnection>> {
    tokio::time::timeout(Duration::from_secs(1), async {
        let acquiring = Arc::new(AtomicBool::new(true));
        let mut connection = MembershipConnection {
            connection: pool.acquire().await?,
            acquiring: acquiring.clone(),
        };
        sqlx::query("BEGIN").execute(connection.as_mut()).await?;
        sqlx::query("SET LOCAL lock_timeout = '750ms'")
            .execute(connection.as_mut())
            .await?;
        let mut guard = membership_lock(community).acquire(connection).await?;
        sqlx::query("COMMIT").execute(&mut *guard).await?;
        acquiring.store(false, Ordering::Relaxed);
        Ok(guard)
    })
    .await
    .map_err(|_| sqlx::Error::PoolTimedOut)?
}

pub fn refuse(code: StatusCode, message: impl Into<String>) -> Refusal {
    Box::new(err_json(code, message))
}

pub fn parse_community_id(s: &str) -> Result<CommunityId, Refusal> {
    CommunityId::parse(s).map_err(|_| {
        Box::new(
            super::http::CommError::bad_request(super::http::INVALID_IDENTIFIER).into_response(),
        )
    })
}

pub fn map_db<T>(r: Result<T, sqlx::Error>) -> Result<T, Refusal> {
    r.map_err(|e| {
        tracing::error!(error = %e, "communities client-write database error");
        Box::new(super::http::database_error(e).into_response())
    })
}

pub struct MultipartFields {
    pub name: Option<String>,
    pub description: Option<String>,
    pub privacy: Option<String>,
    pub visibility: Option<String>,
    /// Upstream's `fields-validator.ts`: absent or empty means no change; a
    /// JSON array of strings replaces the places (`[]` removes them all).
    pub place_ids: Option<Vec<String>>,
    pub thumbnail: Option<Vec<u8>>,
}

pub fn boundary(headers: &HeaderMap) -> Option<String> {
    let ct = headers.get(header::CONTENT_TYPE)?.to_str().ok()?;
    multer::parse_boundary(ct).ok()
}

/// The upstream native FormData parser also accepts URL-encoded forms.
/// Multipart file fields and text fields occupy separate maps: only a file
/// named `thumbnail` supplies image bytes.
pub async fn parse_form(headers: &HeaderMap, body: Bytes) -> Result<MultipartFields, Refusal> {
    if body.len() > 1024 * 1024
        || headers
            .get(header::CONTENT_LENGTH)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<u64>().ok())
            .is_some_and(|length| length > 1024 * 1024)
    {
        return Err(Box::new(
            super::http::CommError::bad_request("Request body too large").into_response(),
        ));
    }
    if headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|ct| {
            ct.split(';').next().is_some_and(|mime| {
                mime.trim()
                    .eq_ignore_ascii_case("application/x-www-form-urlencoded")
            })
        })
    {
        let mut out = MultipartFields {
            name: None,
            description: None,
            privacy: None,
            visibility: None,
            place_ids: None,
            thumbnail: None,
        };
        for (name, value) in url::form_urlencoded::parse(&body) {
            match name.as_ref() {
                "name" => out.name = Some(value.into_owned()),
                "description" => out.description = Some(value.into_owned()),
                "privacy" => out.privacy = Some(value.into_owned()),
                "visibility" => out.visibility = Some(value.into_owned()),
                "placeIds" if !value.is_empty() => {
                    out.place_ids = Some(serde_json::from_str(&value).map_err(|_| {
                        refuse(
                            StatusCode::BAD_REQUEST,
                            "placeIds must be a valid JSON array",
                        )
                    })?)
                }
                "placeIds" => out.place_ids = None,
                _ => {}
            }
        }
        return Ok(out);
    }
    let boundary = boundary(headers).ok_or_else(|| {
        Box::new(
            super::http::CommError::bad_request("Invalid multipart/form-data body").into_response(),
        )
    })?;
    parse_multipart(boundary, body).await
}

/// The fields are handed on as sent (no case folding): `Private` is refused
/// by the enum parse, not quietly accepted.
pub async fn parse_multipart(boundary: String, body: Bytes) -> Result<MultipartFields, Refusal> {
    let stream = futures::stream::once(async move { Ok::<Bytes, std::io::Error>(body) });
    let mut mp = multer::Multipart::new(stream, boundary);
    let mut out = MultipartFields {
        name: None,
        description: None,
        privacy: None,
        visibility: None,
        place_ids: None,
        thumbnail: None,
    };
    loop {
        let field = match mp.next_field().await {
            Ok(Some(f)) => f,
            Ok(None) => break,
            Err(_) => {
                return Err(Box::new(
                    super::http::CommError::bad_request("Invalid multipart/form-data body")
                        .into_response(),
                ));
            }
        };
        let fname = field.name().unwrap_or("").to_string();
        let file = field.file_name().is_some();
        if (fname == "thumbnail") != file {
            let _ = field.bytes().await;
            continue;
        }
        match fname.as_str() {
            "thumbnail" => {
                let data = field.bytes().await.unwrap_or_default();
                out.thumbnail = Some(data.to_vec());
            }
            "name" => out.name = Some(field.text().await.unwrap_or_default()),
            "description" => out.description = Some(field.text().await.unwrap_or_default()),
            "privacy" => out.privacy = Some(field.text().await.unwrap_or_default()),
            "visibility" => out.visibility = Some(field.text().await.unwrap_or_default()),
            "placeIds" => {
                let raw = field.text().await.unwrap_or_default();
                if raw.is_empty() {
                    out.place_ids = None;
                } else {
                    out.place_ids =
                        Some(serde_json::from_str::<Vec<String>>(&raw).map_err(|_| {
                            refuse(
                                StatusCode::BAD_REQUEST,
                                "placeIds must be a valid JSON array",
                            )
                        })?);
                }
            }
            _ => {
                let _ = field.bytes().await;
            }
        }
    }
    Ok(out)
}

pub fn refusal_response(
    refusal: &AuthorityNotEstablished,
    status_when_the_principal_lacks_the_authority: StatusCode,
    message_when_the_principal_lacks_the_authority: impl FnOnce() -> String,
) -> Response {
    let (status, message) = status_and_message_for_refusal(
        refusal,
        status_when_the_principal_lacks_the_authority,
        message_when_the_principal_lacks_the_authority,
    );
    err_json(status, message)
}

/// A fault is a 500 (BC-1), never a silent "not a member".
pub async fn load_client_standing(
    state: &State,
    community_id: CommunityId,
    wallet: &Address,
) -> Result<Standing, Refusal> {
    map_db(load_standing_from_database(&state.pool, community_id, wallet).await)
}

pub async fn load_client_standing_pair(
    state: &State,
    community_id: CommunityId,
    first: &Address,
    second: &Address,
) -> Result<(Standing, Standing), Refusal> {
    map_db(load_standing_pair_from_database(&state.pool, community_id, first, second).await)
}

/// A SQL fault is a 500 rather than "not banned", so an outage denies the
/// write instead of admitting a banned signer. A missing row is "not banned".
pub async fn is_banned(
    state: &State,
    community_id: CommunityId,
    wallet: &Address,
) -> Result<bool, Refusal> {
    map_db(super::queries::banned(&state.pool, community_id, wallet).await)
}

/// Proves a capability against the active community's membership projection.
#[derive(Debug)]
pub struct ClientCommunityWriteAuthority(());

impl ClientCommunityWriteAuthority {
    /// A missing capability refuses with a 401. Handlers apply any separate
    /// ban check using their route's response contract.
    pub async fn resolve_requiring_capability(
        state: &State,
        community_id: CommunityId,
        signer: &Address,
        capability: CommunityPermission,
        action: &str,
    ) -> Result<Self, Refusal> {
        if !map_db(super::queries::community_exists(&state.pool, community_id, false).await)? {
            return Err(Box::new(
                super::http::CommError::not_found(format!("Community not found: {community_id}"))
                    .into_response(),
            ));
        }
        let standing = load_client_standing(state, community_id, signer).await?;
        if !standing.has(capability) {
            return Err(Box::new(super::http::handler_error(
                StatusCode::UNAUTHORIZED,
                format!("The user {} doesn't have permission to {}", signer, action),
            )));
        }
        Ok(Self(()))
    }
}

/// BC-3: the privacy, membership and ban lookups all propagate a fault as a
/// 500 instead of failing open.
pub async fn validate_like_unlike_access(
    state: &State,
    community_id: CommunityId,
    signer: &Address,
) -> Result<(), Refusal> {
    let private = super::queries::is_private(&state.pool, community_id)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, %community_id, "community privacy lookup failed");
            Box::new(super::http::database_error(e).into_response())
        })?;
    let standing = load_client_standing(state, community_id, signer).await?;
    if private && standing.tier() == CommunityStanding::NotAMember {
        return Err(refuse(
            StatusCode::UNAUTHORIZED,
            format!(
                "{} is not a member of private community {}. You need to be a member to like/unlike posts in this community.",
                signer, community_id
            ),
        ));
    }
    if is_banned(state, community_id, signer).await? {
        return Err(refuse(
            StatusCode::UNAUTHORIZED,
            format!(
                "{} is banned from community {}. You cannot like/unlike posts in this community.",
                signer, community_id
            ),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn multipart_hands_the_validator_the_raw_case() {
        let boundary = "XBOUNDARY";
        let body = format!(
            "--{b}\r\nContent-Disposition: form-data; name=\"privacy\"\r\n\r\nPrivate\r\n\
             --{b}\r\nContent-Disposition: form-data; name=\"visibility\"\r\n\r\nALL\r\n\
             --{b}\r\nContent-Disposition: form-data; name=\"placeIds\"\r\n\r\n[\"a\",\"b\"]\r\n\
             --{b}\r\nContent-Disposition: form-data; name=\"thumbnail\"; filename=\"t.png\"\r\nContent-Type: image/png\r\n\r\n\r\n\
             --{b}--\r\n",
            b = boundary
        );
        let fields = parse_multipart(boundary.to_string(), Bytes::from(body))
            .await
            .expect("well-formed multipart");
        assert_eq!(fields.privacy.as_deref(), Some("Private"));
        assert_eq!(fields.visibility.as_deref(), Some("ALL"));
        assert_eq!(
            fields.place_ids,
            Some(vec!["a".to_string(), "b".to_string()])
        );
        assert_eq!(
            fields.thumbnail,
            Some(Vec::new()),
            "an empty file is validated as an undersized thumbnail"
        );
        // upstream `fields-validator.ts`: anything but a JSON array of
        // strings is refused; an empty field is no change, `[]` clears
        let part = |value: &str| {
            Bytes::from(format!(
                "--{b}\r\nContent-Disposition: form-data; name=\"placeIds\"\r\n\r\n{value}\r\n--{b}--\r\n",
                b = boundary
            ))
        };
        for bad in ["x, y", "[1]", "{}"] {
            let refusal = parse_multipart(boundary.to_string(), part(bad))
                .await
                .err()
                .unwrap_or_else(|| panic!("{bad:?} is refused"));
            assert_eq!(refusal.status(), StatusCode::BAD_REQUEST, "{bad:?}");
        }
        let fields = parse_multipart(boundary.to_string(), part(""))
            .await
            .unwrap();
        assert_eq!(fields.place_ids, None);
        let fields = parse_multipart(boundary.to_string(), part("[]"))
            .await
            .unwrap();
        assert_eq!(fields.place_ids, Some(Vec::new()));
    }
}
