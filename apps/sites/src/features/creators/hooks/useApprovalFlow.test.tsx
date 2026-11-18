import { beforeEach, describe, expect, it, vi } from 'vitest'
import { act, renderHook } from '@testing-library/react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { type ReactNode } from 'react'
import { type ContractCall, type Session } from '~/lib/auth'
import { type Collection } from '~/lib/collections'
import { type CollectionCuration } from '~/lib/curation'
import { type Item } from '~/lib/items'
import { allCollectionItemsKey } from '~/hooks/useCollection'
import { itemEntitiesKey } from '~/hooks/useItemSync'
import { captureError } from '~/lib/monitoring'
import { WrongAccountError } from '~/lib/walletErrors'

const flow = vi.hoisted(() => ({
  APPROVAL_INDEX_TIMEOUT_MS: 1000,
  findItemsToRescue: vi.fn(),
  rescueItems: vi.fn(),
  findItemsToDeploy: vi.fn(),
  deployItems: vi.fn(),
  approveOnChain: vi.fn(),
  buildMissingImage: vi.fn(),
  waitForIndexer: vi.fn()
}))
const api = vi.hoisted(() => ({
  fetchAllCollectionItems: vi.fn(),
  fetchCollection: vi.fn(),
  publishCollectionItems: vi.fn(),
  updateCollectionCuration: vi.fn(),
  pushCollectionCuration: vi.fn(),
  fetchContent: vi.fn()
}))
const auth = vi.hoisted(() => ({
  sendContractTransaction: vi.fn(),
  signWithIdentity: vi.fn(),
  waitForTransaction: vi.fn()
}))
vi.mock('~/lib/approveCollection', () => flow)
vi.mock('~/lib/builder', () => api)
vi.mock('~/lib/auth', () => auth)
const catalyst = vi.hoisted(() => ({
  fetchEntitiesByPointers: vi.fn(),
  fetchAvailableContent: vi.fn(),
  deployEntity: vi.fn()
}))
vi.mock('~/lib/catalyst', () => catalyst)
vi.mock('~/lib/analytics', () => ({ track: vi.fn(), errorCode: () => 'unknown' }))
vi.mock('~/lib/monitoring', () => ({ captureError: vi.fn() }))
vi.mock('~/lib/media', () => ({ generateCatalystImage: vi.fn() }))

const { useApprovalFlow } = await import('./useApprovalFlow')
type ApprovalMode = import('./useApprovalFlow').ApprovalMode

type Deps = { sendTransaction: (call: ContractCall) => Promise<string> }

const session = { address: '0xme' } as Session
const item = { id: 'i1', name: 'Hat', urn: 'urn:1', tokenId: '0', isPublished: true, contents: {} } as Item
const collection = { id: 'c1', isApproved: false } as Collection
const pending = { collectionId: 'c1', status: 'pending' } as CollectionCuration

let queryClient: QueryClient
function wrapper({ children }: { children: ReactNode }) {
  return <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
}

function renderFlow(
  subject = collection,
  curation: CollectionCuration | null = pending,
  mode: ApprovalMode = 'approve'
) {
  return renderHook(() => useApprovalFlow(session, subject, curation, mode, [item]), { wrapper })
}

beforeEach(() => {
  queryClient = new QueryClient()
  Object.values(flow).forEach(fn => typeof fn === 'function' && fn.mockReset())
  Object.values(api).forEach(fn => fn.mockReset())
  Object.values(auth).forEach(fn => fn.mockReset())
  api.fetchAllCollectionItems.mockResolvedValue([item])
  Object.values(catalyst).forEach(fn => fn.mockReset())
  catalyst.fetchEntitiesByPointers.mockResolvedValue([])
  api.fetchCollection.mockResolvedValue({ ...collection, isApproved: true })
  flow.findItemsToRescue.mockResolvedValue([])
  flow.findItemsToDeploy.mockReturnValue([])
  flow.rescueItems.mockResolvedValue([item])
  flow.deployItems.mockResolvedValue({ deployed: [item], failed: [] })
  flow.approveOnChain.mockImplementation((_collection: Collection, deps: Deps) =>
    deps.sendTransaction({} as ContractCall)
  )
  flow.waitForIndexer.mockImplementation(async (read: () => Promise<unknown>, isDone: (value: unknown) => boolean) => {
    for (;;) {
      const value = await read()
      if (isDone(value)) return value
    }
  })
  auth.sendContractTransaction.mockResolvedValue('0xtx')
})

describe('useApprovalFlow', () => {
  it('walks a first review through each step it needs, closing the pending request at the end', async () => {
    flow.findItemsToRescue.mockResolvedValue([{ item, contentHash: 'h' }])
    flow.findItemsToDeploy.mockReturnValue([item])
    const { result } = renderFlow()

    await act(() => result.current.start())
    expect(result.current.plan.steps).toEqual(['rescue', 'deploy', 'approve'])
    expect(result.current.view).toMatchObject({ kind: 'step', step: 'rescue', phase: { kind: 'idle' } })
    await act(() => result.current.runRescue())
    expect(result.current.view).toMatchObject({ kind: 'step', step: 'deploy' })
    await act(() => result.current.runDeploy())
    expect(result.current.view).toMatchObject({ kind: 'step', step: 'approve' })
    await act(() => result.current.runApprove())
    expect(result.current.view).toEqual({ kind: 'success' })
    expect(api.updateCollectionCuration).toHaveBeenCalledWith('0xme', 'c1', { status: 'approved' })
  })

  it('skips the steps with nothing to do', async () => {
    const { result } = renderFlow()
    await act(() => result.current.start())
    expect(result.current.plan.steps).toEqual(['approve'])
  })

  it('shows success only once builder-server reads the collection as approved', async () => {
    api.fetchCollection
      .mockResolvedValueOnce({ ...collection, isApproved: false })
      .mockResolvedValueOnce({ ...collection, isApproved: true, name: 'From server' })
    const { result } = renderFlow()
    await act(() => result.current.start())
    await act(() => result.current.runApprove())
    expect(api.fetchCollection).toHaveBeenCalledTimes(2)
    expect(queryClient.getQueryData<Collection>(['collection', '0xme', 'c1'])).toMatchObject({ name: 'From server' })
    expect(result.current.view).toEqual({ kind: 'success' })
  })

  it('still succeeds when builder-server takes too long, keeping the on-chain result', async () => {
    flow.waitForIndexer.mockRejectedValue(new Error('not indexed'))
    const { result } = renderFlow()
    await act(() => result.current.start())
    await act(() => result.current.runApprove())
    expect(queryClient.getQueryData<Collection>(['collection', '0xme', 'c1'])).toMatchObject({ isApproved: true })
    expect(result.current.view).toEqual({ kind: 'success' })
  })

  it('approves a rejected first review through a new request in the approver’s name, so it stops reading as rejected', async () => {
    const { result } = renderFlow(collection, { status: 'rejected' } as CollectionCuration)
    await act(() => result.current.start())
    await act(() => result.current.runApprove())
    expect(api.pushCollectionCuration).toHaveBeenCalledWith('0xme', 'c1', '0xme')
    expect(api.updateCollectionCuration).toHaveBeenCalledWith('0xme', 'c1', { status: 'approved' })
  })

  it('closes the request opened after the flow started, e.g. by assigning the collection', async () => {
    queryClient.setQueryData(['collection-curation', '0xme', 'c1'], pending)
    const { result } = renderFlow(collection, null)
    await act(() => result.current.start())
    await act(() => result.current.runApprove())
    expect(api.updateCollectionCuration).toHaveBeenCalledWith('0xme', 'c1', { status: 'approved' })
  })

  it('closes the request once the approval landed on chain, even if the flow unmounted meanwhile', async () => {
    let land: (txHash: string) => void = () => undefined
    auth.sendContractTransaction.mockReturnValue(new Promise<string>(resolve => (land = resolve)))
    const { result } = renderFlow()
    await act(() => result.current.start())
    let approving: Promise<void> = Promise.resolve()
    act(() => {
      approving = result.current.runApprove()
    })
    result.current.stop()
    await act(async () => {
      land('0xtx')
      await approving
    })
    expect(api.updateCollectionCuration).toHaveBeenCalledWith('0xme', 'c1', { status: 'approved' })
  })

  it('does not close the request when the curator closed the flow while it was loading', async () => {
    let release: (items: Item[]) => void = () => undefined
    api.fetchAllCollectionItems.mockReturnValue(new Promise<Item[]>(resolve => (release = resolve)))
    const { result } = renderFlow({ ...collection, isApproved: true })
    let running: Promise<void> = Promise.resolve()
    act(() => {
      running = result.current.start()
    })
    result.current.stop()
    await act(async () => {
      release([item])
      await running
    })
    expect(api.updateCollectionCuration).not.toHaveBeenCalled()
  })

  it('approves pushed changes of an approved collection without a transaction', async () => {
    const { result } = renderFlow({ ...collection, isApproved: true })
    await act(() => result.current.start())
    expect(result.current.view).toEqual({ kind: 'success' })
    expect(flow.approveOnChain).not.toHaveBeenCalled()
    expect(api.updateCollectionCuration).toHaveBeenCalledWith('0xme', 'c1', { status: 'approved' })
  })

  it('reports each failed upload with its reason, and a retry starts over from what is still missing', async () => {
    flow.findItemsToDeploy.mockReturnValue([item])
    flow.deployItems.mockResolvedValueOnce({ deployed: [], failed: [{ item, message: 'Catalyst said no' }] })
    const { result } = renderFlow({ ...collection, isApproved: true }, null, 'deploy_missing')
    await act(() => result.current.start())
    await act(() => result.current.runDeploy())
    expect(result.current.view).toMatchObject({
      kind: 'error',
      step: 'deploy',
      failed: [{ item, message: 'Catalyst said no' }]
    })

    await act(() => result.current.start())
    await act(() => result.current.runDeploy())
    expect(result.current.view).toEqual({ kind: 'success' })
    expect(flow.findItemsToRescue).not.toHaveBeenCalled()
  })

  it('shows the failed step with the raw error', async () => {
    flow.findItemsToRescue.mockResolvedValue([{ item, contentHash: 'h' }])
    flow.rescueItems.mockRejectedValue(new Error('execution reverted: boom'))
    const { result } = renderFlow(collection, null)
    await act(() => result.current.start())
    await act(() => result.current.runRescue())
    expect(result.current.view).toEqual({
      kind: 'error',
      step: 'rescue',
      detail: 'execution reverted: boom',
      failed: []
    })
  })

  it('goes back to the step when the curator rejects the wallet prompt', async () => {
    auth.sendContractTransaction.mockRejectedValue({ code: 4001 })
    const { result } = renderFlow()
    await act(() => result.current.start())
    await act(() => result.current.runApprove())
    expect(result.current.view).toEqual({ kind: 'step', step: 'approve', phase: { kind: 'idle' } })
  })

  it('asks the curator to switch the wallet back, without reporting it, when it sits on another account', async () => {
    vi.mocked(captureError).mockClear()
    auth.sendContractTransaction.mockRejectedValue(new WrongAccountError('0xme', '0xother'))
    const { result } = renderFlow()
    await act(() => result.current.start())
    await act(() => result.current.runApprove())
    expect(result.current.view).toMatchObject({ kind: 'error', step: 'approve', detail: null, wallet: true })
    expect(captureError).not.toHaveBeenCalled()
  })

  it('backing out of the wallet prompt hides it, and confirming again shows it instead of sending twice', async () => {
    let sign: (txHash: string) => void = () => undefined
    auth.sendContractTransaction.mockReturnValue(new Promise<string>(resolve => (sign = resolve)))
    const { result } = renderFlow()
    await act(() => result.current.start())
    let approving: Promise<void> = Promise.resolve()
    act(() => {
      approving = result.current.runApprove()
    })
    expect(result.current.view).toMatchObject({ phase: { kind: 'signing', tx: 1, txs: 1 } })

    act(() => result.current.cancelSigning())
    expect(result.current.view).toEqual({ kind: 'step', step: 'approve', phase: { kind: 'idle' } })
    await act(() => result.current.runApprove())
    expect(result.current.view).toMatchObject({ phase: { kind: 'signing' } })

    await act(async () => {
      sign('0xtx')
      await approving
    })
    expect(auth.sendContractTransaction).toHaveBeenCalledTimes(1)
    expect(result.current.view).toEqual({ kind: 'success' })
  })

  it('opens no further wallet prompts once the flow is closed', async () => {
    let release: () => void = () => undefined
    const gate = new Promise<void>(resolve => (release = resolve))
    flow.approveOnChain.mockImplementation(async (_collection: Collection, deps: Deps) => {
      await gate
      return deps.sendTransaction({} as ContractCall)
    })
    const { result } = renderFlow()
    await act(() => result.current.start())
    let approving: Promise<void> = Promise.resolve()
    act(() => {
      approving = result.current.runApprove()
    })
    result.current.stop()
    await act(async () => {
      release()
      await approving
    })
    expect(auth.sendContractTransaction).not.toHaveBeenCalled()
  })

  it('leaves the rescue chunks already mined out of a retry, while still waiting for them to be indexed', async () => {
    const first = { item, contentHash: 'h1' }
    const second = { item: { ...item, id: 'i2' }, contentHash: 'h2' }
    flow.findItemsToRescue.mockResolvedValue([first, second])
    flow.rescueItems.mockImplementationOnce(
      async (_c: Collection, _t: unknown, deps: { onChunkMined: (targets: unknown[]) => void }) => {
        deps.onChunkMined([first])
        throw Object.assign(new Error('User rejected the request'), { code: 4001 })
      }
    )
    const { result } = renderFlow()
    await act(() => result.current.start())
    await act(() => result.current.runRescue())
    expect(result.current.view).toMatchObject({ kind: 'step', step: 'rescue', phase: { kind: 'idle' } })

    await act(() => result.current.runRescue())
    expect(flow.rescueItems).toHaveBeenLastCalledWith(
      collection,
      [second],
      expect.objectContaining({ alreadyMined: [first] })
    )
  })

  it('reuses the request it reopened when approving it failed, instead of opening another', async () => {
    api.pushCollectionCuration.mockResolvedValue(pending)
    api.updateCollectionCuration
      .mockRejectedValueOnce(new Error('503'))
      .mockResolvedValue({ ...pending, status: 'approved' })
    const rejected = { collectionId: 'c1', status: 'rejected' } as CollectionCuration
    const { result } = renderFlow({ ...collection, isApproved: true }, rejected)
    await act(() => result.current.start())
    expect(result.current.view).toMatchObject({ kind: 'error' })

    await act(() => result.current.start())
    expect(result.current.view).toEqual({ kind: 'success' })
    expect(api.pushCollectionCuration).toHaveBeenCalledTimes(1)
  })

  it('enables a disabled collection as last approved, leaving its items and its request alone', async () => {
    const { result } = renderFlow({ ...collection, reviewedAt: 5 }, null, 'enable')
    await act(() => result.current.start())
    expect(result.current.plan.steps).toEqual(['approve'])
    await act(() => result.current.runApprove())
    expect(result.current.view).toEqual({ kind: 'success' })
    expect(flow.approveOnChain).toHaveBeenCalled()
    expect(api.fetchAllCollectionItems).not.toHaveBeenCalled()
    expect(flow.rescueItems).not.toHaveBeenCalled()
    expect(api.pushCollectionCuration).not.toHaveBeenCalled()
    expect(api.updateCollectionCuration).not.toHaveBeenCalled()
  })

  it('fails before any step when the reviewed items were loaded without their on-chain data', async () => {
    const blank = { ...item, urn: undefined, isPublished: false }
    const { result } = renderHook(() => useApprovalFlow(session, collection, pending, 'approve', [blank]), { wrapper })
    await act(() => result.current.start())
    expect(result.current.view).toMatchObject({
      kind: 'error',
      step: 'prepare',
      detail: expect.stringContaining('reload')
    })
    expect(flow.findItemsToRescue).not.toHaveBeenCalled()

    const missing = renderHook(() => useApprovalFlow(session, collection, pending, 'deploy_missing', [blank]), {
      wrapper
    })
    await act(() => missing.result.current.start())
    expect(missing.result.current.view).toMatchObject({ kind: 'error', step: 'prepare' })
    expect(flow.findItemsToDeploy).not.toHaveBeenCalled()
  })

  it('carries the rescued hashes into the reviewed items, so a reopened flow does not rescue them again', async () => {
    const stale = { ...item, blockchainContentHash: 'old' }
    flow.findItemsToRescue.mockResolvedValue([{ item: stale, contentHash: 'new' }])
    const other = { ...item, id: 'i2', blockchainContentHash: 'untouched' }
    queryClient.setQueryData(allCollectionItemsKey('0xme', 'c1'), [stale, other])
    api.fetchAllCollectionItems.mockResolvedValue([stale, other])
    flow.rescueItems.mockResolvedValue([
      { ...stale, blockchainContentHash: 'new' },
      { ...other, blockchainContentHash: 'drifted' }
    ])
    flow.findItemsToDeploy.mockReturnValue([stale])
    flow.deployItems.mockResolvedValue({ deployed: [], failed: [{ item: stale, message: 'boom' }] })
    const reviewed = () => queryClient.getQueryData<Item[]>(allCollectionItemsKey('0xme', 'c1'))!
    const first = renderHook(() => useApprovalFlow(session, collection, pending, 'approve', [stale, other]), {
      wrapper
    })
    await act(() => first.result.current.start())
    await act(() => first.result.current.runRescue())
    await act(() => first.result.current.runDeploy())
    expect(first.result.current.view).toMatchObject({ kind: 'error', step: 'deploy' })
    expect(reviewed()).toEqual([{ ...stale, blockchainContentHash: 'new' }, other])
    first.unmount()

    flow.findItemsToRescue.mockResolvedValue([])
    const second = renderHook(() => useApprovalFlow(session, collection, pending, 'approve', reviewed()), { wrapper })
    await act(() => second.result.current.start())
    expect(flow.findItemsToRescue).toHaveBeenLastCalledWith(
      collection,
      [{ ...stale, blockchainContentHash: 'new' }, other],
      expect.any(Function)
    )
    expect(second.result.current.plan.steps).not.toContain('rescue')
  })

  it('still approves when the items cannot be fetched for the changed check', async () => {
    api.fetchAllCollectionItems.mockRejectedValue(new TypeError('Failed to fetch'))
    const { result } = renderFlow()
    await act(() => result.current.start())
    expect(result.current.view).toMatchObject({ kind: 'step', step: 'approve' })
    expect(flow.findItemsToRescue).toHaveBeenCalledWith(collection, [item], expect.any(Function))
  })

  it('plans the deploy from fresh entities, or from the ones the review bar looked up when the Catalyst fails', async () => {
    const stale = { id: 'e0', pointers: ['urn:1'] }
    const fresh = { id: 'e1', pointers: ['urn:1'] }
    queryClient.setQueryData(itemEntitiesKey('c1', ['urn:1']), [stale])
    catalyst.fetchEntitiesByPointers.mockResolvedValue([fresh])
    const { result } = renderFlow()
    await act(() => result.current.start())
    expect(catalyst.fetchEntitiesByPointers).toHaveBeenCalledWith(['urn:1'])
    expect(flow.findItemsToDeploy).toHaveBeenCalledWith([item], [fresh], false)

    catalyst.fetchEntitiesByPointers.mockRejectedValue(new TypeError('Failed to fetch'))
    const offline = renderFlow()
    await act(() => offline.result.current.start())
    expect(offline.result.current.view).toMatchObject({ kind: 'step', step: 'approve' })
    expect(flow.findItemsToDeploy).toHaveBeenLastCalledWith([item], [stale], false)

    queryClient.clear()
    const nothing = renderFlow()
    await act(() => nothing.result.current.start())
    expect(nothing.result.current.view).toMatchObject({ kind: 'error', step: 'prepare', detail: 'Failed to fetch' })
  })

  it('plans from the items the curator reviewed, not from the fresh fetch', async () => {
    // builder-server answers with blank on-chain fields when its subgraph query fails.
    api.fetchAllCollectionItems.mockResolvedValue([{ ...item, urn: undefined, isPublished: false }])
    const { result } = renderFlow()
    await act(() => result.current.start())
    expect(result.current.view).toMatchObject({ kind: 'step', step: 'approve' })
    expect(flow.findItemsToRescue).toHaveBeenCalledWith(collection, [item], expect.any(Function))
    expect(api.publishCollectionItems).not.toHaveBeenCalled()
  })

  it('backfills missing token ids and re-checks the items the server saved again, on their content', async () => {
    const unpublished = { ...item, tokenId: undefined }
    api.publishCollectionItems.mockResolvedValue({ collection, items: [{ ...item, updatedAt: 99 }] })
    const { result } = renderHook(() => useApprovalFlow(session, collection, pending, 'approve', [unpublished]), {
      wrapper
    })
    await act(() => result.current.start())
    expect(result.current.view).toMatchObject({ kind: 'step', step: 'approve' })

    api.publishCollectionItems.mockResolvedValue({
      collection,
      items: [{ ...item, updatedAt: 99, name: 'Edited meanwhile' }]
    })
    await act(() => result.current.start())
    expect(result.current.view).toMatchObject({ kind: 'error', step: 'changed' })
  })

  it('stops when the creator saved changes after the curator opened the collection', async () => {
    api.fetchAllCollectionItems.mockResolvedValue([{ ...item, updatedAt: 99 }])
    const { result } = renderFlow()
    await act(() => result.current.start())
    expect(result.current.view).toMatchObject({ kind: 'error', step: 'changed' })
    expect(flow.findItemsToRescue).not.toHaveBeenCalled()
  })
})
