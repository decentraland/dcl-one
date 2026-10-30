//! The events `/v1` reads: `GET /v1/events` (the `/api/events` list under
//! its v1 name), `GET /v1/events/{id}` (a UUID; the event with its
//! destination, the place or world the places `/v1` answers for it) and
//! `GET /v1/destinations/{id}/events` (a destination's active events, live
//! ones first, then the soonest). The old service asked the places service
//! over HTTP for the destination; here it is `places::v1::find_destination_for`.

use axum::Json;
use axum::extract::{OriginalUri, Path, State};
use axum::http::HeaderMap;
use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::Value;

use super::{EventListFilters, EventListType, EventRecord, SortOrder};
use crate::SharedState;
use crate::shared::errors::{ApiData, ApiError};

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "events/"))]
pub struct V1EventRecord {
    #[serde(flatten)]
    #[cfg_attr(feature = "ts", ts(flatten))]
    pub event: EventRecord,
    pub destination_id: Option<String>,
    #[cfg_attr(feature = "ts", ts(type = "Record<string, unknown> | null"))]
    pub destination: Option<Value>,
}

pub fn is_uuid(id: &str) -> bool {
    let bytes = id.as_bytes();
    bytes.len() == 36
        && bytes.iter().enumerate().all(|(i, c)| match i {
            8 | 13 | 18 | 23 => *c == b'-',
            _ => c.is_ascii_hexdigit(),
        })
}

/// The event's destination: its place, or the world a world event names by `server`.
pub fn destination_id(event: &EventRecord) -> Option<String> {
    event
        .place_id
        .clone()
        .filter(|id| !id.is_empty())
        .or_else(|| {
            event
                .world
                .then(|| event.server.clone())
                .flatten()
                .filter(|s| !s.is_empty())
        })
}

fn is_live_at(event: &EventRecord, now: DateTime<Utc>) -> bool {
    event.next_start_at.is_some_and(|start| start <= now)
        && event.next_finish_at.is_some_and(|finish| finish >= now)
}

fn start_key(event: &EventRecord) -> i64 {
    event
        .next_start_at
        .or(event.start_at)
        .map(|t| t.timestamp_millis())
        .unwrap_or(i64::MAX)
}

/// Live first, then the soonest start; ties keep the query's order.
pub fn order_destination_events(events: &mut [EventRecord], now: DateTime<Utc>) {
    events.sort_by(|a, b| {
        is_live_at(b, now)
            .cmp(&is_live_at(a, now))
            .then_with(|| start_key(a).cmp(&start_key(b)))
    });
}

pub async fn get_v1_event(
    State(state): State<SharedState>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    Path(event_id): Path<String>,
) -> Result<Json<ApiData<V1EventRecord>>, ApiError> {
    let not_found = || ApiError::not_found(format!("Event not found: {event_id}"));
    if !is_uuid(&event_id) {
        return Err(not_found());
    }
    let viewer = state.viewer(&headers, "get", uri.path()).await;
    let admin = super::handlers::viewer_is_admin(&state, &headers, viewer.as_deref()).await?;
    let event = state
        .events
        .get_for_viewer(&event_id, viewer.as_deref())
        .await?
        .filter(|evt| super::handlers::visible_to_viewer(evt, viewer.as_deref(), admin))
        .ok_or_else(not_found)?;
    let destination_id = destination_id(&event);
    let destination = match &destination_id {
        Some(id) => crate::places::v1::find_destination_for(&state, id, viewer.as_deref())
            .await?
            .map(crate::places::destinations::Destination::from)
            .and_then(|d| serde_json::to_value(d).ok()),
        None => None,
    };
    Ok(Json(ApiData::ok(V1EventRecord {
        event,
        destination_id,
        destination,
    })))
}

pub async fn get_v1_destination_events(
    State(state): State<SharedState>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<ApiData<Vec<EventRecord>>>, ApiError> {
    let viewer = state.viewer(&headers, "get", uri.path()).await;
    let filters = EventListFilters {
        limit: 500,
        offset: 0,
        list: EventListType::Active,
        order: SortOrder::Asc,
        destination_ids: vec![id.to_lowercase()],
        user: viewer,
        include_own: true,
        ..Default::default()
    };
    let (mut events, _) = state.events.query(&filters, false).await?;
    order_destination_events(&mut events, Utc::now());
    Ok(Json(ApiData::ok(events)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn at(hour: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2030, 1, 1, hour, 0, 0).unwrap()
    }

    fn event(id: &str, start: Option<u32>, finish: Option<u32>) -> EventRecord {
        EventRecord {
            id: id.to_string(),
            name: id.to_string(),
            image: None,
            image_vertical: None,
            description: None,
            start_at: start.map(at),
            finish_at: finish.map(at),
            next_start_at: start.map(at),
            next_finish_at: finish.map(at),
            duration: Some(0),
            all_day: false,
            x: 0,
            y: 0,
            server: None,
            url: None,
            user: None,
            user_name: None,
            estate_id: None,
            estate_name: None,
            scene_name: None,
            approved: true,
            rejected: false,
            highlighted: false,
            trending: false,
            world: false,
            recurrent: false,
            recurrent_frequency: None,
            recurrent_weekday_mask: 0,
            recurrent_month_mask: 0,
            recurrent_interval: 1,
            recurrent_setpos: None,
            recurrent_monthday: None,
            recurrent_count: None,
            recurrent_until: None,
            recurrent_dates: vec![],
            categories: vec![],
            schedules: vec![],
            total_attendees: 0,
            latest_attendees: vec![],
            coordinates: [0, 0],
            position: [0, 0],
            live: false,
            attending: false,
            place_id: None,
            community_id: None,
            featured_item: None,
            created_at: None,
            updated_at: None,
            approved_by: None,
            rejected_by: None,
            rejection_reason: None,
            deleted_by_user: false,
            deleted_by_admin: false,
            deleted_by: None,
            deleted_at: None,
            deleted_reason: None,
            previous_place_id: None,
            connected_addresses: None,
        }
    }

    fn ids(events: &[EventRecord]) -> Vec<&str> {
        events.iter().map(|e| e.id.as_str()).collect()
    }

    #[test]
    fn live_events_come_first_then_the_soonest_upcoming() {
        let mut events = vec![
            event("later", Some(18), Some(19)),
            event("soon", Some(14), Some(15)),
            event("live-late", Some(11), Some(13)),
            event("live-early", Some(9), Some(13)),
        ];
        order_destination_events(&mut events, at(12));
        assert_eq!(ids(&events), ["live-early", "live-late", "soon", "later"]);
    }

    #[test]
    fn an_occurrence_ending_exactly_now_is_still_live() {
        let mut events = vec![
            event("next", Some(13), Some(14)),
            event("edge", Some(10), Some(12)),
        ];
        order_destination_events(&mut events, at(12));
        assert_eq!(ids(&events), ["edge", "next"]);
    }

    #[test]
    fn ties_keep_the_repository_order_and_undated_rows_sort_last() {
        let mut events = vec![
            event("b", Some(14), Some(15)),
            event("a", Some(14), Some(15)),
            event("undated", None, None),
        ];
        order_destination_events(&mut events, at(12));
        assert_eq!(ids(&events), ["b", "a", "undated"]);
    }

    #[test]
    fn the_destination_id_is_the_place_or_the_named_world() {
        let mut e = event("e", None, None);
        assert_eq!(destination_id(&e), None);
        e.place_id = Some("".into());
        assert_eq!(destination_id(&e), None);
        e.place_id = Some("my-world.dcl.eth".into());
        assert_eq!(destination_id(&e).as_deref(), Some("my-world.dcl.eth"));
        let mut w = event("w", None, None);
        w.world = true;
        w.server = Some("my-world.dcl.eth".into());
        assert_eq!(destination_id(&w).as_deref(), Some("my-world.dcl.eth"));
        w.world = false;
        assert_eq!(destination_id(&w), None);
        assert!(
            is_uuid("123e4567-e89b-12d3-a456-426614174000")
                && !is_uuid("123e4567e89b12d3a456426614174000")
        );
    }

    #[test]
    fn the_v1_record_flattens_the_event_and_adds_the_destination() {
        let mut e = event("e", None, None);
        e.place_id = Some("p".into());
        let v = serde_json::to_value(V1EventRecord {
            destination_id: destination_id(&e),
            destination: Some(serde_json::json!({ "id": "p" })),
            event: e,
        })
        .unwrap();
        assert_eq!(
            (
                v["id"].clone(),
                v["destination_id"].clone(),
                v["destination"]["id"].clone()
            ),
            (
                serde_json::json!("e"),
                serde_json::json!("p"),
                serde_json::json!("p")
            )
        );
    }
}
