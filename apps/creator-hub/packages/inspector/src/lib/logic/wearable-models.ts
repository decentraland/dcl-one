import { parseGltfJson } from '../utils/gltf';
import type { WearableCategory } from './wearables';
import { isSceneWearablePath } from './wearables';

type GltfNode = { name?: unknown };

export type RiggedModel = {
  /** The slot the model's mesh names suggest, when they name one. */
  suggestedCategory?: WearableCategory;
};

// Wearable meshes are usually named after their slot: `Jetpack_uBody`, `M_Feet_Sneakers`.
const MESH_NAME_SLOTS: [RegExp, WearableCategory][] = [
  [/u_?body|upper_?body|torso|shirt|jacket/, 'upper_body'],
  [/l_?body|lower_?body|pants|trousers|legs/, 'lower_body'],
  [/feet|shoes?|boots?/, 'feet'],
  [/facial_?hair|beard|mustache/, 'facial_hair'],
  [/hair/, 'hair'],
  [/helmet/, 'helmet'],
  [/top_?head/, 'top_head'],
  [/hat|cap/, 'hat'],
  [/mask/, 'mask'],
  [/eyewear|glasses/, 'eyewear'],
  [/earring/, 'earring'],
  [/tiara|crown/, 'tiara'],
  [/hands?_?wear|gloves?/, 'hands_wear'],
];

export function suggestCategory(names: readonly string[]): WearableCategory | undefined {
  for (const name of names) {
    const lower = name.toLowerCase();
    for (const [pattern, category] of MESH_NAME_SLOTS) {
      if (pattern.test(lower)) return category;
    }
  }
  return undefined;
}

/**
 * The model when it is made to be worn: a skinned mesh bound to the avatar armature (its bones
 * are named `Avatar_…`), with no animation. An emote has the same bones and an animation.
 * Undefined for any other model.
 */
export function inspectRiggedModel(bytes: Uint8Array): RiggedModel | undefined {
  const gltf = parseGltfJson(bytes);
  if (!gltf) return undefined;
  const skins = gltf.skins as unknown[] | undefined;
  const animations = gltf.animations as unknown[] | undefined;
  const nodes = (gltf.nodes as GltfNode[] | undefined) ?? [];
  if (!skins?.length || animations?.length) return undefined;
  if (!nodes.some($ => typeof $.name === 'string' && $.name.startsWith('Avatar_'))) {
    return undefined;
  }
  const meshes = (gltf.meshes as GltfNode[] | undefined) ?? [];
  const meshNames = meshes
    .map($ => $.name)
    .filter((name): name is string => typeof name === 'string');
  return { suggestedCategory: suggestCategory(meshNames) };
}

/** Model files worth reading: GLBs that the name doesn't already mark as a wearable or an emote. */
export function isWearableCandidate(path: string): boolean {
  const lower = path.toLowerCase();
  if (!lower.endsWith('.glb')) return false;
  if (lower.endsWith('_emote.glb') || isSceneWearablePath(path)) return false;
  // Asset-pack items are scene props, and a scene can hold hundreds of them.
  return !/(^|\/)asset-packs\//.test(lower);
}
