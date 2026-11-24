import { describe, it, expect, vi, afterEach, beforeEach } from 'vitest'
import { fireEvent, render, screen, within, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import * as auth from '@explorer-core/features/auth/sso'
import { PlacesBrowser } from '../features/places/PlacesBrowser'
import { clearPlacesCache, type DiscoverPlace } from '@explorer-core/features/places/placesApi'

const place = (id: string, title: string, extra: Partial<DiscoverPlace> = {}): DiscoverPlace => ({
  id,
  title,
  description: '',
  image: `https://img/${id}.png`,
  positions: ['1,1'],
  base_position: '1,1',
  owner: '0xabc',
  ...extra
})

const busy = place('busy', 'Busy Club', { user_count: 5 })
const empty = place('empty', 'Quiet Gallery', { user_count: 0 })
const curated = place('curated', 'Curated World', { world: true, world_name: 'curated.dcl.eth', highlighted: true })
const other = place('other', 'Other Scene')
const junk = place('junk', 'Empty', { image: 'https://api.decentraland.org/v2/map.png' })

function servePlaces(): void {
  vi.stubGlobal(
    'fetch',
    vi.fn((url: string) => {
      const data = url.includes('only_highlighted=true') ? [curated] : url.includes('limit=40') ? [busy, empty] : [busy, empty, curated, other, junk]
      return Promise.resolve(new Response(JSON.stringify({ ok: true, total: data.length, data })))
    })
  )
}

afterEach(() => { vi.unstubAllGlobals(); vi.restoreAllMocks() })
beforeEach(clearPlacesCache)

describe('places browser', () => {
  it('shows the places a search finds when the worlds search is down', async () => {
    const fetchMock = vi.fn((url: string) =>
      url.includes('/worlds')
        ? Promise.reject(new Error('worlds down'))
        : Promise.resolve(new Response(JSON.stringify({ ok: true, total: 1, data: url.includes('search=other') ? [other] : [busy] })))
    )
    vi.stubGlobal('fetch', fetchMock)
    render(<PlacesBrowser onPick={vi.fn()} />)
    await userEvent.click(screen.getByRole('button', { name: /Filter & Sort/ }))
    await userEvent.click(await screen.findByRole('option', { name: 'Most liked' }))
    fireEvent.change(screen.getByPlaceholderText('Search'), { target: { value: 'other' } })
    await waitFor(() => expect(fetchMock).toHaveBeenCalledWith(expect.stringContaining('/worlds?'), expect.anything()))
    expect(await screen.findByRole('button', { name: /Other Scene/ })).toBeInTheDocument()
    expect(screen.queryByText(/Couldn't load/)).toBeNull()
  })
  it('offers Retry when a search finds neither places nor worlds because both are down', async () => {
    const fetchMock = vi.fn((url: string) =>
      url.includes('search=')
        ? Promise.reject(new Error('down'))
        : Promise.resolve(new Response(JSON.stringify({ ok: true, total: 1, data: [busy] })))
    )
    vi.stubGlobal('fetch', fetchMock)
    render(<PlacesBrowser onPick={vi.fn()} />)
    await userEvent.click(screen.getByRole('button', { name: /Filter & Sort/ }))
    await userEvent.click(await screen.findByRole('option', { name: 'Most liked' }))
    fireEvent.change(screen.getByPlaceholderText('Search'), { target: { value: 'other' } })
    expect(await screen.findByText("Couldn't load places")).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'Retry' })).toBeInTheDocument()
  })
  it('keeps loaded cards visible while Retry requests the failed next page', async () => {
    let failed = false
    vi.stubGlobal('fetch', vi.fn(async (input: string) => {
      const url = new URL(input)
      if (url.searchParams.has('only_highlighted') || url.searchParams.get('limit') === '40') return new Response(JSON.stringify({ ok: true, total: 0, data: [] }))
      if (url.searchParams.get('offset') === '48' && !failed) { failed = true; throw new Error('page unavailable') }
      return new Response(JSON.stringify({ ok: true, total: 49, data: url.searchParams.get('offset') === '48' ? [other] : [busy] }))
    }))
    render(<PlacesBrowser onPick={vi.fn()} />)
    await userEvent.click(await screen.findByRole('button', { name: 'Load more places' }))
    expect(await screen.findByText("Couldn't load more places")).toBeInTheDocument()
    expect(screen.getByRole('button', { name: /Busy Club/ })).toBeInTheDocument()
    await userEvent.click(screen.getByRole('button', { name: 'Retry' }))
    expect(await screen.findByRole('button', { name: /Other Scene/ })).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: 'Load more places' })).toBeNull()
  })

  it('disables sorting in personal sections and restores it for public discovery', async () => {
    servePlaces()
    vi.spyOn(auth, 'getLogin').mockResolvedValue(null)
    render(<PlacesBrowser onPick={vi.fn()} />)
    expect(screen.getByRole('button', { name: /Filter & Sort/ })).toBeEnabled()
    await userEvent.click(screen.getByRole('tab', { name: /Favorites/ }))
    await waitFor(() => expect(screen.getByRole('button', { name: /Filter & Sort/ })).toBeDisabled())
    await userEvent.click(screen.getByRole('tab', { name: /Explore/ }))
    expect(screen.getByRole('button', { name: /Filter & Sort/ })).toBeEnabled()
  })
  it('returns to the busiest order when Explore follows Recent through a personal section', async () => {
    servePlaces()
    vi.spyOn(auth, 'getLogin').mockResolvedValue(null)
    render(<PlacesBrowser onPick={vi.fn()} />)
    await userEvent.click(screen.getByRole('tab', { name: /Recent/ }))
    await userEvent.click(screen.getByRole('tab', { name: /Favorites/ }))
    await userEvent.click(screen.getByRole('tab', { name: /Explore/ }))
    await userEvent.click(screen.getByRole('button', { name: /Filter & Sort/ }))
    expect(screen.getByRole('option', { name: 'Most active' })).toHaveAttribute('aria-selected', 'true')
    expect(screen.getByRole('option', { name: 'Newest' })).toHaveAttribute('aria-selected', 'false')
  })
  // There are no Live Now or Featured rows: one grid holds every destination once.
  it('shows every destination once in one grid, without unfinished deploys', async () => {
    servePlaces()
    render(<PlacesBrowser onPick={vi.fn()} />)
    const results = screen.getByRole('region', { name: 'Places results' })
    expect(await within(results).findByRole('button', { name: /Busy Club/ })).toBeInTheDocument()
    expect(screen.getAllByRole('button', { name: /Busy Club/ })).toHaveLength(1)
    expect(within(results).getByRole('button', { name: /Quiet Gallery/ })).toBeInTheDocument()
    expect(within(results).getByRole('button', { name: /Curated World/ })).toBeInTheDocument()
    expect(within(results).getByRole('button', { name: /Other Scene/ })).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: /\bEmpty\b/ })).not.toBeInTheDocument()
  })

  it('shows a search match even when its record looks like an unfinished deploy', async () => {
    const untitled = place('tortilla', 'My Scene', {
      world: true,
      world_name: 'tortilla.dcl.eth',
      image: 'https://peer/content/contents/bafkreidj26s7aenyxfthfdibnqonzqm5ptc4iamml744gmcyuokewkr76y',
      user_count: 0
    })
    vi.stubGlobal(
      'fetch',
      vi.fn((url: string) => {
        const data = url.includes('search=tortilla') ? [untitled] : []
        return Promise.resolve(new Response(JSON.stringify({ ok: true, total: data.length, data })))
      })
    )
    render(<PlacesBrowser onPick={vi.fn()} />)
    fireEvent.change(screen.getByPlaceholderText('Search'), { target: { value: 'tortilla' } })
    expect(await screen.findByRole('button', { name: /My Scene/ }, { timeout: 2000 })).toBeInTheDocument()
  })

  it('marks the results loading until they answer the current search', async () => {
    let answer: ((response: Response) => void) | undefined
    vi.stubGlobal(
      'fetch',
      vi.fn((url: string) => {
        if (!url.includes('search=gallery')) return Promise.resolve(new Response(JSON.stringify({ ok: true, total: 1, data: [other] })))
        return new Promise<Response>((resolve) => {
          answer = resolve
        })
      })
    )
    render(<PlacesBrowser onPick={vi.fn()} />)
    const results = screen.getByRole('region', { name: 'Places results' })
    await screen.findByRole('button', { name: /Other Scene/ })
    expect(results).toHaveAttribute('aria-busy', 'false')

    fireEvent.change(screen.getByPlaceholderText('Search'), { target: { value: 'gallery' } })
    // the old cards stay while the box waits out its debounce: the panel says they are not this search's
    expect(screen.getByRole('button', { name: /Other Scene/ })).toBeInTheDocument()
    expect(results).toHaveAttribute('aria-busy', 'true')
    await waitFor(() => expect(answer).toBeDefined(), { timeout: 2000 })
    expect(results).toHaveAttribute('aria-busy', 'true')

    answer!(new Response(JSON.stringify({ ok: true, total: 1, data: [empty] })))
    await screen.findByRole('button', { name: /Quiet Gallery/ })
    expect(results).toHaveAttribute('aria-busy', 'false')
  })
})
