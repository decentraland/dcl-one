use common::structs::AppConfig;

use super::{AppSetting, IntAppSetting, SettingCategory};

/// Render resolution in tenths of the window's width and height, so the slider moves 10% per step.
#[derive(Debug, PartialEq, Eq)]
pub struct RenderScaleSetting(i32);

impl IntAppSetting for RenderScaleSetting {
    fn from_int(value: i32) -> Self {
        Self(value)
    }

    fn value(&self) -> i32 {
        self.0
    }

    // Players stop at 50%; hosts may go lower for the session (SessionGraphics::MIN_RENDER_SCALE).
    fn min() -> i32 {
        5
    }

    fn max() -> i32 {
        10
    }

    fn scale() -> f32 {
        10.0
    }

    fn display(&self) -> String {
        format!("{}%", self.0 * 10)
    }
}

impl AppSetting for RenderScaleSetting {
    type Param = ();

    fn title() -> String {
        "Render Resolution".to_owned()
    }

    fn category() -> SettingCategory {
        SettingCategory::Performance
    }

    fn description(&self) -> String {
        "Render Resolution\n\nShare of the window's pixels the world is drawn at, upscaled with FSR to fill the window. Lower values reduce GPU load, most on high-density screens, and make the 3D view softer; the interface stays sharp.".to_owned()
    }

    fn save(&self, config: &mut AppConfig) {
        config.graphics.render_scale = self.0 as f32 / 10.0;
    }

    fn load(config: &AppConfig) -> Self {
        Self(((config.graphics.render_scale * 10.0).round() as i32).clamp(Self::min(), Self::max()))
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
