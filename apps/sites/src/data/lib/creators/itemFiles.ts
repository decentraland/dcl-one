// Single funnel for item file ingestion (drag & drop / browse), ported from the legacy builder's
// ImportStep + @dcl/builder-client loadFile: unzips, reads manifests, normalizes contents and
// detects body shapes. 3D model analysis lives in lib/models; this module is engine-free.
import JSZip from 'jszip'
import {
  EmoteCategory,
  EmotePlayMode,
  HideableWearableCategory,
  Rarity,
  RequiredPermission,
  Scene,
  WearableCategory,
  WearableRepresentation,
  generateLazyValidator,
  type JSONSchema
} from '@dcl/schemas'
import { BodyShapeType, IMAGE_PATH, VIDEO_PATH, hasSceneCode, isSceneCodeFile } from './items'
import { blobsAreEqual } from './blobs'

export const THUMBNAIL_PATH = 'thumbnail.png'
/** A smart wearable's scene manifest; kept inside the item contents so the explorer can run it. */
export const SCENE_PATH = 'scene.json'
export { IMAGE_PATH, VIDEO_PATH }

export const ITEM_EXTENSIONS = ['.zip', '.gltf', '.glb', '.png']
export const VIDEO_EXTENSIONS = ['.mp4']

// The caps mirror the wearable-validator manifest (`manifest.fileSize`), the catalyst's ADR-51 limits; the rule book
// parity specs keep them equal. They are not read from the manifest at runtime: importing it would pull the whole
// rule book, which loads lazily, into the editor chunks.
export const MAX_THUMBNAIL_FILE_SIZE = 1024 * 1024 // 1MB
/** A wearable's whole deployment (lib/itemSize): models, textures, thumbnail and rarity image. */
export const MAX_WEARABLE_FILE_SIZE = 3 * 1024 * 1024 // 3MB
export const MAX_SKIN_FILE_SIZE = 9 * 1024 * 1024 // 9MB
export const MAX_EMOTE_FILE_SIZE = 3 * 1024 * 1024 // 3MB
export const MAX_VIDEO_FILE_SIZE = 262144000 // 250MB
export const MAX_EMOTE_DURATION = 10 // seconds

const WEARABLE_MANIFEST = 'wearable.json'
const EMOTE_MANIFEST = 'emote.json'
const BUILDER_MANIFEST = 'builder.json'
const MAX_ZIP_ENTRIES = 500
/** The unpacked bytes one zip may produce: the largest model budget plus a thumbnail and a smart wearable's video. */
export const MAX_ZIP_CONTENTS_SIZE = MAX_SKIN_FILE_SIZE + MAX_THUMBNAIL_FILE_SIZE + MAX_VIDEO_FILE_SIZE
/** A zip bigger than anything it is allowed to unpack to is refused before it is opened. */
export const MAX_ZIP_FILE_SIZE = MAX_ZIP_CONTENTS_SIZE

/** Import failure the UI can translate: `add_items_modal.file_error.{messageKey}`. */
export class ItemFileError extends Error {
  messageKey: string
  messageParams?: Record<string, string | number>

  constructor(messageKey: string, messageParams?: Record<string, string | number>) {
    super(`Item file error: ${messageKey}`)
    this.name = 'ItemFileError'
    this.messageKey = messageKey
    this.messageParams = messageParams
  }
}

export function toMB(bytes: number): number {
  return Math.round((bytes / 1024 / 1024) * 10) / 10
}

// JSZip documents `internalStream` on entries; its shipped typings leave it out.
type StreamableEntry = JSZip.JSZipObject & {
  internalStream(type: 'uint8array'): JSZip.JSZipStreamHelper<Uint8Array<ArrayBuffer>>
}

/**
 * Inflates zip entries one at a time against a shared byte budget, aborting the very chunk that takes the
 * running unpacked total past `maxBytes`: a zip bomb never gets to allocate its payload.
 */
export function createZipInflater(maxBytes: number) {
  let total = 0
  return {
    inflate(entry: JSZip.JSZipObject): Promise<Blob> {
      return new Promise((resolve, reject) => {
        const chunks: BlobPart[] = []
        let size = 0
        // JSZip may still flush buffered events after pause(); nothing past the first outcome counts.
        let settled = false
        const stream = (entry as StreamableEntry).internalStream('uint8array')
        stream
          .on('data', chunk => {
            if (settled) return
            size += chunk.length
            if (total + size > maxBytes) {
              settled = true
              stream.pause()
              reject(new ItemFileError('zip_too_big', { size: toMB(maxBytes) }))
              return
            }
            chunks.push(chunk)
          })
          .on('error', error => {
            if (settled) return
            settled = true
            reject(error)
          })
          .on('end', () => {
            if (settled) return
            settled = true
            total += size
            resolve(new Blob(chunks))
          })
          .resume()
      })
    }
  }
}

export function getExtension(fileName: string): string | null {
  const matches = /\.[0-9a-z]+$/i.exec(fileName)
  return matches ? matches[0].toLowerCase() : null
}

/** "my_cool-hat.glb" → "my cool hat" — the default item name for an uploaded file. */
export function cleanAssetName(fileName: string): string {
  const matches = /(.*)\.(.*)/g.exec(fileName)
  return matches && matches.length ? matches[1].replace(/[.\-_]/g, ' ') : fileName
}

export function isImageFile(fileName: string): boolean {
  return fileName.toLowerCase().endsWith('.png')
}

export function isVideoFile(fileName: string): boolean {
  const extension = getExtension(fileName)
  return extension !== null && VIDEO_EXTENSIONS.includes(extension)
}

/**
 * Checks a smart wearable preview video's extension and size. Whether the bytes decode is checked
 * separately in the browser (lib/media loadVideoMetadata). Throws ItemFileError.
 */
export function validateVideoFile(file: File): void {
  if (!isVideoFile(file.name)) {
    throw new ItemFileError('video_wrong_format', { extensions: VIDEO_EXTENSIONS.join(', ') })
  }
  if (file.size > MAX_VIDEO_FILE_SIZE) {
    throw new ItemFileError('video_too_big', { size: toMB(MAX_VIDEO_FILE_SIZE) })
  }
}

export function isModelFile(fileName: string): boolean {
  const lower = fileName.toLowerCase()
  return lower.endsWith('.gltf') || lower.endsWith('.glb')
}

/** True when the file can be an item's main file (a model, or a non-auxiliary PNG texture). */
export function isModelPath(fileName: string): boolean {
  const lower = fileName.toLowerCase()
  if (lower.endsWith('.glb') || lower.endsWith('.gltf')) return true
  // The item's own thumbnail and catalyst image (as a downloaded item zip ships them) are never a texture.
  if (lower.indexOf(THUMBNAIL_PATH) !== -1 || lower === IMAGE_PATH) return false
  // `_mask.png` also covers `_expressions_mask.png` by construction.
  if (lower.endsWith('_mask.png') || lower.endsWith('_expressions.png')) return false
  return lower.endsWith('.png')
}

export function isExpressionsMaskFile(fileName: string): boolean {
  return fileName.toLowerCase().endsWith('_expressions_mask.png')
}

export function isExpressionsFile(fileName: string): boolean {
  return fileName.toLowerCase().endsWith('_expressions.png')
}

/** An `_expressions.png` texture marks a facial-feature wearable that animates expressions. */
export function hasFacialExpressions(contents: Record<string, unknown>): boolean {
  return Object.keys(contents).some(isExpressionsFile)
}

export function isMaskFile(fileName: string): boolean {
  const lower = fileName.toLowerCase()
  return lower.endsWith('_mask.png') && !isExpressionsMaskFile(lower)
}

/**
 * For an auxiliary file (mask / expressions / expressions_mask), the counterpart that must exist:
 * foo_mask.png → foo.png · foo_expressions.png → foo.png · foo_expressions_mask.png → foo_expressions.png.
 * Null when the file isn't auxiliary.
 */
export function getRequiredCounterpartFile(fileName: string): string | null {
  if (isExpressionsMaskFile(fileName)) return fileName.replace(/_expressions_mask\.png$/i, '_expressions.png')
  if (isExpressionsFile(fileName)) return fileName.replace(/_expressions\.png$/i, '.png')
  if (isMaskFile(fileName)) return fileName.replace(/_mask\.png$/i, '.png')
  return null
}

/** Auxiliary PNGs present without their required counterpart (case-insensitive lookup). */
export function findOrphanedAuxiliaryFiles(
  contents: Record<string, unknown>
): Array<{ orphan: string; expected: string }> {
  const keys = Object.keys(contents)
  const lowercaseKeys = new Set(keys.map(key => key.toLowerCase()))
  const orphans: Array<{ orphan: string; expected: string }> = []
  for (const key of keys) {
    const expected = getRequiredCounterpartFile(key)
    if (expected && !lowercaseKeys.has(expected.toLowerCase())) {
      orphans.push({ orphan: key, expected })
    }
  }
  return orphans
}

const BODY_SHAPE_TOP_LEVEL_NAMES = new Set<string>(['male', 'female'])

/**
 * When a creator zipped a folder instead of its files, every entry sits under one wrapper
 * directory. Strip it (recursively) so paths look like a flat zip. Never strips `male`/`female`.
 */
export function stripWrappingFolder(contents: Record<string, Blob>): Record<string, Blob> {
  const meaningfulKeys = Object.keys(contents).filter(key => !key.endsWith('/') && contents[key].size > 0)
  if (meaningfulKeys.length === 0) return contents

  let wrapper: string | null = null
  for (const key of meaningfulKeys) {
    const slashIndex = key.indexOf('/')
    if (slashIndex === -1) return contents
    const topSegment = key.substring(0, slashIndex)
    if (wrapper === null) {
      wrapper = topSegment
    } else if (wrapper !== topSegment) {
      return contents
    }
  }

  if (!wrapper || BODY_SHAPE_TOP_LEVEL_NAMES.has(wrapper.toLowerCase())) {
    return contents
  }

  const prefix = `${wrapper}/`
  const stripped: Record<string, Blob> = {}
  for (const [key, value] of Object.entries(contents)) {
    if (!key.startsWith(prefix)) {
      stripped[key] = value
      continue
    }
    const newKey = key.substring(prefix.length)
    if (newKey) stripped[newKey] = value
  }

  return stripWrappingFolder(stripped)
}

/** Body shape encoded by `male/` / `female/` top-level folders, or null when there are none. */
export function getBodyShapeTypeFromContents(contents: Record<string, unknown>): BodyShapeType | null {
  let hasMale = false
  let hasFemale = false
  for (const key in contents) {
    if (key.startsWith('male/')) hasMale = true
    else if (key.startsWith('female/')) hasFemale = true
  }
  if (hasMale && hasFemale) return BodyShapeType.BOTH
  if (hasMale) return BodyShapeType.MALE
  if (hasFemale) return BodyShapeType.FEMALE
  return null
}

/** Drops a `male/` or `female/` prefix from the paths under it; everything else keeps its path. */
function stripBodyShapeFolder(contents: Record<string, Blob>, shape: BodyShapeType): Record<string, Blob> {
  const prefix = `${shape}/`
  return Object.fromEntries(
    Object.entries(contents).map(([path, blob]) => [path.startsWith(prefix) ? path.slice(prefix.length) : path, blob])
  )
}

export function getModelFileNameFromSubfolder(modelPath: string): string {
  return modelPath.split('/').pop()!
}

/**
 * Normalizes a zip's body-shape subfolders: empty entries dropped; model/image files flattened to
 * the root for a single body shape, kept prefixed for BOTH (so male/eyes.png ≠ female/eyes.png);
 * everything else (audio, etc.) keeps its path.
 */
export function cleanContentModelKeys(
  contents: Record<string, Blob>,
  bodyShapeType?: BodyShapeType
): Record<string, Blob> {
  return Object.keys(contents).reduce((newContents: Record<string, Blob>, key: string) => {
    if (key.indexOf('/') !== -1) {
      if (contents[key].size === 0) {
        return newContents
      }
      if (isModelFile(key) || isImageFile(key)) {
        const targetKey = bodyShapeType === BodyShapeType.BOTH ? key : getModelFileNameFromSubfolder(key)
        newContents[targetKey] = contents[key]
        return newContents
      }
    }
    newContents[key] = contents[key]
    return newContents
  }, {})
}

/** wearable.json manifest — the subset of the legacy WearableConfig this app reads. */
export type WearableManifest = {
  /** Linked wearables: the item's URN, whose last segment becomes its URN suffix. */
  id?: string | null
  name: string
  description?: string
  /** Not in the legacy format: this app's addition, saved as the item's utility. */
  utility?: string
  rarity?: string
  data: {
    category?: string
    hides?: string[]
    replaces?: string[]
    tags?: string[]
    blockVrmExport?: boolean
    representations: Array<{
      bodyShapes: string[]
      mainFile: string
      contents: string[]
      overrideHides?: string[]
      overrideReplaces?: string[]
    }>
  }
  /** Linked wearables: the token mapping, as the docs describe it (a single `Mapping`). */
  mapping?: unknown
}

/** emote.json manifest. */
export type EmoteManifest = {
  name?: string
  description?: string
  utility?: string
  rarity?: string
  category?: string
  play_mode?: string
  tags?: string[]
}

/** scene.json of a smart wearable — the subset this app reads. */
export type SceneManifest = {
  main: string
  requiredPermissions?: string[]
  allowedMediaHostnames?: string[]
}

export type LoadedItemFile = {
  /** File contents by path, manifests excluded (a smart wearable keeps its normalized scene.json). */
  contents: Record<string, Blob>
  /** Main model or texture path within contents. */
  model: string
  /** Body shape detected from the zip structure or manifest, when determinable. */
  bodyShape: BodyShapeType | null
  wearable?: WearableManifest
  emote?: EmoteManifest
  /** Present for smart wearables. */
  scene?: SceneManifest
}

function parseJson(text: string, fileName: string): unknown {
  try {
    return JSON.parse(text)
  } catch {
    throw new ItemFileError('invalid_manifest', { fileName })
  }
}

// Same rules as @dcl/builder-client's WearableConfigSchema / EmoteConfigSchema (legacy loadFile),
// except a representation's overrideHides / overrideReplaces are optional: representations are
// rebuilt from the files anyway and hand-written manifests rarely carry them.
const TAGS_SCHEMA = { type: 'array', items: { type: 'string', minLength: 1 } }
const REPRESENTATION_SCHEMA = {
  ...WearableRepresentation.schema,
  required: ['bodyShapes', 'mainFile', 'contents']
}
const validateWearableManifest = generateLazyValidator<WearableManifest>({
  type: 'object',
  properties: {
    id: { type: 'string', nullable: true },
    description: { type: 'string', nullable: true, maxLength: 64 },
    utility: { type: 'string', nullable: true, maxLength: 64 },
    rarity: { ...Rarity.schema, nullable: true },
    name: { type: 'string' },
    data: {
      type: 'object',
      properties: {
        replaces: { type: 'array', items: HideableWearableCategory.schema },
        hides: { type: 'array', items: HideableWearableCategory.schema },
        tags: TAGS_SCHEMA,
        representations: { type: 'array', items: REPRESENTATION_SCHEMA, minItems: 1 },
        category: WearableCategory.schema,
        removesDefaultHiding: { type: 'array', nullable: true, items: HideableWearableCategory.schema },
        blockVrmExport: { type: 'boolean', nullable: true }
      },
      required: ['replaces', 'hides', 'tags', 'representations', 'category']
    },
    mapping: { type: 'object', nullable: true }
  },
  additionalProperties: false,
  required: ['name', 'data']
} as unknown as JSONSchema<WearableManifest>)

const validateEmoteManifest = generateLazyValidator<EmoteManifest>({
  type: 'object',
  properties: {
    name: { type: 'string', nullable: true },
    description: { type: 'string', nullable: true, maxLength: 64 },
    utility: { type: 'string', nullable: true, maxLength: 64 },
    rarity: { ...Rarity.schema, nullable: true },
    category: { ...EmoteCategory.schema, nullable: true },
    play_mode: { ...EmotePlayMode.schema, nullable: true },
    tags: { ...TAGS_SCHEMA, nullable: true }
  },
  additionalProperties: true,
  required: []
} as unknown as JSONSchema<EmoteManifest>)

function toWearableManifest(value: unknown): WearableManifest {
  if (!validateWearableManifest(value)) {
    throw new ItemFileError('invalid_manifest', { fileName: WEARABLE_MANIFEST })
  }
  return value
}

function toEmoteManifest(value: unknown): EmoteManifest {
  if (!validateEmoteManifest(value)) {
    throw new ItemFileError('invalid_manifest', { fileName: EMOTE_MANIFEST })
  }
  return value
}

const KNOWN_PERMISSIONS = new Set<string>(Object.values(RequiredPermission))

// Permission checks run before the schema so each failure gets its own message (legacy loadSceneConfig).
function toSceneManifest(value: unknown): SceneManifest {
  const scene = value as Partial<Scene> | null
  const permissions = scene?.requiredPermissions as unknown
  if (Array.isArray(permissions)) {
    const unknownPermissions = permissions.filter(permission => !KNOWN_PERMISSIONS.has(String(permission)))
    if (unknownPermissions.length > 0) {
      throw new ItemFileError('unknown_required_permissions', { permissions: unknownPermissions.join(', ') })
    }
    if (new Set(permissions).size !== permissions.length) {
      throw new ItemFileError('duplicated_required_permissions')
    }
    if (permissions.includes(RequiredPermission.ALLOW_MEDIA_HOSTNAMES)) {
      const hostnames = scene?.allowedMediaHostnames
      if (
        !Array.isArray(hostnames) ||
        hostnames.length === 0 ||
        hostnames.some(host => typeof host !== 'string' || !host.trim())
      ) {
        throw new ItemFileError('allowed_media_hostnames_empty')
      }
    }
  }
  if (!scene || !Scene.validate(scene)) {
    throw new ItemFileError('invalid_manifest', { fileName: SCENE_PATH })
  }
  return {
    main: scene.main,
    requiredPermissions: scene.requiredPermissions,
    allowedMediaHostnames: scene.allowedMediaHostnames
  }
}

/**
 * The zip's scene.json: at the root, the single one inside a zipped project folder, or the copies a
 * downloaded item carries under male/ and female/ (one manifest stored per body shape, so they must match).
 */
async function findSceneFile(
  zip: JSZip,
  readText: (entry: JSZip.JSZipObject) => Promise<string>
): Promise<JSZip.JSZipObject | null> {
  const root = zip.file(SCENE_PATH)
  if (root) return root
  const nested = zip.file(/(^|\/)scene\.json$/)
  if (nested.length === 1) return nested[0]
  // Exactly the download layout: one copy under female/ and one under male/, inside the same folder.
  const names = nested.map(file => file.name).sort()
  if (names.length !== 2 || names[0].replace(/(^|\/)female\/scene\.json$/, '$1male/scene.json') !== names[1])
    return null
  // Compared as parsed JSON so a reformatted copy still counts as the same manifest.
  const texts = await Promise.all(nested.map(async file => JSON.stringify(parseJson(await readText(file), SCENE_PATH))))
  if (texts[0] !== texts[1]) throw new ItemFileError('scene_code_mismatch', { fileName: SCENE_PATH })
  return nested[0]
}

/** Parses and validates the scene manifest, checks its code bundle shipped, and returns the normalized file to store. */
async function loadScene(
  sceneFile: JSZip.JSZipObject,
  content: Record<string, Blob>,
  readText: (entry: JSZip.JSZipObject) => Promise<string>
): Promise<{ scene: SceneManifest; blob: Blob }> {
  const rawScene = parseJson(await readText(sceneFile), SCENE_PATH)
  const scene = toSceneManifest(rawScene)
  // The entry point is what makes the wearable "smart" downstream (hasSceneCode), so it must be code.
  if (!isSceneCodeFile(scene.main)) throw new ItemFileError('scene_main_not_code', { fileName: scene.main })
  // The bundle sits at the root or, in a downloaded item, in every body-shape folder: one code for every shape.
  const shape = getBodyShapeTypeFromContents(content)
  const folders = shape === BodyShapeType.BOTH ? [BodyShapeType.MALE, BodyShapeType.FEMALE] : shape ? [shape] : []
  const paths =
    content[scene.main] || folders.length === 0 ? [scene.main] : folders.map(folder => `${folder}/${scene.main}`)
  const missing = paths.find(path => !content[path])
  if (missing) throw new ItemFileError('manifest_file_missing', { fileName: missing })
  if (paths.length === 2 && !(await blobsAreEqual(content[paths[0]], content[paths[1]]))) {
    throw new ItemFileError('scene_code_mismatch', { fileName: scene.main })
  }
  return { scene, blob: new Blob([JSON.stringify(rawScene)], { type: 'application/json' }) }
}

function assertNoOrphanedAuxiliaryFiles(content: Record<string, Blob>): void {
  const [orphan] = findOrphanedAuxiliaryFiles(content)
  if (orphan) {
    throw new ItemFileError('orphaned_auxiliary_file', { fileName: orphan.orphan, expected: orphan.expected })
  }
}

function getManifestBodyShape(wearable: WearableManifest): BodyShapeType {
  const bodyShapes = new Set(wearable.data.representations.flatMap(representation => representation.bodyShapes))
  const hasMale = [...bodyShapes].some(shape => shape.endsWith('BaseMale'))
  const hasFemale = [...bodyShapes].some(shape => shape.endsWith('BaseFemale'))
  if (hasMale && hasFemale) return BodyShapeType.BOTH
  if (hasMale) return BodyShapeType.MALE
  if (hasFemale) return BodyShapeType.FEMALE
  throw new ItemFileError('invalid_manifest', { fileName: WEARABLE_MANIFEST })
}

async function loadZip(file: File): Promise<LoadedItemFile> {
  if (file.size > MAX_ZIP_FILE_SIZE) throw new ItemFileError('zip_too_big', { size: toMB(MAX_ZIP_CONTENTS_SIZE) })
  // Read fully into memory first to avoid racing jszip's lazy reads.
  const buffer = await file.arrayBuffer()
  let zip: JSZip
  try {
    zip = await JSZip.loadAsync(buffer)
  } catch {
    throw new ItemFileError('invalid_zip')
  }

  const wearableFile = zip.file(WEARABLE_MANIFEST)
  const emoteFile = zip.file(EMOTE_MANIFEST)

  const manifests = new Set([WEARABLE_MANIFEST, EMOTE_MANIFEST, SCENE_PATH, BUILDER_MANIFEST])
  const entries: Array<{ path: string; entry: JSZip.JSZipObject }> = []
  zip.forEach((path, entry) => {
    const base = path.split('/').pop() ?? path
    if (!entry.dir && !base.startsWith('.') && !manifests.has(base)) {
      entries.push({ path, entry })
    }
  })

  if (entries.length > MAX_ZIP_ENTRIES) {
    throw new ItemFileError('too_many_files', { max: MAX_ZIP_ENTRIES })
  }

  // Manifests share the budget: a bogus wearable.json is as good a bomb as any model file.
  const inflater = createZipInflater(MAX_ZIP_CONTENTS_SIZE)
  const readText = async (entry: JSZip.JSZipObject) => (await inflater.inflate(entry)).text()
  const sceneFile = await findSceneFile(zip, readText)
  const rawContent: Record<string, Blob> = {}
  for (const { path, entry } of entries) {
    const blob = await inflater.inflate(entry)
    if (blob.size > 0) rawContent[path] = blob
  }

  const thumbnailSize = rawContent[THUMBNAIL_PATH]?.size ?? 0
  if (thumbnailSize > MAX_THUMBNAIL_FILE_SIZE) {
    throw new ItemFileError('thumbnail_too_big', { size: toMB(MAX_THUMBNAIL_FILE_SIZE) })
  }

  // A smart wearable zip may ship its preview video (any .mp4, any folder); it is pulled out of the
  // model contents and stored at the root as video.mp4. Plain zips drop it: only smart items have one.
  // Scene code marks a smart wearable everywhere after import (hasSceneCode), so a zip can't ship
  // it without the manifest that makes it runnable and gates the video.
  if (!sceneFile && hasSceneCode(rawContent)) throw new ItemFileError('scene_manifest_missing')

  const videoPaths = Object.keys(rawContent).filter(isVideoFile)
  if (videoPaths.length > 1) throw new ItemFileError('multiple_videos')
  const video = videoPaths.length === 1 && sceneFile ? rawContent[videoPaths[0]] : null
  for (const path of videoPaths) delete rawContent[path]
  if (video && video.size > MAX_VIDEO_FILE_SIZE) {
    throw new ItemFileError('video_too_big', { size: toMB(MAX_VIDEO_FILE_SIZE) })
  }
  const withVideo = (contents: Record<string, Blob>) => (video ? { ...contents, [VIDEO_PATH]: video } : contents)

  // The thumbnail has its own cap.
  const contentsSize = Object.entries(rawContent).reduce(
    (total, [path, blob]) => (path === THUMBNAIL_PATH ? total : total + blob.size),
    0
  )

  if (wearableFile) {
    const wearable = toWearableManifest(parseJson(await readText(wearableFile), WEARABLE_MANIFEST))
    for (const representation of wearable.data.representations) {
      for (const path of [representation.mainFile, ...representation.contents]) {
        if (!rawContent[path]) throw new ItemFileError('manifest_file_missing', { fileName: path })
      }
    }
    const mainFile = wearable.data.representations[0].mainFile
    if (isImageFile(mainFile)) assertNoOrphanedAuxiliaryFiles(rawContent)

    // Smart wearable: the scene code must ship, and the normalized scene.json travels with the contents.
    let scene: SceneManifest | undefined
    if (sceneFile) {
      // The manifest's paths are relative to itself; next to a root wearable.json only a root scene.json
      // can address the files as they will be stored.
      if (sceneFile.name !== SCENE_PATH) throw new ItemFileError('scene_manifest_nested')
      const loaded = await loadScene(sceneFile, rawContent, readText)
      scene = loaded.scene
      rawContent[SCENE_PATH] = loaded.blob
    }

    const isSkin = wearable.data.category === 'skin'
    const maxSize = isSkin ? MAX_SKIN_FILE_SIZE : MAX_WEARABLE_FILE_SIZE
    if (contentsSize > maxSize) {
      throw new ItemFileError('file_too_big', { size: toMB(maxSize) })
    }
    return {
      contents: withVideo(rawContent),
      model: mainFile,
      // Smart wearables are always unisex, like emotes.
      bodyShape: scene ? BodyShapeType.BOTH : getManifestBodyShape(wearable),
      wearable,
      scene
    }
  }

  if (emoteFile) {
    const emote = toEmoteManifest(parseJson(await readText(emoteFile), EMOTE_MANIFEST))
    if (contentsSize > MAX_EMOTE_FILE_SIZE) {
      throw new ItemFileError('file_too_big', { size: toMB(MAX_EMOTE_FILE_SIZE) })
    }
    const model = Object.keys(rawContent).find(isModelPath)
    if (!model) throw new ItemFileError('missing_model_file')
    return { contents: rawContent, model, bodyShape: BodyShapeType.BOTH, emote }
  }

  // No manifest: unwrap a zipped folder, find the main model, normalize body-shape folders.
  const content = stripWrappingFolder(rawContent)

  // Smart wearable without wearable.json (e.g. a packed SDK project): the scene manifest is enough,
  // the form supplies name, category and rarity. Always unisex.
  if (sceneFile) {
    const { scene, blob } = await loadScene(sceneFile, content, readText)
    // A single body-shape folder is just where the model sits: it serves both shapes, so that one prefix
    // goes; other folders (models/, images/) are the scene's own layout and stay as the code expects them.
    const shape = getBodyShapeTypeFromContents(content)
    const files =
      shape === BodyShapeType.MALE || shape === BodyShapeType.FEMALE ? stripBodyShapeFolder(content, shape) : content
    const keys = Object.keys(files)
    const model = keys.find(isModelFile) ?? keys.find(isModelPath)
    if (!model) throw new ItemFileError('missing_model_file')
    if (contentsSize > MAX_WEARABLE_FILE_SIZE) {
      throw new ItemFileError('file_too_big', { size: toMB(MAX_WEARABLE_FILE_SIZE) })
    }
    return { contents: withVideo({ ...files, [SCENE_PATH]: blob }), model, bodyShape: BodyShapeType.BOTH, scene }
  }

  // A 3D model is the item's main file whenever there is one; a PNG only makes a texture item on its own.
  const keys = Object.keys(content)
  const model = keys.find(isModelFile) ?? keys.find(isModelPath)
  if (!model) throw new ItemFileError('missing_model_file')

  if (isImageFile(model)) assertNoOrphanedAuxiliaryFiles(content)

  // Import-time cap: the most permissive model budget (9MB); the exact per-category/type cap is
  // re-checked at the details step once the type and category are known.
  if (contentsSize > MAX_SKIN_FILE_SIZE) {
    throw new ItemFileError('file_too_big', { size: toMB(MAX_SKIN_FILE_SIZE) })
  }

  const detectedBodyShape = getBodyShapeTypeFromContents(content)
  // Image wearables with no body-shape folders populate both representations from the same files.
  const bodyShape = detectedBodyShape ?? (isImageFile(model) ? BodyShapeType.BOTH : null)

  if (detectedBodyShape === BodyShapeType.BOTH) {
    return { contents: cleanContentModelKeys(content, BodyShapeType.BOTH), model, bodyShape }
  }
  return { contents: cleanContentModelKeys(content), model: getModelFileNameFromSubfolder(model), bodyShape }
}

/**
 * Loads one dropped/browsed file into normalized contents. Zips are unpacked (manifests read),
 * single models/textures become one-file contents. Model analysis (emote detection, metrics,
 * GLB validation) happens separately in lib/models.
 */
export async function loadItemFile(file: File): Promise<LoadedItemFile> {
  const extension = getExtension(file.name)
  if (!extension || !ITEM_EXTENSIONS.includes(extension)) {
    throw new ItemFileError('wrong_extension', { extensions: ITEM_EXTENSIONS.join(', ') })
  }

  if (extension === '.zip') {
    return loadZip(file)
  }

  if (extension === '.png') {
    if (file.size > MAX_WEARABLE_FILE_SIZE) {
      throw new ItemFileError('file_too_big', { size: toMB(MAX_WEARABLE_FILE_SIZE) })
    }
    // A bare texture is an image wearable for both body shapes.
    return { contents: { [file.name]: file }, model: file.name, bodyShape: BodyShapeType.BOTH }
  }

  if (file.size > MAX_SKIN_FILE_SIZE) {
    throw new ItemFileError('file_too_big', { size: toMB(MAX_SKIN_FILE_SIZE) })
  }
  return { contents: { [file.name]: file }, model: file.name, bodyShape: null }
}
