use bevy::{
    ecs::system::{
        lifetimeless::{SQuery, Write},
        SystemParamItem,
    },
    platform::collections::HashSet,
    prelude::*,
    window::{PresentMode, PrimaryWindow},
};
use common::structs::AppConfig;

use super::{AppSetting, EnumAppSetting, SettingCategory};

/// Native only: the browser paces the web build to the display.
#[derive(Debug, PartialEq, Eq)]
pub enum VsyncSetting {
    Off,
    On,
}

impl EnumAppSetting for VsyncSetting {
    fn variants() -> Vec<Self> {
        vec![Self::Off, Self::On]
    }

    fn name(&self) -> String {
        match self {
            VsyncSetting::Off => "Off",
            VsyncSetting::On => "On",
        }
        .to_owned()
    }
}

impl AppSetting for VsyncSetting {
    type Param = SQuery<Write<Window>, With<PrimaryWindow>>;

    fn title() -> String {
        "VSync".to_owned()
    }

    fn description(&self) -> String {
        "Sync frames to the monitor's refresh rate.\n\nOn: no tearing, and the frame rate follows the monitor; the Target Frame Rate is ignored. Off: frames show as soon as they are ready, at the Target Frame Rate.".to_owned()
    }

    fn category() -> SettingCategory {
        SettingCategory::Graphics
    }

    fn save(&self, config: &mut AppConfig) {
        config.graphics.vsync = matches!(self, VsyncSetting::On);
    }

    fn load(config: &AppConfig) -> Self {
        if config.graphics.vsync {
            Self::On
        } else {
            Self::Off
        }
    }

    fn apply(&self, mut window: SystemParamItem<Self::Param>, _: Commands, _: &HashSet<Entity>) {
        let Ok(mut window) = window.single_mut() else {
            return;
        };
        window.present_mode = match self {
            VsyncSetting::Off => PresentMode::AutoNoVsync,
            VsyncSetting::On => PresentMode::AutoVsync,
        };
    }
}
