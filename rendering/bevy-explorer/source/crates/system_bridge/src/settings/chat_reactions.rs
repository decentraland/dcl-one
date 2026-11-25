use bevy::prelude::*;
use common::structs::AppConfig;

use super::{AppSetting, EnumAppSetting, SettingCategory};

/// Whether situational reactions are also shown on screen, rising from the chat's reactions
/// button. Over avatars they show either way.
#[derive(Debug, PartialEq, Eq)]
pub enum ChatReactionsSetting {
    Off,
    On,
}

impl EnumAppSetting for ChatReactionsSetting {
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

impl AppSetting for ChatReactionsSetting {
    type Param = ();

    fn title() -> String {
        "Chat Reactions".to_owned()
    }

    fn category() -> SettingCategory {
        SettingCategory::Chat
    }

    fn description(&self) -> String {
        format!(
            "Shows reactions on screen as they happen, rising from the chat's reactions button: yours and those of the players around you.\n\n{}",
            match self {
                Self::On => "Show reactions on screen",
                Self::Off => "Show reactions over avatars only",
            }
        )
    }

    fn save(&self, config: &mut AppConfig) {
        config.chat_reactions = matches!(self, Self::On);
    }

    fn load(config: &AppConfig) -> Self {
        if config.chat_reactions {
            Self::On
        } else {
            Self::Off
        }
    }

    // read from the config as each reaction plays
    fn apply(
        &self,
        _param: bevy::ecs::system::SystemParamItem<Self::Param>,
        _commands: Commands,
        _cameras: &bevy::platform::collections::HashSet<Entity>,
    ) {
    }
}
