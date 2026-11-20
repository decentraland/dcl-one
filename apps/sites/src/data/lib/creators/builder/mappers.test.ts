// @vitest-environment jsdom
import '../test/setup'
import { describe, it, expect } from 'vitest'
import { CollectionSort, CollectionType, CurationStatus } from '../collections'
import { parseCurationFilters } from '../curation'
import { ItemType, type Item } from '../items'
import {
  fromRemoteCollection,
  fromRemoteCuration,
  fromRemoteEvent,
  fromRemoteItem,
  fromRemoteItemCuration,
  fromRemoteLinkedApprovalData,
  fromRemoteThirdParty,
  toCollectionsQueryString,
  toCurationQueryString,
  toRemoteCollection,
  toRemoteItem
} from './mappers'
import type { RemoteCollection, RemoteItem } from './wire'

describe('collection mappers', () => {
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

  describe('fromRemoteCollection', () => {
    it('maps the wire shape to the domain model with epoch dates and a numeric item count', () => {
      const collection = fromRemoteCollection(remote)
      expect(collection).toMatchObject({
        id: 'a1b2',
        name: 'Pirate Hats',
        owner: remote.eth_address,
        contractAddress: '0xcontract',
        salt: '0xsalt',
        isPublished: true,
        isApproved: true,
        itemCount: 250,
        minters: ['0xminter']
      })
      expect(collection.createdAt).toBe(+new Date('2026-03-01T10:00:00Z'))
      expect(collection.updatedAt).toBe(+new Date('2026-04-02T10:00:00Z'))
      expect(collection.reviewedAt).toBe(+new Date('2026-04-01T10:00:00Z'))
      expect(collection.lock).toBeUndefined()
    })

    it('maps the list-only curation status and last activity', () => {
      const collection = fromRemoteCollection({
        ...remote,
        curation_status: CurationStatus.PENDING,
        last_activity_at: '2026-10-02T11:13:21Z'
      })
      expect(collection.curationStatus).toBe(CurationStatus.PENDING)
      expect(collection.lastActivityAt).toBe(+new Date('2026-10-02T11:13:21Z'))
      expect(fromRemoteCollection(remote).lastActivityAt).toBeUndefined()
    })

    it('treats a missing item_count as zero and omits empty optionals', () => {
      const collection = fromRemoteCollection({
        ...remote,
        item_count: undefined,
        salt: null,
        contract_address: null,
        forum_link: null,
        reviewed_at: null
      })
      expect(collection.itemCount).toBe(0)
      expect(collection.salt).toBeUndefined()
      expect(collection.contractAddress).toBeUndefined()
      expect(collection.forumLink).toBeUndefined()
      expect(collection.reviewedAt).toBeUndefined()
    })
  })

  describe('toCollectionsQueryString', () => {
    it('serializes to the snake_case params builder-server expects, dropping absent ones', () => {
      expect(
        toCollectionsQueryString({
          page: 2,
          limit: 20,
          q: 'hat',
          type: CollectionType.STANDARD,
          sort: CollectionSort.LAST_ACTIVITY_DESC,
          isPublished: true
        })
      ).toBe('?is_published=true&type=standard&sort=LAST_ACTIVITY_DESC&q=hat&page=2&limit=20')
    })

    it('sends a contract address as contract_address', () => {
      expect(toCollectionsQueryString({ contractAddress: '0xabc', page: 1, limit: 1 })).toBe(
        '?contract_address=0xabc&page=1&limit=1'
      )
    })

    it('serializes is_published=false and returns an empty string with no params', () => {
      expect(toCollectionsQueryString({ isPublished: false })).toBe('?is_published=false')
      expect(toCollectionsQueryString({})).toBe('')
    })
  })

  describe('toRemoteCollection', () => {
    it('maps to the snake_case upsert payload, forcing the publish flags off', () => {
      const collection = fromRemoteCollection(remote)
      expect(toRemoteCollection(collection)).toEqual({
        id: 'a1b2',
        name: 'Pirate Hats',
        eth_address: remote.eth_address,
        salt: '0xsalt',
        contract_address: '0xcontract',
        urn: remote.urn,
        is_published: false,
        is_approved: false,
        linked_contract_address: null,
        linked_contract_network: null,
        minters: ['0xminter'],
        managers: [],
        forum_link: null,
        reviewed_at: '2026-04-01T10:00:00.000Z'
      })
    })

    it('nulls absent optionals', () => {
      const collection = fromRemoteCollection({
        ...remote,
        salt: null,
        contract_address: null,
        forum_link: null,
        reviewed_at: null
      })
      const payload = toRemoteCollection(collection)
      expect(payload.salt).toBeNull()
      expect(payload.contract_address).toBeNull()
      expect(payload.forum_link).toBeNull()
      expect(payload.reviewed_at).toBeNull()
    })
  })
})

const MALE = 'urn:decentraland:off-chain:base-avatars:BaseMale'
const FEMALE = 'urn:decentraland:off-chain:base-avatars:BaseFemale'

describe('item mappers', () => {
  const remote: RemoteItem = {
    id: 'i1',
    name: 'Pirate Hat',
    description: 'Yarr',
    thumbnail: 'thumbnail.png',
    eth_address: '0xabc',
    collection_id: 'c1',
    price: '1000',
    beneficiary: '0xbenef',
    rarity: 'legendary',
    is_published: true,
    is_approved: true,
    in_catalyst: true,
    type: ItemType.WEARABLE,
    data: {
      category: 'hat',
      representations: [{ bodyShapes: [MALE, FEMALE], mainFile: 'hat.glb', contents: ['hat.glb'] }]
    },
    contents: { 'thumbnail.png': 'Qmhash', 'hat.glb': 'Qmglb' },
    created_at: '2026-03-01T10:00:00Z',
    updated_at: '2026-04-02T10:00:00Z'
  }

  describe('fromRemoteItem', () => {
    it('maps the wire shape to the domain model with epoch dates', () => {
      const item = fromRemoteItem(remote)
      expect(item).toMatchObject({
        id: 'i1',
        name: 'Pirate Hat',
        owner: '0xabc',
        collectionId: 'c1',
        rarity: 'legendary',
        isPublished: true,
        isApproved: true,
        type: ItemType.WEARABLE
      })
      expect(item.createdAt).toBe(+new Date('2026-03-01T10:00:00Z'))
      expect(item.updatedAt).toBe(+new Date('2026-04-02T10:00:00Z'))
    })

    it('omits empty optionals', () => {
      const item = fromRemoteItem({ ...remote, collection_id: null, price: null, beneficiary: null, rarity: null })
      expect(item.collectionId).toBeUndefined()
      expect(item.price).toBeUndefined()
      expect(item.beneficiary).toBeUndefined()
      expect(item.rarity).toBeUndefined()
    })
  })

  describe('toRemoteItem', () => {
    it('maps the write-direction wire shape the legacy builder sends', () => {
      const item: Item = {
        id: 'item-1',
        name: 'Hat',
        description: '',
        thumbnail: 'thumbnail.png',
        owner: '0xowner',
        collectionId: 'col-1',
        rarity: 'epic',
        totalSupply: 0,
        price: '10',
        beneficiary: '0xowner',
        isPublished: true,
        isApproved: true,
        inCatalyst: false,
        type: ItemType.WEARABLE,
        data: { representations: [] },
        metrics: { triangles: 10 },
        contents: { 'male/model.glb': 'Qm1' },
        createdAt: 1,
        updatedAt: 1
      }

      const remote = toRemoteItem(item)

      expect(remote.eth_address).toBe('0xowner')
      expect(remote.collection_id).toBe('col-1')
      expect(remote.total_supply).toBe(0)
      expect(remote.urn).toBeNull()
      expect(remote.video).toBeNull()
      // The server owns publication state; the client always sends false.
      expect(remote.is_published).toBe(false)
      expect(remote.is_approved).toBe(false)
      expect(remote.contents).toEqual({ 'male/model.glb': 'Qm1' })
    })
  })
})

describe('fromRemoteCuration', () => {
  it('maps the row and lowercases the assignee', () => {
    expect(
      fromRemoteCuration({
        id: 'r1',
        collection_id: 'c1',
        status: 'pending',
        assignee: '0xABC',
        created_at: '2026-01-01T00:00:00Z',
        updated_at: '2026-01-02T00:00:00Z'
      })
    ).toEqual({
      id: 'r1',
      collectionId: 'c1',
      status: 'pending',
      assignee: '0xabc',
      reviewedBy: null,
      rejectionReasons: null,
      rejectionMessage: null,
      createdAt: Date.parse('2026-01-01T00:00:00Z'),
      updatedAt: Date.parse('2026-01-02T00:00:00Z')
    })
  })

  it('maps the reviewer and the rejection fields from the row', () => {
    expect(
      fromRemoteCuration({
        id: 'r1',
        collection_id: 'c1',
        status: 'rejected',
        reviewed_by: '0xCURATOR',
        rejection_reasons: ['clipping', 'bogus', 'other'],
        rejection_message: 'Fix the sleeves',
        created_at: '2026-01-01T00:00:00Z',
        updated_at: '2026-01-02T00:00:00Z'
      })
    ).toMatchObject({
      reviewedBy: '0xcurator',
      rejectionReasons: ['clipping', 'other'],
      rejectionMessage: 'Fix the sleeves'
    })
  })
})

describe('fromRemoteEvent', () => {
  it('maps the row, lowercases the actor and defaults an empty payload', () => {
    expect(
      fromRemoteEvent({
        id: 'e1',
        collection_id: 'c1',
        type: 'review.approved',
        actor: 'curator',
        actor_address: '0xABC',
        payload: null,
        created_at: '2026-09-29T10:00:00Z'
      })
    ).toEqual({
      id: 'e1',
      collectionId: 'c1',
      type: 'review.approved',
      actor: 'curator',
      actorAddress: '0xabc',
      payload: {},
      createdAt: Date.parse('2026-09-29T10:00:00Z')
    })
  })
})

describe('toCurationQueryString', () => {
  const ME = '0xME'

  it('builds the committee list query for the picked kind of published collections', () => {
    const filters = parseCurationFilters(
      new URLSearchParams(
        'status=approved&status=to_review&assignee=0xABCDEF0123456789ABCDEF0123456789ABCDEF01&assignee=unassigned&review_type=updates&q=hat&sort=NAME_ASC&page=2&tag=Summer&type=standard'
      ),
      ME
    )
    const query = new URLSearchParams(toCurationQueryString(filters))
    // To review covers a pending request with or without a curator, which builder-server keeps apart.
    expect(query.getAll('status')).toEqual(['approved', 'to_review', 'under_review'])
    expect(query.getAll('assignee')).toEqual(['0xabcdef0123456789abcdef0123456789abcdef01', 'unassigned'])
    expect(Object.fromEntries(query)).toMatchObject({
      is_published: 'true',
      review_type: 'updates',
      type: 'standard',
      sort: 'NAME_ASC',
      q: 'hat',
      tag: 'summer',
      page: '2',
      limit: '12'
    })
  })

  it('leaves cleared filters out of the query, so standard and linked collections list together', () => {
    const query = new URLSearchParams(
      toCurationQueryString(parseCurationFilters(new URLSearchParams('status=all&assignee=all'), ME))
    )
    expect(query.has('status')).toBe(false)
    expect(query.has('assignee')).toBe(false)
    expect(query.has('review_type')).toBe(false)
    expect(query.has('tag')).toBe(false)
    expect(query.has('type')).toBe(false)
  })

  it('asks for third-party collections under the Linked type', () => {
    const query = new URLSearchParams(toCurationQueryString(parseCurationFilters(new URLSearchParams('type=linked'))))
    expect(query.get('type')).toBe('third_party')
  })
})

describe('fromRemoteItemCuration', () => {
  it('maps the wire shape', () => {
    expect(
      fromRemoteItemCuration({
        id: 'ic1',
        item_id: 'i1',
        status: CurationStatus.PENDING,
        content_hash: 'h',
        is_mapping_complete: null,
        created_at: '2026-01-01T00:00:00Z',
        updated_at: '2026-01-02T00:00:00Z'
      })
    ).toEqual({
      id: 'ic1',
      itemId: 'i1',
      status: 'pending',
      contentHash: 'h',
      isMappingComplete: null,
      createdAt: +new Date('2026-01-01T00:00:00Z'),
      updatedAt: +new Date('2026-01-02T00:00:00Z')
    })
  })
})

describe('fromRemoteThirdParty', () => {
  it('fills what a virtual third party lacks', () => {
    expect(fromRemoteThirdParty({ id: 'urn:tp', name: 'Brand', managers: ['0x1'] })).toEqual({
      id: 'urn:tp',
      root: '',
      managers: ['0x1'],
      name: 'Brand',
      description: '',
      isApproved: false,
      isProgrammatic: false,
      maxItems: '0',
      contracts: [],
      published: false
    })
  })
})

describe('fromRemoteLinkedApprovalData', () => {
  it('keeps the cheque and the root, and reads the leaves by item id', () => {
    const cheque = { signature: '0xsig', qty: 2, salt: '0xsalt' }
    expect(
      fromRemoteLinkedApprovalData({
        cheque,
        root: null,
        content_hashes: { i1: 'h1', i2: 'h2' },
        chequeWasConsumed: true
      })
    ).toEqual({ cheque, root: null, contentHashes: { i1: 'h1', i2: 'h2' }, chequeWasConsumed: true })
  })
})
