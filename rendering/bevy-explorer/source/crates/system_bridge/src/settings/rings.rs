use std::ops::RangeInclusive;

use bevy::prelude::*;
use common::structs::{AppConfig, FAR_LOD_RINGS, LIVE_SCENE_RINGS, NEAR_LOD_RINGS};

use super::{AppSetting, IntAppSetting};

fn ring_min(range: RangeInclusive<u16>) -> i32 {
    *range.start() as i32
}

fn ring_max(range: RangeInclusive<u16>) -> i32 {
    *range.end() as i32
}

/// A value clamped to its slider's range, so a hand-edited config or a `SetSetting` outside it
/// shows and saves what the engine uses (`Rings::from_config` clamps the same way).
fn clamp_ring(value: i32, range: RangeInclusive<u16>) -> i32 {
    value.clamp(ring_min(range.clone()), ring_max(range))
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct LiveSceneRingSetting(i32);

impl IntAppSetting for LiveSceneRingSetting {
    fn from_int(value: i32) -> Self {
        Self(value)
    }

    fn value(&self) -> i32 {
        self.0
    }

    fn min() -> i32 {
        ring_min(LIVE_SCENE_RINGS)
    }

    fn max() -> i32 {
        ring_max(LIVE_SCENE_RINGS)
    }
}

impl AppSetting for LiveSceneRingSetting {
    type Param = ();

    fn title() -> String {
        "Live Scene Ring".to_owned()
    }

    fn description(&self) -> String {
        "Live Scene Ring\n\nAdvanced: the Graphics Preset sets this with the other Scene LODs settings; changing it shows the preset as Custom.\n\nScenes this many parcels from yours always run, whatever the Scene Load Distance, and keep running until they are one parcel further. At 0 only the Scene Load Distance decides which nearby scenes run; a larger value starts nearby scenes earlier and uses more memory.".to_string()
    }

    fn save(&self, config: &mut AppConfig) {
        config.live_scene_ring = clamp_ring(self.0, LIVE_SCENE_RINGS) as u16;
    }

    fn load(config: &AppConfig) -> Self {
        Self(clamp_ring(config.live_scene_ring as i32, LIVE_SCENE_RINGS))
    }

    fn category() -> super::SettingCategory {
        super::SettingCategory::Performance
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct NearLodRingSetting(i32);

impl IntAppSetting for NearLodRingSetting {
    fn from_int(value: i32) -> Self {
        Self(value)
    }

    fn value(&self) -> i32 {
        self.0
    }

    fn min() -> i32 {
        ring_min(NEAR_LOD_RINGS)
    }

    fn max() -> i32 {
        ring_max(NEAR_LOD_RINGS)
    }
}

impl AppSetting for NearLodRingSetting {
    type Param = ();

    fn title() -> String {
        "Near LOD Ring".to_owned()
    }

    fn description(&self) -> String {
        "Near LOD Ring\n\nAdvanced: the Graphics Preset sets this with the other Scene LODs settings; changing it shows the preset as Custom.\n\nScenes up to this many parcels away that are not running show their own models, without scripts, until one parcel further. Far LODs start past that. At or inside the Live Scene Ring, only scenes still loading show them.".to_string()
    }

    fn save(&self, config: &mut AppConfig) {
        config.near_lod_ring = clamp_ring(self.0, NEAR_LOD_RINGS) as u16;
    }

    fn load(config: &AppConfig) -> Self {
        Self(clamp_ring(config.near_lod_ring as i32, NEAR_LOD_RINGS))
    }

    fn category() -> super::SettingCategory {
        super::SettingCategory::Performance
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct FarLodRingSetting(i32);

impl IntAppSetting for FarLodRingSetting {
    fn from_int(value: i32) -> Self {
        Self(value)
    }

    fn value(&self) -> i32 {
        self.0
    }

    fn min() -> i32 {
        ring_min(FAR_LOD_RINGS)
    }

    fn max() -> i32 {
        ring_max(FAR_LOD_RINGS)
    }
}

impl AppSetting for FarLodRingSetting {
    type Param = ();

    fn title() -> String {
        "Far LOD Ring".to_owned()
    }

    fn description(&self) -> String {
        "Far LOD Ring\n\nAdvanced: the Graphics Preset sets this with the other Scene LODs settings; changing it shows the preset as Custom.\n\nHow many parcels away distant scenes may show a simplified mesh before they load. Distant Scene Rendering can end them sooner: about 13 to 18 parcels away at its default distances, about 25 to 30 on Ultra. Beyond, only the distant scene tiles remain. While Distant Scene Rendering is off, far LODs are off and this value is kept for when it is turned on.".to_string()
    }

    fn save(&self, config: &mut AppConfig) {
        config.far_lod_ring = clamp_ring(self.0, FAR_LOD_RINGS) as u16;
    }

    fn load(config: &AppConfig) -> Self {
        Self(clamp_ring(config.far_lod_ring as i32, FAR_LOD_RINGS))
    }

    fn is_disabled(config: &AppConfig) -> bool {
        config.scene_imposter_distances.is_empty()
    }

    fn category() -> super::SettingCategory {
        super::SettingCategory::Performance
    }
}
