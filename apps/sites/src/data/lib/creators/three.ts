// Sites type-checks `three` against the reduced declarations the avatar preview ships with. The model checks and
// the thumbnail renderer use more of the library, so they load it through here, typed by its full declarations.
import type { AnimationClip, Group, LoadingManager } from 'three-full'

export type * from 'three-full'

export type ThreeModules = typeof import('three-full')


/** What a loaded glTF file gives the creators code. */
export type GLTF = { scene: Group; animations: AnimationClip[] }

type GltfLoader = new (manager?: LoadingManager) => {
  load: (url: string, onLoad: (gltf: GLTF) => void, onProgress?: undefined, onError?: (error: unknown) => void) => void
}

export async function loadThree(): Promise<ThreeModules> {
  return (await import('three')) as unknown as ThreeModules
}

export async function loadGltfLoader(): Promise<GltfLoader> {
  return (await import('three/examples/jsm/loaders/GLTFLoader.js')).GLTFLoader as unknown as GltfLoader
}
