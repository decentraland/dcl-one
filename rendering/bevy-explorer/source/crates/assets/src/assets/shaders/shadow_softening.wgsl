// Sun shadow fade shared by the fetch_directional_shadow overrides in bound_material.wgsl,
// soft_shadows.wgsl and the grass shell shader. The overrides stay in each top-level shader, and
// this module must not import view bindings: importing such a module breaks the override
// ("can't find function shadows::fetch_directional_shadow for redirection"), so callers pass in
// the cascade bounds.

/// unity: UniversalRenderPipeline - High.asset m_CascadeBorder: the fraction of the last
/// cascade's span over which the shadow fades to lit, so the shadow distance does not pop.
const CASCADE_BORDER: f32 = 0.7053232;

// Fade a directional shadow value from `shadows::fetch_directional_shadow` to lit at the end of
// the last cascade. `near` and `far` are the near and far bounds of the light's last cascade.
fn soften_directional_shadow(base: f32, near: f32, far: f32, view_z: f32) -> f32 {
    if far <= 0.0 { return base; }
    let fade_start = far - CASCADE_BORDER * (far - near);
    return mix(base, 1.0, smoothstep(fade_start, far, -view_z));
}
