import { afterEach, describe, expect, it, vi } from 'vitest'
import { render, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { ProfilePassport } from '../features/profile/ProfilePassport'
import { marketplaceUrl, type ShopItem } from '../features/shop/shopApi'
import type { Profile } from '../engine/protocol'

// The passport's Creations tab (unity-explorer's CreationsDetailsPassportModuleController): the
// creator's wearables and emotes from the marketplace catalog, Buy with a Credits price on items on
// primary sale and View on the rest, both opening the item's shop page.
const CREATOR = '0xC0FFEE00000000000000000000000000000000AA'
const profile: Profile = { address: CREATOR, name: 'maker', hasClaimedName: false, isGuest: false }

const item = (over: Record<string, unknown>): Record<string, unknown> => ({
  id: 'x',
  name: 'Item',
  thumbnail: 'https://x/thumb.png',
  rarity: 'epic',
  category: 'wearable',
  url: '/contracts/0xc0ffee/items/0',
  isOnSale: false,
  price: '0',
  urn: 'urn:decentraland:matic:collections-v2:0xc0ffee:0',
  priceCredits: 0,
  ...over
})

const HAT = item({ id: 'hat', name: 'Neon Hat', urn: 'urn:hat', url: '/contracts/0xc0ffee/items/1', isOnSale: true, priceCredits: 120, data: { wearable: { category: 'hat' } } })
const SHIRT = item({ id: 'shirt', name: 'Old Shirt', urn: 'urn:shirt', url: '/contracts/0xc0ffee/items/2' })
const DANCE = item({ id: 'dance', name: 'Robot Dance', urn: 'urn:dance', category: 'emote', url: '/contracts/0xc0ffee/items/3', isOnSale: true, priceCredits: 0 })

function stubCatalog(byCategory: Record<string, unknown[]> | 'fail'): ReturnType<typeof vi.fn> {
  const fetchMock = vi.fn(async (input: RequestInfo | URL) => {
    if (byCategory === 'fail') return new Response('down', { status: 503 })
    const url = new URL(String(input))
    return Response.json({ data: byCategory[url.searchParams.get('category') ?? ''] ?? [], total: 0 })
  })
  vi.stubGlobal('fetch', fetchMock)
  return fetchMock
}

async function openCreations(): Promise<void> {
  render(<ProfilePassport profile={profile} onClose={vi.fn()} />)
  await userEvent.click(screen.getByRole('tab', { name: 'CREATIONS' }))
}

afterEach(() => {
  vi.unstubAllGlobals()
})

describe('passport creations', () => {
  it('asks the catalog for each category by creator, as unity-explorer does', async () => {
    const fetchMock = stubCatalog({ wearable: [HAT], emote: [] })
    await openCreations()
    await screen.findByText('Neon Hat')
    const urls = fetchMock.mock.calls.map(([u]) => new URL(String(u)))
    expect(urls.map((u) => u.pathname.endsWith('/v3/catalog/items'))).toEqual([true, true])
    expect(urls.map((u) => u.searchParams.get('category')).sort()).toEqual(['emote', 'wearable'])
    for (const u of urls) {
      expect(u.searchParams.get('creator')).toBe(CREATOR.toLowerCase())
      expect(u.searchParams.get('includeSocialEmotes')).toBe('false')
      expect(u.searchParams.get('first')).toBe('100')
    }
  })

  it('does not load until the tab is opened', () => {
    const fetchMock = stubCatalog({})
    render(<ProfilePassport profile={profile} onClose={vi.fn()} />)
    expect(fetchMock).not.toHaveBeenCalled()
  })

  it('lists wearables then emotes, with Buy and a Credits price on sale items and View on the rest', async () => {
    stubCatalog({ wearable: [HAT, SHIRT], emote: [DANCE] })
    await openCreations()
    await screen.findByText('Neon Hat')
    const titles = screen.getAllByRole('heading', { level: 2 }).map((h) => h.textContent)
    expect(titles).toEqual(['Wearables', 'Emotes'])

    const hat = screen.getByText('Neon Hat').parentElement as HTMLElement
    expect(within(hat).getByText('120 Credits')).toBeInTheDocument()
    // The Shop page's own link: the deployment's /shop on dcl-one, the marketplace upstream.
    expect(within(hat).getByRole('link', { name: 'Buy' }).getAttribute('href')).toBe(marketplaceUrl(HAT as unknown as ShopItem))

    const shirt = screen.getByText('Old Shirt').parentElement as HTMLElement
    expect(within(shirt).queryByText(/Credits/)).toBeNull()
    expect(within(shirt).getByRole('link', { name: 'View' })).toBeInTheDocument()

    // On sale without a Credits price: Buy, but no price line.
    const dance = screen.getByText('Robot Dance').parentElement as HTMLElement
    expect(within(dance).getByRole('link', { name: 'Buy' })).toBeInTheDocument()
    expect(within(dance).queryByText(/Credits/)).toBeNull()
  })

  it('drops a section with no items', async () => {
    stubCatalog({ wearable: [], emote: [DANCE] })
    await openCreations()
    await screen.findByText('Robot Dance')
    expect(screen.queryByRole('heading', { name: 'Wearables' })).toBeNull()
  })

  it('says so when the player has created nothing', async () => {
    stubCatalog({ wearable: [], emote: [] })
    await openCreations()
    expect(await screen.findByText('There are no creations to show yet.')).toBeInTheDocument()
  })

  it('gives an item with no shop page no button', async () => {
    stubCatalog({ wearable: [item({ id: 'draft', name: 'Draft Cape', urn: 'urn:draft', url: '' })], emote: [] })
    await openCreations()
    const draft = (await screen.findByText('Draft Cape')).parentElement as HTMLElement
    expect(within(draft).queryByRole('link')).toBeNull()
  })

  it('shows the error state when only one category fails', async () => {
    vi.stubGlobal('fetch', vi.fn(async (input: RequestInfo | URL) => {
      const category = new URL(String(input)).searchParams.get('category')
      return category === 'emote' ? new Response('down', { status: 503 }) : Response.json({ data: [HAT] })
    }))
    await openCreations()
    expect(await screen.findByText("Couldn't load the creations")).toBeInTheDocument()
    expect(screen.queryByText('Neon Hat')).toBeNull()
  })

  it('does not load again when the same address arrives in another case', async () => {
    const fetchMock = stubCatalog({ wearable: [HAT], emote: [] })
    const { rerender } = render(<ProfilePassport profile={profile} onClose={vi.fn()} />)
    await userEvent.click(screen.getByRole('tab', { name: 'CREATIONS' }))
    await screen.findByText('Neon Hat')
    rerender(<ProfilePassport profile={{ ...profile, address: CREATOR.toLowerCase() }} onClose={vi.fn()} />)
    expect(screen.getByText('Neon Hat')).toBeInTheDocument()
    expect(fetchMock).toHaveBeenCalledTimes(2)
  })

  it('never shows a previous player\'s creations after the address changes', async () => {
    const replies: Array<(r: Response) => void> = []
    vi.stubGlobal('fetch', vi.fn(() => new Promise<Response>((resolve) => replies.push(resolve))))
    const { rerender } = render(<ProfilePassport profile={profile} onClose={vi.fn()} />)
    await userEvent.click(screen.getByRole('tab', { name: 'CREATIONS' }))
    const OTHER = '0x00000000000000000000000000000000000000bb'
    rerender(<ProfilePassport profile={{ ...profile, address: OTHER }} onClose={vi.fn()} />)
    // The first player's replies land after the switch; the second player's never do.
    for (const reply of replies.splice(0, 2)) reply(Response.json({ data: [HAT] }))
    await new Promise((r) => setTimeout(r, 0))
    expect(screen.queryByText('Neon Hat')).toBeNull()
    expect(screen.getByRole('status', { name: 'Loading' })).toBeInTheDocument()
  })

  it('shows an error with Retry when the catalog fails, and Retry loads again', async () => {
    stubCatalog('fail')
    await openCreations()
    expect(await screen.findByText("Couldn't load the creations")).toBeInTheDocument()
    const retried = stubCatalog({ wearable: [HAT], emote: [] })
    await userEvent.click(screen.getByRole('button', { name: 'Retry' }))
    expect(await screen.findByText('Neon Hat')).toBeInTheDocument()
    expect(retried).toHaveBeenCalledTimes(2)
  })
})
