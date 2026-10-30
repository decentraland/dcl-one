// The 17 scenes of explorer-automation's Visual suite, in `scenes/dcl-workspace.json` order. Scene
// number k sits at base parcel `12 * k, 0`, which is the parcel its own `scene.json` names.
//
// `settleMs` is the wait between the scene's entities being live and the screenshot. The Unity
// harness waited 5 s after the scene reported ready, and a fixture then slept its own time (named
// per row) for what has nothing to observe: animation timelines, glyph atlases, wearable fetches,
// the trigger scene's retry window. The sum is kept.
//
// `maxDifferentRatio` is the share of pixels that may differ by more than 8 in a channel: 0.3%,
// and 1.5% for the avatar scene, which the Unity suite deliberately gave more room.

export interface VisualScene {
  /** The folder under `scenes/` and the name of the reference, `references/<id>.png`. */
  id: string
  /** The scene's base parcel, where the player is teleported. */
  base: readonly [number, number]
  settleMs: number
  maxDifferentRatio: number
}

export const SCENES: readonly VisualScene[] = [
  { id: 'animations', base: [0, 0], settleMs: 7500, maxDifferentRatio: 0.003 }, // 5 s + 2.5 s
  { id: 'avatar-shape', base: [12, 0], settleMs: 15000, maxDifferentRatio: 0.015 }, // 5 s + 10 s
  { id: 'core', base: [24, 0], settleMs: 5000, maxDifferentRatio: 0.003 },
  { id: 'gltf', base: [36, 0], settleMs: 5000, maxDifferentRatio: 0.003 },
  { id: 'gltf-legacy-assets', base: [48, 0], settleMs: 5000, maxDifferentRatio: 0.003 },
  { id: 'gltf-legacy-assets-2', base: [60, 0], settleMs: 5000, maxDifferentRatio: 0.003 },
  { id: 'gltf-materials', base: [72, 0], settleMs: 5000, maxDifferentRatio: 0.003 },
  { id: 'gltf-meshes', base: [84, 0], settleMs: 5000, maxDifferentRatio: 0.003 },
  { id: 'gltf-transforms', base: [96, 0], settleMs: 5000, maxDifferentRatio: 0.003 },
  { id: 'gltf-uvs', base: [108, 0], settleMs: 5000, maxDifferentRatio: 0.003 },
  { id: 'lights', base: [120, 0], settleMs: 5000, maxDifferentRatio: 0.003 },
  { id: 'materials', base: [132, 0], settleMs: 5000, maxDifferentRatio: 0.003 },
  { id: 'swap_material', base: [144, 0], settleMs: 6000, maxDifferentRatio: 0.003 }, // 5 s + 1 s
  { id: 'text-shape', base: [156, 0], settleMs: 7000, maxDifferentRatio: 0.003 }, // 5 s + 2 s
  { id: 'triggers-raycasts', base: [168, 0], settleMs: 12000, maxDifferentRatio: 0.003 }, // 5 s + 7 s
  { id: 'tweens', base: [180, 0], settleMs: 6000, maxDifferentRatio: 0.003 }, // 5 s + 1 s
  { id: 'ui', base: [192, 0], settleMs: 5000, maxDifferentRatio: 0.003 }
]
