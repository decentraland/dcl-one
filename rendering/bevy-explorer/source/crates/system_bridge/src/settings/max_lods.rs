use bevy::prelude::*;
use common::structs::AppConfig;

use super::{AppSetting, IntAppSetting};

#[derive(Debug, PartialEq, Eq)]
pub struct MaxNearLodsSetting(i32);

impl IntAppSetting for MaxNearLodsSetting {
    fn from_int(value: i32) -> Self {
        Self(value)
    }

    fn value(&self) -> i32 {
        self.0
    }

    fn min() -> i32 {
        0
    }

    fn max() -> i32 {
        40
    }
}

impl AppSetting for MaxNearLodsSetting {
    type Param = ();

    fn title() -> String {
        "Max Near LODs".to_owned()
    }

    fn description(&self) -> String {
        "Max Near LODs\n\nHow many nearby scenes in view, within 3 parcels and kept until they are 4 away, may show their own models while they load. The nearest scenes come first; the rest wait for a place. Scenes behind you keep theirs without counting, up to twice this number in all. Lower it to save memory; 0 turns near LODs off.".to_string()
    }

    fn save(&self, config: &mut AppConfig) {
        config.max_near_lods = self.0 as usize;
    }

    fn load(config: &AppConfig) -> Self {
        Self(config.max_near_lods as i32)
    }

    fn category() -> super::SettingCategory {
        super::SettingCategory::Performance
    }
}
