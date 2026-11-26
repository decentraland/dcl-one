use bevy::prelude::*;
use common::structs::AppConfig;

use super::{AppSetting, EnumAppSetting};

#[derive(Debug, PartialEq, Eq)]
pub enum UnlimitedAvatarsSetting {
    Off,
    On,
}

impl EnumAppSetting for UnlimitedAvatarsSetting {
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

impl AppSetting for UnlimitedAvatarsSetting {
    type Param = ();

    fn title() -> String {
        "Unlimited Avatars".to_owned()
    }

    fn description(&self) -> String {
        format!("Unlimited Avatars\n\nWhether to render every avatar nearby instead of only the closest Max Avatars.\n\n{}",
            match self {
                Self::Off => "Off: Show only the closest Max Avatars, which keeps frame rate steadier in busy environments.",
                Self::On => "On: Render every nearby avatar, other users and scene-created avatars alike. Large crowds will lower your frame rate. Max Avatars is kept and applies again when this is turned off.",
            }
        )
    }

    fn save(&self, config: &mut AppConfig) {
        config.max_avatars_unlimited = matches!(self, Self::On);
    }

    fn load(config: &AppConfig) -> Self {
        if config.max_avatars_unlimited {
            Self::On
        } else {
            Self::Off
        }
    }

    fn category() -> super::SettingCategory {
        super::SettingCategory::Performance
    }
}
