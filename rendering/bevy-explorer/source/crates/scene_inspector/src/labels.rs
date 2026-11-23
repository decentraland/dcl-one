//! What a scene entity shows, in a few words read from the scene's own state: `gltf <src>` (the
//! last 64 characters of a longer one, which hold the file name),
//! `mesh <Box|Sphere|Cylinder|Plane|Gltf>`, `text "<first 24 characters>"`, `hidden`,
//! `parent <id>`, joined by `, `; the reserved entities are `scene root`, `player` and `camera`.
//! `/scene_entities --labels` and `/visible_entities` label with it.

use dcl_component::{
    proto_components::sdk::components::{
        pb_mesh_renderer, PbGltfContainer, PbMeshRenderer, PbTextShape, PbVisibilityComponent,
    },
    transform_and_parent::DclTransformAndParent,
    DclReader, FromDclReader, SceneComponentId, SceneEntityId,
};
use dcl_types::interface::CrdtStore;

const MAX_TEXT_CHARS: usize = 24;
const MAX_SRC_CHARS: usize = 64;

/// The entity's current value of a component, if the scene has put one on it.
fn read<T: FromDclReader>(
    crdt: &CrdtStore,
    component: SceneComponentId,
    entity: SceneEntityId,
) -> Option<T> {
    let entry = crdt.lww.get(&component)?.last_write.get(&entity)?;
    if !entry.is_some {
        return None;
    }
    T::from_reader(&mut DclReader::new(&entry.data)).ok()
}

/// Whether the scene has put `component` on `entity`.
pub(crate) fn has(crdt: &CrdtStore, component: SceneComponentId, entity: SceneEntityId) -> bool {
    crdt.lww
        .get(&component)
        .and_then(|state| state.last_write.get(&entity))
        .is_some_and(|entry| entry.is_some)
}

/// Whether the entity draws a primitive mesh or a glTF: what `/visible_entities` lists.
pub(crate) fn renders(crdt: &CrdtStore, entity: SceneEntityId) -> bool {
    has(crdt, SceneComponentId::MESH_RENDERER, entity)
        || has(crdt, SceneComponentId::GLTF_CONTAINER, entity)
}

/// The entity's label, `None` when nothing describes it.
pub(crate) fn label(crdt: &CrdtStore, entity: SceneEntityId) -> Option<String> {
    match entity.id {
        0 => return Some("scene root".to_owned()),
        1 => return Some("player".to_owned()),
        2 => return Some("camera".to_owned()),
        _ => {}
    }
    let mut parts = Vec::new();
    if let Some(gltf) = read::<PbGltfContainer>(crdt, SceneComponentId::GLTF_CONTAINER, entity) {
        let length = gltf.src.chars().count();
        if length > MAX_SRC_CHARS {
            // the file name is at the end
            let tail: String = gltf.src.chars().skip(length - MAX_SRC_CHARS).collect();
            parts.push(format!("gltf …{tail}"));
        } else {
            parts.push(format!("gltf {}", gltf.src));
        }
    }
    if let Some(mesh) = read::<PbMeshRenderer>(crdt, SceneComponentId::MESH_RENDERER, entity) {
        let case = match mesh.mesh {
            Some(pb_mesh_renderer::Mesh::Box(_)) => "Box",
            Some(pb_mesh_renderer::Mesh::Sphere(_)) => "Sphere",
            Some(pb_mesh_renderer::Mesh::Cylinder(_)) => "Cylinder",
            Some(pb_mesh_renderer::Mesh::Plane(_)) => "Plane",
            Some(pb_mesh_renderer::Mesh::Gltf(_)) => "Gltf",
            None => "None",
        };
        parts.push(format!("mesh {case}"));
    }
    if let Some(text) = read::<PbTextShape>(crdt, SceneComponentId::TEXT_SHAPE, entity) {
        let excerpt = if text.text.chars().count() > MAX_TEXT_CHARS {
            format!(
                "{}…",
                text.text.chars().take(MAX_TEXT_CHARS).collect::<String>()
            )
        } else {
            text.text
        };
        parts.push(format!("text {excerpt:?}"));
    }
    if read::<PbVisibilityComponent>(crdt, SceneComponentId::VISIBILITY, entity)
        .is_some_and(|v| v.visible == Some(false))
    {
        parts.push("hidden".to_owned());
    }
    if let Some(transform) =
        read::<DclTransformAndParent>(crdt, SceneComponentId::TRANSFORM, entity)
    {
        let parent = transform.parent();
        if parent != SceneEntityId::ROOT {
            parts.push(format!(
                "parent {}",
                parent.as_proto_u32().unwrap_or(parent.id as u32)
            ));
        }
    }
    (!parts.is_empty()).then(|| parts.join(", "))
}

#[cfg(test)]
mod tests {
    use bevy::prelude::Transform;
    use dcl_component::{proto_components::sdk::components::PbTextShape, CrdtType, DclWriter};

    use super::*;

    fn put<T: dcl_component::ToDclWriter>(
        crdt: &mut CrdtStore,
        component: SceneComponentId,
        entity: SceneEntityId,
        value: &T,
    ) {
        let mut bytes = Vec::new();
        DclWriter::new(&mut bytes).write(value);
        crdt.force_update(
            component,
            CrdtType::LWW_ENT,
            entity,
            Some(&mut DclReader::new(&bytes)),
        );
    }

    #[test]
    fn a_label_names_the_mesh_the_text_the_visibility_and_the_parent() {
        let parent = SceneEntityId::from_proto_u32(512);
        let entity = SceneEntityId::from_proto_u32(571);
        let mut crdt = CrdtStore::default();
        assert_eq!(label(&crdt, entity), None);
        assert!(!renders(&crdt, entity));

        put(
            &mut crdt,
            SceneComponentId::MESH_RENDERER,
            entity,
            &PbMeshRenderer {
                mesh: Some(pb_mesh_renderer::Mesh::Box(Default::default())),
            },
        );
        put(
            &mut crdt,
            SceneComponentId::TEXT_SHAPE,
            entity,
            &PbTextShape {
                text: "a sign whose text runs past the excerpt".to_owned(),
                ..Default::default()
            },
        );
        put(
            &mut crdt,
            SceneComponentId::VISIBILITY,
            entity,
            &PbVisibilityComponent {
                visible: Some(false),
                ..Default::default()
            },
        );
        put(
            &mut crdt,
            SceneComponentId::TRANSFORM,
            entity,
            &DclTransformAndParent::from_bevy_transform_and_parent(&Transform::IDENTITY, parent),
        );
        assert_eq!(
            label(&crdt, entity).as_deref(),
            Some("mesh Box, text \"a sign whose text runs p…\", hidden, parent 512")
        );
        assert!(renders(&crdt, entity));
        assert_eq!(
            label(&crdt, SceneEntityId::ROOT).as_deref(),
            Some("scene root")
        );
    }

    #[test]
    fn a_long_gltf_src_keeps_its_end() {
        let entity = SceneEntityId::from_proto_u32(600);
        let mut crdt = CrdtStore::default();
        put(
            &mut crdt,
            SceneComponentId::GLTF_CONTAINER,
            entity,
            &PbGltfContainer {
                src: "models/tree.glb".to_owned(),
                ..Default::default()
            },
        );
        assert_eq!(
            label(&crdt, entity).as_deref(),
            Some("gltf models/tree.glb")
        );

        let src = format!("{}/tree.glb", "é".repeat(100));
        put(
            &mut crdt,
            SceneComponentId::GLTF_CONTAINER,
            entity,
            &PbGltfContainer {
                src,
                ..Default::default()
            },
        );
        let label = label(&crdt, entity).unwrap();
        assert!(label.starts_with("gltf …é"), "{label}");
        assert!(label.ends_with("é/tree.glb"), "{label}");
        assert_eq!(
            label.chars().count(),
            "gltf …".chars().count() + MAX_SRC_CHARS
        );
    }
}
