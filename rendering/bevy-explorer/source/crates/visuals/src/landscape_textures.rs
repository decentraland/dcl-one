//! The landscape's ground splat, ground and sand colours and cliff rock textures, embedded from
//! unity-explorer (`crates/assets/LANDSCAPE_NOTICE.md`). They tile every few metres over ground
//! seen out to the horizon, so each gets a mip chain as it loads (the PNG and JPEG loaders build
//! none, and without one the distance shimmers) and a repeating, trilinear sampler.

use bevy::{
    asset::RenderAssetUsages,
    image::{
        ImageAddressMode, ImageFilterMode, ImageLoaderSettings, ImageSampler,
        ImageSamplerDescriptor,
    },
    prelude::*,
    render::render_resource::{ShaderDefVal, TextureFormat},
};

/// The splat covers this many metres of ground and repeats beyond them.
pub(crate) const GROUND_SPLAT_METRES: u32 = 1024;

/// The splat size, for the pipeline of every shader that imports `ground_albedo_normal`.
pub(crate) fn ground_shader_defs() -> [ShaderDefVal; 1] {
    [ShaderDefVal::UInt(
        "GROUND_SPLAT_METRES".to_owned(),
        GROUND_SPLAT_METRES,
    )]
}

/// What a texture's texels hold, which decides its colour space and how its mips average.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum TexelKind {
    /// sRGB colour, averaged in linear light.
    Colour,
    /// Linear values such as blend weights, averaged as stored.
    Data,
    /// A tangent-space normal map, averaged and renormalised.
    Normal,
}

pub(crate) struct LandscapeTexturesPlugin;

impl Plugin for LandscapeTexturesPlugin {
    fn build(&self, app: &mut App) {
        // in Update, before the frame's asset events, so the first upload already has the mips
        app.init_resource::<LandscapeTextures>()
            .add_systems(Update, build_mips);
    }
}

#[derive(Resource, Clone)]
pub(crate) struct LandscapeTextures {
    /// Ground weight in red, sand weight in green.
    pub(crate) splat: Handle<Image>,
    pub(crate) ground: Handle<Image>,
    pub(crate) sand: Handle<Image>,
    pub(crate) cliff: Handle<Image>,
    pub(crate) cliff_normal: Handle<Image>,
}

impl FromWorld for LandscapeTextures {
    fn from_world(world: &mut World) -> Self {
        let server = world.resource::<AssetServer>();
        let load = |path: &str, kind: TexelKind| {
            server.load_with_settings(
                format!("embedded://landscape/{path}"),
                move |settings: &mut ImageLoaderSettings| {
                    settings.is_srgb = kind == TexelKind::Colour;
                    settings.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
                        address_mode_u: ImageAddressMode::Repeat,
                        address_mode_v: ImageAddressMode::Repeat,
                        mag_filter: ImageFilterMode::Linear,
                        min_filter: ImageFilterMode::Linear,
                        mipmap_filter: ImageFilterMode::Linear,
                        ..default()
                    });
                },
            )
        };
        Self {
            splat: load("ground/splat.png", TexelKind::Data),
            ground: load("ground/ground_basecolor.jpg", TexelKind::Colour),
            sand: load("ground/sand_basecolor.jpg", TexelKind::Colour),
            cliff: load("cliff/cliff_basecolor.jpg", TexelKind::Colour),
            cliff_normal: load("cliff/cliff_normal.png", TexelKind::Normal),
        }
    }
}

impl LandscapeTextures {
    pub(crate) fn all(&self) -> [(&Handle<Image>, TexelKind); 5] {
        [
            (&self.splat, TexelKind::Data),
            (&self.ground, TexelKind::Colour),
            (&self.sand, TexelKind::Colour),
            (&self.cliff, TexelKind::Colour),
            (&self.cliff_normal, TexelKind::Normal),
        ]
    }
}

/// "loaded", "loading", or "failed" when any texture failed, for the debug panel.
pub(crate) fn load_state(textures: &LandscapeTextures, server: &AssetServer) -> &'static str {
    let states = textures.all().map(|(handle, _)| server.load_state(handle));
    if states.iter().any(|state| state.is_failed()) {
        "failed"
    } else if states.iter().all(|state| state.is_loaded()) {
        "loaded"
    } else {
        "loading"
    }
}

/// Gives each texture its mips once it has loaded. A mipped texture drops its main-world copy at
/// its upload, so it is not seen again.
fn build_mips(textures: Res<LandscapeTextures>, mut images: ResMut<Assets<Image>>) {
    for (handle, kind) in textures.all() {
        let unmipped = images
            .get(handle)
            .is_some_and(|image| image.texture_descriptor.mip_level_count == 1);
        if unmipped {
            if let Some(image) = images.get_mut(handle) {
                add_mips(image, kind);
            }
        }
    }
}

/// Appends a box-filtered mip chain, down to one texel, to an RGBA8 image.
pub(crate) fn add_mips(image: &mut Image, kind: TexelKind) {
    if image.texture_descriptor.mip_level_count != 1 {
        return;
    }
    let format = image.texture_descriptor.format;
    if !matches!(
        format,
        TextureFormat::Rgba8Unorm | TextureFormat::Rgba8UnormSrgb
    ) {
        warn!("no mips for a {format:?} landscape texture");
        return;
    }
    let Some(top) = image.data.as_ref() else {
        return;
    };
    let (mut width, mut height) = (image.width() as usize, image.height() as usize);
    let mut level: Vec<Vec4> = top
        .chunks_exact(4)
        .map(|texel| decode(texel, kind))
        .collect();
    let mut bytes = top.clone();
    let mut levels = 1;
    while width > 1 || height > 1 {
        let (half_width, half_height) = ((width / 2).max(1), (height / 2).max(1));
        level = (0..half_width * half_height)
            .map(|i| {
                let (x, y) = (i % half_width * 2, i / half_width * 2);
                let (x1, y1) = ((x + 1).min(width - 1), (y + 1).min(height - 1));
                (level[y * width + x]
                    + level[y * width + x1]
                    + level[y1 * width + x]
                    + level[y1 * width + x1])
                    * 0.25
            })
            .collect();
        bytes.extend(level.iter().flat_map(|&texel| encode(texel, kind)));
        (width, height) = (half_width, half_height);
        levels += 1;
    }
    image.data = Some(bytes);
    image.texture_descriptor.mip_level_count = levels;
    // drawn only, never read back: no main-world copy once uploaded
    image.asset_usage = RenderAssetUsages::RENDER_WORLD;
}

fn decode(texel: &[u8], kind: TexelKind) -> Vec4 {
    let value = Vec4::from_array(std::array::from_fn(|c| f32::from(texel[c]) / 255.0));
    let rgb = value.xyz();
    let rgb = match kind {
        TexelKind::Colour => Vec3::from_array(rgb.to_array().map(Srgba::gamma_function)),
        TexelKind::Data => rgb,
        TexelKind::Normal => rgb * 2.0 - Vec3::ONE,
    };
    rgb.extend(value.w)
}

fn encode(texel: Vec4, kind: TexelKind) -> [u8; 4] {
    let rgb = texel.xyz();
    let rgb = match kind {
        TexelKind::Colour => Vec3::from_array(rgb.to_array().map(Srgba::gamma_function_inverse)),
        TexelKind::Data => rgb,
        TexelKind::Normal => rgb.normalize_or(Vec3::Z) * 0.5 + Vec3::splat(0.5),
    };
    rgb.extend(texel.w)
        .to_array()
        .map(|value| (value.clamp(0.0, 1.0) * 255.0).round() as u8)
}

#[cfg(test)]
mod tests;
