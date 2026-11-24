//! Which player settings differ from their defaults: one `Settings Snapshot` event per session,
//! queued once the settings registry and the analytics config exist, and `/settings_snapshot`
//! with its debug panel section. It reads the saved settings, not the host page's session
//! overrides.

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
                fields: &[("Changed", "changed_count"), ("Settings", "changed")],
                actions: &[],
                inputs: &[],
            })
            .add_systems(Update, report_snapshot);
    }
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

/// The settings whose value differs from their default, by name: the variant's name when the
/// setting has named variants, else the number. Unset (negative) values are skipped.
fn changed_settings(settings: &[SettingInfo]) -> serde_json::Map<String, serde_json::Value> {
    let mut changed = std::collections::BTreeMap::new();
    for setting in settings {
        if setting.value < 0.0 || setting.value == setting.default {
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
    let changed = changed_settings(&settings.get());
    cmd.reply_ok(
        serde_json::json!({
            "changed_count": changed.len(),
            "changed": changed,
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
        assert!(changed_settings(&[setting("A", 1.0, 1.0, &[])]).is_empty());

        let mut tenths = setting("Zoom", 0.3, 1.0, &[]);
        tenths.step_size = 0.1;
        let changed = changed_settings(&[
            setting("Same", 2.0, 2.0, &["Off", "On", "Auto"]),
            setting("Left-Drag Camera", 1.0, 0.0, &["Off", "On"]),
            setting("Render Resolution", 80.0, 100.0, &[]),
            setting("Unset", -1.0, 0.0, &["Off", "On"]),
            tenths,
        ]);
        assert_eq!(
            serde_json::Value::Object(changed.clone()),
            serde_json::json!({
                "Left-Drag Camera": "On",
                "Render Resolution": 80.0,
                "Zoom": 0.3,
            })
        );
        assert_eq!(
            changed.keys().collect::<Vec<_>>(),
            ["Left-Drag Camera", "Render Resolution", "Zoom"]
        );
    }

    // A named setting whose value is past its variants is a plain number.
    #[test]
    fn a_value_outside_the_variants_is_a_number() {
        let changed = changed_settings(&[setting("Mode", 5.0, 0.0, &["Off", "On"])]);
        assert_eq!(changed["Mode"], serde_json::json!(5.0));
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
