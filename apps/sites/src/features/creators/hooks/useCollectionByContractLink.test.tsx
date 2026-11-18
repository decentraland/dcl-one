import { beforeEach, describe, expect, it, vi } from 'vitest'
import { renderHook, waitFor } from '@testing-library/react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { type ReactNode } from 'react'
import { MemoryRouter, useLocation } from 'react-router-dom'
import { type Collection } from '~/lib/collections'

const api = vi.hoisted(() => ({ fetchCollections: vi.fn() }))
vi.mock('~/lib/builder', async importOriginal => ({ ...(await importOriginal<object>()), ...api }))
const track = vi.hoisted(() => vi.fn())
vi.mock('~/lib/analytics', () => ({ track }))

const { useCollectionByContractLink } = await import('./useCollectionByContractLink')

const OWNER = '0x00000000000000000000000000000000000000aa'
const CONTRACT = '0x1d9fB685c257E74F869BA302e260C0B68f5eBb37'
const draft = { id: 'draft-1' } as Collection
const hats = { id: 'hats-1', contractAddress: CONTRACT.toLowerCase() } as Collection

function page(results: Collection[]) {
  return { results, total: results.length, page: 1, pages: 1, limit: 1 }
}

function renderLink(
  address: string | undefined,
  url = `/collections?contract=${CONTRACT}`,
  client = new QueryClient({ defaultOptions: { queries: { retry: false } } })
) {
  const wrapper = ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={client}>
      <MemoryRouter initialEntries={[url]}>{children}</MemoryRouter>
    </QueryClientProvider>
  )
  return renderHook(
    () => {
      const link = useCollectionByContractLink(address)
      const location = useLocation()
      return { ...link, url: `${location.pathname}${location.search}` }
    },
    { wrapper }
  )
}

beforeEach(() => {
  api.fetchCollections.mockReset().mockResolvedValue(page([hats]))
  track.mockReset()
})

describe('when a signed-in creator opens a contract link', () => {
  it('opens the detail of their collection deployed at that address', async () => {
    const { result } = renderLink(OWNER)
    expect(result.current.isResolving).toBe(true)

    await waitFor(() => expect(result.current.url).toBe('/collections/hats-1'))
    expect(api.fetchCollections).toHaveBeenCalledWith(OWNER, { contractAddress: CONTRACT, page: 1, limit: 1 })
    expect(track).toHaveBeenCalledWith('Open collection by contract', { outcome: 'found', collectionId: 'hats-1' })
  })

  it('stays on the list, keeping the other params, when none of their collections is at that address', async () => {
    api.fetchCollections.mockResolvedValue(page([]))
    const { result } = renderLink(OWNER, `/collections?status=draft&contract=${CONTRACT}`)

    await waitFor(() => expect(result.current.url).toBe('/collections?status=draft'))
    expect(result.current.isResolving).toBe(false)
    expect(track).toHaveBeenCalledWith('Open collection by contract', { outcome: 'not_found' })
  })

  it('does not open another collection when builder-server ignores the contract filter', async () => {
    api.fetchCollections.mockResolvedValue(page([draft]))
    const { result } = renderLink(OWNER)

    await waitFor(() => expect(result.current.url).toBe('/collections'))
    expect(track).toHaveBeenCalledWith('Open collection by contract', { outcome: 'not_found' })
  })

  it('stays on the list when their collections cannot be read', async () => {
    api.fetchCollections.mockRejectedValue(new Error('boom'))
    const { result } = renderLink(OWNER)

    await waitFor(() => expect(result.current.url).toBe('/collections'), { timeout: 3000 })
    expect(track).toHaveBeenCalledWith('Open collection by contract', { outcome: 'error' })
  })

  it('drops a value that is not a contract address without reading their collections', async () => {
    const { result } = renderLink(OWNER, '/collections?status=draft&contract=undefined')

    await waitFor(() => expect(result.current.url).toBe('/collections?status=draft'))
    expect(result.current.isResolving).toBe(false)
    expect(api.fetchCollections).not.toHaveBeenCalled()
    expect(track).toHaveBeenCalledWith('Open collection by contract', { outcome: 'invalid' })
  })

  it('looks the contract up again when an older answer, read before the collection was deployed, is cached', async () => {
    const client = new QueryClient({ defaultOptions: { queries: { retry: false } } })
    client.setQueryData(['collections', OWNER, 'contract', CONTRACT.toLowerCase()], [])
    const { result } = renderLink(OWNER, undefined, client)

    await waitFor(() => expect(result.current.url).toBe('/collections/hats-1'))
  })
})

describe('when a signed-out visitor opens a contract link', () => {
  it('keeps the link so it resolves after signing in', () => {
    const { result } = renderLink(undefined)

    expect(result.current.url).toBe(`/collections?contract=${CONTRACT}`)
    expect(result.current.isResolving).toBe(false)
    expect(api.fetchCollections).not.toHaveBeenCalled()
  })
})

describe('when the list is opened without a contract link', () => {
  it('reads nothing extra', () => {
    renderLink(OWNER, '/collections')

    expect(api.fetchCollections).not.toHaveBeenCalled()
  })
})
