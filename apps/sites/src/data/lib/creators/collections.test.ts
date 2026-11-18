import { describe, it, expect } from 'vitest'
import {
  findCollectionByContract,
  COLLECTION_NAME_MAX_LENGTH,
  CollectionDisplayStatus,
  canManageCollectionItems,
  canSellCollectionItems,
  CollectionStatusFilter,
  CurationStatus,
  getCollectionDisplayStatus,
  getCollectionRole,
  hasCollectionRole,
  canViewCollection,
  CollectionRole,
  hasBeenApproved,
  isCollectionLocked,
  isLinkedCollection,
  isStatusFilterShown,
  listFilterToParams,
  summarizeCollections,
  LINKED_FILTER,
  validateCollectionName,
  withoutCollection,
  type Collection
} from './collections'
import { fromRemoteCollection, toCollectionsQueryString } from '@/data/lib/creators/builder/mappers'
import type { RemoteCollection } from '@/data/lib/creators/builder/wire'

const remote: RemoteCollection = {
  id: 'a1b2',
  name: 'Pirate Hats',
  eth_address: '0xAbC0000000000000000000000000000000000001',
  salt: '0xsalt',
  contract_address: '0xcontract',
  urn: 'urn:decentraland:matic:collections-v2:0xcontract',
  is_published: true,
  is_approved: true,
  minters: ['0xminter'],
  managers: [],
  forum_link: null,
  lock: null,
  reviewed_at: '2026-04-01T10:00:00Z',
  created_at: '2026-03-01T10:00:00Z',
  updated_at: '2026-04-02T10:00:00Z',
  item_count: '250',
  linked_contract_address: null,
  linked_contract_network: null,
  is_mapping_complete: true
}

describe('listFilterToParams', () => {
  it('lists standard and linked collections together under All', () => {
    expect(toCollectionsQueryString(listFilterToParams(CollectionStatusFilter.ALL))).toBe('')
  })

  it('sends every status chip as the status param', () => {
    expect(toCollectionsQueryString(listFilterToParams(CollectionStatusFilter.UNDER_REVIEW))).toBe(
      '?status=under_review'
    )
    expect(toCollectionsQueryString(listFilterToParams(CollectionStatusFilter.DISABLED))).toBe('?status=disabled')
  })

  it('lists only linked collections under the Linked chip', () => {
    expect(toCollectionsQueryString(listFilterToParams(LINKED_FILTER))).toBe('?type=third_party')
  })
})

describe('summarizeCollections', () => {
  const counts = { draft: 3, under_review: 1, published: 6, rejected: 0, disabled: 0 }

  it('counts as linked the collections the status counts leave out', () => {
    const list = { results: [], total: 12, limit: 1, page: 1, pages: 12, counts }
    expect(summarizeCollections(list)).toEqual({ counts, linkedTotal: 2 })
  })

  it('has nothing to summarize without counts', () => {
    expect(summarizeCollections({ results: [], total: 0, limit: 1, page: 1, pages: 0 })).toBeUndefined()
  })
})

describe('isLinkedCollection', () => {
  it('tells linked collections apart by their third-party URN', () => {
    expect(isLinkedCollection({ urn: 'urn:decentraland:amoy:collections-thirdparty:brand:hats' })).toBe(true)
    expect(isLinkedCollection({ urn: 'urn:decentraland:amoy:collections-v2:0xcontract' })).toBe(false)
  })
})

describe('isStatusFilterShown', () => {
  const counts = { draft: 0, under_review: 0, published: 0, rejected: 2, disabled: 0 }

  it('always shows All, Draft and Published, and the rest only while they have collections', () => {
    expect(isStatusFilterShown(CollectionStatusFilter.ALL, counts)).toBe(true)
    expect(isStatusFilterShown(CollectionStatusFilter.DRAFT, counts)).toBe(true)
    expect(isStatusFilterShown(CollectionStatusFilter.PUBLISHED, counts)).toBe(true)
    expect(isStatusFilterShown(CollectionStatusFilter.REJECTED, counts)).toBe(true)
    expect(isStatusFilterShown(CollectionStatusFilter.UNDER_REVIEW, counts)).toBe(false)
    expect(isStatusFilterShown(CollectionStatusFilter.DISABLED, undefined)).toBe(false)
  })
})

describe('getCollectionDisplayStatus', () => {
  const now = Date.now()
  const base: Collection = { ...fromRemoteCollection(remote), createdAt: 1000, reviewedAt: 1000, lock: undefined }
  const published = { ...base, isPublished: true }
  const approved = { ...published, isApproved: true, reviewedAt: 2000 }
  const disabled = { ...published, isApproved: false, reviewedAt: 2000 }
  const firstReview = { ...published, isApproved: false }

  it.each([
    ['a draft', { ...base, isPublished: false }, null, CollectionDisplayStatus.DRAFT],
    [
      'a linked collection',
      { ...published, urn: 'urn:decentraland:amoy:collections-thirdparty:brand:hats' },
      CurationStatus.APPROVED,
      CollectionDisplayStatus.LINKED
    ],
    [
      'a draft whose publish is in flight',
      { ...base, isPublished: false, lock: now },
      null,
      CollectionDisplayStatus.PUBLISHING
    ],
    ['a first review, unassigned', firstReview, null, CollectionDisplayStatus.UNDER_REVIEW],
    ['a first review, assigned', firstReview, CurationStatus.PENDING, CollectionDisplayStatus.UNDER_REVIEW],
    ['a rejected first review', firstReview, CurationStatus.REJECTED, CollectionDisplayStatus.REJECTED],
    ['an approved collection', approved, null, CollectionDisplayStatus.PUBLISHED],
    [
      'an approved collection with an approved curation',
      approved,
      CurationStatus.APPROVED,
      CollectionDisplayStatus.PUBLISHED
    ],
    [
      'an approved collection with changes sent',
      approved,
      CurationStatus.PENDING,
      CollectionDisplayStatus.UNDER_REVIEW
    ],
    [
      'an approved collection with its changes rejected',
      approved,
      CurationStatus.REJECTED,
      CollectionDisplayStatus.REJECTED
    ],
    ['a disabled collection', disabled, CurationStatus.APPROVED, CollectionDisplayStatus.DISABLED],
    ['a disabled collection approved without a curation', disabled, null, CollectionDisplayStatus.DISABLED],
    [
      'a first approval stuck between rescueItems and setApproved',
      disabled,
      CurationStatus.PENDING,
      CollectionDisplayStatus.UNDER_REVIEW
    ],
    [
      'a disabled collection with a rejected curation',
      disabled,
      CurationStatus.REJECTED,
      CollectionDisplayStatus.REJECTED
    ]
  ])('shows %s as its status', (_, collection, curationStatus, expected) => {
    expect(getCollectionDisplayStatus(collection, curationStatus)).toBe(expected)
  })

  it("defaults to the list's curation status", () => {
    expect(getCollectionDisplayStatus({ ...approved, curationStatus: CurationStatus.PENDING })).toBe(
      CollectionDisplayStatus.UNDER_REVIEW
    )
  })
})

describe('hasBeenApproved', () => {
  const base = fromRemoteCollection(remote)

  it('is true for an approved collection and for one reviewed before that is under review again', () => {
    expect(hasBeenApproved({ ...base, isPublished: true, isApproved: true })).toBe(true)
    expect(hasBeenApproved({ ...base, isPublished: true, isApproved: false, reviewedAt: 1 })).toBe(true)
  })

  it('is false for drafts and for a first review', () => {
    expect(hasBeenApproved({ ...base, isPublished: false, isApproved: false })).toBe(false)
    expect(hasBeenApproved({ ...base, isPublished: true, isApproved: false, reviewedAt: undefined })).toBe(false)
    // The contract stamps reviewedAt with createdAt on creation, before any review.
    expect(hasBeenApproved({ ...base, isPublished: true, isApproved: false, reviewedAt: base.createdAt })).toBe(false)
  })
})

describe('getCollectionRole', () => {
  const base: Collection = {
    ...fromRemoteCollection(remote),
    owner: '0xOwner',
    managers: ['0xManager'],
    minters: ['0xMinter', '0xManager']
  }

  it('reports collaborator or minter for non-owners, case-insensitively', () => {
    expect(getCollectionRole(base, '0xmanager')).toBe(CollectionRole.COLLABORATOR)
    expect(getCollectionRole(base, '0xMINTER')).toBe(CollectionRole.MINTER)
  })

  it('reports no role for the owner or a stranger', () => {
    expect(getCollectionRole(base, '0xowner')).toBeNull()
    expect(getCollectionRole(base, '0xother')).toBeNull()
  })
})

describe('validateCollectionName', () => {
  it('rejects empty, over-long and colon-containing names, accepts the rest', () => {
    expect(validateCollectionName('')).toBe('empty')
    expect(validateCollectionName('   ')).toBe('empty')
    expect(validateCollectionName('a'.repeat(COLLECTION_NAME_MAX_LENGTH + 1))).toBe('too_long')
    expect(validateCollectionName('urn:like')).toBe('invalid_character')
    expect(validateCollectionName('Pirate Hats')).toBeNull()
    expect(validateCollectionName('a'.repeat(COLLECTION_NAME_MAX_LENGTH))).toBeNull()
  })
})

describe('isCollectionLocked', () => {
  const base = fromRemoteCollection(remote)
  const HOUR = 3_600_000

  it('locks for a day after the lock timestamp, unless published', () => {
    const now = Date.now()
    const draft = { ...base, isPublished: false }
    expect(isCollectionLocked({ ...draft, lock: now - HOUR }, now)).toBe(true)
    expect(isCollectionLocked({ ...draft, lock: now - 25 * HOUR }, now)).toBe(false)
    expect(isCollectionLocked({ ...draft, lock: undefined }, now)).toBe(false)
    expect(isCollectionLocked({ ...base, isPublished: true, lock: now - HOUR }, now)).toBe(false)
  })
})

const roleCollection: Collection = {
  id: 'c1',
  name: 'Hats',
  owner: '0xOwner',
  urn: 'urn',
  isPublished: false,
  isApproved: false,
  itemCount: 0,
  minters: ['0xMinter'],
  managers: ['0xManager'],
  createdAt: 1,
  updatedAt: 1
}

describe('hasCollectionRole', () => {
  it('grants access to the owner, collaborators and minters, but not to strangers or signed-out users', () => {
    expect(hasCollectionRole(roleCollection, '0xowner')).toBe(true)
    expect(hasCollectionRole(roleCollection, '0xMANAGER')).toBe(true)
    expect(hasCollectionRole(roleCollection, '0xminter')).toBe(true)
    expect(hasCollectionRole(roleCollection, '0xstranger')).toBe(false)
    expect(hasCollectionRole(roleCollection, undefined)).toBe(false)
  })
})

describe('canViewCollection', () => {
  it('opens published collections to anyone signed in, drafts only to addresses with a role', () => {
    expect(canViewCollection({ ...roleCollection, isPublished: true }, '0xstranger')).toBe(true)
    expect(canViewCollection({ ...roleCollection, isPublished: true }, undefined)).toBe(false)
    expect(canViewCollection({ ...roleCollection, isPublished: false }, '0xstranger')).toBe(false)
    expect(canViewCollection({ ...roleCollection, isPublished: false }, '0xminter')).toBe(true)
  })
})

describe('canManageCollectionItems', () => {
  it('is granted to the owner and collaborators regardless of address casing, never to minters', () => {
    expect(canManageCollectionItems(roleCollection, '0xowner')).toBe(true)
    expect(canManageCollectionItems(roleCollection, '0xMANAGER')).toBe(true)
    expect(canManageCollectionItems(roleCollection, '0xminter')).toBe(false)
    expect(canManageCollectionItems(roleCollection, undefined)).toBe(false)
  })

  it('keeps a former owner who listed themselves as a collaborator able to edit after a transfer', () => {
    const transferred = { ...roleCollection, owner: '0xnewowner', managers: ['0xOwner'] }
    expect(canManageCollectionItems(transferred, '0xowner')).toBe(true)
    expect(getCollectionRole(transferred, '0xowner')).toBe(CollectionRole.COLLABORATOR)
    expect(getCollectionRole({ ...roleCollection, managers: ['0xOwner'] }, '0xowner')).toBeNull()
  })
})

describe('canSellCollectionItems', () => {
  it('lets only the owner sell: collaborators, minters and strangers are kept out', () => {
    expect(canSellCollectionItems(roleCollection, '0xowner')).toBe(true)
    expect(canSellCollectionItems(roleCollection, '0xMANAGER')).toBe(false)
    expect(canSellCollectionItems(roleCollection, '0xminter')).toBe(false)
    expect(canSellCollectionItems(roleCollection, '0xstranger')).toBe(false)
    expect(canSellCollectionItems(roleCollection, undefined)).toBe(false)
  })
})

describe('findCollectionByContract', () => {
  const CONTRACT = '0x1d9fB685c257E74F869BA302e260C0B68f5eBb37'
  const collections = [{ id: 'draft' }, { id: 'published', contractAddress: CONTRACT.toLowerCase() }] as Collection[]

  it('finds the collection deployed at the address, whatever its casing', () => {
    expect(findCollectionByContract(collections, CONTRACT)?.id).toBe('published')
  })

  it('finds nothing for an address none of the collections is deployed at', () => {
    expect(findCollectionByContract(collections, '0x' + '0'.repeat(40))).toBeUndefined()
  })

  it('finds nothing for a value that is not a contract address', () => {
    expect(findCollectionByContract(collections, 'undefined')).toBeUndefined()
  })
})

describe('withoutCollection', () => {
  const kept = { id: 'kept' } as Collection
  const deleted = { id: 'deleted' } as Collection
  const page = { results: [kept, deleted], total: 9, limit: 8, page: 1, pages: 2 }

  it('drops the deleted collection from a cached page, leaving the totals to the refetch', () => {
    expect(withoutCollection(page, 'deleted')).toStrictEqual({ ...page, results: [kept] })
  })

  // The same object, not an equal copy, so react-query sees no change and re-renders nothing.
  it('returns the same page when it does not hold that collection', () => {
    expect(withoutCollection(page, 'other')).toBe(page)
  })

  it('leaves anything that is not a page untouched', () => {
    expect(withoutCollection(undefined, 'deleted')).toBeUndefined()
    const contractLinkResults = [{ id: 'deleted' }]
    expect(withoutCollection(contractLinkResults, 'deleted')).toBe(contractLinkResults)
  })
})
