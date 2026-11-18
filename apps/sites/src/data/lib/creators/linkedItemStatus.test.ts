import { describe, it, expect } from 'vitest'
import { CurationStatus } from './collections'
import { type Mappings } from '@dcl/schemas'
import { ItemType, type Item } from './items'
import {
  LinkedItemFilter,
  LinkedItemStatus,
  canPushLinkedItem,
  getLinkedItemStatus,
  getLinkedPublishAction,
  hasLinkedWork,
  isLinkedDraft,
  isLinkedItemPublished,
  linkedItemFilterToQuery,
  parseLinkedItemFilter,
  splitLinkedSelection,
  type ItemCuration
} from './linkedItemStatus'

const MAPPINGS = { amoy: { ['0x' + '1'.repeat(40)]: [{ type: 'any' }] } } as Mappings

function makeItem(overrides: Partial<Item> = {}): Item {
  return {
    id: 'i1',
    name: 'Cap',
    description: '',
    thumbnail: 'thumbnail.png',
    owner: '0xowner',
    collectionId: 'c1',
    isPublished: false,
    isApproved: false,
    inCatalyst: false,
    mappings: MAPPINGS,
    isMappingComplete: true,
    currentContentHash: 'hash-1',
    type: ItemType.WEARABLE,
    data: { category: 'hat', representations: [] },
    contents: {},
    createdAt: 1,
    updatedAt: 1,
    ...overrides
  }
}

function curation(status: CurationStatus, contentHash = 'hash-1'): ItemCuration {
  return { id: 'ic', itemId: 'i1', status, contentHash, isMappingComplete: true, createdAt: 1, updatedAt: 1 }
}

describe('getLinkedItemStatus', () => {
  it('is ready to publish while the item was never submitted', () => {
    expect(getLinkedItemStatus(makeItem(), null)).toBe(LinkedItemStatus.READY_TO_PUBLISH)
  })

  it('follows a pending or rejected curation', () => {
    expect(getLinkedItemStatus(makeItem({ isPublished: true }), curation(CurationStatus.PENDING))).toBe(
      LinkedItemStatus.UNDER_REVIEW
    )
    expect(getLinkedItemStatus(makeItem({ isPublished: true }), curation(CurationStatus.REJECTED))).toBe(
      LinkedItemStatus.REJECTED
    )
  })

  it('is published while the approved item matches the Catalyst, and ready to push once it differs', () => {
    const approved = makeItem({ isPublished: true, isApproved: true, catalystContentHash: 'hash-1' })
    expect(getLinkedItemStatus(approved, curation(CurationStatus.APPROVED))).toBe(LinkedItemStatus.PUBLISHED)
    expect(getLinkedItemStatus({ ...approved, currentContentHash: 'hash-2' }, curation(CurationStatus.APPROVED))).toBe(
      LinkedItemStatus.READY_TO_PUSH
    )
  })

  it('flags items without a mapping, and items whose mapping was never reviewed', () => {
    expect(getLinkedItemStatus(makeItem({ mappings: null }), null)).toBe(LinkedItemStatus.PENDING_MAPPING)
    expect(getLinkedItemStatus(makeItem({ isMappingComplete: false }), curation(CurationStatus.APPROVED))).toBe(
      LinkedItemStatus.PENDING_MIGRATION
    )
  })
})

describe('isLinkedItemPublished', () => {
  it('is true once the item has any curation or the server says so', () => {
    expect(isLinkedItemPublished(makeItem(), null)).toBe(false)
    expect(isLinkedItemPublished(makeItem(), curation(CurationStatus.REJECTED))).toBe(true)
    expect(isLinkedItemPublished(makeItem({ isPublished: true }), null)).toBe(true)
  })
})

describe('canPushLinkedItem', () => {
  const approved = makeItem({ isPublished: true, isApproved: true, catalystContentHash: 'hash-1' })

  it('needs an approved item whose builder copy moved on', () => {
    expect(canPushLinkedItem(approved, curation(CurationStatus.APPROVED))).toBe(false)
    expect(canPushLinkedItem({ ...approved, currentContentHash: 'hash-2' }, curation(CurationStatus.APPROVED))).toBe(
      true
    )
  })

  it('also allows refreshing a pending request opened for an older hash', () => {
    expect(canPushLinkedItem(approved, curation(CurationStatus.PENDING, 'hash-0'))).toBe(true)
    expect(canPushLinkedItem(makeItem(), null)).toBe(false)
  })

  it('does not send again changes already waiting for review', () => {
    const edited = { ...approved, currentContentHash: 'hash-2' }
    expect(canPushLinkedItem(edited, curation(CurationStatus.PENDING, 'hash-2'))).toBe(false)
    expect(canPushLinkedItem({ ...approved, currentContentHash: 'x' }, curation(CurationStatus.REJECTED))).toBe(false)
  })
})

describe('splitLinkedSelection / getLinkedPublishAction', () => {
  const fresh = makeItem({ id: 'fresh' })
  const changed = makeItem({
    id: 'changed',
    isPublished: true,
    isApproved: true,
    catalystContentHash: 'old',
    currentContentHash: 'new'
  })
  const pending = makeItem({ id: 'pending', isPublished: true })
  const curations = new Map<string, ItemCuration>([
    ['changed', { ...curation(CurationStatus.APPROVED, 'old'), itemId: 'changed' }],
    ['pending', { ...curation(CurationStatus.PENDING), itemId: 'pending' }]
  ])

  it('sorts the selection into publishes and pushes, leaving unselectable items out', () => {
    const selection = splitLinkedSelection([fresh, changed, pending], curations)
    expect(selection.toPublish.map(item => item.id)).toEqual(['fresh'])
    expect(selection.toPush.map(item => item.id)).toEqual(['changed'])
    expect(getLinkedPublishAction(selection)).toBe('publish_and_push')
  })

  it('names the action after what was selected', () => {
    expect(getLinkedPublishAction(splitLinkedSelection([fresh], curations))).toBe('publish')
    expect(getLinkedPublishAction(splitLinkedSelection([changed], curations))).toBe('push')
    expect(getLinkedPublishAction(splitLinkedSelection([pending], curations))).toBeNull()
  })
})

describe('item filters', () => {
  it('maps each chip to the server params', () => {
    expect(linkedItemFilterToQuery(LinkedItemFilter.ALL)).toEqual({})
    expect(linkedItemFilterToQuery(LinkedItemFilter.PUBLISHED)).toEqual({ synced: true })
    expect(linkedItemFilterToQuery(LinkedItemFilter.UNDER_REVIEW)).toEqual({ status: CurationStatus.PENDING })
    expect(linkedItemFilterToQuery(LinkedItemFilter.READY_TO_PUSH)).toEqual({ synced: false })
    expect(linkedItemFilterToQuery(LinkedItemFilter.PENDING_MAPPING)).toEqual({ mappingStatus: 'missing_mapping' })
    expect(linkedItemFilterToQuery(LinkedItemFilter.PENDING_MIGRATION)).toEqual({
      mappingStatus: 'unpublished_mapping'
    })
  })

  it('falls back to All for anything unknown in the URL', () => {
    expect(parseLinkedItemFilter('published')).toBe(LinkedItemFilter.PUBLISHED)
    expect(parseLinkedItemFilter('nope')).toBe(LinkedItemFilter.ALL)
  })
})

describe('hasLinkedWork', () => {
  const curated = (status: CurationStatus) => new Map([['i1', { ...curation(status), itemId: 'i1' }]])

  it('finds items never submitted, as long as they have a mapping', () => {
    expect(
      hasLinkedWork({ itemCount: 2, curations: curated(CurationStatus.APPROVED), unsynced: 0, missingMappingIds: [] })
    ).toBe(true)
    expect(
      hasLinkedWork({
        itemCount: 2,
        curations: curated(CurationStatus.APPROVED),
        unsynced: 0,
        missingMappingIds: ['new']
      })
    ).toBe(false)
  })

  it('does not count curated items without a mapping against the new ones', () => {
    // i1 was approved before mappings existed; one new mapped item is waiting.
    expect(
      hasLinkedWork({
        itemCount: 2,
        curations: curated(CurationStatus.APPROVED),
        unsynced: 0,
        missingMappingIds: ['i1']
      })
    ).toBe(true)
  })

  it('finds rejected items and approved items with changes', () => {
    expect(
      hasLinkedWork({ itemCount: 1, curations: curated(CurationStatus.REJECTED), unsynced: 0, missingMappingIds: [] })
    ).toBe(true)
    expect(
      hasLinkedWork({ itemCount: 1, curations: curated(CurationStatus.APPROVED), unsynced: 1, missingMappingIds: [] })
    ).toBe(true)
  })

  it('has nothing to do when every item is published, under review, or the collection is empty', () => {
    expect(
      hasLinkedWork({ itemCount: 1, curations: curated(CurationStatus.APPROVED), unsynced: 0, missingMappingIds: [] })
    ).toBe(false)
    expect(
      hasLinkedWork({ itemCount: 1, curations: curated(CurationStatus.PENDING), unsynced: 0, missingMappingIds: [] })
    ).toBe(false)
    expect(hasLinkedWork({ itemCount: 0, curations: new Map(), unsynced: 0, missingMappingIds: [] })).toBe(false)
  })
})

describe('isLinkedDraft', () => {
  it('counts items never submitted and rejected ones, and nothing under review or approved', () => {
    expect(isLinkedDraft(null)).toBe(true)
    expect(isLinkedDraft(curation(CurationStatus.REJECTED))).toBe(true)
    expect(isLinkedDraft(curation(CurationStatus.PENDING))).toBe(false)
    expect(isLinkedDraft(curation(CurationStatus.APPROVED))).toBe(false)
  })
})
