// Sun shadow fade shared by the fetch_directional_shadow overrides in bound_material.wgsl,
// soft_shadows.wgsl and the grass shell shader. The overrides stay in each top-level shader, and
// this module must not import view bindings: importing such a module breaks the override
// ("can't find function shadows::fetch_directional_shadow for redirection"), so callers pass in
// the shadow distance and the camera distance.
//
// unity: URP's main light shadow fade, transcribed from com.unity.render-pipelines.universal:
// ShadowUtils.GetScaleAndBiasForLinearDistanceFade (Runtime/ShadowUtils.cs), fed the squared
// shadow distance by MainLightShadowCasterPass.cs, and GetMainLightShadowFade and
// MixRealtimeAndBakedShadows (ShaderLibrary/Shadows.hlsl).

/// unity: UniversalRenderPipeline - High.asset m_CascadeBorder. URP fades from
/// `(1 - border) * distance` to the shadow distance, linear in squared camera distance, whatever
/// the cascade count.
const CASCADE_BORDER: f32 = 0.7053232;

/// How much of the sun a shadow blocks: 1 is full (Unity's sun, `m_Strength` 1), 0 is none.
/// Unity's Trilight ambient fills its shadows in; ours is darker, so shadows block 0.65 of the
/// sun until that ambient lands (see decisions.md).
const SHADOW_OPACITY: f32 = 0.65;

// Soften a directional shadow value from `shadows::fetch_directional_shadow` and fade it to lit
// at the shadow distance. `far` is the far bound of the light's last cascade (the shadow
// distance) and `dist2` the squared distance from the camera to the fragment.
fn soften_directional_shadow(base: f32, far: f32, dist2: f32) -> f32 {
    let softened = mix(1.0, base, SHADOW_OPACITY);
    if far <= 0.0 { return softened; }
    let fade_near = (1.0 - CASCADE_BORDER) * far;
    let fade = saturate((dist2 - fade_near * fade_near) / (far * far - fade_near * fade_near));
    return mix(softened, 1.0, fade);
}
