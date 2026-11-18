// @vitest-environment node
import { describe, expect, it, vi } from 'vitest'
import { type Entity } from '@dcl/schemas'
import { type Collection } from './collections'
import {
  ApprovalError,
  approveOnChain,
  deployItems,
  findItemsToDeploy,
  findItemsToRescue,
  rescueItems,
  waitForIndexer,
  type DeployDeps
} from './approveCollection'
import { computeItemContentHash, getEntityContent } from './catalystEntity'
import { ItemType, type Item } from './items'

const CHAIN_ID = 80002
const collection = { id: 'c1', contractAddress: '0x' + '2'.repeat(40), isApproved: false } as Collection

function item(id: string, overrides: Partial<Item> = {}): Item {
  return {
    id,
    name: `Item ${id}`,
    description: '',
    thumbnail: 'thumbnail.png',
    owner: '0xowner',
    isPublished: true,
    isApproved: true,
    inCatalyst: true,
    type: ItemType.WEARABLE,
    rarity: 'common',
    tokenId: id,
    urn: `urn:decentraland:amoy:collections-v2:${collection.contractAddress}:${id}`,
    contents: { 'thumbnail.png': 'bafthumb', 'image.png': 'bafimage', 'model.glb': `bafmodel${id}` },
    data: {
      category: 'hat',
      hides: [],
      replaces: [],
      tags: [],
      representations: [
        {
          bodyShapes: ['urn:decentraland:off-chain:base-avatars:BaseMale'],
          mainFile: 'model.glb',
          contents: ['model.glb'],
          overrideHides: [],
          overrideReplaces: []
        }
      ]
    },
    metrics: { triangles: 1 },
    createdAt: 0,
    updatedAt: 0,
    ...overrides
  }
}

function entityOf(subject: Item): Entity {
  return {
    id: 'e',
    version: 'v3',
    type: 'wearable',
    pointers: [subject.urn!.toUpperCase()],
    timestamp: 1,
    content: Object.entries(subject.contents).map(([file, hash]) => ({ file, hash })),
    metadata: { name: subject.name, description: subject.description, data: subject.data }
  } as Entity
}

describe('findItemsToRescue', () => {
  const noImage = async () => null

  it('rescues items whose server hash differs from the one on chain', async () => {
    const stale = item('1', { currentContentHash: 'new', blockchainContentHash: 'old' })
    const fresh = item('2', { currentContentHash: 'same', blockchainContentHash: 'same' })
    await expect(findItemsToRescue(collection, [stale, fresh], noImage)).resolves.toEqual([
      { item: stale, contentHash: 'new' }
    ])
  })

  it('hashes emotes locally and accepts an older Qm hash on chain', async () => {
    const emote = item('3', { type: ItemType.EMOTE, currentContentHash: 'server' })
    const content = getEntityContent(emote)
    const v0 = await computeItemContentHash(collection, emote, content, 'v0')
    const v1 = await computeItemContentHash(collection, emote, content, 'v1')
    await expect(findItemsToRescue(collection, [{ ...emote, blockchainContentHash: v0 }], noImage)).resolves.toEqual([])
    const targets = await findItemsToRescue(collection, [{ ...emote, blockchainContentHash: 'other' }], noImage)
    expect(targets.map(target => target.contentHash)).toEqual([v1])
  })
})

describe('rescueItems', () => {
  it('sends the chunks, then waits until the new hashes are indexed, through read errors', async () => {
    const stale = item('1', { blockchainContentHash: 'old' })
    const fetchItems = vi
      .fn()
      .mockResolvedValueOnce([stale])
      .mockRejectedValueOnce(new Error('503'))
      .mockResolvedValueOnce([{ ...stale, blockchainContentHash: 'new' }])
    const deps = {
      chainId: CHAIN_ID,
      sendTransaction: vi.fn().mockResolvedValue('0xtx'),
      waitForTransaction: vi.fn().mockResolvedValue(true),
      fetchItems,
      sleep: vi.fn().mockResolvedValue(undefined)
    }
    const items = await rescueItems(collection, [{ item: stale, contentHash: 'new' }], deps)
    expect(deps.sendTransaction).toHaveBeenCalledTimes(1)
    expect(items[0].blockchainContentHash).toBe('new')
    expect(fetchItems).toHaveBeenCalledTimes(3)
  })

  it('fails when a rescue transaction reverts', async () => {
    const deps = {
      chainId: CHAIN_ID,
      sendTransaction: vi.fn().mockResolvedValue('0xtx'),
      waitForTransaction: vi.fn().mockResolvedValue(false),
      fetchItems: vi.fn()
    }
    await expect(rescueItems(collection, [{ item: item('1'), contentHash: 'x' }], deps)).rejects.toMatchObject({
      reason: 'reverted'
    })
  })

  it('gives up when the indexer never catches up', async () => {
    const deps = {
      chainId: CHAIN_ID,
      sendTransaction: vi.fn().mockResolvedValue('0xtx'),
      waitForTransaction: vi.fn().mockResolvedValue(true),
      fetchItems: vi.fn().mockResolvedValue([item('1', { blockchainContentHash: 'old' })]),
      sleep: vi.fn().mockResolvedValue(undefined)
    }
    await expect(rescueItems(collection, [{ item: item('1'), contentHash: 'new' }], deps, 4000)).rejects.toMatchObject({
      reason: 'not_indexed'
    })
  })
})

describe('findItemsToDeploy', () => {
  it('deploys items with no entity or with a different one', () => {
    const synced = item('1')
    const changed = item('2')
    const missing = item('3')
    const entities = [entityOf(synced), entityOf({ ...changed, name: 'Old name' })]
    expect(findItemsToDeploy([synced, changed, missing], entities)).toEqual([changed, missing])
  })

  it('keeps only the items with no entity when deploying missing ones', () => {
    const changed = item('2')
    const missing = item('3')
    expect(findItemsToDeploy([changed, missing], [entityOf({ ...changed, name: 'Old name' })], true)).toEqual([missing])
  })

  it('deploys a tag-only edit together with its rescue, so the Catalyst never lags the chain', async () => {
    const approved = item('1')
    const content = getEntityContent(approved)
    const approvedHash = await computeItemContentHash(collection, approved, content, 'v1')
    const retagged = { ...approved, data: { ...approved.data, tags: ['pirate'] }, blockchainContentHash: approvedHash }
    const rescue = await findItemsToRescue(collection, [retagged], async () => null)
    expect(rescue.map(target => target.item)).toEqual([retagged])
    expect(findItemsToDeploy([retagged], [entityOf(approved)])).toEqual([retagged])
  })

  it('deploys corrected metrics together with their rescue', async () => {
    const approved = item('1', { metrics: { triangles: 0, materials: 0, textures: 0, meshes: 1 } })
    const approvedHash = await computeItemContentHash(collection, approved, getEntityContent(approved), 'v1')
    const repaired = {
      ...approved,
      metrics: { triangles: 540, materials: 1, textures: 1, meshes: 1 },
      blockchainContentHash: approvedHash
    }
    const deployed = entityOf(approved)
    deployed.metadata = { ...deployed.metadata, metrics: approved.metrics }
    const rescue = await findItemsToRescue(collection, [repaired], async () => null)
    expect(rescue.map(target => target.item)).toEqual([repaired])
    expect(findItemsToDeploy([repaired], [deployed])).toEqual([repaired])
  })
})

describe('deployItems', () => {
  function deps(overrides: Partial<DeployDeps> = {}): DeployDeps {
    return {
      sign: vi.fn(async () => [{ type: 'SIGNER', payload: '0xme', signature: '' }]),
      fetchAvailableContent: vi.fn().mockResolvedValue(new Set(['bafthumb', 'bafimage'])),
      fetchContent: vi.fn().mockResolvedValue(new Blob(['x'])),
      renderCatalystImage: vi.fn(),
      deployEntity: vi.fn().mockResolvedValue(undefined),
      ...overrides
    }
  }

  it('uploads only the files the Catalyst lacks and reports progress', async () => {
    const d = deps()
    const progress = vi.fn()
    const result = await deployItems(collection, [item('1')], d, progress)
    expect(result.deployed).toHaveLength(1)
    expect(d.fetchContent).toHaveBeenCalledTimes(1)
    expect(d.fetchContent).toHaveBeenCalledWith('bafmodel1')
    expect(progress).toHaveBeenCalledWith(1, 1)
  })

  it('keeps going past a failed item and returns it for a retry', async () => {
    const d = deps({ deployEntity: vi.fn().mockRejectedValueOnce(new Error('400')).mockResolvedValue(undefined) })
    const result = await deployItems(collection, [item('1'), item('2')], d)
    expect(result.failed).toEqual([{ item: expect.objectContaining({ id: '1' }), message: '400' }])
    expect(result.deployed.map(deployed => deployed.id)).toEqual(['2'])
  })

  it('stops when the session cannot sign', async () => {
    await expect(deployItems(collection, [item('1')], deps({ sign: async () => null }))).rejects.toBeInstanceOf(
      ApprovalError
    )
  })
})

describe('approveOnChain', () => {
  it('sends setApproved(true) and fails on a revert', async () => {
    const deps = {
      chainId: CHAIN_ID,
      sendTransaction: vi.fn().mockResolvedValue('0xtx'),
      waitForTransaction: vi.fn().mockResolvedValue(true)
    }
    await expect(approveOnChain(collection, deps)).resolves.toBe('0xtx')
    deps.waitForTransaction.mockResolvedValue(false)
    await expect(approveOnChain(collection, deps)).rejects.toMatchObject({ reason: 'reverted' })
  })
})

describe('waitForIndexer', () => {
  const sleep = vi.fn().mockResolvedValue(undefined)

  it('answers the first read that reflects the change, through read errors', async () => {
    const read = vi.fn().mockRejectedValueOnce(new Error('503')).mockResolvedValueOnce(false).mockResolvedValue(true)
    await expect(waitForIndexer(read, done => done === true, 10_000, sleep)).resolves.toBe(true)
    expect(read).toHaveBeenCalledTimes(3)
  })

  it('gives up after the timeout', async () => {
    await expect(
      waitForIndexer(vi.fn().mockResolvedValue(false), done => done === true, 4000, sleep)
    ).rejects.toMatchObject({
      reason: 'not_indexed'
    })
  })
})
