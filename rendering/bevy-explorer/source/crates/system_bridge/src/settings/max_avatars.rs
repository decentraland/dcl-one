use bevy::prelude::*;
use common::structs::AppConfig;

use super::{AppSetting, IntAppSetting};

#[derive(Debug, PartialEq, Eq)]
pub struct MaxAvatarsSetting(i32);

impl IntAppSetting for MaxAvatarsSetting {
    fn from_int(value: i32) -> Self {
        Self(value)
    }

    fn value(&self) -> i32 {
        self.0
    }

    fn min() -> i32 {
        1
    }

    fn max() -> i32 {
        100
    }
}

impl AppSetting for MaxAvatarsSetting {
    type Param = ();

    fn title() -> String {
        "Max Avatars".to_owned()
    }

    fn description(&self) -> String {
        "Max Avatars\n\nHow many avatars to render. Limiting this can help reduce frame rate drops in busy environments. If there are more avatars nearby, only the closest will be shown. It has no effect while Unlimited Avatars is on. This applies to other users and to scene-created avatars.".to_string()
    }

    fn save(&self, config: &mut AppConfig) {
        config.max_avatars = self.0 as usize;
    }

    fn load(config: &AppConfig) -> Self {
        Self(config.max_avatars as i32)
    }

    /// The slider rests while Unlimited Avatars is on; its value is kept.
    fn is_disabled(config: &AppConfig) -> bool {
        config.max_avatars_unlimited
    }

    fn category() -> super::SettingCategory {
        super::SettingCategory::Performance
    }
}
