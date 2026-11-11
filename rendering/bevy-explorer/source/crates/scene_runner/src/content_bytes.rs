//! CPU-side bytes of one scene's visible content.

use bevy::{
    platform::{collections::HashSet, hash::FixedHasher},
    prelude::*,
    render::mesh::Indices,
};
use scene_material::SceneMaterial;
use serde::Serialize;

use crate::ContainerEntity;

/// The textures the scene's visible materials reference and the meshes its visible entities
/// draw, each counted once, with the bytes their CPU copies hold.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct SceneContentBytes {
    pub texture_bytes_sum: u64,
    pub mesh_bytes_sum: u64,
    pub texture_count: u32,
    pub mesh_count: u32,
}

pub fn scene_content_bytes<'a>(
    scene: Entity,
    mesh_handles: impl Iterator<Item = (&'a Mesh3d, &'a ContainerEntity, &'a Visibility)>,
    material_handles: impl Iterator<
        Item = (
            &'a MeshMaterial3d<SceneMaterial>,
            &'a ContainerEntity,
            &'a Visibility,
        ),
    >,
    meshes: &Assets<Mesh>,
    materials: &Assets<SceneMaterial>,
    images: &Assets<Image>,
) -> SceneContentBytes {
    let visible = |container: &ContainerEntity, visibility: &Visibility| {
        container.root == scene && !matches!(visibility, Visibility::Hidden)
    };
    let visible_meshes = mesh_handles
        .filter(|(_, container, visibility)| visible(container, visibility))
        .map(|(handle, ..)| handle.id())
        .collect::<HashSet<_, FixedHasher>>();
    let mesh_bytes_sum = visible_meshes
        .iter()
        .filter_map(|id| meshes.get(*id))
        .map(|mesh| {
            let vertices = mesh.count_vertices() as u64 * mesh.get_vertex_size();
            let indices = match mesh.indices() {
                Some(Indices::U16(indices)) => indices.len() as u64 * 2,
                Some(Indices::U32(indices)) => indices.len() as u64 * 4,
                None => 0,
            };
            vertices + indices
        })
        .sum();
    let textures = material_handles
        .filter(|(_, container, visibility)| visible(container, visibility))
        .filter_map(|(handle, ..)| materials.get(handle.id()))
        .flat_map(|material| {
            material
                .base
                .base_color_texture
                .iter()
                .chain(material.base.emissive_texture.as_ref())
                .chain(material.base.normal_map_texture.as_ref())
                .map(Handle::id)
        })
        .collect::<HashSet<_, FixedHasher>>();
    let texture_bytes_sum = textures
        .iter()
        .filter_map(|id| images.get(*id).and_then(|image| image.data.as_ref()))
        .map(|data| data.len() as u64)
        .sum();
    SceneContentBytes {
        texture_bytes_sum,
        mesh_bytes_sum,
        texture_count: textures.len() as u32,
        mesh_count: visible_meshes.len() as u32,
    }
}
