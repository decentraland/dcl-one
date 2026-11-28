#![forbid(unsafe_code)]

use std::marker::PhantomData;

use bevy::{
    asset::{weak_handle, RenderAssetUsages},
    pbr::{ExtendedMaterial, MaterialExtension},
    platform::collections::{hash_map::Entry, HashMap},
    prelude::*,
    render::{
        mesh::MeshTag,
        render_resource::{AsBindGroup, Face, ShaderDefVal, ShaderRef},
        storage::ShaderStorageBuffer,
    },
};
use boimp::bake::{
    ImposterBakeMaterial, ImposterBakeMaterialExtension, ImposterBakeMaterialPlugin,
};
use common::{
    sky_preset::{ambient_trilight, LIGHT_UNIT_LUX},
    structs::{PreviewMode, ShowOutOfBounds},
    util::InvertedScaleExt,
};
use highlight::HighlightPlugin;

pub mod highlight;

pub type SceneMaterial = ExtendedMaterial<SceneBound>;

pub const SCENE_MATERIAL_SHOW_OUTSIDE_BOUNDS_MESH_TAG: u32 = 0x01000000;
pub const SCENE_MATERIAL_NO_DITHERING_MESH_TAG: u32 = 0x02000000;
// a scene mesh that holds a collider the player camera is kept in front of: it is not faded
// between the camera and the player (bound_material_effect.wgsl's camera_dither)
pub const SCENE_MATERIAL_CAMERA_OBSTACLE_MESH_TAG: u32 = 0x04000000;
// cel-shade this mesh (avatars) instead of standard PBR — see toon.wgsl
pub const SCENE_MATERIAL_TOON_MESH_TAG: u32 = 0x08000000;
// the tone-on-tone avatar outline (bound_material.wgsl, mask_material.wgsl)
pub const SCENE_MATERIAL_OUTLINE_MESH_TAG: u32 = 0x10000000;
// an avatar's eyes, eyebrows or mouth: lit by the ambient light alone (toon.wgsl's
// facial_feature_lighting) instead of the toon bands
pub const SCENE_MATERIAL_FACIAL_FEATURE_MESH_TAG: u32 = 0x20000000;
/// an avatar being revealed bottom-up from its ghost: the shaders discard fragments above the
/// reveal height, carried in the tag's low byte: 0 is no reveal, and 1..=255 span the range from
/// `SCENE_MATERIAL_REVEAL_HIDE_HEIGHT` to `SCENE_MATERIAL_REVEAL_FULL_HEIGHT` above the mesh
/// origin (bound_material_effect.wgsl, which takes the encoding from `reveal_shader_defs`)
pub const SCENE_MATERIAL_REVEAL_MESH_TAG_MASK: u32 = 0x000000FF;
/// the reveal range, in millimetres so the shaders get it as integer defs
const SCENE_MATERIAL_REVEAL_HIDE_MILLIMETRES: i32 = -50;
const SCENE_MATERIAL_REVEAL_FULL_MILLIMETRES: i32 = 2050;
/// a reveal height below the feet: nothing shows
pub const SCENE_MATERIAL_REVEAL_HIDE_HEIGHT: f32 =
    SCENE_MATERIAL_REVEAL_HIDE_MILLIMETRES as f32 / 1000.0;
/// a reveal height above the head: everything shows
pub const SCENE_MATERIAL_REVEAL_FULL_HEIGHT: f32 =
    SCENE_MATERIAL_REVEAL_FULL_MILLIMETRES as f32 / 1000.0;
/// the steps from the first reveal value to the last
const SCENE_MATERIAL_REVEAL_STEPS: f32 = (SCENE_MATERIAL_REVEAL_MESH_TAG_MASK - 1) as f32;

/// `tag` with the reveal set to `height` metres above the mesh origin, clamped to the range
pub fn reveal_mesh_tag(tag: u32, height: f32) -> u32 {
    let range = SCENE_MATERIAL_REVEAL_FULL_HEIGHT - SCENE_MATERIAL_REVEAL_HIDE_HEIGHT;
    let unit = ((height - SCENE_MATERIAL_REVEAL_HIDE_HEIGHT) / range).clamp(0.0, 1.0);
    let step = 1 + (unit * SCENE_MATERIAL_REVEAL_STEPS).round() as u32;
    (tag & !SCENE_MATERIAL_REVEAL_MESH_TAG_MASK) | step
}

/// `tag` with no reveal
pub fn unrevealed_mesh_tag(tag: u32) -> u32 {
    tag & !SCENE_MATERIAL_REVEAL_MESH_TAG_MASK
}

/// the tag bits an avatar's style is read from (the camera dither, the toon lighting and the
/// outline) and the toon values derived here, for the pipeline of every shader that styles an
/// avatar's meshes: bound_material.wgsl and mask_material.wgsl. A def carries only integers, so
/// each `f32` travels as its bit pattern and the shader reads it with `bitcast<f32>(#{NAME}u)`:
/// - `TOON_LIGHT_UNIT_LUX`, the light unit;
/// - `TOON_FACIAL_FLAT_AMBIENT_R/G/B`, the ambient light, in light units, that lights the eyes,
///   eyebrows and mouth in a view without the sky environment map (the Backpack preview, a photo
///   booth): the noon Trilight's equator colour, about what the sky gives a surface facing the
///   horizon at noon. Such a view's flat ambient is set to light the body flat, far above any
///   sky, so it is not read there.
pub fn avatar_style_shader_defs() -> Vec<ShaderDefVal> {
    [
        (
            "NO_DITHERING_MESH_TAG",
            SCENE_MATERIAL_NO_DITHERING_MESH_TAG,
        ),
        (
            "CAMERA_OBSTACLE_MESH_TAG",
            SCENE_MATERIAL_CAMERA_OBSTACLE_MESH_TAG,
        ),
        ("TOON_MESH_TAG", SCENE_MATERIAL_TOON_MESH_TAG),
        ("OUTLINE_MESH_TAG", SCENE_MATERIAL_OUTLINE_MESH_TAG),
        (
            "FACIAL_FEATURE_MESH_TAG",
            SCENE_MATERIAL_FACIAL_FEATURE_MESH_TAG,
        ),
    ]
    .into_iter()
    .map(|(name, tag)| ShaderDefVal::UInt(name.to_owned(), tag))
    .chain({
        let flat = ambient_trilight(0.5)[1];
        [
            ("TOON_LIGHT_UNIT_LUX", LIGHT_UNIT_LUX),
            ("TOON_FACIAL_FLAT_AMBIENT_R", flat.x),
            ("TOON_FACIAL_FLAT_AMBIENT_G", flat.y),
            ("TOON_FACIAL_FLAT_AMBIENT_B", flat.z),
        ]
        .map(|(name, value)| ShaderDefVal::UInt(name.to_owned(), value.to_bits()))
    })
    .collect()
}

/// the reveal encoding, for the pipeline of every shader that imports `reveal_discard`
pub fn reveal_shader_defs() -> [ShaderDefVal; 3] {
    [
        ShaderDefVal::UInt(
            "REVEAL_MESH_TAG_MASK".to_owned(),
            SCENE_MATERIAL_REVEAL_MESH_TAG_MASK,
        ),
        ShaderDefVal::Int(
            "REVEAL_HIDE_MILLIMETRES".to_owned(),
            SCENE_MATERIAL_REVEAL_HIDE_MILLIMETRES,
        ),
        ShaderDefVal::Int(
            "REVEAL_FULL_MILLIMETRES".to_owned(),
            SCENE_MATERIAL_REVEAL_FULL_MILLIMETRES,
        ),
    ]
}

pub trait SceneMaterialExt {
    fn new_unbounded(mat: StandardMaterial) -> Self
    where
        Self: Sized;
}

impl SceneMaterialExt for SceneMaterial {
    fn new_unbounded(mat: StandardMaterial) -> Self
    where
        Self: Sized,
    {
        Self {
            base: mat,
            extension: SceneBound::new_unbounded(),
        }
    }
}

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct SceneBoundKey {
    inverted_scale: bool,
}

impl From<&SceneBound> for SceneBoundKey {
    fn from(value: &SceneBound) -> Self {
        Self {
            inverted_scale: value.inverted_scale,
        }
    }
}

#[derive(Asset, TypePath, Clone, AsBindGroup)]
#[bind_group_data(SceneBoundKey)]
pub struct SceneBound {
    #[uniform(100)]
    pub data: SceneBoundData,
    #[cfg_attr(not(feature = "webgl2"), storage(101, read_only, visibility(fragment)))]
    pub cells: Handle<ShaderStorageBuffer>,
    /// Read only with `SCENE_ALPHA_TEXTURE` set.
    #[texture(102)]
    #[sampler(103)]
    pub alpha_texture: Option<Handle<Image>>,
    /// Meshes that have 1 or 3 negative scale axis require special handling
    /// of the face culling
    pub inverted_scale: bool,
}

impl SceneBound {
    pub fn new(bounds: &SceneBounds, distance: f32) -> Self {
        Self {
            data: SceneBoundData {
                distance,
                ..bounds.data
            },
            cells: bounds.cells.clone(),
            alpha_texture: None,
            inverted_scale: false,
        }
    }

    pub fn new_unbounded() -> Self {
        Self::new(&SceneBounds::unbounded(), 0.0)
    }

    pub fn with_base_opacity(mut self, base_opacity: f32) -> Self {
        self.data.base_opacity = base_opacity;
        self
    }

    pub fn with_capsule_dither(mut self) -> Self {
        self.data.flags |= SCENE_DITHER_CAPSULE;
        self
    }

    pub fn with_plain_emissive(mut self) -> Self {
        self.data.flags |= SCENE_PLAIN_EMISSIVE;
        self
    }

    pub fn with_alpha_texture(mut self, alpha_texture: Handle<Image>) -> Self {
        self.data.flags |= SCENE_ALPHA_TEXTURE;
        self.alpha_texture = Some(alpha_texture);
        self
    }

    /// the material's own rules (emissive, alpha texture) stay
    pub fn rebind(&mut self, bounds: &SceneBounds, distance: f32) {
        let rules = self.data.flags & (SCENE_PLAIN_EMISSIVE | SCENE_ALPHA_TEXTURE);
        self.data = SceneBoundData {
            distance,
            flags: bounds.data.flags | rules,
            ..bounds.data
        };
        self.cells = bounds.cells.clone();
    }
}

pub const SCENE_BOUNDED: u32 = 1;
pub const SCENE_FULL_RECT: u32 = 2;
/// an avatar: the camera-to-player dither fades it as a whole, by its capsule
pub const SCENE_DITHER_CAPSULE: u32 = 4;
/// an SDK material's emissive rules (bound_material.wgsl)
pub const SCENE_PLAIN_EMISSIVE: u32 = 8;
/// the alpha texture is bound
pub const SCENE_ALPHA_TEXTURE: u32 = 16;
/// scenes with a larger bounding box fall back to the bounding box itself
const MAX_SCENE_CELLS: u64 = 1 << 20;
/// empty cell table for unbounded and full-rect scenes, which never read it
pub const EMPTY_SCENE_CELLS: Handle<ShaderStorageBuffer> =
    weak_handle!("7f375a13-2034-4519-b4e9-c570a0fb345b");

/// a scene's bounds in the form the scene-bound shaders read (see scene_bounds.wgsl). built once
/// per scene; the cell table is shared by all the scene's materials.
#[derive(Clone, Debug, PartialEq)]
pub struct SceneBounds {
    data: SceneBoundData,
    cells: Handle<ShaderStorageBuffer>,
}

impl Default for SceneBounds {
    fn default() -> Self {
        Self::unbounded()
    }
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct SceneBoundsKey {
    origin: IVec2,
    size: UVec2,
    rect_height: u32,
    flags: u32,
    cells: AssetId<ShaderStorageBuffer>,
}

impl SceneBounds {
    pub fn unbounded() -> Self {
        Self {
            data: SceneBoundData::default(),
            cells: EMPTY_SCENE_CELLS,
        }
    }

    /// a single rectangle, checked without the cell table
    pub fn rect(region: &BoundRegion) -> Self {
        let (min, max) = region.cells();
        Self::full_rect(min, max, region.height)
    }

    fn full_rect(min: IVec2, max: IVec2, height: f32) -> Self {
        Self {
            data: SceneBoundData {
                origin: min,
                size: (max - min + IVec2::ONE).as_uvec2(),
                rect_height: height,
                flags: SCENE_BOUNDED | SCENE_FULL_RECT,
                ..default()
            },
            cells: EMPTY_SCENE_CELLS,
        }
    }

    /// no regions = unbounded. without `buffers` (or for huge extents) an irregular scene falls
    /// back to its bounding box.
    pub fn new(regions: &[BoundRegion], buffers: Option<&mut Assets<ShaderStorageBuffer>>) -> Self {
        let Some(((min, max), max_height)) = BoundRegion::enclosing(regions) else {
            return Self::unbounded();
        };
        if let [region] = regions {
            return Self::rect(region);
        }

        let size = (max - min + IVec2::ONE).as_uvec2();
        let cell_count = size.x as u64 * size.y as u64;
        let Some(buffers) = buffers.filter(|_| cell_count <= MAX_SCENE_CELLS) else {
            if cell_count > MAX_SCENE_CELLS {
                warn!("scene bounds {size} too large, using the bounding box");
            }
            return Self::full_rect(min, max, max_height);
        };

        // height table indexed from 1; 0 marks cells outside the scene
        let mut heights: Vec<f32> = Vec::new();
        let mut cells = vec![0u16; cell_count as usize];
        for region in regions {
            let index = match heights.iter().position(|h| *h == region.height) {
                Some(index) => index,
                None => {
                    heights.push(region.height);
                    heights.len() - 1
                }
            };
            let (rmin, rmax) = region.cells();
            for z in rmin.y..=rmax.y {
                for x in rmin.x..=rmax.x {
                    let local = (IVec2::new(x, z) - min).as_uvec2();
                    cells[(local.y * size.x + local.x) as usize] = index as u16 + 1;
                }
            }
        }

        if heights.len() == 1 && cells.iter().all(|c| *c != 0) {
            return Self::full_rect(min, max, heights[0]);
        }

        let words = std::iter::once(heights.len() as u32)
            .chain(heights.iter().map(|h| h.to_bits()))
            .chain(
                cells
                    .chunks(2)
                    .map(|pair| pair[0] as u32 | (pair.get(1).copied().unwrap_or(0) as u32) << 16),
            );
        let bytes = words.flat_map(u32::to_le_bytes).collect::<Vec<_>>();
        Self {
            data: SceneBoundData {
                origin: min,
                size,
                rect_height: max_height,
                flags: SCENE_BOUNDED,
                ..default()
            },
            cells: buffers.add(ShaderStorageBuffer::new(
                &bytes,
                RenderAssetUsages::RENDER_WORLD,
            )),
        }
    }

    /// identifies materials built from equal bounds
    pub fn key(&self) -> SceneBoundsKey {
        SceneBoundsKey {
            origin: self.data.origin,
            size: self.data.size,
            rect_height: self.data.rect_height.to_bits(),
            flags: self.data.flags,
            cells: self.cells.id(),
        }
    }
}

mod decl {
    #![expect(
        dead_code,
        reason = "the ShaderType derive emits a never-called check fn per field"
    )]

    use bevy::{
        math::{IVec2, UVec2},
        render::render_resource::ShaderType,
    };
    #[derive(ShaderType, Clone, Copy, Debug, Default)]
    pub struct BoundRegion {
        pub min: u32, // 2x i16
        pub max: u32, // 2x i16
        pub height: f32,
        pub parcel_count: u32,
    }

    /// see scene_bounds.wgsl
    #[derive(ShaderType, Clone, Copy, Debug, Default, PartialEq)]
    pub struct SceneBoundData {
        pub origin: IVec2,
        pub size: UVec2,
        pub rect_height: f32,
        pub flags: u32,
        pub distance: f32,
        /// the opacity the camera-to-player dither fades the material towards: 0 fades out fully
        pub base_opacity: f32,
    }
}
pub use decl::*;

/// The height limit, in metres, of a scene in a World: the parcel-count limit of the largest
/// scene a World allows, 300 × 300 parcels (log2(90001) × 20 = 329.15 m), rounded up.
pub const FIXED_SCENE_HEIGHT: f32 = 330.0;

impl BoundRegion {
    pub fn new(min: IVec2, max: IVec2, parcel_count: u32) -> Self {
        Self {
            min: ((min.x as i16 as u16 as u32) << 16) | (-(max.y + 1) as i16 as u16 as u32),
            max: (((max.x + 1) as i16 as u16 as u32) << 16) | (-min.y as i16 as u16 as u32),
            height: Self::height_for_parcels(parcel_count),
            parcel_count,
        }
    }

    /// the region with this height limit, in metres, in place of its parcel count's
    pub fn with_height(self, height: f32) -> Self {
        Self { height, ..self }
    }

    /// The height limit, in metres, over `parcel_count` parcels.
    pub fn height_for_parcels(parcel_count: u32) -> f32 {
        f32::log2(parcel_count as f32 + 1.0) * 20.0
    }

    /// The rectangle of cells around `regions` (inclusive, as [`Self::cells`] gives them) and
    /// the highest of their height limits. None without regions.
    pub fn enclosing(regions: &[BoundRegion]) -> Option<((IVec2, IVec2), f32)> {
        let (first, rest) = regions.split_first()?;
        Some(rest.iter().fold(
            (first.cells(), first.height),
            |((min, max), height), region| {
                let (region_min, region_max) = region.cells();
                (
                    (min.min(region_min), max.max(region_max)),
                    height.max(region.height),
                )
            },
        ))
    }
}

impl BoundRegion {
    fn unpack_parcel_coords(input: u32) -> IVec2 {
        let x = ((input >> 16) & 0xFFFF) as i32;
        let y = (input & 0xFFFF) as i32;
        IVec2::new(
            if (x & 0x8000) != 0 { x - 0x10000 } else { x },
            if (y & 0x8000) != 0 { y - 0x10000 } else { y },
        )
    }

    pub fn parcel_min(&self) -> IVec2 {
        IVec2::new(
            Self::unpack_parcel_coords(self.min).x,
            -Self::unpack_parcel_coords(self.max).y,
        )
    }

    /// inclusive min/max parcel cells in world orientation (world xz / 16)
    pub fn cells(&self) -> (IVec2, IVec2) {
        let (min, max) = (self.parcel_min(), self.parcel_max());
        (IVec2::new(min.x, -max.y - 1), IVec2::new(max.x, -min.y - 1))
    }

    pub fn parcel_max(&self) -> IVec2 {
        IVec2::new(
            Self::unpack_parcel_coords(self.max).x - 1,
            -Self::unpack_parcel_coords(self.min).y - 1,
        )
    }

    pub fn world_min(&self) -> Vec3 {
        let coords = Self::unpack_parcel_coords(self.min).as_vec2() * 16.0;
        Vec3::new(coords.x, 0.0, coords.y)
    }

    pub fn world_max(&self) -> Vec3 {
        let coords = Self::unpack_parcel_coords(self.max).as_vec2() * 16.0;
        Vec3::new(coords.x, self.height, coords.y)
    }

    pub fn world_size(&self) -> Vec3 {
        self.world_max() - self.world_min()
    }

    pub fn world_midpoint(&self) -> Vec3 {
        (self.world_max() + self.world_min()) * 0.5
    }

    pub fn world_radius(&self) -> f32 {
        (self.world_max() - self.world_min()).length() * 0.5
    }
}

impl MaterialExtension for SceneBound {
    type Base = StandardMaterial;

    fn fragment_shader() -> bevy::render::render_resource::ShaderRef {
        ShaderRef::Path("embedded://shaders/bound_material.wgsl".into())
    }

    fn alpha_mode(base_mode: AlphaMode) -> Option<AlphaMode> {
        Some(match base_mode {
            AlphaMode::Opaque => AlphaMode::Mask(0.0),
            other => other,
        })
    }

    fn prepass_fragment_shader() -> ShaderRef {
        ShaderRef::Path("embedded://shaders/bound_prepass.wgsl".into())
    }

    fn fallback_asset(&self, base: &Self::Base) -> Option<ExtendedMaterial<Self>> {
        let base_color = Color::srgba(1.0, 1.0, 4.0, 1.0);
        let emissive = Color::srgba(1.0, 1.0, 4.0, 1.0);

        Some(ExtendedMaterial::<Self> {
            base: StandardMaterial {
                base_color,
                emissive: emissive.to_linear(),
                double_sided: true,
                cull_mode: base.cull_mode,
                unlit: true,
                alpha_mode: AlphaMode::Blend,
                ..Default::default()
            },
            extension: self.clone(),
        })
    }

    fn specialize(
        _: &bevy::pbr::MaterialExtensionPipeline,
        descriptor: &mut bevy::render::render_resource::RenderPipelineDescriptor,
        layout: &bevy::render::mesh::MeshVertexBufferLayoutRef,
        key: bevy::pbr::MaterialExtensionKey<Self>,
    ) -> Result<(), bevy::render::render_resource::SpecializedMeshPipelineError> {
        let data = key.bind_group_data;

        if descriptor.primitive.cull_mode.is_some() {
            descriptor.primitive.cull_mode = if data.inverted_scale {
                Some(Face::Front)
            } else {
                Some(Face::Back)
            };
        }

        if let Some(fragment) = descriptor.fragment.as_mut() {
            fragment.shader_defs.push(ShaderDefVal::UInt(
                "SHOW_OUTSIDE_BOUNDS_MESH_TAG".to_owned(),
                SCENE_MATERIAL_SHOW_OUTSIDE_BOUNDS_MESH_TAG,
            ));
            fragment.shader_defs.extend(avatar_style_shader_defs());
            fragment.shader_defs.extend(reveal_shader_defs());
            // the material has no cell table (`webgl2`), so scene_bounds.wgsl leaves it out too
            if cfg!(feature = "webgl2") {
                fragment.shader_defs.push("NO_SCENE_CELLS".into());
            }

            if data.inverted_scale {
                fragment.shader_defs.push("INVERTED_SCALE".into());
            }

            // a texture on the second UV set of a mesh without one reads the first, as bevy's
            // main pass does; bevy's prepass alpha test and our normal map read would otherwise
            // read the missing set
            if !layout.0.contains(Mesh::ATTRIBUTE_UV_1) {
                fragment.shader_defs.retain(|def| {
                    !matches!(def, ShaderDefVal::Bool(name, _)
                        if name.starts_with("STANDARD_MATERIAL_") && name.ends_with("_UV_B"))
                });
            }
        }
        Ok(())
    }

    // fn shadow_material_key(&self, base_key: Option<u64>) -> Option<u64> {
    //     base_key.map(|_| (((self.bounds.x as i64) << 32) | (self.bounds.y as i64)) as u64)
    // }
}

impl ImposterBakeMaterialExtension for SceneBound {
    fn imposter_fragment_shader() -> bevy::render::render_resource::ShaderRef {
        "embedded://shaders/bound_material_baker.wgsl".into()
    }
}
pub struct SceneBoundPlugin;

impl Plugin for SceneBoundPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            MaterialExtPlugin::<SceneMaterial>::default(),
            HighlightPlugin,
        ));

        app.init_resource::<InvertedMaterials>();
        // Default false; the app entry overrides it.
        app.init_resource::<ShowOutOfBounds>();

        app.add_observer(new_material);
        app.add_systems(
            PostUpdate,
            (new_materials, clear_old_materials)
                .chain()
                .after(TransformSystem::TransformPropagate),
        );
    }

    fn finish(&self, app: &mut App) {
        // not present when running without rendering
        if let Some(mut buffers) = app
            .world_mut()
            .get_resource_mut::<Assets<ShaderStorageBuffer>>()
        {
            // header + one (unused) cell word: a runtime-sized array binding can't be empty
            buffers.insert(
                EMPTY_SCENE_CELLS.id(),
                ShaderStorageBuffer::new(&[0; 8], RenderAssetUsages::RENDER_WORLD),
            );
        }
    }
}

pub struct MaterialExtPlugin<T: ImposterBakeMaterial> {
    _material_phantom: PhantomData<T>,
}

impl<T: ImposterBakeMaterial> Default for MaterialExtPlugin<T> {
    fn default() -> Self {
        Self {
            _material_phantom: PhantomData,
        }
    }
}

impl<T: ImposterBakeMaterial> Plugin for MaterialExtPlugin<T>
where
    MaterialPlugin<T>: Plugin,
    ImposterBakeMaterialPlugin<T>: Plugin,
{
    fn build(&self, app: &mut App) {
        app.register_required_components::<MeshMaterial3d<T>, MeshTag>();

        app.add_plugins(MaterialPlugin::<T>::default());
        let preview_mode = app
            .world()
            .get_resource::<PreviewMode>()
            .is_some_and(|p| p.is_preview);
        if !preview_mode {
            app.add_plugins(ImposterBakeMaterialPlugin::<T>::default());
        }

        app.add_observer(update_show_outside_bounds::<T>);
        app.add_observer(scene_material_removed::<T>);
    }
}

fn update_show_outside_bounds<T: Material>(
    trigger: Trigger<OnInsert, MeshMaterial3d<T>>,
    mut meshes: Query<&mut MeshTag, With<MeshMaterial3d<T>>>,
    show_oob: Res<ShowOutOfBounds>,
) {
    // Tag the mesh to render out-of-bounds instead of being culled.
    if !show_oob.0 {
        return;
    }

    let entity = trigger.target();
    let Ok(mut mesh_tag) = meshes.get_mut(entity) else {
        return;
    };
    mesh_tag.0 |= SCENE_MATERIAL_SHOW_OUTSIDE_BOUNDS_MESH_TAG;
}

#[derive(Debug, Default, Resource, Deref, DerefMut)]
pub struct InvertedMaterials {
    pub mapping: HashMap<Handle<SceneMaterial>, Handle<SceneMaterial>>,
}

#[derive(Debug, Default, Component)]
#[component(storage = "SparseSet")]
pub struct AddedSceneMaterial;

fn new_material(trigger: Trigger<OnInsert, MeshMaterial3d<SceneMaterial>>, mut commands: Commands) {
    commands.entity(trigger.target()).insert(AddedSceneMaterial);
}

fn new_materials(
    mut commands: Commands,
    materials: Populated<
        (Entity, &GlobalTransform, &MeshMaterial3d<SceneMaterial>),
        With<AddedSceneMaterial>,
    >,
    mut inverted_materials: ResMut<InvertedMaterials>,
    mut scene_material_assets: ResMut<Assets<SceneMaterial>>,
) {
    for (entity, gt, mesh_material) in materials.into_inner() {
        commands.entity(entity).try_remove::<AddedSceneMaterial>();

        if gt.is_inverted() {
            match inverted_materials.entry(mesh_material.clone_weak()) {
                Entry::Vacant(vacant) => {
                    let Some(scene_material_asset) =
                        scene_material_assets.get(mesh_material.id()).filter(
                            |scene_material_asset| !scene_material_asset.extension.inverted_scale,
                        )
                    else {
                        debug!("{entity}'s material was invalid");
                        continue;
                    };

                    let mut inverted_material = scene_material_asset.clone();
                    inverted_material.extension.inverted_scale = true;
                    let new_handle = scene_material_assets.add(inverted_material);

                    vacant.insert(new_handle.clone());
                    commands
                        .entity(entity)
                        .try_insert(MeshMaterial3d(new_handle));
                }
                Entry::Occupied(occupied) => {
                    commands
                        .entity(entity)
                        .try_insert(MeshMaterial3d(occupied.get().clone()));
                }
            }
        }
    }
}

fn clear_old_materials(
    mut inverted_materials: ResMut<InvertedMaterials>,
    scene_material_assets: Res<Assets<SceneMaterial>>,
) {
    let len = inverted_materials.len();
    inverted_materials.retain(|k, _| scene_material_assets.contains(k.id()));
    if inverted_materials.len() != len {
        debug!(
            "Cleared {} inverted materials.",
            len - inverted_materials.len()
        );
    }
}

fn scene_material_removed<T: Material>(
    trigger: Trigger<OnRemove, MeshMaterial3d<T>>,
    mut commands: Commands,
) {
    commands.entity(trigger.target()).try_remove::<MeshTag>();
}

#[cfg(test)]
mod test {
    use bevy::math::{IVec2, Vec2, Vec3, Vec3Swizzles};

    use crate::{
        reveal_mesh_tag, unrevealed_mesh_tag, BoundRegion, FIXED_SCENE_HEIGHT,
        SCENE_MATERIAL_REVEAL_FULL_HEIGHT, SCENE_MATERIAL_REVEAL_HIDE_HEIGHT,
        SCENE_MATERIAL_REVEAL_MESH_TAG_MASK,
    };

    /// the reveal height a shader reads back from a tag (reveal_discard in
    /// bound_material_effect.wgsl)
    fn decode(tag: u32) -> Option<f32> {
        let step = tag & SCENE_MATERIAL_REVEAL_MESH_TAG_MASK;
        (step != 0).then(|| {
            SCENE_MATERIAL_REVEAL_HIDE_HEIGHT
                + (SCENE_MATERIAL_REVEAL_FULL_HEIGHT - SCENE_MATERIAL_REVEAL_HIDE_HEIGHT)
                    * (step - 1) as f32
                    / 254.0
        })
    }

    #[test]
    fn reveal_takes_the_tag_low_byte() {
        let flags = 0xFFFFFF00;
        assert_eq!(
            reveal_mesh_tag(flags, SCENE_MATERIAL_REVEAL_HIDE_HEIGHT),
            flags | 1
        );
        assert_eq!(
            reveal_mesh_tag(flags, SCENE_MATERIAL_REVEAL_FULL_HEIGHT),
            flags | 255
        );
        // heights outside the range clamp to its ends
        assert_eq!(reveal_mesh_tag(flags, -10.0), flags | 1);
        assert_eq!(reveal_mesh_tag(flags, 10.0), flags | 255);
        assert_eq!(unrevealed_mesh_tag(flags | 77), flags);
        assert_eq!(decode(unrevealed_mesh_tag(flags | 77)), None);
    }

    #[test]
    fn reveal_round_trips_within_a_step() {
        let range = SCENE_MATERIAL_REVEAL_FULL_HEIGHT - SCENE_MATERIAL_REVEAL_HIDE_HEIGHT;
        let step = range / 254.0;
        assert!(step < 0.01, "a step is {step} m");
        let mut previous = 0;
        for i in 0..=2100 {
            let height = -0.05 + i as f32 * 0.001;
            let tag = reveal_mesh_tag(0, height);
            assert!(tag >= previous, "{height} m encodes below {previous}");
            previous = tag;
            let decoded = decode(tag).unwrap();
            assert!(
                (decoded - height).abs() <= step / 2.0 + 1e-5,
                "{height} m decodes as {decoded} m"
            );
        }
    }

    #[test]
    fn a_rebound_material_keeps_its_rules() {
        use bevy::{asset::Handle, math::UVec2};

        use crate::{
            SceneBound, SceneBounds, EMPTY_SCENE_CELLS, SCENE_ALPHA_TEXTURE, SCENE_BOUNDED,
            SCENE_FULL_RECT, SCENE_PLAIN_EMISSIVE,
        };

        let region = BoundRegion::new(IVec2::new(3, -2), IVec2::new(3, -2), 1);
        let bounds = SceneBounds::rect(&region);

        let mut sdk = SceneBound::new_unbounded()
            .with_plain_emissive()
            .with_alpha_texture(Handle::default())
            .with_capsule_dither()
            .with_base_opacity(0.5);
        sdk.rebind(&bounds, 1.5);
        assert_eq!(
            sdk.data.flags,
            SCENE_BOUNDED | SCENE_FULL_RECT | SCENE_PLAIN_EMISSIVE | SCENE_ALPHA_TEXTURE
        );
        let (min, _) = region.cells();
        assert_eq!(sdk.data.origin, min);
        assert_eq!(sdk.data.size, UVec2::ONE);
        assert_eq!(sdk.data.rect_height, region.height);
        assert_eq!(sdk.data.distance, 1.5);
        assert_eq!(sdk.data.base_opacity, 0.0);
        assert!(sdk.alpha_texture.is_some());
        assert_eq!(sdk.cells, EMPTY_SCENE_CELLS);

        let mut gltf = SceneBound::new_unbounded();
        gltf.rebind(&bounds, 1.5);
        assert_eq!(gltf.data, SceneBound::new(&bounds, 1.5).data);
    }

    #[test]
    fn test_bounds() {
        for x in [-10, 0, 10] {
            for y in [-10, 0, 10] {
                let region = BoundRegion::new(IVec2::new(x, y), IVec2::new(x, y), 1);

                println!(
                    "[{},{}] -> {:x},{:x} -> {}, {}",
                    x,
                    y,
                    region.min,
                    region.max,
                    region.world_min(),
                    region.world_max()
                );
                assert_eq!(
                    region.world_min(),
                    Vec3::new(x as f32 * 16.0, 0.0, (-y - 1) as f32 * 16.0)
                );
                assert_eq!(
                    region.world_max(),
                    Vec3::new((x + 1) as f32 * 16.0, 20.0, -y as f32 * 16.0)
                );
                assert_eq!(region.parcel_min(), IVec2::new(x, y));
                assert_eq!(region.parcel_max(), IVec2::new(x, y));
            }
        }
    }

    /// The imposter bake (`bound_material_baker.wgsl`) follows the forward shader on an SDK
    /// material: the alpha texture's brightness scales the alpha, and a plain emissive colour is
    /// not tinted by the base colour. Bakes four quads in one tile and reads the tile back. Needs
    /// a GPU, so `#[ignore]`d and skipped by CI:
    ///
    /// ```text
    /// cargo test -p scene_material bake_follows_the_sdk_rules -- --ignored --nocapture
    /// ```
    #[test]
    #[ignore = "needs a GPU; run with --ignored"]
    fn bake_follows_the_sdk_rules() {
        use std::{
            sync::{Arc, Mutex},
            time::{Duration, Instant},
        };

        use bevy::{
            asset::RenderAssetUsages,
            prelude::*,
            render::{
                render_resource::{Extent3d, TextureDimension, TextureFormat},
                storage::ShaderStorageBuffer,
            },
        };
        use boimp::{
            bake::ImposterBakeMaterialPlugin, ImposterBakeBundle, ImposterBakeCamera,
            ImposterBakePlugin,
        };
        use common::sync::MutexExt;
        use pipeline_test::{compile_pipelines, fragment_pipelines, headless_app};

        use crate::{BoundRegion, SceneBound, SceneBounds, SceneMaterial, EMPTY_SCENE_CELLS};

        const TILE: u32 = 64;

        let mut app = headless_app();
        app.add_plugins((
            ImposterBakePlugin,
            MaterialPlugin::<SceneMaterial>::default(),
            ImposterBakeMaterialPlugin::<SceneMaterial>::default(),
        ));
        app.finish();
        app.cleanup();

        let world = app.world_mut();
        // the unbounded scene's cell table, as SceneBoundPlugin inserts it
        world.resource_mut::<Assets<ShaderStorageBuffer>>().insert(
            EMPTY_SCENE_CELLS.id(),
            ShaderStorageBuffer::new(&[0; 8], RenderAssetUsages::RENDER_WORLD),
        );

        let mut images = world.resource_mut::<Assets<Image>>();
        let mut texel = |rgba: [u8; 4]| {
            images.add(Image::new_fill(
                Extent3d::default(),
                TextureDimension::D2,
                &rgba,
                TextureFormat::Rgba8UnormSrgb,
                RenderAssetUsages::RENDER_WORLD,
            ))
        };
        let (black, white) = (texel([0, 0, 0, 255]), texel([255, 255, 255, 255]));

        // the quads reach 0.9 m outside this region, within the distance
        let rebound = |mut extension: SceneBound| {
            let region = BoundRegion::new(IVec2::ZERO, IVec2::ZERO, 1);
            extension.rebind(&SceneBounds::rect(&region), 1.0);
            extension
        };

        // as `MaterialDefinition` builds them: an SDK unlit material with an alpha texture blends
        let unlit = |alpha_texture: Handle<Image>| SceneMaterial {
            base: StandardMaterial {
                unlit: true,
                alpha_mode: AlphaMode::Blend,
                ..Default::default()
            },
            extension: rebound(SceneBound::new_unbounded().with_alpha_texture(alpha_texture)),
        };
        let emissive = |extension: SceneBound| SceneMaterial {
            base: StandardMaterial {
                base_color: Color::linear_rgb(1.0, 0.0, 0.0),
                emissive: LinearRgba::rgb(0.4, 0.4, 0.4),
                ..Default::default()
            },
            extension: rebound(extension),
        };
        let mut materials = world.resource_mut::<Assets<SceneMaterial>>();
        let quads = [
            (Vec2::new(-0.5, 0.5), materials.add(unlit(black))),
            (Vec2::new(0.5, 0.5), materials.add(unlit(white))),
            (
                Vec2::new(-0.5, -0.5),
                materials.add(emissive(SceneBound::new_unbounded().with_plain_emissive())),
            ),
            (
                Vec2::new(0.5, -0.5),
                materials.add(emissive(SceneBound::new_unbounded())),
            ),
        ];
        let quads_at = quads.each_ref().map(|(position, _)| *position);
        let quad = world
            .resource_mut::<Assets<Mesh>>()
            .add(Rectangle::new(0.8, 0.8));
        for (position, material) in quads {
            world.spawn((
                Mesh3d(quad.clone()),
                MeshMaterial3d(material),
                Transform::from_translation(position.extend(0.0)),
            ));
        }

        // one view, facing the quads: the tile spans x and y from -1 to 1
        let baked = Arc::new(Mutex::new(None));
        let mut camera = ImposterBakeCamera {
            radius: 1.0,
            grid_size: 1,
            tile_size: TILE,
            multisample: 1,
            manual_camera_transforms: Some(vec![GlobalTransform::from(
                Transform::from_xyz(0.0, 0.0, 1.0).looking_at(Vec3::ZERO, Vec3::Y),
            )]),
            ..Default::default()
        };
        camera.set_callback({
            let baked = baked.clone();
            move |image| *baked.lock_ignore_poison() = Some(image)
        });
        world.spawn(ImposterBakeBundle {
            camera,
            ..Default::default()
        });

        compile_pipelines(&mut app);
        assert!(
            fragment_pipelines(&app, "embedded://shaders/bound_material_baker.wgsl") >= 1,
            "no bound_material_baker.wgsl pipeline compiled"
        );

        let started = Instant::now();
        let image = loop {
            app.update();
            if let Some(image) = baked.lock_ignore_poison().take() {
                break image;
            }
            assert!(
                started.elapsed() < Duration::from_secs(120),
                "the bake never finished"
            );
            std::thread::sleep(Duration::from_millis(5));
        };

        // the first word of a texel, as boimp's shared.wgsl packs it: 5 bits each of red, green,
        // blue and alpha from bit 0, and the flags (1 unlit, 2 emissive) from bit 28
        let data = image.data.as_ref().unwrap();
        let texel = |position: Vec2| {
            let pixel = ((position * Vec2::new(1.0, -1.0) + 1.0) * 0.5 * TILE as f32).as_uvec2();
            let offset = ((pixel.y * TILE + pixel.x) * 8) as usize;
            let word = u32::from_le_bytes(data[offset..offset + 4].try_into().unwrap());
            let channel = |shift: u32| (word >> shift) & 31;
            (
                [channel(0), channel(5), channel(10)],
                channel(15),
                word >> 28,
            )
        };
        let [cut, kept, plain, tinted] = quads_at.map(texel);

        assert_eq!(cut.1, 0, "a black alpha texture leaves nothing: {cut:?}");
        assert_eq!(kept, ([31; 3], 31, 1), "a white alpha texture keeps it all");
        assert_eq!(plain, ([31; 3], 31, 2), "an SDK emissive colour is its own");
        assert_eq!(
            tinted,
            ([31, 0, 0], 31, 2),
            "a glTF emissive factor takes the base colour"
        );
    }

    // cpu ports of scene_bounds.wgsl (new) and the previous per-rect loop (old)
    fn new_amount(
        bounds: &super::SceneBounds,
        buffers: &bevy::asset::Assets<super::ShaderStorageBuffer>,
        distance: f32,
        pos: Vec3,
    ) -> f32 {
        let d = &bounds.data;
        let words: Vec<u32> = buffers
            .get(&bounds.cells)
            .and_then(|b| b.data.as_ref())
            .map(|bytes| {
                bytes
                    .chunks(4)
                    .map(|c| u32::from_le_bytes(c.try_into().unwrap()))
                    .collect()
            })
            .unwrap_or_default();
        let region_at = |cell: IVec2| -> u32 {
            let local = cell - d.origin;
            if local.x < 0
                || local.y < 0
                || local.x as u32 >= d.size.x
                || local.y as u32 >= d.size.y
            {
                return 0;
            }
            let ix = local.y as u32 * d.size.x + local.x as u32;
            (words[(1 + words[0] + (ix >> 1)) as usize] >> ((ix & 1) * 16)) & 0xFFFF
        };
        let height = |r: u32| f32::from_bits(words[r as usize]);

        if d.flags & super::SCENE_BOUNDED == 0 {
            return 0.0;
        }
        let p = pos.xz();
        if d.flags & super::SCENE_FULL_RECT != 0 {
            let min_wp = d.origin.as_vec2() * 16.0;
            let max_wp = min_wp + d.size.as_vec2() * 16.0;
            let o = (p.clamp(min_wp, max_wp) - p).abs();
            return o.x.max(o.y).max(pos.y - d.rect_height);
        }
        let cell = (p / 16.0).floor().as_ivec2();
        let r = region_at(cell);
        if r != 0 {
            return (pos.y - height(r)).max(0.0);
        }
        let local = p - cell.as_vec2() * 16.0;
        let step = IVec2::new(
            if local.x >= 8.0 { 1 } else { -1 },
            if local.y >= 8.0 { 1 } else { -1 },
        );
        let edge = Vec2::new(
            if local.x >= 8.0 {
                16.0 - local.x
            } else {
                local.x
            },
            if local.y >= 8.0 {
                16.0 - local.y
            } else {
                local.y
            },
        );
        let reach = distance.max(0.05);
        let mut amount = 9999.0f32;
        let mut check = |off: IVec2, dist: f32| {
            let r = region_at(cell + off);
            if r != 0 {
                amount = amount.min(dist.max(pos.y - height(r)));
            }
        };
        if edge.x < reach {
            check(IVec2::new(step.x, 0), edge.x);
        }
        if edge.y < reach {
            check(IVec2::new(0, step.y), edge.y);
            if edge.x < reach {
                check(step, edge.x.max(edge.y));
            }
        }
        amount
    }

    fn old_amount(regions: &[BoundRegion], pos: Vec3) -> f32 {
        let unpack = |packed: u32| {
            let v = |x: i32| if x & 0x8000 != 0 { x - 0x10000 } else { x };
            Vec2::new(
                v(((packed >> 16) & 0xFFFF) as i32) as f32 * 16.0,
                v((packed & 0xFFFF) as i32) as f32 * 16.0,
            )
        };
        let (mut outside, mut nearest, mut nearest_height) = (9999.0f32, 9999.0f32, 9999.0f32);
        for r in regions {
            let o = (pos.xz().clamp(unpack(r.min), unpack(r.max)) - pos.xz()).abs();
            let distance = o.x.max(o.y);
            if distance < nearest {
                nearest = distance;
                nearest_height = r.height;
            }
            outside = outside.min(distance);
        }
        outside.max((pos.y - nearest_height).max(0.0))
    }

    #[test]
    fn enclosing_is_the_rectangle_around_the_regions_and_their_highest_limit() {
        assert!(BoundRegion::enclosing(&[]).is_none());

        // parcels (0, 0) and (2, 3), each a region of its own, the second with the higher limit
        let regions = [
            BoundRegion::new(IVec2::ZERO, IVec2::ZERO, 1),
            BoundRegion::new(IVec2::new(2, 3), IVec2::new(2, 3), 3),
        ];
        let ((min, max), height) = BoundRegion::enclosing(&regions).unwrap();
        // cells are world xz / 16, so parcel (x, y) is cell (x, -y - 1)
        assert_eq!((min, max), (IVec2::new(0, -4), IVec2::new(2, -1)));
        assert_eq!(height, BoundRegion::height_for_parcels(3));
    }

    #[test]
    fn parcel_index_matches_rect_loop() {
        use common::bounds_calc::scene_regions;

        // an L with a hole, a disconnected pair (different height), and a plain rectangle
        let shapes: Vec<Vec<IVec2>> = vec![
            (0..4)
                .flat_map(|x| (0..4).map(move |y| IVec2::new(x - 20, y + 30)))
                .filter(|p| *p != IVec2::new(-19, 31) && !(p.x >= -18 && p.y >= 32))
                .chain([IVec2::new(-10, 40), IVec2::new(-10, 41)])
                .collect(),
            (0..3)
                .flat_map(|x| (0..2).map(move |y| IVec2::new(x + 5, y - 7)))
                .collect(),
        ];

        let mut seed = 12345u32;
        let mut rand = move || {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            (seed >> 8) as f32 / (1 << 24) as f32
        };

        for parcels in shapes {
            let regions = scene_regions(parcels.iter().copied())
                .into_iter()
                .map(|r| BoundRegion::new(r.min, r.max, r.count))
                .collect::<Vec<_>>();
            let mut buffers = bevy::asset::Assets::<super::ShaderStorageBuffer>::default();
            let bounds = super::SceneBounds::new(&regions, Some(&mut buffers));

            let (min, max) = regions
                .iter()
                .map(BoundRegion::cells)
                .fold((IVec2::MAX, IVec2::MIN), |(a, b), (c, d)| {
                    (a.min(c), b.max(d))
                });
            let lo = (min - IVec2::ONE).as_vec2() * 16.0;
            let hi = (max + IVec2::splat(2)).as_vec2() * 16.0;
            for distance in [0.0, 2.0] {
                let reach = f32::max(distance, 0.05);
                for _ in 0..20000 {
                    let xz = lo + (hi - lo) * Vec2::new(rand(), rand());
                    let pos = Vec3::new(xz.x, rand() * 80.0 - 10.0, xz.y);
                    let old = old_amount(&regions, pos);
                    let new = new_amount(&bounds, &buffers, distance, pos);
                    if old < reach {
                        assert!((old - new).abs() < 1e-4, "{pos} old {old} new {new}");
                    } else {
                        assert!(new >= reach, "{pos} old {old} new {new}");
                    }
                }
            }
        }
    }

    #[test]
    fn bounds_clip_at_the_height_their_regions_carry() {
        use common::bounds_calc::scene_regions;

        // one parcel (a single rectangle), and an L with a parcel apart (the cell table)
        let shapes = [
            vec![IVec2::ZERO],
            vec![IVec2::ZERO, IVec2::X, IVec2::Y, IVec2::splat(5)],
        ];

        for parcels in shapes {
            let regions = scene_regions(parcels.iter().copied())
                .into_iter()
                .map(|r| BoundRegion::new(r.min, r.max, r.count).with_height(FIXED_SCENE_HEIGHT))
                .collect::<Vec<_>>();
            let mut buffers = bevy::asset::Assets::<super::ShaderStorageBuffer>::default();
            let bounds = super::SceneBounds::new(&regions, Some(&mut buffers));

            for parcel in parcels {
                let centre = Vec3::new(
                    parcel.x as f32 * 16.0 + 8.0,
                    0.0,
                    -parcel.y as f32 * 16.0 - 8.0,
                );
                let below = centre.with_y(FIXED_SCENE_HEIGHT - 1.0);
                let above = centre.with_y(FIXED_SCENE_HEIGHT + 1.0);
                assert_eq!(new_amount(&bounds, &buffers, 0.0, below), 0.0, "{parcel}");
                assert_eq!(new_amount(&bounds, &buffers, 0.0, above), 1.0, "{parcel}");
            }
        }
    }

    /// The forward shader (`bound_material.wgsl`) compiles with a normal map where it reads its
    /// normal from the prepass: a mesh with uvs and tangents, in front of a camera with a normal
    /// prepass and no MSAA, as the player's camera is while the avatar outline is on. Needs a
    /// GPU, so `#[ignore]`d and skipped by CI:
    ///
    /// ```text
    /// cargo test -p scene_material normal_map_compiles_with_prepass_normals -- --ignored --nocapture
    /// ```
    #[test]
    #[ignore = "needs a GPU; run with --ignored"]
    fn normal_map_compiles_with_prepass_normals() {
        use bevy::{
            asset::RenderAssetUsages,
            core_pipeline::prepass::{DepthPrepass, NormalPrepass},
            prelude::*,
            render::{
                camera::RenderTarget,
                render_resource::{
                    CachedPipelineState, Extent3d, PipelineCache, PipelineDescriptor,
                    TextureDimension, TextureFormat,
                },
                storage::ShaderStorageBuffer,
                RenderApp,
            },
        };
        use pipeline_test::{compile_pipelines, headless_app, render_target};

        use crate::{SceneBound, SceneMaterial, EMPTY_SCENE_CELLS};

        let mut app = headless_app();
        app.add_plugins(MaterialPlugin::<SceneMaterial>::default());
        app.finish();
        app.cleanup();

        let target = render_target(&mut app);
        let world = app.world_mut();
        // the unbounded scene's cell table, as SceneBoundPlugin inserts it
        world.resource_mut::<Assets<ShaderStorageBuffer>>().insert(
            EMPTY_SCENE_CELLS.id(),
            ShaderStorageBuffer::new(&[0; 8], RenderAssetUsages::RENDER_WORLD),
        );

        let normal_map = world.resource_mut::<Assets<Image>>().add(Image::new_fill(
            Extent3d::default(),
            TextureDimension::D2,
            &[128, 128, 255, 255],
            TextureFormat::Rgba8Unorm,
            RenderAssetUsages::RENDER_WORLD,
        ));
        let mut cube = Mesh::from(Cuboid::new(0.5, 0.5, 0.5));
        cube.generate_tangents().unwrap();
        let cube = world.resource_mut::<Assets<Mesh>>().add(cube);
        let material = world
            .resource_mut::<Assets<SceneMaterial>>()
            .add(SceneMaterial {
                base: StandardMaterial {
                    normal_map_texture: Some(normal_map),
                    ..Default::default()
                },
                extension: SceneBound::new_unbounded(),
            });

        world.spawn((
            Camera3d::default(),
            Camera {
                target: RenderTarget::Image(target.into()),
                ..default()
            },
            Msaa::Off,
            DepthPrepass,
            NormalPrepass,
            Transform::from_xyz(0.0, 0.0, 4.0).looking_at(Vec3::ZERO, Vec3::Y),
        ));
        world.spawn((Mesh3d(cube), MeshMaterial3d(material)));

        compile_pipelines(&mut app);

        let shader = app
            .world()
            .resource::<AssetServer>()
            .get_handle::<Shader>("embedded://shaders/bound_material.wgsl")
            .expect("bound_material.wgsl never loaded")
            .id();
        let cache = app.sub_app(RenderApp).world().resource::<PipelineCache>();
        let compiled = cache.pipelines().any(|pipeline| {
            let PipelineDescriptor::RenderPipelineDescriptor(descriptor) = &pipeline.descriptor
            else {
                return false;
            };
            let Some(fragment) = &descriptor.fragment else {
                return false;
            };
            let has = |def: &str| fragment.shader_defs.iter().any(|d| *d == def.into());
            fragment.shader.id() == shader
                && matches!(pipeline.state, CachedPipelineState::Ok(_))
                && has("LOAD_PREPASS_NORMALS")
                && has("VERTEX_TANGENTS")
                && has("STANDARD_MATERIAL_NORMAL_MAP")
        });
        assert!(
            compiled,
            "no bound_material.wgsl pipeline compiled with a normal map and prepass normals"
        );
    }
}
