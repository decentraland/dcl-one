use bevy::prelude::*;
use common::structs::AppConfig;

use super::{AppSetting, EnumAppSetting};

#[derive(Debug, PartialEq, Eq)]
pub struct FpsTargetSetting(usize);

impl EnumAppSetting for FpsTargetSetting {
    fn variants() -> Vec<Self> {
        vec![
            Self(10),
            Self(15),
            Self(20),
            Self(30),
            Self(60),
            Self(120),
            Self(144),
            Self(999),
        ]
    }

    fn name(&self) -> String {
        if self.0 == 999 {
            "Uncapped".to_owned()
        } else {
            format!("{} fps", self.0)
        }
    }
}

impl AppSetting for FpsTargetSetting {
    type Param = ();
    fn title() -> String {
        "Target Frame Rate".to_owned()
    }

    fn description(&self) -> String {
        let text = "The target frame rate. Lower values may be uncomfortable or jerky, while higher values will result in a smoother experience but increased CPU and GPU load.";
        if cfg!(target_arch = "wasm32") {
            text.to_owned()
        } else {
            format!(
                "{text} Ignored while VSync is on: frames then follow the monitor's refresh rate."
            )
        }
    }

    fn save(&self, config: &mut AppConfig) {
        config.graphics.fps_target = self.0;
    }

    fn load(config: &AppConfig) -> Self {
        Self(config.graphics.fps_target)
    }

    fn category() -> super::SettingCategory {
        super::SettingCategory::Performance
    }

    // VSync paces native frames at the refresh rate; the web has no VSync setting
    fn is_disabled(config: &AppConfig) -> bool {
        !cfg!(target_arch = "wasm32") && config.graphics.vsync
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    use crate::settings::Settings;

    #[test]
    fn the_frame_rate_target_is_greyed_out_while_vsync_is_on() {
        let mut config = AppConfig::default();
        let mut settings = Settings {
            settings: Vec::new(),
        };
        settings.add_enum_setting::<FpsTargetSetting>(&config);
        assert!(!settings.settings[0].info.disabled);
        config.graphics.vsync = true;
        settings.refresh(&config);
        assert!(settings.settings[0].info.disabled);
    }
}
