// Builds builder-server Items out of reviewed upload drafts, ported from the legacy
// CreateSingleItemModal's createItem/addItemRepresentation + utils (sortContent,
// buildRepresentations) and modules/deployment/contentUtils (computeHashes).
import { hashV1 } from '@dcl/hashing'
import { type Mappings } from '@dcl/schemas'
import { maxUint256 } from 'viem'
import {
  BODY_SHAPE_FEMALE,
  BODY_SHAPE_MALE,
  BodyShapeType,
  ItemType,
  getItemBodyShapeType,
  tracksVideoOnSave,
  type Item,
  type ItemMetrics,
  type ItemRepresentation
} from './items'
import {
  IMAGE_PATH,
  ItemFileError,
  MAX_THUMBNAIL_FILE_SIZE,
  THUMBNAIL_PATH,
  VIDEO_PATH,
  getBodyShapeTypeFromContents,
  isModelFile,
  isModelPath,
  toMB
} from './itemFiles'
import { compressPngBlob, generateCatalystImage } from './media'
import { mergeSpringBonesIntoItem, type SpringBoneParamsByName } from './springBones'

export const ITEM_NAME_MAX_LENGTH = 32

// Same rules as collection names: non-empty, ≤32 chars and no ':' (names feed the on-chain metadata).
export function isValidItemName(name: string): boolean {
  const trimmed = name.trim()
  return trimmed.length > 0 && trimmed.length <= ITEM_NAME_MAX_LENGTH && !trimmed.includes(':')
}

export enum EmotePlayMode {
  SIMPLE = 'simple',
  LOOP = 'loop'
}

const UPPER_BODY_CATEGORY = 'upper_body'
const HANDS_BODY_PART = 'hands'

export type SortedContent = {
  male: Record<string, Blob>
  female: Record<string, Blob>
  all: Record<string, Blob>
}

/** A reviewed draft, ready to become an Item. */
export type ItemDraftPayload = {
  id: string
  name: string
  type: ItemType
  bodyShape: BodyShapeType
  category: string
  rarity: string
  playMode?: EmotePlayMode
  /** Smart wearables only: permissions read from the zip's scene.json. */
  requiredPermissions?: string[]
  /** From wearable.json / emote.json when the zip ships one. */
  description?: string
  utility?: string
  tags?: string[]
  blockVrmExport?: boolean
  /** Wearables only: slots and body parts the item hides (the Blender live preview hands these over). */
  hides?: string[]
  /** Wearables only: tuned spring bone params for the model, keyed by bone name. */
  springBoneParams?: SpringBoneParamsByName
  /** Normalized contents including thumbnail.png (and video.mp4 for a smart wearable); model/texture keys unprefixed unless a BOTH zip. */
  contents: Record<string, Blob>
  /** Main model/texture path within contents. */
  model: string
  metrics: ItemMetrics
  owner: string
  collectionId: string
  /** Linked items only: the item's URN (collection URN + its own suffix) and the tokens that unlock it. */
  urn?: string
  mappings?: Mappings
}

const prefixContentName = (bodyShape: BodyShapeType, contentKey: string): string => `${bodyShape}/${contentKey}`

// Item-level files that are never part of a body-shape representation.
const ROOT_PATHS = new Set([THUMBNAIL_PATH, VIDEO_PATH, IMAGE_PATH])

/** Prefixes every content key with the body shape; the thumbnail, catalyst image and video stay at the root. */
const prefixContents = (bodyShape: BodyShapeType, contents: Record<string, Blob>): Record<string, Blob> => {
  return Object.keys(contents).reduce((newContents: Record<string, Blob>, key: string) => {
    if (ROOT_PATHS.has(key)) {
      return newContents
    }
    newContents[prefixContentName(bodyShape, key)] = contents[key]
    return newContents
  }, {})
}

/**
 * Sorts contents into male / female / all according to the chosen body shape. `all` carries the
 * thumbnail plus both prefixed representations.
 */
export function sortContent(bodyShape: BodyShapeType, contents: Record<string, Blob>): SortedContent {
  const male =
    bodyShape === BodyShapeType.BOTH || bodyShape === BodyShapeType.MALE
      ? prefixContents(BodyShapeType.MALE, contents)
      : {}
  const female =
    bodyShape === BodyShapeType.BOTH || bodyShape === BodyShapeType.FEMALE
      ? prefixContents(BodyShapeType.FEMALE, contents)
      : {}
  return { male, female, all: withRootFiles({ ...male, ...female }, contents) }
}

function withRootFiles(all: Record<string, Blob>, contents: Record<string, Blob>): Record<string, Blob> {
  for (const path of ROOT_PATHS) {
    if (contents[path]) all[path] = contents[path]
  }
  return all
}

/** Variant of sortContent for zips that already ship male/ and female/ folders. */
export function sortContentZipBothBodyShape(bodyShape: BodyShapeType, contents: Record<string, Blob>): SortedContent {
  let male: Record<string, Blob> = {}
  let female: Record<string, Blob> = {}
  const both: Record<string, Blob> = {}

  for (const [key, value] of Object.entries(contents)) {
    // The folder of a shape not picked is left out, never treated as a shared file.
    if (key.startsWith('male/')) {
      if (bodyShape === BodyShapeType.BOTH || bodyShape === BodyShapeType.MALE) male[key] = value
    } else if (key.startsWith('female/')) {
      if (bodyShape === BodyShapeType.BOTH || bodyShape === BodyShapeType.FEMALE) female[key] = value
    } else {
      both[key] = value
    }
  }

  male = {
    ...male,
    ...(bodyShape === BodyShapeType.BOTH || bodyShape === BodyShapeType.MALE
      ? prefixContents(BodyShapeType.MALE, both)
      : {})
  }
  female = {
    ...female,
    ...(bodyShape === BodyShapeType.BOTH || bodyShape === BodyShapeType.FEMALE
      ? prefixContents(BodyShapeType.FEMALE, both)
      : {})
  }

  return { male, female, all: withRootFiles({ ...male, ...female }, contents) }
}

export function buildRepresentations(
  bodyShape: BodyShapeType,
  model: string,
  contents: SortedContent
): ItemRepresentation[] {
  const representations: ItemRepresentation[] = []

  if (bodyShape === BodyShapeType.MALE || bodyShape === BodyShapeType.BOTH) {
    representations.push({
      bodyShapes: [BODY_SHAPE_MALE],
      mainFile: prefixContentName(BodyShapeType.MALE, model),
      contents: Object.keys(contents.male),
      overrideHides: [],
      overrideReplaces: []
    })
  }

  if (bodyShape === BodyShapeType.FEMALE || bodyShape === BodyShapeType.BOTH) {
    representations.push({
      bodyShapes: [BODY_SHAPE_FEMALE],
      mainFile: prefixContentName(BodyShapeType.FEMALE, model),
      contents: Object.keys(contents.female),
      overrideHides: [],
      overrideReplaces: []
    })
  }

  return representations
}

export function buildRepresentationsZipBothBodyShape(
  bodyShape: BodyShapeType,
  contents: SortedContent
): ItemRepresentation[] {
  const representations: ItemRepresentation[] = []

  if (bodyShape === BodyShapeType.MALE || bodyShape === BodyShapeType.BOTH) {
    const maleMainFile = Object.keys(contents.male).find(isModelFile) ?? Object.keys(contents.male).find(isModelPath)
    if (!maleMainFile) throw new ItemFileError('missing_model_file')
    representations.push({
      bodyShapes: [BODY_SHAPE_MALE],
      mainFile: maleMainFile,
      contents: Object.keys(contents.male),
      overrideHides: [],
      overrideReplaces: []
    })
  }

  if (bodyShape === BodyShapeType.FEMALE || bodyShape === BodyShapeType.BOTH) {
    const femaleMainFile =
      Object.keys(contents.female).find(isModelFile) ?? Object.keys(contents.female).find(isModelPath)
    if (!femaleMainFile) throw new ItemFileError('missing_model_file')
    representations.push({
      bodyShapes: [BODY_SHAPE_FEMALE],
      mainFile: femaleMainFile,
      contents: Object.keys(contents.female),
      overrideHides: [],
      overrideReplaces: []
    })
  }

  return representations
}

export async function computeHashes(contents: Record<string, Blob>): Promise<Record<string, string>> {
  const hashes: Record<string, string> = {}
  for (const path in contents) {
    const buffer = await contents[path].arrayBuffer()
    hashes[path] = await hashV1(new Uint8Array(buffer))
  }
  return hashes
}

/** The thumbnail's own cap, re-checked right before a save uploads a new one (legacy sagas). */
export function assertThumbnailSize(blobs: Record<string, Blob>): void {
  const thumbnail = blobs[THUMBNAIL_PATH]
  if (thumbnail && thumbnail.size > MAX_THUMBNAIL_FILE_SIZE) {
    throw new ItemFileError('thumbnail_too_big', { size: toMB(MAX_THUMBNAIL_FILE_SIZE) })
  }
}

/**
 * An emote has one model for both body shapes: from a zip with a folder per body shape, only the folder of the
 * picked model is kept, moved to the root.
 */
function toSingleModel(
  contents: Record<string, Blob>,
  model: string
): { contents: Record<string, Blob>; model: string } {
  if (getBodyShapeTypeFromContents(contents) !== BodyShapeType.BOTH) return { contents, model }
  const [kept, dropped] = [BodyShapeType.MALE, BodyShapeType.FEMALE].map(shape => `${shape}/`)
  const [keep, drop] = model.startsWith(dropped) ? [dropped, kept] : [kept, dropped]
  const strip = (path: string) => (path.startsWith(keep) ? path.slice(keep.length) : path)
  return {
    contents: Object.fromEntries(
      Object.entries(contents)
        .filter(([path]) => !path.startsWith(drop))
        .map(([path, blob]) => [strip(path), blob])
    ),
    model: strip(model)
  }
}

/** An item to persist plus the blobs to upload, keyed by content path. */
export type BuiltItem = {
  item: Item
  blobs: Record<string, Blob>
}

/**
 * Builds a brand-new Item from a reviewed draft (legacy createItem): contents are prefixed per
 * body shape and hashed, representations derived, and the item is priced as "not for sale".
 */
export async function buildItem(draft: ItemDraftPayload): Promise<BuiltItem> {
  if (draft.type === ItemType.EMOTE) draft = { ...draft, ...toSingleModel(draft.contents, draft.model) }
  const isBothZip =
    draft.type === ItemType.WEARABLE && getBodyShapeTypeFromContents(draft.contents) === BodyShapeType.BOTH
  const sorted = isBothZip
    ? sortContentZipBothBodyShape(draft.bodyShape, draft.contents)
    : sortContent(draft.bodyShape, draft.contents)
  const representations = isBothZip
    ? buildRepresentationsZipBothBodyShape(draft.bodyShape, sorted)
    : buildRepresentations(draft.bodyShape, draft.model, sorted)

  const hides = draft.hides ?? []
  const data =
    draft.type === ItemType.WEARABLE
      ? {
          category: draft.category,
          replaces: [],
          hides,
          removesDefaultHiding:
            draft.category === UPPER_BODY_CATEGORY || hides.includes(UPPER_BODY_CATEGORY) ? [HANDS_BODY_PART] : [],
          tags: draft.tags ?? [],
          representations:
            hides.length > 0
              ? representations.map(representation => ({ ...representation, overrideHides: hides }))
              : representations,
          blockVrmExport: draft.blockVrmExport ?? false,
          outlineCompatible: true,
          // Legacy sends an empty list for every wearable; a smart one carries its scene.json permissions.
          requiredPermissions: draft.requiredPermissions ?? []
        }
      : {
          category: draft.category,
          loop: draft.playMode === EmotePlayMode.LOOP,
          tags: draft.tags ?? [],
          representations
        }

  const now = Date.now()
  const blobs = await withCatalystImage(sorted.all, draft.rarity)
  const contents = await computeHashes(blobs)
  // Spring bones are keyed by the representation model hash, known only now.
  const springBones =
    draft.type === ItemType.WEARABLE && draft.springBoneParams
      ? mergeSpringBonesIntoItem(
          Object.fromEntries(
            representations.map(representation => [contents[representation.mainFile], draft.springBoneParams!])
          )
        )
      : undefined
  const item: Item = {
    id: draft.id,
    name: draft.name,
    description: (draft.description ?? '').trim(),
    ...(draft.utility?.trim() ? { utility: draft.utility.trim() } : {}),
    thumbnail: THUMBNAIL_PATH,
    ...(contents[VIDEO_PATH] ? { video: contents[VIDEO_PATH] } : {}),
    owner: draft.owner,
    collectionId: draft.collectionId,
    totalSupply: 0,
    isPublished: false,
    isApproved: false,
    inCatalyst: false,
    rarity: draft.rarity,
    ...(draft.urn ? { urn: draft.urn } : {}),
    ...(draft.mappings ? { mappings: draft.mappings } : {}),
    type: draft.type,
    data: springBones ? { ...data, springBones } : data,
    metrics: draft.metrics,
    contents,
    // "Not for sale" defaults, as the legacy modal saves standard items.
    price: maxUint256.toString(),
    beneficiary: draft.owner,
    createdAt: now,
    updatedAt: now
  }

  return { item, blobs }
}

/**
 * Quantizes the thumbnail and adds the catalyst image next to it (legacy compressPngBlob + generateCatalystImage
 * before every save). Quantizing first keeps the stored thumbnail under its cap more often.
 */
async function withCatalystImage(blobs: Record<string, Blob>, rarity: string): Promise<Record<string, Blob>> {
  if (!blobs[THUMBNAIL_PATH]) return blobs
  const thumbnail = await compressPngBlob(blobs[THUMBNAIL_PATH])
  const image = await compressPngBlob(await generateCatalystImage(thumbnail, rarity))
  return { ...blobs, [THUMBNAIL_PATH]: thumbnail, [IMAGE_PATH]: image }
}

/**
 * Rebuilds only the catalyst image from the stored thumbnail, for a rarity change that uploads no new thumbnail
 * (legacy regenerated the image whenever the rarity changed).
 */
export async function withCatalystImageForRarity(item: Item, thumbnail: Blob): Promise<BuiltItem> {
  const image = await compressPngBlob(await generateCatalystImage(thumbnail, item.rarity ?? ''))
  const hashes = await computeHashes({ [IMAGE_PATH]: image })
  return { item: { ...item, contents: { ...item.contents, ...hashes } }, blobs: { [IMAGE_PATH]: image } }
}

/**
 * Appends a single-body-shape draft to an existing item as a new representation (legacy
 * addItemRepresentation). The target's thumbnail is kept — the draft's is dropped.
 */
export async function addRepresentationToItem(
  target: BuiltItem | { item: Item; blobs?: Record<string, Blob> },
  draft: ItemDraftPayload
): Promise<BuiltItem> {
  if (draft.bodyShape === BodyShapeType.BOTH) {
    throw new ItemFileError('invalid_representation')
  }
  const targetItem = target.item
  if (
    getItemBodyShapeType(targetItem) !==
    (draft.bodyShape === BodyShapeType.MALE ? BodyShapeType.FEMALE : BodyShapeType.MALE)
  ) {
    throw new ItemFileError('invalid_representation')
  }

  const sorted = sortContent(draft.bodyShape, draft.contents)
  const newBlobs = draft.bodyShape === BodyShapeType.MALE ? sorted.male : sorted.female
  const hashedContents = await computeHashes(newBlobs)
  const representations = buildRepresentations(draft.bodyShape, draft.model, sorted)

  const removesDefaultHiding =
    targetItem.data.category === UPPER_BODY_CATEGORY || (targetItem.data.hides ?? []).includes(UPPER_BODY_CATEGORY)
      ? [HANDS_BODY_PART]
      : targetItem.data.removesDefaultHiding

  const item: Item = {
    ...targetItem,
    data: {
      ...targetItem.data,
      representations: [...targetItem.data.representations, ...representations],
      removesDefaultHiding
    },
    contents: { ...targetItem.contents, ...hashedContents },
    updatedAt: Date.now()
  }

  return { item, blobs: { ...(target.blobs ?? {}), ...newBlobs } }
}

/** Replaces a saved item's thumbnail with a new PNG, hashing it so the file can be uploaded alongside. */
export async function withThumbnail(item: Item, thumbnail: Blob): Promise<BuiltItem> {
  const blobs = await withCatalystImage({ [THUMBNAIL_PATH]: thumbnail }, item.rarity ?? '')
  const hashes = await computeHashes(blobs)
  const contents = { ...item.contents }
  if (item.thumbnail !== THUMBNAIL_PATH) delete contents[item.thumbnail]
  return {
    item: { ...item, thumbnail: THUMBNAIL_PATH, contents: { ...contents, ...hashes }, updatedAt: Date.now() },
    blobs
  }
}

/** Hashes from the legacy CIDv0 algorithm ("Qm…"); Catalyst deployments need the current hashV1. */
export function isOldHash(hash: string): boolean {
  return hash.startsWith('Qm')
}

export function hasOldHashedContents(item: Item): boolean {
  return Object.values(item.contents).some(isOldHash)
}

/**
 * Re-hashes the item's legacy-hashed files with the current algorithm (legacy reHashOlderContents):
 * downloads each of them and returns the item pointing at the new hashes plus the files to re-upload.
 */
export async function withRehashedContents(item: Item, download: (hash: string) => Promise<Blob>): Promise<BuiltItem> {
  const stale = Object.entries(item.contents).filter(([, hash]) => isOldHash(hash))
  const blobs = Object.fromEntries(
    await Promise.all(stale.map(async ([path, hash]) => [path, await download(hash)] as const))
  )
  const hashes = await computeHashes(blobs)
  return { item: { ...item, contents: { ...item.contents, ...hashes }, updatedAt: Date.now() }, blobs }
}

/** A freshly imported model for an existing item: the same file-side shape the add-items flow produces. */
export type ReplacementModel = {
  contents: Record<string, Blob>
  model: string
  bodyShape: BodyShapeType
  metrics: ItemMetrics
  /** A smart wearable's new preview video; `null` drops the current one (the new file has no scene), `undefined` keeps it. */
  video?: Blob | null
  /** A smart wearable's scene permissions; `null` clears them (the new file has no scene), `undefined` keeps them. */
  requiredPermissions?: string[] | null
}

/**
 * Swaps a saved item's model for a new file (legacy "change file"): the representations are rebuilt for
 * `replacement.bodyShape` from the new contents (every previous representation goes), while the
 * thumbnail, catalyst image and video stay. Callers reject a wearable↔emote type switch before getting here.
 */
export async function withReplacedModel(item: Item, replacement: ReplacementModel): Promise<BuiltItem> {
  const { bodyShape } = replacement
  const { contents: files, model } =
    item.type === ItemType.EMOTE ? toSingleModel(replacement.contents, replacement.model) : replacement
  const isBothZip = getBodyShapeTypeFromContents(files) === BodyShapeType.BOTH
  const sorted = isBothZip ? sortContentZipBothBodyShape(bodyShape, files) : sortContent(bodyShape, files)
  const representations = isBothZip
    ? buildRepresentationsZipBothBodyShape(bodyShape, sorted)
    : buildRepresentations(bodyShape, model, sorted)
  const blobs = { ...sorted.male, ...sorted.female }
  const kept: Record<string, string> = {}
  for (const path of ROOT_PATHS) {
    if (item.contents[path] && (path !== VIDEO_PATH || replacement.video === undefined))
      kept[path] = item.contents[path]
  }
  if (replacement.video) blobs[VIDEO_PATH] = replacement.video
  const hashes = await computeHashes(blobs)
  const data = { ...item.data, representations }
  if (replacement.requiredPermissions === null) delete data.requiredPermissions
  else if (replacement.requiredPermissions) data.requiredPermissions = replacement.requiredPermissions
  const next: Item = {
    ...item,
    data,
    metrics: replacement.metrics,
    contents: { ...kept, ...hashes },
    updatedAt: Date.now()
  }
  // The approved hash stays until curation copies the new one over (see tracksVideoOnSave).
  if (replacement.video && tracksVideoOnSave(item)) next.video = hashes[VIDEO_PATH]
  if (replacement.video === null) delete next.video
  return { item: next, blobs }
}

/** Files that stay with the item when `shape` is replaced: every path the other shape's representation lists. */
export function getKeptRepresentationPaths(item: Item, shape: string): string[] {
  return item.data.representations
    .filter(representation => !representation.bodyShapes.includes(shape))
    .flatMap(representation => representation.contents)
}

/**
 * Replaces one body shape's model of a saved item and keeps the other representation and its files. The
 * item must already have that shape, in a representation of its own: this editor and the legacy builder
 * never store one representation for both shapes.
 */
export async function withReplacedRepresentation(
  item: Item,
  bodyShape: BodyShapeType.MALE | BodyShapeType.FEMALE,
  replacement: Omit<ReplacementModel, 'bodyShape' | 'video' | 'requiredPermissions'>
): Promise<BuiltItem> {
  const wearableBodyShape = bodyShape === BodyShapeType.MALE ? BODY_SHAPE_MALE : BODY_SHAPE_FEMALE
  const index = item.data.representations.findIndex(representation =>
    representation.bodyShapes.includes(wearableBodyShape)
  )
  // Not a shape of its own (one representation for both): replacing it would drop the other shape.
  if (index === -1 || item.data.representations[index].bodyShapes.length > 1) {
    throw new ItemFileError('invalid_representation')
  }

  const sorted = sortContent(bodyShape, replacement.contents)
  const representations = buildRepresentations(bodyShape, replacement.model, sorted)
  const blobs = bodyShape === BodyShapeType.MALE ? sorted.male : sorted.female
  const hashes = await computeHashes(blobs)
  const replaced = item.data.representations[index]
  const kept = new Set(getKeptRepresentationPaths(item, wearableBodyShape))
  const contents: Record<string, string> = {}
  for (const [path, hash] of Object.entries(item.contents)) {
    if (!replaced.contents.includes(path) || kept.has(path)) contents[path] = hash
  }
  const next = [...item.data.representations]
  next[index] = {
    ...representations[0],
    overrideHides: replaced.overrideHides ?? [],
    overrideReplaces: replaced.overrideReplaces ?? []
  }
  return {
    item: {
      ...item,
      data: { ...item.data, representations: next },
      metrics: replacement.metrics,
      contents: { ...contents, ...hashes },
      updatedAt: Date.now()
    },
    blobs
  }
}
