//! Secondary motion for the extra bones of wearables: hair, tails, capes.
//!
//! [`chains`] decides which bones swing and how they group, and [`sim`] is the solver, which
//! nothing runs yet. An avatar's
//! chains are found when it is built and kept on it as [`SpringChains`], so a rebuilt avatar
//! starts with fresh ones and a removed avatar takes its own with it.

use bevy::{platform::collections::HashMap, prelude::*};
use collectibles::spring_bones::{SpringBoneModel, SpringBoneParams};

pub mod chains;
mod diagnostics;
pub mod sim;

use chains::{plan_chains, SkinJoint};

/// How far up a wearable's own bones a branch is followed to the body bone it hangs off.
pub const MAX_BRANCH_DEPTH: usize = 64;

pub struct SpringBonesPlugin;

impl Plugin for SpringBonesPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(diagnostics::SpringBonesDiagnosticsPlugin);
    }
}

/// The spring chains of a built avatar, on the entity that holds its model.
#[derive(Component, Debug, Default)]
pub struct SpringChains(pub Vec<SpringChain>);

#[derive(Debug)]
pub struct SpringChain {
    /// the body bone the chain hangs off
    pub anchor: Entity,
    /// from the anchor outwards
    pub joints: Vec<SpringJoint>,
}

#[derive(Debug)]
pub struct SpringJoint {
    pub entity: Entity,
    pub params: SpringBoneParams,
    /// the bone's local rotation as the wearable was authored, which it returns to
    pub rest_rotation: Quat,
}

/// The chains of one wearable, from its skin's joints as loaded. `body_bone` gives the body's
/// bone for a wearable joint that names one.
pub fn wearable_chains(
    joints: &[Entity],
    nodes: &Query<(&Name, &ChildOf)>,
    transforms: &Query<&Transform>,
    body_bone: impl Fn(Entity) -> Option<Entity>,
    params: Option<&SpringBoneModel>,
) -> Vec<SpringChain> {
    let mut places = HashMap::new();
    for (ix, joint) in joints.iter().enumerate() {
        places.entry(*joint).or_insert(ix);
    }

    let skin: Vec<SkinJoint> = joints
        .iter()
        .map(|joint| match nodes.get(*joint) {
            Ok((name, parent)) => SkinJoint {
                name: name.as_str(),
                parent: places.get(&parent.parent()).copied(),
                avatar_bone: name.to_lowercase().starts_with("avatar_"),
            },
            // a joint with no name or parent is no bone of the avatar and hangs off nothing
            Err(_) => SkinJoint {
                name: "",
                parent: None,
                avatar_bone: false,
            },
        })
        .collect();

    plan_chains(&skin, params)
        .into_iter()
        .filter_map(|chain| {
            if chain.dropped > 0 {
                warn!(
                    "spring chain from {:?} is too long: its last {} bones stay at rest",
                    skin[chain.joints[0].0].name, chain.dropped
                );
            }
            Some(SpringChain {
                anchor: body_bone(joints[chain.anchor])?,
                joints: chain
                    .joints
                    .into_iter()
                    .map(|(ix, params)| SpringJoint {
                        entity: joints[ix],
                        params,
                        rest_rotation: transforms
                            .get(joints[ix])
                            .map_or(Quat::IDENTITY, |transform| transform.rotation),
                    })
                    .collect(),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;

    #[derive(Resource)]
    struct Skin {
        joints: Vec<Entity>,
        body_head: Entity,
        wearable_head: Entity,
    }

    #[derive(Resource, Default)]
    struct Planned(Vec<SpringChain>);

    fn plan(
        skin: Res<Skin>,
        nodes: Query<(&Name, &ChildOf)>,
        transforms: Query<&Transform>,
        mut planned: ResMut<Planned>,
    ) {
        planned.0 = wearable_chains(
            &skin.joints,
            &nodes,
            &transforms,
            |joint| (joint == skin.wearable_head).then_some(skin.body_head),
            None,
        );
    }

    #[test]
    fn chains_hang_off_the_body_bone_and_remember_each_rest_rotation() {
        let mut world = World::new();
        let body_head = world.spawn(Name::new("Avatar_Head")).id();
        let armature = world.spawn(Name::new("Armature")).id();
        let wearable_head = world
            .spawn((Name::new("Avatar_Head"), ChildOf(armature)))
            .id();
        let rest = Quat::from_rotation_x(0.3);
        let hair_01 = world
            .spawn((
                Name::new("Hair_01"),
                ChildOf(wearable_head),
                Transform::from_rotation(rest),
            ))
            .id();
        let hair_02 = world
            .spawn((Name::new("Hair_02"), ChildOf(hair_01), Transform::IDENTITY))
            .id();
        world.insert_resource(Skin {
            joints: vec![wearable_head, hair_01, hair_02],
            body_head,
            wearable_head,
        });
        world.init_resource::<Planned>();

        world.run_system_once(plan).unwrap();

        let planned = &world.resource::<Planned>().0;
        assert_eq!(planned.len(), 1);
        assert_eq!(planned[0].anchor, body_head);
        let entities: Vec<_> = planned[0].joints.iter().map(|j| j.entity).collect();
        assert_eq!(entities, [hair_01, hair_02]);
        assert_eq!(planned[0].joints[0].rest_rotation, rest);
        assert_eq!(planned[0].joints[0].params, chains::DEFAULT_PARAMS);
    }

    #[test]
    fn a_chain_off_a_bone_the_body_lacks_is_left_out() {
        let mut world = World::new();
        let armature = world.spawn(Name::new("Armature")).id();
        let wearable_tail = world
            .spawn((Name::new("Avatar_Tail"), ChildOf(armature)))
            .id();
        let tail_01 = world
            .spawn((Name::new("Tail_01"), ChildOf(wearable_tail)))
            .id();
        world.insert_resource(Skin {
            joints: vec![wearable_tail, tail_01],
            body_head: armature,
            wearable_head: armature,
        });
        world.init_resource::<Planned>();

        world.run_system_once(plan).unwrap();

        assert!(world.resource::<Planned>().0.is_empty());
    }
}
