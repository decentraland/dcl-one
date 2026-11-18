// Pure state model for the add-items modal: drafts, review/check bookkeeping and completeness
// rules. All async work (file loading, model analysis, thumbnails, upload) lives outside and
// feeds results in through actions, so this reducer stays fully unit-testable.
import { EmoteCategory, WearableCategory } from '@dcl/schemas'
import { type ValidationIssue } from '~/lib/validation'
import { type Mapping, type Mappings } from '~/lib/mappings'
import { VIDEO_PATH, cleanAssetName, isImageFile, isModelFile } from '~/lib/itemFiles'
import { EmotePlayMode, ITEM_NAME_MAX_LENGTH, isValidItemName } from '~/lib/itemFactory'
import { getItemSizeLimit, isOverSizeLimit, type ItemSize } from '~/lib/itemSize'
import { BodyShapeType, ItemType, getMissingBodyShapeType, type Item, type ItemMetrics } from '~/lib/items'
import { applyPreviewResult, type PreviewResult } from '~/lib/draftPreview'
import { type UploadFailureReason } from '~/lib/uploadItems'
import { type AnimationMetrics } from '~/lib/models'
import { DEFAULT_RARITY } from '~/lib/rarities'
import { IMAGE_WEARABLE_CATEGORIES, hidesForCategory } from '~/lib/wearableCategories'

export type DraftStatus = 'processing' | 'ready' | 'failed'

export type ItemDraft = {
  id: string
  fileName: string
  name: string
  status: DraftStatus
  /** i18n key under add_items_modal.file_error when status is 'failed'. */
  errorKey?: string
  errorParams?: Record<string, string | number>
  type: ItemType | null
  contents: Record<string, Blob>
  model: string
  bodyShape: BodyShapeType
  /** True when the zip structure fixed the shape (male/+female/ folders or a manifest), or the item is unisex by nature. */
  bodyShapeLocked: boolean
  /** Wearable shipping scene code (a zip with scene.json). Unisex, no variants, needs a preview video (contents['video.mp4']) to be saved. */
  isSmart: boolean
  requiredPermissions: string[]
  isVariant: boolean
  variantTargetId: string | null
  category: string | null
  /** Category guessed from the model's geometry; shown as a hint while the user keeps it. */
  suggestedCategory: string | null
  rarity: string
  /** From wearable.json / emote.json when the zip ships one. */
  description: string
  utility: string
  tags: string[]
  blockVrmExport: boolean
  /** Wearables only: the live preview's hides, else the manifest's hides and replaces. Saved and used by every triangle check. */
  hides: string[]
  playMode: EmotePlayMode
  /** Data URL of the current thumbnail; also stored as contents['thumbnail.png'] once final. */
  thumbnail: string | null
  /** True when we generated the thumbnail (not zip-provided or user-picked), so it can be regenerated. */
  isAutoThumbnail: boolean
  /** Category the auto thumbnail was rendered for; its pose decides whether a category change regenerates it. */
  autoThumbnailCategory: string | null
  metrics: ItemMetrics | null
  emoteMetrics: AnimationMetrics | null
  validationIssues: ValidationIssue[]
  /** Category the validation issues were computed for; a different pick re-runs the validator. */
  validatedCategory: string | null
  /** Linked collections only: which tokens of the linked contract unlock the item. */
  mapping: Mapping | null
  /** Edit in bulk: the item's whole mappings, saved as they are while `mapping` is left untouched. */
  keptMappings: Mappings | null
  /** Linked collections only: the URN suffix a zip's wearable.json fixed; generated from the name otherwise. */
  urnSuffix: string | null
  checked: boolean
}

export type AddItemsView = 'details' | 'error'

export type AddItemsState = {
  drafts: ItemDraft[]
  selectedId: string | null
  view: AddItemsView
  failureReason: UploadFailureReason | null
  isUploading: boolean
}

export function createDraft(file: File, { nameFromFile = true } = {}): ItemDraft {
  return {
    id: crypto.randomUUID(),
    fileName: file.name,
    name: nameFromFile ? cleanAssetName(file.name).slice(0, ITEM_NAME_MAX_LENGTH) : '',
    status: 'processing',
    type: null,
    contents: {},
    model: '',
    bodyShape: BodyShapeType.BOTH,
    bodyShapeLocked: false,
    isSmart: false,
    requiredPermissions: [],
    isVariant: false,
    variantTargetId: null,
    category: null,
    suggestedCategory: null,
    rarity: DEFAULT_RARITY,
    description: '',
    utility: '',
    tags: [],
    blockVrmExport: false,
    hides: [],
    playMode: EmotePlayMode.SIMPLE,
    thumbnail: null,
    isAutoThumbnail: false,
    autoThumbnailCategory: null,
    metrics: null,
    emoteMetrics: null,
    validationIssues: [],
    validatedCategory: null,
    mapping: null,
    keptMappings: null,
    urnSuffix: null,
    checked: false
  }
}

export function createInitialState(drafts: ItemDraft[]): AddItemsState {
  return {
    drafts,
    selectedId: drafts[0]?.id ?? null,
    view: 'details',
    failureReason: null,
    isUploading: false
  }
}

export type AddItemsAction =
  | { type: 'draftsAdded'; drafts: ItemDraft[] }
  | { type: 'draftAnalyzed'; id: string; patch: Partial<ItemDraft> }
  | { type: 'draftPreviewed'; id: string; result: PreviewResult }
  | { type: 'draftFailed'; id: string; errorKey: string; errorParams?: Record<string, string | number> }
  | { type: 'draftSelected'; id: string }
  | { type: 'draftRemoved'; id: string }
  | { type: 'draftUpdated'; id: string; patch: Partial<ItemDraft> }
  | { type: 'draftChecked'; id: string }
  | { type: 'uploadStarted' }
  | { type: 'uploadFailed'; failureReason: UploadFailureReason; failedDraftIds: string[] }
  | { type: 'retryRequested' }

/** A wearable model whose issues were computed for another category than the one now picked. */
export function isValidationStale(draft: ItemDraft): boolean {
  return (
    draft.status === 'ready' &&
    draft.type === ItemType.WEARABLE &&
    !isImageFile(draft.model) &&
    draft.category !== draft.validatedCategory
  )
}

function updateDraft(state: AddItemsState, id: string, update: (draft: ItemDraft) => ItemDraft): AddItemsState {
  return { ...state, drafts: state.drafts.map(draft => (draft.id === id ? update(draft) : draft)) }
}

export function addItemsReducer(state: AddItemsState, action: AddItemsAction): AddItemsState {
  switch (action.type) {
    case 'draftsAdded': {
      const drafts = [...state.drafts, ...action.drafts]
      return { ...state, drafts, selectedId: state.selectedId ?? action.drafts[0]?.id ?? null }
    }
    case 'draftAnalyzed':
      return updateDraft(state, action.id, draft => ({ ...draft, ...action.patch, status: 'ready' }))
    case 'draftPreviewed':
      return updateDraft(state, action.id, draft => applyPreviewResult(draft, action.result))
    case 'draftFailed':
      return updateDraft(state, action.id, draft => ({
        ...draft,
        status: 'failed',
        errorKey: action.errorKey,
        errorParams: action.errorParams
      }))
    case 'draftSelected':
      return state.drafts.some(draft => draft.id === action.id) ? { ...state, selectedId: action.id } : state
    case 'draftRemoved': {
      const drafts = state.drafts
        .filter(draft => draft.id !== action.id)
        // A variant pointing at the removed draft loses its target and needs re-review.
        .map(draft =>
          draft.variantTargetId === action.id ? { ...draft, variantTargetId: null, checked: false } : draft
        )
      const selectedId =
        state.selectedId === action.id
          ? (drafts[
              Math.min(
                state.drafts.findIndex(draft => draft.id === action.id),
                drafts.length - 1
              )
            ]?.id ?? null)
          : state.selectedId
      return { ...state, drafts, selectedId }
    }
    case 'draftUpdated':
      // Any edit invalidates a previous review: the draft must be saved (checked) again.
      return updateDraft(state, action.id, draft => ({
        ...draft,
        ...action.patch,
        ...(action.patch.category !== undefined ? { hides: hidesForCategory(action.patch.category, draft.hides) } : {}),
        checked: false
      }))
    case 'draftChecked': {
      const checkedState = updateDraft(state, action.id, draft => ({ ...draft, checked: true }))
      // SAVE & NEXT advances to the next unchecked draft, wrapping around.
      const index = checkedState.drafts.findIndex(draft => draft.id === action.id)
      const ordered = [...checkedState.drafts.slice(index + 1), ...checkedState.drafts.slice(0, index)]
      const next = ordered.find(draft => !draft.checked)
      return { ...checkedState, selectedId: next?.id ?? action.id }
    }
    case 'uploadStarted':
      return { ...state, isUploading: true }
    case 'uploadFailed': {
      const failed = new Set(action.failedDraftIds)
      const drafts = state.drafts.filter(draft => failed.has(draft.id))
      return {
        ...state,
        drafts,
        selectedId: drafts[0]?.id ?? null,
        view: 'error',
        failureReason: action.failureReason,
        isUploading: false
      }
    }
    case 'retryRequested':
      return { ...state, view: 'details', failureReason: null }
  }
}

export type VariantTargetOption = {
  id: string
  label: string
  /** The body shape the target already has (the variant supplies the other one). */
  bodyShape: BodyShapeType
}

/**
 * Items a single-shape draft can become a representation of: other batch drafts and unpublished
 * collection wearables that are missing exactly the draft's body shape and are the same kind of
 * wearable (a PNG can only pair with eyes/eyebrows/mouth, a model only with model categories).
 */
export function getVariantTargets(
  draft: ItemDraft,
  drafts: ItemDraft[],
  collectionItems: Item[]
): VariantTargetOption[] {
  if (draft.bodyShape === BodyShapeType.BOTH) return []
  const targetShape = draft.bodyShape === BodyShapeType.MALE ? BodyShapeType.FEMALE : BodyShapeType.MALE
  const isImage = isImageWearable(draft)

  const draftTargets = drafts
    .filter(
      candidate =>
        candidate.id !== draft.id &&
        candidate.status === 'ready' &&
        candidate.type === ItemType.WEARABLE &&
        !candidate.isVariant &&
        candidate.bodyShape === targetShape &&
        isImageWearable(candidate) === isImage &&
        candidate.name.trim().length > 0
    )
    .map(candidate => ({ id: candidate.id, label: candidate.name, bodyShape: candidate.bodyShape }))

  const itemTargets = collectionItems
    .filter(
      item =>
        item.type === ItemType.WEARABLE &&
        !item.isPublished &&
        getMissingBodyShapeType(item) === draft.bodyShape &&
        isImageCategory(item.data.category) === isImage
    )
    .map(item => ({ id: item.id, label: item.name, bodyShape: targetShape }))

  return [...draftTargets, ...itemTargets]
}

function isImageCategory(category: string | undefined): boolean {
  return category !== undefined && IMAGE_WEARABLE_CATEGORIES.includes(category)
}

/** Wearable made of a plain PNG (eyes / eyebrows / mouth) rather than a 3D model. */
export function isImageWearable(draft: ItemDraft): boolean {
  return draft.type !== ItemType.EMOTE && !Object.keys(draft.contents).some(isModelFile)
}

/** Category choices for the draft: model vs image wearable lists, or emote categories. */
export function getCategoryOptions(draft: ItemDraft): string[] {
  if (draft.type === ItemType.EMOTE) {
    return EmoteCategory.schema.enum as string[]
  }
  if (isImageWearable(draft)) return IMAGE_WEARABLE_CATEGORIES
  return (WearableCategory.schema.enum as string[]).filter(
    category => category !== (WearableCategory.BODY_SHAPE as string) && !IMAGE_WEARABLE_CATEGORIES.includes(category)
  )
}

export type DraftRules = {
  /** Linked collections: no rarity, and a token mapping is required when the collection has a linked contract. */
  linked?: boolean
  requiresMapping?: boolean
}

/**
 * The size limit of the item a draft will be saved as. A body-shape variant joins its target, so the target's
 * category decides (9 MB for a skin, 3 MB otherwise).
 */
export function getDraftSizeLimit(draft: ItemDraft, drafts: ItemDraft[], collectionItems: Item[]): number {
  if (!draft.isVariant) return getItemSizeLimit(draft.type, draft.category)
  const batchTarget = drafts.find(candidate => candidate.id === draft.variantTargetId)
  const category = batchTarget
    ? batchTarget.category
    : collectionItems.find(item => item.id === draft.variantTargetId)?.data.category
  return getItemSizeLimit(draft.type, category)
}

/**
 * What a draft's size depends on, across the batch (a variant is saved inside its target): the files, the rarity its
 * image is rendered for and how the drafts pair up. Names, descriptions and tags never measure again.
 */
export function draftSizeInputs(drafts: ItemDraft[]): unknown[] {
  return drafts.flatMap(draft => [
    draft.id,
    draft.status,
    draft.type,
    draft.bodyShape,
    draft.rarity,
    draft.model,
    draft.isVariant,
    draft.variantTargetId,
    draft.contents
  ])
}

/**
 * Whether the draft satisfies every requirement to be saved (mirrors legacy validateItemDraft). `size` is the item
 * the draft will be saved as, measured the catalyst way (lib/itemSize); unknown, it blocks nothing, since the upload
 * measures again.
 */
export function isDraftComplete(
  draft: ItemDraft,
  drafts: ItemDraft[],
  collectionItems: Item[],
  rules: DraftRules = {},
  size: ItemSize | null = null
): boolean {
  if (draft.status !== 'ready' || !draft.type || !draft.thumbnail || !draft.metrics) return false
  if (size && isOverSizeLimit(size)) return false
  // Legacy UploadVideoStep made the preview video mandatory for smart wearables.
  if (draft.isSmart && !draft.contents[VIDEO_PATH]) return false

  if (draft.isVariant) {
    if (!draft.variantTargetId) return false
    return getVariantTargets(draft, drafts, collectionItems).some(target => target.id === draft.variantTargetId)
  }

  if (!isValidItemName(draft.name)) return false
  if (!draft.category) return false
  if (rules.linked) return !rules.requiresMapping || draft.mapping !== null
  return !!draft.rarity
}
