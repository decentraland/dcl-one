import * as THREE from "three";
import { expect, test } from "vitest";
import { shareSkeleton } from "./skeleton";

/** An armature of one bone and a mesh skinned to it, as the loader builds it: `name` unique, the file's in `userData`. */
function armature(boneName: string, fileName = boneName) {
  const root = new THREE.Group();
  const bone = new THREE.Bone();
  bone.name = boneName;
  bone.userData.name = fileName;
  const mesh = new THREE.SkinnedMesh();
  root.add(bone, mesh);
  mesh.bind(new THREE.Skeleton([bone]));
  return { root, bone, mesh };
}

test("a mesh skinned to a repeated armature follows the first bone of each name", () => {
  const model = new THREE.Group();
  const lead = armature("Avatar_LeftArm");
  const repeat = armature("Avatar_LeftArm_1", "Avatar_LeftArm");
  model.add(lead.root, repeat.root);
  const inverses = repeat.mesh.skeleton.boneInverses;
  const leadSkeleton = lead.mesh.skeleton;

  shareSkeleton(model);

  expect(repeat.mesh.skeleton.bones).toHaveLength(1);
  expect(repeat.mesh.skeleton.bones[0]).toBe(lead.bone);
  expect(repeat.mesh.skeleton.boneInverses).toBe(inverses);
  expect(lead.mesh.skeleton).toBe(leadSkeleton);
});

test("a bone no other armature names keeps its own", () => {
  const model = new THREE.Group();
  const lead = armature("Avatar_LeftArm");
  const other = armature("Tail_01");
  model.add(lead.root, other.root);
  const skeleton = other.mesh.skeleton;

  shareSkeleton(model);

  expect(other.mesh.skeleton).toBe(skeleton);
});
