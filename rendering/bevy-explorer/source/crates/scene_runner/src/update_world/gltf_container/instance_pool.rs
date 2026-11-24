//! Processed glTF instances a scene no longer shows, kept for its next container of the same
//! model. A pooled instance hangs hidden under one holder entity, without its container links and
//! colliders; the oldest go first once their component bytes pass the budget.

use std::collections::VecDeque;

use bevy::{gltf::Gltf, platform::collections::HashMap, prelude::*, scene::InstanceId};
use dcl_component::{
    proto_components::sdk::components::{
        common::LoadingState, PbGltfContainer, PbGltfContainerLoadingState,
    },
    SceneComponentId, SceneEntityId,
};
use dcl_types::interface::CrdtType;

use super::{GltfLoaded, GltfNameCache, GltfNames, GltfPooledNodes, GltfProcessed, GltfReady};
use crate::{
    renderer_context::RendererSceneContext,
    update_world::{
        mesh_collider::{CtCollider, HasCollider},
        trigger_area::CtTrigger,
    },
    ContainerEntity,
};

/// The component bytes pooled instances may hold together.
pub const GLTF_INSTANCE_POOL_BUDGET_BYTES: usize = 16 * 1024 * 1024;

/// What a processed model was made from, on a container whose model may be pooled: one with no
/// animation and no light, which no node link or node modifier has touched.
#[derive(Component, Clone)]
pub(crate) struct GltfPoolable {
    pub(super) source: PbGltfContainer,
    pub(super) gltf: AssetId<Gltf>,
    pub(super) named_nodes: HashMap<String, Entity>,
}

#[derive(Resource)]
pub struct GltfInstancePool {
    holder: Option<Entity>,
    entries: VecDeque<PooledInstance>,
    bytes_sum: usize,
    budget_bytes: usize,
    reused_count: u64,
    released_count: u64,
    evicted_count: u64,
}

impl Default for GltfInstancePool {
    fn default() -> Self {
        Self {
            holder: None,
            entries: VecDeque::new(),
            bytes_sum: 0,
            budget_bytes: GLTF_INSTANCE_POOL_BUDGET_BYTES,
            reused_count: 0,
            released_count: 0,
            evicted_count: 0,
        }
    }
}

pub(super) struct PooledInstance {
    root: Entity,
    key: GltfPoolable,
    instance: InstanceId,
    nodes: Vec<Entity>,
    top_nodes: Vec<Entity>,
    bytes: usize,
}

/// The pool's state for the debug panel.
pub struct GltfInstancePoolStats {
    pub instance_count: usize,
    pub node_count: usize,
    pub bytes_sum: usize,
    pub budget_bytes: usize,
    pub reused_count: u64,
    pub released_count: u64,
    pub evicted_count: u64,
}

impl GltfInstancePool {
    pub fn stats(&self) -> GltfInstancePoolStats {
        GltfInstancePoolStats {
            instance_count: self.entries.len(),
            node_count: self.entries.iter().map(|entry| entry.nodes.len()).sum(),
            bytes_sum: self.bytes_sum,
            budget_bytes: self.budget_bytes,
            reused_count: self.reused_count,
            released_count: self.released_count,
            evicted_count: self.evicted_count,
        }
    }

    /// The instance most recently pooled in `root` for `source` loaded as `gltf`.
    pub(super) fn take(
        &mut self,
        root: Entity,
        source: &PbGltfContainer,
        gltf: AssetId<Gltf>,
    ) -> Option<PooledInstance> {
        let index = self.entries.iter().rposition(|entry| {
            entry.root == root && entry.key.gltf == gltf && entry.key.source == *source
        })?;
        let entry = self.entries.remove(index)?;
        self.bytes_sum -= entry.bytes;
        Some(entry)
    }

    fn evict_over_budget(&mut self) -> Vec<PooledInstance> {
        let mut evicted = Vec::new();
        while self.bytes_sum > self.budget_bytes {
            let Some(entry) = self.entries.pop_front() else {
                break;
            };
            self.bytes_sum -= entry.bytes;
            self.evicted_count += 1;
            evicted.push(entry);
        }
        evicted
    }
}

/// Pools the processed instance `nodes` of `container` in scene `root`, or despawns them when the
/// container or the scene is going or a node is gone.
pub(super) fn release(
    world: &mut World,
    container: Entity,
    root: Entity,
    instance: InstanceId,
    nodes: Vec<Entity>,
    key: GltfPoolable,
) {
    let whole = world.get_entity(container).is_ok()
        && world.get_entity(root).is_ok()
        && nodes.iter().all(|node| world.get_entity(*node).is_ok());
    if !whole {
        despawn_nodes(world, &nodes);
        return;
    }

    let holder = holder(world);
    let top_nodes = nodes
        .iter()
        .copied()
        .filter(|node| {
            world
                .get::<ChildOf>(*node)
                .is_some_and(|child_of| child_of.parent() == container)
        })
        .collect::<Vec<_>>();
    // the colliders leave the scene with the container link they are found by
    for &node in &nodes {
        world.entity_mut(node).remove::<(
            HasCollider<CtCollider>,
            HasCollider<CtTrigger>,
            ContainerEntity,
        )>();
    }
    for &node in &top_nodes {
        world.entity_mut(node).insert(ChildOf(holder));
    }
    let bytes: usize = nodes.iter().map(|node| component_bytes(world, *node)).sum();

    let mut pool = world.resource_mut::<GltfInstancePool>();
    pool.released_count += 1;
    pool.bytes_sum += bytes;
    pool.entries.push_back(PooledInstance {
        root,
        key,
        instance,
        nodes,
        top_nodes,
        bytes,
    });
    let evicted = pool.evict_over_budget();
    for entry in evicted {
        despawn_nodes(world, &entry.nodes);
    }
}

/// Gives `entry` to `container`, scene entity `container_id` of `root`, as its processed model.
/// When the container or a node is gone, or its asset was reloaded since, the nodes are
/// despawned and the container, still without a `GltfLoaded`, spawns the model.
pub(super) fn rebind(
    world: &mut World,
    container: Entity,
    root: Entity,
    container_id: SceneEntityId,
    entry: PooledInstance,
) {
    let names = world
        .resource::<GltfNameCache>()
        .0
        .get(&entry.key.gltf)
        .cloned();
    let whole = world.get_entity(container).is_ok()
        && entry
            .nodes
            .iter()
            .all(|node| world.get_entity(*node).is_ok());
    let (true, Some(names)) = (whole, names) else {
        despawn_nodes(world, &entry.nodes);
        return;
    };

    for &node in &entry.nodes {
        world.entity_mut(node).insert(ContainerEntity {
            container,
            root,
            container_id,
        });
    }
    for &node in &entry.top_nodes {
        world.entity_mut(node).insert(ChildOf(container));
    }

    let PooledInstance {
        key,
        instance,
        nodes,
        ..
    } = entry;
    let node_paths = key.named_nodes.keys().cloned().collect();
    let named_nodes = key.named_nodes.clone();
    world.entity_mut(container).insert((
        GltfLoaded(Some(instance)),
        GltfProcessed {
            instance_id: Some(instance),
            named_nodes,
        },
        GltfReady(instance),
        GltfPooledNodes(nodes),
        key,
    ));

    if let Some(mut context) = world.get_mut::<RendererSceneContext>(root) {
        let GltfNames {
            meshes: mesh_names,
            skins: skin_names,
            materials: material_names,
            animations: animation_names,
        } = names;
        context.update_crdt(
            SceneComponentId::GLTF_CONTAINER_LOADING_STATE,
            CrdtType::LWW_ANY,
            container_id,
            &PbGltfContainerLoadingState {
                current_state: LoadingState::Finished as i32,
                node_paths,
                mesh_names,
                material_names,
                skin_names,
                animation_names,
            },
        );
    }
    world.resource_mut::<GltfInstancePool>().reused_count += 1;
}

/// Despawns the instances of scenes that are gone.
pub(super) fn drop_gone_scenes(
    mut commands: Commands,
    mut pool: ResMut<GltfInstancePool>,
    entities: Query<()>,
) {
    if pool
        .entries
        .iter()
        .all(|entry| entities.contains(entry.root))
    {
        return;
    }
    for entry in std::mem::take(&mut pool.entries) {
        if entities.contains(entry.root) {
            pool.entries.push_back(entry);
            continue;
        }
        pool.bytes_sum -= entry.bytes;
        for node in entry.nodes {
            if let Ok(mut node) = commands.get_entity(node) {
                node.try_despawn();
            }
        }
    }
}

fn holder(world: &mut World) -> Entity {
    let existing = world.resource::<GltfInstancePool>().holder;
    if let Some(holder) = existing.filter(|holder| world.get_entity(*holder).is_ok()) {
        return holder;
    }
    let holder = world
        .spawn((
            Name::new("glTF instance pool"),
            Transform::default(),
            Visibility::Hidden,
        ))
        .id();
    world.resource_mut::<GltfInstancePool>().holder = Some(holder);
    holder
}

/// The bytes of the components `entity` has, as the ECS stores them.
fn component_bytes(world: &World, entity: Entity) -> usize {
    let Ok(entity) = world.get_entity(entity) else {
        return 0;
    };
    let components = world.components();
    entity
        .archetype()
        .components()
        .filter_map(|id| components.get_info(id))
        .map(|info| info.layout().size())
        .sum()
}

/// Despawns each node on its own: a node may already be gone with its parent.
fn despawn_nodes(world: &mut World, nodes: &[Entity]) {
    for &node in nodes {
        if let Ok(node) = world.get_entity_mut(node) {
            node.despawn();
        }
    }
}

#[cfg(test)]
mod tests {
    use bevy::scene::SceneSpawner;

    use super::*;

    fn instance_id() -> InstanceId {
        SceneSpawner::default().spawn(Handle::<Scene>::default())
    }

    fn pooled(world: &mut World, root: Entity, budget_bytes: usize) -> (Entity, Vec<Entity>) {
        let container = world.spawn_empty().id();
        let top = world
            .spawn((
                ChildOf(container),
                Transform::default(),
                ContainerEntity {
                    container,
                    root,
                    container_id: SceneEntityId::new(512, 0),
                },
            ))
            .id();
        let child = world
            .spawn((
                ChildOf(top),
                Transform::default(),
                ContainerEntity {
                    container,
                    root,
                    container_id: SceneEntityId::new(512, 0),
                },
            ))
            .id();
        world.resource_mut::<GltfInstancePool>().budget_bytes = budget_bytes;
        release(
            world,
            container,
            root,
            instance_id(),
            vec![top, child],
            GltfPoolable {
                source: PbGltfContainer {
                    src: "model.glb".to_owned(),
                    ..Default::default()
                },
                gltf: AssetId::default(),
                named_nodes: HashMap::default(),
            },
        );
        (container, vec![top, child])
    }

    fn world() -> World {
        let mut world = World::new();
        world.init_resource::<GltfInstancePool>();
        world.init_resource::<GltfNameCache>();
        world
    }

    #[test]
    fn released_instance_is_hidden_and_unlinked() {
        let mut world = world();
        let root = world.spawn_empty().id();
        let (_, nodes) = pooled(&mut world, root, usize::MAX);

        let holder = world.resource::<GltfInstancePool>().holder.unwrap();
        assert_eq!(world.get::<ChildOf>(nodes[0]).unwrap().parent(), holder);
        assert_eq!(world.get::<Visibility>(holder), Some(&Visibility::Hidden));
        assert!(nodes
            .iter()
            .all(|node| world.get::<ContainerEntity>(*node).is_none()));
        let stats = world.resource::<GltfInstancePool>().stats();
        assert_eq!((stats.instance_count, stats.node_count), (1, 2));
        assert!(stats.bytes_sum > 0);
    }

    #[test]
    fn pooled_instance_goes_to_the_next_container_of_its_model() {
        let mut world = world();
        let root = world.spawn_empty().id();
        let (_, nodes) = pooled(&mut world, root, usize::MAX);
        world
            .resource_mut::<GltfNameCache>()
            .0
            .insert(AssetId::default(), GltfNames::default());
        let source = PbGltfContainer {
            src: "model.glb".to_owned(),
            ..Default::default()
        };

        let other = PbGltfContainer {
            src: "other.glb".to_owned(),
            ..Default::default()
        };
        assert!(world
            .resource_mut::<GltfInstancePool>()
            .take(root, &other, AssetId::default())
            .is_none());

        let entry = world
            .resource_mut::<GltfInstancePool>()
            .take(root, &source, AssetId::default())
            .unwrap();
        let container = world.spawn_empty().id();
        let id = SceneEntityId::new(513, 0);
        rebind(&mut world, container, root, id, entry);

        assert_eq!(world.get::<ChildOf>(nodes[0]).unwrap().parent(), container);
        assert_eq!(
            world.get::<ContainerEntity>(nodes[1]).unwrap().container_id,
            id
        );
        assert!(world.get::<GltfProcessed>(container).is_some());
        assert_eq!(world.get::<GltfPooledNodes>(container).unwrap().0, nodes);
        let stats = world.resource::<GltfInstancePool>().stats();
        assert_eq!((stats.instance_count, stats.bytes_sum), (0, 0));
        assert_eq!(stats.reused_count, 1);
    }

    #[test]
    fn oldest_instance_is_evicted_past_the_budget() {
        let mut world = world();
        let root = world.spawn_empty().id();
        let (_, first) = pooled(&mut world, root, usize::MAX);
        let one_instance = world.resource::<GltfInstancePool>().bytes_sum;
        let (_, second) = pooled(&mut world, root, one_instance);

        assert!(first.iter().all(|node| world.get_entity(*node).is_err()));
        assert!(second.iter().all(|node| world.get_entity(*node).is_ok()));
        let stats = world.resource::<GltfInstancePool>().stats();
        assert_eq!((stats.instance_count, stats.evicted_count), (1, 1));
        assert!(stats.bytes_sum <= stats.budget_bytes);
    }

    #[test]
    fn instance_of_a_gone_container_is_despawned() {
        let mut world = world();
        let root = world.spawn_empty().id();
        let container = world.spawn_empty().id();
        let node = world.spawn_empty().id();
        world.entity_mut(container).despawn();
        release(
            &mut world,
            container,
            root,
            instance_id(),
            vec![node],
            GltfPoolable {
                source: PbGltfContainer::default(),
                gltf: AssetId::default(),
                named_nodes: HashMap::default(),
            },
        );

        assert!(world.get_entity(node).is_err());
        assert_eq!(
            world.resource::<GltfInstancePool>().stats().instance_count,
            0
        );
    }
}
