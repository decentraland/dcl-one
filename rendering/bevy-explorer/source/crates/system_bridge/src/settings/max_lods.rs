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
        "Max Near LODs\n\nHow many loading scenes nearby may draw their own models, the nearest in view first; the rest show their far LOD. When all are taken, a scene behind you gives its place to one in view. Lower it to save memory; 0 draws none. Every loading scene within 3 parcels still blocks you with its own colliders.".to_string()
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

#[derive(Debug, PartialEq, Eq)]
pub struct MaxFarLodsSetting(i32);

impl IntAppSetting for MaxFarLodsSetting {
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
        600
    }
}

impl AppSetting for MaxFarLodsSetting {
    type Param = ();

    fn title() -> String {
        "Max Far LODs".to_owned()
    }

    fn description(&self) -> String {
        "Max Far LODs\n\nHow many distant scenes may show a simplified mesh before they load, counting those in view and those behind you: from 5 parcels away (or nearer, for a scene whose detailed model has no place) out to where the distant tiles grow to 4 parcels a side (about 12 to 18 parcels at the default distances), and never past 20. The nearest scenes come first; the rest show their distant tiles. When all are taken, a scene behind you gives its place to one in view. Lower it to save memory; 0 turns far LODs off.".to_string()
    }

    fn save(&self, config: &mut AppConfig) {
        config.max_far_lods = self.0 as usize;
    }

    fn load(config: &AppConfig) -> Self {
        Self(config.max_far_lods as i32)
    }

    fn category() -> super::SettingCategory {
        super::SettingCategory::Performance
    }
}
