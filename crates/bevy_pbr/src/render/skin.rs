use core::mem::{self, size_of};
use core::ops::Range;
use std::sync::OnceLock;

use bevy_asset::{prelude::AssetChanged, Assets};
use bevy_ecs::prelude::*;
use bevy_math::Mat4;
use bevy_platform::collections::hash_map::Entry;
use bevy_render::render_resource::{Buffer, BufferDescriptor};
use bevy_render::sync_world::{MainEntity, MainEntityHashMap, MainEntityHashSet};
use bevy_render::{
    batching::NoAutomaticBatching,
    mesh::skinning::{SkinnedMesh, SkinnedMeshInverseBindposes},
    render_resource::BufferUsages,
    renderer::{RenderCapabilities, RenderDevice, RenderQueue},
    settings::WgpuLimits,
    view::ViewVisibility,
    Extract,
};
use bevy_transform::prelude::GlobalTransform;
use offset_allocator::{Allocation, Allocator};
use smallvec::SmallVec;
use tracing::error;

/// Maximum number of joints supported for skinned meshes.
///
/// It is used to allocate buffers.
/// The correctness of the value depends on the GPU/platform.
/// The current value is chosen because it is guaranteed to work everywhere.
/// To allow for bigger values, a check must be made for the limits
/// of the GPU at runtime, which would mean not using consts anymore.
pub const MAX_JOINTS: usize = 256;

/// The total number of joints we support.
///
/// This is 256 GiB worth of joint matrices, which we will never hit under any
/// reasonable circumstances.
const MAX_TOTAL_JOINTS: u32 = 1024 * 1024 * 1024;

/// The number of joints that we allocate at a time.
///
/// Some hardware requires that uniforms be allocated on 256-byte boundaries, so
/// we need to allocate 4 64-byte matrices at a time to satisfy alignment
/// requirements.
const JOINTS_PER_ALLOCATION_UNIT: u32 = (256 / size_of::<Mat4>()) as u32;

/// The maximum ratio of the number of entities whose transforms changed to the
/// total number of joints before we re-extract all joints.
///
/// We use this as a heuristic to decide whether it's worth switching over to
/// fine-grained detection to determine which skins need extraction. If the
/// number of changed entities is over this threshold, we skip change detection
/// and simply re-extract the transforms of all joints.
const JOINT_EXTRACTION_THRESHOLD_FACTOR: f64 = 0.25;

/// The location of the first joint matrix in the skin uniform buffer.
#[derive(Clone, Copy)]
pub struct SkinByteOffset {
    /// The byte offset of the first joint matrix.
    pub byte_offset: u32,
}

impl SkinByteOffset {
    /// Index to be in address space based on the size of a skin uniform.
    const fn from_index(index: usize) -> Self {
        SkinByteOffset {
            byte_offset: (index * size_of::<Mat4>()) as u32,
        }
    }

    /// Returns this skin index in elements (not bytes).
    ///
    /// Each element is a 4x4 matrix.
    pub fn index(&self) -> u32 {
        self.byte_offset / size_of::<Mat4>() as u32
    }
}

/// The GPU buffers containing joint matrices for all skinned meshes.
///
/// This is double-buffered: we store the joint matrices of each mesh for the
/// previous frame in addition to those of each mesh for the current frame. This
/// is for motion vector calculation. Every frame, we swap buffers and overwrite
/// the joint matrix buffer from two frames ago with the data for the current
/// frame.
///
/// Where skins use storage buffers, the double-buffered matrices are the world
/// matrices of the joint entities, one per distinct joint entity regardless of
/// how many skins reference it, and slot 0 is always the identity. A separate,
/// static skin joints buffer holds one entry per joint of each skin: the
/// inverse bindpose with the joint's slot stored as a number in `x_axis.w`. The vertex shader multiplies the two. Where skins use uniform
/// buffers, the double-buffered matrices are the final skinning matrices of
/// each skin and the skin joints buffer is unused.
///
/// Notes on implementation: see comment on top of the `extract_skins` system.
#[derive(Resource)]
pub struct SkinUniforms {
    /// The CPU-side buffer that stores the joint matrices for skinned meshes in
    /// the current frame.
    pub current_staging_buffer: Vec<Mat4>,
    /// The GPU-side buffer that stores the joint matrices for skinned meshes in
    /// the current frame.
    pub current_buffer: Buffer,
    /// The GPU-side buffer that stores the joint matrices for skinned meshes in
    /// the previous frame.
    pub prev_buffer: Buffer,
    /// The CPU-side bookkeeping of joint slots and skin joint entries (storage
    /// buffers only).
    storage: StorageSkins,
    /// The GPU-side buffer of per-skin inverse bindposes and joint slots
    /// (storage buffers only). It is only written when a skin is added or one of
    /// its joints gains a transform.
    pub skin_joints_buffer: Buffer,
    /// Whether skins use uniform buffers on this platform.
    use_uniform_buffers: bool,
    /// The offset allocator that manages the placement of the joints within the
    /// skin joints buffer, or [`Self::current_buffer`] with uniform buffers.
    allocator: Allocator,
    /// Allocation information that we keep about each skin.
    skin_uniform_info: MainEntityHashMap<SkinUniformInfo>,
    /// Maps each joint entity to the skins it's associated with (uniform
    /// buffers only).
    ///
    /// We use this in conjunction with change detection to only update the
    /// skins that need updating each frame.
    ///
    /// Note that conceptually this is a hash map of sets, but we use a
    /// [`SmallVec`] to avoid allocations for the vast majority of the cases in
    /// which each bone belongs to exactly one skin.
    joint_to_skins: MainEntityHashMap<SmallVec<[MainEntity; 1]>>,
    /// The total number of joints in the scene (uniform buffers only).
    ///
    /// We use this as part of our heuristic to decide whether to use
    /// fine-grained change detection.
    total_joints: usize,
}

impl FromWorld for SkinUniforms {
    fn from_world(world: &mut World) -> Self {
        let device = world.resource::<RenderDevice>();
        let use_uniform_buffers = skins_use_uniform_buffers(device);
        let buffer_usages = (if use_uniform_buffers {
            BufferUsages::UNIFORM
        } else {
            BufferUsages::STORAGE
        }) | BufferUsages::COPY_DST;

        // Create the current and previous buffer with the minimum sizes.
        //
        // These will be swapped every frame.
        let current_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("skin uniform buffer"),
            size: MAX_JOINTS as u64 * size_of::<Mat4>() as u64,
            usage: buffer_usages,
            mapped_at_creation: false,
        });
        let prev_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("skin uniform buffer"),
            size: MAX_JOINTS as u64 * size_of::<Mat4>() as u64,
            usage: buffer_usages,
            mapped_at_creation: false,
        });

        let skin_joints_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("skin joints buffer"),
            size: MAX_JOINTS as u64 * size_of::<Mat4>() as u64,
            usage: if use_uniform_buffers {
                BufferUsages::COPY_DST
            } else {
                BufferUsages::STORAGE | BufferUsages::COPY_DST
            },
            mapped_at_creation: false,
        });

        Self {
            // With storage buffers, slot 0 is reserved as the identity.
            current_staging_buffer: if use_uniform_buffers {
                vec![]
            } else {
                vec![Mat4::IDENTITY]
            },
            current_buffer,
            prev_buffer,
            storage: StorageSkins::default(),
            skin_joints_buffer,
            use_uniform_buffers,
            allocator: Allocator::new(MAX_TOTAL_JOINTS),
            skin_uniform_info: MainEntityHashMap::default(),
            joint_to_skins: MainEntityHashMap::default(),
            total_joints: 0,
        }
    }
}

impl SkinUniforms {
    /// Returns the current offset in joints of the skin in the buffer.
    pub fn skin_index(&self, skin: MainEntity) -> Option<u32> {
        self.skin_uniform_info
            .get(&skin)
            .map(SkinUniformInfo::offset)
    }

    /// Returns the current offset in bytes of the skin in the buffer.
    pub fn skin_byte_offset(&self, skin: MainEntity) -> Option<SkinByteOffset> {
        self.skin_uniform_info.get(&skin).map(|skin_uniform_info| {
            SkinByteOffset::from_index(skin_uniform_info.offset() as usize)
        })
    }

    /// Returns an iterator over all skins in the scene.
    pub fn all_skins(&self) -> impl Iterator<Item = &MainEntity> {
        self.skin_uniform_info.keys()
    }
}

/// The skin joints entries and joint matrix slots of the skins that use
/// storage buffers.
///
/// This holds no GPU state: the joint matrices live in a `Vec<Mat4>` that the
/// caller passes in, and a joint's world matrix is passed in as an `Option`.
#[derive(Default)]
struct StorageSkins {
    /// The CPU-side copy of the skin joints buffer.
    skin_joints_staging: Vec<Mat4>,
    /// The range of [`Self::skin_joints_staging`], in matrices, written since
    /// the last upload.
    dirty: Option<Range<usize>>,
    /// The slot and reference count of the joint matrix of each joint entity.
    joint_slots: MainEntityHashMap<(u32, u32)>,
    /// Slots in the joint matrices that no skin references.
    free_slots: Vec<u32>,
    /// Maps each joint that has an inverse bindpose but no transform to the
    /// skins waiting for it. Such a joint holds no slot.
    pending_joints: MainEntityHashMap<SmallVec<[MainEntity; 1]>>,
    /// The entries of each skin.
    skins: MainEntityHashMap<StorageSkin>,
}

/// The joints of one skin that uses storage buffers.
struct StorageSkin {
    /// The offset in matrices of the skin's first entry.
    offset: usize,
    /// The joint entities of the skin.
    joints: Vec<MainEntity>,
    /// The inverse bindposes. A joint past the end has none.
    inverse_bindposes: Vec<Mat4>,
    /// Whether each joint holds a reference to a slot.
    held: Vec<bool>,
}

impl StorageSkins {
    /// Writes the entries of a skin at `offset` (in matrices) and takes a
    /// reference to the slot of each joint that has an inverse bindpose and a
    /// transform. A joint without a transform is recorded as pending, and one
    /// without an inverse bindpose stays at the identity in slot 0.
    fn add_skin(
        &mut self,
        matrices: &mut Vec<Mat4>,
        skin: MainEntity,
        offset: usize,
        joints: Vec<MainEntity>,
        inverse_bindposes: Vec<Mat4>,
        world: impl Fn(MainEntity) -> Option<Mat4>,
    ) {
        // Grow the staging buffer to cover the whole allocation.
        let end = offset
            + joints
                .len()
                .next_multiple_of(JOINTS_PER_ALLOCATION_UNIT as usize);
        if self.skin_joints_staging.len() < end {
            self.skin_joints_staging.resize(end, Mat4::IDENTITY);
        }

        let mut state = StorageSkin {
            offset,
            held: vec![false; joints.len()],
            joints,
            inverse_bindposes,
        };
        for joint_index in 0..state.joints.len() {
            self.write_entry(matrices, skin, &mut state, joint_index, &world);
        }
        self.mark_dirty(offset..offset + state.joints.len());
        self.skins.insert(skin, state);
    }

    /// Writes the entry of one joint of a skin, taking a reference to the
    /// joint's slot if it has an inverse bindpose and a transform.
    fn write_entry(
        &mut self,
        matrices: &mut Vec<Mat4>,
        skin: MainEntity,
        state: &mut StorageSkin,
        joint_index: usize,
        world: &impl Fn(MainEntity) -> Option<Mat4>,
    ) {
        let joint = state.joints[joint_index];
        let entry = match state.inverse_bindposes.get(joint_index) {
            None => Mat4::IDENTITY,
            Some(&inverse_bindpose) => match world(joint) {
                Some(world_matrix) => {
                    let slot = self.acquire_joint_slot(matrices, joint, world_matrix);
                    state.held[joint_index] = true;
                    self.remove_pending(joint, skin);
                    let mut entry = inverse_bindpose;
                    entry.x_axis.w = slot as f32;
                    entry
                }
                None => {
                    let waiting = self.pending_joints.entry(joint).or_default();
                    if !waiting.contains(&skin) {
                        waiting.push(skin);
                    }
                    Mat4::IDENTITY
                }
            },
        };
        self.skin_joints_staging[state.offset + joint_index] = entry;
    }

    /// Releases the slots of a skin and forgets that it waits for any joint.
    fn remove_skin(&mut self, skin: MainEntity) {
        let Some(state) = self.skins.remove(&skin) else {
            return;
        };
        for (joint_index, &joint) in state.joints.iter().enumerate() {
            if state.held[joint_index] {
                self.release_joint_slot(joint);
            } else if joint_index < state.inverse_bindposes.len() {
                self.remove_pending(joint, skin);
            }
        }
    }

    /// Writes the new world matrix of a joint into its slot. Every skin that
    /// was waiting for the joint gets the entry of each of its joints that now
    /// has a transform.
    fn joint_changed(
        &mut self,
        matrices: &mut Vec<Mat4>,
        joint: MainEntity,
        world_matrix: Mat4,
        world: impl Fn(MainEntity) -> Option<Mat4>,
    ) {
        if let Some(&(slot, _)) = self.joint_slots.get(&joint) {
            matrices[slot as usize] = world_matrix;
        }
        if self.pending_joints.is_empty() {
            return;
        }
        let Some(waiting) = self.pending_joints.remove(&joint) else {
            return;
        };
        for skin in waiting {
            let Some(mut state) = self.skins.remove(&skin) else {
                continue;
            };
            for joint_index in 0..state.joints.len() {
                if !state.held[joint_index] && joint_index < state.inverse_bindposes.len() {
                    self.write_entry(matrices, skin, &mut state, joint_index, &world);
                }
            }
            self.mark_dirty(state.offset..state.offset + state.joints.len());
            self.skins.insert(skin, state);
        }
    }

    /// Returns the slot of the joint's matrix, taking a reference to it. A new
    /// slot starts out holding the joint's world matrix.
    fn acquire_joint_slot(
        &mut self,
        matrices: &mut Vec<Mat4>,
        joint: MainEntity,
        world_matrix: Mat4,
    ) -> u32 {
        match self.joint_slots.entry(joint) {
            Entry::Occupied(mut entry) => {
                entry.get_mut().1 += 1;
                entry.get().0
            }
            Entry::Vacant(entry) => {
                let slot = self.free_slots.pop().unwrap_or_else(|| {
                    matrices.push(Mat4::IDENTITY);
                    matrices.len() as u32 - 1
                });
                matrices[slot as usize] = world_matrix;
                entry.insert((slot, 1));
                slot
            }
        }
    }

    /// Drops a reference to the joint's slot, freeing the slot when no skin
    /// references it any more.
    fn release_joint_slot(&mut self, joint: MainEntity) {
        if let Entry::Occupied(mut entry) = self.joint_slots.entry(joint) {
            entry.get_mut().1 -= 1;
            if entry.get().1 == 0 {
                self.free_slots.push(entry.remove().0);
            }
        }
    }

    /// Removes the skin from the list of skins waiting for the joint.
    fn remove_pending(&mut self, joint: MainEntity, skin: MainEntity) {
        if let Entry::Occupied(mut entry) = self.pending_joints.entry(joint) {
            entry.get_mut().retain(|waiting| *waiting != skin);
            if entry.get().is_empty() {
                entry.remove();
            }
        }
    }

    /// Adds a range of entries, in matrices, to those to upload.
    fn mark_dirty(&mut self, range: Range<usize>) {
        self.dirty = Some(match self.dirty.take() {
            Some(dirty) => dirty.start.min(range.start)..dirty.end.max(range.end),
            None => range,
        });
    }

    /// Returns the range of entries written since the last call.
    fn take_dirty(&mut self) -> Option<Range<usize>> {
        self.dirty.take()
    }
}

/// Allocation information about each skin.
struct SkinUniformInfo {
    /// The allocation of the joints within the skin joints buffer, or
    /// [`SkinUniforms::current_buffer`] with uniform buffers.
    allocation: Allocation,
    /// The entities that comprise the joints (uniform buffers only).
    joints: Vec<MainEntity>,
}

impl SkinUniformInfo {
    /// The offset in joints within the skin's buffer.
    fn offset(&self) -> u32 {
        self.allocation.offset * JOINTS_PER_ALLOCATION_UNIT
    }
}

/// Returns true if skinning must use uniforms (and dynamic offsets) because
/// storage buffers aren't supported on the current platform.
pub fn skins_use_uniform_buffers(render_device: &RenderDevice) -> bool {
    skins_use_uniform_buffers_with(&render_device.limits())
}

/// [`skins_use_uniform_buffers`] from the device limits, for use where no device handle is
/// available (the main world).
pub fn skins_use_uniform_buffers_with(limits: &WgpuLimits) -> bool {
    static SKINS_USE_UNIFORM_BUFFERS: OnceLock<bool> = OnceLock::new();
    *SKINS_USE_UNIFORM_BUFFERS.get_or_init(|| limits.max_storage_buffers_per_shader_stage == 0)
}

/// Uploads the buffers containing the joints to the GPU.
pub fn prepare_skins(
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
    uniform: ResMut<SkinUniforms>,
) {
    let uniform = uniform.into_inner();

    // There is nothing to upload until the first skin exists.
    let nothing_to_upload = if uniform.use_uniform_buffers {
        uniform.current_staging_buffer.is_empty()
    } else {
        uniform.storage.skin_joints_staging.is_empty()
    };
    if nothing_to_upload {
        return;
    }

    // Grow the skin joints buffer if necessary and upload what changed.
    if !uniform.use_uniform_buffers {
        let staging_len = uniform.storage.skin_joints_staging.len();
        let mut upload = uniform.storage.take_dirty();
        let needed_size = staging_len as u64 * size_of::<Mat4>() as u64;
        if uniform.skin_joints_buffer.size() < needed_size {
            let mut new_size = uniform.skin_joints_buffer.size();
            while new_size < needed_size {
                // 1.5× growth factor.
                new_size = (new_size + new_size / 2).next_multiple_of(4);
            }
            uniform.skin_joints_buffer = render_device.create_buffer(&BufferDescriptor {
                label: Some("skin joints buffer"),
                usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
                size: new_size,
                mapped_at_creation: false,
            });
            // The new buffer is empty, so upload everything.
            upload = Some(0..staging_len);
        }
        if let Some(range) = upload {
            render_queue.write_buffer(
                &uniform.skin_joints_buffer,
                (range.start * size_of::<Mat4>()) as u64,
                bytemuck::must_cast_slice(&uniform.storage.skin_joints_staging[range]),
            );
        }
    }

    // Swap current and previous buffers.
    mem::swap(&mut uniform.current_buffer, &mut uniform.prev_buffer);

    // Resize the buffers if necessary. Include extra space equal to `MAX_JOINTS`
    // because we need to be able to bind a full uniform buffer's worth of data
    // if skins use uniform buffers on this platform.
    let needed_size = (uniform.current_staging_buffer.len() as u64 + MAX_JOINTS as u64)
        * size_of::<Mat4>() as u64;
    if uniform.current_buffer.size() < needed_size {
        let mut new_size = uniform.current_buffer.size();
        while new_size < needed_size {
            // 1.5× growth factor.
            new_size = (new_size + new_size / 2).next_multiple_of(4);
        }

        // Create the new buffers.
        let buffer_usages = if skins_use_uniform_buffers(&render_device) {
            BufferUsages::UNIFORM
        } else {
            BufferUsages::STORAGE
        } | BufferUsages::COPY_DST;
        uniform.current_buffer = render_device.create_buffer(&BufferDescriptor {
            label: Some("skin uniform buffer"),
            usage: buffer_usages,
            size: new_size,
            mapped_at_creation: false,
        });
        uniform.prev_buffer = render_device.create_buffer(&BufferDescriptor {
            label: Some("skin uniform buffer"),
            usage: buffer_usages,
            size: new_size,
            mapped_at_creation: false,
        });

        // We've created a new `prev_buffer` but we don't have the previous joint
        // data needed to fill it out correctly. Use the current joint data
        // instead.
        //
        // TODO: This is a bug - will cause motion blur to ignore joint movement
        // for one frame.
        render_queue.write_buffer(
            &uniform.prev_buffer,
            0,
            bytemuck::must_cast_slice(&uniform.current_staging_buffer[..]),
        );
    }

    // Write the data from `uniform.current_staging_buffer` into
    // `uniform.current_buffer`.
    render_queue.write_buffer(
        &uniform.current_buffer,
        0,
        bytemuck::must_cast_slice(&uniform.current_staging_buffer[..]),
    );

    // We don't need to write `uniform.prev_buffer` because we already wrote it
    // last frame, and the data should still be on the GPU.
}

// Notes on implementation (uniform buffers only; storage buffers bind the whole
// buffers and index them in the shader):
// We define the uniform binding as an array<mat4x4<f32>, N> in the shader,
// where N is the maximum number of Mat4s we can fit in the uniform binding,
// which may be as little as 16kB or 64kB. But, we may not need all N.
// We may only need, for example, 10.
//
// If we used uniform buffers ‘normally’ then we would have to write a full
// binding of data for each dynamic offset binding, which is wasteful, makes
// the buffer much larger than it needs to be, and uses more memory bandwidth
// to transfer the data, which then costs frame time So @superdump came up
// with this design: just bind data at the specified offset and interpret
// the data at that offset as an array<T, N> regardless of what is there.
//
// So instead of writing N Mat4s when you only need 10, you write 10, and
// then pad up to the next dynamic offset alignment. Then write the next.
// And for the last dynamic offset binding, make sure there is a full binding
// of data after it so that the buffer is of size
// `last dynamic offset` + `array<mat4x4<f32>>`.
//
// Then when binding the first dynamic offset, the first 10 entries in the array
// are what you expect, but if you read the 11th you’re reading ‘invalid’ data
// which could be padding or could be from the next binding.
//
// In this way, we can pack ‘variable sized arrays’ into uniform buffer bindings
// which normally only support fixed size arrays. You just have to make sure
// in the shader that you only read the values that are valid for that binding.
pub fn extract_skins(
    skin_uniforms: ResMut<SkinUniforms>,
    skinned_meshes: Extract<Query<(Entity, &SkinnedMesh)>>,
    changed_skinned_meshes: Extract<
        Query<
            (Entity, &ViewVisibility, &SkinnedMesh),
            Or<(
                Changed<ViewVisibility>,
                Changed<SkinnedMesh>,
                AssetChanged<SkinnedMesh>,
            )>,
        >,
    >,
    skinned_mesh_inverse_bindposes: Extract<Res<Assets<SkinnedMeshInverseBindposes>>>,
    changed_transforms: Extract<Query<(Entity, &GlobalTransform), Changed<GlobalTransform>>>,
    joints: Extract<Query<&GlobalTransform>>,
    mut removed_visibilities_query: Extract<RemovedComponents<ViewVisibility>>,
    mut removed_skinned_meshes_query: Extract<RemovedComponents<SkinnedMesh>>,
) {
    let skin_uniforms = skin_uniforms.into_inner();

    // Find skins that have become visible or invisible on this frame. Allocate,
    // reallocate, or free space for them as necessary.
    add_or_delete_skins(
        skin_uniforms,
        &changed_skinned_meshes,
        &skinned_mesh_inverse_bindposes,
        &joints,
    );

    if skin_uniforms.use_uniform_buffers {
        // Extract the transforms for all joints from the scene, and write them
        // into the staging buffer at the appropriate spot.
        extract_joints(
            skin_uniforms,
            &skinned_meshes,
            &changed_skinned_meshes,
            &skinned_mesh_inverse_bindposes,
            &changed_transforms,
            &joints,
        );
    } else {
        // Write the world matrix of every changed joint entity into its slot,
        // and complete the skins that were waiting for it.
        for (joint, transform) in changed_transforms.iter() {
            skin_uniforms.storage.joint_changed(
                &mut skin_uniforms.current_staging_buffer,
                MainEntity::from(joint),
                transform.compute_matrix(),
                |joint| joints.get(*joint).ok().map(GlobalTransform::compute_matrix),
            );
        }
    }

    // Delete skins that became invisible.
    for skinned_mesh_entity in removed_visibilities_query
        .read()
        .chain(removed_skinned_meshes_query.read())
    {
        // Only remove a skin if we didn't pick it up in `add_or_delete_skins`.
        // It's possible that a necessary component was removed and re-added in
        // the same frame.
        if !changed_skinned_meshes.contains(skinned_mesh_entity) {
            remove_skin(skin_uniforms, skinned_mesh_entity.into());
        }
    }
}

/// Searches for all skins that have become visible or invisible this frame and
/// allocations for them as necessary.
fn add_or_delete_skins(
    skin_uniforms: &mut SkinUniforms,
    changed_skinned_meshes: &Query<
        (Entity, &ViewVisibility, &SkinnedMesh),
        Or<(
            Changed<ViewVisibility>,
            Changed<SkinnedMesh>,
            AssetChanged<SkinnedMesh>,
        )>,
    >,
    skinned_mesh_inverse_bindposes: &Assets<SkinnedMeshInverseBindposes>,
    joints: &Query<&GlobalTransform>,
) {
    // Find every skinned mesh that changed one of (1) visibility; (2) joint
    // entities (part of `SkinnedMesh`); (3) the associated
    // `SkinnedMeshInverseBindposes` asset.
    for (skinned_mesh_entity, skinned_mesh_view_visibility, skinned_mesh) in changed_skinned_meshes
    {
        // Remove the skin if it existed last frame.
        let skinned_mesh_entity = MainEntity::from(skinned_mesh_entity);
        remove_skin(skin_uniforms, skinned_mesh_entity);

        // If the skin is invisible, we're done.
        if !(*skinned_mesh_view_visibility).get() {
            continue;
        }

        // Initialize the skin.
        add_skin(
            skinned_mesh_entity,
            skinned_mesh,
            skin_uniforms,
            skinned_mesh_inverse_bindposes,
            joints,
        );
    }
}

/// Extracts the global transforms of all joints and updates the staging buffer
/// as necessary.
fn extract_joints(
    skin_uniforms: &mut SkinUniforms,
    skinned_meshes: &Query<(Entity, &SkinnedMesh)>,
    changed_skinned_meshes: &Query<
        (Entity, &ViewVisibility, &SkinnedMesh),
        Or<(
            Changed<ViewVisibility>,
            Changed<SkinnedMesh>,
            AssetChanged<SkinnedMesh>,
        )>,
    >,
    skinned_mesh_inverse_bindposes: &Assets<SkinnedMeshInverseBindposes>,
    changed_transforms: &Query<(Entity, &GlobalTransform), Changed<GlobalTransform>>,
    joints: &Query<&GlobalTransform>,
) {
    // If the number of entities that changed transforms exceeds a certain
    // fraction (currently 25%) of the total joints in the scene, then skip
    // fine-grained change detection.
    //
    // Note that this is a crude heuristic, for performance reasons. It doesn't
    // consider the ratio of modified *joints* to total joints, only the ratio
    // of modified *entities* to total joints. Thus in the worst case we might
    // end up re-extracting all skins even though none of the joints changed.
    // But making the heuristic finer-grained would make it slower to evaluate,
    // and we don't want to lose performance.
    let threshold =
        (skin_uniforms.total_joints as f64 * JOINT_EXTRACTION_THRESHOLD_FACTOR).floor() as usize;

    if changed_transforms.iter().nth(threshold).is_some() {
        // Go ahead and re-extract all skins in the scene.
        for (skin_entity, skin) in skinned_meshes {
            extract_joints_for_skin(
                skin_entity.into(),
                skin,
                skin_uniforms,
                changed_skinned_meshes,
                skinned_mesh_inverse_bindposes,
                joints,
            );
        }
        return;
    }

    // Use fine-grained change detection to figure out only the skins that need
    // to have their joints re-extracted.
    let dirty_skins: MainEntityHashSet = changed_transforms
        .iter()
        .flat_map(|(joint, _)| skin_uniforms.joint_to_skins.get(&MainEntity::from(joint)))
        .flat_map(|skin_joint_mappings| skin_joint_mappings.iter())
        .copied()
        .collect();

    // Re-extract the joints for only those skins.
    for skin_entity in dirty_skins {
        let Ok((_, skin)) = skinned_meshes.get(*skin_entity) else {
            continue;
        };
        extract_joints_for_skin(
            skin_entity,
            skin,
            skin_uniforms,
            changed_skinned_meshes,
            skinned_mesh_inverse_bindposes,
            joints,
        );
    }
}

/// Extracts all joints for a single skin and writes their transforms into the
/// CPU staging buffer.
fn extract_joints_for_skin(
    skin_entity: MainEntity,
    skin: &SkinnedMesh,
    skin_uniforms: &mut SkinUniforms,
    changed_skinned_meshes: &Query<
        (Entity, &ViewVisibility, &SkinnedMesh),
        Or<(
            Changed<ViewVisibility>,
            Changed<SkinnedMesh>,
            AssetChanged<SkinnedMesh>,
        )>,
    >,
    skinned_mesh_inverse_bindposes: &Assets<SkinnedMeshInverseBindposes>,
    joints: &Query<&GlobalTransform>,
) {
    // If we initialized the skin this frame, we already populated all
    // the joints, so there's no need to populate them again.
    if changed_skinned_meshes.contains(*skin_entity) {
        return;
    }

    // Fetch information about the skin.
    let Some(skin_uniform_info) = skin_uniforms.skin_uniform_info.get(&skin_entity) else {
        return;
    };
    let Some(skinned_mesh_inverse_bindposes) =
        skinned_mesh_inverse_bindposes.get(&skin.inverse_bindposes)
    else {
        return;
    };

    // Calculate and write in the new joint matrices.
    for (joint_index, (&joint, skinned_mesh_inverse_bindpose)) in skin
        .joints
        .iter()
        .zip(skinned_mesh_inverse_bindposes.iter())
        .enumerate()
    {
        let Ok(joint_transform) = joints.get(joint) else {
            continue;
        };

        let joint_matrix = joint_transform.affine() * *skinned_mesh_inverse_bindpose;
        skin_uniforms.current_staging_buffer[skin_uniform_info.offset() as usize + joint_index] =
            joint_matrix;
    }
}

/// Allocates space for a new skin in the buffers, and populates its joints.
fn add_skin(
    skinned_mesh_entity: MainEntity,
    skinned_mesh: &SkinnedMesh,
    skin_uniforms: &mut SkinUniforms,
    skinned_mesh_inverse_bindposes: &Assets<SkinnedMeshInverseBindposes>,
    joints: &Query<&GlobalTransform>,
) {
    // Allocate space for the joints.
    let Some(allocation) = skin_uniforms.allocator.allocate(
        skinned_mesh
            .joints
            .len()
            .div_ceil(JOINTS_PER_ALLOCATION_UNIT as usize) as u32,
    ) else {
        error!(
            "Out of space for skin: {:?}. Tried to allocate space for {:?} joints.",
            skinned_mesh_entity,
            skinned_mesh.joints.len()
        );
        return;
    };

    if !skin_uniforms.use_uniform_buffers {
        let inverse_bindposes = skinned_mesh_inverse_bindposes
            .get(&skinned_mesh.inverse_bindposes)
            .map(|bindposes| bindposes.iter().copied().collect())
            .unwrap_or_default();
        skin_uniforms.storage.add_skin(
            &mut skin_uniforms.current_staging_buffer,
            skinned_mesh_entity,
            (allocation.offset * JOINTS_PER_ALLOCATION_UNIT) as usize,
            skinned_mesh
                .joints
                .iter()
                .map(|entity| MainEntity::from(*entity))
                .collect(),
            inverse_bindposes,
            |joint| joints.get(*joint).ok().map(GlobalTransform::compute_matrix),
        );
        skin_uniforms.skin_uniform_info.insert(
            skinned_mesh_entity,
            SkinUniformInfo {
                allocation,
                joints: vec![],
            },
        );
        return;
    }

    // Store that allocation.
    let skin_uniform_info = SkinUniformInfo {
        allocation,
        joints: skinned_mesh
            .joints
            .iter()
            .map(|entity| MainEntity::from(*entity))
            .collect(),
    };

    let skinned_mesh_inverse_bindposes =
        skinned_mesh_inverse_bindposes.get(&skinned_mesh.inverse_bindposes);

    for (joint_index, &joint) in skinned_mesh.joints.iter().enumerate() {
        // Calculate the initial joint matrix.
        let skinned_mesh_inverse_bindpose =
            skinned_mesh_inverse_bindposes.and_then(|skinned_mesh_inverse_bindposes| {
                skinned_mesh_inverse_bindposes.get(joint_index)
            });
        let joint_matrix = match (skinned_mesh_inverse_bindpose, joints.get(joint)) {
            (Some(skinned_mesh_inverse_bindpose), Ok(transform)) => {
                transform.affine() * *skinned_mesh_inverse_bindpose
            }
            _ => Mat4::IDENTITY,
        };

        // Write in the new joint matrix, growing the staging buffer if
        // necessary.
        let buffer_index = skin_uniform_info.offset() as usize + joint_index;
        if skin_uniforms.current_staging_buffer.len() < buffer_index + 1 {
            skin_uniforms
                .current_staging_buffer
                .resize(buffer_index + 1, Mat4::IDENTITY);
        }
        skin_uniforms.current_staging_buffer[buffer_index] = joint_matrix;

        // Record the inverse mapping from the joint back to the skin. We use
        // this in order to perform fine-grained joint extraction.
        skin_uniforms
            .joint_to_skins
            .entry(MainEntity::from(joint))
            .or_default()
            .push(skinned_mesh_entity);
    }

    // Record the number of joints.
    skin_uniforms.total_joints += skinned_mesh.joints.len();

    skin_uniforms
        .skin_uniform_info
        .insert(skinned_mesh_entity, skin_uniform_info);
}

/// Deallocates a skin and removes it from the [`SkinUniforms`].
fn remove_skin(skin_uniforms: &mut SkinUniforms, skinned_mesh_entity: MainEntity) {
    let Some(old_skin_uniform_info) = skin_uniforms.skin_uniform_info.remove(&skinned_mesh_entity)
    else {
        return;
    };

    // Free the allocation.
    skin_uniforms
        .allocator
        .free(old_skin_uniform_info.allocation);

    if !skin_uniforms.use_uniform_buffers {
        skin_uniforms.storage.remove_skin(skinned_mesh_entity);
        return;
    }

    // Remove the inverse mapping from each joint back to the skin.
    for &joint in &old_skin_uniform_info.joints {
        if let Entry::Occupied(mut entry) = skin_uniforms.joint_to_skins.entry(joint) {
            entry.get_mut().retain(|skin| *skin != skinned_mesh_entity);
            if entry.get_mut().is_empty() {
                entry.remove();
            }
        }
    }

    // Update the total number of joints.
    skin_uniforms.total_joints -= old_skin_uniform_info.joints.len();
}

// NOTE: The skinned joints uniform buffer has to be bound at a dynamic offset per
// entity and so cannot currently be batched on WebGL 2.
pub fn no_automatic_skin_batching(
    mut commands: Commands,
    query: Query<Entity, (With<SkinnedMesh>, Without<NoAutomaticBatching>)>,
    capabilities: Option<Res<RenderCapabilities>>,
) {
    let Some(capabilities) = capabilities else {
        return;
    };
    if !skins_use_uniform_buffers_with(&capabilities.limits()) {
        return;
    }

    for entity in &query {
        commands.entity(entity).try_insert(NoAutomaticBatching);
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use bevy_math::Vec3;

    use super::*;

    fn entity(index: u32) -> MainEntity {
        MainEntity::from(Entity::from_raw(index))
    }

    fn bindpose(index: usize) -> Mat4 {
        Mat4::from_translation(Vec3::new(index as f32 + 1.0, 0.0, 0.0))
    }

    fn world_matrix(index: usize) -> Mat4 {
        Mat4::from_translation(Vec3::new(0.0, index as f32 + 10.0, 0.0))
    }

    /// The entry a joint with this bindpose and slot must have.
    fn expected_entry(bindpose: Mat4, slot: u32) -> Mat4 {
        let mut entry = bindpose;
        entry.x_axis.w = slot as f32;
        entry
    }

    /// The joint matrices with slot 0 as the identity.
    fn matrices() -> Vec<Mat4> {
        vec![Mat4::IDENTITY]
    }

    fn slot_of(storage: &StorageSkins, skin: MainEntity, joint_index: usize) -> u32 {
        let state = &storage.skins[&skin];
        storage.skin_joints_staging[state.offset + joint_index]
            .x_axis
            .w as u32
    }

    fn entry_of(storage: &StorageSkins, skin: MainEntity, joint_index: usize) -> Mat4 {
        let state = &storage.skins[&skin];
        storage.skin_joints_staging[state.offset + joint_index]
    }

    fn add(
        storage: &mut StorageSkins,
        matrices: &mut Vec<Mat4>,
        skin: MainEntity,
        offset: usize,
        joints: &[MainEntity],
        world: &HashMap<MainEntity, Mat4>,
    ) {
        let bindposes = (0..joints.len()).map(bindpose).collect();
        storage.add_skin(
            matrices,
            skin,
            offset,
            joints.to_vec(),
            bindposes,
            |joint| world.get(&joint).copied(),
        );
    }

    #[test]
    fn skins_share_joint_slots_and_keep_their_bindposes() {
        let (skin_a, skin_b) = (entity(100), entity(101));
        let joints = [entity(1), entity(2)];
        let world: HashMap<_, _> = [(joints[0], world_matrix(0)), (joints[1], world_matrix(1))]
            .into_iter()
            .collect();
        let mut storage = StorageSkins::default();
        let mut matrices = matrices();

        add(&mut storage, &mut matrices, skin_a, 0, &joints, &world);
        let slots = [slot_of(&storage, skin_a, 0), slot_of(&storage, skin_a, 1)];
        // Two joints share no slot with each other, and neither uses slot 0.
        assert_ne!(slots[0], slots[1]);
        assert!(slots.iter().all(|&slot| slot != 0));

        // The second skin gets different inverse bindposes for the same joints.
        let other_bindposes = vec![bindpose(5), bindpose(6)];
        storage.add_skin(
            &mut matrices,
            skin_b,
            4,
            joints.to_vec(),
            other_bindposes.clone(),
            |joint| world.get(&joint).copied(),
        );

        for index in 0..2 {
            assert_eq!(slot_of(&storage, skin_b, index), slots[index]);
            assert_eq!(
                entry_of(&storage, skin_a, index),
                expected_entry(bindpose(index), slots[index])
            );
            assert_eq!(
                entry_of(&storage, skin_b, index),
                expected_entry(other_bindposes[index], slots[index])
            );
            assert_eq!(matrices[slots[index] as usize], world_matrix(index));
            assert_eq!(storage.joint_slots[&joints[index]].1, 2);
        }
        // Slot 0 and one slot per distinct joint.
        assert_eq!(matrices.len(), 3);
    }

    #[test]
    fn joint_without_transform_is_identity_in_slot_zero() {
        let skin = entity(100);
        let joints = [entity(1), entity(2)];
        let world: HashMap<_, _> = [(joints[0], world_matrix(0))].into_iter().collect();
        let mut storage = StorageSkins::default();
        let mut matrices = matrices();

        add(&mut storage, &mut matrices, skin, 0, &joints, &world);

        assert_eq!(entry_of(&storage, skin, 1), Mat4::IDENTITY);
        assert_eq!(slot_of(&storage, skin, 1), 0);
        assert!(!storage.joint_slots.contains_key(&joints[1]));
        assert_eq!(matrices.len(), 2);
        assert_eq!(storage.pending_joints[&joints[1]].as_slice(), &[skin]);
        assert!(!storage.pending_joints.contains_key(&joints[0]));
    }

    #[test]
    fn joint_without_bindpose_is_identity_and_not_pending() {
        let skin = entity(100);
        let joints = [entity(1), entity(2)];
        let world: HashMap<_, _> = [(joints[0], world_matrix(0)), (joints[1], world_matrix(1))]
            .into_iter()
            .collect();
        let mut storage = StorageSkins::default();
        let mut matrices = matrices();

        storage.add_skin(
            &mut matrices,
            skin,
            0,
            joints.to_vec(),
            vec![bindpose(0)],
            |joint| world.get(&joint).copied(),
        );

        assert_eq!(entry_of(&storage, skin, 1), Mat4::IDENTITY);
        assert!(!storage.joint_slots.contains_key(&joints[1]));
        assert!(storage.pending_joints.is_empty());
    }

    #[test]
    fn pending_joint_completes_when_it_gets_a_transform() {
        let (skin_a, skin_b) = (entity(100), entity(101));
        let joints = [entity(1), entity(2)];
        let mut world: HashMap<_, _> = [(joints[0], world_matrix(0))].into_iter().collect();
        let mut storage = StorageSkins::default();
        let mut matrices = matrices();
        add(&mut storage, &mut matrices, skin_a, 0, &joints, &world);
        add(&mut storage, &mut matrices, skin_b, 4, &joints, &world);
        storage.take_dirty();

        world.insert(joints[1], world_matrix(1));
        storage.joint_changed(&mut matrices, joints[1], world_matrix(1), |joint| {
            world.get(&joint).copied()
        });

        let slot = slot_of(&storage, skin_a, 1);
        assert_ne!(slot, 0);
        assert_eq!(slot_of(&storage, skin_b, 1), slot);
        assert_eq!(matrices[slot as usize], world_matrix(1));
        assert_eq!(
            entry_of(&storage, skin_a, 1),
            expected_entry(bindpose(1), slot)
        );
        assert_eq!(
            entry_of(&storage, skin_b, 1),
            expected_entry(bindpose(1), slot)
        );
        assert_eq!(storage.joint_slots[&joints[1]], (slot, 2));
        assert!(storage.pending_joints.is_empty());
        // Both skins were rewritten.
        assert_eq!(storage.take_dirty(), Some(0..6));

        // Later changes only move the matrix.
        storage.joint_changed(&mut matrices, joints[1], world_matrix(7), |_| None);
        assert_eq!(matrices[slot as usize], world_matrix(7));
        assert_eq!(storage.take_dirty(), None);
    }

    #[test]
    fn skin_with_several_pending_joints_completes_in_one_pass() {
        let skin = entity(100);
        let joints = [entity(1), entity(2)];
        let mut world = HashMap::new();
        let mut storage = StorageSkins::default();
        let mut matrices = matrices();
        add(&mut storage, &mut matrices, skin, 0, &joints, &world);
        assert_eq!(matrices.len(), 1);

        world.insert(joints[0], world_matrix(0));
        world.insert(joints[1], world_matrix(1));
        storage.joint_changed(&mut matrices, joints[0], world_matrix(0), |joint| {
            world.get(&joint).copied()
        });
        // The second joint was completed by the same pass.
        assert!(storage.pending_joints.is_empty());
        storage.joint_changed(&mut matrices, joints[1], world_matrix(1), |joint| {
            world.get(&joint).copied()
        });

        assert_eq!(storage.joint_slots[&joints[0]].1, 1);
        assert_eq!(storage.joint_slots[&joints[1]].1, 1);
        assert_eq!(matrices.len(), 3);
    }

    #[test]
    fn removing_skins_releases_slots() {
        let (skin_a, skin_b, skin_c) = (entity(100), entity(101), entity(102));
        let joints = [entity(1), entity(2)];
        let world: HashMap<_, _> = [(joints[0], world_matrix(0)), (joints[1], world_matrix(1))]
            .into_iter()
            .collect();
        let mut storage = StorageSkins::default();
        let mut matrices = matrices();
        add(&mut storage, &mut matrices, skin_a, 0, &joints, &world);
        add(&mut storage, &mut matrices, skin_b, 4, &joints, &world);
        let slots = [slot_of(&storage, skin_a, 0), slot_of(&storage, skin_a, 1)];

        // A shared joint keeps its slot while one skin remains.
        storage.remove_skin(skin_a);
        assert_eq!(storage.joint_slots[&joints[0]], (slots[0], 1));
        assert!(storage.free_slots.is_empty());

        storage.remove_skin(skin_b);
        assert!(storage.joint_slots.is_empty());
        assert_eq!(storage.free_slots.len(), 2);

        // A freed slot is reused instead of growing the matrices.
        let new_joint = entity(3);
        let new_world: HashMap<_, _> = [(new_joint, world_matrix(3))].into_iter().collect();
        add(
            &mut storage,
            &mut matrices,
            skin_c,
            0,
            &[new_joint],
            &new_world,
        );
        assert!(slots.contains(&slot_of(&storage, skin_c, 0)));
        assert_eq!(matrices.len(), 3);
        assert_eq!(storage.free_slots.len(), 1);

        // Removing a skin that was never added changes nothing.
        storage.remove_skin(entity(999));
        assert_eq!(storage.free_slots.len(), 1);
    }

    #[test]
    fn removing_a_skin_with_a_pending_joint_balances_everything() {
        let (skin_a, skin_b) = (entity(100), entity(101));
        let joints = [entity(1), entity(2)];
        let mut world: HashMap<_, _> = [(joints[0], world_matrix(0))].into_iter().collect();
        let mut storage = StorageSkins::default();
        let mut matrices = matrices();
        add(&mut storage, &mut matrices, skin_a, 0, &joints, &world);
        add(&mut storage, &mut matrices, skin_b, 4, &joints, &world);

        // Only skin A leaves; skin B keeps waiting.
        storage.remove_skin(skin_a);
        assert_eq!(storage.pending_joints[&joints[1]].as_slice(), &[skin_b]);
        assert_eq!(storage.joint_slots[&joints[0]].1, 1);

        storage.remove_skin(skin_b);
        assert!(storage.pending_joints.is_empty());
        assert!(storage.joint_slots.is_empty());
        assert_eq!(storage.free_slots.len(), 1);

        // A joint that appears later finds nobody waiting.
        world.insert(joints[1], world_matrix(1));
        storage.joint_changed(&mut matrices, joints[1], world_matrix(1), |joint| {
            world.get(&joint).copied()
        });
        assert!(storage.joint_slots.is_empty());
        assert_eq!(storage.take_dirty().map(|range| range.start), Some(0));
    }

    #[test]
    fn dirty_range_covers_the_entries_written_since_the_last_take() {
        let joints = [entity(1), entity(2), entity(3)];
        let world: HashMap<_, _> = joints
            .iter()
            .enumerate()
            .map(|(index, &joint)| (joint, world_matrix(index)))
            .collect();
        let mut storage = StorageSkins::default();
        let mut matrices = matrices();
        assert_eq!(storage.take_dirty(), None);

        // One skin of three joints at matrix 8.
        add(&mut storage, &mut matrices, entity(100), 8, &joints, &world);
        assert_eq!(storage.take_dirty(), Some(8..11));
        assert_eq!(storage.take_dirty(), None);

        // Adds within a frame merge into one range.
        add(
            &mut storage,
            &mut matrices,
            entity(101),
            16,
            &joints[..2],
            &world,
        );
        add(
            &mut storage,
            &mut matrices,
            entity(102),
            4,
            &joints[..1],
            &world,
        );
        assert_eq!(storage.take_dirty(), Some(4..18));
        assert_eq!(storage.take_dirty(), None);
    }
}
