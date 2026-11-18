// Curation domain: the committee's review requests on standard collections, ported from the legacy
// builder (modules/curations, modules/committee, CurationPage) against the same builder-server API.
import { CollectionSort, hasBeenApproved, type Collection } from '@/data/lib/creators/collections'
import { isFinalValidationError, latestStageEvent, type CollectionEvent, type RejectReasonCode } from '@/data/lib/creators/events'

export type CurationRequestStatus = 'pending' | 'approved' | 'rejected'

/** `reviewed_by` value when the automated review, not a committee member, decided. */
export const VALIDATOR_REVIEWER = 'validator'

export type CollectionCuration = {
  id: string
  collectionId: string
  status: CurationRequestStatus
  assignee: string | null
  /** `'validator'`, the deciding curator's lowercase address, or `null` while pending (and on legacy rows). */
  reviewedBy: string | null
  rejectionReasons: RejectReasonCode[] | null
  rejectionMessage: string | null
  createdAt: number
  updatedAt: number
}

export enum ReviewStage {
  AUTOMATED_REVIEWING = 'automated_reviewing',
  AWAITING_CURATOR = 'awaiting_curator',
  /** The validator did not pass it (skipped, could not check, or gave up): a curator decides. */
  NEEDS_CURATOR = 'needs_curator',
  REJECTED_BY_VALIDATOR = 'rejected_by_validator',
  APPEALED = 'appealed',
  REJECTED_BY_CURATOR = 'rejected_by_curator',
  APPROVED = 'approved'
}

/**
 * Where the review stands, from the latest request and the newest stage-defining timeline event (the frozen
 * curation contract's table). `events` newest first; `null` while the timeline is unavailable (feature off,
 * not loaded, or a server without it), which keeps every surface on the legacy curation state. Never claims
 * more than the chain does: an approved request on a collection that is not approved on chain is disabled.
 */
export function getReviewStage(
  collection: Collection,
  curation: CollectionCuration | null,
  events: CollectionEvent[] | null
): ReviewStage | null {
  if (!curation || !events) return null
  if (curation.status === 'approved') return collection.isApproved ? ReviewStage.APPROVED : null
  if (curation.status === 'rejected') {
    if (curation.reviewedBy === VALIDATOR_REVIEWER) return ReviewStage.REJECTED_BY_VALIDATOR
    return curation.reviewedBy ? ReviewStage.REJECTED_BY_CURATOR : null
  }
  const latest = latestStageEvent(events)
  switch (latest?.type) {
    case 'review.automated_started':
      return ReviewStage.AUTOMATED_REVIEWING
    case 'review.automated_error':
      return isFinalValidationError(latest) ? ReviewStage.NEEDS_CURATOR : ReviewStage.AUTOMATED_REVIEWING
    case 'review.automated_passed':
      return ReviewStage.AWAITING_CURATOR
    case 'review.human_required':
      return ReviewStage.NEEDS_CURATOR
    case 'review.appeal_requested':
      return ReviewStage.APPEALED
    default:
      return null
  }
}

/** A review is open or was sent back, so the creator has a curator to write to; an approved one is closed. */
export function hasReviewConversation(curation: CollectionCuration | null): boolean {
  return curation?.status === 'pending' || curation?.status === 'rejected'
}

/** Stages in which the creator may ask for another automated review or a human one. */
export function isRejectedStage(stage: ReviewStage | null): boolean {
  return stage === ReviewStage.REJECTED_BY_VALIDATOR || stage === ReviewStage.REJECTED_BY_CURATOR
}

export function isCommitteeMember(members: string[] | undefined, address: string | undefined): boolean {
  if (!members || !address) return false
  const target = address.toLowerCase()
  return members.some(member => member.toLowerCase() === target)
}

/** A pending request is "to review" whether or not a curator has it: the assignee says who, the state doesn't. */
export enum CurationState {
  TO_REVIEW = 'to_review',
  /** Pending, but the automated review is still running: not a curator's turn yet. */
  AUTO_REVIEWING = 'auto_reviewing',
  APPROVED = 'approved',
  REJECTED = 'rejected',
  DISABLED = 'disabled'
}

/**
 * Disabled by the committee, as builder-server's collection status reads it: not approved on chain, with an approved
 * latest request, or with none but reviewed before (approved before requests existed). A rejected request wins, as it
 * does for the creator; `reviewedAt` alone isn't proof, since the rescue step stamps it on a first approval too.
 */
function isDisabled(collection: Collection, curation: CollectionCuration | null): boolean {
  if (collection.isApproved) return false
  return curation ? curation.status === 'approved' : hasBeenApproved(collection)
}

/** What the committee sees for a collection, from its on-chain approval and its latest review request. */
export function getCurationState(collection: Collection, curation: CollectionCuration | null): CurationState {
  if (collection.isApproved) {
    if (!curation || curation.status === 'approved') return CurationState.APPROVED
    if (curation.status === 'rejected') return CurationState.REJECTED
  } else {
    if (isDisabled(collection, curation)) return CurationState.DISABLED
    if (curation?.status === 'rejected') return CurationState.REJECTED
  }
  return CurationState.TO_REVIEW
}

/** The curator-facing state: the curation state, except a pending request still in its automated review. */
export function getCuratorState(
  collection: Collection,
  curation: CollectionCuration | null,
  stage: ReviewStage | null
): CurationState {
  return stage === ReviewStage.AUTOMATED_REVIEWING
    ? CurationState.AUTO_REVIEWING
    : getCurationState(collection, curation)
}

export enum ReviewAction {
  APPROVE = 'approve',
  REJECT = 'reject',
  ENABLE = 'enable',
  DISABLE = 'disable',
  DEPLOY_MISSING = 'deploy_missing'
}

/** The review bar's buttons, in display order (legacy TopPanel.renderButtons). */
export function getReviewActions(
  collection: Collection,
  curation: CollectionCuration | null,
  hasMissingEntities: boolean
): ReviewAction[] {
  const disable = hasMissingEntities ? [ReviewAction.DISABLE, ReviewAction.DEPLOY_MISSING] : [ReviewAction.DISABLE]
  if (collection.isApproved) {
    return curation?.status === 'pending' ? [ReviewAction.APPROVE, ReviewAction.REJECT] : disable
  }
  if (isDisabled(collection, curation)) return [ReviewAction.ENABLE]
  // A rejected first review can still be approved later, like in the legacy builder.
  return curation?.status === 'rejected' ? [ReviewAction.APPROVE] : [ReviewAction.APPROVE, ReviewAction.REJECT]
}

/**
 * The pencil on an assignee: gone once the collection is published (approved on chain with an approved request or
 * none at all), and on a disabled collection with no request. Assigning there would open a request, putting it back
 * under review, and Enable never opens one.
 */
export function canEditAssignee(collection: Collection, curation: CollectionCuration | null): boolean {
  if (!curation) return !collection.isApproved && !hasBeenApproved(collection)
  return !(collection.isApproved && curation.status === 'approved')
}

/** The status picker's options: one "to review" covers a pending request whether or not a curator has it. */
export enum CurationStatusFilter {
  TO_REVIEW = 'to_review',
  AUTO_REVIEWING = 'auto_reviewing',
  APPROVED = 'approved',
  REJECTED = 'rejected',
  DISABLED = 'disabled'
}

/** The status picker's options, in display order; nothing picked means every status. */
export const STATUS_FILTERS = Object.values(CurationStatusFilter)
export const DEFAULT_STATUS_FILTERS = [CurationStatusFilter.TO_REVIEW]

/** What a review is for: a collection never approved on chain, or changes to one that was (or is). */
export enum ReviewType {
  NEW = 'new',
  UPDATES = 'updates'
}

export function getReviewType(collection: Collection): ReviewType {
  // Same rule as builder-server's review_type filter, so the pill and the chips agree. It reads a first approval
  // stalled after the rescue step (reviewedAt moved, never approved) as updates, on both sides.
  return hasBeenApproved(collection) ? ReviewType.UPDATES : ReviewType.NEW
}

/** Assignee picker value for the collections without a curator; builder-server takes it as is. */
export const UNASSIGNED = 'unassigned'

/** What the assignee picker opens on: no curator, plus the signed-in one. */
export function defaultAssignees(address?: string): string[] {
  return address ? [UNASSIGNED, address.toLowerCase()] : [UNASSIGNED]
}

// Only sorts builder-server implements: an unknown one drops the ORDER BY and paginates at random.
export const CURATION_SORTS = [
  CollectionSort.REQUESTED_AT_DESC,
  CollectionSort.REQUESTED_AT_ASC,
  CollectionSort.MOST_RELEVANT,
  CollectionSort.CREATED_AT_DESC,
  CollectionSort.NAME_ASC,
  CollectionSort.NAME_DESC
] as const
export type CurationSort = (typeof CURATION_SORTS)[number]

/** URL value that clears a filter with defaults, as opposed to leaving it out. */
const NONE = 'all'
const ADDRESS = /^0x[0-9a-f]{40}$/i
export const CURATION_PAGE_SIZE = 12

export enum CurationTypeFilter {
  ALL = 'all',
  STANDARD = 'standard',
  LINKED = 'linked'
}

export type CurationFilters = {
  page: number
  search: string
  /** Any of them matches; empty means every status. */
  statuses: CurationStatusFilter[]
  /** Curator addresses and/or `UNASSIGNED`; empty means everyone. */
  assignees: string[]
  reviewType: ReviewType | null
  sort: CurationSort
  tag: string | null
  type: CurationTypeFilter
}

/**
 * The page's filters as they live in the URL; unknown values fall back to the defaults. A missing `status` or
 * `assignee` means the defaults (to review, unassigned and the signed-in curator), while `all` means the filter
 * was cleared on purpose.
 */
export function parseCurationFilters(params: URLSearchParams, address?: string): CurationFilters {
  const sort = params.get('sort') as CurationSort
  const reviewType = params.get('review_type') as ReviewType
  const type = params.get('type') as CurationTypeFilter
  return {
    page: Math.max(1, Number(params.get('page')) || 1),
    search: params.get('q') ?? '',
    statuses: parseMulti(params, 'status', DEFAULT_STATUS_FILTERS, value =>
      STATUS_FILTERS.includes(value as CurationStatusFilter) ? (value as CurationStatusFilter) : null
    ),
    assignees: parseMulti(params, 'assignee', defaultAssignees(address), value =>
      value === UNASSIGNED ? value : ADDRESS.test(value) ? value.toLowerCase() : null
    ),
    reviewType: Object.values(ReviewType).includes(reviewType) ? reviewType : null,
    sort: CURATION_SORTS.includes(sort) ? sort : CollectionSort.REQUESTED_AT_DESC,
    tag: params.get('tag') || null,
    type: Object.values(CurationTypeFilter).includes(type) ? type : CurationTypeFilter.ALL
  }
}

function parseMulti<T extends string>(
  params: URLSearchParams,
  key: string,
  defaults: T[],
  parse: (value: string) => T | null
): T[] {
  const values = params.getAll(key)
  if (values.includes(NONE)) return []
  const known = Array.from(new Set(values.map(parse).filter((value): value is T => value !== null)))
  return known.length ? known : defaults
}

/** The URL params for a multi-value filter: each value, `all` for an empty one, or nothing for the defaults. */
export function toMultiParam(values: string[], defaults: string[]): string[] | null {
  if (values.length === defaults.length && values.every(value => defaults.includes(value))) return null
  return values.length ? values : [NONE]
}

/** Committee members for the assignee pickers: the signed-in curator first, the rest as the server lists them. */
export function orderCurators(members: string[], address: string | undefined): string[] {
  const self = address?.toLowerCase()
  const lower = members.map(member => member.toLowerCase())
  return self && lower.includes(self) ? [self, ...lower.filter(member => member !== self)] : lower
}

/**
 * Owners and collaborators may ask the committee for another look: an approved collection once its items
 * are unsynced, or one that isn't approved on chain and reads as rejected (a rejected first review, or a
 * collection disabled after its changes were rejected); its items always read as under review, so there is
 * no sync signal to wait for.
 */
export function canPushChanges(
  collection: Collection,
  curation: CollectionCuration | null,
  hasUnsyncedItems: boolean,
  canManage: boolean
): boolean {
  if (!collection.isPublished || !canManage || curation?.status === 'pending') return false
  return collection.isApproved ? hasUnsyncedItems : curation?.status === 'rejected'
}

const LIST_SEARCH_KEY = 'wemotes-builder.curation-search'

/** Remembers the list's filters so the review bar's back link returns to the same view. */
export function rememberCurationSearch(search: string): void {
  try {
    window.sessionStorage.setItem(LIST_SEARCH_KEY, search)
  } catch {
    // Storage blocked: the back link falls back to the unfiltered list.
  }
}

export function curationListUrl(): string {
  try {
    return `/curation${window.sessionStorage.getItem(LIST_SEARCH_KEY) ?? ''}`
  } catch {
    return '/curation'
  }
}
