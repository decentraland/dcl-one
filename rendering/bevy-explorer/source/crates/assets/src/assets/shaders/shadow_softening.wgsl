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

// Fade a directional shadow value from `shadows::fetch_directional_shadow` to lit at the shadow
// distance. `far` is the far bound of the light's last cascade (the shadow distance) and `dist2`
// the squared distance from the camera to the fragment.
fn soften_directional_shadow(base: f32, far: f32, dist2: f32) -> f32 {
    if far <= 0.0 { return base; }
    let fade_near = (1.0 - CASCADE_BORDER) * far;
    let fade = saturate((dist2 - fade_near * fade_near) / (far * far - fade_near * fade_near));
    return mix(base, 1.0, fade);
}
