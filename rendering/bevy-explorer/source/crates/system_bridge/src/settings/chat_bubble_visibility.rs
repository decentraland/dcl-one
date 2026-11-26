use bevy::prelude::*;
use common::structs::{AppConfig, ChatBubbleVisibility};

use super::{AppSetting, EnumAppSetting, SettingCategory};

impl EnumAppSetting for ChatBubbleVisibility {
    fn variants() -> Vec<Self> {
        vec![Self::All, Self::NearbyOnly, Self::None]
    }

    fn name(&self) -> String {
        match self {
            Self::All => "All",
            Self::NearbyOnly => "Nearby only",
            Self::None => "None",
        }
        .to_owned()
    }
}

impl AppSetting for ChatBubbleVisibility {
    type Param = ();

    fn title() -> String {
        "Chat bubbles and reactions".to_owned()
    }

    fn category() -> SettingCategory {
        SettingCategory::Gameplay
    }

    fn description(&self) -> String {
        format!(
            "Controls which chat messages show in a bubble over the nametag of whoever wrote them, and whether reactions show over avatars.\n\n{}",
            match self {
                Self::All => "Show nearby, community and direct messages, and reactions",
                Self::NearbyOnly => "Show nearby messages only, and reactions",
                Self::None => "Don't show chat bubbles or reactions over avatars",
            }
        )
    }

    fn save(&self, config: &mut AppConfig) {
        config.chat_bubble_visibility = *self;
    }

    fn load(config: &AppConfig) -> Self {
        config.chat_bubble_visibility
    }

    // read from the config when a message arrives or a reaction plays
    fn apply(
        &self,
        _param: bevy::ecs::system::SystemParamItem<Self::Param>,
        _commands: Commands,
        _cameras: &bevy::platform::collections::HashSet<Entity>,
    ) {
    }
}
