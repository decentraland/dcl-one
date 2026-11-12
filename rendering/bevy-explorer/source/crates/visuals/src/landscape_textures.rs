//! The landscape's ground splat, ground and sand colours and cliff rock textures, embedded from
//! unity-explorer (`crates/assets/LANDSCAPE_NOTICE.md`). They tile every few metres over ground
//! seen out to the horizon, so each gets a mip chain as it loads (the PNG and JPEG loaders build
//! none, and without one the distance shimmers) and a repeating, trilinear sampler.

use std::sync::Arc;

use bevy::{
    asset::{LoadState, RenderAssetUsages},
    image::{
        ImageAddressMode, ImageFilterMode, ImageLoaderSettings, ImageSampler,
        ImageSamplerDescriptor,
    },
    prelude::*,
    render::render_resource::{Extent3d, ShaderDefVal, TextureDimension, TextureFormat},
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
            .init_resource::<ScatterGround>()
            .add_event::<LandscapeTexturesReplaced>()
            .add_systems(Update, resolve_textures.in_set(ResolveLandscapeTextures));
    }
}

/// Sent in a frame in which a file replaced its stand-in.
#[derive(Event)]
pub(crate) struct LandscapeTexturesReplaced;

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
struct ResolveLandscapeTextures;

/// A prepared material keeps the texture views it was prepared with, and a texture replaced
/// under the same handle does not prepare it again. So every material of type `M`, which binds
/// these textures, is touched in a frame in which one was replaced, which prepares it again.
pub(crate) fn rebind_on_replace<M: Asset>(app: &mut App) {
    app.add_systems(
        Update,
        touch_all::<M>
            .run_if(on_event::<LandscapeTexturesReplaced>)
            .after(ResolveLandscapeTextures),
    );
}

fn touch_all<M: Asset>(mut materials: ResMut<Assets<M>>) {
    // iter_mut marks each one modified
    for _ in materials.iter_mut() {}
}

/// The flat ground colour, which the ground showed before it was textured.
const FLAT_GROUND: [u8; 4] = [77, 115, 51, 255];
/// The plain rock grey the cliffs showed before they were textured.
const PLAIN_ROCK: [u8; 4] = [110, 105, 102, 255];

/// Each texture's file under `embedded://landscape/`, what it holds, and the one texel drawn in
/// its place until it loads, or for good if it fails (for example an LFS pointer embedded in place
/// of the image): together they give the ground and cliffs their former flat colours.
const SOURCES: [(&str, TexelKind, [u8; 4]); 5] = [
    // all ground, no sand
    ("ground/splat.png", TexelKind::Data, [255, 0, 0, 0]),
    (
        "ground/ground_basecolor.jpg",
        TexelKind::Colour,
        FLAT_GROUND,
    ),
    ("ground/sand_basecolor.jpg", TexelKind::Colour, FLAT_GROUND),
    ("cliff/cliff_basecolor.jpg", TexelKind::Colour, PLAIN_ROCK),
    // straight up
    (
        "cliff/cliff_normal.png",
        TexelKind::Normal,
        [128, 128, 255, 255],
    ),
];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum TextureState {
    Loading,
    Ready,
    Failed,
}

/// The handles materials bind. Each holds a one-texel stand-in from the start, so the ground and
/// cliffs always draw, and is replaced in place by its file, with mips, once that has loaded.
#[derive(Resource, Clone)]
pub(crate) struct LandscapeTextures {
    /// Ground weight in red, sand weight in green.
    pub(crate) splat: Handle<Image>,
    pub(crate) ground: Handle<Image>,
    pub(crate) sand: Handle<Image>,
    pub(crate) cliff: Handle<Image>,
    pub(crate) cliff_normal: Handle<Image>,
    /// The loading files, dropped once resolved.
    sources: [Option<Handle<Image>>; 5],
    states: [TextureState; 5],
    /// What the flower and reed scatter reads of the splat, ground and sand files, taken as each
    /// loads: the drawn copies keep no main-world data.
    scatter_samples: [Option<Arc<[f32]>>; 3],
}

impl FromWorld for LandscapeTextures {
    fn from_world(world: &mut World) -> Self {
        Self::load(world, SOURCES.map(|(path, _, _)| path))
    }
}

impl LandscapeTextures {
    /// Starts loading `paths` (in `SOURCES` order) and gives each its stand-in.
    pub(crate) fn load(world: &mut World, paths: [&str; 5]) -> Self {
        let server = world.resource::<AssetServer>().clone();
        let sources = std::array::from_fn(|i| {
            let kind = SOURCES[i].1;
            Some(server.load_with_settings(
                format!("embedded://landscape/{}", paths[i]),
                move |settings: &mut ImageLoaderSettings| {
                    settings.is_srgb = kind == TexelKind::Colour;
                    settings.sampler = tiling_sampler();
                },
            ))
        });
        let mut images = world.resource_mut::<Assets<Image>>();
        let [splat, ground, sand, cliff, cliff_normal] = SOURCES.map(|(_, kind, texel)| {
            let mut image = Image::new_fill(
                Extent3d::default(),
                TextureDimension::D2,
                &texel,
                match kind {
                    TexelKind::Colour => TextureFormat::Rgba8UnormSrgb,
                    TexelKind::Data | TexelKind::Normal => TextureFormat::Rgba8Unorm,
                },
                RenderAssetUsages::default(),
            );
            image.sampler = tiling_sampler();
            images.add(image)
        });
        Self {
            splat,
            ground,
            sand,
            cliff,
            cliff_normal,
            sources,
            states: [TextureState::Loading; 5],
            scatter_samples: Default::default(),
        }
    }

    /// The bound handles, in `SOURCES` order.
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

fn tiling_sampler() -> ImageSampler {
    ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::Repeat,
        address_mode_v: ImageAddressMode::Repeat,
        mag_filter: ImageFilterMode::Linear,
        min_filter: ImageFilterMode::Linear,
        mipmap_filter: ImageFilterMode::Linear,
        ..default()
    })
}

/// "loaded", "loading", or "failed" when any texture failed, for the debug panel.
pub(crate) fn load_state(textures: &LandscapeTextures) -> &'static str {
    if textures.states.contains(&TextureState::Failed) {
        "failed"
    } else if textures.states.contains(&TextureState::Loading) {
        "loading"
    } else {
        "loaded"
    }
}

/// Moves each file, once loaded and given its mips, into its bound handle; a file that fails
/// leaves its stand-in. Each file is handled once, so a failure or an unsupported image is
/// reported once.
fn resolve_textures(
    mut textures: ResMut<LandscapeTextures>,
    server: Res<AssetServer>,
    mut images: ResMut<Assets<Image>>,
    mut replaced: EventWriter<LandscapeTexturesReplaced>,
    mut scatter: ResMut<ScatterGround>,
) {
    if !textures.states.contains(&TextureState::Loading) {
        return;
    }
    let textures = &mut *textures;
    let targets = textures.all().map(|(handle, kind)| (handle.id(), kind));
    for (i, (target, kind)) in targets.into_iter().enumerate() {
        let Some(source) = textures.sources[i].as_ref() else {
            continue;
        };
        let path = source.path().map(ToString::to_string).unwrap_or_default();
        textures.states[i] = match server.load_state(source) {
            LoadState::Loaded => {
                let Some(mut image) = images.remove(source) else {
                    continue;
                };
                if add_mips(&mut image, kind) {
                    // drawn only, never read back: no main-world copy once uploaded
                    image.asset_usage = RenderAssetUsages::RENDER_WORLD;
                }
                if let Some(samples) = textures.scatter_samples.get_mut(i) {
                    *samples = scatter_samples(&image, i);
                }
                images.insert(target, image);
                replaced.write(LandscapeTexturesReplaced);
                TextureState::Ready
            }
            LoadState::Failed(error) => {
                error!("landscape texture {path} failed to load, drawing a flat colour: {error}");
                TextureState::Failed
            }
            _ => continue,
        };
        textures.sources[i] = None;
    }
    if scatter.0.is_none() {
        if let [Some(splat), Some(ground), Some(sand)] = &textures.scatter_samples {
            scatter.0 = Some(Arc::new(GroundGreen::from_samples(splat, ground, sand)));
            textures.scatter_samples = Default::default();
        }
    }
}

/// Appends a box-filtered mip chain, down to one texel, to an RGBA8 image. False, with the image
/// untouched, if it already has mips, is not RGBA8 or has no data.
pub(crate) fn add_mips(image: &mut Image, kind: TexelKind) -> bool {
    if image.texture_descriptor.mip_level_count != 1 {
        return false;
    }
    let format = image.texture_descriptor.format;
    if !matches!(
        format,
        TextureFormat::Rgba8Unorm | TextureFormat::Rgba8UnormSrgb
    ) {
        warn!("no mips for a {format:?} landscape texture");
        return false;
    }
    let Some(top) = image.data.as_ref() else {
        return false;
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
    true
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

/// Spacing in metres of the corners at which the flower and reed scatter reads the ground.
const SCATTER_CELL: u32 = 4;
/// Scatter corners along each side of the splat's tile.
const SCATTER_CORNERS: usize = (GROUND_SPLAT_METRES / SCATTER_CELL) as usize;
/// The splat level the scatter reads: one texel per scatter cell.
const SCATTER_SPLAT_LEVEL: u32 = 2;

/// What the flower and reed scatter reads of the ground. None until the splat and both colours
/// have loaded, and for good if one fails: their one-texel stand-ins would read as sand
/// everywhere.
#[derive(Resource, Default, Clone)]
pub(crate) struct ScatterGround(pub(crate) Option<Arc<GroundGreen>>);

/// The green of the splat-weighted ground and sand colour at every 4 m corner of the splat's
/// tile, as unity-explorer's flower and reed scatter reads it: the splat at its 4 m level, the
/// ground and sand colours at their full level and laid at the splat's tiling, not the ground's.
/// The scatter tests that green against its flower and reed limits.
#[derive(Debug, PartialEq)]
pub(crate) struct GroundGreen {
    /// Rows along bevy +z (unity -z), as the textures are laid.
    values: Vec<f32>,
}

impl GroundGreen {
    /// The green at each corner, given its Unity x/z in metres within the first tile.
    #[cfg(test)]
    pub(crate) fn from_fn(green: impl Fn(IVec2) -> f32) -> Self {
        let cell = SCATTER_CELL as i32;
        Self {
            values: (0..SCATTER_CORNERS * SCATTER_CORNERS)
                .map(|i| {
                    let (x, row) = ((i % SCATTER_CORNERS) as i32, (i / SCATTER_CORNERS) as i32);
                    green(IVec2::new(x * cell, -row * cell))
                })
                .collect(),
        }
    }

    /// `splat` holds red and green per corner, `ground` and `sand` their colour's green.
    fn from_samples(splat: &[f32], ground: &[f32], sand: &[f32]) -> Self {
        Self {
            values: (0..SCATTER_CORNERS * SCATTER_CORNERS)
                .map(|i| ground[i] * splat[2 * i] + sand[i] * splat[2 * i + 1])
                .collect(),
        }
    }

    /// The green at a corner, given its Unity x/z in metres (a multiple of 4).
    pub(crate) fn at(&self, unity: IVec2) -> f32 {
        let cell = SCATTER_CELL as i32;
        let corners = SCATTER_CORNERS as i32;
        let x = (unity.x / cell).rem_euclid(corners);
        let row = (-unity.y / cell).rem_euclid(corners);
        self.values[(row * corners + x) as usize]
    }
}

/// What the scatter reads of the splat (`index` 0: red and green per corner) or of the ground or
/// sand colour (1, 2: green per corner). None if the image has too few levels or is not RGBA8.
fn scatter_samples(image: &Image, index: usize) -> Option<Arc<[f32]>> {
    let format = image.texture_descriptor.format;
    if !matches!(
        format,
        TextureFormat::Rgba8Unorm | TextureFormat::Rgba8UnormSrgb
    ) {
        return None;
    }
    let corner = |i: usize| {
        let (x, row) = (i % SCATTER_CORNERS, i / SCATTER_CORNERS);
        Vec2::new(x as f32, row as f32) * (SCATTER_CELL as f32 / GROUND_SPLAT_METRES as f32)
    };
    let corners = 0..SCATTER_CORNERS * SCATTER_CORNERS;
    Some(if index == 0 {
        let level = level_texels(image, SCATTER_SPLAT_LEVEL)?;
        corners
            .flat_map(|i| {
                let uv = corner(i);
                [0, 1].map(|channel| level.bilinear(uv, channel, |texel| texel as f32 / 255.0))
            })
            .collect()
    } else {
        let level = level_texels(image, 0)?;
        let linear = |texel: u8| Srgba::gamma_function(texel as f32 / 255.0);
        corners
            .map(|i| level.bilinear(corner(i), 1, linear))
            .collect()
    })
}

/// One mip level of an RGBA8 image.
struct LevelTexels<'a> {
    texels: &'a [u8],
    width: usize,
    height: usize,
}

fn level_texels(image: &Image, level: u32) -> Option<LevelTexels<'_>> {
    if level >= image.texture_descriptor.mip_level_count {
        return None;
    }
    let data = image.data.as_ref()?;
    let size = |level: u32| {
        (
            (image.width() as usize >> level).max(1),
            (image.height() as usize >> level).max(1),
        )
    };
    let start: usize = (0..level)
        .map(|level| {
            let (width, height) = size(level);
            width * height * 4
        })
        .sum();
    let (width, height) = size(level);
    let texels = data.get(start..start + width * height * 4)?;
    Some(LevelTexels {
        texels,
        width,
        height,
    })
}

impl LevelTexels<'_> {
    /// A repeating bilinear sample of one channel at `uv`, as a GPU sampler takes it.
    fn bilinear(&self, uv: Vec2, channel: usize, decode: impl Fn(u8) -> f32) -> f32 {
        let size = Vec2::new(self.width as f32, self.height as f32);
        let position = uv * size - 0.5;
        let base = position.floor();
        let fraction = position - base;
        let texel = |dx: i32, dy: i32| {
            let x = (base.x as i32 + dx).rem_euclid(self.width as i32) as usize;
            let y = (base.y as i32 + dy).rem_euclid(self.height as i32) as usize;
            decode(self.texels[(y * self.width + x) * 4 + channel])
        };
        let top = texel(0, 0) + (texel(1, 0) - texel(0, 0)) * fraction.x;
        let bottom = texel(0, 1) + (texel(1, 1) - texel(0, 1)) * fraction.x;
        top + (bottom - top) * fraction.y
    }
}
#[cfg(test)]
mod tests;
