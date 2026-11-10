use common::structs::{AppConfig, WindowSetting};

use super::{AppSetting, EnumAppSetting};

impl EnumAppSetting for WindowSetting {
    fn variants() -> Vec<Self> {
        vec![
            Self::Windowed,
            #[cfg(not(target_arch = "wasm32"))]
            Self::Fullscreen,
            Self::Borderless,
        ]
    }

    fn name(&self) -> String {
        match self {
            #[cfg(not(target_arch = "wasm32"))]
            WindowSetting::Fullscreen => "Fullscreen",
            WindowSetting::Windowed => "Window",
            WindowSetting::Borderless => "Borderless Fullscreen",
        }
        .to_owned()
    }
}

impl AppSetting for WindowSetting {
    // applied with the fullscreen size by `resolution_setting`
    type Param = ();

    fn title() -> String {
        "Window mode".to_owned()
    }

    fn description(&self) -> String {
        format!("Window mode.\n\n{}", 
            match self {
                #[cfg(not(target_arch = "wasm32"))]
                WindowSetting::Fullscreen => "Fullscreen: Native fullscreen mode, at the size chosen in Resolution.",
                WindowSetting::Windowed => "Windowed: Not fullscreen.",
                WindowSetting::Borderless => "Borderless Fullscreen: Use a fullscreen window at the monitor's own resolution; the Resolution setting has no effect.",
            }
        )
    }

    fn save(&self, config: &mut AppConfig) {
        config.graphics.window = *self;
    }

    fn load(config: &AppConfig) -> Self {
        config.graphics.window
    }

    fn category() -> super::SettingCategory {
        super::SettingCategory::Graphics
    }
}
