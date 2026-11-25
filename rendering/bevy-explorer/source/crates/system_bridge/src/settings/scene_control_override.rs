use common::structs::AppConfig;

use super::{AppSetting, EnumAppSetting, SettingCategory};

#[derive(Debug, PartialEq, Eq)]
pub enum SceneControlOverrideSetting {
    Off,
    On,
}

impl EnumAppSetting for SceneControlOverrideSetting {
    fn variants() -> Vec<Self> {
        vec![Self::Off, Self::On]
    }

    fn name(&self) -> String {
        match self {
            Self::Off => "Off",
            Self::On => "On",
        }
        .to_owned()
    }
}

impl AppSetting for SceneControlOverrideSetting {
    type Param = ();

    fn title() -> String {
        "Allow overriding scene controls".to_owned()
    }

    fn category() -> SettingCategory {
        SettingCategory::Gameplay
    }

    fn description(&self) -> String {
        format!(
            "Whether you can change what the scene you stand in controls, such as the time of day.\n\n{}",
            match self {
                Self::Off => "Off\nThe scene's time of day stays, and the Skybox menu is locked there.",
                Self::On => "On\nA time you set wins over the scene's until you leave. The scene's time is back when you return.",
            }
        )
    }

    fn save(&self, config: &mut AppConfig) {
        config.allow_scene_control_override = *self == Self::On;
    }

    fn load(config: &AppConfig) -> Self {
        if config.allow_scene_control_override {
            Self::On
        } else {
            Self::Off
        }
    }
}
