use super::SettingCategory;
use bevy::{core_pipeline::bloom::Bloom, ecs::system::SystemParamItem, prelude::*};
use common::structs::{scatter_bloom, AppConfig, BloomSetting, StageCamera, BLOOM_SCATTER};

use super::{AppSetting, EnumAppSetting};

impl EnumAppSetting for BloomSetting {
    fn variants() -> Vec<Self> {
        vec![Self::Off, Self::On]
    }

    fn name(&self) -> String {
        match self {
            BloomSetting::Off => "Off",
            BloomSetting::On => "On",
        }
        .to_owned()
    }
}

impl AppSetting for BloomSetting {
    type Param = ();

    fn title() -> String {
        "Bloom".to_owned()
    }

    fn category() -> SettingCategory {
        SettingCategory::Graphics
    }

    fn description(&self) -> String {
        format!("Bloom is a post-processing effect used to reproduce an imaging artifact of real-world cameras. The effect produces fringes (or feathers) of light extending from the borders of bright areas in an image, contributing to the illusion of an extremely bright light overwhelming the camera capturing the scene.\n\n{}", 
        match self {
            BloomSetting::Off => "Off: No bloom.",
            BloomSetting::On => "On: Bright and emissive surfaces glow.",
        })
    }

    fn save(&self, config: &mut AppConfig) {
        config.graphics.bloom = *self;
    }

    fn load(config: &AppConfig) -> Self {
        config.graphics.bloom
    }

    fn apply_to_camera(
        &self,
        _: &SystemParamItem<Self::Param>,
        mut commands: Commands,
        camera_entity: Entity,
    ) {
        let Ok(mut cmds) = commands.get_entity(camera_entity) else {
            return;
        };

        let bloom = match self {
            BloomSetting::Off => None,
            BloomSetting::On => Some(scatter_bloom(BLOOM_SCATTER)),
        };
        // read when the command applies, so the stage's marker counts whichever system ran first
        cmds.queue_silenced(move |mut camera: EntityWorldMut| {
            if camera.contains::<StageCamera>() {
                return;
            }
            match bloom {
                Some(bloom) => {
                    camera.insert(bloom);
                }
                None => {
                    camera.remove::<Bloom>();
                }
            }
        });
    }
}
