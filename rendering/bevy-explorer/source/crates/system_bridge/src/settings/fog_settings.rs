use bevy::prelude::*;
use common::structs::{AppConfig, FogSetting};

use super::{AppSetting, EnumAppSetting};

impl EnumAppSetting for FogSetting {
    fn variants() -> Vec<Self> {
        vec![Self::Off, Self::On]
    }

    fn name(&self) -> String {
        match self {
            FogSetting::Off => "Off",
            FogSetting::On => "On",
        }
        .to_owned()
    }
}

impl AppSetting for FogSetting {
    type Param = ();

    fn title() -> String {
        "Fog".to_owned()
    }

    fn description(&self) -> String {
        format!("Rendering of distant objects. No performance impact, just an aesthetic preference.\n\n{}", 
        match self {
            FogSetting::Off => "Off: No fog.",
            FogSetting::On => "On: Distant objects fade into an exponential fog whose colour follows the time of day, as in Unity.",
        })
    }

    fn save(&self, config: &mut AppConfig) {
        config.graphics.fog = *self;
    }

    fn load(config: &AppConfig) -> Self {
        config.graphics.fog
    }

    fn category() -> super::SettingCategory {
        super::SettingCategory::Graphics
    }
}
