import * as THREE from "three";

/** The name the file gives a node: the loader makes names unique, and keeps the file's in `userData.name`. */
const fileName = (node: THREE.Object3D): string => {
  const given = node.userData?.name;
  return typeof given === "string" && given ? given : node.name;
};

/**
 * A model can carry several armatures whose bones share names. The loader renames the repeats, and a clip drives a
 * bone by its name, so of each name only the first under the model moves and a mesh skinned to another armature stays
 * in its bind pose. This binds every skinned mesh to those first bones, keeping the mesh's own inverse bind matrices.
 */
export function shareSkeleton(root: THREE.Object3D): void {
  const first = new Map<string, THREE.Bone>();
  const meshes: THREE.SkinnedMesh[] = [];
  root.traverse((node) => {
    const name = fileName(node);
    if ((node as THREE.Bone).isBone && name && !first.has(name)) first.set(name, node as THREE.Bone);
    if ((node as THREE.SkinnedMesh).isSkinnedMesh) meshes.push(node as THREE.SkinnedMesh);
  });
  for (const mesh of meshes) {
    const own = mesh.skeleton.bones;
    const bones = own.map((bone) => first.get(fileName(bone)) ?? bone);
    if (bones.some((bone, i) => bone !== own[i])) mesh.bind(new THREE.Skeleton(bones, mesh.skeleton.boneInverses), mesh.bindMatrix);
  }
}
