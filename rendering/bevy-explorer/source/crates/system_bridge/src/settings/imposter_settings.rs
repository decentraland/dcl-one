use super::SettingCategory;
use bevy::{ecs::system::lifetimeless::SResMut, prelude::*};
use common::structs::{AppConfig, SceneLoadDistance};

use super::{AppSetting, EnumAppSetting};

#[derive(Debug, PartialEq, Eq)]
pub enum ImposterSetting {
    Off,
    Low,
    High,
}

impl EnumAppSetting for ImposterSetting {
    fn variants() -> Vec<Self> {
        vec![Self::Off, Self::Low, Self::High]
    }

    fn name(&self) -> String {
        match self {
            ImposterSetting::Off => "Off",
            ImposterSetting::Low => "Normal",
            ImposterSetting::High => "Ultra",
        }
        .to_owned()
    }
}

impl AppSetting for ImposterSetting {
    type Param = SResMut<SceneLoadDistance>;

    fn title() -> String {
        "Distant Scene Rendering".to_owned()
    }

    fn category() -> SettingCategory {
        SettingCategory::Graphics
    }

    fn description(&self) -> String {
        format!("Distant Scene Rendering\n\nWhat you see past the scenes around you: far LODs (simplified meshes of distant scenes) and, beyond them, low quality imposter tiles. The Graphics Preset sets it; the Performance tab's Scene LODs settings tune it further.\n\n{}",
        match self {
            ImposterSetting::Off => "Off: No far LODs and no imposters; only the scenes around you and their near LODs are drawn. Max Far LODs and Far LOD Ring keep their values for when it is turned on.",
            ImposterSetting::Low => "Normal: Far LODs, then imposters at normal quality.",
            ImposterSetting::High => "Ultra: Far LODs, then imposters at higher quality, which uses more video memory.",
        })
    }

    fn save(&self, config: &mut AppConfig) {
        config.scene_imposter_distances = match self {
            ImposterSetting::Off => vec![],
            ImposterSetting::Low => vec![100.0, 200.0, 400.0, 800.0, 1600.0, 99999.0],
            ImposterSetting::High => vec![200.0, 400.0, 800.0, 1600.0, 99999.0],
        };
        config.scene_imposter_multisample = false;
        config.scene_imposter_multisample_amount = 0.0;
    }

    fn load(config: &AppConfig) -> Self {
        match config.scene_imposter_distances.first() {
            None => Self::Off,
            Some(200.0) => Self::High,
            _ => Self::Low,
        }
    }

    fn apply(
        &self,
        mut param: bevy::ecs::system::SystemParamItem<Self::Param>,
        _commands: Commands,
        _cameras: &bevy::platform::collections::HashSet<Entity>,
    ) {
        let max_distance = match self {
            ImposterSetting::Off => 0.0,
            _ => 99999.0,
        };

        param.load_imposter = max_distance;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::{max_lods::MaxFarLodsSetting, rings::FarLodRingSetting, AppSetting as _};

    #[test]
    fn off_turns_far_lods_off_and_keeps_the_far_settings() {
        let mut config = AppConfig::default();
        assert_eq!(ImposterSetting::load(&config), ImposterSetting::Low);
        assert!(!MaxFarLodsSetting::is_disabled(&config));
        assert!(!FarLodRingSetting::is_disabled(&config));

        ImposterSetting::Off.save(&mut config);
        assert_eq!(ImposterSetting::load(&config), ImposterSetting::Off);
        assert_eq!(config.far_lods_cap(), 0);
        assert!(MaxFarLodsSetting::is_disabled(&config));
        assert!(FarLodRingSetting::is_disabled(&config));
        let default = AppConfig::default();
        assert_eq!(config.max_far_lods, default.max_far_lods);
        assert_eq!(config.far_lod_ring, default.far_lod_ring);

        for setting in [ImposterSetting::Low, ImposterSetting::High] {
            setting.save(&mut config);
            assert_eq!(ImposterSetting::load(&config), setting);
            assert_eq!(config.far_lods_cap(), default.max_far_lods);
            assert!(!MaxFarLodsSetting::is_disabled(&config));
        }
    }
}
