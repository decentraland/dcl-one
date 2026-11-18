// Wraps draft contents into the blob shapes the WearablePreview iframe accepts, ported from the
// legacy CreateSingleItemModal utils (toWearableWithBlobs / toEmoteWithBlobs).
import {
  BodyShape,
  EmoteCategory,
  Locale,
  PreviewEmote,
  WearableCategory,
  type EmoteDataADR74,
  type EmoteDefinition,
  type EmoteWithBlobs,
  type HideableWearableCategory,
  type PreviewOptions,
  type WearableDefinition,
  type WearableWithBlobs
} from '@dcl/schemas'
import { type AvatarAttributes } from './avatar'
import { getContentsStorageUrl } from './builder'
import { isImageFile, isModelFile } from './itemFiles'
import { ItemType, type Item } from './items'

function findMainFile(contents: Record<string, Blob>): string {
  const model = Object.keys(contents).find(isModelFile)
  const image = Object.keys(contents).find(isImageFile)
  const mainFile = model ?? image
  if (!mainFile) throw new Error('No main content for the preview')
  return mainFile
}

// WearablePreview diffs its options with deep-equal, which sees every Blob as equal to any other, so a
// reused iframe only notices a new model when the wrapper carries a distinct `id`.
export function toWearableWithBlobs(contents: Record<string, Blob>, id = 'preview-item'): WearableWithBlobs {
  return {
    id,
    name: '',
    description: '',
    image: '',
    thumbnail: '',
    i18n: [],
    data: {
      category: WearableCategory.HAT,
      hides: [],
      replaces: [],
      removesDefaultHiding: [],
      tags: [],
      representations: [
        {
          bodyShapes: [BodyShape.MALE, BodyShape.FEMALE],
          mainFile: findMainFile(contents),
          contents: Object.entries(contents).map(([key, blob]) => ({ key, blob })),
          overrideHides: [],
          overrideReplaces: []
        }
      ],
      blockVrmExport: false,
      outlineCompatible: true
    }
  }
}

export function toEmoteWithBlobs(contents: Record<string, Blob>, id = 'preview-item'): EmoteWithBlobs {
  return {
    id,
    name: '',
    description: '',
    image: '',
    thumbnail: '',
    i18n: [],
    emoteDataADR74: {
      category: EmoteCategory.DANCE,
      tags: [],
      representations: [
        {
          bodyShapes: [BodyShape.MALE, BodyShape.FEMALE],
          mainFile: findMainFile(contents),
          contents: Object.entries(contents).map(([key, blob]) => ({ key, blob }))
        }
      ],
      loop: false
    }
  }
}

/**
 * A saved item as the definition the preview iframe renders (legacy toWearable / toEmote): every
 * content path resolved to its public storage URL. `id = item.id` so a changed model needs no new id.
 */
export function itemToDefinition(item: Item): WearableDefinition | EmoteDefinition {
  const toContents = (paths: string[]) =>
    paths.map(path => ({ key: path, url: getContentsStorageUrl(item.contents[path]) }))
  const base = {
    id: item.id,
    name: item.name,
    description: item.description,
    thumbnail: item.thumbnail,
    image: item.thumbnail,
    i18n: [{ code: Locale.EN, text: item.name }]
  }
  if (item.type === ItemType.EMOTE) {
    return {
      ...base,
      emoteDataADR74: {
        ...item.data,
        category: item.data.category as EmoteCategory,
        // `Item` keeps social emote data opaque: the editor never edits it, it only passes it through.
        startAnimation: item.data.startAnimation as EmoteDataADR74['startAnimation'],
        outcomes: item.data.outcomes as EmoteDataADR74['outcomes'],
        tags: item.data.tags ?? [],
        loop: !!item.data.loop,
        representations: item.data.representations.map(representation => ({
          bodyShapes: representation.bodyShapes as BodyShape[],
          mainFile: representation.mainFile,
          contents: toContents(representation.contents)
        }))
      }
    } satisfies EmoteDefinition
  }
  return {
    ...base,
    data: {
      ...item.data,
      category: item.data.category as WearableCategory,
      hides: (item.data.hides ?? []) as HideableWearableCategory[],
      replaces: (item.data.replaces ?? []) as HideableWearableCategory[],
      removesDefaultHiding: item.data.removesDefaultHiding as HideableWearableCategory[] | undefined,
      tags: item.data.tags ?? [],
      representations: item.data.representations.map(representation => ({
        bodyShapes: representation.bodyShapes as BodyShape[],
        mainFile: representation.mainFile,
        contents: toContents(representation.contents),
        overrideHides: (representation.overrideHides ?? []) as HideableWearableCategory[],
        overrideReplaces: (representation.overrideReplaces ?? []) as HideableWearableCategory[]
      }))
    }
  } satisfies WearableDefinition
}

/**
 * The `base64` URL/option form of a definition. Non-ASCII is dropped, as the legacy encoder does:
 * wearable-preview decodes with a bare `JSON.parse(atob(base64))`, so UTF-8 bytes smuggled through
 * `btoa` would come back as mojibake. Only display strings (an item's name) can carry them, and the
 * name shown in the editor comes from the item itself, not from the definition.
 */
export function definitionToBase64(definition: WearableDefinition | EmoteDefinition): string {
  return btoa(JSON.stringify(definition).replace(/[^\x20-\x7F]/g, ''))
}

/** An item's model files picked in the editor but not uploaded yet, by content path. */
export type UnsavedFiles = { itemId: string; blobs: Record<string, Blob> }

/**
 * What the avatar preview shows: saved items (storage URLs), optionally one of them with unsaved
 * model files, or one in-memory definition (blobs).
 */
export type AvatarPreviewSource =
  | { kind: 'items'; items: Item[]; unsaved?: UnsavedFiles }
  | { kind: 'definition'; definition: WearableWithBlobs | EmoteWithBlobs }

/**
 * An item with unsaved model files as the iframe can load it. Storage 404s on the new files' hashes, so
 * the representation the avatar wears goes as blobs; one still on storage (adding the other body shape's
 * representation) goes as a plain definition with only the stored representations.
 */
export function unsavedItemPreview(
  item: Item,
  blobs: Record<string, Blob>,
  bodyShape: BodyShape
): { blob: WearableWithBlobs | EmoteWithBlobs } | { item: Item } | null {
  const isLocal = (representation: Item['data']['representations'][number]) =>
    representation.contents.every(path => path in blobs)
  const local = item.data.representations.filter(isLocal)
  if (!local.some(representation => representation.bodyShapes.includes(bodyShape))) {
    const stored = item.data.representations.filter(representation => !isLocal(representation))
    return stored.length > 0 ? { item: { ...item, data: { ...item.data, representations: stored } } } : null
  }
  const definition = itemToDefinition({ ...item, data: { ...item.data, representations: local } })
  const toBlobs = (contents: { key: string }[]) => contents.map(({ key }) => ({ key, blob: blobs[key] }))
  // The bridge and the iframe compare options as JSON, where every Blob is `{}`: the content hashes in
  // the id are what tell two picked files apart.
  const hashes = local.flatMap(representation => representation.contents.map(path => item.contents[path]))
  const id = `${item.id}:${hashes.join(',')}`
  if ('emoteDataADR74' in definition) {
    const { emoteDataADR74 } = definition
    return {
      blob: {
        ...definition,
        id,
        emoteDataADR74: {
          ...emoteDataADR74,
          representations: emoteDataADR74.representations.map(representation => ({
            ...representation,
            contents: toBlobs(representation.contents)
          }))
        }
      }
    }
  }
  return {
    blob: {
      ...definition,
      id,
      data: {
        ...definition.data,
        representations: definition.data.representations.map(representation => ({
          ...representation,
          contents: toBlobs(representation.contents)
        }))
      }
    }
  }
}

/** Whether the preview is showing an emote item, which then owns the animation. */
export function isEmoteSubject(source: AvatarPreviewSource): boolean {
  if (source.kind === 'items') return source.items.some(item => item.type === ItemType.EMOTE)
  return 'emoteDataADR74' in source.definition
}

/**
 * Camera range, Babylon only (Unity drives its own camera). wearable-preview frames the avatar between
 * `radius / (zoom × zoomScale)` (closest) and `wheelZoom ×` that (farthest), starting at `wheelStart`% of
 * the way in — the prop is inverted, so 50 means halfway. With these values the start is the legacy
 * framing (`radius / 1.75`) and the wheel zooms 3× in like Unity's FOV range, and ~1.7× out.
 */
export const PREVIEW_ZOOM_SCALE = 3
export const PREVIEW_WHEEL_ZOOM = 5
export const PREVIEW_WHEEL_START = 50
/** wearable-preview's own default multiplier; restated because the scale has to start from a known value. */
const BASE_ZOOM = 1.75
// The jump animation leaves the frame at the default zoom, so it is framed further out (legacy quirk).
const JUMP_ZOOM = 1
// The `zoom` option is not a multiplier: wearable-preview maps 0–100 onto this range. Keep in sync with
// its `computeZoom` (src/lib/zoom.ts there), or the framing shifts silently.
const MIN_ZOOM = 1
const MAX_ZOOM = 2.8

/** The `zoom` option (0–100) that puts the camera at the emote's legacy framing before scaling. */
export function getPreviewZoom(emote: PreviewEmote | undefined): number {
  const multiplier = emote === PreviewEmote.JUMP ? JUMP_ZOOM : BASE_ZOOM
  return ((multiplier - MIN_ZOOM) / (MAX_ZOOM - MIN_ZOOM)) * 100
}

/**
 * The full option set the iframe needs to dress and animate the avatar. Sent whole on every change:
 * wearable-preview replaces its overrides with each UPDATE rather than merging them.
 */
export function buildPreviewOptions(
  source: AvatarPreviewSource,
  avatar: AvatarAttributes,
  emote: PreviewEmote,
  showSceneBoundaries = false
): PreviewOptions {
  const emoteSubject = isEmoteSubject(source)
  const options: PreviewOptions = {
    bodyShape: avatar.bodyShape,
    skin: avatar.skin,
    eyes: avatar.eyes,
    hair: avatar.hair,
    urns: avatar.baseWearableUrns,
    // An emptied slot stays empty instead of wearable-preview filling it with its default outfit.
    disableDefaultWearables: avatar.hasOutfit,
    disableDefaultEmotes: emoteSubject,
    showSceneBoundaries
  }
  if (source.kind === 'items') {
    const unsavedItem = source.unsaved && source.items.find(item => item.id === source.unsaved?.itemId)
    const unsaved =
      unsavedItem && source.unsaved ? unsavedItemPreview(unsavedItem, source.unsaved.blobs, avatar.bodyShape) : null
    options.base64s = source.items.flatMap(item => {
      if (item !== unsavedItem) return [definitionToBase64(itemToDefinition(item))]
      return unsaved && 'item' in unsaved ? [definitionToBase64(itemToDefinition(unsaved.item))] : []
    })
    if (unsaved && 'blob' in unsaved) options.blob = unsaved.blob
  } else {
    options.blob = source.definition
  }
  options.zoom = getPreviewZoom(emoteSubject ? undefined : emote)
  // ui2 has no prop for it, so it only reaches the iframe through the UPDATE message.
  options.zoomScale = PREVIEW_ZOOM_SCALE
  if (!emoteSubject) options.emote = emote
  return options
}

// The Unity renderer refuses scene queries (screenshot, metrics) while a reload is in flight, aborts a
// capture that a reload overtook, and its first LOAD can belong to the scene it booted with, before the
// item sent by UPDATE was applied.
const PREVIEW_RELOADING_REASONS = ['The preview is reloading', 'The preview reloaded during the capture']
const PREVIEW_EMPTY_REASON = 'No requested item is loaded'

export function isPreviewReloading(error: unknown): boolean {
  return error instanceof Error && PREVIEW_RELOADING_REASONS.includes(error.message)
}

/** Unity's answer to a metrics read against its boot scene, where Babylon reports all zeros. */
export function isPreviewEmpty(error: unknown): boolean {
  return error instanceof Error && error.message === PREVIEW_EMPTY_REASON
}

/** Runs a preview query, asking again while the renderer is still reloading, up to `maxWaitMs`. */
export async function retryWhilePreviewReloads<T>(
  query: () => Promise<T>,
  { delayMs = 500, maxWaitMs = 30_000 }: { delayMs?: number; maxWaitMs?: number } = {}
): Promise<T> {
  const deadline = Date.now() + maxWaitMs
  for (;;) {
    try {
      return await query()
    } catch (error) {
      if (!isPreviewReloading(error) || Date.now() + delayMs > deadline) throw error
      await new Promise(resolve => setTimeout(resolve, delayMs))
    }
  }
}
