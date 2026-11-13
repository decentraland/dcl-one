// Sun shadow fade shared by the fetch_directional_shadow overrides in bound_material.wgsl,
// soft_shadows.wgsl and the grass shell shader. The overrides stay in each top-level shader, and
// this module must not import view bindings: importing such a module breaks the override
// ("can't find function shadows::fetch_directional_shadow for redirection"), so callers pass in
// the shadow distance and the camera distance.

/// The fraction of the shadow distance the fade spans: shadows fade from `(1 - border) * distance`
/// to the shadow distance, linear in squared camera distance, whatever the cascade count, so the
/// shadow reaches the far floor without a hard edge.
const CASCADE_BORDER: f32 = 0.7053232;

/// How much of the sun a shadow blocks: 1 is full, 0 is none.
/// unity: the sun's shadow strength is 1 (`Main.unity`) and the Trilight ambient alone fills the
/// shadow in. Below 1 the shadow lets that share of the sun through, its specular highlight
/// included, and a highlight inside a shadow reads as wet or metallic.
const SHADOW_OPACITY: f32 = 1.0;

/// How much of the sun is blocked at and past the shadow distance, where bevy returns fully lit.
/// Holding a partial shadow there keeps the change between near and far small (see decisions.md).
const CASCADE_FAR_SHADOW: f32 = 0.35;

// Soften a directional shadow value from `shadows::fetch_directional_shadow` and fade it towards
// the far floor at the shadow distance. `far` is the far bound of the light's last cascade (the
// shadow distance) and `dist2` the squared distance from the camera to the fragment.
fn soften_directional_shadow(base: f32, far: f32, dist2: f32) -> f32 {
    let softened = mix(1.0, base, SHADOW_OPACITY);
    if far <= 0.0 { return softened; }
    let fade_near = (1.0 - CASCADE_BORDER) * far;
    let fade = saturate((dist2 - fade_near * fade_near) / (far * far - fade_near * fade_near));
    return mix(softened, 1.0 - CASCADE_FAR_SHADOW, fade);
}
