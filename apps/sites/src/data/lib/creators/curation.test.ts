import { describe, expect, it } from 'vitest'
import { CollectionSort, type Collection } from './collections'
import { type CollectionEvent } from './events'
import {
  CurationState,
  CurationStatusFilter,
  CurationTypeFilter,
  ReviewAction,
  ReviewType,
  UNASSIGNED,
  ReviewStage,
  canEditAssignee,
  canPushChanges,
  curationListUrl,
  defaultAssignees,
  rememberCurationSearch,
  getCurationState,
  getCuratorState,
  getReviewActions,
  getReviewType,
  getReviewStage,
  hasReviewConversation,
  isCommitteeMember,
  orderCurators,
  parseCurationFilters,
  toMultiParam,
  type CollectionCuration
} from './curation'

const CREATED = 1_700_000_000_000

function collection(overrides: Partial<Collection> = {}): Collection {
  return {
    id: 'c1',
    name: 'Hats',
    owner: '0xowner',
    urn: 'urn',
    isPublished: true,
    isApproved: false,
    itemCount: 2,
    minters: [],
    managers: [],
    reviewedAt: CREATED,
    createdAt: CREATED,
    updatedAt: CREATED,
    ...overrides
  }
}

function curation(overrides: Partial<CollectionCuration> = {}): CollectionCuration {
  return {
    id: 'r1',
    collectionId: 'c1',
    status: 'pending',
    assignee: null,
    reviewedBy: null,
    rejectionReasons: null,
    rejectionMessage: null,
    createdAt: 1,
    updatedAt: 2,
    ...overrides
  }
}

const reviewedBefore = { reviewedAt: CREATED + 1000 }

describe('isCommitteeMember', () => {
  it('matches addresses case-insensitively and fails closed without data', () => {
    expect(isCommitteeMember(['0xabc'], '0xABC')).toBe(true)
    expect(isCommitteeMember(['0xabc'], '0xdef')).toBe(false)
    expect(isCommitteeMember(undefined, '0xabc')).toBe(false)
    expect(isCommitteeMember(['0xabc'], undefined)).toBe(false)
  })
})

describe('getCuratorState', () => {
  it('holds a pending request back while its automated review runs, and reads the curation state otherwise', () => {
    expect(getCuratorState(collection(), curation(), ReviewStage.AUTOMATED_REVIEWING)).toBe(
      CurationState.AUTO_REVIEWING
    )
    expect(getCuratorState(collection(), curation(), ReviewStage.AWAITING_CURATOR)).toBe(CurationState.TO_REVIEW)
    expect(getCuratorState(collection(), curation({ status: 'rejected' }), ReviewStage.REJECTED_BY_VALIDATOR)).toBe(
      CurationState.REJECTED
    )
  })
})

describe('getCurationState', () => {
  it.each([
    ['a new collection nobody picked up', collection(), null, CurationState.TO_REVIEW],
    ['a pending request without a curator', collection(), curation(), CurationState.TO_REVIEW],
    ['a pending request with a curator', collection(), curation({ assignee: '0xc' }), CurationState.TO_REVIEW],
    ['a rejected first review', collection(), curation({ status: 'rejected' }), CurationState.REJECTED],
    ['an approved collection', collection({ isApproved: true }), null, CurationState.APPROVED],
    [
      'an approved collection with an approved request',
      collection({ isApproved: true }),
      curation({ status: 'approved' }),
      CurationState.APPROVED
    ],
    [
      'rejected pushed changes',
      collection({ isApproved: true }),
      curation({ status: 'rejected' }),
      CurationState.REJECTED
    ],
    [
      'pushed changes waiting for review',
      collection({ isApproved: true }),
      curation({ assignee: '0xc' }),
      CurationState.TO_REVIEW
    ],
    ['a disabled collection', collection(reviewedBefore), null, CurationState.DISABLED],
    [
      'a collection disabled after its request was approved',
      collection(reviewedBefore),
      curation({ status: 'approved' }),
      CurationState.DISABLED
    ],
    [
      // Only an approved (or missing) request proves a past approval: the rescue step stamps reviewedAt too.
      'a collection disabled after its pushed changes were rejected, as the creator sees it',
      collection(reviewedBefore),
      curation({ status: 'rejected' }),
      CurationState.REJECTED
    ],
    [
      'a first review stopped after the rescue step',
      collection(reviewedBefore),
      curation({ status: 'pending', assignee: '0xc' }),
      CurationState.TO_REVIEW
    ],
    [
      'changes pushed on a disabled collection',
      collection(reviewedBefore),
      curation({ assignee: '0xc' }),
      CurationState.TO_REVIEW
    ],
    [
      // An approved request is the proof, whatever reviewedAt says.
      'an approved request on a collection no longer approved, reviewedAt untouched',
      collection(),
      curation({ status: 'approved' }),
      CurationState.DISABLED
    ]
  ])('%s', (_, subject, request, expected) => {
    expect(getCurationState(subject, request)).toBe(expected)
  })
})

describe('getReviewActions', () => {
  it('offers approve and reject on a first review', () => {
    expect(getReviewActions(collection(), null, false)).toEqual([ReviewAction.APPROVE, ReviewAction.REJECT])
    expect(getReviewActions(collection(), curation({ assignee: '0xc' }), false)).toEqual([
      ReviewAction.APPROVE,
      ReviewAction.REJECT
    ])
  })

  it('only offers approve once a first review was rejected', () => {
    expect(getReviewActions(collection(), curation({ status: 'rejected' }), false)).toEqual([ReviewAction.APPROVE])
  })

  it('offers approve and reject on pushed changes of an approved collection', () => {
    expect(getReviewActions(collection({ isApproved: true }), curation(), false)).toEqual([
      ReviewAction.APPROVE,
      ReviewAction.REJECT
    ])
  })

  it('offers disable on an approved collection, plus the deploy when entities are missing', () => {
    const approved = collection({ isApproved: true })
    expect(getReviewActions(approved, curation({ status: 'approved' }), false)).toEqual([ReviewAction.DISABLE])
    expect(getReviewActions(approved, null, true)).toEqual([ReviewAction.DISABLE, ReviewAction.DEPLOY_MISSING])
  })

  it('offers enable on a disabled collection', () => {
    expect(getReviewActions(collection(reviewedBefore), null, false)).toEqual([ReviewAction.ENABLE])
  })
})

describe('canEditAssignee', () => {
  it('locks the assignee once the collection is published', () => {
    expect(canEditAssignee(collection({ isApproved: true }), curation({ status: 'approved' }))).toBe(false)
    // Approved before review requests existed: assigning would put it back under review.
    expect(canEditAssignee(collection({ isApproved: true }), null)).toBe(false)
    expect(canEditAssignee(collection({ isApproved: true }), curation())).toBe(true)
    expect(canEditAssignee(collection(), curation({ status: 'approved' }))).toBe(true)
  })

  it('locks it on a disabled collection with no request, which Enable turns back on without opening one', () => {
    expect(canEditAssignee(collection(reviewedBefore), null)).toBe(false)
    expect(canEditAssignee(collection(), null)).toBe(true)
  })
})

describe('curation filters', () => {
  const ME = '0xME'

  it('opens on to review, unassigned and the signed-in curator, newest requests first', () => {
    expect(parseCurationFilters(new URLSearchParams('status=bogus&sort=nope&page=-3'), ME)).toEqual({
      page: 1,
      search: '',
      statuses: [CurationStatusFilter.TO_REVIEW],
      assignees: [UNASSIGNED, '0xme'],
      reviewType: null,
      sort: CollectionSort.REQUESTED_AT_DESC,
      tag: null,
      type: CurationTypeFilter.ALL
    })
  })

  it('reads several statuses and assignees, and the review type', () => {
    const filters = parseCurationFilters(
      new URLSearchParams(
        'status=approved&status=rejected&status=nope&assignee=0xABCDEF0123456789ABCDEF0123456789ABCDEF01&assignee=unassigned&review_type=new'
      ),
      ME
    )
    expect(filters.statuses).toEqual([CurationStatusFilter.APPROVED, CurationStatusFilter.REJECTED])
    expect(filters.assignees).toEqual(['0xabcdef0123456789abcdef0123456789abcdef01', UNASSIGNED])
    expect(filters.reviewType).toBe(ReviewType.NEW)
  })

  it('takes "all" as a cleared filter rather than the defaults, even next to a picked value', () => {
    const filters = parseCurationFilters(new URLSearchParams('status=all&status=approved&assignee=all'), ME)
    expect(filters.statuses).toEqual([])
    expect(filters.assignees).toEqual([])
  })

  it('keeps only well-formed addresses and "unassigned" as assignees, falling back to the defaults', () => {
    const valid = '0x' + 'a'.repeat(40)
    expect(
      parseCurationFilters(new URLSearchParams(`assignee=garbage&assignee=${valid.toUpperCase()}`), ME).assignees
    ).toEqual([valid])
    expect(parseCurationFilters(new URLSearchParams('assignee=garbage&assignee='), ME).assignees).toEqual([
      UNASSIGNED,
      '0xme'
    ])
  })

  it('writes a picked set to the URL, "all" for none and nothing for the defaults', () => {
    const defaults = [CurationStatusFilter.TO_REVIEW, CurationStatusFilter.REJECTED]
    expect(toMultiParam([CurationStatusFilter.REJECTED, CurationStatusFilter.TO_REVIEW], defaults)).toBeNull()
    expect(toMultiParam([], defaults)).toEqual(['all'])
    expect(toMultiParam([CurationStatusFilter.REJECTED], defaults)).toEqual([CurationStatusFilter.REJECTED])
    expect(toMultiParam(['0xme', UNASSIGNED], defaultAssignees('0xME'))).toBeNull()
  })
})

describe('defaultAssignees', () => {
  it('opens on the unassigned collections and the signed-in curator, lowercased', () => {
    expect(defaultAssignees('0xME')).toEqual([UNASSIGNED, '0xme'])
    expect(defaultAssignees(undefined)).toEqual([UNASSIGNED])
  })
})

describe('getReviewType', () => {
  it('reads a collection approved on chain, or disabled after an approval, as updates', () => {
    expect(getReviewType(collection({ isApproved: true }))).toBe(ReviewType.UPDATES)
    expect(getReviewType(collection({ isApproved: false, createdAt: 1, reviewedAt: 2 }))).toBe(ReviewType.UPDATES)
  })

  it('reads a collection never approved as new', () => {
    expect(getReviewType(collection({ isApproved: false, createdAt: 1, reviewedAt: 1 }))).toBe(ReviewType.NEW)
  })
})

describe('orderCurators', () => {
  it('puts the signed-in curator first', () => {
    expect(orderCurators(['0xa', '0xB', '0xc'], '0xb')).toEqual(['0xb', '0xa', '0xc'])
    expect(orderCurators(['0xa'], '0xz')).toEqual(['0xa'])
  })
})

describe('canPushChanges', () => {
  const approved = collection({ isApproved: true })

  it('lets managers send unsynced changes of an approved collection', () => {
    expect(canPushChanges(approved, null, true, true)).toBe(true)
    expect(canPushChanges(approved, curation({ status: 'rejected' }), true, true)).toBe(true)
  })

  it('refuses while a request is pending, without changes or without rights', () => {
    expect(canPushChanges(approved, curation(), true, true)).toBe(false)
    expect(canPushChanges(approved, null, false, true)).toBe(false)
    expect(canPushChanges(approved, null, true, false)).toBe(false)
  })

  it('lets managers ask for the first review again after a rejection, and only then', () => {
    expect(canPushChanges(collection(), curation({ status: 'rejected' }), false, true)).toBe(true)
    expect(canPushChanges(collection(), curation({ status: 'rejected' }), false, false)).toBe(false)
    expect(canPushChanges(collection(), null, true, true)).toBe(false)
    expect(canPushChanges(collection(), curation(), true, true)).toBe(false)
    expect(canPushChanges(collection({ isPublished: false }), curation({ status: 'rejected' }), true, true)).toBe(false)
  })
})

describe('curationListUrl', () => {
  it('returns to the list with the filters it was left with', () => {
    sessionStorage.clear()
    expect(curationListUrl()).toBe('/curation')
    rememberCurationSearch('?status=approved&page=2')
    expect(curationListUrl()).toBe('/curation?status=approved&page=2')
  })
})

describe('getReviewStage', () => {
  const event = (type: string, payload: CollectionEvent['payload'] = {}) => ({
    id: 'e',
    collectionId: 'c1',
    type,
    actor: 'validator' as const,
    actorAddress: null,
    payload,
    createdAt: 1
  })
  const approvedOnChain = collection({ isApproved: true })

  it.each([
    ['nothing without a request', collection(), null, [event('review.automated_started')], null],
    ['nothing without a timeline', collection(), curation(), null, null],
    [
      'in automated review while the validator works',
      collection(),
      curation(),
      [event('review.automated_started')],
      ReviewStage.AUTOMATED_REVIEWING
    ],
    [
      'still in automated review after a validator error',
      collection(),
      curation(),
      [event('review.automated_error')],
      ReviewStage.AUTOMATED_REVIEWING
    ],
    [
      'awaiting a curator once the automated review passed',
      collection(),
      curation(),
      [event('review.automated_passed')],
      ReviewStage.AWAITING_CURATOR
    ],
    [
      'needing a curator when a third-party collection skips the automated review',
      collection(),
      curation(),
      [event('review.human_required'), event('review.automated_error'), event('review.automated_started')],
      ReviewStage.NEEDS_CURATOR
    ],
    [
      'needing a curator once the server gives up resending',
      collection(),
      curation(),
      [event('review.automated_error', { reason: 'sweep_exhausted' }), event('review.automated_error')],
      ReviewStage.NEEDS_CURATOR
    ],
    [
      'needing a curator for a collection too large to send',
      collection(),
      curation(),
      [event('review.automated_error', { reason: 'too_large' }), event('review.automated_started')],
      ReviewStage.NEEDS_CURATOR
    ],
    ['appealed', collection(), curation(), [event('review.appeal_requested')], ReviewStage.APPEALED],
    [
      'in automated review again after a retry: the server opens a new pending request over the rejected one',
      collection(),
      curation(),
      [event('review.automated_started'), event('review.automated_rejected')],
      ReviewStage.AUTOMATED_REVIEWING
    ],
    [
      'appealed after a rejection: the appeal also reopens the request as pending',
      collection(),
      curation(),
      [event('review.appeal_requested'), event('review.rejected')],
      ReviewStage.APPEALED
    ],
    [
      'nothing for a pending request without a telling event',
      collection(),
      curation(),
      [event('review.assigned')],
      null
    ],
    ['nothing for a pending request with an empty timeline', collection(), curation(), [], null],
    [
      'awaiting a curator through a later assignment',
      collection(),
      curation({ assignee: '0xc' }),
      [event('review.assigned'), event('changes.submitted'), event('review.automated_passed')],
      ReviewStage.AWAITING_CURATOR
    ],
    [
      'appealed through a later assignment',
      collection(),
      curation(),
      [event('review.assigned'), event('review.appeal_requested')],
      ReviewStage.APPEALED
    ],
    [
      'rejected by the validator',
      collection(),
      curation({ status: 'rejected', reviewedBy: 'validator' }),
      [event('review.automated_rejected')],
      ReviewStage.REJECTED_BY_VALIDATOR
    ],
    [
      'rejected by a curator',
      collection(),
      curation({ status: 'rejected', reviewedBy: '0xcurator' }),
      [event('review.rejected')],
      ReviewStage.REJECTED_BY_CURATOR
    ],
    [
      'nothing for a curator rejection without a timeline, so the legacy state stays',
      collection(),
      curation({ status: 'rejected', reviewedBy: '0xcurator' }),
      null,
      null
    ],
    ['nothing for a legacy rejection', collection(), curation({ status: 'rejected' }), [], null],
    [
      'approved',
      approvedOnChain,
      curation({ status: 'approved', reviewedBy: '0xcurator' }),
      [event('review.approved')],
      ReviewStage.APPROVED
    ],
    [
      'nothing for an approved request on a collection disabled on chain',
      collection(reviewedBefore),
      curation({ status: 'approved', reviewedBy: '0xcurator' }),
      [event('review.approved')],
      null
    ]
  ])('%s', (_, subject, request, events, expected) => {
    expect(getReviewStage(subject, request, events)).toBe(expected)
  })
})

describe('hasReviewConversation', () => {
  const request = (status: CollectionCuration['status']) => ({ status }) as CollectionCuration

  it('holds while a review is open or was sent back', () => {
    expect(hasReviewConversation(request('pending'))).toBe(true)
    expect(hasReviewConversation(request('rejected'))).toBe(true)
  })

  it('ends with the approval, and never started without a request', () => {
    expect(hasReviewConversation(request('approved'))).toBe(false)
    expect(hasReviewConversation(null)).toBe(false)
  })
})
