//! Which of a wearable's joints are spring bones, and how they group into chains.
//!
//! A wearable's skin lists the avatar skeleton's bones and any bones of its own. The avatar
//! bones follow the body. Each bone of its own is a spring bone, whether or not the wearable
//! declares parameters for it. One hanging directly off an avatar bone starts a chain, anchored
//! to that bone; the spring bones listed after it, up to the next such bone, are the rest of the
//! chain, in the order the skin lists them.

use collectibles::spring_bones::{SpringBoneModel, SpringBoneParams};

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
    /// the avatar bone the chain hangs off
    pub anchor: usize,
    pub joints: Vec<(usize, SpringBoneParams)>,
    /// joints past the chain's limit, left at rest
    pub dropped: usize,
}

/// Groups a skin's spring bones into chains.
pub fn plan_chains(joints: &[SkinJoint], params: Option<&SpringBoneModel>) -> Vec<PlannedChain> {
    let mut chains: Vec<PlannedChain> = Vec::new();

    for (ix, joint) in joints.iter().enumerate() {
        if joint.avatar_bone {
            continue;
        }
        // a bone under a node the skin does not list has no pose to hang from
        let Some(parent) = joint.parent.filter(|parent| *parent < joints.len()) else {
            continue;
        };

        let resolved = resolve_params(joints, ix, params);

        if joints[parent].avatar_bone {
            chains.push(PlannedChain {
                anchor: parent,
                joints: vec![(ix, resolved)],
                dropped: 0,
            });
        } else if let Some(chain) = chains.last_mut() {
            if chain.joints.len() < MAX_CHAIN_JOINTS {
                chain.joints.push((ix, resolved));
            } else {
                chain.dropped += 1;
            }
        }
        // else: listed before any chain started, so there is none to join
    }

    chains
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
    fn a_branch_joins_the_chain_listed_before_it() {
        // Tail_02a and Tail_02b both hang off Tail_01: the chain takes them in list order
        let joints = [
            avatar("Avatar_Hips", None),
            extra("Tail_01", Some(0)),
            extra("Tail_02a", Some(1)),
            extra("Tail_02b", Some(1)),
        ];

        let chains = plan_chains(&joints, None);

        assert_eq!(chains.len(), 1);
        assert_eq!(indices(&chains[0]), [1, 2, 3]);
    }

    #[test]
    fn a_bone_under_an_unlisted_node_is_left_out_but_its_children_are_not() {
        let joints = [
            avatar("Avatar_Hips", None),
            extra("Cape_01", Some(0)),
            extra("Loose", None),
            extra("Loose_child", Some(2)),
        ];

        let chains = plan_chains(&joints, None);

        assert_eq!(chains.len(), 1);
        assert_eq!(indices(&chains[0]), [1, 3]);
    }

    #[test]
    fn a_bone_listed_before_any_chain_starts_is_left_out() {
        let joints = [
            avatar("Avatar_Hips", None),
            extra("Tail_02", Some(2)),
            extra("Tail_01", Some(0)),
        ];

        let chains = plan_chains(&joints, None);

        assert_eq!(chains.len(), 1);
        assert_eq!(indices(&chains[0]), [2]);
    }

    #[test]
    fn a_chain_keeps_its_first_joints_up_to_the_limit() {
        let names: Vec<String> = (0..15).map(|ix| format!("Tail_{ix}")).collect();
        let mut joints = vec![avatar("Avatar_Hips", None)];
        joints.extend(
            names
                .iter()
                .enumerate()
                .map(|(ix, name)| extra(name, Some(ix))),
        );

        let chains = plan_chains(&joints, None);

        assert_eq!(chains.len(), 1);
        assert_eq!(chains[0].joints.len(), MAX_CHAIN_JOINTS);
        assert_eq!(chains[0].dropped, 3);
        assert_eq!(chains[0].joints[0].0, 1);
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
