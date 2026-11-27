use std::ops::RangeInclusive;

use bevy::ecs::system::lifetimeless::{SRes, SResMut};
use bevy::prelude::*;
use common::structs::{
    AppConfig, SceneLoadDistance, SceneRing, FAR_LOD_RINGS, IMPOSTER_RINGS, LIVE_SCENE_RINGS,
    NEAR_LOD_RINGS,
};

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
        "Live Scene Ring\n\nHow many parcels around yours run their scenes in full, scripts included: 1 is your parcel and the eight around it. A scene keeps running until it is one parcel further. The Graphics Preset sets it; a larger value runs more scenes at once and uses more memory and CPU. Moving it moves the other rings along where they would cross.".to_string()
    }

    fn save(&self, config: &mut AppConfig) {
        config.set_scene_ring(SceneRing::Live, clamp_ring(self.0, LIVE_SCENE_RINGS) as u16);
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
        "Near LOD Ring\n\nHow many parcels away scenes that are not running show their own models, without scripts, and block you with their colliders. They are kept one parcel further. A wider ring draws more of them at once (6, 12 and 20 at 2, 3 and 4), the nearest in view first. The Graphics Preset sets it. Moving it moves the other rings along where they would cross.".to_string()
    }

    fn save(&self, config: &mut AppConfig) {
        config.set_scene_ring(SceneRing::Near, clamp_ring(self.0, NEAR_LOD_RINGS) as u16);
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
        "Far LOD Ring\n\nHow many parcels away distant scenes show a simplified mesh. The imposter tiles can end them sooner: about 13 to 18 parcels away on Normal, about 25 to 30 on Ultra. A wider ring shows more of them at once (100, 200 and 400 at 12, 20 and 30), the nearest in view first. The Graphics Preset sets it. While Distant Scene Rendering is off there are no far LODs and this value is kept. Moving it moves the other rings along where they would cross.".to_string()
    }

    fn save(&self, config: &mut AppConfig) {
        config.set_scene_ring(SceneRing::Far, clamp_ring(self.0, FAR_LOD_RINGS) as u16);
    }

    fn load(config: &AppConfig) -> Self {
        Self(clamp_ring(config.far_lod_ring as i32, FAR_LOD_RINGS))
    }

    fn is_disabled(config: &AppConfig) -> bool {
        config.distant_scenes_off()
    }

    fn category() -> super::SettingCategory {
        super::SettingCategory::Performance
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct ImposterRingSetting(i32);

impl IntAppSetting for ImposterRingSetting {
    fn from_int(value: i32) -> Self {
        Self(value)
    }

    fn value(&self) -> i32 {
        self.0
    }

    fn min() -> i32 {
        ring_min(IMPOSTER_RINGS)
    }

    fn max() -> i32 {
        ring_max(IMPOSTER_RINGS)
    }
}

impl AppSetting for ImposterRingSetting {
    type Param = (SResMut<SceneLoadDistance>, SRes<AppConfig>);

    fn title() -> String {
        "Imposter Ring".to_owned()
    }

    fn description(&self) -> String {
        "Imposter Ring\n\nHow many parcels away the imposter tiles (flat pictures of the scenes past the far LODs) are drawn. The highest value has no limit: they reach the horizon. The Graphics Preset sets it. While Distant Scene Rendering is off there are no imposters and this value is kept. Moving it moves the other rings along where they would cross.".to_string()
    }

    fn save(&self, config: &mut AppConfig) {
        config.set_scene_ring(
            SceneRing::Imposter,
            clamp_ring(self.0, IMPOSTER_RINGS) as u16,
        );
    }

    fn load(config: &AppConfig) -> Self {
        Self(clamp_ring(config.imposter_ring as i32, IMPOSTER_RINGS))
    }

    fn is_disabled(config: &AppConfig) -> bool {
        config.distant_scenes_off()
    }

    fn apply(
        &self,
        (mut distance, config): bevy::ecs::system::SystemParamItem<Self::Param>,
        _: Commands,
        _: &bevy::platform::collections::HashSet<Entity>,
    ) {
        distance.load_imposter = config.imposter_range();
    }

    fn category() -> super::SettingCategory {
        super::SettingCategory::Performance
    }
}
