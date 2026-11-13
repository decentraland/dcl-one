//! The surfaces of the trees, bushes and stones: the reference values on bevy's lit material, the
//! leaves swaying in the wind, and the mip chains their textures need.

use bevy::{
    image::{
        ImageAddressMode, ImageFilterMode, ImageLoaderSettings, ImageSampler,
        ImageSamplerDescriptor,
    },
    pbr::{ExtendedMaterial, MaterialExtension},
    prelude::*,
    render::render_resource::{AsBindGroup, Face, ShaderRef, TextureFormat},
};

use super::ASSET_DIR;

/// A material slot of the models, which name their slots after these variants.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Surface {
    LeafYellow,
    LeafPink,
    LeafGreen,
    LeafBushGreen,
    LeafBushGreenDark,
    Trunk,
    Rock01,
    Rock02,
    Rock03,
}

/// Wind on a surface, in the reference's frame (x east, z north).
pub(crate) struct Wind {
    pub speed: f32,
    pub strength: f32,
    pub frequency: f32,
    pub turbulence: f32,
    pub direction: Vec3,
    pub flutter_speed: f32,
    pub flutter_strength: f32,
    pub flutter_frequency: f32,
}

pub(crate) struct SurfaceSpec {
    /// In sRGB, multiplied with the texture.
    pub base_color: Color,
    /// Under `landscape/trees/`.
    pub texture: &'static str,
    /// The texture's stored values are used as they are, not decoded from sRGB.
    pub linear: bool,
    /// Under `landscape/trees/`.
    pub normal_map: Option<&'static str>,
    /// Texels whose alpha is below this are cut out, and both faces are drawn; without it the
    /// surface is opaque and drawn from the front.
    pub cutoff: Option<f32>,
    pub perceptual_roughness: f32,
    pub wind: Option<Wind>,
}

/// The tree leaves' wind; the bushes' is gentler and slower to flutter.
const LEAF_WIND: Wind = Wind {
    speed: 1.0,
    strength: 0.5,
    frequency: 0.0,
    turbulence: 1.3,
    direction: Vec3::X,
    flutter_speed: 0.8,
    flutter_strength: 0.1,
    flutter_frequency: 5.0,
};
const BUSH_WIND: Wind = Wind {
    speed: 1.5,
    strength: 0.05,
    frequency: 0.0,
    turbulence: 1.5,
    direction: Vec3::X,
    flutter_speed: 0.35,
    flutter_strength: 0.1,
    flutter_frequency: 5.0,
};
const LEAF_CUTOFF: f32 = 0.8;
/// Leaves under bevy's image-based light: a glossy leaf mirrors the sky, so they stay fully rough.
const LEAF_ROUGHNESS: f32 = 1.0;

const fn leaf(base_color: Color, texture: &'static str, wind: Wind) -> SurfaceSpec {
    SurfaceSpec {
        base_color,
        texture,
        linear: true,
        normal_map: None,
        cutoff: Some(LEAF_CUTOFF),
        perceptual_roughness: LEAF_ROUGHNESS,
        wind: Some(wind),
    }
}

/// Smoothness 0.209 as roughness.
const fn rock(texture: &'static str, normal_map: &'static str) -> SurfaceSpec {
    SurfaceSpec {
        base_color: Color::WHITE,
        texture,
        linear: false,
        normal_map: Some(normal_map),
        cutoff: None,
        perceptual_roughness: 1.0 - 0.209,
        wind: None,
    }
}

/// Indexed by `Surface`.
pub(crate) const SURFACES: [SurfaceSpec; 9] = [
    leaf(
        Color::srgb(1.0, 0.7611569, 0.28773582),
        "leaf.png",
        LEAF_WIND,
    ),
    leaf(Color::srgb(1.0, 0.5224057, 0.504717), "leaf.png", LEAF_WIND),
    leaf(
        Color::srgb(0.27367392, 0.7735849, 0.5473779),
        "leaf.png",
        LEAF_WIND,
    ),
    leaf(
        Color::srgb(0.1699448, 0.6320754, 0.42576712),
        "bush_leaves.png",
        BUSH_WIND,
    ),
    leaf(
        Color::srgb(0.11320755, 0.43626308, 0.4528302),
        "bush_leaves.png",
        BUSH_WIND,
    ),
    // smoothness 0.33 as roughness
    SurfaceSpec {
        base_color: Color::WHITE,
        texture: "trunk_color.jpg",
        linear: false,
        normal_map: Some("trunk_normal.jpg"),
        cutoff: None,
        perceptual_roughness: 1.0 - 0.33,
        wind: None,
    },
    rock("stone01_color.jpg", "stone01_normal.jpg"),
    rock("stone02_color.jpg", "stone02_normal.jpg"),
    rock("stone03_color.jpg", "stone03_normal.jpg"),
];

pub(crate) const ALL_SURFACES: [Surface; 9] = [
    Surface::LeafYellow,
    Surface::LeafPink,
    Surface::LeafGreen,
    Surface::LeafBushGreen,
    Surface::LeafBushGreenDark,
    Surface::Trunk,
    Surface::Rock01,
    Surface::Rock02,
    Surface::Rock03,
];

impl Surface {
    pub(crate) fn spec(self) -> &'static SurfaceSpec {
        &SURFACES[self as usize]
    }
}

pub(crate) type TreeMaterial = ExtendedMaterial<TreeSway>;

/// The wind of `landscape_tree.wgsl`.
#[derive(Asset, TypePath, AsBindGroup, Clone, Default)]
pub(crate) struct TreeSway {
    /// Speed, strength, frequency and turbulence of the sway.
    #[uniform(100)]
    sway: Vec4,
    /// Direction in the reference's frame (x east, z north).
    #[uniform(101)]
    direction: Vec4,
    /// Speed, strength and frequency of the flutter; w is 1 while the surface sways, else 0.
    #[uniform(102)]
    flutter: Vec4,
}

impl TreeSway {
    pub(crate) fn new(wind: Option<&Wind>, enabled: bool) -> Self {
        let Some(wind) = wind else {
            return Self::default();
        };
        Self {
            sway: Vec4::new(wind.speed, wind.strength, wind.frequency, wind.turbulence),
            direction: wind.direction.extend(0.0),
            flutter: Vec4::new(
                wind.flutter_speed,
                wind.flutter_strength,
                wind.flutter_frequency,
                f32::from(u8::from(enabled)),
            ),
        }
    }
}

impl MaterialExtension for TreeSway {
    type Base = StandardMaterial;

    fn vertex_shader() -> ShaderRef {
        super::tree_vertex_shader()
    }
    fn prepass_vertex_shader() -> ShaderRef {
        super::tree_vertex_shader()
    }
    fn deferred_vertex_shader() -> ShaderRef {
        super::tree_vertex_shader()
    }
    fn fragment_shader() -> ShaderRef {
        super::soft_shadows_fragment()
    }
}

/// Repeating, filtered within a mip and not between mips.
fn sampler() -> ImageSampler {
    ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::Repeat,
        address_mode_v: ImageAddressMode::Repeat,
        mag_filter: ImageFilterMode::Linear,
        min_filter: ImageFilterMode::Linear,
        mipmap_filter: ImageFilterMode::Nearest,
        ..default()
    })
}

pub(crate) fn load_texture(asset_server: &AssetServer, file: &str, srgb: bool) -> Handle<Image> {
    asset_server.load_with_settings(
        format!("{ASSET_DIR}/{file}"),
        move |settings: &mut ImageLoaderSettings| {
            settings.is_srgb = srgb;
            settings.sampler = sampler();
        },
    )
}

/// The material of a surface, with its textures loading.
pub(crate) fn surface_material(
    spec: &SurfaceSpec,
    asset_server: &AssetServer,
    wind: bool,
) -> (TreeMaterial, Vec<Handle<Image>>) {
    let texture = load_texture(asset_server, spec.texture, !spec.linear);
    let normal_map = spec
        .normal_map
        .map(|file| load_texture(asset_server, file, false));
    let textures = [Some(texture.clone()), normal_map.clone()]
        .into_iter()
        .flatten()
        .collect();
    let material = TreeMaterial {
        base: StandardMaterial {
            base_color: spec.base_color,
            base_color_texture: Some(texture),
            normal_map_texture: normal_map,
            alpha_mode: spec.cutoff.map_or(AlphaMode::Opaque, AlphaMode::Mask),
            // the leaf cards keep their normals on both faces
            cull_mode: if spec.cutoff.is_some() {
                None
            } else {
                Some(Face::Back)
            },
            double_sided: false,
            perceptual_roughness: spec.perceptual_roughness,
            ..default()
        },
        extension: TreeSway::new(spec.wind.as_ref(), wind),
    };
    (material, textures)
}

/// Appends a box-filtered mip chain to a square, power-of-two RGBA8 image, averaging in linear
/// light when its texels are sRGB. Returns false, leaving the image as it is, for any other image.
pub(crate) fn add_mips(image: &mut Image) -> bool {
    let size = image.texture_descriptor.size;
    let srgb = match image.texture_descriptor.format {
        TextureFormat::Rgba8UnormSrgb => true,
        TextureFormat::Rgba8Unorm => false,
        _ => return false,
    };
    let side = size.width as usize;
    if size.height != size.width
        || !side.is_power_of_two()
        || size.depth_or_array_layers != 1
        || image.texture_descriptor.mip_level_count != 1
    {
        return false;
    }
    let Some(data) = image.data.as_mut() else {
        return false;
    };
    if data.len() != side * side * 4 {
        return false;
    }

    let decode: [f32; 256] = std::array::from_fn(|byte| {
        let value = byte as f32 / 255.0;
        if srgb {
            if value <= 0.04045 {
                value / 12.92
            } else {
                ((value + 0.055) / 1.055).powf(2.4)
            }
        } else {
            value
        }
    });
    let encode = |value: f32| {
        let value = value.clamp(0.0, 1.0);
        let value = if !srgb {
            value
        } else if value <= 0.0031308 {
            value * 12.92
        } else {
            1.055 * value.powf(1.0 / 2.4) - 0.055
        };
        (value * 255.0).round() as u8
    };

    let mut level_start = 0;
    let mut level_side = side;
    let mut levels = 1;
    while level_side > 1 {
        let next_side = level_side / 2;
        let mut next = Vec::with_capacity(next_side * next_side * 4);
        for y in 0..next_side {
            for x in 0..next_side {
                for channel in 0..4 {
                    let texel = |dx: usize, dy: usize| {
                        data[level_start + ((2 * y + dy) * level_side + 2 * x + dx) * 4 + channel]
                    };
                    let bytes = [texel(0, 0), texel(1, 0), texel(0, 1), texel(1, 1)];
                    next.push(if channel == 3 {
                        (bytes.iter().map(|&b| u32::from(b)).sum::<u32>() as f32 / 4.0).round()
                            as u8
                    } else {
                        encode(bytes.iter().map(|&b| decode[usize::from(b)]).sum::<f32>() / 4.0)
                    });
                }
            }
        }
        level_start = data.len();
        data.extend_from_slice(&next);
        level_side = next_side;
        levels += 1;
    }
    image.texture_descriptor.mip_level_count = levels;
    true
}
