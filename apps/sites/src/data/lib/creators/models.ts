// Three.js model analysis for uploaded items: emote detection, emote metrics and the GLB
// validation suite. Ported from the legacy builder's lib/getModelData; rendering (metrics +
// thumbnails) is NOT done here — that comes from the WearablePreview iframe controller.
import type { GLTF } from 'three/examples/jsm/loaders/GLTFLoader.js'
import { ArmatureId, WearableCategory } from '@dcl/schemas'
import { ItemType } from './items'
import { ItemFileError, MAX_EMOTE_DURATION, isImageFile } from './itemFiles'
import { suggestWearableCategory } from './suggestWearableCategory'
import { captureError } from './monitoring'
import { getValidator, type ValidationContext, type ValidationIssue, type ValidationSource } from './validation'
import { hidesForCategory } from './wearableCategories'

const ARMATURE_PREFIX = 'Armature'
// Kept equal to the validator manifest's propArmatureName by springBones.spec.ts's rule book parity test.
export const PROP_ARMATURE_NAME = 'Armature_Prop'

export type AnimationMetrics = {
  sequences: number
  duration: number
  frames: number
  fps: number
  props: number
  additionalArmatures: number
}

export type ModelAnalysis = {
  type: ItemType
  validationIssues: ValidationIssue[]
  suggestedCategory: WearableCategory | null
  emoteMetrics?: AnimationMetrics
}

/** Blob URLs for every content file, so the loader can resolve a .gltf's external buffers/textures. */
function toObjectURLs(contents: Record<string, Blob>): Record<string, string> {
  return Object.fromEntries(Object.entries(contents).map(([path, blob]) => [path, URL.createObjectURL(blob)]))
}

function revokeObjectURLs(mappings: Record<string, string>): void {
  for (const url of Object.values(mappings)) URL.revokeObjectURL(url)
}

export async function loadGltf(url: string, mappings: Record<string, string>): Promise<GLTF> {
  const Three = await import('three')
  const { GLTFLoader } = await import('three/examples/jsm/loaders/GLTFLoader.js')

  const manager = new Three.LoadingManager()
  const missingResources: string[] = []
  manager.setURLModifier(requestedUrl => {
    const path = requestedUrl.replace('blob:', '').split('/').pop() ?? requestedUrl
    const key = Object.keys(mappings).find(mappingKey => mappingKey.endsWith(path))
    return key ? mappings[key] : requestedUrl
  })
  manager.onError = failedUrl => {
    missingResources.push(failedUrl.replace('blob:', '').split('/').pop() ?? failedUrl)
  }

  const loader = new GLTFLoader(manager)
  return new Promise<GLTF>((resolve, reject) => {
    loader.load(url, resolve, undefined, error => {
      // Surface the real parse failure for debugging/Sentry; the user still gets friendly copy.
      console.error('GLTF load failed:', error)
      reject(
        missingResources.length > 0
          ? new ItemFileError('missing_external_resources', { fileNames: missingResources.join(', ') })
          : new ItemFileError('invalid_model_file')
      )
    })
  })
}

export function getEmoteMetrics(gltf: Pick<GLTF, 'scene' | 'animations'>): AnimationMetrics {
  const { scene, animations } = gltf
  const armatures = scene.children.filter(({ name }) => name.startsWith(ARMATURE_PREFIX))
  const animation = animations[0]
  const propsAnimation = animations.length > 1 ? animations[1] : null

  const hasBaseMesh = scene.children.some(sceneItem =>
    sceneItem.children.some(
      item => item.name.toLowerCase().includes('basemesh') || item.name.toLowerCase().includes('avatar_mesh')
    )
  )
  // Social emotes (a second avatar rig) can't be authored here yet: without startAnimation/outcomes
  // metadata they would publish as broken plain emotes, so they are refused at import. Checked first so
  // a creator isn't asked to fix the mesh of a file that is unsupported anyway.
  if (armatures.some(({ name }) => name === (ArmatureId.Armature_Other as string))) {
    throw new ItemFileError('emote_social_unsupported')
  }
  if (hasBaseMesh) {
    throw new ItemFileError('emote_with_mesh')
  }
  if (propsAnimation && propsAnimation.duration !== animation.duration) {
    throw new ItemFileError('emote_animations_out_of_sync')
  }

  let frames = 0
  for (const track of animation.tracks) {
    frames = Math.max(frames, track.times.length)
  }

  return {
    sequences: animations.length,
    duration: animation.duration,
    frames,
    fps: animation.duration > 0 ? frames / animation.duration : 0,
    props: armatures.some(({ name }) => name === PROP_ARMATURE_NAME) ? 1 : 0,
    additionalArmatures: 0
  }
}

/** Validation is advisory at import: a validator failure (e.g. its chunk failing to load) keeps the model. */
async function validateAdvisory(source: ValidationSource, ctx: ValidationContext): Promise<ValidationIssue[]> {
  try {
    return (await getValidator().validate(source, ctx)).issues
  } catch (error) {
    captureError(error, { flow: 'add_items', step: 'validate' })
    return []
  }
}

/**
 * Loads the main model once and derives everything the details step needs: wearable vs emote,
 * the full validation-issue list, a suggested category (wearables) and emote metrics (emotes).
 * PNG image wearables skip the 3D parse entirely.
 */
export async function analyzeModel(
  model: string,
  contents: Record<string, Blob>,
  category?: WearableCategory,
  hides?: string[]
): Promise<ModelAnalysis> {
  if (isImageFile(model)) {
    return { type: ItemType.WEARABLE, validationIssues: [], suggestedCategory: null }
  }

  const Three = await import('three')
  const mappings = toObjectURLs(contents)
  const url = mappings[model]
  const source = { kind: 'blob', contents, mainFile: model } as const
  try {
    const gltf = await loadGltf(url, mappings)
    const isEmote = gltf.animations.length > 0

    if (isEmote) {
      const emoteMetrics = getEmoteMetrics(gltf)
      if (emoteMetrics.duration > MAX_EMOTE_DURATION) {
        throw new ItemFileError('emote_duration_too_long', { seconds: MAX_EMOTE_DURATION })
      }
      const issues = await validateAdvisory(source, { type: ItemType.EMOTE })
      return { type: ItemType.EMOTE, validationIssues: issues, suggestedCategory: null, emoteMetrics }
    }

    // A wearable without geometry can't be previewed or published; the hidden preview would wait for it forever.
    let hasMesh = false
    gltf.scene.traverse(object => {
      if ((object as { isMesh?: boolean }).isMesh) hasMesh = true
    })
    if (!hasMesh) throw new ItemFileError('invalid_model_file')

    // Suggestion only — a throw on malformed geometry must never fail the import.
    let suggestedCategory: WearableCategory | null = null
    try {
      suggestedCategory = suggestWearableCategory(Three, gltf.scene)
    } catch {
      suggestedCategory = null
    }
    // Validated for the category the draft will start with, so the modal doesn't re-run it right away. A hide
    // on that category itself would pool its triangle budget although renderers ignore it.
    const startCategory = category ?? suggestedCategory ?? undefined
    const issues = await validateAdvisory(source, {
      type: ItemType.WEARABLE,
      category: startCategory,
      hides: hides && hidesForCategory(startCategory, hides)
    })
    return { type: ItemType.WEARABLE, validationIssues: issues, suggestedCategory }
  } finally {
    revokeObjectURLs(mappings)
  }
}
