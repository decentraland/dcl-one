use common::structs::AppConfig;

use super::{AppSetting, EnumAppSetting};

#[derive(Debug, PartialEq, Eq)]
pub enum AvatarGhostsSetting {
    Off,
    On,
}

impl EnumAppSetting for AvatarGhostsSetting {
    fn variants() -> Vec<Self> {
        vec![Self::Off, Self::On]
    }

    fn name(&self) -> String {
        match self {
            AvatarGhostsSetting::Off => "Off",
            AvatarGhostsSetting::On => "On",
        }
        .to_owned()
    }
}

impl AppSetting for AvatarGhostsSetting {
    type Param = ();

    fn title() -> String {
        "Avatar Ghosts".to_owned()
    }

    fn description(&self) -> String {
        "A hologram ghost where an avatar is still loading.\n\nOn: a ghost in the player's name colour appears at once and hands over to the avatar once it has loaded. Off: nothing shows until the avatar has loaded.".to_owned()
    }

    fn category() -> super::SettingCategory {
        super::SettingCategory::Graphics
    }

    fn save(&self, config: &mut AppConfig) {
        config.graphics.avatar_ghosts = matches!(self, AvatarGhostsSetting::On);
    }

    fn load(config: &AppConfig) -> Self {
        if config.graphics.avatar_ghosts {
            Self::On
        } else {
            Self::Off
        }
    }
}
