import { beforeEach, describe, expect, it, vi } from 'vitest'
import { act, renderHook, waitFor } from '@testing-library/react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { type ReactNode } from 'react'
import { type Collection } from '~/lib/collections'

const api = vi.hoisted(() => ({
  fetchCollectionEvents: vi.fn(),
  requestCollectionValidation: vi.fn(),
  appealCollectionCuration: vi.fn(),
  postCollectionComment: vi.fn()
}))
vi.mock('~/lib/builder', async importOriginal => ({ ...(await importOriginal<object>()), ...api }))
vi.mock('~/lib/analytics', () => ({ track: vi.fn(), errorCode: () => 'unknown' }))
vi.mock('~/lib/monitoring', () => ({ captureError: vi.fn() }))
const flag = vi.hoisted(() => ({ enabled: true }))
vi.mock('~/hooks/useFeatureFlag', () => ({ useFeatureFlag: () => ({ enabled: flag.enabled, isLoading: false }) }))

const { BuilderServerError } = await import('~/lib/builder')
const { captureError } = await import('~/lib/monitoring')
const { useAppealCuration, useCollectionEvents, usePostComment, useRequestValidation } =
  await import('./useCollectionEvents')

const collection = { id: 'c1', name: 'Hats', isPublished: true } as Collection
const remote = (id: string) => ({
  id,
  collectionId: 'c1',
  type: 'review.automated_started',
  actor: 'validator' as const,
  actorAddress: null,
  payload: {},
  createdAt: 1
})

function wrapper({ children }: { children: ReactNode }) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false }, mutations: { retry: false } } })
  return <QueryClientProvider client={client}>{children}</QueryClientProvider>
}

beforeEach(() => {
  Object.values(api).forEach(fn => fn.mockReset())
  vi.mocked(captureError).mockReset()
  flag.enabled = true
})

describe('useCollectionEvents', () => {
  it('flattens the pages newest first and knows when more remain', async () => {
    api.fetchCollectionEvents.mockResolvedValueOnce({
      results: [remote('e1'), remote('e2')],
      total: 3,
      page: 1,
      limit: 2
    })
    api.fetchCollectionEvents.mockResolvedValueOnce({ results: [remote('e3')], total: 3, page: 2, limit: 20 })
    const { result } = renderHook(() => useCollectionEvents('0xme', collection), { wrapper })
    await waitFor(() => expect(result.current.events).toHaveLength(2))
    expect(result.current.hasNextPage).toBe(true)
    expect(result.current.isAvailable).toBe(true)
    await act(async () => {
      await result.current.fetchNextPage()
    })
    await waitFor(() => expect(result.current.events?.map(event => event.id)).toEqual(['e1', 'e2', 'e3']))
    expect(result.current.hasNextPage).toBe(false)
    const limits = api.fetchCollectionEvents.mock.calls.map(call => (call[2] as { limit: number }).limit)
    expect(new Set(limits).size).toBe(1)
  })

  it('reads a server without the timeline as no timeline, not an empty one', async () => {
    api.fetchCollectionEvents.mockRejectedValue(new BuilderServerError('not found', 404))
    const { result } = renderHook(() => useCollectionEvents('0xme', collection), { wrapper })
    await waitFor(() => expect(result.current.isSuccess).toBe(true))
    expect(result.current.events).toBeNull()
    expect(result.current.isAvailable).toBe(false)
  })

  it('reports a broken timeline endpoint once, from the hook', async () => {
    api.fetchCollectionEvents.mockRejectedValue(new BuilderServerError('boom', 500))
    const { result } = renderHook(() => useCollectionEvents('0xme', collection), { wrapper })
    await waitFor(() => expect(result.current.isError).toBe(true))
    expect(captureError).toHaveBeenCalledWith(expect.anything(), { flow: 'curation_events', collectionId: 'c1' })
    expect(result.current.events).toBeNull()
  })

  it('never asks for the timeline while auto-curation is off', () => {
    flag.enabled = false
    const { result } = renderHook(() => useCollectionEvents('0xme', collection), { wrapper })
    expect(api.fetchCollectionEvents).not.toHaveBeenCalled()
    expect(result.current.events).toBeNull()
  })

  it('drops a row repeated by an event landing between two pages', async () => {
    api.fetchCollectionEvents.mockResolvedValueOnce({
      results: [remote('e1'), remote('e2')],
      total: 3,
      page: 1,
      limit: 2
    })
    api.fetchCollectionEvents.mockResolvedValueOnce({
      results: [remote('e2'), remote('e3')],
      total: 4,
      page: 2,
      limit: 2
    })
    const { result } = renderHook(() => useCollectionEvents('0xme', collection), { wrapper })
    await waitFor(() => expect(result.current.events).toHaveLength(2))
    await act(async () => {
      await result.current.fetchNextPage()
    })
    await waitFor(() => expect(result.current.events?.map(event => event.id)).toEqual(['e1', 'e2', 'e3']))
  })
})

describe('useRequestValidation', () => {
  it('reports unexpected failures but not the 409 the creator can act on', async () => {
    api.requestCollectionValidation.mockRejectedValueOnce(
      new BuilderServerError('A validation is already in progress for this collection', 409)
    )
    const { result } = renderHook(() => useRequestValidation('0xme'), { wrapper })
    await act(() => result.current.mutateAsync({ collection, events: [] }).catch(() => undefined))
    expect(captureError).not.toHaveBeenCalled()

    api.requestCollectionValidation.mockRejectedValueOnce(new Error('boom'))
    await act(() => result.current.mutateAsync({ collection, events: [] }).catch(() => undefined))
    expect(captureError).toHaveBeenCalledWith(expect.any(Error), { flow: 'curation_validate', collectionId: 'c1' })
  })
})

describe('useAppealCuration', () => {
  it('asks for a curator review of the collection', async () => {
    api.appealCollectionCuration.mockResolvedValue(undefined)
    const { result } = renderHook(() => useAppealCuration('0xme'), { wrapper })
    await act(() => result.current.mutateAsync({ collection }))
    expect(api.appealCollectionCuration).toHaveBeenCalledWith('0xme', 'c1')
  })

  it('sends the note the creator wrote with it', async () => {
    api.appealCollectionCuration.mockResolvedValue(undefined)
    const { result } = renderHook(() => useAppealCuration('0xme'), { wrapper })
    await act(() => result.current.mutateAsync({ collection, note: 'Meant to clip' }))
    expect(api.appealCollectionCuration).toHaveBeenCalledWith('0xme', 'c1', 'Meant to clip')
  })
})

describe('usePostComment', () => {
  it('posts the comment, anchored to an item when one is given', async () => {
    api.postCollectionComment.mockResolvedValue({})
    const { result } = renderHook(() => usePostComment('0xme'), { wrapper })
    await act(() => result.current.mutateAsync({ collection, message: 'Fixed', role: 'creator' }))
    expect(api.postCollectionComment).toHaveBeenLastCalledWith('0xme', 'c1', { message: 'Fixed' })
    await act(() => result.current.mutateAsync({ collection, message: 'Legs', itemId: 'i1', role: 'curator' }))
    expect(api.postCollectionComment).toHaveBeenLastCalledWith('0xme', 'c1', { message: 'Legs', itemId: 'i1' })
  })

  it('reports a comment that could not be stored', async () => {
    api.postCollectionComment.mockRejectedValueOnce(new Error('boom'))
    const { result } = renderHook(() => usePostComment('0xme'), { wrapper })
    await act(() =>
      result.current.mutateAsync({ collection, message: 'Fixed', role: 'creator' }).catch(() => undefined)
    )
    expect(captureError).toHaveBeenCalledWith(expect.any(Error), { flow: 'curation_comment', collectionId: 'c1' })
  })
})
