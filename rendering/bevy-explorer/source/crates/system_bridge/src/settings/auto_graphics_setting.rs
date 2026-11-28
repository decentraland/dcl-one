use common::structs::AppConfig;

use super::{AppSetting, EnumAppSetting, SettingCategory};

/// Whether the explorer lowers graphics for the session by itself (`visuals::auto_graphics`).
#[derive(Debug, PartialEq, Eq)]
pub enum AutoGraphicsSetting {
    Off,
    On,
}

impl EnumAppSetting for AutoGraphicsSetting {
    fn variants() -> Vec<Self> {
        vec![Self::Off, Self::On]
    }

    fn name(&self) -> String {
        match self {
            AutoGraphicsSetting::Off => "Off",
            AutoGraphicsSetting::On => "On",
        }
        .to_owned()
    }
}

impl AppSetting for AutoGraphicsSetting {
    type Param = ();

    fn title() -> String {
        "Auto Graphics".to_owned()
    }

    fn category() -> SettingCategory {
        SettingCategory::Performance
    }

    fn description(&self) -> String {
        "Auto Graphics\n\nOn: while the frame rate is under its target, graphics are lowered for this session, one step at a time and only while each step makes frames shorter: shadows off first, then the render resolution down to 50%, then bloom off. They go back up when there is room again, and a message says when they were lowered. Your saved settings do not change. Off: graphics always follow your settings.".to_owned()
    }

    fn save(&self, config: &mut AppConfig) {
        config.graphics.auto_graphics = matches!(self, AutoGraphicsSetting::On);
    }

    fn load(config: &AppConfig) -> Self {
        if config.graphics.auto_graphics {
            Self::On
        } else {
            Self::Off
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::Settings;

    #[test]
    fn is_off_until_the_player_turns_it_on() {
        let mut settings = Settings { settings: vec![] };
        let mut config = AppConfig::default();
        settings.add_enum_setting::<AutoGraphicsSetting>(&config);
        assert_eq!(settings.settings[0].info.value, 0.0);
        assert!(!config.graphics.auto_graphics);

        let name = AutoGraphicsSetting::title();
        settings.set_value(&mut config, &name, 1.0).unwrap();
        assert!(config.graphics.auto_graphics);
    }
}
