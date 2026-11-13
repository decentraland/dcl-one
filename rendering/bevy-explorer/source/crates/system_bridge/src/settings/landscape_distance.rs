use bevy::{ecs::system::lifetimeless::SResMut, platform::collections::HashSet, prelude::*};
use common::structs::{AppConfig, LandscapeDistance};

use super::{AppSetting, IntAppSetting, SettingCategory};

#[derive(Debug, PartialEq, Eq)]
pub struct LandscapeDistanceSetting(i32);

impl IntAppSetting for LandscapeDistanceSetting {
    fn from_int(value: i32) -> Self {
        Self(value.clamp(Self::min(), Self::max()))
    }

    fn value(&self) -> i32 {
        self.0
    }

    fn min() -> i32 {
        500
    }

    fn max() -> i32 {
        7000
    }
}

impl AppSetting for LandscapeDistanceSetting {
    type Param = SResMut<LandscapeDistance>;

    fn title() -> String {
        "Landscape Distance".to_owned()
    }

    fn description(&self) -> String {
        "How far away trees, foliage and other landscape details remain visible.".to_owned()
    }

    fn category() -> SettingCategory {
        SettingCategory::Graphics
    }

    fn save(&self, config: &mut AppConfig) {
        config.landscape_distance = self.0 as f32;
    }

    fn load(config: &AppConfig) -> Self {
        Self::from_int(config.landscape_distance.round() as i32)
    }

    fn apply(&self, mut distance: ResMut<LandscapeDistance>, _: Commands, _: &HashSet<Entity>) {
        distance.0 = self.0 as f32;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::Settings;

    #[test]
    fn setting_range_is_500_to_7000() {
        assert_eq!(LandscapeDistanceSetting::min(), 500);
        assert_eq!(LandscapeDistanceSetting::max(), 7000);
        let mut settings = Settings {
            settings: Vec::new(),
        };
        settings.add_int_setting::<LandscapeDistanceSetting>(&AppConfig::default());
        let info = &settings.settings[0].info;
        assert_eq!(info.name, "Landscape Distance");
        assert_eq!(info.category, "Graphics");
        assert_eq!((info.min_value, info.max_value), (500.0, 7000.0));
        assert_eq!((info.default, info.value), (1000.0, 1000.0));

        // values outside the slider land on its ends, whether set or saved by hand
        let mut config = AppConfig::default();
        settings
            .set_value(&mut config, "Landscape Distance", 9000.0)
            .unwrap();
        assert_eq!(config.landscape_distance, 7000.0);
        settings
            .set_value(&mut config, "Landscape Distance", 100.0)
            .unwrap();
        assert_eq!(config.landscape_distance, 500.0);
        config.landscape_distance = 20000.0;
        assert_eq!(LandscapeDistanceSetting::load(&config).value(), 7000);
    }
}
