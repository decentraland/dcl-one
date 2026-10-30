use chrono::{DateTime, SecondsFormat, Utc};
use serde::Serialize;

/// The os natively; wasm32-unknown-unknown has no os name (`std::env::consts::OS` is empty).
#[cfg(not(target_arch = "wasm32"))]
const PLATFORM: &str = std::env::consts::OS;
#[cfg(target_arch = "wasm32")]
const PLATFORM: &str = "web";

/// Mobile's wallet value before a sign-in, and after a sign-out.
const UNAUTHENTICATED: &str = "unauthenticated";

#[derive(Serialize)]
pub struct SegmentMetricEventBody {
    #[serde(rename = "type")]
    r#type: String,
    event: String,
    #[serde(rename = "userId")]
    user_id: String,
    // When it happened: the page's time for a page event (which can predate the engine), the
    // queue time for the engine's own, as a batch can leave long after behind the consent gate.
    timestamp: String,
    // Minted when the event is queued (by the page for its own), so Segment drops the duplicates
    // a resent batch makes.
    #[serde(rename = "messageId")]
    message_id: String,
    properties: serde_json::Value,
}

#[derive(Serialize, Clone)]
// Same for all events sent from the explorer
pub struct SegmentEventCommonExplorerFields {
    // User’s wallet id, even for guests.
    pub dcl_eth_address: String,
    // If the user is a guest or not.
    pub dcl_is_guest: bool,
    // Realm where the user is connected.
    pub realm: String,
    // Current user position.
    pub position: String,
    // What type of client was used to render the world (Web/Native/VR)
    pub dcl_renderer_type: String,
    // Explorer’s unique session id.
    pub session_id: String,
    // Explorer’s release used.
    pub renderer_version: String,
    // `prod` or `dev` (`segment_system::environment`); the data stack filters on it.
    pub environment: String,
    // The landing page's Segment anonymous id, handed over by the host so the warehouse joins
    // this session to the web visit that led to it (as the launcher does for the desktop client).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub campaign_anon_user_id: Option<String>,
    // A scene being developed (the sdk's or an editor's preview, a local dev server's realm), as
    // the desktop client flags it: not a visit, the warehouse filters on it.
    pub is_local_scene: bool,
}

impl SegmentEventCommonExplorerFields {
    pub fn new(session_id: String, version: String, campaign_anon_user_id: Option<String>) -> Self {
        let dcl_renderer_type = format!("dao-bevy-{PLATFORM}");

        Self {
            // mobile's values until the engine knows better
            dcl_eth_address: UNAUTHENTICATED.into(),
            dcl_is_guest: false,
            realm: "no-realm".into(),
            position: "no-position".into(),
            dcl_renderer_type,
            session_id,
            renderer_version: version,
            environment: crate::segment_system::environment().to_owned(),
            campaign_anon_user_id,
            is_local_scene: false,
        }
    }

    pub fn reset_identity(&mut self) {
        self.dcl_eth_address = UNAUTHENTICATED.into();
        self.dcl_is_guest = false;
    }
}

pub enum SegmentEvent {
    PerformanceMetrics(SegmentEventPerformanceMetrics),
    ExplorerError(SegmentEventExplorerError),
    ExplorerSceneLoadTimes(SegmentEventExplorerSceneLoadTimes),
    ExplorerMoveToParcel(String, SegmentEventExplorerMoveToParcel),
    SystemInfoReport(SegmentEventSystemInfoReport),
    ChatMessageSent(SegmentEventChatMessageSent),
    FirstMoveInWorld(SegmentEventFirstMoveInWorld),
    TimeSpentInWorld(SegmentEventTimeSpentInWorld),
    ScreenViewed(SegmentEventScreenViewed),
    /// An event the host page reports through `/track`: its name, properties and time as sent.
    Page {
        name: String,
        properties: serde_json::Map<String, serde_json::Value>,
        timestamp: String,
    },
}

impl SegmentEvent {
    /// The engine's own events by their Segment name and properties, without the common fields;
    /// `None` for a page event, which arrives named.
    pub fn name_and_properties(&self) -> Option<(String, serde_json::Value)> {
        let (name, properties) = match self {
            SegmentEvent::PerformanceMetrics(event) => {
                ("Performance Metrics", serde_json::to_value(event))
            }
            SegmentEvent::ExplorerError(event) => ("Explorer Error", serde_json::to_value(event)),
            SegmentEvent::ExplorerSceneLoadTimes(event) => {
                ("Explorer Scene Load Times", serde_json::to_value(event))
            }
            // the parcel moved to is the batch item's `position`; the stream gets it the same way
            SegmentEvent::ExplorerMoveToParcel(current_position, event) => (
                "Explorer Move To Parcel",
                serde_json::to_value(event).map(|mut properties| {
                    properties["position"] = current_position.clone().into();
                    properties
                }),
            ),
            SegmentEvent::SystemInfoReport(event) => {
                ("System Info Report", serde_json::to_value(event))
            }
            SegmentEvent::ChatMessageSent(event) => {
                ("Chat Message Sent", serde_json::to_value(event))
            }
            SegmentEvent::FirstMoveInWorld(event) => {
                ("First Move In World", serde_json::to_value(event))
            }
            SegmentEvent::TimeSpentInWorld(event) => {
                ("time_spent_in_world", serde_json::to_value(event))
            }
            SegmentEvent::ScreenViewed(event) => ("Screen Viewed", serde_json::to_value(event)),
            SegmentEvent::Page { .. } => return None,
        };
        Some((name.to_string(), properties.unwrap()))
    }
}

#[derive(Serialize)]
pub struct SegmentEventPerformanceMetrics {
    // Total number of frames measured for this event.
    pub samples: u32,
    // Total length of the performance report.
    pub total_time: f32,
    // Amount of hiccups in 1000 frames.
    pub hiccups_in_thousand_frames: u32,
    // Total time length of hiccups measured in seconds.
    pub hiccups_time: f32,
    // Minimum delta (difference) between frames in milliseconds
    pub min_frame_time: f32,
    // Maximum delta (difference) between frames in milliseconds
    pub max_frame_time: f32,
    // Average delta (difference) between frames in milliseconds
    pub mean_frame_time: f32,
    // Median delta (difference) between frames in milliseconds
    pub median_frame_time: f32,
    // Percentile 1 of the delta (difference) between frames in milliseconds
    pub p1_frame_time: f32,
    // Percentile 5 of the delta (difference) between frames in milliseconds
    pub p5_frame_time: f32,
    // Percentile 10 of the delta (difference) between frames in milliseconds
    pub p10_frame_time: f32,
    // Percentile 20 of the delta (difference) between frames in milliseconds
    pub p20_frame_time: f32,
    // Percentile 50 of the delta (difference) between frames in milliseconds
    pub p50_frame_time: f32,
    // Percentile 75 of the delta (difference) between frames in milliseconds
    pub p75_frame_time: f32,
    // Percentile 80 of the delta (difference) between frames in milliseconds
    pub p80_frame_time: f32,
    // Percentile 90 of the delta (difference) between frames in milliseconds
    pub p90_frame_time: f32,
    // Percentile 95 of the delta (difference) between frames in milliseconds
    pub p95_frame_time: f32,
    // Percentile 99 of the delta (difference) between frames in milliseconds
    pub p99_frame_time: f32,
    // How many users where nearby the current user
    pub player_count: i32,
    // Javascript heap memory used by the scenes in kilo bytes
    pub used_jsheap_size: i32,
    // Memory used only by the explorer in kilo bytes
    pub memory_usage: i32,
}

#[derive(Serialize)]
pub struct SegmentEventExplorerError {
    // Generic or Fatal.
    pub error_type: String,
    // Error description.
    pub error_message: String,
    // Error’s stack
    pub error_stack: String,
}

#[derive(Serialize)]
pub struct SegmentEventExplorerSceneLoadTimes {
    // Unique hash for the scene.
    scene_hash: String,
    // Time to load in seconds.
    elapsed: f32,
    // Boolean flag indicating wether the scene loaded without errors.
    success: bool,
}

// TODO: maybe important what realm?
#[derive(Serialize)]
pub struct SegmentEventExplorerMoveToParcel {
    // Parcel where the user is coming from.
    pub old_parcel: String,
}

#[derive(Serialize)]
pub struct SegmentEventSystemInfoReport {
    // Processor used by the user.
    processor_type: String,
    // How many processors are available in user’s device.
    processor_count: u32,
    // Graphic Device used by the user.
    graphics_device_name: String,
    // Graphic device memory in mb.
    graphics_memory_mb: u32,
    // RAM memory in mb.
    system_memory_size_mb: u32,
}

// Same shape as godot's event
#[derive(Serialize)]
pub struct SegmentEventChatMessageSent {
    // Length of the message sent.
    pub length: u32,
    // Whether it is Public or Private.
    pub channel: String,
    // Whether the message typed is a command or not (if applies).
    pub is_command: bool,
    // Whether the message is Private or not.
    pub is_private: bool,
    // ID of the Community the message was sent to. Otherwise NULL.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub community_id: Option<String>,
    // Whether the message contains a mention from another User (i.e. @XYZ).
    pub is_mention: bool,
    // If the user is not in world, this is the screen where the event is fired.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub screen_name: Option<String>,
}

impl SegmentEventChatMessageSent {
    pub fn new(message: &str, is_private: bool) -> Self {
        Self {
            length: message.chars().count() as u32,
            channel: if is_private { "private" } else { "nearby" }.to_owned(),
            is_command: message.starts_with('/'),
            is_private,
            community_id: None,
            is_mention: message.contains('@'),
            screen_name: None,
        }
    }
}

/// The first frame of a login with the player moving in world; the common fields say where.
#[derive(Serialize)]
pub struct SegmentEventFirstMoveInWorld {}

/// A chunk of time in world (`in_world.rs`), named and shaped as the desktop client's so one
/// query sums the playtime of both.
#[derive(Serialize)]
pub struct SegmentEventTimeSpentInWorld {
    pub time_spent: f32,
    pub realm_name: String,
}

/// A screen of mobile's funnel the engine shows itself (the loading screen); the page reports
/// its own with the same shape through `/track`.
#[derive(Serialize)]
pub struct SegmentEventScreenViewed {
    pub screen_name: String,
    // JSON with extra payload, in case we need to track additional metadata.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extra_properties: Option<String>,
}

impl SegmentEventScreenViewed {
    pub fn new(screen_name: &str, extra: serde_json::Value) -> Self {
        Self {
            screen_name: screen_name.to_owned(),
            extra_properties: Some(extra.to_string()),
        }
    }
}

pub fn build_segment_event_batch_item(
    user_id: String,
    common: &SegmentEventCommonExplorerFields,
    event_data: &SegmentEvent,
    queued_at: DateTime<Utc>,
    message_id: String,
) -> SegmentMetricEventBody {
    let mut timestamp = queued_at.to_rfc3339_opts(SecondsFormat::Millis, true);
    let override_position = match event_data {
        SegmentEvent::ExplorerMoveToParcel(current_position, _) => Some(current_position.clone()),
        _ => None,
    };
    let (event_name, event_properties) = match event_data {
        SegmentEvent::Page {
            name,
            properties,
            timestamp: at,
        } => {
            timestamp = at.clone();
            (name.clone(), serde_json::Value::Object(properties.clone()))
        }
        event => event
            .name_and_properties()
            .expect("the engine's events are named above"),
    };

    let mut properties = serde_json::to_value(common).unwrap();
    // merge specific event properties with common properties
    for (k, v) in event_properties.as_object().unwrap().iter() {
        properties[k] = v.clone();
    }

    if let Some(position) = override_position {
        properties["position"] = serde_json::Value::String(position);
    }

    SegmentMetricEventBody {
        r#type: "track".to_string(),
        event: event_name,
        user_id,
        timestamp,
        message_id,
        properties,
    }
}
