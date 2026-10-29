//! Shared pieces of the client write paths: the multipart create/update
//! body, the standing loads that render a fault as a 500 (never as "not a
//! member"), the ban pre-check that renders a fault as a 500 (never as "not
//! banned"), and the write authority proven against `community_members`
//! plus `community_bans`.

use axum::body::Bytes;
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};

use crate::{
    Address, AuthorityNotEstablished, CommunityId, CommunityPermission, CommunityStanding,
};

use super::State;
use super::http::{err_json, status_and_message_for_refusal};
use super::standing::{Standing, load_standing, load_standing_pair};

pub type Refusal = Box<Response>;

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
        refuse(StatusCode::INTERNAL_SERVER_ERROR, "database error")
    })
}

pub struct MultipartFields {
    pub name: Option<String>,
    pub description: Option<String>,
    pub privacy: Option<String>,
    pub visibility: Option<String>,
    pub place_ids: Vec<String>,
    pub thumbnail: Option<Vec<u8>>,
}

pub fn boundary(headers: &HeaderMap) -> Option<String> {
    let ct = headers.get(header::CONTENT_TYPE)?.to_str().ok()?;
    multer::parse_boundary(ct).ok()
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
        place_ids: Vec::new(),
        thumbnail: None,
    };
    loop {
        let field = match mp.next_field().await {
            Ok(Some(f)) => f,
            Ok(None) => break,
            Err(e) => {
                return Err(refuse(
                    StatusCode::BAD_REQUEST,
                    format!("invalid multipart: {e}"),
                ));
            }
        };
        let fname = field.name().unwrap_or("").to_string();
        match fname.as_str() {
            "thumbnail" => {
                let data = field.bytes().await.unwrap_or_default();
                out.thumbnail = if data.is_empty() {
                    None
                } else {
                    Some(data.to_vec())
                };
            }
            "name" => out.name = Some(field.text().await.unwrap_or_default()),
            "description" => out.description = Some(field.text().await.unwrap_or_default()),
            "privacy" => out.privacy = Some(field.text().await.unwrap_or_default()),
            "visibility" => out.visibility = Some(field.text().await.unwrap_or_default()),
            "placeIds" => {
                let raw = field.text().await.unwrap_or_default();
                if let Ok(parsed) = serde_json::from_str::<Vec<String>>(&raw) {
                    out.place_ids = parsed;
                } else if !raw.trim().is_empty() {
                    out.place_ids = raw
                        .split(',')
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect();
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
    load_standing(&state.pool, community_id, wallet)
        .await
        .map_err(|refusal| {
            refusal_response(&refusal, StatusCode::FORBIDDEN, || {
                "Forbidden: authority could not be established".to_string()
            })
            .into()
        })
}

pub async fn load_client_standing_pair(
    state: &State,
    community_id: CommunityId,
    first: &Address,
    second: &Address,
) -> Result<(Standing, Standing), Refusal> {
    load_standing_pair(&state.pool, community_id, first, second)
        .await
        .map_err(|refusal| {
            refusal_response(&refusal, StatusCode::FORBIDDEN, || {
                "Forbidden: authority could not be established".to_string()
            })
            .into()
        })
}

/// A SQL fault is a 500 rather than "not banned", so an outage denies the
/// write instead of admitting a banned signer. A missing row is "not banned".
pub async fn is_banned(
    state: &State,
    community_id: CommunityId,
    wallet: &Address,
) -> Result<bool, Refusal> {
    let banned: Option<bool> = map_db(
        sqlx::query_scalar("SELECT active FROM social.community_bans WHERE community_id = $1 AND banned_address = $2")
            .bind(community_id)
            .bind(wallet)
            .fetch_optional(&state.pool)
            .await,
    )?;
    Ok(banned.unwrap_or(false))
}

/// Proven against `community_members` plus `community_bans`.
#[derive(Debug)]
pub struct ClientCommunityWriteAuthority(());

impl ClientCommunityWriteAuthority {
    /// `community_bans` pre-check first, then the tier test; both refuse
    /// with a 403.
    pub async fn resolve_requiring_at_least(
        state: &State,
        community_id: CommunityId,
        signer: &Address,
        minimum: CommunityStanding,
    ) -> Result<Self, Refusal> {
        if is_banned(state, community_id, signer).await? {
            return Err(refuse(
                StatusCode::FORBIDDEN,
                "Forbidden: banned from this community",
            ));
        }
        let standing = load_client_standing(state, community_id, signer).await?;
        if !standing.is_at_least(minimum) {
            return Err(refuse(
                StatusCode::FORBIDDEN,
                format!(
                    "Forbidden: signer role {} below required {}",
                    standing.tier().as_wire_text(),
                    minimum.as_wire_text()
                ),
            ));
        }
        Ok(Self(()))
    }

    /// `community_bans` pre-check refuses with a 403; a missing capability
    /// with a 401.
    pub async fn resolve_requiring_capability(
        state: &State,
        community_id: CommunityId,
        signer: &Address,
        capability: CommunityPermission,
        action: &str,
    ) -> Result<Self, Refusal> {
        if is_banned(state, community_id, signer).await? {
            return Err(refuse(
                StatusCode::FORBIDDEN,
                "Forbidden: banned from this community",
            ));
        }
        let standing = load_client_standing(state, community_id, signer).await?;
        if !standing.has(capability) {
            return Err(refuse(
                StatusCode::UNAUTHORIZED,
                format!("The user {} doesn't have permission to {}", signer, action),
            ));
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
            refuse(StatusCode::INTERNAL_SERVER_ERROR, "database error")
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
        assert_eq!(fields.place_ids, vec!["a", "b"]);
        assert!(
            fields.thumbnail.is_none(),
            "an empty thumbnail part is no thumbnail"
        );
        let body = format!(
            "--{b}\r\nContent-Disposition: form-data; name=\"placeIds\"\r\n\r\nx, y\r\n--{b}--\r\n",
            b = boundary
        );
        let fields = parse_multipart(boundary.to_string(), Bytes::from(body))
            .await
            .unwrap();
        assert_eq!(fields.place_ids, vec!["x", "y"]);
    }
}
