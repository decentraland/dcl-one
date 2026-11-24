//! Native event posters use Metamorph's shared conversion queue. A cold redirect points
//! back to the source; a later redirect points to the published KTX2. Only the latter is
//! memoized. Browser requests and explicit thumbnail sizes use the local converter.

use std::collections::HashMap;
use std::sync::{Arc, Weak};
use std::time::Duration;

use axum::extract::{Query, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use parking_lot::Mutex;
use tokio::sync::OnceCell;
use url::Url;

use super::convert::{self as media, ConvertParams, Converted};
use super::{MediaState, SharedState};

/// Only overlapping requests hold a preview. After they finish, the next caller
/// checks Metamorph again. Admission bounds the number of live cells.
#[derive(Default)]
pub(super) struct Flights(Mutex<HashMap<String, Weak<OnceCell<Converted>>>>);

impl Flights {
    fn for_key(&self, key: &str) -> Arc<OnceCell<Converted>> {
        let mut flights = self.0.lock();
        if let Some(existing) = flights.get(key).and_then(Weak::upgrade) {
            return existing;
        }
        flights.retain(|_, cell| cell.strong_count() > 0);
        let cell = Arc::new(OnceCell::new());
        flights.insert(key.to_owned(), Arc::downgrade(&cell));
        cell
    }
}

pub(super) async fn convert(state: State<SharedState>, query: Query<ConvertParams>) -> Response {
    let sources = &state.convert_sources;
    if query.width.is_none()
        && sources.remote
        && let Some(endpoint) = &sources.poster_converter
        && let Some(node) = &sources.own_url
        && let Some(source) = crate::explore::node_assets::public_event_poster(&query.url, node)
    {
        let key = format!("poster-texture:{source}");
        if let Some(hit) = state.convert_cache.get_fresh(&key) {
            return media::cached_response(hit);
        }
        let Some(_admitted) = state.convert_slots.admit() else {
            return media::busy_response();
        };
        let flight = state.poster_flights.for_key(&key);
        let result = flight.get_or_try_init(|| async {
            let result = fetch(&state, endpoint.clone(), source).await?;
            if result.1 == "image/ktx2" && result.2.len() <= media::CONVERT_CACHE_MAX_BODY {
                state.convert_cache.insert(key, result.clone());
            }
            Ok::<_, Response>(result)
        });
        return match tokio::time::timeout(media::THUMBNAIL_WAIT, result).await {
            Ok(Ok(result)) => {
                let mut response = media::cached_response(result.clone());
                if result.1 != "image/ktx2" {
                    response
                        .headers_mut()
                        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
                }
                response
            }
            Ok(Err(response)) => response,
            Err(_) => media::busy_response(),
        };
    }
    media::convert(state, query).await
}

async fn fetch(state: &MediaState, mut endpoint: Url, source: Url) -> Result<Converted, Response> {
    let _slot = state
        .convert_slots
        .slot(media::THUMBNAIL_WAIT)
        .await
        .ok_or_else(media::busy_response)?;
    if !state.http.allows_private_hosts() && crate::http::resolve_and_pin(&source).await.is_none() {
        return Err((StatusCode::FORBIDDEN, "url host is not publicly routable").into_response());
    }
    endpoint
        .query_pairs_mut()
        .append_pair("url", source.as_str());
    let answer = state
        .http
        .get(endpoint)
        .following_redirects(3)
        .timeout(Duration::from_secs(5))
        .send()
        .await;

    if let Ok(reply) = answer
        && reply.status() == StatusCode::OK
    {
        if reply.url() == &source {
            return preview(reply).await;
        }
        let is_ktx = reply
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.split(';').next())
            .is_some_and(|mime| mime.trim().eq_ignore_ascii_case("image/ktx2"));
        if is_ktx
            && let Ok(bytes) =
                crate::http::read_body_capped(reply, media::MAX_THUMBNAIL_SOURCE_BYTES).await
            && ktx2::Reader::new(&bytes).is_ok_and(|texture| {
                texture.header().pixel_height > 0
                    && texture.basic_dfd().is_some()
                    && texture.levels().all(|level| !level.data.is_empty())
            })
        {
            return Ok((200, "image/ktx2".into(), bytes.into()));
        }
    }
    // A converter outage or unusable artifact must not hide an available poster.
    preview(media::send_remote(state, source).await?).await
}

async fn preview(reply: reqwest::Response) -> Result<Converted, Response> {
    let (status, mime, body) = media::read_remote(reply, media::MAX_THUMBNAIL_SOURCE_BYTES).await?;
    if matches!(mime.as_str(), "image/webp" | "image/gif") {
        let (mime, bytes) = crate::content::entity_thumbnail::render_poster_preview(body)
            .await
            .map_err(|message| (StatusCode::UNPROCESSABLE_ENTITY, message).into_response())?;
        return Ok((status.as_u16(), mime.into(), bytes));
    }
    Ok((status.as_u16(), mime, body))
}
