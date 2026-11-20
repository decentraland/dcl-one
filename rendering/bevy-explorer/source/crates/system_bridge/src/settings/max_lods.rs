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
        "Max Near LODs\n\nHow many loading scenes nearby may draw their own models, the nearest in view first. When all are taken, a scene behind you gives its place to one in view. Lower it to save memory; 0 draws none. Every loading scene within 3 parcels still blocks you with its own colliders.".to_string()
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
