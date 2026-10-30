use axum::Json;
use axum::extract::State;
use chrono::{NaiveDateTime, SecondsFormat, Utc};
use serde::Serialize;

use super::SharedState;
use super::queries::{self, CatalogueCounts, MirrorPassRow};
use crate::content::errors::AppResult;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "worlds/"))]
pub struct WorldsCount {
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub ens: i64,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub dcl: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "worlds/"))]
pub struct ContentStatus {
    pub commit_hash: String,
    pub worlds_count: WorldsCount,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "worlds/"))]
pub struct CommsStatus {
    pub adapter_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub status_url: Option<String>,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub rooms: i64,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub users: i64,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub timestamp: i64,
}

/// One mirror's freshness: when it last completed a pass, where it is in
/// the current one, and what the last pass did. `ageSeconds` is the time
/// since the last complete pass, `null` before the first one.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "worlds/"))]
pub struct MirrorFreshness {
    pub source: String,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub pass_number: i64,
    pub last_completed_at: Option<String>,
    #[cfg_attr(feature = "ts", ts(type = "number | null"))]
    pub age_seconds: Option<i64>,
    pub last_attempt_at: Option<String>,
    pub in_progress: bool,
    pub cursor: Option<String>,
    #[cfg_attr(feature = "ts", ts(type = "Record<string, unknown>"))]
    pub stats: serde_json::Value,
    pub last_error: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "worlds/"))]
pub struct MirrorStatus {
    pub enabled: bool,
    pub source: Option<String>,
    #[cfg_attr(feature = "ts", ts(type = "number | null"))]
    pub interval_seconds: Option<u64>,
    pub catalogue: Option<MirrorFreshness>,
    pub scenes: Option<MirrorFreshness>,
    pub worlds: CatalogueCounts,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "worlds/"))]
pub struct PersonalWorldsStatus {
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub max_worlds: u64,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub max_size_bytes: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "worlds/"))]
pub struct StatusResponse {
    pub content: ContentStatus,
    pub comms: CommsStatus,
    pub personal_worlds: Option<PersonalWorldsStatus>,
    pub mirror: MirrorStatus,
}

fn rfc3339(ts: Option<NaiveDateTime>) -> Option<String> {
    ts.map(|t| t.and_utc().to_rfc3339_opts(SecondsFormat::Millis, true))
}

fn freshness(row: &MirrorPassRow, now: NaiveDateTime) -> MirrorFreshness {
    MirrorFreshness {
        source: row.source.clone(),
        pass_number: row.pass_number,
        last_completed_at: rfc3339(row.pass_completed_at),
        age_seconds: row.pass_completed_at.map(|t| (now - t).num_seconds()),
        last_attempt_at: rfc3339(row.last_attempt_at),
        in_progress: row.cursor.is_some(),
        cursor: row.cursor.clone(),
        stats: row.stats.clone(),
        last_error: row.last_error.clone(),
    }
}

/// GET /worlds/status: the old `content.worldsCount` block, the comms block
/// (upstream's LiveKit adapter status: the world rooms with players and their
/// sum, from this node's presence registry instead of a LiveKit room listing;
/// `offline` when no signaling host is configured), the personal-worlds
/// policy (`null` when off) and the mirror freshness.
fn comms_status(livekit_client_url: &str, world_counts: &[(String, i64)]) -> CommsStatus {
    let timestamp = Utc::now().timestamp_millis();
    if livekit_client_url.is_empty() {
        return CommsStatus {
            adapter_type: "offline".to_string(),
            status_url: None,
            rooms: 0,
            users: 0,
            timestamp,
        };
    }
    let occupied: Vec<i64> = world_counts
        .iter()
        .map(|(_, n)| *n)
        .filter(|n| *n > 0)
        .collect();
    CommsStatus {
        adapter_type: "livekit".to_string(),
        status_url: Some(crate::shared::livekit::status_url(livekit_client_url)),
        rooms: occupied.len() as i64,
        users: occupied.iter().sum(),
        timestamp,
    }
}

pub async fn get_status(State(state): State<SharedState>) -> AppResult<Json<StatusResponse>> {
    let worlds_count = queries::deployed_world_count(&state.pool).await?;
    let passes = queries::mirror_passes(&state.pool).await?;
    let counts = queries::catalogue_counts(&state.pool).await?;
    let now = Utc::now().naive_utc();
    let settings = &state.settings;
    let personal_worlds = (settings.personal_worlds_max > 0).then_some(PersonalWorldsStatus {
        max_worlds: settings.personal_worlds_max,
        max_size_bytes: settings.personal_world_max_size_bytes as i64,
    });
    let find = |name: &str| {
        passes
            .iter()
            .find(|p| p.mirror == name)
            .map(|p| freshness(p, now))
    };
    Ok(Json(StatusResponse {
        content: ContentStatus {
            commit_hash: state
                .settings
                .commit_hash
                .clone()
                .unwrap_or_else(|| "unknown".to_string()),
            worlds_count: WorldsCount {
                ens: worlds_count.ens,
                dcl: worlds_count.dcl,
            },
        },
        comms: comms_status(&settings.livekit_client_url, &state.presence.world_counts()),
        personal_worlds,
        mirror: MirrorStatus {
            enabled: state.mirror.as_ref().is_some_and(|m| m.scheduled),
            source: state.mirror.as_ref().map(|m| m.source.clone()),
            interval_seconds: state
                .mirror
                .as_ref()
                .filter(|m| m.scheduled)
                .map(|m| m.interval.as_secs()),
            catalogue: find("catalogue"),
            scenes: find("scenes"),
            worlds: counts,
        },
    }))
}

#[cfg(test)]
mod tests {
    use super::comms_status;

    #[test]
    fn the_comms_block_reports_the_signaling_host_and_the_occupied_world_rooms() {
        let counts = vec![
            ("alpha.dcl.eth".to_string(), 3),
            ("beta.dcl.eth".to_string(), 1),
        ];
        let c = comms_status("wss://dcl.livekit.cloud", &counts);
        assert_eq!(c.adapter_type, "livekit");
        assert_eq!(c.status_url.as_deref(), Some("https://dcl.livekit.cloud/"));
        assert_eq!((c.rooms, c.users), (2, 4));
        let off = comms_status("", &counts);
        assert_eq!(off.adapter_type, "offline");
        assert!(off.status_url.is_none());
        assert_eq!((off.rooms, off.users), (0, 0));
        assert!(!serde_json::to_string(&off).unwrap().contains("statusUrl"));
    }
}
