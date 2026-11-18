//! Which of a wearable's joints are spring bones, and how they group into chains.
//!
//! A wearable's skin lists the avatar skeleton's bones and any bones of its own. The avatar
//! bones follow the body. Each bone of its own is a spring bone, whether or not the wearable
//! declares parameters for it. One hanging directly off an avatar bone starts a chain, anchored
//! to that bone, and the chain runs down from parent to child: the solver places each joint
//! under the one before it, so a chain never holds two bones that are not parent and child.
//! Where a bone has several spring bones under it, the first by name carries the chain on and
//! each of the others starts a chain of its own, anchored to that bone. The order the skin lists
//! its joints in changes nothing.

use collectibles::spring_bones::{SpringBoneModel, SpringBoneParams};

use std::collections::VecDeque;

use super::sim::MAX_CHAIN_JOINTS;

/// The parameters of a bone when neither it nor any bone above it declares some.
pub const DEFAULT_PARAMS: SpringBoneParams = SpringBoneParams {
    stiffness: 0.5,
    drag: 0.4,
    gravity_dir: bevy::math::Vec3::ZERO,
    gravity_power: 0.0,
};

/// One joint of a wearable's skin, in the skin's order.
#[derive(Debug, Clone, Copy)]
pub struct SkinJoint<'a> {
    /// the node's name as authored: parameters match it exactly
    pub name: &'a str,
    /// the parent node's place in the same list, when it is a joint too
    pub parent: Option<usize>,
    /// whether this is a bone of the avatar skeleton
    pub avatar_bone: bool,
}

/// A chain as places in the joint list it was planned from.
#[derive(Debug, Clone, PartialEq)]
pub struct PlannedChain {
    /// the bone the chain hangs off: an avatar bone, or the joint of an earlier chain that this
    /// one branches from
    pub anchor: usize,
    /// from the anchor outwards, each the child of the one before it
    pub joints: Vec<(usize, SpringBoneParams)>,
    /// bones past the chain's limit, left at rest with every bone under them
    pub dropped: usize,
}

/// Groups a skin's spring bones into chains. A chain that branches from another comes after it.
/// A bone that no line of spring bones joins to an avatar bone is in no chain.
pub fn plan_chains(joints: &[SkinJoint], params: Option<&SpringBoneModel>) -> Vec<PlannedChain> {
    // the spring bones directly under each joint, by name
    let mut below: Vec<Vec<usize>> = vec![Vec::new(); joints.len()];
    for (ix, joint) in joints.iter().enumerate() {
        if joint.avatar_bone {
            continue;
        }
        if let Some(parent) = joint.parent.filter(|parent| *parent < joints.len()) {
            below[parent].push(ix);
        }
    }
    for bones in &mut below {
        bones.sort_by_key(|ix| joints[*ix].name);
    }

    // the chains still to build, as the bone each hangs off and its first joint
    let mut starts: Vec<(usize, usize)> = (0..joints.len())
        .filter(|anchor| joints[*anchor].avatar_bone)
        .flat_map(|anchor| below[anchor].iter().map(move |first| (anchor, *first)))
        .collect();
    starts.sort_by_key(|(anchor, first)| (joints[*anchor].name, joints[*first].name));
    let mut starts = VecDeque::from(starts);

    // a bone reached from an avatar bone has one line of parents, so each is visited once
    let mut chains = Vec::new();
    while let Some((anchor, first)) = starts.pop_front() {
        let mut chain = PlannedChain {
            anchor,
            joints: Vec::new(),
            dropped: 0,
        };
        let mut next = Some(first);
        while let Some(ix) = next {
            if chain.joints.len() == MAX_CHAIN_JOINTS {
                chain.dropped = bones_from(&below, ix);
                break;
            }
            chain.joints.push((ix, resolve_params(joints, ix, params)));
            let mut under = below[ix].iter().copied();
            next = under.next();
            starts.extend(under.map(|branch| (ix, branch)));
        }
        chains.push(chain);
    }

    chains
}

/// How many bones `top` and everything under it come to.
fn bones_from(below: &[Vec<usize>], top: usize) -> usize {
    let mut count = 0;
    let mut pending = vec![top];
    while let Some(ix) = pending.pop() {
        count += 1;
        pending.extend(&below[ix]);
    }
    count
}

/// The bone's own entry, else that of the nearest bone above it that has one, avatar bones
/// included. An entry is taken whole: what it leaves out is zero, not the default.
fn resolve_params(
    joints: &[SkinJoint],
    ix: usize,
    params: Option<&SpringBoneModel>,
) -> SpringBoneParams {
    let Some(params) = params else {
        return DEFAULT_PARAMS;
    };

    let mut next = Some(ix);
    // a parent list is a tree, but it is the wearable's data: never walk it more than once round
    for _ in 0..joints.len() {
        let Some(joint) = next.and_then(|ix| joints.get(ix)) else {
            break;
        };
        if let Some(found) = params.get(joint.name) {
            return *found;
        }
        next = joint.parent;
    }

    DEFAULT_PARAMS
}

#[cfg(test)]
mod tests {
    use super::*;

    fn avatar(name: &str, parent: Option<usize>) -> SkinJoint<'_> {
        SkinJoint {
            name,
            parent,
            avatar_bone: true,
        }
    }

    fn extra(name: &str, parent: Option<usize>) -> SkinJoint<'_> {
        SkinJoint {
            name,
            parent,
            avatar_bone: false,
        }
    }

    fn params(entries: &[(&str, SpringBoneParams)]) -> SpringBoneModel {
        entries
            .iter()
            .map(|(name, params)| (name.to_string(), *params))
            .collect()
    }

    fn stiff(stiffness: f32) -> SpringBoneParams {
        SpringBoneParams {
            stiffness,
            ..Default::default()
        }
    }

    fn indices(chain: &PlannedChain) -> Vec<usize> {
        chain.joints.iter().map(|(ix, _)| *ix).collect()
    }

    #[test]
    fn extra_bones_are_spring_bones_without_any_declared_parameters() {
        let joints = [
            avatar("Avatar_Hips", None),
            avatar("Avatar_Head", Some(0)),
            extra("Hair_01", Some(1)),
            extra("Hair_02", Some(2)),
        ];

        let chains = plan_chains(&joints, None);

        assert_eq!(chains.len(), 1);
        assert_eq!(chains[0].anchor, 1);
        assert_eq!(
            chains[0].joints,
            vec![(2, DEFAULT_PARAMS), (3, DEFAULT_PARAMS)]
        );
    }

    #[test]
    fn a_skin_of_avatar_bones_only_has_no_chains() {
        let joints = [avatar("Avatar_Hips", None), avatar("Avatar_Spine", Some(0))];
        assert!(plan_chains(&joints, Some(&params(&[("Avatar_Spine", stiff(1.0))]))).is_empty());
    }

    #[test]
    fn each_bone_under_an_avatar_bone_starts_its_own_chain() {
        let joints = [
            avatar("Avatar_Head", None),
            extra("Left_01", Some(0)),
            extra("Left_02", Some(1)),
            extra("Right_01", Some(0)),
            extra("Right_02", Some(3)),
        ];

        let chains = plan_chains(&joints, None);

        assert_eq!(chains.len(), 2);
        assert_eq!(indices(&chains[0]), [1, 2]);
        assert_eq!(indices(&chains[1]), [3, 4]);
    }

    #[test]
    fn a_branch_starts_its_own_chain_off_the_bone_it_forks_from() {
        // Tail_02a and Tail_02b both hang off Tail_01: one carries the chain on, the other is
        // solved under Tail_01 too, never under its sibling
        let joints = [
            avatar("Avatar_Hips", None),
            extra("Tail_01", Some(0)),
            extra("Tail_02b", Some(1)),
            extra("Tail_02a", Some(1)),
            extra("Tail_03b", Some(2)),
        ];

        let chains = plan_chains(&joints, None);

        assert_eq!(chains.len(), 2);
        assert_eq!((chains[0].anchor, indices(&chains[0])), (0, vec![1, 3]));
        assert_eq!((chains[1].anchor, indices(&chains[1])), (1, vec![2, 4]));
    }

    #[test]
    fn every_joint_of_a_chain_is_the_child_of_the_one_before_it() {
        // two chains listed a joint at a time, with a fork and a bone under an unlisted node
        let joints = [
            avatar("Avatar_Head", None),
            extra("Left_01", Some(0)),
            extra("Right_01", Some(0)),
            extra("Left_02", Some(1)),
            extra("Right_02", Some(2)),
            extra("Right_02_fork", Some(2)),
            extra("Loose", None),
            extra("Left_03", Some(3)),
        ];

        let chains = plan_chains(&joints, None);

        assert_eq!(chains.len(), 3);
        assert_eq!(indices(&chains[0]), [1, 3, 7]);
        assert_eq!(indices(&chains[1]), [2, 4]);
        assert_eq!(indices(&chains[2]), [5]);
        for chain in &chains {
            let mut parent = chain.anchor;
            for (ix, _) in &chain.joints {
                assert_eq!(joints[*ix].parent, Some(parent));
                parent = *ix;
            }
        }
    }

    #[test]
    fn a_bone_under_an_unlisted_node_is_left_out_with_the_bones_under_it() {
        let joints = [
            avatar("Avatar_Hips", None),
            extra("Cape_01", Some(0)),
            extra("Loose", None),
            extra("Loose_child", Some(2)),
        ];

        let chains = plan_chains(&joints, None);

        assert_eq!(chains.len(), 1);
        assert_eq!(indices(&chains[0]), [1]);
    }

    #[test]
    fn the_order_the_skin_lists_its_joints_in_changes_nothing() {
        let joints = [
            avatar("Avatar_Hips", None),
            extra("Tail_02", Some(2)),
            extra("Tail_01", Some(0)),
        ];

        let chains = plan_chains(&joints, None);

        assert_eq!(chains.len(), 1);
        assert_eq!(indices(&chains[0]), [2, 1]);
    }

    #[test]
    fn bones_that_only_lead_to_each_other_are_in_no_chain() {
        let joints = [
            avatar("Avatar_Hips", None),
            extra("Ring_a", Some(2)),
            extra("Ring_b", Some(1)),
            extra("Self", Some(3)),
        ];

        assert!(plan_chains(&joints, None).is_empty());
    }

    #[test]
    fn a_chain_keeps_its_first_joints_up_to_the_limit() {
        // 15 bones in a line, with a fork inside the limit and one past it
        let names: Vec<String> = (0..15).map(|ix| format!("Tail_{ix:02}")).collect();
        let mut joints = vec![avatar("Avatar_Hips", None)];
        joints.extend(
            names
                .iter()
                .enumerate()
                .map(|(ix, name)| extra(name, Some(ix))),
        );
        joints.push(extra("Wing", Some(5)));
        joints.push(extra("Wing_far", Some(14)));

        let chains = plan_chains(&joints, None);

        assert_eq!(chains.len(), 2);
        assert_eq!(
            indices(&chains[0]),
            (1..=MAX_CHAIN_JOINTS).collect::<Vec<_>>()
        );
        // the three bones past the limit and the fork under one of them
        assert_eq!(chains[0].dropped, 4);
        assert_eq!((chains[1].anchor, indices(&chains[1])), (5, vec![16]));
    }

    #[test]
    fn parameters_come_from_the_bone_then_the_nearest_bone_above_it() {
        let joints = [
            avatar("Avatar_Head", None),
            extra("Hair_01", Some(0)),
            extra("Hair_02", Some(1)),
            extra("Hair_03", Some(2)),
        ];
        let params = params(&[("Hair_01", stiff(0.1)), ("Hair_03", stiff(0.3))]);

        let chains = plan_chains(&joints, Some(&params));

        assert_eq!(
            chains[0].joints,
            vec![(1, stiff(0.1)), (2, stiff(0.1)), (3, stiff(0.3))]
        );
    }

    #[test]
    fn parameters_declared_on_an_avatar_bone_reach_the_bones_under_it() {
        let joints = [
            avatar("Avatar_Hips", None),
            avatar("Avatar_Head", Some(0)),
            extra("Hair_01", Some(1)),
            extra("Tail_01", Some(0)),
        ];
        let params = params(&[("Avatar_Head", stiff(0.9))]);

        let chains = plan_chains(&joints, Some(&params));

        assert_eq!(chains[0].joints, vec![(2, stiff(0.9))]);
        assert_eq!(chains[1].joints, vec![(3, DEFAULT_PARAMS)]);
    }

    #[test]
    fn a_declared_entry_is_taken_whole_and_names_match_exactly() {
        let joints = [
            avatar("Avatar_Head", None),
            extra("Hair_01", Some(0)),
            extra("hair_02", Some(0)),
        ];
        let params = params(&[("Hair_01", stiff(1.0)), ("Hair_02", stiff(1.0))]);

        let chains = plan_chains(&joints, Some(&params));

        // drag is zero, not the default: the entry left it out
        assert_eq!(chains[0].joints, vec![(1, stiff(1.0))]);
        // a different case is a different name
        assert_eq!(chains[1].joints, vec![(2, DEFAULT_PARAMS)]);
    }
}
