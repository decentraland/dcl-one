use super::SettingCategory;
use bevy::{
    core_pipeline::dof::{DepthOfField, DepthOfFieldMode},
    ecs::system::{lifetimeless::SRes, SystemParamItem},
    prelude::*,
};
use common::structs::{AppConfig, CameraLook, DofConfig, DofSetting};

use super::{AppSetting, EnumAppSetting};

impl EnumAppSetting for DofSetting {
    fn variants() -> Vec<Self> {
        vec![Self::Off, Self::Low, Self::High]
    }

    fn name(&self) -> String {
        match self {
            DofSetting::Off => "Off",
            DofSetting::Low => "Low",
            DofSetting::High => "High",
        }
        .to_owned()
    }
}

impl AppSetting for DofSetting {
    type Param = SRes<AppConfig>;

    fn title() -> String {
        "Depth of Field".to_owned()
    }

    fn category() -> SettingCategory {
        SettingCategory::Graphics
    }

    fn description(&self) -> String {
        format!("Depth of field blurs what is far from the focus, like a camera lens. Not available with MSAA, which turns it off.\n\n{}",
        match self {
            DofSetting::Off => "Off: No depth of field.",
            DofSetting::Low => "Low: A bit of depth of field is applied.",
            DofSetting::High => "High: Lots of depth of field is applied.",
        })
    }

    fn save(&self, config: &mut AppConfig) {
        config.graphics.dof = *self;
    }

    fn load(config: &AppConfig) -> Self {
        config.graphics.dof
    }

    /// MSAA turns depth of field off (`apply_to_camera`).
    fn is_disabled(config: &AppConfig) -> bool {
        config.graphics.msaa.is_msaa()
    }

    fn apply_to_camera(
        &self,
        config: &SystemParamItem<Self::Param>,
        mut commands: Commands,
        camera_entity: Entity,
    ) {
        let Ok(mut cmds) = commands.get_entity(camera_entity) else {
            return;
        };

        // the fork's transparent focus texture is single-sampled while the transparent pass it
        // joins is multisampled, which wgpu refuses (`prepare_transparent_focus_textures` takes
        // the sample count of the resolved main texture). so depth of field yields to MSAA, and no
        // graphics preset combines them. without it the stars draw before the transparent pass
        // (`visuals::stars`)
        if config.graphics.msaa.is_msaa() {
            if *self != DofSetting::Off {
                warn_once!("depth of field is off while MSAA is on");
            }
            cmds.remove::<DofConfig>().queue_silenced(set_dof(None));
            return;
        }

        let (mode, aperture_f_stops) = match self {
            // no blur, but depth of field stays active so its transparent focus texture is still
            // rendered: the stars read it to hide behind imposters
            DofSetting::Off => (DepthOfFieldMode::Disabled, 0.15),
            DofSetting::Low => (DepthOfFieldMode::Gaussian, 0.15),
            DofSetting::High => (DepthOfFieldMode::Gaussian, 0.05),
        };
        cmds.try_insert(DofConfig {
            default_sensor_height: 0.06,
            extra_focal_distance: 50.0,
        })
        .queue_silenced(set_dof(Some(DepthOfField {
            mode,
            focal_distance: 50.0, // updated based on cam + extra
            aperture_f_stops,
            max_depth: 250.0,
            max_circle_of_confusion_diameter: 20.0,
            sensor_height: 0.06,
            linear_ramp: 0.0,
        })));
    }
}

/// Sets the depth of field of a camera's own look.
fn set_dof(dof: Option<DepthOfField>) -> impl FnOnce(EntityWorldMut) {
    move |mut camera: EntityWorldMut| {
        if let Some(mut look) = camera.get_mut::<CameraLook>() {
            look.dof = dof;
        }
    }
}
