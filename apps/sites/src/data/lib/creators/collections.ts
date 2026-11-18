// Collection domain model, ported from the legacy builder (src/modules/collection).
import type { CurationRequestStatus } from '~/lib/curation'

export type Collection = {
  id: string
  name: string
  owner: string
  contractAddress?: string
  urn: string
  salt?: string
  isPublished: boolean
  isApproved: boolean
  itemCount: number
  minters: string[]
  managers: string[]
  forumLink?: string
  lock?: number
  reviewedAt?: number
  linkedContractAddress?: string
  linkedContractNetwork?: string
  createdAt: number
  updatedAt: number
  isMappingComplete?: boolean
  isProgrammatic?: boolean
  /** Status of the latest curation, when the collection comes from a list. */
  curationStatus?: CurationStatus | null
  /** Latest change to the collection, its items or its curation, when the collection comes from a list. */
  lastActivityAt?: number
}

export enum CollectionType {
  STANDARD = 'standard',
  THIRD_PARTY = 'third_party'
}

export enum CollectionSort {
  MOST_RELEVANT = 'MOST_RELEVANT',
  NAME_DESC = 'NAME_DESC',
  NAME_ASC = 'NAME_ASC',
  CREATED_AT_DESC = 'CREATED_AT_DESC',
  CREATED_AT_ASC = 'CREATED_AT_ASC',
  UPDATED_AT_DESC = 'UPDATED_AT_DESC',
  UPDATED_AT_ASC = 'UPDATED_AT_ASC',
  LAST_ACTIVITY_DESC = 'LAST_ACTIVITY_DESC',
  LAST_ACTIVITY_ASC = 'LAST_ACTIVITY_ASC',
  REQUESTED_AT_DESC = 'REQUESTED_AT_DESC',
  REQUESTED_AT_ASC = 'REQUESTED_AT_ASC'
}

export enum CurationStatus {
  UNDER_REVIEW = 'under_review',
  TO_REVIEW = 'to_review',
  PENDING = 'pending',
  APPROVED = 'approved',
  REJECTED = 'rejected',
  DISABLED = 'disabled'
}

/** A server-provided link, or undefined unless it is http(s): it is opened as a page, so `javascript:` must not pass. */
export function toSafeLink(url: string | null): string | undefined {
  if (!url) return undefined
  try {
    return ['https:', 'http:'].includes(new URL(url).protocol) ? url : undefined
  } catch {
    return undefined
  }
}

/** The status a collection pill displays. */
export enum CollectionDisplayStatus {
  DRAFT = 'draft',
  PUBLISHING = 'publishing',
  UNDER_REVIEW = 'under_review',
  PUBLISHED = 'published',
  REJECTED = 'rejected',
  DISABLED = 'disabled',
  /** A linked (third-party) collection: read-only here, its items are curated one by one. */
  LINKED = 'linked'
}

/** The page's status filter chips, in display order. Values double as the `status` URL and API param. */
export enum CollectionStatusFilter {
  ALL = 'all',
  PUBLISHED = 'published',
  DRAFT = 'draft',
  UNDER_REVIEW = 'under_review',
  REJECTED = 'rejected',
  DISABLED = 'disabled'
}

export type CollectionStatusCounts = Record<Exclude<CollectionStatusFilter, CollectionStatusFilter.ALL>, number>

const OPTIONAL_STATUS_FILTERS = [
  CollectionStatusFilter.UNDER_REVIEW,
  CollectionStatusFilter.REJECTED,
  CollectionStatusFilter.DISABLED
]

/** Under review, rejected and disabled chips only show while some collection matches them. */
export function isStatusFilterShown(
  filter: CollectionStatusFilter,
  counts: CollectionStatusCounts | undefined
): boolean {
  if (!OPTIONAL_STATUS_FILTERS.includes(filter)) return true
  return !!counts?.[filter as keyof CollectionStatusCounts]
}

export type FetchCollectionsParams = {
  page?: number
  limit?: number
  q?: string
  contractAddress?: string
  type?: CollectionType
  sort?: CollectionSort
  status?: Exclude<CollectionStatusFilter, CollectionStatusFilter.ALL>
  isPublished?: boolean
}

export type PaginationStats = { total: number; limit: number; page: number; pages: number }
export type PaginatedResource<T> = { results: T[] } & PaginationStats
/** Paginated `/:address/collections` lists also count the collections in each status, search included. */
export type CollectionsList = PaginatedResource<Collection> & { counts?: CollectionStatusCounts }

const CONTRACT_ADDRESS = /^0x[0-9a-fA-F]{40}$/

export function isContractAddress(value: string): boolean {
  return CONTRACT_ADDRESS.test(value)
}

/** The collection deployed at `contractAddress`, whatever the casing of either side. */
export function findCollectionByContract(collections: Collection[], contractAddress: string): Collection | undefined {
  if (!isContractAddress(contractAddress)) return undefined
  const wanted = contractAddress.toLowerCase()
  return collections.find(collection => collection.contractAddress?.toLowerCase() === wanted)
}

/** The Linked chip: a type filter beside the status chips, sharing their `status` URL param. */
export const LINKED_FILTER = 'linked'

export type CollectionListFilter = CollectionStatusFilter | typeof LINKED_FILTER

/**
 * Extra fetch params each list chip translates to. All omits the type so standard and linked collections
 * page together; builder-server gives linked collections no status, so status chips never match them.
 */
export function listFilterToParams(filter: CollectionListFilter): Partial<FetchCollectionsParams> {
  if (filter === LINKED_FILTER) return { type: CollectionType.THIRD_PARTY }
  return filter === CollectionStatusFilter.ALL ? {} : { status: filter }
}

export type CollectionsSummary = { counts: CollectionStatusCounts; linkedTotal: number }

/**
 * A cached collections page without the collection just deleted, so the list stops rendering it (and its cover
 * stops asking builder-server for the previews of a collection that no longer exists) while the refetch is in
 * flight. Anything that is not a page holding that collection comes back as it was.
 */
export function withoutCollection<T>(data: T, collectionId: string): T {
  if (!isCollectionsPage(data)) return data
  const results = data.results.filter(collection => collection.id !== collectionId)
  if (results.length === data.results.length) return data
  // `total` and `counts` are left as they were: the refetch corrects both, and adjusting only one of them would
  // throw off the linked count `summarizeCollections` derives from the pair.
  return { ...data, results }
}

function isCollectionsPage(data: unknown): data is PaginatedResource<Collection> {
  return typeof data === 'object' && data !== null && Array.isArray((data as { results?: unknown }).results)
}

/**
 * The chip counts, from an unfiltered list: builder-server counts only standard collections by status
 * (linked ones have none), so the linked ones are the rest of the total.
 */
export function summarizeCollections(list: CollectionsList): CollectionsSummary | undefined {
  if (!list.counts) return undefined
  const counts = list.counts
  const standardTotal = counts.published + counts.draft + counts.under_review + counts.rejected + counts.disabled
  return { counts, linkedTotal: Math.max(0, list.total - standardTotal) }
}

/** builder-server omits `third_party_id`, so the URN (`urn:decentraland:{network}:collections-thirdparty:…`) is the tell. */
export function isLinkedCollection(collection: Pick<Collection, 'urn'>): boolean {
  return collection.urn?.split(':')[3] === 'collections-thirdparty'
}

/** Mirrors builder-server's `/:address/collections` status filter, so a pill always matches its chip. */
export function getCollectionDisplayStatus(
  collection: Collection,
  curationStatus: CurationStatus | CurationRequestStatus | null | undefined = collection.curationStatus
): CollectionDisplayStatus {
  if (isLinkedCollection(collection)) return CollectionDisplayStatus.LINKED
  // A locked draft has its publish transaction in flight: the server just hasn't seen it yet, so it stays
  // under the Draft chip while its pill already says Publishing.
  if (!collection.isPublished) {
    return isCollectionLocked(collection) ? CollectionDisplayStatus.PUBLISHING : CollectionDisplayStatus.DRAFT
  }
  if (curationStatus === CurationStatus.REJECTED) return CollectionDisplayStatus.REJECTED
  // reviewedAt also moves on rescueItems, so a pending curation means a first approval in progress, not a disable.
  if (!collection.isApproved) {
    const isDisabled =
      curationStatus === CurationStatus.APPROVED || (curationStatus == null && hasBeenApproved(collection))
    return isDisabled ? CollectionDisplayStatus.DISABLED : CollectionDisplayStatus.UNDER_REVIEW
  }
  return curationStatus === CurationStatus.PENDING
    ? CollectionDisplayStatus.UNDER_REVIEW
    : CollectionDisplayStatus.PUBLISHED
}

/**
 * Published and approved at least once: the collection is on the market even while a later change of
 * its items is being reviewed again. The contract stamps `reviewedAt` with `createdAt` on creation,
 * so only a later review counts.
 */
export function hasBeenApproved(collection: Collection): boolean {
  if (!collection.isPublished) return false
  if (collection.isApproved) return true
  return collection.reviewedAt !== undefined && collection.reviewedAt !== collection.createdAt
}

export enum CollectionRole {
  COLLABORATOR = 'collaborator',
  MINTER = 'minter'
}

/** The non-owner role that gives this address access to the collection, if any. Collaborator wins over minter. */
export function getCollectionRole(collection: Collection, address: string): CollectionRole | null {
  const target = address.toLowerCase()
  if (collection.owner.toLowerCase() === target) return null
  if (collection.managers.some(manager => manager.toLowerCase() === target)) return CollectionRole.COLLABORATOR
  if (collection.minters.some(minter => minter.toLowerCase() === target)) return CollectionRole.MINTER
  return null
}

// Same limit as the legacy builder's standard collections (the server schema allows 42, but the
// legacy UI caps standard collections at 32 and we keep that contract).
export const COLLECTION_NAME_MAX_LENGTH = 32

// The exact error string builder-server answers when the (globally unique, case-insensitive)
// collection name is taken — the legacy front end string-matches it too.
export const NAME_ALREADY_IN_USE_ERROR = 'Name already in use'

export type CollectionNameError = 'empty' | 'too_long' | 'invalid_character'

// Same rules the legacy builder enforces: non-empty, ≤32 chars, and no ':' (names feed URNs).
export function validateCollectionName(name: string): CollectionNameError | null {
  const trimmed = name.trim()
  if (!trimmed) return 'empty'
  if (trimmed.length > COLLECTION_NAME_MAX_LENGTH) return 'too_long'
  if (trimmed.includes(':')) return 'invalid_character'
  return null
}

/** Whether the collection is under the one-day publish lock (legacy `isLocked`). */
export function isCollectionLocked(collection: Collection, now = Date.now()): boolean {
  if (!collection.lock || collection.isPublished) return false
  const DAY = 24 * 60 * 60 * 1000
  return collection.lock + DAY > now
}

/** Whether this address is the owner, a collaborator or a minter of the collection. */
export function hasCollectionRole(collection: Collection, address: string | undefined): boolean {
  if (!address) return false
  return collection.owner.toLowerCase() === address.toLowerCase() || getCollectionRole(collection, address) !== null
}

/**
 * Whether the item editor opens this collection: published collections are public there, read-only to any signer as
 * builder-server serves them, while drafts stay private to the addresses with a role. The detail page is roles-only.
 */
export function canViewCollection(collection: Collection, address: string | undefined): boolean {
  return !!address && (collection.isPublished || hasCollectionRole(collection, address))
}

/** Owners and collaborators (managers) may change a collection's items; minters only sell them. */
export function canManageCollectionItems(collection: Collection, address: string | undefined): boolean {
  if (!address) return false
  const isOwner = collection.owner.toLowerCase() === address.toLowerCase()
  return isOwner || getCollectionRole(collection, address) === CollectionRole.COLLABORATOR
}

/**
 * Only the collection's creator may sell its items: the off-chain marketplace rejects a primary order
 * whose signer is not the creator (`NotCreator`), and enabling sales edits the contract's minters,
 * which is creator-only as well.
 */
export function canSellCollectionItems(collection: Collection, address: string | undefined): boolean {
  if (!address) return false
  return collection.owner.toLowerCase() === address.toLowerCase()
}
