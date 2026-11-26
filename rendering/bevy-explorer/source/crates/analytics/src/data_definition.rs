use chrono::{DateTime, SecondsFormat, Utc};
use serde::Serialize;

/// The os natively; wasm32-unknown-unknown has no os name (`std::env::consts::OS` is empty).
#[cfg(not(target_arch = "wasm32"))]
const PLATFORM: &str = std::env::consts::OS;
#[cfg(target_arch = "wasm32")]
const PLATFORM: &str = "web";

/// Mobile's wallet value before a sign-in, and after a sign-out.
pub(crate) const UNAUTHENTICATED: &str = "unauthenticated";

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
    // The identity the node's client-event webhook verifies (Segment `context`).
    context: SegmentEventContext,
}

/// Segment `context`: the identity the node's client-event webhook reads (`dcl_eth_address`,
/// `auth_chain`, `realm`, `session_id`, `dcl_renderer_type`).
#[derive(Serialize)]
pub struct SegmentEventContext {
    dcl_eth_address: String,
    // Only on the events the node stores; absent for a guest, which has no delegation.
    #[serde(skip_serializing_if = "Option::is_none")]
    auth_chain: Option<String>,
    realm: String,
    session_id: String,
    dcl_renderer_type: String,
}

/// The events the node's webhook turns into client-event rows for badges.
pub(crate) const CLIENT_EVENTS: [&str; 5] = [
    "move_to_parcel",
    "used_emote",
    "passport_opened",
    "walked_distance",
    "vertical_height_reached",
];

pub(crate) fn is_client_event(name: &str) -> bool {
    CLIENT_EVENTS.iter().any(|e| e.eq_ignore_ascii_case(name))
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
    // The login's `[SIGNER, ECDSA_EPHEMERAL]` chain as JSON, for `context` only.
    #[serde(skip)]
    pub auth_chain: Option<String>,
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
            auth_chain: None,
        }
    }

    pub fn reset_identity(&mut self) {
        self.dcl_eth_address = UNAUTHENTICATED.into();
        self.dcl_is_guest = false;
        self.auth_chain = None;
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
    OpenCamera(SegmentEventOpenCamera),
    TakePhoto(SegmentEventTakePhoto),
    MoveToParcel(SegmentEventMoveToParcel),
    UsedEmote(SegmentEventUsedEmote),
    WalkedDistance(SegmentEventWalkedDistance),
    VerticalHeightReached(SegmentEventVerticalHeightReached),
    FeatureFlagsResolved(SegmentEventFeatureFlagsResolved),
    ReactSituation(SegmentEventReactSituation),
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
            SegmentEvent::OpenCamera(event) => ("open_camera", serde_json::to_value(event)),
            SegmentEvent::TakePhoto(event) => ("take_photo", serde_json::to_value(event)),
            SegmentEvent::MoveToParcel(event) => ("move_to_parcel", serde_json::to_value(event)),
            SegmentEvent::UsedEmote(event) => ("used_emote", serde_json::to_value(event)),
            SegmentEvent::WalkedDistance(event) => ("walked_distance", serde_json::to_value(event)),
            SegmentEvent::VerticalHeightReached(event) => {
                ("vertical_height_reached", serde_json::to_value(event))
            }
            SegmentEvent::FeatureFlagsResolved(event) => {
                ("Feature Flags Resolved", serde_json::to_value(event))
            }
            SegmentEvent::ReactSituation(event) => ("react_situation", serde_json::to_value(event)),
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
    // CPU frames whose duration strictly exceeds the threshold.
    pub hiccup_count: u32,
    // CPU frames measured in this snapshot, including frames below the threshold.
    pub hiccup_sample_count: u32,
    // CPU frame duration threshold in milliseconds.
    pub hiccup_threshold_ms: f32,
    // Sum of the full durations of CPU hiccups in milliseconds.
    pub hiccup_time_ms: f32,
    // Sum of the milliseconds CPU hiccups spent above the threshold.
    pub hiccup_excess_time_ms: f32,
    // GPU timing metrics are present only when the renderer supplied readings.
    #[serde(flatten)]
    pub gpu: Option<SegmentGpuFrameTimes>,
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
pub struct SegmentGpuFrameTimes {
    pub gpu_hiccup_count: u32,
    pub gpu_hiccup_sample_count: u32,
    pub gpu_hiccup_threshold_ms: f32,
    pub gpu_hiccup_time_ms: f32,
    pub gpu_hiccup_excess_time_ms: f32,
    pub gpu_min_frame_time: f32,
    pub gpu_max_frame_time: f32,
    pub gpu_mean_frame_time: f32,
    pub gpu_p1_frame_time: f32,
    pub gpu_p5_frame_time: f32,
    pub gpu_p10_frame_time: f32,
    pub gpu_p20_frame_time: f32,
    pub gpu_p50_frame_time: f32,
    pub gpu_p75_frame_time: f32,
    pub gpu_p80_frame_time: f32,
    pub gpu_p90_frame_time: f32,
    pub gpu_p95_frame_time: f32,
    pub gpu_p99_frame_time: f32,
}

/// The session's rollout flags once the read settled.
#[derive(Serialize)]
pub struct SegmentEventFeatureFlagsResolved {
    /// The flags that are on, as an `--ff` / `?ff=` value that reproduces them.
    pub feature_flags: String,
    /// The launch option's own overrides, empty without any.
    pub feature_flags_override: String,
    /// `service`, `unavailable`, `ignored` or `other-domain`.
    pub feature_flags_source: String,
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
) -> Result<SegmentMetricEventBody, serde_json::Error> {
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

    let mut properties = serde_json::to_value(common)?;
    // merge specific event properties with common properties
    for (k, v) in event_properties.as_object().unwrap().iter() {
        properties[k] = v.clone();
    }

    if let Some(position) = override_position {
        properties["position"] = serde_json::Value::String(position);
    }

    let context = SegmentEventContext {
        dcl_eth_address: common.dcl_eth_address.clone(),
        auth_chain: common
            .auth_chain
            .clone()
            .filter(|_| is_client_event(&event_name)),
        realm: common.realm.clone(),
        session_id: common.session_id.clone(),
        dcl_renderer_type: common.dcl_renderer_type.clone(),
    };

    Ok(SegmentMetricEventBody {
        r#type: "track".to_string(),
        event: event_name,
        user_id,
        timestamp,
        message_id,
        properties,
        context,
    })
}

/// A parcel as `(x, y)`, which the node strips to an `x,y` pointer.
fn parcel_text(parcel: bevy::math::IVec2) -> String {
    format!("({}, {})", parcel.x, parcel.y)
}

/// `move_to_parcel`; `is_empty_scene` and `is_empty_parcel` agree, the node's traveler reads the
/// latter.
#[derive(Serialize, Debug, PartialEq)]
pub struct SegmentEventMoveToParcel {
    pub old_parcel: String,
    pub new_parcel: String,
    // The scene's entity id; null while the parcel is unresolved or empty.
    pub scene_hash: Option<String>,
    pub is_empty_scene: Option<bool>,
    pub is_empty_parcel: Option<bool>,
}

impl SegmentEventMoveToParcel {
    /// `scene`: `None` unresolved, `Some(None)` an empty parcel, `Some(Some(hash))` a scene.
    pub fn new(
        old: Option<bevy::math::IVec2>,
        new: bevy::math::IVec2,
        scene: Option<Option<String>>,
    ) -> Self {
        let empty = scene.as_ref().map(Option::is_none);
        Self {
            old_parcel: old.map_or_else(|| "(NaN, NaN)".to_owned(), parcel_text),
            new_parcel: parcel_text(new),
            scene_hash: scene.flatten(),
            is_empty_scene: empty,
            is_empty_parcel: empty,
        }
    }
}

/// `used_emote`; `name` and `emote_index` are not known here and go empty and -1.
#[derive(Serialize, Debug, PartialEq)]
pub struct SegmentEventUsedEmote {
    pub item_id: String,
    pub is_base: bool,
    pub name: String,
    pub emote_index: i32,
    pub source: String,
}

impl SegmentEventUsedEmote {
    /// `source`: `self` for the player's own (HUD, console), `scene` for a scene's.
    pub fn new(urn: &str, source: &str) -> Self {
        let on_chain = urn.starts_with("urn:") && !urn.starts_with("urn:decentraland:off-chain:");
        Self {
            item_id: urn.to_owned(),
            is_base: !on_chain,
            name: String::new(),
            emote_index: -1,
            source: source.to_owned(),
        }
    }
}

/// `walked_distance`: the steps since the last report.
#[derive(Serialize, Debug, PartialEq)]
pub struct SegmentEventWalkedDistance {
    pub step_count: u64,
}

/// `vertical_height_reached`, once per login; `height` is the elevation gain in metres, which the
/// node's vertical voyager compares with its threshold.
#[derive(Serialize, Debug, PartialEq)]
pub struct SegmentEventVerticalHeightReached {
    pub height: f32,
}

/// The local user reacted with an emoji over their avatar: `reaction_id` is the emoji's index
/// on the wire. Named and shaped as the desktop client's.
#[derive(Serialize)]
pub struct SegmentEventReactSituation {
    pub reaction_id: i32,
}

/// Photo mode opened, from the key or a button.
#[derive(Serialize)]
pub struct SegmentEventOpenCamera {
    pub source: String,
}

impl SegmentEventOpenCamera {
    pub fn new(source: impl ToString) -> Self {
        Self {
            source: source.to_string(),
        }
    }
}

/// A photo was uploaded, with the people in it.
#[derive(Serialize)]
pub struct SegmentEventTakePhoto {
    #[serde(rename = "Photo UUID")]
    pub photo_uuid: String,
    #[serde(rename = "Profiles Enhanced")]
    pub profiles_enhanced: Vec<SegmentEventPhotoProfile>,
    pub source: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SegmentEventPhotoProfile {
    pub address: String,
    pub is_emoting: bool,
}

impl SegmentEventTakePhoto {
    pub fn new(photo_uuid: String, people: &[(String, bool)], source: impl ToString) -> Self {
        Self {
            photo_uuid,
            profiles_enhanced: people
                .iter()
                .map(|(address, is_emoting)| SegmentEventPhotoProfile {
                    address: address.clone(),
                    is_emoting: *is_emoting,
                })
                .collect(),
            source: source.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::math::IVec2;

    fn item(common: &SegmentEventCommonExplorerFields, event: SegmentEvent) -> serde_json::Value {
        let body =
            build_segment_event_batch_item("user".into(), common, &event, Utc::now(), "m".into())
                .unwrap();
        serde_json::to_value(body).unwrap()
    }

    fn signed_in() -> SegmentEventCommonExplorerFields {
        let mut common = SegmentEventCommonExplorerFields::new("s-1".into(), "1.0".into(), None);
        common.dcl_eth_address = "0xabc".into();
        common.realm = "https://peer.decentraland.org/".into();
        common.auth_chain = Some(r#"[{"type":"SIGNER","payload":"0xabc","signature":""}]"#.into());
        common
    }

    // The node reads the identity from `context`; the chain rides only on the events it stores,
    // and never in `properties`, which keep their warehouse shape.
    #[test]
    fn the_context_carries_the_identity_and_the_chain_only_for_client_events() {
        let common = signed_in();
        let moved = item(
            &common,
            SegmentEvent::WalkedDistance(SegmentEventWalkedDistance { step_count: 12 }),
        );
        assert_eq!(moved["type"], "track");
        assert_eq!(moved["event"], "walked_distance");
        assert_eq!(moved["context"]["dcl_eth_address"], "0xabc");
        assert_eq!(
            moved["context"]["auth_chain"],
            common.auth_chain.clone().unwrap()
        );
        assert_eq!(moved["context"]["realm"], "https://peer.decentraland.org/");
        assert_eq!(moved["context"]["session_id"], "s-1");
        assert!(moved["context"]["dcl_renderer_type"]
            .as_str()
            .is_some_and(|t| t.starts_with("dao-bevy-")));
        assert_eq!(moved["properties"]["step_count"], 12);
        assert!(moved["properties"].get("auth_chain").is_none());
        assert_eq!(moved["properties"]["dcl_eth_address"], "0xabc");

        let page = |name: &str| SegmentEvent::Page {
            name: name.into(),
            properties: serde_json::json!({ "receiver_id": "0xdef" })
                .as_object()
                .unwrap()
                .clone(),
            timestamp: "2026-07-02T13:38:08.243Z".into(),
        };
        let passport = item(&common, page("passport_opened"));
        assert_eq!(
            passport["context"]["auth_chain"],
            moved["context"]["auth_chain"]
        );
        assert_eq!(passport["properties"]["receiver_id"], "0xdef");
        let screen = item(&common, page("Screen Viewed"));
        assert_eq!(screen["context"]["dcl_eth_address"], "0xabc");
        assert!(screen["context"].get("auth_chain").is_none());

        let mut guest = common.clone();
        guest.auth_chain = None;
        let walked = item(
            &guest,
            SegmentEvent::WalkedDistance(SegmentEventWalkedDistance { step_count: 1 }),
        );
        assert!(walked["context"].get("auth_chain").is_none());
    }

    // Parcel text the node strips to a pointer; the first parcel of a run comes from nowhere, and
    // an unresolved parcel says nothing about its scene.
    #[test]
    fn move_to_parcel_names_the_parcels_and_the_scene() {
        let common = signed_in();
        let first = item(
            &common,
            SegmentEvent::MoveToParcel(SegmentEventMoveToParcel::new(
                None,
                IVec2::new(-10, 3),
                Some(Some("bafkscene".into())),
            )),
        );
        assert_eq!(first["event"], "move_to_parcel");
        let p = &first["properties"];
        assert_eq!(p["old_parcel"], "(NaN, NaN)");
        assert_eq!(p["new_parcel"], "(-10, 3)");
        assert_eq!(p["scene_hash"], "bafkscene");
        assert_eq!(p["is_empty_scene"], false);
        assert_eq!(p["is_empty_parcel"], false);
        assert!(first["context"]["auth_chain"].is_string());

        let empty =
            SegmentEventMoveToParcel::new(Some(IVec2::new(-10, 3)), IVec2::new(-11, 3), Some(None));
        assert_eq!(empty.old_parcel, "(-10, 3)");
        assert_eq!(empty.scene_hash, None);
        assert_eq!(empty.is_empty_parcel, Some(true));
        let unknown = SegmentEventMoveToParcel::new(None, IVec2::ZERO, None);
        assert_eq!(unknown.is_empty_scene, None);
        assert_eq!(unknown.scene_hash, None);
    }

    // Base and other off-chain emotes count as base; collection emotes do not.
    #[test]
    fn used_emote_tells_base_from_on_chain() {
        let common = signed_in();
        let base = item(
            &common,
            SegmentEvent::UsedEmote(SegmentEventUsedEmote::new(
                "urn:decentraland:off-chain:base-emotes:wave",
                "self",
            )),
        );
        assert_eq!(base["event"], "used_emote");
        assert_eq!(
            base["properties"]["item_id"],
            "urn:decentraland:off-chain:base-emotes:wave"
        );
        assert_eq!(base["properties"]["is_base"], true);
        assert_eq!(base["properties"]["name"], "");
        assert_eq!(base["properties"]["emote_index"], -1);
        assert_eq!(base["properties"]["source"], "self");
        assert!(SegmentEventUsedEmote::new("wave", "scene").is_base);
        assert!(
            !SegmentEventUsedEmote::new("urn:decentraland:matic:collections-v2:0xabc:0", "scene")
                .is_base
        );

        let height = item(
            &common,
            SegmentEvent::VerticalHeightReached(SegmentEventVerticalHeightReached {
                height: 500.5,
            }),
        );
        assert_eq!(height["event"], "vertical_height_reached");
        assert_eq!(height["properties"]["height"], 500.5);
    }
}
