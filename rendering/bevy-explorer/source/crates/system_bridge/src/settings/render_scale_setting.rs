use common::structs::{AppConfig, GraphicsSettings};

use super::{AppSetting, IntAppSetting, SettingCategory};

/// Render resolution in steps of the window's width and height
/// ([`GraphicsSettings::RENDER_SCALE_STEPS`] per whole window).
#[derive(Debug, PartialEq, Eq)]
pub struct RenderScaleSetting(i32);

impl IntAppSetting for RenderScaleSetting {
    fn from_int(value: i32) -> Self {
        Self(value)
    }

    fn value(&self) -> i32 {
        self.0
    }

    fn min() -> i32 {
        GraphicsSettings::MIN_PLAYER_RENDER_SCALE_STEPS
    }

    fn max() -> i32 {
        GraphicsSettings::RENDER_SCALE_STEPS
    }

    fn scale() -> f32 {
        100.0 / GraphicsSettings::RENDER_SCALE_STEPS as f32
    }

    fn display(&self) -> String {
        format!("{}%", self.0 as f32 * Self::scale())
    }
}

impl AppSetting for RenderScaleSetting {
    type Param = ();

    fn title() -> String {
        "Render Resolution".to_owned()
    }

    fn category() -> SettingCategory {
        SettingCategory::Graphics
    }

    fn description(&self) -> String {
        "Render Resolution\n\nShare of the window's pixels the world is drawn at, upscaled with FSR to fill the window. Lower values reduce GPU load, most on high-density screens, and make the 3D view softer; the interface stays sharp.".to_owned()
    }

    fn save(&self, config: &mut AppConfig) {
        config.graphics.set_render_scale_steps(self.0);
    }

    fn load(config: &AppConfig) -> Self {
        Self(config.graphics.render_scale_steps())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::Settings;

    fn render_resolution(settings: &Settings) -> &system_api_types::SettingInfo {
        &settings.settings[0].info
    }

    #[test]
    fn moves_in_ten_percent_steps() {
        let mut settings = Settings { settings: vec![] };
        let mut config = AppConfig::default();
        settings.add_int_setting::<RenderScaleSetting>(&config);
        let info = render_resolution(&settings);
        assert_eq!(
            (info.min_value, info.max_value, info.step_size, info.value),
            (50.0, 100.0, 10.0, 100.0)
        );

        let name = RenderScaleSetting::title();
        for (requested, shown, saved) in [(90.0, 90.0, 0.9), (73.0, 70.0, 0.7), (100.0, 100.0, 1.0)]
        {
            settings.set_value(&mut config, &name, requested).unwrap();
            assert_eq!(render_resolution(&settings).value, shown);
            assert_eq!(config.graphics.render_scale, saved);
        }
    }
}
