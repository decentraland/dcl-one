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

use super::{AppSetting, EnumAppSetting};

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
        "VSync\n\nOn: frames are shown in step with the monitor's refresh, which stops screen tearing and runs the game at the monitor's refresh rate; Target Frame Rate is ignored. Off: frames are shown as soon as they are ready, at the Target Frame Rate.".to_owned()
    }

    fn category() -> super::SettingCategory {
        super::SettingCategory::Graphics
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

    // must match the `present_mode` the window is created with
    fn apply(&self, mut window: SystemParamItem<Self::Param>, _: Commands, _: &HashSet<Entity>) {
        let Ok(mut window) = window.single_mut() else {
            return;
        };
        let present_mode = match self {
            VsyncSetting::On => PresentMode::AutoVsync,
            VsyncSetting::Off => PresentMode::AutoNoVsync,
        };
        if window.present_mode != present_mode {
            window.present_mode = present_mode;
        }
    }
}
