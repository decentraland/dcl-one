//! The upstream profile-images service (`PROFILE_IMAGES_ORIGIN_URL`): reached
//! through the guarded outbound client (which follows its redirect to the
//! bucket), the entity id already validated as a CID, the body capped. A
//! transient failure (a connection or timeout error, a 5xx, a 429) is retried
//! with backoff until the caller's deadline; a 404 is final unless the caller
//! knows the profile is new enough for the upstream to be still drawing it.

use std::time::Duration;

use bytes::Bytes;
use tokio::time::Instant;

use super::cache::ImageKind;
use crate::http::{Outbound, read_body_capped};

const MAX_IMAGE_BYTES: usize = 8 * 1024 * 1024;
/// One attempt's limit; a slow attempt leaves time for another.
const ATTEMPT_TIMEOUT: Duration = Duration::from_secs(10);
const FIRST_BACKOFF: Duration = Duration::from_millis(250);
const MAX_BACKOFF: Duration = Duration::from_secs(4);
/// A `Retry-After` longer than this is not waited for.
const MAX_RETRY_AFTER: Duration = Duration::from_secs(5);

#[derive(Debug)]
pub enum OriginResult {
    Hit(Bytes),
    NotFound,
    Error(String),
}

enum Attempt {
    Done(OriginResult),
    Retry {
        after: Option<Duration>,
        why: String,
    },
}

pub struct Origin {
    http: Outbound,
    base_url: String,
}

impl Origin {
    pub fn new(http: Outbound, base_url: String) -> Self {
        Self {
            http,
            base_url: base_url.trim_end_matches('/').to_string(),
        }
    }

    /// The image, retrying transient failures (and a 404 when
    /// `retry_not_found`) with backoff while `deadline` allows another try.
    pub async fn fetch_until(
        &self,
        entity: &str,
        kind: ImageKind,
        deadline: Instant,
        retry_not_found: bool,
    ) -> OriginResult {
        let mut backoff = FIRST_BACKOFF;
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return OriginResult::Error("no time left for the origin".into());
            }
            let (after, why, not_found) = match self
                .attempt(entity, kind, remaining.min(ATTEMPT_TIMEOUT))
                .await
            {
                Attempt::Done(OriginResult::NotFound) if retry_not_found => {
                    (None, "origin has no image yet".to_string(), true)
                }
                Attempt::Done(result) => return result,
                Attempt::Retry { after, why } => (after, why, false),
            };
            let wait = after.unwrap_or(backoff);
            if wait > MAX_RETRY_AFTER || Instant::now() + wait >= deadline {
                return if not_found {
                    OriginResult::NotFound
                } else {
                    OriginResult::Error(why)
                };
            }
            tracing::debug!(entity, kind = ?kind, ?wait, %why, "profile image origin retry");
            tokio::time::sleep(wait).await;
            backoff = (backoff * 2).min(MAX_BACKOFF);
        }
    }

    async fn attempt(&self, entity: &str, kind: ImageKind, timeout: Duration) -> Attempt {
        let url = format!("{}/entities/{}/{}", self.base_url, entity, kind.filename());
        let resp = match self.http.get(&url).timeout(timeout).send().await {
            Ok(r) => r,
            Err(e) if e.is_transient() => {
                return Attempt::Retry {
                    after: None,
                    why: format!("request failed: {e}"),
                };
            }
            Err(e) => return Attempt::Done(OriginResult::Error(format!("request refused: {e}"))),
        };
        let status = resp.status();
        if status == reqwest::StatusCode::NOT_FOUND {
            return Attempt::Done(OriginResult::NotFound);
        }
        if status == reqwest::StatusCode::TOO_MANY_REQUESTS || status.is_server_error() {
            let after = resp
                .headers()
                .get(reqwest::header::RETRY_AFTER)
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.trim().parse::<u64>().ok())
                .map(Duration::from_secs);
            return Attempt::Retry {
                after,
                why: format!("origin status {status}"),
            };
        }
        if !status.is_success() {
            return Attempt::Done(OriginResult::Error(format!("origin status {status}")));
        }
        match read_body_capped(resp, MAX_IMAGE_BYTES).await {
            Ok(b) if !b.is_empty() => Attempt::Done(OriginResult::Hit(Bytes::from(b))),
            Ok(_) => Attempt::Retry {
                after: None,
                why: "origin answered an empty body".into(),
            },
            Err(e) => Attempt::Retry {
                after: None,
                why: format!("read failed: {e}"),
            },
        }
    }
}
