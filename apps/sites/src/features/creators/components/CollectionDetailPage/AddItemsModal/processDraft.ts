// Async half of draft creation: file loading + model analysis + manifest prefills. Results are
// dispatched into the pure reducer; WearablePreview-dependent data (wearable metrics, auto
// thumbnails) is filled afterwards by DraftProcessor.
import { type WearableCategory } from '@dcl/schemas'
import { blobToDataURL, loadVideoMetadata } from '~/lib/media'
import { sanitizeItemDescription, unique } from '~/lib/itemDraft'
import { EmotePlayMode, ITEM_NAME_MAX_LENGTH } from '~/lib/itemFactory'
import { ItemFileError, THUMBNAIL_PATH, VIDEO_PATH, isImageFile, loadItemFile } from '~/lib/itemFiles'
import { BodyShapeType, IMAGE_WEARABLE_METRICS, ItemType } from '~/lib/items'
import { analyzeModel } from '~/lib/models'
import { isRarity } from '~/lib/rarities'
import { imageThumbnailPatch, needsPreviewData } from '~/lib/draftPreview'
import { getWearableCategoryOptions, hidesForCategory } from '~/lib/wearableCategories'
import { resolveLinkedManifest, type LinkedManifestTarget } from '~/lib/linkedManifest'
import { type ItemDraft } from './AddItemsModal.state'

/** The linked collection a draft is for: what its items may be and where their URN and mapping come from. */
export type LinkedDraftTarget = LinkedManifestTarget

function sanitizeRarity(rarity: string | undefined): string | null {
  const value = rarity?.toLowerCase()
  return isRarity(value) ? value : null
}

function sanitizePlayMode(playMode: string | undefined): EmotePlayMode | null {
  if (playMode === (EmotePlayMode.LOOP as string)) return EmotePlayMode.LOOP
  if (playMode === (EmotePlayMode.SIMPLE as string)) return EmotePlayMode.SIMPLE
  return null
}

/**
 * Loads and analyzes one dropped file into a draft patch. Throws ItemFileError for user-facing
 * import problems. The returned patch may still lack metrics/thumbnail — DraftProcessor fills
 * those through the WearablePreview iframe.
 */
export async function processDraftFile(
  file: File,
  prefillHides?: string[],
  linked?: LinkedDraftTarget
): Promise<Partial<ItemDraft>> {
  const loaded = await loadItemFile(file)
  // Linked collections hold plain wearables only (legacy: "This item type is not allowed in Linked Wearables Collections").
  if (linked && loaded.scene) throw new ItemFileError('linked_smart_wearable')
  // A video shipped in the zip gets the same decode check as one picked in the form.
  if (loaded.contents[VIDEO_PATH]) {
    await loadVideoMetadata(loaded.contents[VIDEO_PATH]).catch(() => {
      throw new ItemFileError('invalid_video')
    })
  }
  // A wearable manifest category the file can't take (a 3D model as eyes) is ignored, so the picker, the checks and
  // the saved item all fall back to the suggestion instead of disagreeing.
  const manifest = loaded.wearable?.data
  const wearableCategory =
    manifest?.category && getWearableCategoryOptions(loaded.contents).includes(manifest.category)
      ? manifest.category
      : undefined
  // The manifest's category and hides drive the category-dependent limits (triangle budget, skin caps). Its
  // `replaces` joins the hides, as the editor does on load. A live preview prefill never ships a manifest.
  const hides =
    prefillHides ??
    hidesForCategory(wearableCategory, unique([...(manifest?.hides ?? []), ...(manifest?.replaces ?? [])]))
  const analysis = await analyzeModel(
    loaded.model,
    loaded.contents,
    wearableCategory as WearableCategory | undefined,
    hides
  )

  if (linked && analysis.type === ItemType.EMOTE) throw new ItemFileError('linked_emote')
  const isSmart = !!loaded.scene
  // Emotes and smart wearables are always unisex.
  const isUnisex = analysis.type === ItemType.EMOTE || isSmart
  const patch: Partial<ItemDraft> = {
    contents: loaded.contents,
    model: loaded.model,
    type: analysis.type,
    validationIssues: analysis.validationIssues,
    validatedCategory: wearableCategory ?? analysis.suggestedCategory,
    emoteMetrics: analysis.emoteMetrics ?? null,
    bodyShape: isUnisex ? BodyShapeType.BOTH : (loaded.bodyShape ?? BodyShapeType.BOTH),
    bodyShapeLocked: isUnisex,
    isSmart,
    requiredPermissions: loaded.scene?.requiredPermissions ?? [],
    hides
  }

  if (analysis.suggestedCategory) {
    patch.category = analysis.suggestedCategory
    patch.suggestedCategory = analysis.suggestedCategory
  }

  if (analysis.type === ItemType.EMOTE && analysis.emoteMetrics) {
    const { sequences, duration, frames, fps, props, additionalArmatures } = analysis.emoteMetrics
    patch.metrics = { sequences, duration, frames, fps, props, additionalArmatures }
  }

  if (linked) {
    // A zip's wearable.json may fix the URN and the mapping; anything else gets the default "any token" mapping.
    Object.assign(patch, resolveLinkedManifest(loaded.wearable, linked))
  }

  if (loaded.wearable) {
    patch.name = loaded.wearable.name.slice(0, ITEM_NAME_MAX_LENGTH)
    if (wearableCategory) patch.category = wearableCategory
    const rarity = sanitizeRarity(loaded.wearable.rarity)
    if (rarity) patch.rarity = rarity
    patch.description = sanitizeItemDescription(loaded.wearable.description ?? '')
    patch.utility = (loaded.wearable.utility ?? '').trim()
    patch.tags = loaded.wearable.data.tags ?? []
    patch.blockVrmExport = loaded.wearable.data.blockVrmExport ?? false
  } else if (loaded.emote) {
    if (loaded.emote.name) patch.name = loaded.emote.name.slice(0, ITEM_NAME_MAX_LENGTH)
    if (loaded.emote.category) patch.category = loaded.emote.category
    const rarity = sanitizeRarity(loaded.emote.rarity)
    if (rarity) patch.rarity = rarity
    patch.description = sanitizeItemDescription(loaded.emote.description ?? '')
    patch.utility = (loaded.emote.utility ?? '').trim()
    patch.tags = loaded.emote.tags ?? []
    const playMode = sanitizePlayMode(loaded.emote.play_mode)
    if (playMode) patch.playMode = playMode
  }

  // The suggestion may stand in for an unusable manifest category, so self-hides are dropped against the final one.
  if (patch.hides && patch.category) patch.hides = hidesForCategory(patch.category, patch.hides)

  if (isImageFile(loaded.model)) {
    // Image wearables need no 3D render: fixed metrics + canvas-padded thumbnail.
    patch.metrics = IMAGE_WEARABLE_METRICS
    Object.assign(patch, await imageThumbnailPatch(loaded.contents, loaded.model, patch.category ?? null))
    // A zip-provided thumbnail is the creator's; only our own padding is redone on a category change.
    patch.isAutoThumbnail = !loaded.contents[THUMBNAIL_PATH]
    patch.autoThumbnailCategory = patch.category ?? null
  } else if (loaded.contents[THUMBNAIL_PATH]) {
    // A zip-provided thumbnail wins over the auto screenshot.
    patch.thumbnail = await blobToDataURL(loaded.contents[THUMBNAIL_PATH])
  }

  return patch
}

/** Image wearable whose auto thumbnail was padded for a different category (the padding differs per category). */
export function isImageThumbnailStale(draft: ItemDraft): boolean {
  return (
    draft.status === 'ready' &&
    isImageFile(draft.model) &&
    draft.isAutoThumbnail &&
    draft.category !== null &&
    draft.autoThumbnailCategory !== draft.category
  )
}

/**
 * Next draft for the DraftProcessor. Switching the processor mid-load throws that load away, so a
 * draft already in flight keeps its turn; otherwise the one on screen goes before the rest, and the rest
 * finish the type the iframe was last mounted for (`lastId`), since a type switch reboots the preview.
 */
export function pickPreviewDraft(
  drafts: ItemDraft[],
  selectedId: string | null,
  lastId: string | null
): ItemDraft | null {
  const pending = (draft: ItemDraft) => draft.status === 'ready' && needsPreviewData(draft)
  const last = drafts.find(draft => draft.id === lastId)
  if (last && pending(last)) return last
  const selected = drafts.find(draft => draft.id === selectedId)
  if (selected && pending(selected)) return selected
  return drafts.find(draft => pending(draft) && draft.type === last?.type) ?? drafts.find(pending) ?? null
}
