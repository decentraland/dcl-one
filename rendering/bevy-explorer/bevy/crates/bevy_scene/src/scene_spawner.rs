use crate::{DynamicScene, Scene};
use bevy_asset::{AssetEvent, AssetId, Assets, Handle};
use bevy_ecs::{
    entity::{Entity, EntityHashMap},
    event::{Event, EventCursor, Events},
    hierarchy::{ChildOf, Children},
    query::QueryState,
    reflect::AppTypeRegistry,
    resource::Resource,
    world::{Mut, World, WorldId},
};
use bevy_platform::{
    collections::{HashMap, HashSet},
    time::Instant,
};
use bevy_reflect::{Reflect, TypeRegistry};
#[cfg(feature = "bevy_render")]
use bevy_render::view::visibility::Visibility;
use thiserror::Error;
use uuid::Uuid;

use crate::{DynamicSceneRoot, SceneRoot};
use bevy_derive::{Deref, DerefMut};
use bevy_ecs::{
    change_detection::ResMut,
    prelude::{Changed, Component, Without},
    system::{Commands, Query},
};
/// Triggered on a scene's parent entity when [`crate::SceneInstance`] becomes ready to use.
///
/// See also [`Trigger`], [`SceneSpawner::instance_is_ready`].
///
/// [`Trigger`]: bevy_ecs::observer::Trigger
#[derive(Clone, Copy, Debug, Eq, PartialEq, Event, Reflect)]
#[reflect(Debug, PartialEq, Clone)]
pub struct SceneInstanceReady {
    /// Instance which has been spawned.
    pub instance_id: InstanceId,
}

/// Information about a scene instance.
#[derive(Debug)]
pub struct InstanceInfo {
    /// Mapping of entities from the scene world to the instance world.
    pub entity_map: EntityHashMap<Entity>,
}

/// Unique id identifying a scene instance.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Reflect)]
#[reflect(Debug, PartialEq, Hash, Clone)]
pub struct InstanceId(Uuid);

impl InstanceId {
    fn new() -> Self {
        InstanceId(Uuid::new_v4())
    }
}

/// Handles spawning and despawning scenes in the world, either synchronously or batched through the [`scene_spawner_system`].
///
/// Synchronous methods: (Scene operations will take effect immediately)
/// - [`spawn_dynamic_sync`](Self::spawn_dynamic_sync)
/// - [`spawn_sync`](Self::spawn_sync)
/// - [`despawn_sync`](Self::despawn_sync)
/// - [`despawn_instance_sync`](Self::despawn_instance_sync)
/// - [`update_spawned_scenes`](Self::update_spawned_scenes)
/// - [`spawn_queued_scenes`](Self::spawn_queued_scenes)
/// - [`despawn_queued_scenes`](Self::despawn_queued_scenes)
/// - [`despawn_queued_instances`](Self::despawn_queued_instances)
///
/// Deferred methods: (Scene operations will be processed when the [`scene_spawner_system`] is run)
/// - [`spawn_dynamic`](Self::spawn_dynamic)
/// - [`spawn_dynamic_as_child`](Self::spawn_dynamic_as_child)
/// - [`spawn`](Self::spawn)
/// - [`spawn_as_child`](Self::spawn_as_child)
/// - [`spawn_as_child_hidden`](Self::spawn_as_child_hidden)
/// - [`despawn`](Self::despawn)
/// - [`despawn_instance`](Self::despawn_instance)
///
/// With a [spawn deadline](Self::set_spawn_deadline), queued [`Scene`] instances are written an
/// entity at a time, parents before children, one instance after another in request order, and
/// what is left when the deadline passes waits for the next run. The progress is on the entities:
/// those whose parent is written carry a marker until they are written themselves. A partial
/// instance's top-level entities are attached to its parent and kept [`Visibility::Hidden`] until
/// it is complete. Without a deadline, instances are written whole, as stock Bevy writes them.
/// A spawner keeps a query on the world it spawns into, so one spawner serves one [`World`].
///
/// [`Visibility::Hidden`]: https://docs.rs/bevy/0.16.1/bevy/render/view/enum.Visibility.html
#[derive(Default, Resource)]
pub struct SceneSpawner {
    pub(crate) spawned_dynamic_scenes: HashMap<AssetId<DynamicScene>, HashSet<InstanceId>>,
    pub(crate) spawned_instances: HashMap<InstanceId, InstanceInfo>,
    scene_asset_event_reader: EventCursor<AssetEvent<DynamicScene>>,
    dynamic_scenes_to_spawn: Vec<(Handle<DynamicScene>, InstanceId, Option<Entity>)>,
    scenes_to_spawn: Vec<(Handle<Scene>, InstanceId, Option<Entity>)>,
    scenes_to_despawn: Vec<AssetId<DynamicScene>>,
    instances_to_despawn: Vec<InstanceId>,
    scenes_with_parent: Vec<(InstanceId, Entity)>,
    // instances spawned as a child are forgotten when their parent is despawned
    instance_parents: HashMap<InstanceId, Entity>,
    instances_ready: Vec<(InstanceId, Option<Entity>)>,
    spawn_deadline: Option<Instant>,
    /// The [`Scene`] instance the last run left part-written.
    scene_in_progress: Option<SceneInProgress>,
    /// Finds the marked entities of the instance in progress, to write next.
    marked: Option<QueryState<(Entity, &'static SceneEntityToWrite)>>,
    /// Instances spawned with [`Self::spawn_as_child_hidden`], not complete or revealed yet: once
    /// complete, their top-level entities stay hidden until revealed.
    keep_hidden: HashSet<InstanceId>,
}

/// Marks an entity of the [`Scene`] instance in progress whose parent is written: a later step
/// writes the components of its scene entity, `source`, into it. Only the top-level entities carry
/// it at first, and writing an entity passes it on to the entity's children, so an instance is
/// written parents first. Bevy shows a child whose parent has no visibility yet, so a child must
/// never be written before its parent.
#[derive(Component, Clone, Copy)]
#[component(storage = "SparseSet")]
struct SceneEntityToWrite {
    instance_id: InstanceId,
    source: Entity,
}

/// A top-level entity of a [`Scene`] instance that the [`SceneSpawner`] keeps
/// [`Visibility::Hidden`], while the instance is part-written or until
/// [`SceneSpawner::reveal_instance_sync`]. It holds the visibility the entity's scene gives it.
/// Hiding it hides what inherits from it: a descendant with an explicit `Visibility::Visible`
/// still shows.
#[cfg(feature = "bevy_render")]
#[derive(Component)]
#[component(storage = "SparseSet")]
struct HiddenSceneRoot {
    visibility: Option<Visibility>,
}

/// The marked entities of the instance in progress.
type Marked = QueryState<(Entity, &'static SceneEntityToWrite)>;

/// A [`Scene`] instance being written into the world, an entity at a time, parents first.
struct SceneInProgress {
    handle: Handle<Scene>,
    instance_id: InstanceId,
    parent: Option<Entity>,
    /// The scene's world when writing began: another one means the asset was replaced.
    scene_world: WorldId,
    /// The instance's entity for every scene entity, all allocated before the first write so
    /// that references between them map. It becomes the instance's [`InstanceInfo`].
    entity_map: EntityHashMap<Entity>,
    /// How many scene entities are not written yet.
    unwritten: usize,
    /// Attached to the parent, with its top-level entities hidden until it is complete.
    hidden: bool,
    /// Started over once already because no entity was marked while some were unwritten.
    retried: bool,
}

impl SceneInProgress {
    /// Copies the scene's resources, allocates an entity for each of its entities and marks the
    /// top-level ones to be written first.
    fn start(
        world: &mut World,
        scene: &Scene,
        type_registry: &TypeRegistry,
        (handle, instance_id, parent): (Handle<Scene>, InstanceId, Option<Entity>),
    ) -> Result<Self, SceneSpawnError> {
        scene.write_resources_to_world(world, type_registry)?;
        let mut entity_map = EntityHashMap::default();
        scene.allocate_entities(world, &mut entity_map);
        for (source, root) in roots(scene, &entity_map) {
            world.entity_mut(root).insert(SceneEntityToWrite {
                instance_id,
                source,
            });
        }
        Ok(Self {
            handle,
            instance_id,
            parent,
            scene_world: scene.world.id(),
            unwritten: entity_map.len(),
            entity_map,
            hidden: false,
            retried: false,
        })
    }

    fn is_complete(&self) -> bool {
        self.unwritten == 0
    }

    /// Writes a marked entity and marks its children. `false` when no entity is marked: those
    /// left are out of the hierarchy's reach, or were despawned by others, or the scene changed.
    fn write_next(
        &mut self,
        world: &mut World,
        scene: &Scene,
        marked: &mut Marked,
        type_registry: &TypeRegistry,
    ) -> Result<bool, SceneSpawnError> {
        loop {
            let Some((entity, mark)) = marked
                .iter(world)
                .next()
                .map(|(entity, mark)| (entity, *mark))
            else {
                return Ok(false);
            };
            world.entity_mut(entity).remove::<SceneEntityToWrite>();
            if mark.instance_id != self.instance_id {
                // left by an instance that is gone
                continue;
            }
            if scene.world.get_entity(mark.source).is_err() {
                // the scene was changed in place
                return Ok(false);
            }

            scene.write_entity_to_world(world, mark.source, &mut self.entity_map, type_registry)?;
            self.unwritten = self.unwritten.saturating_sub(1);
            if self.hidden && !scene.world.entity(mark.source).contains::<ChildOf>() {
                // a top-level entity: its own visibility was just written over the hiding
                keep_root_hidden(world, entity);
            }
            for &child in scene
                .world
                .get::<Children>(mark.source)
                .into_iter()
                .flatten()
            {
                let Some(child_entity) = self.entity_map.get(&child) else {
                    continue;
                };
                if let Ok(mut child_entity) = world.get_entity_mut(*child_entity) {
                    child_entity.insert(SceneEntityToWrite {
                        instance_id: self.instance_id,
                        source: child,
                    });
                }
            }
            return Ok(true);
        }
    }

    /// Attaches the top-level entities to the parent, in the order a whole spawn attaches them,
    /// and hides them: hiding them hides everything written below.
    fn attach_and_hide(&mut self, world: &mut World, scene: &Scene) {
        for (source, root) in roots(scene, &self.entity_map) {
            if world.get_entity(root).is_err() {
                continue;
            }
            if let Some(mut parent) = self
                .parent
                .and_then(|parent| world.get_entity_mut(parent).ok())
            {
                parent.add_child(root);
            }
            hide_root(world, scene, source, root);
        }
        self.hidden = true;
    }

    /// Despawns whatever of the instance exists. Entities not written yet are attached to
    /// nothing, so despawning the parent misses them.
    fn despawn(self, world: &mut World) {
        for entity in self.entity_map.values() {
            if let Ok(entity) = world.get_entity_mut(*entity) {
                entity.despawn();
            }
        }
    }

    /// Despawns the entities never written, which have no components.
    fn despawn_unwritten(&self, world: &mut World) {
        for entity in self.entity_map.values() {
            if world
                .get_entity(*entity)
                .is_ok_and(|entity| entity.archetype().component_count() == 0)
            {
                world.despawn(*entity);
            }
        }
    }

    /// Despawns what was written and returns the request, to start over.
    fn restart(self, world: &mut World) -> (Handle<Scene>, InstanceId, Option<Entity>) {
        let request = (self.handle.clone(), self.instance_id, self.parent);
        self.despawn(world);
        request
    }
}

/// A scene's top-level entities and their entities in an instance, in the order
/// [`SceneSpawner::set_scene_instance_parent_sync`] attaches a whole instance's. Entries for
/// entities outside the scene, which the entity mapper adds, are skipped.
fn roots<'a>(
    scene: &'a Scene,
    entity_map: &'a EntityHashMap<Entity>,
) -> impl Iterator<Item = (Entity, Entity)> + 'a {
    entity_map
        .iter()
        .filter(|(source, _)| {
            scene
                .world
                .get_entity(**source)
                .is_ok_and(|source| !source.contains::<ChildOf>())
        })
        .map(|(source, entity)| (*source, *entity))
}

/// Hides a top-level entity, keeping the visibility its scene entity `source` gives it.
#[cfg(feature = "bevy_render")]
fn hide_root(world: &mut World, scene: &Scene, source: Entity, root: Entity) {
    let visibility = scene.world.get::<Visibility>(source).copied();
    if let Ok(mut root) = world.get_entity_mut(root) {
        if !root.contains::<HiddenSceneRoot>() {
            root.insert(HiddenSceneRoot { visibility });
        }
        root.insert(Visibility::Hidden);
    }
}

#[cfg(not(feature = "bevy_render"))]
fn hide_root(_world: &mut World, _scene: &Scene, _source: Entity, _root: Entity) {}

/// Hides a hidden top-level entity again, once writing it has applied its scene's visibility.
#[cfg(feature = "bevy_render")]
fn keep_root_hidden(world: &mut World, root: Entity) {
    if let Ok(mut root) = world.get_entity_mut(root) {
        if root.contains::<HiddenSceneRoot>() {
            root.insert(Visibility::Hidden);
        }
    }
}

#[cfg(not(feature = "bevy_render"))]
fn keep_root_hidden(_world: &mut World, _root: Entity) {}

/// Gives each hidden top-level entity of an instance the visibility its scene gives it, while it
/// still holds the spawner's `Hidden`: a visibility something set meanwhile stays. One whose
/// scene gives it none is left `Inherited`, since something may require the visibility
/// components by now.
#[cfg(feature = "bevy_render")]
fn show_instance(world: &mut World, entity_map: &EntityHashMap<Entity>) {
    for entity in entity_map.values() {
        if world.get::<HiddenSceneRoot>(*entity).is_none() {
            continue;
        }
        let mut root = world.entity_mut(*entity);
        let Some(HiddenSceneRoot { visibility }) = root.take::<HiddenSceneRoot>() else {
            continue;
        };
        if root.get::<Visibility>() == Some(&Visibility::Hidden) {
            root.insert(visibility.unwrap_or(Visibility::Inherited));
        }
    }
}

#[cfg(not(feature = "bevy_render"))]
fn show_instance(_world: &mut World, _entity_map: &EntityHashMap<Entity>) {}

/// Errors that can occur when spawning a scene.
#[derive(Error, Debug)]
pub enum SceneSpawnError {
    /// Scene contains an unregistered component type.
    #[error("scene contains the unregistered component `{type_path}`. consider adding `#[reflect(Component)]` to your type")]
    UnregisteredComponent {
        /// Type of the unregistered component.
        type_path: String,
    },
    /// Scene contains an unregistered resource type.
    #[error("scene contains the unregistered resource `{type_path}`. consider adding `#[reflect(Resource)]` to your type")]
    UnregisteredResource {
        /// Type of the unregistered resource.
        type_path: String,
    },
    /// Scene contains an unregistered type.
    #[error(
        "scene contains the unregistered type `{std_type_name}`. \
        consider reflecting it with `#[derive(Reflect)]` \
        and registering the type using `app.register_type::<T>()`"
    )]
    UnregisteredType {
        /// The [type name](std::any::type_name) for the unregistered type.
        std_type_name: String,
    },
    /// Scene contains an unregistered type which has a `TypePath`.
    #[error(
        "scene contains the reflected type `{type_path}` but it was not found in the type registry. \
        consider registering the type using `app.register_type::<T>()``"
    )]
    UnregisteredButReflectedType {
        /// The unregistered type.
        type_path: String,
    },
    /// Scene contains a proxy without a represented type.
    #[error("scene contains dynamic type `{type_path}` without a represented type. consider changing this using `set_represented_type`.")]
    NoRepresentedType {
        /// The dynamic instance type.
        type_path: String,
    },
    /// Dynamic scene with the given id does not exist.
    #[error("scene does not exist")]
    NonExistentScene {
        /// Id of the non-existent dynamic scene.
        id: AssetId<DynamicScene>,
    },
    /// Scene with the given id does not exist.
    #[error("scene does not exist")]
    NonExistentRealScene {
        /// Id of the non-existent scene.
        id: AssetId<Scene>,
    },
}

impl SceneSpawner {
    /// Schedule the spawn of a new instance of the provided dynamic scene.
    pub fn spawn_dynamic(&mut self, id: impl Into<Handle<DynamicScene>>) -> InstanceId {
        let instance_id = InstanceId::new();
        self.dynamic_scenes_to_spawn
            .push((id.into(), instance_id, None));
        instance_id
    }

    /// Schedule the spawn of a new instance of the provided dynamic scene as a child of `parent`.
    pub fn spawn_dynamic_as_child(
        &mut self,
        id: impl Into<Handle<DynamicScene>>,
        parent: Entity,
    ) -> InstanceId {
        let instance_id = InstanceId::new();
        self.dynamic_scenes_to_spawn
            .push((id.into(), instance_id, Some(parent)));
        self.scenes_with_parent.push((instance_id, parent));
        self.instance_parents.insert(instance_id, parent);
        instance_id
    }

    /// Schedule the spawn of a new instance of the provided scene.
    pub fn spawn(&mut self, id: impl Into<Handle<Scene>>) -> InstanceId {
        let instance_id = InstanceId::new();
        self.scenes_to_spawn.push((id.into(), instance_id, None));
        instance_id
    }

    /// Schedule the spawn of a new instance of the provided scene as a child of `parent`.
    pub fn spawn_as_child(&mut self, id: impl Into<Handle<Scene>>, parent: Entity) -> InstanceId {
        let instance_id = InstanceId::new();
        self.scenes_to_spawn
            .push((id.into(), instance_id, Some(parent)));
        self.scenes_with_parent.push((instance_id, parent));
        self.instance_parents.insert(instance_id, parent);
        instance_id
    }

    /// Schedule the despawn of all instances of the provided dynamic scene.
    pub fn despawn(&mut self, id: impl Into<AssetId<DynamicScene>>) {
        self.scenes_to_despawn.push(id.into());
    }

    /// Schedule the despawn of a scene instance, removing all its entities from the world.
    ///
    /// Note: this will despawn _all_ entities associated with this instance, including those
    /// that have been removed from the scene hierarchy. To despawn _only_ entities still in the hierarchy,
    /// despawn the relevant root entity directly.
    pub fn despawn_instance(&mut self, instance_id: InstanceId) {
        self.instances_to_despawn.push(instance_id);
    }

    /// This will remove all records of this instance, without despawning any entities.
    /// If the instance has not spawned yet, it will not be spawned. An instance that is part-written
    /// is cancelled, and the next run of the [`scene_spawner_system`] despawns what was written.
    pub fn unregister_instance(&mut self, instance_id: InstanceId) {
        self.spawned_instances.remove(&instance_id);
        self.forget(instance_id);
        if self
            .scene_in_progress
            .as_ref()
            .is_some_and(|progress| progress.instance_id == instance_id)
        {
            // what is written is no use: the next run despawns it before it spawns anything
            self.instances_to_despawn.push(instance_id);
        }
    }

    /// Bounds how long the next runs of [`Self::spawn_queued_scenes`] write queued [`Scene`]
    /// instances: once `deadline` has passed, the rest waits for a later run, though each run
    /// writes at least one entity. `None`, the default, spawns every queued instance whole.
    /// Dynamic scenes always spawn whole.
    pub fn set_spawn_deadline(&mut self, deadline: Option<Instant>) {
        self.spawn_deadline = deadline;
    }

    /// The deadline [`Self::set_spawn_deadline`] set.
    pub fn spawn_deadline(&self) -> Option<Instant> {
        self.spawn_deadline
    }

    /// Like [`Self::spawn_as_child`], but the instance's top-level entities stay
    /// [`Visibility::Hidden`] once it has spawned, until [`Self::reveal_instance_sync`].
    ///
    /// [`Visibility::Hidden`]: https://docs.rs/bevy/0.16.1/bevy/render/view/enum.Visibility.html
    pub fn spawn_as_child_hidden(
        &mut self,
        id: impl Into<Handle<Scene>>,
        parent: Entity,
    ) -> InstanceId {
        let instance_id = self.spawn_as_child(id, parent);
        self.keep_hidden.insert(instance_id);
        instance_id
    }

    /// Gives the top-level entities of an instance spawned with [`Self::spawn_as_child_hidden`]
    /// the visibility its scene gives them; an instance not complete yet shows when it completes.
    pub fn reveal_instance_sync(&mut self, world: &mut World, instance_id: InstanceId) {
        self.keep_hidden.remove(&instance_id);
        if let Some(instance) = self.spawned_instances.get(&instance_id) {
            show_instance(world, &instance.entity_map);
        }
    }

    /// Drops a cancelled instance from the queues and the parent bookkeeping: one still queued
    /// would otherwise spawn later without its parent, and one part-written would wait for its
    /// parent forever.
    fn forget(&mut self, instance_id: InstanceId) {
        self.instance_parents.remove(&instance_id);
        self.scenes_with_parent.retain(|(id, _)| *id != instance_id);
        self.scenes_to_spawn.retain(|(_, id, _)| *id != instance_id);
        self.dynamic_scenes_to_spawn
            .retain(|(_, id, _)| *id != instance_id);
        self.keep_hidden.remove(&instance_id);
    }

    fn take_in_progress(&mut self, instance_id: InstanceId) -> Option<SceneInProgress> {
        self.scene_in_progress
            .take_if(|progress| progress.instance_id == instance_id)
    }

    /// Immediately despawns all instances of a dynamic scene.
    pub fn despawn_sync(
        &mut self,
        world: &mut World,
        id: impl Into<AssetId<DynamicScene>>,
    ) -> Result<(), SceneSpawnError> {
        if let Some(instance_ids) = self.spawned_dynamic_scenes.remove(&id.into()) {
            for instance_id in instance_ids {
                self.despawn_instance_sync(world, &instance_id);
            }
        }
        Ok(())
    }

    /// Immediately despawns a scene instance, removing all its entities from the world.
    pub fn despawn_instance_sync(&mut self, world: &mut World, instance_id: &InstanceId) {
        self.forget(*instance_id);
        if let Some(progress) = self.take_in_progress(*instance_id) {
            progress.despawn(world);
        }
        if let Some(instance) = self.spawned_instances.remove(instance_id) {
            for &entity in instance.entity_map.values() {
                if let Ok(entity_mut) = world.get_entity_mut(entity) {
                    entity_mut.despawn();
                };
            }
        }
    }

    /// Immediately spawns a new instance of the provided dynamic scene.
    pub fn spawn_dynamic_sync(
        &mut self,
        world: &mut World,
        id: impl Into<AssetId<DynamicScene>>,
    ) -> Result<InstanceId, SceneSpawnError> {
        let mut entity_map = EntityHashMap::default();
        let id = id.into();
        Self::spawn_dynamic_internal(world, id, &mut entity_map)?;
        let instance_id = InstanceId::new();
        self.spawned_instances
            .insert(instance_id, InstanceInfo { entity_map });
        let spawned = self.spawned_dynamic_scenes.entry(id).or_default();
        spawned.insert(instance_id);
        Ok(instance_id)
    }

    fn spawn_dynamic_internal(
        world: &mut World,
        id: AssetId<DynamicScene>,
        entity_map: &mut EntityHashMap<Entity>,
    ) -> Result<(), SceneSpawnError> {
        world.resource_scope(|world, scenes: Mut<Assets<DynamicScene>>| {
            let scene = scenes
                .get(id)
                .ok_or(SceneSpawnError::NonExistentScene { id })?;

            scene.write_to_world(world, entity_map)
        })
    }

    /// Immediately spawns a new instance of the provided scene.
    pub fn spawn_sync(
        &mut self,
        world: &mut World,
        id: impl Into<AssetId<Scene>>,
    ) -> Result<InstanceId, SceneSpawnError> {
        let mut entity_map = EntityHashMap::default();
        let id = id.into();
        Self::spawn_sync_internal(world, id, &mut entity_map)?;
        let instance_id = InstanceId::new();
        self.spawned_instances
            .insert(instance_id, InstanceInfo { entity_map });
        Ok(instance_id)
    }

    fn spawn_sync_internal(
        world: &mut World,
        id: AssetId<Scene>,
        entity_map: &mut EntityHashMap<Entity>,
    ) -> Result<(), SceneSpawnError> {
        world.resource_scope(|world, scenes: Mut<Assets<Scene>>| {
            let scene = scenes
                .get(id)
                .ok_or(SceneSpawnError::NonExistentRealScene { id })?;

            scene.write_to_world_with(
                world,
                entity_map,
                &world.resource::<AppTypeRegistry>().clone(),
            )
        })
    }

    /// Iterate through all instances of the provided scenes and update those immediately.
    ///
    /// Useful for updating already spawned scene instances after their corresponding scene has been modified.
    pub fn update_spawned_scenes(
        &mut self,
        world: &mut World,
        scene_ids: &[AssetId<DynamicScene>],
    ) -> Result<(), SceneSpawnError> {
        for id in scene_ids {
            if let Some(spawned_instances) = self.spawned_dynamic_scenes.get(id) {
                for instance_id in spawned_instances {
                    if let Some(instance_info) = self.spawned_instances.get_mut(instance_id) {
                        Self::spawn_dynamic_internal(world, *id, &mut instance_info.entity_map)?;
                    }
                }
            }
        }
        Ok(())
    }

    /// Immediately despawns all scenes scheduled for despawn by despawning their instances.
    pub fn despawn_queued_scenes(&mut self, world: &mut World) -> Result<(), SceneSpawnError> {
        let scenes_to_despawn = core::mem::take(&mut self.scenes_to_despawn);

        for scene_handle in scenes_to_despawn {
            self.despawn_sync(world, scene_handle)?;
        }
        Ok(())
    }

    /// Immediately despawns all scene instances scheduled for despawn.
    pub fn despawn_queued_instances(&mut self, world: &mut World) {
        let instances_to_despawn = core::mem::take(&mut self.instances_to_despawn);

        for instance_id in instances_to_despawn {
            self.despawn_instance_sync(world, &instance_id);
        }
    }

    /// Immediately spawns all scenes scheduled for spawn.
    pub fn spawn_queued_scenes(&mut self, world: &mut World) -> Result<(), SceneSpawnError> {
        let scenes_to_spawn = core::mem::take(&mut self.dynamic_scenes_to_spawn);

        for (handle, instance_id, parent) in scenes_to_spawn {
            let mut entity_map = EntityHashMap::default();

            match Self::spawn_dynamic_internal(world, handle.id(), &mut entity_map) {
                Ok(_) => {
                    self.spawned_instances
                        .insert(instance_id, InstanceInfo { entity_map });
                    let spawned = self.spawned_dynamic_scenes.entry(handle.id()).or_default();
                    spawned.insert(instance_id);

                    // Scenes with parents need more setup before they are ready.
                    // See `set_scene_instance_parent_sync()`.
                    if parent.is_none() {
                        // We trigger `SceneInstanceReady` events after processing all scenes
                        // SceneSpawner may not be available in the observer.
                        self.instances_ready.push((instance_id, None));
                    }
                }
                Err(SceneSpawnError::NonExistentScene { .. }) => {
                    self.dynamic_scenes_to_spawn
                        .push((handle, instance_id, parent));
                }
                Err(err) => return Err(err),
            }
        }

        self.spawn_queued_real_scenes(world)
    }

    /// Writes queued [`Scene`] instances an entity at a time, parents first, in request order,
    /// until the [spawn deadline](Self::set_spawn_deadline); without one, writes them whole.
    /// Instances whose scene is not loaded wait.
    fn spawn_queued_real_scenes(&mut self, world: &mut World) -> Result<(), SceneSpawnError> {
        if self.scene_in_progress.is_none() && self.scenes_to_spawn.is_empty() {
            return Ok(());
        }
        let type_registry = world.resource::<AppTypeRegistry>().clone();
        let type_registry = type_registry.read();
        let deadline = self.spawn_deadline;
        // every run writes at least one entity
        let has_time = |wrote_any: bool| {
            !wrote_any || deadline.is_none_or(|deadline| Instant::now() < deadline)
        };
        let mut marked = self.marked.take().unwrap_or_else(|| world.query());

        let result = world.resource_scope(|world, scenes: Mut<Assets<Scene>>| {
            let mut wrote_any = false;
            while has_time(wrote_any) {
                let mut progress = match self.scene_in_progress.take() {
                    Some(progress) => progress,
                    None => {
                        let Some(next) = self
                            .scenes_to_spawn
                            .iter()
                            .position(|(handle, ..)| scenes.contains(handle.id()))
                        else {
                            break;
                        };
                        let request = self.scenes_to_spawn.remove(next);
                        let scene = scenes.get(request.0.id()).expect("checked above");
                        if deadline.is_none() {
                            self.spawn_whole(world, scene, &type_registry, request)?;
                            wrote_any = true;
                            continue;
                        }
                        self.start(world, scene, &type_registry, request)?
                    }
                };

                // a scene removed or replaced since writing began: start over from what is there
                let Some(scene) = scenes
                    .get(progress.handle.id())
                    .filter(|scene| scene.world.id() == progress.scene_world)
                else {
                    self.scenes_to_spawn.insert(0, progress.restart(world));
                    continue;
                };

                let mut stuck = false;
                while !progress.is_complete() && has_time(wrote_any) {
                    match progress.write_next(world, scene, &mut marked, &type_registry) {
                        Ok(true) => wrote_any = true,
                        Ok(false) => {
                            stuck = true;
                            break;
                        }
                        Err(error) => {
                            // nothing of an instance that failed stays behind
                            let instance_id = progress.instance_id;
                            progress.despawn(world);
                            self.forget(instance_id);
                            return Err(error);
                        }
                    }
                }

                if progress.is_complete() {
                    let SceneInProgress {
                        instance_id,
                        parent,
                        entity_map,
                        hidden,
                        ..
                    } = progress;
                    self.finish(world, scene, (instance_id, parent), entity_map, hidden);
                } else if stuck {
                    if roots(scene, &progress.entity_map)
                        .any(|(_, root)| world.get_entity(root).is_err())
                    {
                        // a top-level entity despawned by others: what was never written goes, and
                        // the instance completes with what is left, as a whole spawn whose root
                        // was despawned afterwards would be, so its ready event still fires
                        progress.despawn_unwritten(world);
                        let SceneInProgress {
                            instance_id,
                            parent,
                            entity_map,
                            hidden,
                            ..
                        } = progress;
                        self.finish(world, scene, (instance_id, parent), entity_map, hidden);
                    } else if progress.retried {
                        // stuck again: the hierarchy does not reach every entity, so write it whole
                        let request = progress.restart(world);
                        self.spawn_whole(world, scene, &type_registry, request)?;
                        wrote_any = true;
                    } else {
                        // an entity despawned by others, or the scene changed in place: start over,
                        // still in slices
                        let request = progress.restart(world);
                        let mut progress = self.start(world, scene, &type_registry, request)?;
                        progress.retried = true;
                        self.scene_in_progress = Some(progress);
                    }
                } else {
                    // out of time: what is written stays out of view until it is complete
                    if !progress.hidden {
                        progress.attach_and_hide(world, scene);
                    }
                    self.scene_in_progress = Some(progress);
                }
            }
            Ok(())
        });
        self.marked = Some(marked);
        result
    }

    /// Starts writing a queued instance; one that fails to start is forgotten.
    fn start(
        &mut self,
        world: &mut World,
        scene: &Scene,
        type_registry: &TypeRegistry,
        request: (Handle<Scene>, InstanceId, Option<Entity>),
    ) -> Result<SceneInProgress, SceneSpawnError> {
        let instance_id = request.1;
        SceneInProgress::start(world, scene, type_registry, request)
            .inspect_err(|_| self.forget(instance_id))
    }

    /// Writes a queued instance whole, as stock Bevy does.
    fn spawn_whole(
        &mut self,
        world: &mut World,
        scene: &Scene,
        type_registry: &TypeRegistry,
        (_, instance_id, parent): (Handle<Scene>, InstanceId, Option<Entity>),
    ) -> Result<(), SceneSpawnError> {
        let mut entity_map = EntityHashMap::default();
        scene.write_to_world_with_types(world, &mut entity_map, type_registry)?;
        self.finish(world, scene, (instance_id, parent), entity_map, false);
        Ok(())
    }

    /// Records a complete instance. Its top-level entities are shown, unless it was spawned with
    /// [`Self::spawn_as_child_hidden`] and not revealed yet.
    fn finish(
        &mut self,
        world: &mut World,
        scene: &Scene,
        (instance_id, parent): (InstanceId, Option<Entity>),
        entity_map: EntityHashMap<Entity>,
        hidden: bool,
    ) {
        match (self.keep_hidden.remove(&instance_id), hidden) {
            (true, false) => {
                for (source, root) in roots(scene, &entity_map) {
                    hide_root(world, scene, source, root);
                }
            }
            (false, true) => show_instance(world, &entity_map),
            _ => {}
        }

        self.spawned_instances
            .insert(instance_id, InstanceInfo { entity_map });

        // Scenes with parents need more setup before they are ready.
        // See `set_scene_instance_parent_sync()`.
        if parent.is_none() {
            // We trigger `SceneInstanceReady` events after processing all scenes
            // SceneSpawner may not be available in the observer.
            self.instances_ready.push((instance_id, None));
        }
    }

    pub(crate) fn set_scene_instance_parent_sync(&mut self, world: &mut World) {
        let scenes_with_parent = core::mem::take(&mut self.scenes_with_parent);

        for (instance_id, parent) in scenes_with_parent {
            if let Some(instance) = self.spawned_instances.get(&instance_id) {
                for &entity in instance.entity_map.values() {
                    // Add the `ChildOf` component to the scene root, and update the `Children` component of
                    // the scene parent
                    if !world
                        .get_entity(entity)
                        .ok()
                        // This will filter only the scene root entity, as all other from the
                        // scene have a parent
                        // Entities that wouldn't exist anymore are also skipped
                        // this case shouldn't happen anyway
                        .is_none_or(|entity| entity.contains::<ChildOf>())
                    {
                        world.entity_mut(parent).add_child(entity);
                    }
                }

                // We trigger `SceneInstanceReady` events after processing all scenes
                // SceneSpawner may not be available in the observer.
                self.instances_ready.push((instance_id, Some(parent)));
            } else {
                self.scenes_with_parent.push((instance_id, parent));
            }
        }
    }

    fn trigger_scene_ready_events(&mut self, world: &mut World) {
        for (instance_id, parent) in self.instances_ready.drain(..) {
            if let Some(parent) = parent {
                // Defer via commands otherwise SceneSpawner is not available in the observer.
                world
                    .commands()
                    .trigger_targets(SceneInstanceReady { instance_id }, parent);
            } else {
                // Defer via commands otherwise SceneSpawner is not available in the observer.
                world.commands().trigger(SceneInstanceReady { instance_id });
            }
        }
    }

    /// Check that a scene instance spawned previously is ready to use
    pub fn instance_is_ready(&self, instance_id: InstanceId) -> bool {
        self.spawned_instances.contains_key(&instance_id)
    }

    /// Get an iterator over the entities in an instance, once it's spawned.
    ///
    /// Before the scene is spawned, the iterator will be empty. Use [`Self::instance_is_ready`]
    /// to check if the instance is ready.
    pub fn iter_instance_entities(
        &'_ self,
        instance_id: InstanceId,
    ) -> impl Iterator<Item = Entity> + '_ {
        self.spawned_instances
            .get(&instance_id)
            .map(|instance| instance.entity_map.values())
            .into_iter()
            .flatten()
            .copied()
    }
}

/// System that handles scheduled scene instance spawning and despawning through a [`SceneSpawner`].
pub fn scene_spawner_system(world: &mut World) {
    world.resource_scope(|world, mut scene_spawner: Mut<SceneSpawner>| {
        // forget any instances (loading or spawned) where parent is deleted
        let mut dead_instances = <HashSet<_>>::default();
        scene_spawner.instance_parents.retain(|instance, parent| {
            let retain = world.get_entity(*parent).is_ok();

            if !retain {
                dead_instances.insert(*instance);
            }

            retain
        });
        for instance in &dead_instances {
            scene_spawner.spawned_instances.remove(instance);
            scene_spawner.keep_hidden.remove(instance);
            if let Some(progress) = scene_spawner.take_in_progress(*instance) {
                progress.despawn(world);
            }
        }
        scene_spawner
            .scenes_with_parent
            .retain(|(instance, _)| !dead_instances.contains(instance));
        scene_spawner
            .dynamic_scenes_to_spawn
            .retain(|(_, instance, _)| !dead_instances.contains(instance));
        scene_spawner
            .scenes_to_spawn
            .retain(|(_, instance, _)| !dead_instances.contains(instance));

        let scene_asset_events = world.resource::<Events<AssetEvent<DynamicScene>>>();

        let mut updated_spawned_scenes = Vec::new();
        let scene_spawner = &mut *scene_spawner;
        for event in scene_spawner
            .scene_asset_event_reader
            .read(scene_asset_events)
        {
            if let AssetEvent::Modified { id } = event {
                if scene_spawner.spawned_dynamic_scenes.contains_key(id) {
                    updated_spawned_scenes.push(*id);
                }
            }
        }

        scene_spawner.despawn_queued_scenes(world).unwrap();
        scene_spawner.despawn_queued_instances(world);
        scene_spawner
            .spawn_queued_scenes(world)
            .unwrap_or_else(|err| panic!("{}", err));
        scene_spawner
            .update_spawned_scenes(world, &updated_spawned_scenes)
            .unwrap();
        scene_spawner.set_scene_instance_parent_sync(world);
        scene_spawner.trigger_scene_ready_events(world);
    });
}

/// [`InstanceId`] of a spawned scene. It can be used with the [`SceneSpawner`] to
/// interact with the spawned scene.
#[derive(Component, Deref, DerefMut)]
pub struct SceneInstance(pub(crate) InstanceId);

/// System that will spawn scenes from the [`SceneRoot`] and [`DynamicSceneRoot`] components.
pub fn scene_spawner(
    mut commands: Commands,
    mut scene_to_spawn: Query<
        (Entity, &SceneRoot, Option<&mut SceneInstance>),
        (Changed<SceneRoot>, Without<DynamicSceneRoot>),
    >,
    mut dynamic_scene_to_spawn: Query<
        (Entity, &DynamicSceneRoot, Option<&mut SceneInstance>),
        (Changed<DynamicSceneRoot>, Without<SceneRoot>),
    >,
    mut scene_spawner: ResMut<SceneSpawner>,
) {
    for (entity, scene, instance) in &mut scene_to_spawn {
        let new_instance = scene_spawner.spawn_as_child(scene.0.clone(), entity);
        if let Some(mut old_instance) = instance {
            scene_spawner.despawn_instance(**old_instance);
            *old_instance = SceneInstance(new_instance);
        } else {
            commands.entity(entity).insert(SceneInstance(new_instance));
        }
    }
    for (entity, dynamic_scene, instance) in &mut dynamic_scene_to_spawn {
        let new_instance = scene_spawner.spawn_dynamic_as_child(dynamic_scene.0.clone(), entity);
        if let Some(mut old_instance) = instance {
            scene_spawner.despawn_instance(**old_instance);
            *old_instance = SceneInstance(new_instance);
        } else {
            commands.entity(entity).insert(SceneInstance(new_instance));
        }
    }
}

#[cfg(test)]
mod resumable_tests;
#[cfg(test)]
mod spawn_bench;

#[cfg(test)]
mod tests {
    use bevy_app::App;
    use bevy_asset::{AssetPlugin, AssetServer, Handle};
    use bevy_ecs::{
        component::Component,
        hierarchy::Children,
        observer::Trigger,
        prelude::ReflectComponent,
        query::With,
        system::{Commands, Query, Res, ResMut, RunSystemOnce},
    };
    use bevy_reflect::Reflect;

    use crate::{DynamicSceneBuilder, DynamicSceneRoot, ScenePlugin};

    use super::*;
    use crate::{DynamicScene, SceneSpawner};
    use bevy_app::ScheduleRunnerPlugin;
    use bevy_asset::Assets;
    use bevy_ecs::{
        entity::Entity,
        prelude::{AppTypeRegistry, World},
    };

    #[derive(Component, Reflect, Default)]
    #[reflect(Component)]
    struct ComponentA {
        pub x: f32,
        pub y: f32,
    }

    #[test]
    fn spawn_and_delete() {
        let mut app = App::new();

        app.add_plugins(ScheduleRunnerPlugin::default())
            .add_plugins(AssetPlugin::default())
            .add_plugins(ScenePlugin)
            .register_type::<ComponentA>();
        app.update();

        let mut scene_world = World::new();

        // create a new DynamicScene manually
        let type_registry = app.world().resource::<AppTypeRegistry>().clone();
        scene_world.insert_resource(type_registry);
        scene_world.spawn(ComponentA { x: 3.0, y: 4.0 });
        let scene = DynamicScene::from_world(&scene_world);
        let scene_handle = app
            .world_mut()
            .resource_mut::<Assets<DynamicScene>>()
            .add(scene);

        // spawn the scene as a child of `entity` using `DynamicSceneRoot`
        let entity = app
            .world_mut()
            .spawn(DynamicSceneRoot(scene_handle.clone()))
            .id();

        // run the app's schedule once, so that the scene gets spawned
        app.update();

        // make sure that the scene was added as a child of the root entity
        let (scene_entity, scene_component_a) = app
            .world_mut()
            .query::<(Entity, &ComponentA)>()
            .single(app.world())
            .unwrap();
        assert_eq!(scene_component_a.x, 3.0);
        assert_eq!(scene_component_a.y, 4.0);
        assert_eq!(
            app.world().entity(entity).get::<Children>().unwrap().len(),
            1
        );

        // let's try to delete the scene
        let mut scene_spawner = app.world_mut().resource_mut::<SceneSpawner>();
        scene_spawner.despawn(&scene_handle);

        // run the scene spawner system to despawn the scene
        app.update();

        // the scene entity does not exist anymore
        assert!(app.world().get_entity(scene_entity).is_err());

        // the root entity does not have any children anymore
        assert!(app.world().entity(entity).get::<Children>().is_none());
    }

    #[derive(Reflect, Component, Debug, PartialEq, Eq, Clone, Copy, Default)]
    #[reflect(Component)]
    struct A(usize);

    #[test]
    fn clone_dynamic_entities() {
        let mut world = World::default();

        // setup
        let atr = AppTypeRegistry::default();
        atr.write().register::<A>();
        world.insert_resource(atr);
        world.insert_resource(Assets::<DynamicScene>::default());

        // start test
        world.spawn(A(42));

        assert_eq!(world.query::<&A>().iter(&world).len(), 1);

        // clone only existing entity
        let mut scene_spawner = SceneSpawner::default();
        let entity = world
            .query_filtered::<Entity, With<A>>()
            .single(&world)
            .unwrap();
        let scene = DynamicSceneBuilder::from_world(&world)
            .extract_entity(entity)
            .build();

        let scene_id = world.resource_mut::<Assets<DynamicScene>>().add(scene);
        let instance_id = scene_spawner
            .spawn_dynamic_sync(&mut world, &scene_id)
            .unwrap();

        // verify we spawned exactly one new entity with our expected component
        assert_eq!(world.query::<&A>().iter(&world).len(), 2);

        // verify that we can get this newly-spawned entity by the instance ID
        let new_entity = scene_spawner
            .iter_instance_entities(instance_id)
            .next()
            .unwrap();

        // verify this is not the original entity
        assert_ne!(entity, new_entity);

        // verify this new entity contains the same data as the original entity
        let [old_a, new_a] = world
            .query::<&A>()
            .get_many(&world, [entity, new_entity])
            .unwrap();
        assert_eq!(old_a, new_a);
    }

    #[derive(Component, Reflect, Default)]
    #[reflect(Component)]
    struct ComponentF;

    #[derive(Resource, Default)]
    struct TriggerCount(u32);

    fn setup() -> App {
        let mut app = App::new();
        app.add_plugins((AssetPlugin::default(), ScenePlugin));
        app.init_resource::<TriggerCount>();

        app.register_type::<ComponentF>();
        app.world_mut().spawn(ComponentF);
        app.world_mut().spawn(ComponentF);

        app
    }

    fn build_scene(app: &mut App) -> Handle<Scene> {
        app.world_mut()
            .run_system_once(
                |world: &World,
                 type_registry: Res<'_, AppTypeRegistry>,
                 asset_server: Res<'_, AssetServer>| {
                    asset_server.add(
                        Scene::from_dynamic_scene(&DynamicScene::from_world(world), &type_registry)
                            .unwrap(),
                    )
                },
            )
            .expect("Failed to run scene builder system.")
    }

    fn build_dynamic_scene(app: &mut App) -> Handle<DynamicScene> {
        app.world_mut()
            .run_system_once(|world: &World, asset_server: Res<'_, AssetServer>| {
                asset_server.add(DynamicScene::from_world(world))
            })
            .expect("Failed to run dynamic scene builder system.")
    }

    fn observe_trigger(app: &mut App, scene_id: InstanceId, scene_entity: Entity) {
        // Add observer
        app.world_mut().add_observer(
            move |trigger: Trigger<SceneInstanceReady>,
                  scene_spawner: Res<SceneSpawner>,
                  mut trigger_count: ResMut<TriggerCount>| {
                assert_eq!(
                    trigger.event().instance_id,
                    scene_id,
                    "`SceneInstanceReady` contains the wrong `InstanceId`"
                );
                assert_eq!(
                    trigger.target(),
                    scene_entity,
                    "`SceneInstanceReady` triggered on the wrong parent entity"
                );
                assert!(
                    scene_spawner.instance_is_ready(trigger.event().instance_id),
                    "`InstanceId` is not ready"
                );
                trigger_count.0 += 1;
            },
        );

        // Check observer is triggered once.
        app.update();
        app.world_mut()
            .run_system_once(|trigger_count: Res<TriggerCount>| {
                assert_eq!(
                    trigger_count.0, 1,
                    "wrong number of `SceneInstanceReady` triggers"
                );
            })
            .unwrap();
    }

    #[test]
    fn observe_scene() {
        let mut app = setup();

        // Build scene.
        let scene = build_scene(&mut app);

        // Spawn scene.
        let scene_id = app
            .world_mut()
            .run_system_once(move |mut scene_spawner: ResMut<'_, SceneSpawner>| {
                scene_spawner.spawn(scene.clone())
            })
            .unwrap();

        // Check trigger.
        observe_trigger(&mut app, scene_id, Entity::PLACEHOLDER);
    }

    #[test]
    fn observe_dynamic_scene() {
        let mut app = setup();

        // Build scene.
        let scene = build_dynamic_scene(&mut app);

        // Spawn scene.
        let scene_id = app
            .world_mut()
            .run_system_once(move |mut scene_spawner: ResMut<'_, SceneSpawner>| {
                scene_spawner.spawn_dynamic(scene.clone())
            })
            .unwrap();

        // Check trigger.
        observe_trigger(&mut app, scene_id, Entity::PLACEHOLDER);
    }

    #[test]
    fn observe_scene_as_child() {
        let mut app = setup();

        // Build scene.
        let scene = build_scene(&mut app);

        // Spawn scene as child.
        let (scene_id, scene_entity) = app
            .world_mut()
            .run_system_once(
                move |mut commands: Commands<'_, '_>,
                      mut scene_spawner: ResMut<'_, SceneSpawner>| {
                    let entity = commands.spawn_empty().id();
                    let id = scene_spawner.spawn_as_child(scene.clone(), entity);
                    (id, entity)
                },
            )
            .unwrap();

        // Check trigger.
        observe_trigger(&mut app, scene_id, scene_entity);
    }

    #[test]
    fn observe_dynamic_scene_as_child() {
        let mut app = setup();

        // Build scene.
        let scene = build_dynamic_scene(&mut app);

        // Spawn scene as child.
        let (scene_id, scene_entity) = app
            .world_mut()
            .run_system_once(
                move |mut commands: Commands<'_, '_>,
                      mut scene_spawner: ResMut<'_, SceneSpawner>| {
                    let entity = commands.spawn_empty().id();
                    let id = scene_spawner.spawn_dynamic_as_child(scene.clone(), entity);
                    (id, entity)
                },
            )
            .unwrap();

        // Check trigger.
        observe_trigger(&mut app, scene_id, scene_entity);
    }

    #[test]
    fn despawn_scene() {
        let mut app = App::new();
        app.add_plugins((AssetPlugin::default(), ScenePlugin));
        app.register_type::<ComponentF>();

        let asset_server = app.world().resource::<AssetServer>();

        // Build scene.
        let scene = asset_server.add(DynamicScene::default());
        let count = 10;

        // Checks the number of scene instances stored in `SceneSpawner`.
        let check = |world: &mut World, expected_count: usize| {
            let scene_spawner = world.resource::<SceneSpawner>();
            assert_eq!(
                scene_spawner.spawned_dynamic_scenes[&scene.id()].len(),
                expected_count
            );
            assert_eq!(scene_spawner.spawned_instances.len(), expected_count);
        };

        // Spawn scene.
        for _ in 0..count {
            app.world_mut()
                .spawn((ComponentF, DynamicSceneRoot(scene.clone())));
        }

        app.update();
        check(app.world_mut(), count);

        // Despawn scene.
        app.world_mut()
            .run_system_once(
                |mut commands: Commands, query: Query<Entity, With<ComponentF>>| {
                    for entity in query.iter() {
                        commands.entity(entity).despawn();
                    }
                },
            )
            .unwrap();

        app.update();
        check(app.world_mut(), 0);
    }

    #[test]
    fn child_instance_is_forgotten_with_its_parent() {
        let mut app = setup();
        let scene = build_scene(&mut app);

        let parent = app.world_mut().spawn_empty().id();
        let instance = app
            .world_mut()
            .resource_mut::<SceneSpawner>()
            .spawn_as_child(scene, parent);
        app.update();
        assert!(app
            .world()
            .resource::<SceneSpawner>()
            .instance_is_ready(instance));

        app.world_mut().entity_mut(parent).despawn();
        app.update();

        let scene_spawner = app.world().resource::<SceneSpawner>();
        assert!(!scene_spawner.instance_is_ready(instance));
        assert!(scene_spawner.instance_parents.is_empty());
    }

    #[test]
    fn unregistered_instance_is_not_spawned() {
        let mut app = setup();
        let scene = build_scene(&mut app);

        let parent = app.world_mut().spawn_empty().id();
        let mut scene_spawner = app.world_mut().resource_mut::<SceneSpawner>();
        let instance = scene_spawner.spawn_as_child(scene, parent);
        scene_spawner.unregister_instance(instance);
        app.update();

        assert!(!app
            .world()
            .resource::<SceneSpawner>()
            .instance_is_ready(instance));
        let mut components = app.world_mut().query::<&ComponentF>();
        assert_eq!(components.iter(app.world()).count(), 2);
    }

    #[test]
    fn scene_child_order_preserved_when_archetype_order_mismatched() {
        let mut app = App::new();

        app.add_plugins(ScheduleRunnerPlugin::default())
            .add_plugins(AssetPlugin::default())
            .add_plugins(ScenePlugin)
            .register_type::<ComponentA>()
            .register_type::<ComponentF>();
        app.update();

        let mut scene_world = World::new();
        let root = scene_world.spawn_empty().id();
        let temporary_root = scene_world.spawn_empty().id();
        // Spawn entities with different parent first before parenting them to the actual root, allowing us
        // to decouple child order from archetype-creation-order
        let child1 = scene_world
            .spawn((ChildOf(temporary_root), ComponentA { x: 1.0, y: 1.0 }))
            .id();
        let child2 = scene_world
            .spawn((ChildOf(temporary_root), ComponentA { x: 2.0, y: 2.0 }))
            .id();
        // the "first" child is intentionally spawned with a different component to force it into a "newer" archetype,
        // meaning it will be iterated later in the spawn code.
        let child0 = scene_world
            .spawn((ChildOf(temporary_root), ComponentF))
            .id();

        scene_world
            .entity_mut(root)
            .add_children(&[child0, child1, child2]);

        let scene = Scene::new(scene_world);
        let scene_handle = app.world_mut().resource_mut::<Assets<Scene>>().add(scene);

        let spawned = app.world_mut().spawn(SceneRoot(scene_handle.clone())).id();

        app.update();
        let world = app.world_mut();

        let spawned_root = world.entity(spawned).get::<Children>().unwrap()[0];
        let children = world.entity(spawned_root).get::<Children>().unwrap();
        assert_eq!(children.len(), 3);
        assert!(world.entity(children[0]).get::<ComponentF>().is_some());
        assert_eq!(
            world.entity(children[1]).get::<ComponentA>().unwrap().x,
            1.0
        );
        assert_eq!(
            world.entity(children[2]).get::<ComponentA>().unwrap().x,
            2.0
        );
    }
}
