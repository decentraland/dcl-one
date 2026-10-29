//! The places `/v1` reads: `GET /v1/destinations` (the `/api/destinations`
//! feed under its v1 name), `GET /v1/destinations/{id}` (one place by UUID or
//! one world by name, enabled and listed, with the feed's decorations) and
//! `GET /v1/categories` (`?target=events` answers the events categories).
//! The v1 writes are in `places::interactions`; `/v1/destinations/{id}/events`
//! is the events domain's (`events::v1`).

use axum::Json;
use axum::extract::{OriginalUri, Path, Query, State};
use axum::http::{HeaderMap, Method};
use serde::Serialize;
use serde_json::Value;

use super::destinations::{
    Destination, decorate_next_event, enrich, list_destinations, parse_with_options,
};
use super::handlers::{category_i18n_en, curated_counts};
use super::interactions::is_place_id;
use super::{CategoryTarget, PlaceRow};
use crate::SharedState;
use crate::shared::errors::{ApiData, ApiDataTotal, ApiError};

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "places/"))]
pub struct DestinationCategory {
    pub name: String,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub count: i64,
    pub i18n: CategoryI18n,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "places/"))]
pub struct CategoryI18n {
    pub en: String,
}

pub fn destination_category(name: String, count: i64) -> DestinationCategory {
    let en = category_i18n_en(&name)
        .map(str::to_string)
        .unwrap_or_else(|| name.clone());
    DestinationCategory {
        name,
        count,
        i18n: CategoryI18n { en },
    }
}

pub async fn get_v1_destinations_list(
    State(state): State<SharedState>,
    method: Method,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    Query(pairs): Query<Vec<(String, String)>>,
) -> Result<Json<ApiDataTotal<Destination>>, ApiError> {
    list_destinations(&state, &headers, method.as_str(), uri.path(), &pairs).await
}

/// A place by its UUID (enabled, not a world) or a world by its name
/// (enabled, a world, listed in places).
pub async fn find_destination_for(
    state: &SharedState,
    id: &str,
    viewer: Option<&str>,
) -> Result<Option<PlaceRow>, ApiError> {
    if let Some(scene) = state.places.find_world_scene_place(id, viewer).await? {
        return Ok(scene);
    }
    if is_place_id(id) {
        Ok(state
            .places
            .find_by_id_for(&id.to_lowercase(), viewer)
            .await?
            .filter(|p| !p.disabled && !p.world))
    } else {
        Ok(state
            .places
            .find_world_by_id_for(id, viewer)
            .await?
            .filter(|w| !w.disabled && w.world && w.show_in_places))
    }
}

pub async fn get_v1_destination(
    State(state): State<SharedState>,
    method: Method,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    Query(pairs): Query<Vec<(String, String)>>,
    Path(id): Path<String>,
) -> Result<Json<ApiData<Destination>>, ApiError> {
    let flags = parse_with_options(&pairs)?;
    let user = state.viewer(&headers, method.as_str(), uri.path()).await;
    let Some(row) = find_destination_for(&state, &id, user.as_deref()).await? else {
        return Err(ApiError::not_found(format!("Destination not found: {id}")));
    };
    let mut rows = vec![row];
    enrich(&state, &mut rows, &flags).await;
    let mut out: Vec<Destination> = rows.into_iter().map(Destination::from).collect();
    decorate_next_event(&state, &mut out, &flags).await;
    let destination = out
        .pop()
        .ok_or_else(|| ApiError::not_found(format!("Destination not found: {id}")))?;
    Ok(Json(ApiData::ok(destination)))
}

pub async fn get_v1_categories(
    State(state): State<SharedState>,
    Query(pairs): Query<Vec<(String, String)>>,
) -> Result<Json<ApiData<Vec<Value>>>, ApiError> {
    let target = pairs
        .iter()
        .find(|(k, _)| k == "target")
        .map(|(_, v)| v.as_str())
        .unwrap_or("destinations");
    if target == "events" {
        let data = crate::events::categories::list()
            .into_iter()
            .map(|c| serde_json::to_value(c).unwrap_or(Value::Null))
            .collect();
        return Ok(Json(ApiData::ok(data)));
    }
    let counts = curated_counts(state.places.category_counts(CategoryTarget::All).await?);
    let data = counts
        .into_iter()
        .map(|(name, count)| {
            serde_json::to_value(destination_category(name, count)).unwrap_or(Value::Null)
        })
        .collect();
    Ok(Json(ApiData::ok(data)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    use std::sync::Arc;

    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    use crate::{ExploreState, Settings, router};

    async fn get(app: &axum::Router, uri: &str) -> (StatusCode, serde_json::Value) {
        let resp = app
            .clone()
            .oneshot(Request::get(uri).body(Body::empty()).unwrap())
            .await
            .unwrap();
        let status = resp.status();
        let bytes = axum::body::to_bytes(resp.into_body(), 1 << 20)
            .await
            .unwrap();
        (
            status,
            serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null),
        )
    }

    /// A destination by place UUID or world name; a disabled place and a
    /// world that opted out of the directory resolve to 404; the categories
    /// answer both targets; the v1 list is the destinations feed.
    #[tokio::test]
    async fn a_destination_resolves_by_place_uuid_or_world_name() {
        let Some(pool) = crate::db::scratch::pool(4).await else {
            return;
        };
        let tag = &uuid::Uuid::new_v4().simple().to_string()[..10];
        let place = uuid::Uuid::new_v4().to_string();
        let disabled = uuid::Uuid::new_v4().to_string();
        let (listed, hidden) = (format!("v1l{tag}.dcl.eth"), format!("v1h{tag}.dcl.eth"));
        for (id, disabled_row, raw) in [
            (place.clone(), false, json!({ "positions": ["7,7"] })),
            (disabled.clone(), true, json!({ "positions": ["8,8"] })),
            (
                uuid::Uuid::new_v4().to_string(),
                false,
                json!({ "world": true, "world_name": listed }),
            ),
            (
                uuid::Uuid::new_v4().to_string(),
                false,
                json!({ "world": true, "world_name": hidden, "show_in_places": false }),
            ),
        ] {
            sqlx::query("INSERT INTO explore.place (id, base_position, title, disabled, raw) VALUES ($1, '0,0', 't', $2, $3)")
                .bind(id)
                .bind(disabled_row)
                .bind(raw)
                .execute(&pool)
                .await
                .unwrap();
        }
        let app = router(Arc::new(ExploreState::new(
            pool.clone(),
            Settings::for_tests(),
            None,
        )));
        let (status, body) = get(&app, &format!("/v1/destinations/{}", place.to_uppercase())).await;
        assert_eq!(
            (status, body["data"]["id"].clone()),
            (StatusCode::OK, json!(place))
        );
        let (status, body) = get(&app, &format!("/v1/destinations/{listed}")).await;
        assert_eq!(
            (status, body["data"]["world_name"].clone()),
            (StatusCode::OK, json!(listed))
        );
        for gone in [disabled.as_str(), hidden.as_str()] {
            let (status, body) = get(&app, &format!("/v1/destinations/{gone}")).await;
            assert_eq!(status, StatusCode::NOT_FOUND, "{gone}: {body}");
            assert_eq!(
                body["message"],
                json!(format!("Destination not found: {gone}"))
            );
        }
        let (status, body) = get(&app, "/v1/categories?target=events").await;
        assert_eq!(status, StatusCode::OK);
        assert!(
            body["data"].as_array().is_some_and(|a| !a.is_empty()),
            "{body}"
        );
        let (status, body) = get(&app, "/v1/categories").await;
        assert_eq!((status, body["ok"].clone()), (StatusCode::OK, json!(true)));
        let (status, body) = get(
            &app,
            &format!("/v1/destinations?world_names={listed}&world_names={hidden}"),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        let names: Vec<&str> = body["data"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|d| d["world_name"].as_str())
            .collect();
        assert!(
            !names.contains(&hidden.as_str()),
            "an opted-out world is not listed: {names:?}"
        );
    }

    #[test]
    fn destination_categories_fall_back_to_the_name_for_the_label() {
        let known = destination_category("art".to_string(), 4);
        assert_eq!(known.i18n.en, "\u{1F3A8} Art");
        let unknown = destination_category("zzz".to_string(), 0);
        assert_eq!(unknown.i18n.en, "zzz");
        assert_eq!(
            serde_json::to_value(unknown).unwrap(),
            json!({ "name": "zzz", "count": 0, "i18n": { "en": "zzz" } })
        );
    }
}
