use bevy::{ecs::system::lifetimeless::SResMut, platform::collections::HashSet, prelude::*};
use common::structs::{AppConfig, AudioSettings};

use super::{AppSetting, EnumAppSetting};

#[derive(Debug, PartialEq, Eq)]
pub enum MuteMicInBackgroundSetting {
    Off,
    On,
}

impl EnumAppSetting for MuteMicInBackgroundSetting {
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

impl AppSetting for MuteMicInBackgroundSetting {
    type Param = SResMut<AudioSettings>;

    fn title() -> String {
        "Mute Mic in Background".to_owned()
    }

    fn description(&self) -> String {
        "Mute Mic in Background\n\nOn: the microphone pauses while the Explorer is in the background (another app or tab). Off: it stays live.".to_owned()
    }

    fn category() -> super::SettingCategory {
        super::SettingCategory::Audio
    }

    fn apply(&self, mut settings: ResMut<AudioSettings>, _: Commands, _: &HashSet<Entity>) {
        settings.mute_mic_in_background = matches!(self, Self::On);
    }

    fn save(&self, config: &mut AppConfig) {
        config.audio.mute_mic_in_background = matches!(self, Self::On);
    }

    fn load(config: &AppConfig) -> Self {
        if config.audio.mute_mic_in_background {
            Self::On
        } else {
            Self::Off
        }
    }
}
