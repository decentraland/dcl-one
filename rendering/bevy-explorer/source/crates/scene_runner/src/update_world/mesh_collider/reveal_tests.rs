use super::*;

fn container() -> Entity {
    Entity::from_raw(1)
}

fn node() -> Entity {
    Entity::from_raw(2)
}

/// Where a node of `container()` points: the glTF container it belongs to.
fn of_container() -> ContainerEntity {
    ContainerEntity {
        container: container(),
        root: container(),
        container_id: SceneEntityId::ROOT,
    }
}

#[test]
fn a_new_collider_of_a_model_in_progress_waits_for_its_reveal() {
    assert!(waits_for_reveal(node(), &of_container(), false, true));
}

#[test]
fn a_change_to_a_registered_collider_applies_at_once() {
    // the old model's nodes and their colliders stay while the new model is processed
    assert!(!waits_for_reveal(node(), &of_container(), true, true));
}

#[test]
fn the_containers_own_collider_never_waits() {
    // a primitive MeshCollider on the container entity is not part of the hidden model
    assert!(!waits_for_reveal(container(), &of_container(), false, true));
}

#[test]
fn nothing_waits_once_the_model_is_done() {
    assert!(!waits_for_reveal(node(), &of_container(), false, false));
}
