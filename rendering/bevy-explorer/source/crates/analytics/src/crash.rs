//! A crash report the host can post once the engine no longer can: the engine keeps what an
//! Explorer Error batch needs in a static snapshot, and [`crash_batch`] builds the request from
//! it. On the web the page thread holds a wasm instance on the shared memory, so `src/web.rs`
//! exposes it to boot.js's crash watchdog there. The trapped engine worker may hold any lock, so
//! the snapshot is only ever `try_lock`ed on that path.

use std::sync::Mutex;

use bevy::prelude::*;
use chrono::Utc;
use common::structs::AppConfig;

use crate::{
    data_definition::{
        build_segment_event_batch_item, SegmentEvent, SegmentEventCommonExplorerFields,
        SegmentEventExplorerError,
    },
    segment_system::{batch_body, SegmentConfig, SEGMENT_BATCH_URL},
};

struct Snapshot {
    user_id: String,
    write_key: String,
    /// the terms are accepted: nothing goes out otherwise, a crash included
    consent: bool,
    common: SegmentEventCommonExplorerFields,
}

static SNAPSHOT: Mutex<Option<Snapshot>> = Mutex::new(None);

/// The page's `Feature Flags Resolved` event: the session's rollout flags, which a crash report
/// carries so the session can be run again with them.
pub(crate) const FEATURE_FLAGS_EVENT: &str = "Feature Flags Resolved";
const FEATURE_FLAGS_FIELDS: [&str; 2] = ["feature_flags", "feature_flags_override"];

/// The session's flags as the page last reported them, `(field, value)`.
static FEATURE_FLAGS: Mutex<Vec<(&'static str, String)>> = Mutex::new(Vec::new());

/// Keep the flags of a `Feature Flags Resolved` event for the crash report, when the event is
/// this session's: the page's queue can still hold one from an earlier load.
pub(crate) fn note_feature_flags(
    properties: &serde_json::Map<String, serde_json::Value>,
    session_id: &str,
) {
    if properties.get("session_id").and_then(|v| v.as_str()) != Some(session_id) {
        return;
    }
    let flags = FEATURE_FLAGS_FIELDS
        .into_iter()
        .filter_map(|field| Some((field, properties.get(field)?.as_str()?.to_owned())))
        .collect();
    if let Ok(mut kept) = FEATURE_FLAGS.lock() {
        *kept = flags;
    }
}

pub fn refresh_crash_snapshot(config: Res<SegmentConfig>, app_config: Res<AppConfig>) {
    if !config.is_changed() && !app_config.is_changed() {
        return;
    }
    if let Ok(mut snapshot) = SNAPSHOT.lock() {
        *snapshot = Some(Snapshot {
            user_id: config.user_id().to_owned(),
            write_key: config.write_key().to_owned(),
            consent: app_config.terms_accepted(),
            common: config.common().clone(),
        });
    }
}

/// The Segment request for an Explorer Error, `(url, body)`, with the common fields as the engine
/// last knew them. None before the terms are accepted, or while the engine holds the snapshot.
pub fn crash_batch(message: &str, source: &str) -> Option<(String, String)> {
    let guard = SNAPSHOT.try_lock().ok()?;
    let snapshot = guard.as_ref().filter(|s| s.consent)?;
    let item = build_segment_event_batch_item(
        snapshot.user_id.clone(),
        &snapshot.common,
        &SegmentEvent::ExplorerError(SegmentEventExplorerError {
            error_type: "Crash".to_owned(),
            error_message: format!("[{source}] {message}"),
            error_stack: String::new(),
        }),
        Utc::now(),
        uuid::Uuid::new_v4().to_string(),
    );
    let mut item = serde_json::to_value(&item).ok()?;
    // Left out rather than waited for: the trapped engine may hold this lock too.
    if let (Ok(flags), Some(properties)) = (
        FEATURE_FLAGS.try_lock(),
        item.get_mut("properties")
            .and_then(serde_json::Value::as_object_mut),
    ) {
        for (field, value) in flags.iter() {
            properties.insert((*field).to_owned(), value.clone().into());
        }
    }
    let item = item.to_string();
    Some((
        SEGMENT_BATCH_URL.to_owned(),
        batch_body(&snapshot.write_key, &[item]),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    // One static for the whole test binary: this is the only test that touches it.
    #[test]
    fn crash_batch_needs_consent_and_carries_the_snapshot() {
        let common = SegmentEventCommonExplorerFields::new("s".into(), "1.0".into(), None);
        let set = |consent| {
            *SNAPSHOT.lock().unwrap() = Some(Snapshot {
                user_id: "u".into(),
                write_key: "k".into(),
                consent,
                common: common.clone(),
            })
        };
        assert!(crash_batch("panicked at x", "panic").is_none());
        set(false);
        assert!(crash_batch("panicked at x", "panic").is_none());
        set(true);
        let (url, body) = crash_batch("panicked at x", "panic").unwrap();
        assert_eq!(url, SEGMENT_BATCH_URL);
        let body: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(body["writeKey"], "k");
        let item = &body["batch"][0];
        assert_eq!(item["event"], "Explorer Error");
        assert_eq!(item["userId"], "u");
        assert_eq!(item["properties"]["session_id"], "s");
        assert_eq!(item["properties"]["error_type"], "Crash");
        assert_eq!(item["properties"]["error_message"], "[panic] panicked at x");
        assert!(item["timestamp"].is_string());
        assert!(item["properties"].get("feature_flags").is_none());

        // The page's names (library/explorer-core/lib/featureFlags.ts, FLAGS_RESOLVED_EVENT and
        // its `settle`): a rename here needs the same one there.
        assert_eq!(FEATURE_FLAGS_EVENT, "Feature Flags Resolved");
        assert_eq!(
            FEATURE_FLAGS_FIELDS,
            ["feature_flags", "feature_flags_override"]
        );
        let flags = |session: &str, flags: &str| {
            serde_json::json!({
                "session_id": session,
                "feature_flags": flags,
                "feature_flags_override": "",
                "feature_flags_source": "service",
            })
        };
        note_feature_flags(flags("s", "-*,test").as_object().unwrap(), "s");
        // an earlier load's event, still in the page's queue, and one that names no session
        note_feature_flags(flags("earlier", "-*,other").as_object().unwrap(), "s");
        let mut unnamed = flags("s", "-*,unnamed");
        unnamed.as_object_mut().unwrap().remove("session_id");
        note_feature_flags(unnamed.as_object().unwrap(), "s");
        let (_, body) = crash_batch("panicked at x", "panic").unwrap();
        let body: serde_json::Value = serde_json::from_str(&body).unwrap();
        let properties = &body["batch"][0]["properties"];
        assert_eq!(properties["feature_flags"], "-*,test");
        assert_eq!(properties["feature_flags_override"], "");
        assert!(properties.get("feature_flags_source").is_none());
        assert_eq!(properties["session_id"], "s");
    }
}
