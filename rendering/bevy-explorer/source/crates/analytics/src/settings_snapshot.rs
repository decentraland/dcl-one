//! The `Settings Snapshot` event: the settings whose value differs from their default. It is
//! queued at startup (`reason: startup`) and again once a change has settled (`reason: changed`).
//! Every registered setting is reported except the ones in [`EXCLUDED`] and the ones whose
//! choices are filled in at runtime. It reads the saved settings, not the host page's session
//! overrides. `/settings_snapshot` and its debug panel section show the same result live.

use std::time::Duration;

use bevy::prelude::*;
use bevy_console::ConsoleCommand;
use common::structs::{DmPrivacy, WindowSetting};
use console::{AddDebugSection, DebugSection, DoAddConsoleCommand};
use system_bridge::settings::{
    cache_size::CacheSizeSetting, max_downloads::MaxDownloadsSetting,
    scene_threads::SceneThreadsSetting, step_number, video_threads::VideoThreadsSetting,
    AppSetting, SettingInfo, Settings,
};

use crate::{
    data_definition::{SegmentEvent, SegmentEventSettingsSnapshot},
    segment_system::{SegmentConfig, SegmentMetricsEvents},
};

type Changed = serde_json::Map<String, serde_json::Value>;

/// How long the changed settings stay the same before they are sent again, on the real clock so
/// slow or throttled frames do not stretch it.
const SETTLE: Duration = Duration::from_secs(5);

/// The settings the snapshot leaves out, by type so a renamed title still matches. Every other
/// registered setting is reported, a new one included. DM privacy is the server's value, unset
/// until social reads it after sign-in. Cache size, the thread counts and max downloads tune the
/// machine, not the player's preferences. The window mode is window state.
///
/// "Differs from its default" is read against the default of this build. A setting whose default
/// changes needs a migration in `AppConfig::reset_outdated_settings`, or every saved config still
/// on the old default is reported as changed.
const EXCLUDED: &[fn() -> String] = &[
    <DmPrivacy as AppSetting>::title,
    <CacheSizeSetting as AppSetting>::title,
    <SceneThreadsSetting as AppSetting>::title,
    <VideoThreadsSetting as AppSetting>::title,
    <MaxDownloadsSetting as AppSetting>::title,
    <WindowSetting as AppSetting>::title,
];

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

/// The registry's changed settings, as the event reports them. A setting whose choices are filled
/// in at runtime is left out: its choice names come from the device (the microphone list), not
/// from the engine.
pub(crate) fn snapshot(settings: &Settings) -> Changed {
    changed_settings(
        settings
            .settings
            .iter()
            .filter(|setting| !setting.runtime_choices())
            .map(|setting| &setting.info),
    )
}

/// The settings that are not [`EXCLUDED`] and whose value differs from their default, by name:
/// the variant's name when the setting has named variants, else the number. Unset (negative)
/// values are skipped.
fn changed_settings<'a>(settings: impl IntoIterator<Item = &'a SettingInfo>) -> Changed {
    let mut changed = std::collections::BTreeMap::new();
    for setting in settings {
        if EXCLUDED.iter().any(|title| title() == setting.name)
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
            None => serde_json::Value::from(step_number(setting.value, setting.step_size)),
        };
        changed.insert(setting.name.clone(), value);
    }
    changed.into_iter().collect()
}

/// Decides when a snapshot is sent: at once the first time, then once the changed settings have
/// stayed the same for [`SETTLE`] and differ from the last map sent.
#[derive(Default)]
struct Reporter {
    sent: Option<Changed>,
    /// The map waiting to settle and the time it was first seen.
    pending: Option<(Changed, Duration)>,
}

impl Reporter {
    /// `current` is the map on frames it was recomputed, `now` the elapsed time. Returns the
    /// event's reason and map when one is due.
    fn observe(
        &mut self,
        current: Option<Changed>,
        now: Duration,
    ) -> Option<(&'static str, Changed)> {
        let Some(sent) = &self.sent else {
            let map = current?;
            self.sent = Some(map.clone());
            return Some(("startup", map));
        };
        if let Some(map) = current {
            if &map == sent {
                self.pending = None;
            } else if self.pending.as_ref().map(|(pending, _)| pending) != Some(&map) {
                self.pending = Some((map, now));
            }
        }
        let (_, since) = self.pending.as_ref()?;
        if now.saturating_sub(*since) < SETTLE {
            return None;
        }
        let (map, _) = self.pending.take()?;
        self.sent = Some(map.clone());
        Some(("changed", map))
    }
}

/// Queues the snapshot at startup, and again once a change to the settings has settled.
fn report_snapshot(
    settings: Res<Settings>,
    config: Res<SegmentConfig>,
    time: Res<Time<Real>>,
    mut metrics: ResMut<SegmentMetricsEvents>,
    mut reporter: Local<Reporter>,
) {
    let current = (reporter.sent.is_none() || settings.is_changed()).then(|| snapshot(&settings));
    let Some((reason, changed)) = reporter.observe(current, time.elapsed()) else {
        return;
    };
    metrics.add_event(
        SegmentEvent::SettingsSnapshot(SegmentEventSettingsSnapshot {
            reason,
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
    let changed = snapshot(&settings);
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
    use common::{rpc::RpcStreamSender, structs::AppConfig};
    use system_bridge::settings::{player_settings::WalkSpeedSetting, NamedVariant};

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
        let mut tenths = setting("Camera Zoom", 0.3, 1.0, &[]);
        tenths.step_size = 0.1;
        let settings = [
            setting("Fog", 1.0, 1.0, &[]),
            setting("VSync", 1.0, 0.0, &["Off", "On"]),
            setting("Render Resolution", 80.0, 100.0, &[]),
            setting("Chat bubbles", -1.0, 0.0, &["All", "Nearby only", "None"]),
            tenths,
            setting(&CacheSizeSetting::title(), 3.0, 1.0, &[]),
        ];
        let changed = changed_settings(&settings);
        assert_eq!(
            serde_json::Value::Object(changed.clone()),
            serde_json::json!({
                "Camera Zoom": 0.3,
                "Render Resolution": 80.0,
                "VSync": "On",
            })
        );
        assert_eq!(
            changed.keys().collect::<Vec<_>>(),
            ["Camera Zoom", "Render Resolution", "VSync"]
        );
    }

    // A named setting whose value is past its variants is a plain number.
    #[test]
    fn a_value_outside_the_variants_is_a_number() {
        let changed = changed_settings(&[setting("Anti-aliasing", 9.0, 2.0, &["Off", "On"])]);
        assert_eq!(changed["Anti-aliasing"], serde_json::json!(9.0));
    }

    fn map(value: i32) -> Changed {
        let mut map = Changed::new();
        map.insert("Fog".to_owned(), value.into());
        map
    }

    #[test]
    fn a_change_is_sent_once_it_has_settled() {
        let secs = Duration::from_secs;
        let mut reporter = Reporter::default();
        assert_eq!(
            reporter.observe(Some(Changed::new()), secs(0)).unwrap().0,
            "startup"
        );
        assert!(reporter.observe(None, secs(60)).is_none());

        // a second change during the wait restarts it
        assert!(reporter.observe(Some(map(1)), secs(100)).is_none());
        assert!(reporter.observe(Some(map(2)), secs(103)).is_none());
        assert!(reporter.observe(None, secs(105)).is_none());
        let (reason, changed) = reporter.observe(None, secs(108)).unwrap();
        assert_eq!((reason, changed), ("changed", map(2)));
        assert!(reporter.observe(None, secs(200)).is_none());

        // changing back to the sent map cancels the wait
        assert!(reporter.observe(Some(map(3)), secs(300)).is_none());
        assert!(reporter.observe(Some(map(2)), secs(302)).is_none());
        assert!(reporter.observe(None, secs(400)).is_none());
    }

    fn app() -> (
        App,
        common::rpc::RpcStreamReceiver<system_bridge::AnalyticsEvent>,
    ) {
        let mut app = App::new();
        let (sender, receiver) = RpcStreamSender::channel();
        let mut metrics = SegmentMetricsEvents::default();
        metrics.add_listener(sender);
        let mut settings = Settings { settings: vec![] };
        settings.add_int_setting::<WalkSpeedSetting>(&AppConfig::default());
        app.insert_resource(metrics)
            .insert_resource(settings)
            .insert_resource(SegmentConfig::new(
                "user".into(),
                "s".into(),
                "1.0".into(),
                None,
                false,
            ))
            .insert_resource(Time::<Real>::default())
            .add_systems(Update, report_snapshot);
        (app, receiver)
    }

    // Time is advanced by hand: no time plugin runs in the test app.
    #[test]
    fn the_system_sends_at_startup_and_again_after_a_change_settles() {
        let (mut app, mut receiver) = app();
        app.update();
        let got = receiver.try_recv().unwrap();
        assert_eq!(got.event, "Settings Snapshot");
        assert_eq!(got.properties["reason"], "startup");
        assert_eq!(got.properties["changed_count"], 0);
        assert!(receiver.try_recv().is_err());

        let mut config = AppConfig::default();
        config.player_settings.walk_speed = 1.3;
        app.world_mut().resource_mut::<Settings>().refresh(&config);
        app.update();
        assert!(receiver.try_recv().is_err());

        app.world_mut()
            .resource_mut::<Time<Real>>()
            .advance_by(SETTLE + Duration::from_secs(1));
        app.update();
        let got = receiver.try_recv().unwrap();
        assert_eq!(got.properties["reason"], "changed");
        assert_eq!(got.properties["changed"]["Walk Speed"], 1.3);
        app.update();
        assert!(receiver.try_recv().is_err());
    }
}
