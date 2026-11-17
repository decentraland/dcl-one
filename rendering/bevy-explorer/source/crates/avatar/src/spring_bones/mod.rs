//! Secondary motion for the extra bones of wearables: hair, tails, capes.
//!
//! [`chains`] decides which bones swing and how they group, and [`sim`] is the solver, which
//! nothing runs yet. An avatar's chains are found when it is built and kept on it as
//! [`SpringChains`], so a rebuilt avatar starts with fresh ones and a removed avatar takes its
//! own with it.

use bevy::{
    platform::collections::{HashMap, HashSet},
    prelude::*,
};
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

/// The spring chains of a built avatar, on the entity that holds its model. A chain that branches
/// from another comes after it, so solving them in order places every anchor before the chain
/// that hangs off it.
#[derive(Component, Debug, Default)]
pub struct SpringChains(pub Vec<SpringChain>);

#[derive(Debug)]
pub struct SpringChain {
    /// the bone the chain hangs off: a body bone, or the joint of an earlier chain that this one
    /// branches from
    pub anchor: Entity,
    /// from the anchor outwards, each the child of the one before it
    pub joints: Vec<SpringJoint>,
}

#[derive(Debug)]
pub struct SpringJoint {
    pub entity: Entity,
    pub params: SpringBoneParams,
    /// the bone's local rotation as the wearable was authored, which it returns to
    pub rest_rotation: Quat,
}

/// The chains of one wearable, from the joints of its skins as loaded, each joint once.
/// `body_bone` gives the body's bone for a wearable joint that names one. A chain off a bone the
/// body lacks is left out, with the chains that branch from it.
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

    // the joints of the chains kept so far, which a later chain may branch from
    let mut kept = HashSet::new();
    let mut chains = Vec::new();
    for chain in plan_chains(&skin, params) {
        let anchor = if skin[chain.anchor].avatar_bone {
            body_bone(joints[chain.anchor])
        } else {
            kept.get(&joints[chain.anchor]).copied()
        };
        let Some(anchor) = anchor else {
            continue;
        };
        if chain.dropped > 0 {
            warn!(
                "spring chain from {:?} is too long: {} bones past its first {} stay at rest",
                skin[chain.joints[0].0].name,
                chain.dropped,
                chain.joints.len()
            );
        }
        kept.extend(chain.joints.iter().map(|(ix, _)| joints[*ix]));
        chains.push(SpringChain {
            anchor,
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
        });
    }
    chains
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
    fn a_branch_hangs_off_the_joint_it_forks_from() {
        let mut world = World::new();
        let body_head = world.spawn(Name::new("Avatar_Head")).id();
        let armature = world.spawn(Name::new("Armature")).id();
        let wearable_head = world
            .spawn((Name::new("Avatar_Head"), ChildOf(armature)))
            .id();
        let hair_01 = world
            .spawn((Name::new("Hair_01"), ChildOf(wearable_head)))
            .id();
        let hair_02 = world.spawn((Name::new("Hair_02"), ChildOf(hair_01))).id();
        let hair_fork = world.spawn((Name::new("Hair_fork"), ChildOf(hair_01))).id();
        world.insert_resource(Skin {
            joints: vec![hair_fork, wearable_head, hair_02, hair_01],
            body_head,
            wearable_head,
        });
        world.init_resource::<Planned>();

        world.run_system_once(plan).unwrap();

        let planned = &world.resource::<Planned>().0;
        let chains: Vec<(Entity, Vec<Entity>)> = planned
            .iter()
            .map(|chain| {
                (
                    chain.anchor,
                    chain.joints.iter().map(|j| j.entity).collect(),
                )
            })
            .collect();
        assert_eq!(
            chains,
            [
                (body_head, vec![hair_01, hair_02]),
                (hair_01, vec![hair_fork])
            ]
        );
    }

    #[test]
    fn a_chain_off_a_bone_the_body_lacks_is_left_out_with_its_branches() {
        let mut world = World::new();
        let armature = world.spawn(Name::new("Armature")).id();
        let wearable_tail = world
            .spawn((Name::new("Avatar_Tail"), ChildOf(armature)))
            .id();
        let tail_01 = world
            .spawn((Name::new("Tail_01"), ChildOf(wearable_tail)))
            .id();
        let tail_02 = world.spawn((Name::new("Tail_02"), ChildOf(tail_01))).id();
        let tail_fork = world.spawn((Name::new("Tail_fork"), ChildOf(tail_01))).id();
        world.insert_resource(Skin {
            joints: vec![wearable_tail, tail_01, tail_02, tail_fork],
            body_head: armature,
            wearable_head: armature,
        });
        world.init_resource::<Planned>();

        world.run_system_once(plan).unwrap();

        assert!(world.resource::<Planned>().0.is_empty());
    }
}
