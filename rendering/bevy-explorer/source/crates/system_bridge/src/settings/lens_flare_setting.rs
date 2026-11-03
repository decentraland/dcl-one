use bevy::{
    ecs::system::{lifetimeless::SRes, SystemParamItem},
    prelude::*,
};
use common::structs::{AppConfig, LensFlare, PrimaryCameraRes};

use super::{AppSetting, EnumAppSetting};

#[derive(Debug, PartialEq, Eq)]
pub enum LensFlareSetting {
    Off,
    On,
}

impl EnumAppSetting for LensFlareSetting {
    fn variants() -> Vec<Self> {
        vec![Self::Off, Self::On]
    }

    fn name(&self) -> String {
        match self {
            LensFlareSetting::Off => "Off",
            LensFlareSetting::On => "On",
        }
        .to_owned()
    }
}

impl AppSetting for LensFlareSetting {
    type Param = (SRes<AppConfig>, SRes<PrimaryCameraRes>);

    fn title() -> String {
        "Lens Flare".to_owned()
    }

    fn description(&self) -> String {
        "Lens flare when looking towards the sun or the moon.\n\nOn: glows and ghost shapes are drawn across the screen while the sun or the moon is in view and not hidden behind anything. Off: no flare is drawn, and its full-screen pass is skipped.".to_owned()
    }

    fn category() -> super::SettingCategory {
        super::SettingCategory::Graphics
    }

    fn save(&self, config: &mut AppConfig) {
        config.graphics.lens_flare = matches!(self, LensFlareSetting::On);
    }

    fn load(config: &AppConfig) -> Self {
        if config.graphics.lens_flare {
            Self::On
        } else {
            Self::Off
        }
    }

    fn apply_to_camera(
        &self,
        (_, primary): &SystemParamItem<Self::Param>,
        mut commands: Commands,
        camera_entity: Entity,
    ) {
        // only the player's camera flares: scene texture cameras do not flare in Unity
        if camera_entity != primary.0 {
            return;
        }
        let Ok(mut cmds) = commands.get_entity(camera_entity) else {
            return;
        };
        match self {
            LensFlareSetting::Off => {
                cmds.remove::<LensFlare>();
            }
            LensFlareSetting::On => {
                cmds.try_insert(LensFlare);
            }
        }
    }
}
