// @vitest-environment node
import { describe, expect, it, vi } from 'vitest'
import { generateTree, verifyProof } from '@dcl/content-hash-tree'
import { MappingType } from '@dcl/schemas'
import { ApprovalError } from './approveCollection'
import { computeLinkedItemContentHash } from './catalystEntity'
import {
  deployLinkedItems,
  getReviewThreshold,
  prepareLinkedApproval,
  setLinkedMerkleRoot,
  type LinkedApprovalPlan,
  type LinkedDeployDeps
} from './approveLinkedCollection'
import { type LinkedApprovalData } from './builder'
import { CurationStatus, type Collection } from './collections'
import { ItemType, type Item } from './items'
import { type ThirdParty } from './thirdParty'

const CHAIN_ID = 80002
const THIRD_PARTY_ID = 'urn:decentraland:amoy:collections-thirdparty:brand'
const collection = { id: 'lc1', urn: `${THIRD_PARTY_ID}:lc1`, isPublished: true } as Collection
const SIGNATURE = `0x${'11'.repeat(32)}${'22'.repeat(32)}1b`

function item(id: string, overrides: Partial<Item> = {}): Item {
  const built: Item = {
    id,
    name: `Item ${id}`,
    description: '',
    thumbnail: 'thumbnail.png',
    owner: '0xowner',
    isPublished: true,
    isApproved: false,
    inCatalyst: false,
    type: ItemType.WEARABLE,
    urn: `${collection.urn}:${id}`,
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
    mappings: { amoy: { '0xabc': [{ type: MappingType.ANY }] } },
    createdAt: 0,
    updatedAt: 0,
    ...overrides
  }
  // As builder-server stores it: the item's own content hash, unless the case says otherwise.
  if (!('currentContentHash' in overrides) && built.urn) {
    built.currentContentHash = computeLinkedItemContentHash(collection, built)
  }
  return built
}

const hashOf = (id: string) => `${'0'.repeat(60)}${id.padStart(4, '0')}`

function approvalData(overrides: Partial<LinkedApprovalData> = {}): LinkedApprovalData {
  return {
    cheque: { signature: SIGNATURE, qty: 2, salt: '0xsalt' },
    root: null,
    // Items 1 and 2 are pending; 9 was approved in an earlier publish and 8 was rejected.
    contentHashes: {
      '1': item('1').currentContentHash!,
      '2': item('2').currentContentHash!,
      '9': hashOf('9'),
      '8': hashOf('8')
    },
    chequeWasConsumed: false,
    ...overrides
  }
}

function curationOf(id: string, status: CurationStatus) {
  return { id: `ic${id}`, itemId: id, status, contentHash: '', isMappingComplete: true, createdAt: 0, updatedAt: 0 }
}

function prepareDeps(items: Item[], data: LinkedApprovalData) {
  const curations = new Map([
    ['1', curationOf('1', CurationStatus.PENDING)],
    ['2', curationOf('2', CurationStatus.PENDING)],
    ['9', curationOf('9', CurationStatus.APPROVED)],
    ['8', curationOf('8', CurationStatus.REJECTED)]
  ])
  return {
    fetchPendingItems: vi.fn().mockResolvedValue(items),
    fetchApprovalData: vi.fn().mockResolvedValue(data),
    fetchItemCurations: vi.fn().mockResolvedValue(curations),
    fetchRegistryRoot: vi.fn().mockResolvedValue(''),
    // Item 9 was approved and deployed with the content its curation holds.
    fetchDeployedHashes: vi.fn().mockResolvedValue(new Map([['9', hashOf('9')]]))
  }
}

// The tree the approval must build: approved and pending content only.
function expectedRoot(data: LinkedApprovalData, pending: string[]) {
  const leaves = Object.entries(data.contentHashes)
    .filter(([id]) => pending.includes(id) || id === '9')
    .map(([, hash]) => hash)
  return generateTree(leaves).merkleRoot
}

describe('getReviewThreshold', () => {
  it.each([
    [0, 0],
    [1, 1],
    [49, 49],
    [50, 50],
    [4999, 50],
    [5000, 50],
    [5001, 51],
    [10000, 100],
    [29999, 300],
    [30000, 300],
    [100000, 300]
  ])('asks for %i of %i items', (total, expected) => {
    expect(getReviewThreshold(total)).toBe(expected)
  })
})

describe('prepareLinkedApproval', () => {
  it('builds the tree over approved and pending content, leaving rejected content out, and proves each pending item', async () => {
    const data = approvalData()
    const plan = await prepareLinkedApproval(collection, prepareDeps([item('1'), item('2')], data))
    expect(plan.merkleRoot).toBe(expectedRoot(data, ['1', '2']))
    expect(plan.merkleRoot).not.toBe(generateTree(Object.values(data.contentHashes)).merkleRoot)
    expect(plan.needsRoot).toBe(true)
    expect(plan.pendingItems.map(pending => pending.id)).toEqual(['1', '2'])
    for (const id of ['1', '2']) {
      const { index, proof, entityHash } = plan.proofOf(id)
      expect(entityHash).toBe(data.contentHashes[id])
      const buffers = proof.map(node => Buffer.from(node.slice(2), 'hex'))
      expect(verifyProof(index, entityHash, buffers, Buffer.from(plan.merkleRoot.slice(2), 'hex'))).toBe(true)
    }
  })

  it('fails closed before any root when a pending item is not the curated content', async () => {
    // Edited after publishing without pushing: only the creator submitting it again can unblock it.
    const edited = item('1', { name: 'Edited later' })
    await expect(
      prepareLinkedApproval(collection, prepareDeps([edited, item('2')], approvalData()))
    ).rejects.toMatchObject({ reason: 'unsubmitted' })
    // The server's hash still matches, but the content on screen does not.
    const drifted = item('1', { name: 'Drifted', currentContentHash: item('1').currentContentHash })
    await expect(
      prepareLinkedApproval(collection, prepareDeps([drifted, item('2')], approvalData()))
    ).rejects.toMatchObject({ reason: 'changed' })
  })

  it('leaves out an approved hash the Catalyst does not hold, such as content saved while it was being approved', async () => {
    const data = approvalData()
    const deps = prepareDeps([item('1'), item('2')], data)
    deps.fetchDeployedHashes.mockResolvedValue(new Map([['9', 'reviewed-content']]))
    const plan = await prepareLinkedApproval(collection, deps)
    const pendingOnly = generateTree([data.contentHashes['1'], data.contentHashes['2']]).merkleRoot
    expect(plan.merkleRoot).toBe(pendingOnly)
  })

  it('accepts a retry once some reviewed items were approved by an earlier attempt', async () => {
    const deps = prepareDeps([item('2')], approvalData())
    deps.fetchItemCurations.mockResolvedValue(
      new Map([
        ['1', curationOf('1', CurationStatus.APPROVED)],
        ['2', curationOf('2', CurationStatus.PENDING)],
        ['9', curationOf('9', CurationStatus.APPROVED)]
      ])
    )
    await expect(
      prepareLinkedApproval(collection, deps, {
        pendingIds: new Set(['1', '2']),
        // Both were opened; '1' was deployed and approved by the first attempt.
        reviewedHashes: new Map([
          ['1', item('1').currentContentHash],
          ['2', item('2').currentContentHash]
        ])
      })
    ).resolves.toMatchObject({ pendingItems: [expect.objectContaining({ id: '2' })] })
  })

  it('fails closed when the pending items are not the ones the curator reviewed', async () => {
    const pending = [item('1'), item('2')]
    const deps = prepareDeps(pending, approvalData())
    const reviewed = new Map([['1', pending[0].currentContentHash]])

    await expect(
      prepareLinkedApproval(collection, deps, { pendingIds: new Set(['1', '2']), reviewedHashes: reviewed })
    ).resolves.toBeDefined()
    // Another item was published after the curator looked.
    await expect(
      prepareLinkedApproval(collection, deps, { pendingIds: new Set(['1']), reviewedHashes: reviewed })
    ).rejects.toMatchObject({ reason: 'changed' })
    // An opened item was edited since.
    await expect(
      prepareLinkedApproval(collection, deps, {
        pendingIds: new Set(['1', '2']),
        reviewedHashes: new Map([['1', 'h0']])
      })
    ).rejects.toMatchObject({ reason: 'changed' })
  })

  it('consumes the cheque with the root, unless it was consumed already', async () => {
    const fresh = await prepareLinkedApproval(collection, prepareDeps([item('1')], approvalData()))
    expect(fresh.slots).toEqual([['2', '0xsalt', `0x${'11'.repeat(32)}`, `0x${'22'.repeat(32)}`, 27]])

    const consumed = await prepareLinkedApproval(
      collection,
      prepareDeps([item('1')], approvalData({ chequeWasConsumed: true }))
    )
    expect(consumed.slots).toEqual([])
  })

  it('skips the root transaction when the chain already holds this root', async () => {
    const data = approvalData()
    const root = expectedRoot(data, ['1'])
    const plan = await prepareLinkedApproval(
      collection,
      prepareDeps([item('1')], { ...data, root: root.toUpperCase() })
    )
    expect(plan.needsRoot).toBe(false)
  })

  it('reads the root from the registry, so a retry after a slow indexer does not resend a consumed cheque', async () => {
    const data = approvalData()
    const deps = prepareDeps([item('1')], data)
    deps.fetchRegistryRoot.mockResolvedValue(expectedRoot(data, ['1']))
    const plan = await prepareLinkedApproval(collection, deps)
    expect(plan.needsRoot).toBe(false)
    expect(plan.slots).toEqual([])
  })

  it('refuses a cheque that covers fewer new items than are pending', async () => {
    const data = approvalData({ cheque: { signature: SIGNATURE, qty: 1, salt: '0xsalt' } })
    await expect(prepareLinkedApproval(collection, prepareDeps([item('1'), item('2')], data))).rejects.toMatchObject({
      reason: 'invalid_cheque'
    })
    // Items the Catalyst already holds take no slot, and a consumed cheque paid for its slots already.
    await expect(
      prepareLinkedApproval(collection, prepareDeps([item('1'), item('2', { isApproved: true })], data))
    ).resolves.toBeDefined()
    await expect(
      prepareLinkedApproval(collection, prepareDeps([item('1'), item('2')], { ...data, chequeWasConsumed: true }))
    ).resolves.toBeDefined()
  })

  it('refuses, before any root, items with a preview video the content servers would reject', async () => {
    const withVideo = item('1', { contents: { ...item('1').contents, 'video.mp4': 'bafvideo' } })
    const data = approvalData({
      contentHashes: { ...approvalData().contentHashes, '1': withVideo.currentContentHash! }
    })
    await expect(prepareLinkedApproval(collection, prepareDeps([withVideo, item('2')], data))).rejects.toMatchObject({
      reason: 'has_video',
      items: [withVideo]
    })
  })

  it('refuses when there is nothing pending or a pending item was never hashed', async () => {
    await expect(prepareLinkedApproval(collection, prepareDeps([], approvalData()))).rejects.toMatchObject({
      reason: 'nothing_pending'
    })
    await expect(prepareLinkedApproval(collection, prepareDeps([item('3')], approvalData()))).rejects.toMatchObject({
      reason: 'missing_hash'
    })
  })
})

// The curated hash of an item is the one builder-server computes from it; items that can't be hashed get a stand-in.
function curatedHash(target: Item): string {
  try {
    return computeLinkedItemContentHash(collection, target)
  } catch {
    return hashOf(target.id)
  }
}

// What the server answers for an item while deploying: the items of the latest plan, as they were planned.
let served: Item[] = []

function planFor(items: Item[], overrides: Partial<LinkedApprovalPlan> = {}): LinkedApprovalPlan {
  served = items
  const hashes = new Map([...items.map(target => [target.id, curatedHash(target)] as const), ['9', hashOf('9')]])
  const data = approvalData({ contentHashes: Object.fromEntries(hashes) })
  const tree = generateTree(Object.values(data.contentHashes))
  return {
    pendingItems: items,
    approvalData: data,
    merkleRoot: tree.merkleRoot,
    needsRoot: true,
    slots: [],
    proofOf: id => ({ ...tree.proofs[hashes.get(id)!], entityHash: hashes.get(id)! }),
    ...overrides
  }
}

const thirdParty = (root: string) => ({ id: THIRD_PARTY_ID, root }) as ThirdParty

describe('setLinkedMerkleRoot', () => {
  it('sends the root with the slots and waits until builder-server reads it back', async () => {
    const plan = planFor([item('1')], { slots: [['2', '0xsalt', '0xr', '0xs', 27]] })
    const deps = {
      chainId: CHAIN_ID,
      sendTransaction: vi.fn().mockResolvedValue('0xtx'),
      settleTransaction: vi.fn().mockResolvedValue('mined'),
      fetchThirdParty: vi
        .fn()
        .mockResolvedValueOnce(thirdParty(''))
        .mockRejectedValueOnce(new Error('offline'))
        .mockResolvedValue(thirdParty(plan.merkleRoot.toUpperCase())),
      fetchRegistryRoot: vi.fn().mockResolvedValue(''),
      sleep: vi.fn().mockResolvedValue(undefined)
    }
    await expect(setLinkedMerkleRoot(THIRD_PARTY_ID, plan, deps, 60_000)).resolves.toBe('0xtx')
    expect(deps.sendTransaction).toHaveBeenCalledWith(
      expect.objectContaining({
        method: 'reviewThirdPartyWithRoot',
        args: [THIRD_PARTY_ID, plan.merkleRoot, [['2', '0xsalt', '0xr', '0xs', 27]]]
      })
    )
    expect(deps.fetchThirdParty).toHaveBeenCalledTimes(3)
  })

  it('fails on a reverted or still pending transaction, and when the root is never indexed', async () => {
    const plan = planFor([item('1')])
    const deps = {
      chainId: CHAIN_ID,
      sendTransaction: vi.fn().mockResolvedValue('0xtx'),
      settleTransaction: vi.fn().mockResolvedValue('reverted'),
      fetchThirdParty: vi.fn().mockResolvedValue(thirdParty('0xold')),
      fetchRegistryRoot: vi.fn().mockResolvedValue(''),
      sleep: vi.fn().mockResolvedValue(undefined)
    }
    await expect(setLinkedMerkleRoot(THIRD_PARTY_ID, plan, deps)).rejects.toMatchObject({ reason: 'reverted' })

    deps.settleTransaction.mockResolvedValue('pending')
    await expect(setLinkedMerkleRoot(THIRD_PARTY_ID, plan, deps)).rejects.toMatchObject({ reason: 'not_indexed' })
    expect(deps.fetchThirdParty).not.toHaveBeenCalled()

    deps.settleTransaction.mockResolvedValue('mined')
    await expect(setLinkedMerkleRoot(THIRD_PARTY_ID, plan, deps, 0)).rejects.toMatchObject({ reason: 'not_indexed' })
  })

  it('waits on the root an earlier attempt sent instead of sending another with the same cheque', async () => {
    const plan = planFor([item('1')], { slots: [['2', '0xsalt', '0xr', '0xs', 27]] })
    const deps = {
      chainId: CHAIN_ID,
      previousTx: { hash: '0xearlier', merkleRoot: plan.merkleRoot },
      sendTransaction: vi.fn().mockResolvedValue('0xnew'),
      settleTransaction: vi.fn().mockResolvedValue('mined'),
      fetchThirdParty: vi.fn().mockResolvedValue(thirdParty(plan.merkleRoot)),
      fetchRegistryRoot: vi.fn().mockResolvedValue(''),
      sleep: vi.fn().mockResolvedValue(undefined)
    }
    await expect(setLinkedMerkleRoot(THIRD_PARTY_ID, plan, deps)).resolves.toBe('0xearlier')
    expect(deps.sendTransaction).not.toHaveBeenCalled()

    deps.settleTransaction.mockResolvedValueOnce('pending')
    await expect(setLinkedMerkleRoot(THIRD_PARTY_ID, plan, deps)).rejects.toMatchObject({ reason: 'not_indexed' })
    expect(deps.sendTransaction).not.toHaveBeenCalled()

    // One that reverted can never land: a new one goes out.
    deps.settleTransaction.mockResolvedValueOnce('reverted')
    await expect(setLinkedMerkleRoot(THIRD_PARTY_ID, plan, deps)).resolves.toBe('0xnew')
  })

  it('sends nothing more when an earlier root was replaced by a speed-up that already set it', async () => {
    const plan = planFor([item('1')])
    const deps = {
      chainId: CHAIN_ID,
      previousTx: { hash: '0xearlier', merkleRoot: plan.merkleRoot },
      sendTransaction: vi.fn().mockResolvedValue('0xnew'),
      settleTransaction: vi.fn().mockResolvedValue('dropped'),
      // builder-server hasn't indexed it yet; the registry already holds it.
      fetchThirdParty: vi.fn().mockResolvedValue(thirdParty('')),
      fetchRegistryRoot: vi.fn().mockResolvedValue(plan.merkleRoot),
      sleep: vi.fn().mockResolvedValue(undefined)
    }
    await expect(setLinkedMerkleRoot(THIRD_PARTY_ID, plan, deps)).resolves.toBe('0xearlier')
    expect(deps.sendTransaction).not.toHaveBeenCalled()
  })

  it('ignores an earlier root transaction for another root, and records the one it sends', async () => {
    const plan = planFor([item('1')])
    const onSent = vi.fn()
    const deps = {
      chainId: CHAIN_ID,
      previousTx: { hash: '0xstale', merkleRoot: '0x' + 'cd'.repeat(32) },
      sendTransaction: vi.fn().mockResolvedValue('0xnew'),
      settleTransaction: vi.fn().mockResolvedValue('mined'),
      fetchThirdParty: vi.fn().mockResolvedValue(thirdParty(plan.merkleRoot)),
      onSent,
      fetchRegistryRoot: vi.fn().mockResolvedValue(''),
      sleep: vi.fn().mockResolvedValue(undefined)
    }
    await expect(setLinkedMerkleRoot(THIRD_PARTY_ID, plan, deps)).resolves.toBe('0xnew')
    expect(deps.settleTransaction).not.toHaveBeenCalledWith('0xstale')
    expect(onSent).toHaveBeenCalledWith({ hash: '0xnew', merkleRoot: plan.merkleRoot })
  })

  it('records who sent the root once the watch sees it, so a later attempt can tell it was replaced', async () => {
    const plan = planFor([item('1')])
    const onSent = vi.fn()
    const sender = { from: '0x' + 'aa'.repeat(20), nonce: 2 }
    const deps = {
      chainId: CHAIN_ID,
      sendTransaction: vi.fn().mockResolvedValue('0xnew'),
      settleTransaction: vi.fn(async (_hash: string, _sender: unknown, onSeen?: (seen: typeof sender) => void) => {
        onSeen?.(sender)
        return 'pending' as const
      }),
      fetchThirdParty: vi.fn(),
      fetchRegistryRoot: vi.fn().mockResolvedValue(''),
      onSent
    }
    await expect(setLinkedMerkleRoot(THIRD_PARTY_ID, plan, deps)).rejects.toMatchObject({ reason: 'not_indexed' })
    expect(onSent).toHaveBeenLastCalledWith({ hash: '0xnew', merkleRoot: plan.merkleRoot, sender })
  })
})

function deployDeps(overrides: Partial<LinkedDeployDeps> = {}): LinkedDeployDeps {
  return {
    fetchContent: vi.fn(async (hash: string) => new Blob([hash])),
    sign: vi.fn(async () => [{ type: 'SIGNER', payload: '0xme' }]),
    fetchAvailableContent: vi.fn(async () => new Set<string>()),
    deployEntity: vi.fn(async () => undefined),
    fetchItem: vi.fn(async (id: string) => served.find(target => target.id === id)!),
    approveItem: vi.fn(async (id: string) => ({ contentHash: curatedHash(served.find(target => target.id === id)!) })),
    rejectItem: vi.fn(async () => undefined),
    ...overrides
  }
}

describe('deployLinkedItems', () => {
  it('uploads only what the Catalyst lacks, then approves the item curation', async () => {
    const deps = deployDeps({ fetchAvailableContent: vi.fn(async () => new Set(['bafthumb', 'bafimage'])) })
    const result = await deployLinkedItems(collection, planFor([item('1')]), deps)
    expect(result.failed).toEqual([])
    expect(result.deployed.map(deployed => deployed.id)).toEqual(['1'])
    expect(deps.fetchContent).toHaveBeenCalledTimes(1)
    expect(deps.fetchContent).toHaveBeenCalledWith('bafmodel1')
    const form = vi.mocked(deps.deployEntity).mock.calls[0][0]
    expect(form.has('bafmodel1')).toBe(true)
    expect(form.has('bafthumb')).toBe(false)
    expect(deps.approveItem).toHaveBeenCalledWith('1')
  })

  it('uploads exactly the content its metadata declares, generating no image for an item saved without one', async () => {
    const contents = { 'thumbnail.png': 'bafthumb', 'model.glb': 'bafmodel1' }
    const deps = deployDeps()
    const result = await deployLinkedItems(collection, planFor([item('1', { contents })]), deps)
    expect(result.failed).toEqual([])
    const form = vi.mocked(deps.deployEntity).mock.calls[0][0]
    const entity = JSON.parse(await (form.get(form.get('entityId') as string) as Blob).text())
    const uploaded = Object.fromEntries(
      entity.content.map((entry: { file: string; hash: string }) => [entry.file, entry.hash])
    )
    // The Catalyst refuses any difference between the two, in either direction.
    expect(uploaded).toEqual(entity.metadata.content)
  })

  it('takes back an approval recorded with content saved during the approval, and reports the item', async () => {
    const deps = deployDeps({ approveItem: vi.fn(async () => ({ contentHash: 'saved-meanwhile' })) })
    const result = await deployLinkedItems(collection, planFor([item('1')]), deps)
    expect(deps.rejectItem).toHaveBeenCalledWith('1')
    expect(result.failed.map(({ item, stage }) => [item.id, stage])).toEqual([['1', 'changed']])
  })

  it('records each failure with the stage it happened at and carries on with the rest', async () => {
    const deps = deployDeps({
      deployEntity: vi.fn(async (form: FormData) => {
        if (form.has('bafmodel2')) throw new Error('catalyst down')
      }),
      approveItem: vi.fn(async (id: string) => {
        if (id === '3') throw new Error('curation 500')
        return { contentHash: curatedHash(served.find(target => target.id === id)!) }
      })
    })
    const items = [item('1'), item('2'), item('3'), item('4', { urn: undefined })]
    const progress = vi.fn()
    const result = await deployLinkedItems(collection, planFor(items), deps, progress)
    expect(result.deployed.map(deployed => deployed.id)).toEqual(['1'])
    expect(result.failed.map(({ item, stage, message }) => [item.id, stage, message])).toEqual(
      expect.arrayContaining([
        ['2', 'deploy', 'catalyst down'],
        ['3', 'curation', 'curation 500'],
        ['4', 'build', expect.stringContaining('not a published item')]
      ])
    )
    expect(progress).toHaveBeenLastCalledWith(4, 4)
    expect(progress).toHaveBeenCalledTimes(4)
  })

  it('stops before signing when an item is not the curated content, so the curator reviews again', async () => {
    const plan = planFor([item('1')])
    const edited = { ...item('1'), name: 'Edited after curation' }
    const deps = deployDeps()
    await expect(deployLinkedItems(collection, { ...plan, pendingItems: [edited] }, deps)).rejects.toMatchObject({
      reason: 'changed'
    })
    expect(deps.sign).not.toHaveBeenCalled()
    expect(deps.deployEntity).not.toHaveBeenCalled()
  })

  it('leaves an item the creator saves mid-run pending instead of approving the new content', async () => {
    const plan = planFor([item('1'), item('2')])
    const editedLater = { ...item('2'), currentContentHash: 'new-hash' }
    let reads = 0
    const deps = deployDeps({
      fetchItem: vi.fn(async (id: string) => (id === '2' && ++reads > 1 ? editedLater : served.find(t => t.id === id)!))
    })
    const result = await deployLinkedItems(collection, plan, deps)
    expect(result.deployed.map(deployed => deployed.id)).toEqual(['1'])
    expect(result.failed.map(({ item, stage }) => [item.id, stage])).toEqual([['2', 'changed']])
    expect(deps.approveItem).not.toHaveBeenCalledWith('2')
  })

  it('stops reporting progress after a fatal error, and only throws once every lane is idle', async () => {
    let signs = 0
    let finished = 0
    const deps = deployDeps({
      concurrency: 2,
      sign: vi.fn(async () => (++signs === 1 ? null : [{ type: 'SIGNER', payload: '0xme' }])),
      deployEntity: vi.fn(async () => {
        await new Promise(resolve => setTimeout(resolve, 5))
        finished++
      })
    })
    const progress = vi.fn()
    await expect(
      deployLinkedItems(collection, planFor(['1', '2', '3'].map(id => item(id))), deps, progress)
    ).rejects.toMatchObject({ reason: 'no_identity' })
    expect(finished).toBe(1)
    expect(progress).not.toHaveBeenCalled()
  })

  it('reports a failed take-back as a curation failure, not as an item sent back to the creator', async () => {
    const deps = deployDeps({
      approveItem: vi.fn(async () => ({ contentHash: 'saved-meanwhile' })),
      rejectItem: vi.fn(async () => {
        throw new Error('offline')
      })
    })
    const result = await deployLinkedItems(collection, planFor([item('1')]), deps)
    expect(result.failed.map(({ item, stage }) => [item.id, stage])).toEqual([['1', 'curation']])
  })

  it('stops at once when the session cannot sign', async () => {
    const deps = deployDeps({ sign: vi.fn(async () => null) })
    await expect(deployLinkedItems(collection, planFor([item('1')]), deps)).rejects.toBeInstanceOf(ApprovalError)
    expect(deps.deployEntity).not.toHaveBeenCalled()
  })

  it('deploys a few items at a time', async () => {
    let inFlight = 0
    let peak = 0
    const deps = deployDeps({
      concurrency: 2,
      deployEntity: vi.fn(async () => {
        inFlight++
        peak = Math.max(peak, inFlight)
        await new Promise(resolve => setTimeout(resolve, 5))
        inFlight--
      })
    })
    const items = ['1', '2', '3', '4', '5'].map(id => item(id))
    const result = await deployLinkedItems(collection, planFor(items), deps)
    expect(result.deployed).toHaveLength(5)
    expect(peak).toBe(2)
  })
})
