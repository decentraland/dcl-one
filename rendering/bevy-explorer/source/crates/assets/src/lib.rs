use bevy::{asset::io::embedded::EmbeddedAssetRegistry, prelude::*};

include!(concat!(env!("OUT_DIR"), "/generated_asset_embedding.rs"));

pub struct EmbedAssetsPlugin;

impl Plugin for EmbedAssetsPlugin {
    fn build(&self, app: &mut App) {
        let embedded = app.world_mut().resource_mut::<EmbeddedAssetRegistry>();
        embed_assets(embedded.into_inner());
    }
}

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;
/// Salt for the GPU pipeline-cache key. `precomputed_shader_hash` only sees the
/// shader *files*, so a change that alters generated shaders without touching
/// them (e.g. the naga_oil override-emit-order fix) is invisible to it. Bump
/// this to force every client to drop its stale cached pipelines on next load.
#[cfg(target_arch = "wasm32")]
const GPU_CACHE_SALT: u32 = 1;

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn gpu_cache_hash() -> String {
    format!("{}-{}", precomputed_shader_hash(), GPU_CACHE_SALT)
}

#[cfg(test)]
mod test {
    const SHADOW_SOFTENING: &str = include_str!("assets/shaders/shadow_softening.wgsl");

    /// `soften_directional_shadow`'s fade in shadow_softening.wgsl: 0 keeps the shadow, 1 is lit.
    fn shadow_fade(distance: f32, shadow_distance: f32) -> f32 {
        let fade_near = (1.0 - 0.7053232) * shadow_distance;
        let near2 = fade_near * fade_near;
        ((distance * distance - near2) / (shadow_distance * shadow_distance - near2))
            .clamp(0.0, 1.0)
    }

    #[test]
    fn shadow_fade_and_opacity() {
        for expression in [
            "const CASCADE_BORDER: f32 = 0.7053232;",
            "let fade_near = (1.0 - CASCADE_BORDER) * far;",
            "saturate((dist2 - fade_near * fade_near) / (far * far - fade_near * fade_near))",
            // shadows block 0.65 of the sun (decisions.md)
            "const SHADOW_OPACITY: f32 = 0.65;",
            "let softened = mix(1.0, base, SHADOW_OPACITY);",
            // past the distance the shadow holds a floor (decisions.md)
            "const CASCADE_FAR_SHADOW: f32 = 0.35;",
            "return mix(softened, 1.0 - CASCADE_FAR_SHADOW, fade);",
        ] {
            assert!(
                SHADOW_SOFTENING.contains(expression),
                "fade drift: {expression}"
            );
        }
        // High (100 m) starts fading at 29.5 m whatever the cascade count
        assert_eq!(shadow_fade(29.4, 100.0), 0.0);
        assert!((shadow_fade(75.0, 100.0) - 0.521).abs() < 0.001);
        assert_eq!(shadow_fade(100.0, 100.0), 1.0);
        // Medium (60 m)
        assert!((shadow_fade(40.0, 60.0) - 0.392).abs() < 0.001);
    }
}
