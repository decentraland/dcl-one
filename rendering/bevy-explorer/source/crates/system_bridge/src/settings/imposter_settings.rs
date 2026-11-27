use super::SettingCategory;
use bevy::{
    ecs::system::lifetimeless::{SRes, SResMut},
    prelude::*,
};
use common::structs::{
    AppConfig, SceneLoadDistance, IMPOSTER_DISTANCES_NORMAL, IMPOSTER_DISTANCES_ULTRA,
};

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
    type Param = (SResMut<SceneLoadDistance>, SRes<AppConfig>);

    fn title() -> String {
        "Distant Scene Rendering".to_owned()
    }

    fn category() -> SettingCategory {
        SettingCategory::Graphics
    }

    fn description(&self) -> String {
        format!("Distant Scene Rendering\n\nWhat you see past the scenes around you: far LODs (simplified meshes of distant scenes) and, beyond them, low quality imposter tiles. The Graphics Preset sets it; the Performance tab's Scene Loading rings tune it further.\n\n{}",
        match self {
            ImposterSetting::Off => "Off: No far LODs and no imposters; only the scenes around you and their near LODs are drawn. Far LOD Ring and Imposter Ring keep their values for when it is turned on.",
            ImposterSetting::Low => "Normal: Far LODs, then imposters at normal quality.",
            ImposterSetting::High => "Ultra: Far LODs, then imposters at higher quality, which uses more video memory.",
        })
    }

    fn save(&self, config: &mut AppConfig) {
        config.set_imposters(match self {
            ImposterSetting::Off => &[],
            ImposterSetting::Low => &IMPOSTER_DISTANCES_NORMAL,
            ImposterSetting::High => &IMPOSTER_DISTANCES_ULTRA,
        });
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
        (mut distance, config): bevy::ecs::system::SystemParamItem<Self::Param>,
        _commands: Commands,
        _cameras: &bevy::platform::collections::HashSet<Entity>,
    ) {
        // the Imposter Ring writes it too, from the same distances
        distance.load_imposter = config.imposter_range();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::{
        rings::{FarLodRingSetting, ImposterRingSetting},
        IntAppSetting,
    };

    #[test]
    fn off_turns_far_lods_off_and_keeps_the_rings() {
        let mut config = AppConfig::default();
        assert_eq!(ImposterSetting::load(&config), ImposterSetting::Low);
        assert!(!FarLodRingSetting::is_disabled(&config));
        assert!(!ImposterRingSetting::is_disabled(&config));

        ImposterRingSetting::from_int(50).save(&mut config);
        ImposterSetting::Off.save(&mut config);
        assert_eq!(ImposterSetting::load(&config), ImposterSetting::Off);
        assert_eq!(config.far_lods_cap(), 0);
        assert_eq!(config.imposter_range(), 0.0);
        assert!(FarLodRingSetting::is_disabled(&config));
        assert!(ImposterRingSetting::is_disabled(&config));
        assert_eq!(config.far_lod_ring, AppConfig::default().far_lod_ring);
        assert_eq!(config.imposter_ring, 50);

        for setting in [ImposterSetting::Low, ImposterSetting::High] {
            setting.save(&mut config);
            assert_eq!(ImposterSetting::load(&config), setting);
            assert_eq!(config.far_lods_cap(), 200);
            // the ring still ends the tiles, at 50 parcels
            assert_eq!(config.scene_imposter_distances.last(), Some(&800.0));
            assert!(!FarLodRingSetting::is_disabled(&config));
        }
    }

    #[test]
    fn the_imposter_ring_keeps_the_level() {
        let mut config = AppConfig::default();
        ImposterSetting::High.save(&mut config);
        for ring in [ImposterRingSetting::min(), 50, ImposterRingSetting::max()] {
            ImposterRingSetting::from_int(ring).save(&mut config);
            assert_eq!(ImposterSetting::load(&config), ImposterSetting::High);
        }
        assert_eq!(config.scene_imposter_distances, IMPOSTER_DISTANCES_ULTRA);

        // while off, the ring is kept and no tiles come back
        ImposterSetting::Off.save(&mut config);
        ImposterRingSetting::from_int(30).save(&mut config);
        assert!(config.distant_scenes_off());
        assert_eq!(config.imposter_ring, 30);
    }
}
