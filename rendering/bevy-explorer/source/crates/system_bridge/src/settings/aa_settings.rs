use bevy::{
    core_pipeline::fxaa::{Fxaa, Sensitivity},
    ecs::system::{lifetimeless::SRes, SystemParamItem},
    prelude::*,
    render::{render_resource::TextureFormat, renderer::RenderAdapter},
};
use common::structs::{AaSetting, AppConfig};

use super::{AppSetting, EnumAppSetting, SettingCategory};

impl EnumAppSetting for AaSetting {
    fn variants() -> Vec<Self> {
        vec![
            Self::Off,
            Self::FxaaLow,
            Self::FxaaHigh,
            #[cfg(not(target_arch = "wasm32"))]
            Self::Msaa2x,
            #[cfg(not(target_arch = "wasm32"))]
            Self::Msaa4x,
            #[cfg(not(target_arch = "wasm32"))]
            Self::Msaa8x,
        ]
    }

    fn name(&self) -> String {
        match self {
            AaSetting::Off => "Off",
            AaSetting::FxaaLow => "FXAA (Low)",
            AaSetting::FxaaHigh => "FXAA (High)",
            AaSetting::Msaa2x => "MSAA 2x",
            AaSetting::Msaa4x => "MSAA 4x",
            AaSetting::Msaa8x => "MSAA 8x",
        }
        .to_owned()
    }
}

/// The highest MSAA sample count up to `requested` that `supported` accepts, falling back from 8
/// to 4, 2 and 1.
fn clamp_msaa(requested: u32, supported: impl Fn(u32) -> bool) -> u32 {
    [8, 4, 2]
        .into_iter()
        .filter(|&samples| samples <= requested)
        .find(|&samples| supported(samples))
        .unwrap_or(1)
}

impl AppSetting for AaSetting {
    // absent until the renderer is up
    type Param = Option<SRes<RenderAdapter>>;

    fn title() -> String {
        "Anti-aliasing".to_owned()
    }

    fn category() -> SettingCategory {
        SettingCategory::Graphics
    }

    fn description(&self) -> String {
        format!("Aliasing reduction strategy. Alternatives for reducing jagged edges in the world.\n\n{}", 
            match self {
                AaSetting::Off => "Off\nNo aliasing is applied. Fastest, lowest quality.",
                AaSetting::FxaaLow => "Fast Approximate Anti-Aliasing\nA blur filter is applied over the entire image rather than just around the edges to give them a smoother look. This method has a lower GPU impact than MSAA, but is less effective and can result in a blurry image.",
                AaSetting::FxaaHigh => "Fast Approximate Anti-Aliasing\nA blur filter is applied over the entire image rather than just around the edges to give them a smoother look. This method has a lower GPU impact than MSAA, but is less effective and can result in a blurry image.",
                AaSetting::Msaa2x => "MSAA 2x\n2x Multisampling of pixels with mesh overlaps. 2x sampling gives a small quality boost for a small GPU cost.",
                AaSetting::Msaa4x => "MSAA 4x\n4x Multisampling of pixels with mesh overlaps. 4x sampling gives a good quality boost for a medium GPU cost.",
                AaSetting::Msaa8x => "MSAA 8x\n8x Multisampling of pixels with mesh overlaps. 8x sampling gives a high quality boost for a high GPU cost.",
            }
        )
    }

    fn save(&self, config: &mut AppConfig) {
        config.graphics.msaa = *self;
    }

    fn load(config: &AppConfig) -> Self {
        config.graphics.msaa
    }

    fn apply_to_camera(
        &self,
        adapter: &SystemParamItem<Self::Param>,
        mut commands: Commands,
        camera_entity: Entity,
    ) {
        let requested = match self {
            AaSetting::Off | AaSetting::FxaaLow | AaSetting::FxaaHigh => 1,
            AaSetting::Msaa2x => 2,
            AaSetting::Msaa4x => 4,
            AaSetting::Msaa8x => 8,
        };
        // unity: URP clamps the MSAA count silently; wgpu fails validation on a count the HDR
        // view target or the depth texture does not support (Apple GPUs stop at 4). The saved
        // setting keeps the requested count, so the HUD still shows the preset.
        let samples = match adapter {
            Some(adapter) => clamp_msaa(requested, |samples| {
                [TextureFormat::Rgba16Float, TextureFormat::Depth32Float]
                    .into_iter()
                    .all(|format| {
                        adapter
                            .get_texture_format_features(format)
                            .flags
                            .sample_count_supported(samples)
                    })
            }),
            None => requested,
        };
        if samples != requested {
            warn_once!("MSAA {requested}x is not supported by this GPU, using {samples}x");
        }
        let msaa = Msaa::from_samples(samples);

        commands
            .entity(camera_entity)
            .remove::<Fxaa>()
            .try_insert(msaa);
        if let Some(sensitivity) = match self {
            AaSetting::FxaaLow => Some(Sensitivity::Medium),
            AaSetting::FxaaHigh => Some(Sensitivity::Ultra),
            _ => None,
        } {
            commands.entity(camera_entity).try_insert(Fxaa {
                enabled: true,
                edge_threshold: sensitivity,
                edge_threshold_min: sensitivity,
            });
        }
    }
}

#[cfg(test)]
mod test {
    use super::clamp_msaa;

    #[test]
    fn msaa_clamps_to_the_highest_supported_count() {
        let apple = |n: u32| matches!(n, 1 | 2 | 4);
        assert_eq!(clamp_msaa(8, apple), 4);
        assert_eq!(clamp_msaa(4, apple), 4);
        assert_eq!(clamp_msaa(2, apple), 2);
        assert_eq!(clamp_msaa(1, apple), 1);
        assert_eq!(clamp_msaa(8, |n| n <= 8), 8);
        assert_eq!(clamp_msaa(8, |n| n == 1 || n == 2), 2);
        assert_eq!(clamp_msaa(8, |n| n == 1), 1);
    }
}
