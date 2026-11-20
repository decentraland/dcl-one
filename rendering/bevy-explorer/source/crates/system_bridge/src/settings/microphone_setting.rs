use anyhow::anyhow;
use bevy::prelude::*;
use common::structs::{AppConfig, MicrophoneDevices};

use super::{NamedVariant, Setting, SettingCategory, SettingInfo, Settings};

const TITLE: &str = "Microphone";
const DESCRIPTION: &str = "The microphone voice chat captures from.\n\nSystem Default follows the device's own choice. While a chosen microphone is unplugged, voice chat uses the system default.\n\nThe list fills in once microphone access is granted.";

fn choice(ids: &[String], config: &AppConfig) -> f32 {
    config
        .microphone
        .as_ref()
        .and_then(|id| ids.iter().position(|known| known == id))
        .map_or(0.0, |index| (index + 1) as f32)
}

fn info(labels: Vec<String>, ids: &[String], config: &AppConfig) -> SettingInfo {
    let variant = |name: String| NamedVariant {
        name,
        description: DESCRIPTION.to_owned(),
    };
    SettingInfo {
        name: TITLE.to_owned(),
        category: SettingCategory::Audio.to_string(),
        description: DESCRIPTION.to_owned(),
        min_value: 0.0,
        max_value: (labels.len() + 1) as f32,
        named_variants: std::iter::once(variant("System Default".to_owned()))
            .chain(labels.into_iter().map(variant))
            .collect(),
        value: choice(ids, config),
        default: 0.0,
        step_size: 1.0,
    }
}

/// Points the setting at `devices`. The list only grows, so the newest ids also resolve an index
/// read from an older list.
fn set_choices(setting: &mut Setting, devices: &MicrophoneDevices, config: &AppConfig) {
    let ids = devices.ids();
    setting.info = info(devices.labels(), &ids, config);
    let apply_ids = ids.clone();
    setting.apply = Some(Box::new(
        move |config: &mut AppConfig, value: f32| -> Result<f32, anyhow::Error> {
            let index = value as usize;
            config.microphone = match index {
                0 => None,
                index => Some(
                    apply_ids
                        .get(index - 1)
                        .ok_or(anyhow!("invalid variant index"))?
                        .clone(),
                ),
            };
            Ok(index as f32)
        },
    ));
    setting.load = Box::new(move |config: &AppConfig| choice(&ids, config));
}

/// Adds the setting; its choices follow [`MicrophoneDevices`], which the comms plugin creates,
/// seeds with the saved choice and fills.
pub(super) fn add(app: &mut App, settings: &mut Settings, config: &AppConfig) {
    let mut setting = Setting {
        info: info(Vec::new(), &[], config),
        apply: None,
        load: Box::new(|_: &AppConfig| 0.0),
        runtime_choices: true,
    };
    set_choices(&mut setting, &MicrophoneDevices::default(), config);
    settings.settings.push(setting);
    app.add_systems(
        Update,
        sync_choices.run_if(resource_changed::<MicrophoneDevices>),
    );
}

fn sync_choices(
    devices: Res<MicrophoneDevices>,
    config: Res<AppConfig>,
    mut settings: ResMut<Settings>,
) {
    if let Some(setting) = settings.settings.iter_mut().find(|s| s.info.name == TITLE) {
        set_choices(setting, &devices, &config);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use common::structs::MicrophoneInput;

    fn device(id: &str) -> MicrophoneInput {
        MicrophoneInput {
            id: id.to_owned(),
            label: format!("{id} label"),
        }
    }

    fn app() -> App {
        let mut app = App::new();
        app.insert_resource(AppConfig::default());
        app.init_resource::<MicrophoneDevices>();
        let mut settings = Settings {
            settings: Vec::default(),
        };
        add(&mut app, &mut settings, &AppConfig::default());
        app.insert_resource(settings);
        app
    }

    fn report(app: &mut App, ids: &[&str]) {
        let mut devices = app.world_mut().resource_mut::<MicrophoneDevices>();
        devices.replace(ids.iter().map(|id| device(id)).collect());
        app.update();
    }

    fn set(app: &mut App, value: f32) -> Result<(), anyhow::Error> {
        app.world_mut()
            .resource_scope(|world, mut settings: Mut<Settings>| {
                settings.set_value(&mut world.resource_mut::<AppConfig>(), TITLE, value)
            })
    }

    fn shown(app: &App) -> SettingInfo {
        app.world().resource::<Settings>().get().remove(0)
    }

    #[test]
    fn the_choices_follow_the_reported_devices_and_save_the_chosen_id() {
        let mut app = app();
        app.update();
        assert_eq!(shown(&app).named_variants.len(), 1);

        report(&mut app, &["a", "b"]);
        let names: Vec<_> = shown(&app)
            .named_variants
            .into_iter()
            .map(|v| v.name)
            .collect();
        assert_eq!(names, ["System Default", "a label", "b label"]);

        set(&mut app, 2.0).unwrap();
        assert_eq!(
            app.world().resource::<AppConfig>().microphone.as_deref(),
            Some("b")
        );
        assert_eq!(shown(&app).value, 2.0);

        set(&mut app, 0.0).unwrap();
        assert_eq!(app.world().resource::<AppConfig>().microphone, None);
    }

    #[test]
    fn a_choice_past_the_list_fails_and_the_setting_still_changes_after() {
        let mut app = app();
        report(&mut app, &["a"]);
        assert!(set(&mut app, 2.0).is_err());
        assert_eq!(app.world().resource::<AppConfig>().microphone, None);

        set(&mut app, 1.0).unwrap();
        assert_eq!(
            app.world().resource::<AppConfig>().microphone.as_deref(),
            Some("a")
        );
    }

    #[test]
    fn an_unplugged_choice_stays_chosen_and_keeps_its_place() {
        let mut app = app();
        report(&mut app, &["a", "b"]);
        set(&mut app, 2.0).unwrap();

        report(&mut app, &["a"]);
        let shown_now = shown(&app);
        assert_eq!(shown_now.value, 2.0);
        assert_eq!(shown_now.named_variants[2].name, "b label (unplugged)");
        assert!(!app.world().resource::<MicrophoneDevices>().offers("b"));

        report(&mut app, &["b", "a"]);
        assert_eq!(shown(&app).value, 2.0);
        assert_eq!(shown(&app).named_variants[2].name, "b label");
        assert!(app.world().resource::<MicrophoneDevices>().offers("b"));
    }

    #[test]
    fn an_index_from_an_older_list_names_the_input_it_named_then() {
        let mut app = app();
        report(&mut app, &["a", "b"]);
        // a new input plugged in ahead of the others joins the end
        report(&mut app, &["c", "a"]);
        set(&mut app, 1.0).unwrap();
        assert_eq!(
            app.world().resource::<AppConfig>().microphone.as_deref(),
            Some("a")
        );
        set(&mut app, 2.0).unwrap();
        assert_eq!(
            app.world().resource::<AppConfig>().microphone.as_deref(),
            Some("b")
        );
    }

    #[test]
    fn a_saved_choice_not_yet_reported_shows_as_chosen_by_its_saved_name() {
        let config = AppConfig {
            microphone: Some("headset".to_owned()),
            ..Default::default()
        };
        let mut app = App::new();
        app.insert_resource(config.clone());
        // as the comms plugin seeds it
        let mut devices = MicrophoneDevices::default();
        devices.remember("headset");
        app.insert_resource(devices);
        let mut settings = Settings {
            settings: Vec::default(),
        };
        add(&mut app, &mut settings, &config);
        app.insert_resource(settings);
        app.update();
        let shown_now = shown(&app);
        assert_eq!(shown_now.value, 1.0);
        assert_eq!(shown_now.named_variants[1].name, "Saved microphone");
        assert!(!app
            .world()
            .resource::<MicrophoneDevices>()
            .missing("headset"));

        report(&mut app, &["a", "headset"]);
        let shown_now = shown(&app);
        assert_eq!(shown_now.value, 1.0);
        assert_eq!(shown_now.named_variants[1].name, "headset label");
    }
}
