use super::*;

#[test]
fn despawning_an_entity_keeps_the_collider_another_entity_has_taken_over() {
    let mut world = World::new();
    world.add_observer(on_collider_removed::<CtCollider>);
    let root = world.spawn(SceneColliderData::default()).id();
    let container = ContainerEntity {
        container: root,
        root,
        container_id: SceneEntityId::ROOT,
    };
    // a model's replacement registers its colliders under the same ids while the old one is
    // still shown
    let id = ColliderId::new(SceneEntityId::ROOT, Some("mesh".to_owned()), 0);
    let has_collider = || HasCollider::<CtCollider>(id.clone(), Default::default());
    let old = world.spawn((container, has_collider())).id();
    let new = world.spawn((container, has_collider())).id();
    let mut colliders = world.get_mut::<SceneColliderData>(root).unwrap();
    colliders.set_collider(&id, ColliderBuilder::ball(0.5).build(), Some(old));
    colliders.set_collider(&id, ColliderBuilder::ball(0.5).build(), Some(new));

    world.entity_mut(old).despawn();

    let colliders = world.get::<SceneColliderData>(root).unwrap();
    assert_eq!(colliders.get_collider_entity(&id), Some(new));
}
