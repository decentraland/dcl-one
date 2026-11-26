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
        "Max Near LODs\n\nAdvanced: the Graphics Preset sets this with the other Scene LODs settings; changing it shows the preset as Custom.\n\nHow many loading scenes nearby may draw their own models, within the Near LOD Ring, the nearest in view first; the rest show their far LOD. When all are taken, a scene behind you gives its place to one in view. Lower it to save memory; 0 draws none. Every loading scene within the Near LOD Ring still blocks you with its own colliders.".to_string()
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
        "Max Far LODs\n\nAdvanced: the Graphics Preset sets this with the other Scene LODs settings; changing it shows the preset as Custom.\n\nHow many distant scenes may show a simplified mesh before they load, counting those in view and those behind you: past the Near LOD Ring (or nearer, for a scene whose detailed model has no place), out to where the distant tiles grow to 4 parcels a side (about 13 to 18 parcels at the default distances) and never past the Far LOD Ring. The nearest in view come first; the rest show their distant tiles. When all are taken, a scene behind you gives its place to one in view. Lower it to save memory; 0 turns far LODs off. While Distant Scene Rendering is off, far LODs are off and this value is kept for when it is turned on.".to_string()
    }

    fn save(&self, config: &mut AppConfig) {
        config.max_far_lods = self.0 as usize;
    }

    fn load(config: &AppConfig) -> Self {
        Self(config.max_far_lods as i32)
    }

    fn is_disabled(config: &AppConfig) -> bool {
        config.scene_imposter_distances.is_empty()
    }

    fn category() -> super::SettingCategory {
        super::SettingCategory::Performance
    }
}
