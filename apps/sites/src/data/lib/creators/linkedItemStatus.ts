// Per-item state of linked collections, ported from the legacy builder (item/selectors getStatusForTP,
// item/utils isAllowedToPushChanges, ItemStatusBadge). Linked items are curated one by one: their latest
// item curation, not the collection, says where they stand.
import { CurationStatus } from './collections'
import { type Item } from './items'

export type ItemCuration = {
  id: string
  itemId: string
  status: CurationStatus
  /** The builder copy's content hash when the curation was opened. */
  contentHash: string
  isMappingComplete: boolean | null
  createdAt: number
  updatedAt: number
}

export enum LinkedItemStatus {
  /** Never sent for review (or its last request was rejected). */
  READY_TO_PUBLISH = 'ready_to_publish',
  UNDER_REVIEW = 'under_review',
  REJECTED = 'rejected',
  /** Approved and in the Catalyst as last reviewed. */
  PUBLISHED = 'published',
  /** Approved, then edited: the changes wait to be sent for review. */
  READY_TO_PUSH = 'ready_to_push',
  /** The item has no token mapping yet, so it can't be published. */
  PENDING_MAPPING = 'pending_mapping',
  /** Published before mappings existed: its mapping has not been reviewed yet. */
  PENDING_MIGRATION = 'pending_migration'
}

/** Whether the item has ever been sent for review: builder-server locks URN edits and deletes from then on. */
export function isLinkedItemPublished(item: Item, curation: ItemCuration | null | undefined): boolean {
  return item.isPublished || !!curation
}

/** The row's status; `curation` is the item's latest item curation, null when it has none. */
export function getLinkedItemStatus(item: Item, curation: ItemCuration | null | undefined): LinkedItemStatus {
  if (!item.mappings) return LinkedItemStatus.PENDING_MAPPING
  if (item.isMappingComplete === false) return LinkedItemStatus.PENDING_MIGRATION
  if (!curation) return LinkedItemStatus.READY_TO_PUBLISH
  if (curation.status === CurationStatus.PENDING) return LinkedItemStatus.UNDER_REVIEW
  if (curation.status === CurationStatus.REJECTED) return LinkedItemStatus.REJECTED
  if (curation.status === CurationStatus.APPROVED) {
    return item.currentContentHash !== item.catalystContentHash
      ? LinkedItemStatus.READY_TO_PUSH
      : LinkedItemStatus.PUBLISHED
  }
  return LinkedItemStatus.READY_TO_PUBLISH
}

/** Items that go through the publish flow (a cheque and, when slots are missing, a fee). */
export function canPublishLinkedItem(status: LinkedItemStatus): boolean {
  return status === LinkedItemStatus.READY_TO_PUBLISH || status === LinkedItemStatus.REJECTED
}

/**
 * Items whose changes can be sent for review again without a cheque (legacy isAllowedToPushChanges): approved
 * ones whose builder copy no longer matches what curation last saw.
 */
export function canPushLinkedItem(item: Item, curation: ItemCuration | null | undefined): boolean {
  if (!item.isApproved || !curation) return false
  if (curation.status === CurationStatus.REJECTED) return false
  // Already sent: the pending request holds this very content.
  if (curation.status === CurationStatus.PENDING && curation.contentHash === item.currentContentHash) return false
  if (item.currentContentHash !== item.catalystContentHash) return true
  return curation.contentHash !== item.currentContentHash
}

export type LinkedPublishAction = 'publish' | 'push' | 'publish_and_push'

export type LinkedSelection = { toPublish: Item[]; toPush: Item[] }

/** Splits the selected items into the ones to publish and the ones to push, as the legacy publish button does. */
export function splitLinkedSelection(selected: Iterable<Item>, curations: Map<string, ItemCuration>): LinkedSelection {
  const toPublish: Item[] = []
  const toPush: Item[] = []
  for (const item of selected) {
    const curation = curations.get(item.id) ?? null
    if (canPublishLinkedItem(getLinkedItemStatus(item, curation))) toPublish.push(item)
    else if (canPushLinkedItem(item, curation)) toPush.push(item)
  }
  return { toPublish, toPush }
}

export function getLinkedPublishAction(selection: LinkedSelection): LinkedPublishAction | null {
  if (selection.toPublish.length > 0 && selection.toPush.length > 0) return 'publish_and_push'
  if (selection.toPublish.length > 0) return 'publish'
  if (selection.toPush.length > 0) return 'push'
  return null
}

/** Whether a row can be selected for the publish button at all. */
export function isLinkedItemSelectable(item: Item, curation: ItemCuration | null | undefined): boolean {
  const status = getLinkedItemStatus(item, curation)
  return canPublishLinkedItem(status) || canPushLinkedItem(item, curation)
}

/** The filters of the linked collection's item list, mapped to builder-server's query params. */
export enum LinkedItemFilter {
  ALL = 'all',
  /** Never submitted for review, or rejected: picked client-side, as builder-server can't filter it. */
  DRAFT = 'draft',
  UNDER_REVIEW = 'under_review',
  PUBLISHED = 'published',
  READY_TO_PUSH = 'ready_to_push',
  PENDING_MAPPING = 'pending_mapping',
  PENDING_MIGRATION = 'pending_migration'
}

export type LinkedItemsQuery = {
  status?: CurationStatus
  synced?: boolean
  mappingStatus?: 'missing_mapping' | 'unpublished_mapping'
}

export function linkedItemFilterToQuery(filter: LinkedItemFilter): LinkedItemsQuery {
  switch (filter) {
    case LinkedItemFilter.UNDER_REVIEW:
      return { status: CurationStatus.PENDING }
    case LinkedItemFilter.PUBLISHED:
      return { synced: true }
    case LinkedItemFilter.READY_TO_PUSH:
      return { synced: false }
    case LinkedItemFilter.PENDING_MAPPING:
      return { mappingStatus: 'missing_mapping' }
    case LinkedItemFilter.PENDING_MIGRATION:
      return { mappingStatus: 'unpublished_mapping' }
    default:
      return {}
  }
}

/** A draft: an item never submitted for review, or one a curator rejected. */
export function isLinkedDraft(curation: ItemCuration | null | undefined): boolean {
  return !curation || curation.status === CurationStatus.REJECTED
}

export function parseLinkedItemFilter(raw: string | null): LinkedItemFilter {
  return Object.values(LinkedItemFilter).includes(raw as LinkedItemFilter)
    ? (raw as LinkedItemFilter)
    : LinkedItemFilter.ALL
}

export type LinkedWorkCounts = {
  /** Items in the collection. */
  itemCount: number
  /** Latest curation of every item ever submitted. */
  curations: Map<string, ItemCuration>
  /** Approved items whose content changed since (the server's `synced=false`). */
  unsynced: number
  /** Ids of the items with no token mapping, which can't be published yet. */
  missingMappingIds: string[]
}

/** Whether anything in the collection, on any page, can be published or pushed. */
export function hasLinkedWork({ itemCount, curations, unsynced, missingMappingIds }: LinkedWorkCounts): boolean {
  if (unsynced > 0) return true
  const rejected = [...curations.values()].filter(curation => curation.status === CurationStatus.REJECTED).length
  // Items published before mappings existed are curated already: only the never-submitted ones count here.
  const unsubmittedWithoutMapping = missingMappingIds.filter(id => !curations.has(id)).length
  return rejected > 0 || itemCount - curations.size - unsubmittedWithoutMapping > 0
}
