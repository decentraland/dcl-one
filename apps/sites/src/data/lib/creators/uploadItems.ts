// Turns reviewed drafts into an ordered upload plan and executes it against builder-server.
// Variants merge into their target (batch drafts client-side, existing items via representation
// append) so a base item is never uploaded after a variant that depends on it.
import {
  ALREADY_PUBLISHED_STATUS,
  BuilderServerError,
  COLLECTION_LOCKED_STATUS,
  fetchContentSize,
  saveItem
} from './builder'
import { ItemFileError, THUMBNAIL_PATH } from './itemFiles'
import {
  addRepresentationToItem,
  assertThumbnailSize,
  buildItem,
  type BuiltItem,
  type ItemDraftPayload
} from './itemFactory'
import { assertItemSize, type MeasureDeps } from './itemSize'
import { type Item } from './items'

export type UploadDraft = ItemDraftPayload & {
  /** When set, this draft is a body-shape variant of another batch draft or an existing item. */
  variantTargetId?: string
}

/** One PUT+files request; carries every draft id it satisfies so failures map back to drafts. */
export type UploadOperation = {
  built: BuiltItem
  draftIds: string[]
  /** True when this operation updates an existing collection item instead of creating one. */
  isExistingItemUpdate: boolean
}

export type UploadFailureReason = 'locked' | 'published' | 'urn_taken' | 'duplicate_urn' | 'too_big' | 'generic'

export type UploadResult = {
  savedDraftIds: string[]
  failedDraftIds: string[]
  failureReason: UploadFailureReason | null
}

/** Drafts whose linked URN an earlier draft of the batch already takes: uploading both would overwrite or fail one. */
export function findDuplicateUrns(drafts: UploadDraft[]): string[] {
  const seen = new Set<string>()
  return drafts.flatMap(draft => {
    if (!draft.urn || draft.variantTargetId) return []
    if (seen.has(draft.urn)) return [draft.id]
    seen.add(draft.urn)
    return []
  })
}

/**
 * Builds the ordered operation list: base items first (batch variants merged into them), then
 * representation appends onto existing collection items. `replaceByUrn` (linked edit in bulk) turns a draft with an
 * existing item's URN into an update of that item; otherwise it stays a new item, which the server refuses.
 */
export async function planUpload(
  drafts: UploadDraft[],
  existingItems: Item[],
  { replaceByUrn = false }: { replaceByUrn?: boolean } = {}
): Promise<UploadOperation[]> {
  const operations: UploadOperation[] = []
  const builtByDraftId = new Map<string, UploadOperation>()

  for (const draft of drafts.filter(candidate => !candidate.variantTargetId)) {
    const built = await buildItem(draft)
    // A linked item re-uploaded under its own URN replaces the existing item (legacy "edit in bulk").
    const existing = replaceByUrn && draft.urn ? existingItems.find(item => item.urn === draft.urn) : undefined
    const operation: UploadOperation = {
      built: existing
        ? { ...built, item: { ...built.item, id: existing.id, owner: existing.owner, createdAt: existing.createdAt } }
        : built,
      draftIds: [draft.id],
      isExistingItemUpdate: !!existing
    }
    operations.push(operation)
    builtByDraftId.set(draft.id, operation)
  }

  for (const draft of drafts.filter(candidate => !!candidate.variantTargetId)) {
    const targetOperation = builtByDraftId.get(draft.variantTargetId!)
    if (targetOperation) {
      // Variant of another draft in this batch: merge into that item before it uploads.
      targetOperation.built = await addRepresentationToItem(targetOperation.built, draft)
      targetOperation.draftIds.push(draft.id)
      continue
    }

    const existingItem = existingItems.find(item => item.id === draft.variantTargetId)
    if (!existingItem) {
      throw new Error(`Variant target "${draft.variantTargetId!}" is not in the batch or the collection`)
    }
    const built = await addRepresentationToItem({ item: existingItem }, draft)
    // Never replace an existing item's thumbnail when appending a representation.
    delete built.blobs[THUMBNAIL_PATH]
    operations.push({ built, draftIds: [draft.id], isExistingItemUpdate: true })
  }

  return operations
}

/**
 * The operation a draft will be saved in, built exactly as the upload builds it (thumbnail quantized, image.png
 * rendered, a variant merged into its target), so its size can be shown before saving. Only the drafts that share
 * that operation are built: the draft, the batch draft it is a variant of, and that draft's other variants.
 */
export async function planDraftUpload(
  draftId: string,
  drafts: UploadDraft[],
  existingItems: Item[]
): Promise<UploadOperation | null> {
  const draft = drafts.find(candidate => candidate.id === draftId)
  if (!draft) return null
  const baseId =
    draft.variantTargetId && drafts.some(c => c.id === draft.variantTargetId) ? draft.variantTargetId : null
  const group = baseId
    ? drafts.filter(candidate => candidate.id === baseId || candidate.variantTargetId === baseId)
    : [draft, ...drafts.filter(candidate => candidate.variantTargetId === draft.id)]
  const operations = await planUpload(group, existingItems)
  return operations.find(operation => operation.draftIds.includes(draftId)) ?? null
}

const URN_TAKEN_PATTERN = /belong to another item/i

function toFailureReason(error: unknown): UploadFailureReason {
  if (error instanceof BuilderServerError) {
    if (error.status === COLLECTION_LOCKED_STATUS) return 'locked'
    // builder-server answers a URN another item holds (URNAlreadyInUseError) with the same status as a published
    // collection; only its message tells them apart, and it sends no error code.
    if (error.status === ALREADY_PUBLISHED_STATUS) {
      return URN_TAKEN_PATTERN.test(error.message) ? 'urn_taken' : 'published'
    }
  }
  if (error instanceof ItemFileError) return 'too_big'
  return 'generic'
}

// Only a collection-level refusal makes the remaining operations pointless.
const ABORTING_REASONS: UploadFailureReason[] = ['locked', 'published']

/**
 * Re-checks the caps right before the PUT (legacy sagas): the thumbnail's own, then the item's whole size as the
 * catalyst counts it (lib/itemSize). An update onto an existing item counts the stored files it keeps, read by hash.
 */
async function checkOperationSize(operation: UploadOperation, measure: MeasureDeps): Promise<void> {
  assertThumbnailSize(operation.built.blobs)
  await assertItemSize(operation.built, measure)
}

/**
 * Uploads the operations sequentially. A locked/published collection aborts the rest (retrying
 * can never succeed); any other failure moves on to the next operation.
 */
export async function executeUpload(
  address: string,
  operations: UploadOperation[],
  measure: MeasureDeps = { sizeOf: fetchContentSize }
): Promise<UploadResult> {
  const savedDraftIds: string[] = []
  const failedDraftIds: string[] = []
  let failureReason: UploadFailureReason | null = null

  for (let i = 0; i < operations.length; i++) {
    const operation = operations[i]
    try {
      await checkOperationSize(operation, measure)
      await saveItem(address, operation.built.item, operation.built.blobs)
      savedDraftIds.push(...operation.draftIds)
    } catch (error) {
      const reason = toFailureReason(error)
      failureReason = failureReason === null || reason !== 'generic' ? reason : failureReason
      failedDraftIds.push(...operation.draftIds)
      if (ABORTING_REASONS.includes(reason)) {
        for (const remaining of operations.slice(i + 1)) {
          failedDraftIds.push(...remaining.draftIds)
        }
        break
      }
    }
  }

  return { savedDraftIds, failedDraftIds, failureReason }
}
