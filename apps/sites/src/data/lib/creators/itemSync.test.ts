// @vitest-environment jsdom
import '@/data/lib/creators/test/setup'
import { describe, expect, it } from 'vitest'
import { type Entity, EntityType } from '@dcl/schemas'
import { type Collection } from './collections'
import { ItemType, type Item } from './items'
import {
  ItemRowStatus,
  ItemSyncStatus,
  buildResetItem,
  getItemRowStatus,
  hasUpdatesSinceRejection,
  showsItemStatusColumn,
  getItemSyncStatus,
  isItemSynced,
  mapEntitiesByItemId
} from './itemSync'

const MALE = 'urn:decentraland:off-chain:base-avatars:BaseMale'
const URN = 'urn:decentraland:amoy:collections-v2:0xc0ffee:0'

const wearable: Item = {
  id: 'i1',
  name: 'Pirate Hat',
  description: 'Yarr',
  thumbnail: 'thumbnail.png',
  urn: URN,
  owner: '0xabc',
  collectionId: 'c1',
  rarity: 'legendary',
  isPublished: true,
  isApproved: true,
  inCatalyst: true,
  type: ItemType.WEARABLE,
  data: {
    category: 'hat',
    hides: [],
    replaces: ['helmet'],
    tags: ['pirate'],
    representations: [{ bodyShapes: [MALE], mainFile: 'hat.glb', contents: ['hat.glb', 'textures/'] }]
  },
  contents: {
    'hat.glb': 'Qmglb',
    'thumbnail.png': 'Qmthumb',
    'image.png': 'Qmimage',
    'textures/': 'bafkreihdwdcefgh4dqkjv67uzcmw7ojee6xedzdetojuzjevtenxquvyku'
  },
  createdAt: 1000,
  updatedAt: 1000
}

function entityFor(item: Item, metadataOverrides: Record<string, unknown> = {}, contents = item.contents): Entity {
  const deployed = Object.fromEntries(Object.entries(contents).filter(([path]) => !path.endsWith('/')))
  const data = {
    ...item.data,
    representations: item.data.representations.map(rep => ({
      ...rep,
      contents: rep.contents.filter(path => !path.endsWith('/'))
    }))
  }
  const metadata =
    item.type === ItemType.EMOTE
      ? { id: item.urn, name: item.name, description: item.description, emoteDataADR74: data }
      : { id: item.urn, name: item.name, description: item.description, data }
  return {
    version: 'v3',
    id: 'bafyentity',
    type: EntityType.WEARABLE,
    pointers: [item.urn!],
    timestamp: 1,
    content: Object.entries(deployed).map(([file, hash]) => ({ file, hash })),
    metadata: { ...metadata, ...metadataOverrides }
  }
}

const emote: Item = {
  ...wearable,
  id: 'e1',
  urn: `${URN}1`,
  type: ItemType.EMOTE,
  data: {
    category: 'dance',
    loop: true,
    tags: [],
    representations: [{ bodyShapes: [MALE], mainFile: 'e.glb', contents: ['e.glb'] }]
  },
  contents: { 'e.glb': 'Qme', 'thumbnail.png': 'Qmthumb' }
}

describe('isItemSynced', () => {
  it('is synced when metadata, representations and deployed files all match', () => {
    expect(isItemSynced(wearable, entityFor(wearable))).toBe(true)
    expect(isItemSynced(emote, entityFor(emote))).toBe(true)
  })

  it('ignores directory entries, which are never deployed', () => {
    const entity = entityFor(wearable)
    expect(entity.content.some(({ file }) => file === 'textures/')).toBe(false)
    expect(isItemSynced(wearable, entity)).toBe(true)
  })

  it('is out of sync after renaming or re-describing the item', () => {
    expect(isItemSynced({ ...wearable, name: 'Corsair Hat' }, entityFor(wearable))).toBe(false)
    expect(isItemSynced({ ...wearable, description: 'Arr' }, entityFor(wearable))).toBe(false)
  })

  it('is out of sync when a wearable field changes', () => {
    const entity = entityFor(wearable)
    expect(isItemSynced({ ...wearable, data: { ...wearable.data, category: 'helmet' } }, entity)).toBe(false)
    expect(isItemSynced({ ...wearable, data: { ...wearable.data, replaces: [] } }, entity)).toBe(false)
  })

  it('is out of sync after editing the tags, unless asked to ignore them', () => {
    const entity = entityFor(wearable)
    const retagged = { ...wearable, data: { ...wearable.data, tags: ['pirate', 'hat'] } }
    const untagged = { ...wearable, data: { ...wearable.data, tags: [] } }
    expect(isItemSynced(retagged, entity)).toBe(false)
    expect(isItemSynced(untagged, entity)).toBe(false)
    expect(isItemSynced(retagged, entity, { ignoreTags: true })).toBe(true)
    expect(isItemSynced(untagged, entity, { ignoreTags: true })).toBe(true)
  })

  it('is out of sync when the metrics were corrected, unless asked to ignore them', () => {
    const zeroed = {
      ...wearable,
      metrics: { triangles: 0, materials: 0, textures: 0, meshes: 1, bodies: 1, entities: 1 }
    }
    const entity = entityFor(zeroed, { metrics: zeroed.metrics })
    const repaired = { ...zeroed, metrics: { ...zeroed.metrics, triangles: 540, materials: 1, textures: 1 } }
    expect(isItemSynced(zeroed, entity)).toBe(true)
    expect(isItemSynced(repaired, entity)).toBe(false)
    expect(isItemSynced(repaired, entity, { ignoreMetrics: true })).toBe(true)
  })

  it('does not confuse a tag containing a comma with two tags', () => {
    expect(
      isItemSynced(
        { ...wearable, data: { ...wearable.data, tags: ['pirate,hat'] } },
        entityFor({ ...wearable, data: { ...wearable.data, tags: ['pirate', 'hat'] } })
      )
    ).toBe(false)
  })

  it('still catches any other change while ignoring the tags', () => {
    const entity = entityFor(wearable)
    const edited = { ...wearable, name: 'Corsair Hat', data: { ...wearable.data, tags: ['pirate', 'hat'] } }
    expect(isItemSynced(edited, entity, { ignoreTags: true })).toBe(false)
  })

  it('is out of sync when an emote changes its play mode', () => {
    expect(isItemSynced({ ...emote, data: { ...emote.data, loop: false } }, entityFor(emote))).toBe(false)
  })

  it('is out of sync when a file was replaced', () => {
    expect(
      isItemSynced({ ...wearable, contents: { ...wearable.contents, 'hat.glb': 'Qmnew' } }, entityFor(wearable))
    ).toBe(false)
  })

  it('is out of sync when a representation was added', () => {
    const FEMALE = 'urn:decentraland:off-chain:base-avatars:BaseFemale'
    const representations = [
      ...wearable.data.representations,
      { bodyShapes: [FEMALE], mainFile: 'hat.glb', contents: ['hat.glb'] }
    ]
    expect(isItemSynced({ ...wearable, data: { ...wearable.data, representations } }, entityFor(wearable))).toBe(false)
  })

  it('is out of sync when a representation gains or drops per-representation overrides', () => {
    const [rep] = wearable.data.representations
    const withOverrides = {
      ...wearable,
      data: { ...wearable.data, representations: [{ ...rep, overrideHides: ['hair'] }] }
    }
    expect(isItemSynced(withOverrides, entityFor(wearable))).toBe(false)
    expect(isItemSynced(wearable, entityFor(withOverrides))).toBe(false)
    expect(isItemSynced(withOverrides, entityFor(withOverrides))).toBe(true)
  })

  it('treats an emote deployed before ADR-74 as out of sync', () => {
    const legacy = entityFor(emote, { emoteDataADR74: undefined, data: emote.data })
    expect(isItemSynced(emote, legacy)).toBe(false)
  })
})

describe('getItemSyncStatus', () => {
  const loaded = { isCurationPending: false, curationLoaded: true, entitiesLoaded: true }

  it('is synced or unsynced by comparing with the entity', () => {
    expect(getItemSyncStatus(wearable, entityFor(wearable), loaded)).toBe(ItemSyncStatus.SYNCED)
    expect(getItemSyncStatus({ ...wearable, name: 'x' }, entityFor(wearable), loaded)).toBe(ItemSyncStatus.UNSYNCED)
  })

  it('reads a tag-only edit as synced when asked to ignore tags, whatever the curation says', () => {
    const retagged = { ...wearable, data: { ...wearable.data, tags: ['pirate', 'hat'] } }
    expect(getItemSyncStatus(retagged, entityFor(wearable), loaded)).toBe(ItemSyncStatus.UNSYNCED)
    const ignoring = { ...loaded, ignoreTags: true }
    expect(getItemSyncStatus(retagged, entityFor(wearable), ignoring)).toBe(ItemSyncStatus.SYNCED)
    expect(getItemSyncStatus(retagged, entityFor(wearable), { ...ignoring, isCurationPending: true })).toBe(
      ItemSyncStatus.SYNCED
    )
  })

  it('is under review, not unsynced, while the committee has a pending request for the collection', () => {
    const status = getItemSyncStatus({ ...wearable, name: 'x' }, entityFor(wearable), {
      ...loaded,
      isCurationPending: true
    })
    expect(status).toBe(ItemSyncStatus.UNDER_REVIEW)
  })

  it('follows the publication flags when nothing is deployed', () => {
    expect(getItemSyncStatus({ ...wearable, isPublished: false }, undefined, loaded)).toBe(ItemSyncStatus.UNPUBLISHED)
    expect(getItemSyncStatus({ ...wearable, isApproved: false }, undefined, loaded)).toBe(ItemSyncStatus.UNDER_REVIEW)
  })

  it('is loading until the curation request settles, so an edited item never flashes as unsynced first', () => {
    const context = { ...loaded, curationLoaded: false }
    expect(getItemSyncStatus({ ...wearable, name: 'x' }, entityFor(wearable), context)).toBe(ItemSyncStatus.LOADING)
    expect(getItemSyncStatus(wearable, undefined, context)).toBe(ItemSyncStatus.LOADING)
    expect(getItemSyncStatus({ ...wearable, isPublished: false }, undefined, context)).toBe(ItemSyncStatus.UNPUBLISHED)
  })

  it('is unsynced when an approved item has no entity, but loading until the entities arrive', () => {
    expect(getItemSyncStatus(wearable, undefined, loaded)).toBe(ItemSyncStatus.UNSYNCED)
    expect(getItemSyncStatus(wearable, undefined, { ...loaded, entitiesLoaded: false })).toBe(ItemSyncStatus.LOADING)
  })
})

describe('getItemRowStatus', () => {
  const entity = entityFor(wearable)
  const rejected = { status: 'rejected' as const, updatedAt: 2000 }
  const pending = { status: 'pending' as const, updatedAt: 2000 }
  const status = (sync: Parameters<typeof getItemRowStatus>[1], curation: Parameters<typeof getItemRowStatus>[2]) =>
    getItemRowStatus(wearable, sync, curation)

  it('names the sync state the creator can act on', () => {
    expect(status({ status: ItemSyncStatus.SYNCED, entity }, null)).toBe(ItemRowStatus.PUBLISHED)
    expect(status({ status: ItemSyncStatus.UNSYNCED, entity }, null)).toBe(ItemRowStatus.MODIFIED)
    expect(status({ status: ItemSyncStatus.UNSYNCED, entity }, { status: 'approved', updatedAt: 1 })).toBe(
      ItemRowStatus.MODIFIED
    )
    expect(status({ status: ItemSyncStatus.UNDER_REVIEW, entity }, pending)).toBe(ItemRowStatus.UNDER_REVIEW)
  })

  it('reads rejected only for edits the committee saw: a later edit is modified again', () => {
    const unsynced = { status: ItemSyncStatus.UNSYNCED, entity }
    expect(status(unsynced, rejected)).toBe(ItemRowStatus.REJECTED)
    expect(getItemRowStatus({ updatedAt: 3000 }, unsynced, rejected)).toBe(ItemRowStatus.MODIFIED)
  })

  it('flags an approved item with no entity as missing, or under review while its redeploy is being reviewed', () => {
    expect(status({ status: ItemSyncStatus.UNSYNCED }, null)).toBe(ItemRowStatus.MISSING)
    expect(status({ status: ItemSyncStatus.UNSYNCED }, rejected)).toBe(ItemRowStatus.MISSING)
    expect(status({ status: ItemSyncStatus.UNSYNCED }, pending)).toBe(ItemRowStatus.UNDER_REVIEW)
  })

  it('leaves an unapproved item alone outside a review: the collection pill says rejected or disabled', () => {
    expect(status({ status: ItemSyncStatus.UNDER_REVIEW }, rejected)).toBeNull()
    expect(status({ status: ItemSyncStatus.UNDER_REVIEW }, null)).toBeNull()
  })

  it('says nothing while the sync is unknown or the item is not published', () => {
    expect(status(undefined, null)).toBeNull()
    expect(status({ status: ItemSyncStatus.LOADING }, null)).toBeNull()
    expect(status({ status: ItemSyncStatus.UNPUBLISHED }, null)).toBeNull()
  })
})

describe('showsItemStatusColumn', () => {
  const collection: Collection = {
    id: 'c1',
    name: 'Hats',
    owner: '0xowner',
    urn: 'urn',
    isPublished: true,
    isApproved: true,
    itemCount: 1,
    minters: [],
    managers: [],
    createdAt: 1,
    updatedAt: 1
  }

  it('shows the column only when some row says more than published', () => {
    expect(showsItemStatusColumn(collection, [ItemRowStatus.PUBLISHED, ItemRowStatus.MODIFIED])).toBe(true)
    expect(showsItemStatusColumn(collection, [ItemRowStatus.PUBLISHED, ItemRowStatus.PUBLISHED])).toBe(false)
    expect(showsItemStatusColumn(collection, [null, ItemRowStatus.PUBLISHED])).toBe(false)
  })

  it('never shows it until the collection is approved on chain, where the collection pill tells the whole story', () => {
    const underFirstReview = { ...collection, isApproved: false }
    expect(showsItemStatusColumn(underFirstReview, [ItemRowStatus.UNDER_REVIEW])).toBe(false)
    // The rescue step of a first approval stamps reviewedAt before the deploy and the on-chain approval.
    const rescued = { ...underFirstReview, createdAt: 1, reviewedAt: 2 }
    expect(showsItemStatusColumn(rescued, [ItemRowStatus.UNDER_REVIEW])).toBe(false)
    expect(showsItemStatusColumn({ ...rescued, isApproved: true }, [ItemRowStatus.MODIFIED])).toBe(true)
  })
})

describe('mapEntitiesByItemId', () => {
  it('pairs each entity with the item sharing its URN', () => {
    const map = mapEntitiesByItemId([wearable, emote], [entityFor(emote)])
    expect(map.get('e1')?.pointers).toEqual([emote.urn])
    expect(map.has('i1')).toBe(false)
  })
})

describe('buildResetItem', () => {
  it('takes name, description, item data and files from the deployed entity', () => {
    const edited: Item = {
      ...wearable,
      name: 'Corsair Hat',
      description: 'Arr',
      data: { ...wearable.data, category: 'helmet' },
      contents: { ...wearable.contents, 'hat.glb': 'Qmnew', 'extra.png': 'Qmextra' }
    }
    const reset = buildResetItem(edited, entityFor(wearable))
    expect(reset).toMatchObject({ id: 'i1', name: 'Pirate Hat', description: 'Yarr', rarity: 'legendary' })
    expect(reset.data.category).toBe('hat')
    expect(reset.contents).toEqual({ 'hat.glb': 'Qmglb', 'thumbnail.png': 'Qmthumb', 'image.png': 'Qmimage' })
  })

  it('keeps the tags edited in the builder, which are live without a review', () => {
    const edited: Item = { ...wearable, data: { ...wearable.data, category: 'helmet', tags: ['pirate', 'hat'] } }
    const reset = buildResetItem(edited, entityFor(wearable))
    expect(reset.data.tags).toEqual(['pirate', 'hat'])
  })

  it('falls back to the deployed tags when the builder item has none', () => {
    const data = { ...wearable.data }
    delete data.tags
    const reset = buildResetItem({ ...wearable, data }, entityFor(wearable))
    expect(reset.data.tags).toEqual(wearable.data.tags)
  })

  it('keeps the smart wearable video and permissions the Catalyst never receives, so the reset item is synced', () => {
    const smart: Item = {
      ...wearable,
      video: 'Qmvideo',
      data: { ...wearable.data, requiredPermissions: ['USE_FETCH'] },
      contents: { ...wearable.contents, 'game.js': 'Qmjs', 'video.mp4': 'Qmvideo' }
    }
    const deployedContents = Object.fromEntries(Object.entries(smart.contents).filter(([path]) => path !== 'video.mp4'))
    const entity = entityFor({ ...smart, data: wearable.data }, {}, deployedContents)
    const reset = buildResetItem({ ...smart, contents: { ...smart.contents, 'game.js': 'Qmedited' } }, entity)
    expect(reset.contents['video.mp4']).toBe('Qmvideo')
    expect(reset.data.requiredPermissions).toEqual(['USE_FETCH'])
    expect(isItemSynced(reset, entity)).toBe(true)
  })

  it('discards a video uploaded after approval, going back to the approved one', () => {
    const smart: Item = {
      ...wearable,
      isPublished: true,
      isApproved: true,
      video: 'Qmvideo',
      contents: { ...wearable.contents, 'game.js': 'Qmjs', 'video.mp4': 'Qmpending' }
    }
    const deployedContents = { ...wearable.contents, 'game.js': 'Qmjs' }
    const entity = entityFor(smart, {}, deployedContents)
    const reset = buildResetItem(smart, entity)
    expect(reset.contents['video.mp4']).toBe('Qmvideo')
    expect(isItemSynced(reset, entity)).toBe(true)
  })

  it('takes the ADR-74 data for an emote', () => {
    const reset = buildResetItem({ ...emote, data: { ...emote.data, loop: false } }, entityFor(emote))
    expect(reset.data.loop).toBe(true)
  })
})

describe('hasUpdatesSinceRejection', () => {
  const rejected = { status: 'rejected' as const, updatedAt: 100 }

  it('finds an update once an item was saved or added after the request was sent back', () => {
    expect(hasUpdatesSinceRejection(rejected, [{ updatedAt: 50 }, { updatedAt: 150 }])).toBe(true)
    expect(hasUpdatesSinceRejection(rejected, [{ updatedAt: 50 }, { updatedAt: 100 }])).toBe(false)
  })

  it('finds none on a request that was not sent back', () => {
    expect(hasUpdatesSinceRejection({ status: 'pending', updatedAt: 100 }, [{ updatedAt: 150 }])).toBe(false)
    expect(hasUpdatesSinceRejection(null, [{ updatedAt: 150 }])).toBe(false)
  })
})
