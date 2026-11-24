//! Which of the [`REPORTED`] settings differ from their defaults: one `Settings Snapshot` event
//! per session, queued once the settings registry and the analytics config exist, and
//! `/settings_snapshot` with its debug panel section. It reads the saved settings, not the host
//! page's session overrides.

use bevy::prelude::*;
use bevy_console::ConsoleCommand;
use console::{AddDebugSection, DebugSection, DoAddConsoleCommand};
use system_bridge::settings::{SettingInfo, Settings};

use crate::{
    data_definition::{SegmentEvent, SegmentEventSettingsSnapshot},
    segment_system::{SegmentConfig, SegmentMetricsEvents},
};

pub(crate) struct SettingsSnapshotPlugin;

impl Plugin for SettingsSnapshotPlugin {
    fn build(&self, app: &mut App) {
        app.add_console_command::<SettingsSnapshotCommand, _>(settings_snapshot_command)
            .add_debug_section(DebugSection {
                title: "Settings snapshot",
                query: "/settings_snapshot",
                fields: &[
                    ("Changed", "changed_count"),
                    ("Settings", "changed"),
                    ("Not on this platform", "missing"),
                ],
                actions: &[],
                inputs: &[],
            })
            .add_systems(Update, report_snapshot);
    }
}

/// The settings the snapshot reports, by their name in the settings registry (`title()` in
/// `system_bridge/src/settings/`). Every other setting is left out, even when changed: they are
/// tuning knobs few players touch (cache size, threads, downloads, LOD rings) or window and
/// rendering details that say little about the player. A new setting is reported only once it is
/// added here; a name that matches no registered setting is shown as `missing` by
/// `/settings_snapshot` (some settings exist on one platform only).
const REPORTED: &[&str] = &[
    // Gameplay
    "Left-Drag Camera",
    "Walk Speed",
    "Jog Speed",
    "Run Speed",
    "Jump Height",
    "Running Jump Height",
    "Chat bubbles",
    "Point at marker visibility",
    "Constrain Scene Uis",
    // Controls
    "Camera smoothing",
    "Pointer and Locked Camera sensitivity",
    "Camera Sensitivity",
    "Camera Zoom",
    "Movement sensitivity",
    "Scroll sensitivity",
    // Audio
    "Master Volume",
    "Scene Volume",
    "Voice Volume",
    "Voice Chat Volume",
    "System Volume",
    "Avatar Volume",
    "Mute Mic in Background",
    // Chat. "Receive direct messages from" is left out: it is the server's value, unset until
    // social reads it after sign-in, so a snapshot taken at startup would always skip it.
    "Language",
    // Graphics: the settings the HUD's Graphics Preset sets
    // (`react-web/bridge-scene/src/domains/graphicsPresets.ts`), so the preset can be told apart
    "Target Frame Rate",
    "VSync",
    "Render Resolution",
    "Anti-aliasing",
    "Bloom",
    "Avatar Outline",
    "Jiggle Bones",
    "Shadow settings",
    "Shadow Distance",
    "Light Count",
    "Shadow Caster Count",
    "Lens Flare",
    "Depth of Field",
    "SSAO",
    "Fog",
    "Sky Reflections",
    // Performance
    "Max Avatars",
    "Scene Load Distance",
];

/// The [`REPORTED`] names that match no registered setting.
fn missing(settings: &[SettingInfo]) -> Vec<&'static str> {
    REPORTED
        .iter()
        .copied()
        .filter(|name| !settings.iter().any(|s| s.name == *name))
        .collect()
}

// Settings use whole steps (including tens, tenths and twentieths). Convert the step count to
// a JSON number without widening a binary f32 approximation of e.g. 0.05 or 1/10.
fn number(value: f32, step: f32) -> f64 {
    if step <= 0.0 || !step.is_finite() {
        return f64::from(value);
    }
    let steps = f64::from((value / step).round());
    if step >= 1.0 {
        steps * f64::from(step)
    } else {
        steps / f64::from(step.recip())
    }
}

/// The [`REPORTED`] settings whose value differs from their default, by name: the variant's name
/// when the setting has named variants, else the number. Unset (negative) values are skipped.
fn changed_settings(settings: &[SettingInfo]) -> serde_json::Map<String, serde_json::Value> {
    let mut changed = std::collections::BTreeMap::new();
    for setting in settings {
        if !REPORTED.contains(&setting.name.as_str())
            || setting.value < 0.0
            || setting.value == setting.default
        {
            continue;
        }
        let variant = (setting.value.fract() == 0.0)
            .then(|| setting.named_variants.get(setting.value as usize))
            .flatten();
        let value = match variant {
            Some(variant) => serde_json::Value::from(variant.name.clone()),
            None => serde_json::Value::from(number(setting.value, setting.step_size)),
        };
        changed.insert(setting.name.clone(), value);
    }
    changed.into_iter().collect()
}

/// Queues the event once, when the settings and the analytics config both exist.
fn report_snapshot(
    settings: Option<Res<Settings>>,
    config: Option<Res<SegmentConfig>>,
    mut metrics: ResMut<SegmentMetricsEvents>,
    mut reported: Local<bool>,
) {
    if *reported {
        return;
    }
    let (Some(settings), Some(config)) = (settings, config) else {
        return;
    };
    *reported = true;
    let changed = changed_settings(&settings.get());
    metrics.add_event(
        SegmentEvent::SettingsSnapshot(SegmentEventSettingsSnapshot {
            changed_count: changed.len() as u32,
            changed,
        }),
        config.common(),
    );
}

/// Show which settings differ from their defaults, as `Settings Snapshot` reports them.
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/settings_snapshot")]
struct SettingsSnapshotCommand;

fn settings_snapshot_command(
    mut cmd: ConsoleCommand<SettingsSnapshotCommand>,
    settings: Res<Settings>,
) {
    if cmd.take().is_none() {
        return;
    }
    let settings = settings.get();
    let changed = changed_settings(&settings);
    cmd.reply_ok(
        serde_json::json!({
            "changed_count": changed.len(),
            "changed": changed,
            "missing": missing(&settings),
        })
        .to_string(),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use common::rpc::RpcStreamSender;
    use system_bridge::settings::NamedVariant;

    fn setting(name: &str, value: f32, default: f32, variants: &[&str]) -> SettingInfo {
        SettingInfo {
            name: name.to_owned(),
            category: "Gameplay".to_owned(),
            description: String::new(),
            min_value: -1.0,
            max_value: 100.0,
            named_variants: variants
                .iter()
                .map(|name| NamedVariant {
                    name: (*name).to_owned(),
                    description: String::new(),
                })
                .collect(),
            step_size: 1.0,
            value,
            default,
            disabled: false,
        }
    }

    #[test]
    fn only_changed_set_values_are_named_or_numbered() {
        assert!(changed_settings(&[setting("Fog", 1.0, 1.0, &[])]).is_empty());

        let mut tenths = setting("Camera Zoom", 0.3, 1.0, &[]);
        tenths.step_size = 0.1;
        let changed = changed_settings(&[
            setting("Shadow settings", 2.0, 2.0, &["Off", "Low", "High"]),
            setting("Left-Drag Camera", 1.0, 0.0, &["Off", "On"]),
            setting("Render Resolution", 80.0, 100.0, &[]),
            setting("Chat bubbles", -1.0, 0.0, &["All", "Nearby only", "None"]),
            tenths,
        ]);
        assert_eq!(
            serde_json::Value::Object(changed.clone()),
            serde_json::json!({
                "Camera Zoom": 0.3,
                "Left-Drag Camera": "On",
                "Render Resolution": 80.0,
            })
        );
        assert_eq!(
            changed.keys().collect::<Vec<_>>(),
            ["Camera Zoom", "Left-Drag Camera", "Render Resolution"]
        );
    }

    // A named setting whose value is past its variants is a plain number.
    #[test]
    fn a_value_outside_the_variants_is_a_number() {
        let changed = changed_settings(&[setting("Anti-aliasing", 9.0, 2.0, &["Off", "On"])]);
        assert_eq!(changed["Anti-aliasing"], serde_json::json!(9.0));
    }

    // Only the listed settings are reported, even when another one changed.
    #[test]
    fn a_setting_outside_the_list_is_not_reported() {
        assert!(!REPORTED.contains(&"Disk Cache Size"));
        let changed = changed_settings(&[setting("Disk Cache Size", 3.0, 1.0, &[])]);
        assert!(changed.is_empty());
        let registry = [setting("Fog", 1.0, 1.0, &["Off", "On"])];
        let absent = missing(&registry);
        assert!(!absent.contains(&"Fog") && absent.contains(&"Left-Drag Camera"));
    }

    fn app(
        with_settings: bool,
    ) -> (
        App,
        common::rpc::RpcStreamReceiver<system_bridge::AnalyticsEvent>,
    ) {
        let mut app = App::new();
        let (sender, receiver) = RpcStreamSender::channel();
        let mut metrics = SegmentMetricsEvents::default();
        metrics.add_listener(sender);
        app.insert_resource(metrics)
            .add_systems(Update, report_snapshot);
        if with_settings {
            app.insert_resource(Settings { settings: vec![] })
                .insert_resource(SegmentConfig::new(
                    "user".into(),
                    "s".into(),
                    "1.0".into(),
                    None,
                    false,
                ));
        }
        (app, receiver)
    }

    // The count of nothing changed is still sent, once: it is the denominator.
    #[test]
    fn the_snapshot_is_sent_once_even_when_nothing_changed() {
        let (mut app, mut receiver) = app(true);
        app.update();
        app.update();
        let got = receiver.try_recv().unwrap();
        assert_eq!(got.event, "Settings Snapshot");
        assert_eq!(got.properties["changed_count"], 0);
        assert!(got.properties["changed"].as_object().unwrap().is_empty());
        assert!(receiver.try_recv().is_err());
    }

    #[test]
    fn nothing_is_sent_before_the_settings_exist() {
        let (mut app, mut receiver) = app(false);
        app.update();
        assert!(receiver.try_recv().is_err());
    }
}
