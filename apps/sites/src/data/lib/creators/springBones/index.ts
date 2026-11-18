// Spring bone physics for wearables, ported from the legacy builder's lib/springBones: default
// params, chain roots, subtree sizes (the MAX_SPRING_BONES cap counts every bone a chain drives) and
// the merge back into `item.data.springBones`, keyed by representation model hash.
import { type BuiltItem } from '../itemFactory'
import { type SpringBoneParams, type SpringBonesData } from '@dcl/schemas'
import { ItemType, type Item } from '../items'
import { type BoneNode } from './parseSpringBones'

// Local copies: importing the validator package root pulls every check into the importing chunk.
// springBones.spec.ts keeps them equal to the validator manifest.
export const MAX_SPRING_BONES = 12
export const SPRING_BONE_RANGES = {
  stiffness: [0, 4],
  gravityPower: [0, 2],
  drag: [0, 1],
  gravityDir: [-10, 10]
} as const

export { parseSpringBones, extractGltfJson, type BoneNode } from './parseSpringBones'

export const SPRING_BONES_VERSION = 1

/** Params per spring bone name, for one model. */
export type SpringBoneParamsByName = Record<string, SpringBoneParams>
/** Params per model hash: what `item.data.springBones.models` holds. */
export type SpringBoneParamsByHash = Record<string, SpringBoneParamsByName>

export function getDefaultSpringBoneParams(): SpringBoneParams {
  return { stiffness: 2, gravityPower: 0, gravityDir: [0, -1, 0], drag: 0.5, center: undefined, isRoot: true }
}

/**
 * The tunable half of a chain's params, without `isRoot`: what the editor's copy/paste moves between
 * chains. `center` is always present so pasting a chain without one clears the target's.
 */
export function pickTunableSpringBoneParams(params: SpringBoneParams): Omit<SpringBoneParams, 'isRoot'> {
  return {
    stiffness: params.stiffness,
    gravityPower: params.gravityPower,
    gravityDir: [...params.gravityDir],
    drag: params.drag,
    center: params.center
  }
}

/** Whether two chains carry the same physics. */
function sameParams(a: SpringBoneParams, b: SpringBoneParams): boolean {
  return (
    a.stiffness === b.stiffness &&
    a.gravityPower === b.gravityPower &&
    a.drag === b.drag &&
    a.center === b.center &&
    a.isRoot === b.isRoot &&
    a.gravityDir.every((value, axis) => value === b.gravityDir[axis])
  )
}

/**
 * Whether two sets of params are the same, by value: the editor's dirty check. Key order is not
 * content — deleting a chain and adding it back must not read as an edit.
 */
export function sameSpringBoneParams(a: SpringBoneParamsByHash, b: SpringBoneParamsByHash): boolean {
  const hashes = Object.keys(a)
  if (hashes.length !== Object.keys(b).length) return false
  return hashes.every(hash => {
    const left = a[hash]
    const right = b[hash]
    if (!right) return false
    const names = Object.keys(left)
    return (
      names.length === Object.keys(right).length &&
      names.every(name => right[name] && sameParams(left[name], right[name]))
    )
  })
}

export function getSpringBones(bones: BoneNode[]): BoneNode[] {
  return bones.filter(bone => bone.type === 'spring')
}

export function hasSpringBones(bones: BoneNode[]): boolean {
  return bones.some(bone => bone.type === 'spring')
}

function countSubtreeSize(boneById: Map<number, BoneNode>, nodeId: number): number {
  const bone = boneById.get(nodeId)
  if (!bone) return 0
  return bone.children.reduce((count, childId) => count + countSubtreeSize(boneById, childId), 1)
}

/** Spring bone name → number of bones its chain drives (itself plus descendants). */
export function buildSubtreeSizes(bones: BoneNode[]): Map<string, number> {
  const boneById = new Map(bones.map(bone => [bone.nodeId, bone]))
  return new Map(getSpringBones(bones).map(bone => [bone.name, countSubtreeSize(boneById, bone.nodeId)]))
}

/** Bones driven by every configured chain, against the MAX_SPRING_BONES cap. */
export function sumConfiguredBones(subtreeSizes: Map<string, number>, params: SpringBoneParamsByName): number {
  return Object.keys(params).reduce((total, name) => total + (subtreeSizes.get(name) ?? 1), 0)
}

/** Spring bones whose parent is not a spring bone: the only sensible chain starts. */
export function getChainRoots(bones: BoneNode[]): BoneNode[] {
  const parentById = new Map<number, number>()
  const springIds = new Set<number>()
  for (const bone of bones) {
    if (bone.type === 'spring') springIds.add(bone.nodeId)
    for (const childId of bone.children) parentById.set(childId, bone.nodeId)
  }
  return getSpringBones(bones).filter(bone => {
    const parentId = parentById.get(bone.nodeId)
    return parentId === undefined || !springIds.has(parentId)
  })
}

/**
 * Default params for the chain roots, admitted greedily in node order while the driven-bone total
 * stays within the cap (a root too big for the remaining budget is skipped, smaller ones may still fit).
 */
export function getDefaultSpringBoneRoots(bones: BoneNode[]): SpringBoneParamsByName {
  const subtreeSizes = buildSubtreeSizes(bones)
  const roots: SpringBoneParamsByName = {}
  let total = 0
  for (const root of getChainRoots(bones)) {
    const size = subtreeSizes.get(root.name) ?? 1
    if (total + size > MAX_SPRING_BONES) continue
    roots[root.name] = getDefaultSpringBoneParams()
    total += size
  }
  return roots
}

/** A bone with its bone children: what the hierarchy pickers render. */
export type BoneTreeNode = {
  bone: BoneNode
  children: BoneTreeNode[]
}

/**
 * The model's bones as a tree. Nodes that are not skin joints (meshes, the armature, empty
 * containers) are dropped and their bone descendants lift to the nearest bone ancestor, so the
 * picker shows the skeleton and nothing else.
 */
export function buildBoneTree(bones: BoneNode[]): BoneTreeNode[] {
  const boneById = new Map(bones.map(bone => [bone.nodeId, bone]))
  const childIds = new Set(bones.flatMap(bone => bone.children))
  const visit = (nodeId: number): BoneTreeNode[] => {
    const bone = boneById.get(nodeId)
    if (!bone) return []
    const children = bone.children.flatMap(visit)
    return bone.isJoint ? [{ bone, children }] : children
  }
  return bones.filter(bone => !childIds.has(bone.nodeId)).flatMap(bone => visit(bone.nodeId))
}

/** Bone names in depth-first tree order, so configured chains list top-down like the model. */
export function sortByHierarchy(bones: BoneNode[], names: string[]): string[] {
  const order = new Map<string, number>()
  const boneById = new Map(bones.map(bone => [bone.nodeId, bone]))
  const childIds = new Set(bones.flatMap(bone => bone.children))
  let index = 0
  const visit = (nodeId: number) => {
    const bone = boneById.get(nodeId)
    if (!bone) return
    order.set(bone.name, index++)
    bone.children.forEach(visit)
  }
  for (const bone of bones) if (!childIds.has(bone.nodeId)) visit(bone.nodeId)
  return [...names].sort((a, b) => (order.get(a) ?? 0) - (order.get(b) ?? 0))
}

/** The content hash of each representation's main model, de-duplicated (a unisex GLB appears once). */
export function getRepresentationModelHashes(item: Item): string[] {
  const hashes: string[] = []
  for (const representation of item.data.representations) {
    const hash = item.contents[representation.mainFile]
    if (hash && !hashes.includes(hash)) hashes.push(hash)
  }
  return hashes
}

/** Saved params for one model, or the seeded defaults when the item has none yet. */
export function getInitialSpringBoneParams(item: Item, hash: string, bones: BoneNode[]): SpringBoneParamsByName {
  const saved = item.data.springBones?.models[hash]
  if (saved) {
    // Bones that left the model since the params were saved are dropped.
    const names = new Set(getSpringBones(bones).map(bone => bone.name))
    return Object.fromEntries(Object.entries(saved).filter(([name]) => names.has(name)))
  }
  return getDefaultSpringBoneRoots(bones)
}

/** What a model change carries spring bones from: the item as it stood and its params per model hash. */
export type SpringBonesCarrySource = { item: Item; params: SpringBoneParamsByHash }

/**
 * Params for a model change, carried over per body shape: a new model inherits the chains of the
 * model that served its first body shape, kept by bone name, and defaults only for roots that are
 * new to the model. Unchanged models keep their params as they are.
 */
export function carrySpringBoneParams(
  source: SpringBonesCarrySource,
  next: Item,
  bonesByHash: Record<string, BoneNode[]>
): SpringBoneParamsByHash {
  const sourceHashes = getRepresentationModelHashes(source.item)
  const params: SpringBoneParamsByHash = {}
  for (const hash of getRepresentationModelHashes(next)) {
    const bones = bonesByHash[hash]
    if (!bones || !hasSpringBones(bones)) continue
    const sourceHash = sourceHashes.includes(hash) ? hash : getSourceModelHash(source.item, next, hash)
    const sourceParams = sourceHash ? source.params[sourceHash] : undefined
    params[hash] =
      sourceHash && sourceParams
        ? carryModelParams(sourceParams, bonesByHash[sourceHash], bones)
        : getDefaultSpringBoneRoots(bones)
  }
  return params
}

/** The source model hash that served the first body shape of the new model. */
function getSourceModelHash(source: Item, next: Item, hash: string): string | undefined {
  const bodyShapes = next.data.representations
    .filter(representation => next.contents[representation.mainFile] === hash)
    .flatMap(representation => representation.bodyShapes)
  for (const bodyShape of bodyShapes) {
    const representation = source.data.representations.find(candidate => candidate.bodyShapes.includes(bodyShape))
    const sourceHash = representation && source.contents[representation.mainFile]
    if (sourceHash) return sourceHash
  }
  return undefined
}

function carryModelParams(
  source: SpringBoneParamsByName,
  sourceBones: BoneNode[] | undefined,
  bones: BoneNode[]
): SpringBoneParamsByName {
  const names = new Set(getSpringBones(bones).map(bone => bone.name))
  const carried: SpringBoneParamsByName = Object.fromEntries(Object.entries(source).filter(([name]) => names.has(name)))
  // A root the old model already had but left unconfigured was removed on purpose; only new roots get defaults.
  const known = new Set(sourceBones ? getChainRoots(sourceBones).map(bone => bone.name) : Object.keys(source))
  const subtreeSizes = buildSubtreeSizes(bones)
  let total = sumConfiguredBones(subtreeSizes, carried)
  for (const root of getChainRoots(bones)) {
    if (known.has(root.name) || carried[root.name]) continue
    const size = subtreeSizes.get(root.name) ?? 1
    if (total + size > MAX_SPRING_BONES) continue
    carried[root.name] = getDefaultSpringBoneParams()
    total += size
  }
  return carried
}

/** `item.data.springBones` for the given params; undefined when no model has any chain configured. */
export function mergeSpringBonesIntoItem(paramsByHash: SpringBoneParamsByHash): SpringBonesData | undefined {
  const models: SpringBoneParamsByHash = {}
  for (const [hash, params] of Object.entries(paramsByHash)) {
    if (Object.keys(params).length > 0) models[hash] = params
  }
  return Object.keys(models).length > 0 ? { version: SPRING_BONES_VERSION, models } : undefined
}

export function isSpringBoneItem(item: Item): boolean {
  return item.type === ItemType.WEARABLE
}

/**
 * For a two-model item, the hashes whose model has spring bones but no chain configured while the
 * other model has some: saving would ship physics for one body shape only.
 */
export function getShapesMissingSpringBones(
  paramsByHash: SpringBoneParamsByHash,
  bonesByHash: Record<string, BoneNode[]>
): string[] {
  const hashes = Object.keys(bonesByHash).filter(hash => hasSpringBones(bonesByHash[hash]))
  if (hashes.length < 2) return []
  const configured = hashes.filter(hash => Object.keys(paramsByHash[hash] ?? {}).length > 0)
  if (configured.length === 0 || configured.length === hashes.length) return []
  return hashes.filter(hash => !configured.includes(hash))
}

/** The new model's files by hash: they exist only in memory, storage would 404 on them. */
export function pendingBlobsByHash(pending: BuiltItem): Record<string, Blob> {
  const blobs: Record<string, Blob> = {}
  for (const [path, blob] of Object.entries(pending.blobs)) {
    const hash = pending.item.contents[path]
    if (hash) blobs[hash] = blob
  }
  return blobs
}

/**
 * The spring bones to store with a model change saved on its own: the params as they stand (the form's,
 * unsaved tuning included, else the saved ones) carried onto the new models by bone name, defaults for
 * roots new to them. `null` when no new model has spring bones (the replaced model's params go),
 * `undefined` for items without models.
 */
export async function carrySpringBonesForSave(
  fetchBones: (hash: string, blob?: Blob) => Promise<BoneNode[]>,
  item: Item,
  pending: BuiltItem,
  params?: SpringBoneParamsByHash
): Promise<SpringBonesData | null | undefined> {
  const next = pending.item
  if (!isSpringBoneItem(next)) return undefined
  const hashes = getRepresentationModelHashes(next)
  if (hashes.length === 0) return undefined
  // The form may hold params for some models only (still loading); the saved ones fill the rest.
  const source: SpringBonesCarrySource = { item, params: { ...item.data.springBones?.models, ...params } }
  const sourceHashes = isSpringBoneItem(item) ? getRepresentationModelHashes(item) : []
  const blobsByHash = pendingBlobsByHash(pending)
  const parsed: Record<string, BoneNode[]> = {}
  await Promise.all([
    ...hashes.map(async hash => {
      parsed[hash] = await fetchBones(hash, blobsByHash[hash])
    }),
    // A replaced model that can't be read any more carries nothing by itself; the params still carry by name.
    ...sourceHashes
      .filter(hash => !hashes.includes(hash))
      .map(async hash => {
        try {
          parsed[hash] = await fetchBones(hash)
        } catch {
          /* nothing to tell a removed root from a new one; defaults apply */
        }
      })
  ])
  const carried = carrySpringBoneParams(source, next, parsed)
  const result: SpringBoneParamsByHash = {}
  for (const hash of hashes) {
    const bones = parsed[hash]
    if (!bones || !hasSpringBones(bones)) continue
    result[hash] = carried[hash] ?? getInitialSpringBoneParams(item, hash, bones)
  }
  return mergeSpringBonesIntoItem(result) ?? null
}
