import { beforeEach, describe, expect, it, vi } from 'vitest'
import { act, renderHook, waitFor } from '@testing-library/react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { type ReactNode } from 'react'
import { type Collection } from '~/lib/collections'
import { type CollectionCuration } from '~/lib/curation'
import { type Session } from '~/lib/auth'
import { WrongAccountError } from '~/lib/walletErrors'

const api = vi.hoisted(() => ({
  fetchCommittee: vi.fn(),
  pushCollectionCuration: vi.fn(),
  updateCollectionCuration: vi.fn(),
  createCurationForumReply: vi.fn()
}))
vi.mock('~/lib/builder', async importOriginal => {
  const actual = await importOriginal<typeof import('~/lib/builder')>()
  return {
    ...api,
    BuilderServerError: actual.BuilderServerError,
    ValidationLimitError: actual.ValidationLimitError,
    isCollectionBusy: actual.isCollectionBusy,
    isReviewConflict: actual.isReviewConflict,
    isValidationRunning: actual.isValidationRunning,
    fetchCollectionCuration: vi.fn(),
    fetchCurationCollections: vi.fn(),
    fetchCurations: vi.fn()
  }
})
vi.mock('~/lib/analytics', () => ({ track: vi.fn(), errorCode: () => 'unknown' }))
vi.mock('~/lib/monitoring', () => ({ captureError: vi.fn() }))
const auth = vi.hoisted(() => ({
  sendContractTransaction: vi.fn(),
  waitForTransaction: vi.fn()
}))
vi.mock('~/lib/auth', () => auth)

const {
  collectionCurationKey,
  collectionEventsKey,
  useAssignCurator,
  useDisableCollection,
  usePushCuration,
  useRejectCuration
} = await import('./useCuration')
const { captureError } = await import('~/lib/monitoring')
const { BuilderServerError, ValidationLimitError } = await import('~/lib/builder')

const ADDRESS = '0xCurator'
const collection = { id: 'c1', name: 'Hats', isPublished: true } as Collection
const pending: CollectionCuration = {
  id: 'r1',
  collectionId: 'c1',
  status: 'pending',
  assignee: null,
  reviewedBy: null,
  rejectionReasons: null,
  rejectionMessage: null,
  createdAt: 1,
  updatedAt: 1
}

function wrapper({ children }: { children: ReactNode }) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false }, mutations: { retry: false } } })
  return <QueryClientProvider client={client}>{children}</QueryClientProvider>
}

beforeEach(() => {
  Object.values(api).forEach(fn => fn.mockReset())
  api.pushCollectionCuration.mockResolvedValue(pending)
  api.updateCollectionCuration.mockImplementation((_a: string, _c: string, change: object) =>
    Promise.resolve({ ...pending, ...change })
  )
})

const decision = { rejectionReasons: ['clipping' as const], rejectionMessage: 'Clips through the torso' }
const withDecision = { decision }

describe('useRejectCuration', () => {
  it('rejects the pending request with the reasons and message the creator will read', async () => {
    const { result } = renderHook(() => useRejectCuration(ADDRESS), { wrapper })
    await act(() => result.current.mutateAsync({ collection, curation: pending, ...withDecision }))
    expect(api.pushCollectionCuration).not.toHaveBeenCalled()
    expect(api.updateCollectionCuration).toHaveBeenCalledWith(ADDRESS, 'c1', { status: 'rejected', ...decision })
  })

  it('sends only the status when there is no decision to attach (auto-curation off)', async () => {
    const { result } = renderHook(() => useRejectCuration(ADDRESS), { wrapper })
    await act(() => result.current.mutateAsync({ collection, curation: pending }))
    expect(api.updateCollectionCuration).toHaveBeenCalledWith(ADDRESS, 'c1', { status: 'rejected' })
  })

  it('opens a request first when nobody asked for a review, so the rejection reaches the creator', async () => {
    const { result } = renderHook(() => useRejectCuration(ADDRESS), { wrapper })
    const rejected = await act(() => result.current.mutateAsync({ collection, curation: null, ...withDecision }))
    expect(api.pushCollectionCuration).toHaveBeenCalledWith(ADDRESS, 'c1')
    expect(rejected.status).toBe('rejected')
  })
})

describe('useRejectCuration after a half-success', () => {
  it('patches the request it already opened instead of opening another', async () => {
    api.updateCollectionCuration.mockRejectedValueOnce(new Error('down'))
    const { result } = renderHook(() => useRejectCuration(ADDRESS), { wrapper })
    await act(() => result.current.mutateAsync({ collection, curation: null, ...withDecision }).catch(() => undefined))
    expect(api.pushCollectionCuration).toHaveBeenCalledTimes(1)
    await act(() => result.current.mutateAsync({ collection, curation: pending, ...withDecision }))
    expect(api.pushCollectionCuration).toHaveBeenCalledTimes(1)
  })
})

describe('useAssignCurator', () => {
  it('assigns through the existing request, or opens one for a never-requested collection', async () => {
    const { result } = renderHook(() => useAssignCurator(ADDRESS), { wrapper })
    await act(() => result.current.mutateAsync({ collection, curation: pending, assignee: '0xcurator' }))
    expect(api.updateCollectionCuration).toHaveBeenCalledWith(ADDRESS, 'c1', { assignee: '0xcurator' })

    await act(() => result.current.mutateAsync({ collection, curation: null, assignee: '0xcurator' }))
    expect(api.pushCollectionCuration).toHaveBeenCalledWith(ADDRESS, 'c1', '0xcurator')
  })

  it('tells the collection forum post about the new assignee, when there is one', async () => {
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue(new Response(null, { status: 404 })))
    api.createCurationForumReply.mockResolvedValue(undefined)
    const { result } = renderHook(() => useAssignCurator(ADDRESS), { wrapper })

    await act(() => result.current.mutateAsync({ collection, curation: pending, assignee: '0xcurator' }))
    expect(api.createCurationForumReply).not.toHaveBeenCalled()

    const posted = { ...collection, forumLink: 'https://forum.decentraland.org/t/hats/77' }
    await act(() => result.current.mutateAsync({ collection: posted, curation: pending, assignee: null }))
    await waitFor(() =>
      expect(api.createCurationForumReply).toHaveBeenCalledWith(ADDRESS, 'c1', {
        raw: 'The collection has been unassigned.'
      })
    )
    vi.unstubAllGlobals()
  })

  it('keeps the assignment when the forum reply fails', async () => {
    api.createCurationForumReply.mockRejectedValue(new Error('forum down'))
    const posted = { ...collection, forumLink: 'https://forum.decentraland.org/t/hats/77' }
    const { result } = renderHook(() => useAssignCurator(ADDRESS), { wrapper })

    await act(() => result.current.mutateAsync({ collection: posted, curation: pending, assignee: null }))
    await waitFor(() => expect(result.current.isSuccess).toBe(true))
    await waitFor(() => expect(api.createCurationForumReply).toHaveBeenCalled())
  })
})

describe('usePushCuration', () => {
  it('rereads the review timeline once the changes are sent, so the review they started shows', async () => {
    const client = new QueryClient({ defaultOptions: { queries: { retry: false }, mutations: { retry: false } } })
    const eventsKey = collectionEventsKey(ADDRESS, collection.id)
    client.setQueryData(eventsKey, { pages: [], pageParams: [] })
    const { result } = renderHook(() => usePushCuration(ADDRESS), {
      wrapper: ({ children }: { children: ReactNode }) => (
        <QueryClientProvider client={client}>{children}</QueryClientProvider>
      )
    })

    await act(() => result.current.mutateAsync({ collection }))

    expect(client.getQueryState(eventsKey)?.isInvalidated).toBe(true)
  })

  it('sends what changed along with the changes when the creator wrote it', async () => {
    const { result } = renderHook(() => usePushCuration(ADDRESS), { wrapper })
    await act(() => result.current.mutateAsync({ collection, note: 'Only added a tag' }))
    expect(api.pushCollectionCuration).toHaveBeenCalledWith(ADDRESS, 'c1', undefined, 'Only added a tag')
  })

  it('rereads the review timeline when the push is refused because a review is running', async () => {
    const client = new QueryClient({ defaultOptions: { queries: { retry: false }, mutations: { retry: false } } })
    const eventsKey = collectionEventsKey(ADDRESS, collection.id)
    const curationKey = collectionCurationKey(ADDRESS, collection.id)
    client.setQueryData(eventsKey, { pages: [], pageParams: [] })
    client.setQueryData(curationKey, pending)
    api.pushCollectionCuration.mockRejectedValueOnce(
      new BuilderServerError('A validation is already in progress for this collection', 409)
    )
    const { result } = renderHook(() => usePushCuration(ADDRESS), {
      wrapper: ({ children }: { children: ReactNode }) => (
        <QueryClientProvider client={client}>{children}</QueryClientProvider>
      )
    })

    await act(() => result.current.mutateAsync({ collection }).catch(() => undefined))

    expect(client.getQueryState(eventsKey)?.isInvalidated).toBe(true)
    expect(client.getQueryState(curationKey)?.isInvalidated).toBe(true)
  })

  it('leaves the review as it is when the push fails for another reason', async () => {
    const client = new QueryClient({ defaultOptions: { queries: { retry: false }, mutations: { retry: false } } })
    const eventsKey = collectionEventsKey(ADDRESS, collection.id)
    client.setQueryData(eventsKey, { pages: [], pageParams: [] })
    api.pushCollectionCuration.mockRejectedValueOnce(new BuilderServerError('Internal error', 500))
    const { result } = renderHook(() => usePushCuration(ADDRESS), {
      wrapper: ({ children }: { children: ReactNode }) => (
        <QueryClientProvider client={client}>{children}</QueryClientProvider>
      )
    })

    await act(() => result.current.mutateAsync({ collection }).catch(() => undefined))

    expect(client.getQueryState(eventsKey)?.isInvalidated).toBe(false)
  })

  it("reports a failure, but not the answers the creator acts on: a running review, today's limit or a busy collection", async () => {
    vi.mocked(captureError).mockClear()
    const { result } = renderHook(() => usePushCuration(ADDRESS), { wrapper })

    api.pushCollectionCuration.mockRejectedValueOnce(
      new ValidationLimitError(new BuilderServerError('Daily limit', 429))
    )
    await act(() => result.current.mutateAsync({ collection }).catch(() => undefined))
    api.pushCollectionCuration.mockRejectedValueOnce(
      new BuilderServerError('A validation is already in progress for this collection', 409)
    )
    await act(() => result.current.mutateAsync({ collection }).catch(() => undefined))
    api.pushCollectionCuration.mockRejectedValueOnce(
      new BuilderServerError('The collection is being updated, try again in a moment', 503)
    )
    await act(() => result.current.mutateAsync({ collection }).catch(() => undefined))
    expect(captureError).not.toHaveBeenCalled()

    api.pushCollectionCuration.mockRejectedValueOnce(new BuilderServerError('Internal error', 500))
    await act(() => result.current.mutateAsync({ collection }).catch(() => undefined))
    expect(captureError).toHaveBeenCalledTimes(1)
  })
})

describe('useDisableCollection', () => {
  const session = { address: ADDRESS } as Session
  const approved = { ...collection, isApproved: true, contractAddress: '0x00000000000000000000000000000000000000cc' }

  beforeEach(() => vi.mocked(captureError).mockClear())

  it('fails without reporting it when the wallet sits on another account', async () => {
    auth.sendContractTransaction.mockRejectedValue(new WrongAccountError(ADDRESS, '0xother'))
    const { result } = renderHook(() => useDisableCollection(session), { wrapper })

    await act(() => result.current.mutateAsync({ collection: approved }).catch(() => undefined))
    await waitFor(() => expect(result.current.isError).toBe(true))
    expect(captureError).not.toHaveBeenCalled()
  })

  it('reports any other failure', async () => {
    auth.sendContractTransaction.mockRejectedValue(new Error('relayer down'))
    const { result } = renderHook(() => useDisableCollection(session), { wrapper })

    await act(() => result.current.mutateAsync({ collection: approved }).catch(() => undefined))
    await waitFor(() =>
      expect(captureError).toHaveBeenCalledWith(
        expect.objectContaining({ message: 'relayer down' }),
        expect.objectContaining({ flow: 'curation_disable' })
      )
    )
  })
})
