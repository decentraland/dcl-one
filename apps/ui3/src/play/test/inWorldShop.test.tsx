import { describe, it, expect, vi, afterEach, beforeEach } from 'vitest'
import { fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import InWorldShop, { type InWorldShopProps } from '../features/shop/InWorldShop'
import { tryOnUrn, webShopUrl } from '../features/shop/itemLinks'
import { primeRemoteFlags } from '../../shop/discovery/lib/remoteFlags'
import { defaultFilters, pageCount, readsListings, serverSort, SHOP_PAGE_SIZE, shopQueries, type CatalogueFilters } from '../features/shop/catalogueQuery'
import { PopupHost, resetPopups } from '../design'
import type { Wearable } from '../engine/protocol'
import { fakeProfileState } from './harness'

const CONTRACT = '0x1111111111111111111111111111111111111111'
const CREATOR = '0x2222222222222222222222222222222222222222'
const ARMOR_URN = `urn:decentraland:matic:collections-v2:${CONTRACT}:1`

// A row of the catalogue (/v3/catalog/items), the feed of every status but "on sale".
const catalogueRow = (over: Record<string, unknown> = {}): Record<string, unknown> => ({
  id: `${CONTRACT}-1`,
  name: 'Golden Armor',
  creator: CREATOR,
  contractAddress: CONTRACT,
  itemId: '1',
  category: 'wearable',
  rarity: 'epic',
  network: 'MATIC',
  chainId: 137,
  thumbnail: 'https://example.com/armor.png',
  priceCredits: 500,
  price: '5000000000000000000',
  available: 35,
  isOnSale: true,
  data: { wearable: { category: 'upper_body', bodyShapes: [], isSmart: false } },
  ...over
})

// A row of the listings feed (/v3/catalog/unified): what is on sale, trending and new.
const listingRow = (over: Record<string, unknown> = {}): Record<string, unknown> => ({
  tradeId: null,
  listingType: 'primary',
  source: 'native',
  acquisition: 'store',
  contractAddress: CONTRACT,
  itemId: '1',
  tokenId: null,
  name: 'Golden Armor',
  thumbnail: 'https://example.com/armor.png',
  rarity: 'epic',
  category: 'wearable',
  wearableCategory: 'upper_body',
  creator: CREATOR,
  priceCredits: 500,
  manaWei: null,
  available: 35,
  network: 'MATIC',
  chainId: 137,
  ...over
})

type Feed = Record<string, unknown>[] | 'down'
interface Routes {
  catalogue?: Feed[]
  listings?: Feed[]
  trending?: Record<string, unknown>[] | 'down'
  outfits?: unknown[]
  byIds?: Record<string, unknown>[]
  entity?: { description: string; hides: string[] }
  total?: number
  /** The feature-flags service's dapps.json; without it the file is missing (every flag off). */
  flags?: Record<string, boolean>
}

const answer = (queue: Feed[] | undefined, total?: number): Response => {
  const next = queue == null || queue.length === 0 ? [] : queue.length > 1 ? queue.shift()! : queue[0]!
  if (next === 'down') return new Response('', { status: 503 })
  return Response.json({ data: next, total: total ?? next.length })
}

/** The Shop's endpoints by path, and the flags file; anything else (profiles) is a 404. */
function serve(routes: Routes): ReturnType<typeof vi.fn> {
  const f = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
    const url = new URL(String(input instanceof Request ? input.url : input), location.href)
    const path = url.pathname
    if (path.endsWith('/v3/catalog/items')) return url.searchParams.has('id') ? Response.json({ data: routes.byIds ?? [] }) : answer(routes.catalogue, routes.total)
    if (path.endsWith('/v3/catalog/unified')) return answer(routes.listings, routes.total)
    if (path.endsWith('/v3/catalog/trending')) return routes.trending === 'down' ? new Response('', { status: 503 }) : Response.json({ data: routes.trending ?? [] })
    if (path.endsWith('/v1/outfits')) return Response.json({ outfits: routes.outfits ?? [] })
    if (path.endsWith('/dapps.json') && routes.flags != null) return Response.json({ flags: routes.flags })
    if (path.endsWith('/content/entities/active') && init?.method === 'POST') {
      const e = routes.entity
      return Response.json(e == null ? [] : [{ metadata: { description: e.description, data: { hides: e.hides } } }])
    }
    return new Response('', { status: 404 })
  })
  vi.stubGlobal('fetch', f)
  return f
}

const calls = (f: ReturnType<typeof vi.fn>, suffix: string): URL[] =>
  f.mock.calls.map(([input]) => new URL(String(input instanceof Request ? input.url : input), location.href)).filter((u) => u.pathname.endsWith(suffix))

// The look the engine reports: the body shape first, then the wearables.
const BODY = 'urn:decentraland:off-chain:base-avatars:BaseMale'
const LOOK = [
  { urn: BODY, category: 'body_shape' },
  { urn: 'urn:old-shirt', category: 'upper_body' },
  { urn: 'urn:jeans', category: 'lower_body' }
] as Wearable[]

/** The page loads its chunk, its feed and its profiles first, so the first card may take a while under load. */
const findCard = (name = 'Golden Armor'): Promise<HTMLElement> => screen.findByText(name, {}, { timeout: 5000 })
const cardOf = async (name = 'Golden Armor'): Promise<HTMLElement> => (await findCard(name)).closest('[data-rarity]') as HTMLElement

function renderShop(over: Partial<InWorldShopProps> = {}) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } })
  const props: InWorldShopProps = {
    shop: { open: true, toggle: vi.fn() },
    profile: fakeProfileState(),
    onNavigate: vi.fn(),
    setEngineViewport: vi.fn(),
    equipped: LOOK,
    preview: vi.fn(),
    focus: vi.fn(),
    stage: vi.fn(),
    emote: vi.fn(),
    ...over
  }
  const view = render(
    <QueryClientProvider client={client}>
      <InWorldShop {...props} />
      <PopupHost />
    </QueryClientProvider>
  )
  const rerender = (next: Partial<InWorldShopProps>): void =>
    view.rerender(
      <QueryClientProvider client={client}>
        <InWorldShop {...props} {...next} />
        <PopupHost />
      </QueryClientProvider>
    )
  return { props, rerender, unmount: view.unmount }
}

async function openTab(name: 'Wearables' | 'Emotes' | 'All'): Promise<void> {
  await userEvent.click(screen.getByRole('tab', { name }))
}

// jsdom has no element scrolling, which the home's rails use.
const scrollTo = HTMLElement.prototype.scrollTo
beforeEach(() => {
  HTMLElement.prototype.scrollTo = function (): void {}
})
afterEach(() => {
  HTMLElement.prototype.scrollTo = scrollTo
  // A flags answer is handed to the web Shop's fetchers' module (flagsQuery): forget it between tests.
  primeRemoteFlags(null)
  resetPopups()
  vi.restoreAllMocks()
  vi.unstubAllGlobals()
})

describe('in-world shop catalogue query', () => {
  const base = defaultFilters('wearable')

  it('keys every input that changes a page: the filters, the page and the creator-sales flag', () => {
    const key = (f: CatalogueFilters, page = 0, creatorSales = false): string => JSON.stringify(shopQueries.catalogue(f, page, { creatorSales }).queryKey)
    const keys = [key(base), key({ ...base, category: 'Feet' }), key({ ...base, sort: 'cheapest' }), key({ ...base, search: 'armor' }), key({ ...base, rarities: ['epic'] }), key({ ...base, status: 'on_sale' }), key(base, 1), key(base, 0, true)]
    expect(new Set(keys).size).toBe(keys.length)
  })

  it('reads the listings feed for what is on sale and deals, the catalogue otherwise', () => {
    expect(readsListings({ ...base, status: 'on_sale' }, false)).toBe(true)
    expect(readsListings({ ...base, deals: true }, true)).toBe(true)
    expect(readsListings({ ...base, deals: true }, false)).toBe(false)
    expect(readsListings(base, true)).toBe(false)
  })

  it('orders a search by relevance unless another sort was picked, and counts pages', () => {
    expect(serverSort({ search: 'armor', sort: 'newest' })).toBe('relevance')
    expect(serverSort({ search: 'armor', sort: 'cheapest' })).toBe('cheapest')
    expect(pageCount(0)).toBe(1)
    expect(pageCount(SHOP_PAGE_SIZE)).toBe(1)
    expect(pageCount(SHOP_PAGE_SIZE + 1)).toBe(2)
  })

  it('tries on an item by its own URN, else its Polygon collection URN', () => {
    const item = { contractAddress: CONTRACT, itemId: '1', chainId: 137, category: 'wearable' } as Parameters<typeof tryOnUrn>[0]
    expect(tryOnUrn(item)).toBe(ARMOR_URN)
    expect(tryOnUrn({ ...item, chainId: 1 })).toBeNull()
    expect(tryOnUrn({ ...item, chainId: 1, urn: 'urn:own' })).toBe('urn:own')
    expect(webShopUrl(item)).toContain(`/shop/item/${CONTRACT}/1`)
    // Each segment is encoded, and an item without an id has no page to offer.
    expect(webShopUrl({ ...item, itemId: 'a/b' })).toContain(`/shop/item/${CONTRACT}/a%2Fb`)
    expect(webShopUrl({ ...item, itemId: null } as unknown as typeof item)).toBeNull()
  })
})

describe('in-world shop', () => {
  it('opens on the wearables catalogue, as the design does', async () => {
    const f = serve({ catalogue: [[catalogueRow()]] })
    renderShop()
    expect(screen.getByRole('tab', { name: 'Wearables' })).toHaveAttribute('aria-selected', 'true')
    expect(await findCard('Golden Armor')).toBeInTheDocument()
    expect(calls(f, '/v3/catalog/items').at(-1)!.searchParams.get('category')).toBe('wearable')
  })

  it('shows the All home with its sections', async () => {
    serve({ trending: [listingRow()], listings: [[listingRow({ itemId: '2', name: 'Pixel Hoodie' })]] })
    renderShop()
    await openTab('All')
    expect(await findCard('Golden Armor')).toBeInTheDocument()
    expect(await findCard('Pixel Hoodie')).toBeInTheDocument()
    expect(screen.getByRole('region', { name: 'Trending' })).toBeInTheDocument()
    expect(screen.getByRole('region', { name: 'New' })).toBeInTheDocument()
  })

  it("sends New's VIEW ALL to the wearables on sale, newest first", async () => {
    const f = serve({ trending: [listingRow()], listings: [[listingRow()]] })
    renderShop()
    await openTab('All')
    await screen.findAllByText('Golden Armor', {}, { timeout: 5000 })
    const before = calls(f, '/v3/catalog/unified').length
    await userEvent.click(within(screen.getByRole('region', { name: 'New' })).getByRole('button', { name: /view all/i }))
    expect(screen.getByRole('tab', { name: 'Wearables' })).toHaveAttribute('aria-selected', 'true')
    await waitFor(() => expect(calls(f, '/v3/catalog/unified').length).toBeGreaterThan(before))
    expect(calls(f, '/v3/catalog/unified').at(-1)!.searchParams.get('category')).toBe('wearable')
    expect(calls(f, '/v3/catalog/unified').at(-1)!.searchParams.get('sortBy')).toBe('newest')
  })

  it("sends Trending's VIEW ALL to the catalogue as it opens, as the web Shop does", async () => {
    const f = serve({ trending: [listingRow()], listings: [[listingRow()]], catalogue: [[catalogueRow()]] })
    renderShop()
    await openTab('All')
    await screen.findAllByText('Golden Armor', {}, { timeout: 5000 })
    await userEvent.click(within(screen.getByRole('region', { name: 'Trending' })).getByRole('button', { name: /view all/i }))
    expect(screen.getByRole('tab', { name: 'Wearables' })).toHaveAttribute('aria-selected', 'true')
    // Every status, the catalogue's own feed: there is no trending order to keep.
    await waitFor(() => expect(calls(f, '/v3/catalog/items').at(-1)!.searchParams.get('category')).toBe('wearable'))
  })

  it('shows what was searched for on the All home in the catalogue', async () => {
    const f = serve({ trending: [listingRow()], listings: [[listingRow()]], catalogue: [[catalogueRow()]] })
    renderShop()
    await openTab('All')
    await userEvent.type(screen.getByPlaceholderText('Search'), 'armor')
    await waitFor(() => expect(screen.getByRole('tab', { name: 'Wearables' })).toHaveAttribute('aria-selected', 'true'))
    expect(await screen.findByText(/Results for ‘armor’/)).toBeInTheDocument()
    await waitFor(() => expect(calls(f, '/v3/catalog/items').at(-1)!.searchParams.get('search')).toBe('armor'))
  })

  it('keeps the rows that loaded and offers Retry on the one that failed', async () => {
    const f = serve({ trending: 'down', listings: [[listingRow({ itemId: '2', name: 'Pixel Hoodie' })]] })
    renderShop()
    await openTab('All')
    expect(await findCard('Pixel Hoodie')).toBeInTheDocument()
    const trending = await screen.findByRole('region', { name: 'Trending' })
    expect(within(trending).getByRole('alert')).toHaveTextContent('Couldn’t load these items.')
    const before = calls(f, '/v3/catalog/trending').length
    await userEvent.click(within(trending).getByRole('button', { name: 'Retry' }))
    await waitFor(() => expect(calls(f, '/v3/catalog/trending').length).toBeGreaterThan(before))
  })

  it("pages the home's rows with their arrows, without dots, as the design draws them", async () => {
    const rows = Array.from({ length: 8 }, (_, i) => listingRow({ itemId: String(i + 1), name: `Item ${i + 1}` }))
    serve({ trending: rows, listings: [rows] })
    renderShop()
    await openTab('All')
    const trending = await screen.findByRole('region', { name: 'Trending' })
    await within(trending).findByText('Item 1', {}, { timeout: 5000 })
    expect(within(trending).getByRole('button', { name: 'Next' })).toBeInTheDocument()
    expect(within(trending).queryByRole('button', { name: /^Page \d/ })).not.toBeInTheDocument()
  })

  it('says so when the All home has nothing to show', async () => {
    serve({ trending: [], listings: [[]], outfits: [] })
    renderShop()
    await openTab('All')
    expect(await screen.findByText('Nothing on sale right now', {}, { timeout: 5000 })).toBeInTheDocument()
  })

  it('lets a newer pick win over an outfit still loading', async () => {
    let release: () => void = () => {}
    const f = serve({
      outfits: [{ id: 'o1', name: 'Neon Night', thumbnailHash: '', items: [{ contractAddress: CONTRACT, itemId: '1' }], gradientFrom: '#ff0000', gradientTo: '#0000ff' }],
      trending: [listingRow({ itemId: '2', name: 'Moonwalk', category: 'emote' })],
      byIds: [catalogueRow()]
    })
    const served = f.getMockImplementation() as (input: RequestInfo | URL, init?: RequestInit) => Promise<Response>
    f.mockImplementation(async (input: RequestInfo | URL, init?: RequestInit) => {
      const url = new URL(String(input), location.href)
      if (url.pathname.endsWith('/v3/catalog/items') && url.searchParams.has('id')) await new Promise<void>((r) => (release = r))
      return served(input, init)
    })
    const { props } = renderShop()
    await openTab('All')
    await userEvent.click(await screen.findByRole('button', { name: 'Try on Neon Night' }, { timeout: 5000 }))
    // An emote (no try-on yet) picked while the outfit's items are still on their way.
    await userEvent.click(await screen.findByRole('button', { name: 'Moonwalk' }))
    expect(screen.getByRole('complementary', { name: 'Moonwalk' })).toBeInTheDocument()
    release()
    await new Promise((r) => setTimeout(r, 50))
    expect(props.preview).not.toHaveBeenCalledWith([BODY, ARMOR_URN])
    expect(screen.getByRole('complementary', { name: 'Moonwalk' })).toBeInTheDocument()
  })

  it("doesn't dress the avatar in an outfit whose items answer after the Shop closed", async () => {
    let release: () => void = () => {}
    const f = serve({
      outfits: [{ id: 'o1', name: 'Neon Night', thumbnailHash: '', items: [{ contractAddress: CONTRACT, itemId: '1' }], gradientFrom: '#ff0000', gradientTo: '#0000ff' }],
      byIds: [catalogueRow()]
    })
    const served = f.getMockImplementation() as (input: RequestInfo | URL, init?: RequestInit) => Promise<Response>
    f.mockImplementation(async (input: RequestInfo | URL, init?: RequestInit) => {
      const url = new URL(String(input), location.href)
      if (url.pathname.endsWith('/v3/catalog/items') && url.searchParams.has('id')) await new Promise<void>((r) => (release = r))
      return served(input, init)
    })
    const { props, rerender } = renderShop()
    await openTab('All')
    await userEvent.click(await screen.findByRole('button', { name: 'Try on Neon Night' }, { timeout: 5000 }))
    rerender({ shop: { open: false, toggle: vi.fn() } })
    release()
    await new Promise((r) => setTimeout(r, 50))
    expect(props.preview).not.toHaveBeenCalledWith([BODY, ARMOR_URN])
  })

  // An outfit of two items: the Golden Armor and a pair of boots.
  const TWO_PIECE = { id: 'o1', name: 'Neon Night', thumbnailHash: '', items: [{ contractAddress: CONTRACT, itemId: '1' }, { contractAddress: CONTRACT, itemId: '2' }], gradientFrom: '#ff0000', gradientTo: '#0000ff' }
  const boots = (over: Record<string, unknown> = {}): Record<string, unknown> => catalogueRow({ id: `${CONTRACT}-2`, itemId: '2', name: 'Silver Boots', priceCredits: 250, ...over })
  const outfitTile = async (): Promise<HTMLElement> => (await screen.findByRole('button', { name: 'Try on Neon Night' }, { timeout: 5000 })).parentElement!

  it('counts an outfit’s items and prices them when every one sells in credits', async () => {
    serve({ outfits: [TWO_PIECE], byIds: [catalogueRow(), boots()] })
    renderShop()
    await openTab('All')
    const tile = await outfitTile()
    expect(within(tile).getByText('2 Items')).toBeInTheDocument()
    expect(await within(tile).findByText('750')).toBeInTheDocument()
  })

  it('sends an outfit to the web Shop on its BUY outside the web shell, after asking', async () => {
    const open = vi.spyOn(window, 'open').mockReturnValue(null)
    serve({ outfits: [TWO_PIECE], byIds: [catalogueRow(), boots()] })
    renderShop()
    await openTab('All')
    await userEvent.click(within(await outfitTile()).getByRole('button', { name: 'BUY' }))
    const dialog = await screen.findByRole('dialog', {}, { timeout: 5000 })
    expect(within(dialog).getByText('You will be redirected to the web Shop')).toBeInTheDocument()
    await userEvent.click(within(dialog).getByRole('button', { name: 'CONTINUE' }))
    expect(open).toHaveBeenCalledWith(expect.stringContaining('/shop/items/outfits/o1'), '_blank', 'noopener')
  })

  it('leaves an outfit unpriced when one of its items doesn’t sell in credits or didn’t load', async () => {
    serve({ outfits: [TWO_PIECE], byIds: [catalogueRow(), boots({ priceCredits: 0, price: null, isOnSale: false })] })
    renderShop()
    await openTab('All')
    const tile = await outfitTile()
    await waitFor(() => expect(within(tile).getByText('2 Items')).toBeInTheDocument())
    await new Promise((r) => setTimeout(r, 50))
    expect(within(tile).queryByText('500')).toBeNull()
    expect(within(tile).queryByText('750')).toBeNull()
  })

  it('leaves an outfit unpriced when one of its items is missing from the catalogue', async () => {
    serve({ outfits: [TWO_PIECE], byIds: [catalogueRow()] })
    renderShop()
    await openTab('All')
    const tile = await outfitTile()
    await new Promise((r) => setTimeout(r, 50))
    expect(within(tile).getByText('2 Items')).toBeInTheDocument()
    expect(within(tile).queryByText('500')).toBeNull()
  })

  it('takes a sort picked on the All home into the wearables catalogue', async () => {
    const f = serve({ trending: [listingRow()], listings: [[listingRow()]], catalogue: [[catalogueRow()]] })
    renderShop()
    await openTab('All')
    await userEvent.click(screen.getByRole('button', { name: 'Sort by: Newest' }))
    await userEvent.click(screen.getByText('Cheapest'))
    expect(screen.getByRole('tab', { name: 'Wearables' })).toHaveAttribute('aria-selected', 'true')
    await waitFor(() => expect(calls(f, '/v3/catalog/items').at(-1)!.searchParams.get('sortBy')).toBe('cheapest'))
    expect(calls(f, '/v3/catalog/items').at(-1)!.searchParams.get('category')).toBe('wearable')
  })

  it('dresses the avatar in an outfit, keeping the body shape', async () => {
    serve({
      outfits: [{ id: 'o1', name: 'Neon Night', thumbnailHash: '', items: [{ contractAddress: CONTRACT, itemId: '1' }], gradientFrom: '#ff0000', gradientTo: '#0000ff' }],
      byIds: [catalogueRow()]
    })
    const { props } = renderShop()
    await openTab('All')
    await userEvent.click(await screen.findByRole('button', { name: 'Try on Neon Night' }, { timeout: 5000 }))
    await waitFor(() => expect(props.preview).toHaveBeenLastCalledWith([BODY, ARMOR_URN]))
  })

  it('lists every wearable a page at a time, with its credits price and creator', async () => {
    const f = serve({ catalogue: [[catalogueRow()]], total: 120 })
    renderShop()
    await openTab('Wearables')
    const card = await cardOf()
    expect(within(card).getByText('500')).toBeInTheDocument()
    expect(within(card).getByText('epic')).toBeInTheDocument()
    expect(within(card).getByText('0x2222…2222')).toBeInTheDocument()
    const q = calls(f, '/v3/catalog/items').at(-1)!.searchParams
    expect(q.get('category')).toBe('wearable')
    expect(q.get('first')).toBe(String(SHOP_PAGE_SIZE))
    expect(q.get('skip')).toBe('0')
    await userEvent.click(screen.getByRole('button', { name: '2' }))
    await waitFor(() => expect(calls(f, '/v3/catalog/items').at(-1)!.searchParams.get('skip')).toBe(String(SHOP_PAGE_SIZE)))
  })

  it('prices a resale in MANA', async () => {
    serve({ listings: [[listingRow({ listingType: 'secondary', tokenId: '7', tradeId: 'trade-1', priceCredits: 0, manaWei: '12500000000000000000' })]] })
    renderShop()
    await openTab('Wearables')
    await userEvent.click(screen.getByRole('button', { name: 'Filters' }))
    await userEvent.click(screen.getByRole('radio', { name: 'On sale' }))
    expect(within(await cardOf()).getByText('12.5')).toBeInTheDocument()
  })

  it('tags a discounted credits price while creator sales are on', async () => {
    serve({ listings: [[listingRow({ priceCredits: 400, compareAtCredits: 500 })]], flags: { 'dapps-shop-creator-sales': true } })
    renderShop()
    await openTab('Wearables')
    await userEvent.click(screen.getByRole('button', { name: 'Filters' }))
    await userEvent.click(screen.getByRole('radio', { name: 'On sale' }))
    // The flag arrives with the flags file; the page asks again for the discounted prices once it has.
    await screen.findByText('-20%', {}, { timeout: 5000 })
    const card = await cardOf()
    expect(within(card).getByText('-20%')).toBeInTheDocument()
    expect(within(card).getByText('400')).toBeInTheDocument()
  })

  it('narrows by category, rarity and price', async () => {
    const f = serve({ catalogue: [[catalogueRow()]] })
    renderShop()
    await openTab('Wearables')
    await findCard()
    await userEvent.click(screen.getByRole('button', { name: 'Upper body' }))
    await waitFor(() => expect(calls(f, '/v3/catalog/items').at(-1)!.searchParams.get('wearableCategory')).toBe('upper_body'))
    await userEvent.click(screen.getByRole('button', { name: 'Filters' }))
    await userEvent.click(screen.getByRole('button', { name: 'epic' }))
    await waitFor(() => expect(calls(f, '/v3/catalog/items').at(-1)!.searchParams.get('rarity')).toBe('epic'))
    fireEvent.change(screen.getByRole('textbox', { name: 'Minimum price' }), { target: { value: '100' } })
    fireEvent.blur(screen.getByRole('textbox', { name: 'Minimum price' }))
    await waitFor(() => expect(calls(f, '/v3/catalog/items').at(-1)!.searchParams.get('minPriceCredits')).toBe('100'))
  })

  it('sorts by the option picked', async () => {
    const f = serve({ catalogue: [[catalogueRow()]] })
    renderShop()
    await openTab('Wearables')
    await findCard()
    await userEvent.click(screen.getByRole('button', { name: 'Sort by: Newest' }))
    await userEvent.click(screen.getByText('Cheapest'))
    await waitFor(() => expect(calls(f, '/v3/catalog/items').at(-1)!.searchParams.get('sortBy')).toBe('cheapest'))
  })

  it('searches, counts the results and leaves the search', async () => {
    const f = serve({ catalogue: [[catalogueRow()]] })
    renderShop()
    await openTab('Wearables')
    await findCard()
    fireEvent.change(screen.getByPlaceholderText('Search'), { target: { value: '  Armor ' } })
    await waitFor(() => expect(calls(f, '/v3/catalog/items').at(-1)!.searchParams.get('search')).toBe('armor'))
    expect(calls(f, '/v3/catalog/items').at(-1)!.searchParams.get('sortBy')).toBe('relevance')
    expect(await screen.findByText('Results for ‘armor’ (1)')).toBeInTheDocument()
    await userEvent.click(screen.getByRole('button', { name: 'Exit search' }))
    await waitFor(() => expect(screen.queryByText(/Results for/)).toBeNull())
  })

  it('offers to explore every wearable when a narrowed list is empty', async () => {
    serve({ catalogue: [[catalogueRow()], []] })
    renderShop()
    await openTab('Wearables')
    await findCard()
    await userEvent.click(screen.getByRole('button', { name: 'Feet' }))
    expect(await screen.findByText('No matches found', {}, { timeout: 5000 })).toBeInTheDocument()
    await userEvent.click(screen.getByRole('button', { name: 'EXPLORE WEARABLES' }))
    expect(await findCard()).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'All' })).toHaveAttribute('aria-pressed', 'true')
  })

  it('says nothing is on sale when the whole list is empty', async () => {
    serve({ catalogue: [[]] })
    renderShop()
    await openTab('Wearables')
    expect(await screen.findByText('Nothing on sale right now', {}, { timeout: 5000 })).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: 'EXPLORE WEARABLES' })).toBeNull()
  })

  it('shows the error and retries', async () => {
    serve({ catalogue: ['down', [catalogueRow()]] })
    renderShop()
    await openTab('Wearables')
    expect(await screen.findByText("Couldn't load the Shop", {}, { timeout: 5000 })).toBeInTheDocument()
    await userEvent.click(screen.getByRole('button', { name: 'Retry' }))
    expect(await findCard()).toBeInTheDocument()
  })

  it('opens the panel of a selected item, with its stock, description and what it hides, and tries it on', async () => {
    serve({ catalogue: [[catalogueRow()]], entity: { description: 'No flesh, just bones!', hides: ['eyebrows', 'tiara'] } })
    const { props } = renderShop()
    await openTab('Wearables')
    await findCard()
    await userEvent.click(screen.getByRole('button', { name: 'Golden Armor' }))
    const panel = screen.getByRole('complementary', { name: 'Golden Armor' })
    expect(within(panel).getByText(/epic\s*35\/1000/i)).toBeInTheDocument()
    expect(await within(panel).findByText('No flesh, just bones!')).toBeInTheDocument()
    expect(within(panel).getByText('tiara')).toBeInTheDocument()
    expect(within(panel).getByRole('button', { name: /buy item/i })).toBeInTheDocument()
    expect(props.preview).toHaveBeenLastCalledWith([BODY, 'urn:jeans', ARMOR_URN])
    await userEvent.click(within(panel).getByRole('button', { name: 'Close item' }))
    expect(screen.queryByRole('complementary', { name: 'Golden Armor' })).toBeNull()
    expect(props.preview).toHaveBeenLastCalledWith(null)
  })

  it('sends what is not for sale here to the web Shop, after asking', async () => {
    const open = vi.spyOn(window, 'open').mockReturnValue(null)
    serve({ catalogue: [[catalogueRow({ isOnSale: false, price: '0', priceCredits: 0 })]] })
    renderShop()
    await openTab('Wearables')
    await userEvent.click(within(await cardOf()).getByRole('button', { name: /view/i }))
    const dialog = await screen.findByRole('dialog', {}, { timeout: 5000 })
    expect(within(dialog).getByText('You will be redirected to the web Shop')).toBeInTheDocument()
    await userEvent.click(within(dialog).getByRole('button', { name: 'CONTINUE' }))
    expect(open).toHaveBeenCalledWith(expect.stringContaining(`/shop/item/${CONTRACT}/1`), '_blank', 'noopener')
  })

  it('frames the whole avatar when it opens', () => {
    serve({})
    const { props } = renderShop()
    expect(props.focus).toHaveBeenCalledWith('body')
  })

  it('tries an item on over the current look and takes it off again, then restores the look on close', async () => {
    serve({ catalogue: [[catalogueRow()]] })
    const { props, rerender } = renderShop()
    await openTab('Wearables')
    await findCard()
    await userEvent.click(screen.getByRole('button', { name: 'TRY ON' }))
    expect(props.preview).toHaveBeenLastCalledWith([BODY, 'urn:jeans', ARMOR_URN])
    await userEvent.click(screen.getByRole('button', { name: 'TAKE OFF' }))
    expect(props.preview).toHaveBeenLastCalledWith(null)
    await userEvent.click(screen.getByRole('button', { name: 'TRY ON' }))
    rerender({ shop: { open: false, toggle: vi.fn() } })
    expect(props.preview).toHaveBeenLastCalledWith(null)
  })

  it('waits for the look before trying anything on', async () => {
    serve({ catalogue: [[catalogueRow()]] })
    const { props } = renderShop({ equipped: [] })
    await openTab('Wearables')
    await findCard()
    expect(screen.getByRole('button', { name: 'TRY ON' })).toBeDisabled()
    expect(props.preview).not.toHaveBeenCalled()
  })

  it('offers no try-on for an item without a Polygon collection URN', async () => {
    serve({ catalogue: [[catalogueRow({ chainId: 1, network: 'ETHEREUM' })]] })
    renderShop()
    await openTab('Wearables')
    await findCard()
    expect(screen.queryByRole('button', { name: 'TRY ON' })).toBeNull()
  })

  it('leaves the preview avatar alone when nothing was tried on, and restores it on unmount when something was', async () => {
    serve({ catalogue: [[catalogueRow()]] })
    const first = renderShop()
    await openTab('Wearables')
    await findCard()
    first.rerender({ shop: { open: false, toggle: vi.fn() } })
    expect(first.props.preview).not.toHaveBeenCalled()
    first.unmount()
    serve({ catalogue: [[catalogueRow()]] })
    const second = renderShop()
    await openTab('Wearables')
    await findCard()
    await userEvent.click(screen.getByRole('button', { name: 'TRY ON' }))
    second.unmount()
    expect(second.props.preview).toHaveBeenLastCalledWith(null)
  })

  it('browses emotes by their own categories', async () => {
    const f = serve({ catalogue: [[catalogueRow({ category: 'emote', name: 'Moonwalk', data: { emote: { category: 'dance' } } })]] })
    renderShop()
    await openTab('Emotes')
    expect(await findCard('Moonwalk')).toBeInTheDocument()
    expect(calls(f, '/v3/catalog/items').at(-1)!.searchParams.get('category')).toBe('emote')
    await userEvent.click(screen.getByRole('button', { name: 'Dance' }))
    await waitFor(() => expect(calls(f, '/v3/catalog/items').at(-1)!.searchParams.get('emoteCategory')).toBe('dance'))
  })

  it("doesn't show one kind's cards while the other kind loads", async () => {
    const f = serve({ catalogue: [[catalogueRow()]] })
    const served = f.getMockImplementation() as (input: RequestInfo | URL, init?: RequestInit) => Promise<Response>
    // The emotes never answer: the wearables must not stand in for them meanwhile.
    f.mockImplementation(async (input: RequestInfo | URL, init?: RequestInit) =>
      new URL(String(input), location.href).searchParams.get('category') === 'emote' ? new Promise<Response>(() => {}) : served(input, init)
    )
    renderShop()
    await findCard()
    await openTab('Emotes')
    expect(screen.queryByText('Golden Armor')).not.toBeInTheDocument()
    expect(screen.getByRole('status', { name: 'Loading the catalogue' })).toBeInTheDocument()
  })

  it("keeps emotes to browse and open, without trying them on, while the engine can't play them", async () => {
    serve({ catalogue: [[catalogueRow({ category: 'emote', name: 'Moonwalk', data: { emote: { category: 'dance' } } })]] })
    const { props } = renderShop()
    await openTab('Emotes')
    await findCard('Moonwalk')
    expect(screen.queryByRole('button', { name: 'TRY ON' })).not.toBeInTheDocument()
    await userEvent.click(screen.getByRole('button', { name: 'Moonwalk' }))
    expect(screen.getByRole('complementary', { name: 'Moonwalk' })).toBeInTheDocument()
    expect(props.emote).not.toHaveBeenCalled()
    expect(screen.queryByRole('button', { name: 'STOP EMOTE' })).not.toBeInTheDocument()
  })

  it('keeps the open tab as it is when it is clicked again', async () => {
    const f = serve({ catalogue: [[catalogueRow()]] })
    renderShop()
    await findCard()
    await userEvent.click(screen.getByRole('button', { name: 'Feet' }))
    await waitFor(() => expect(calls(f, '/v3/catalog/items').at(-1)!.searchParams.get('wearableCategory')).toBe('feet'))
    const before = calls(f, '/v3/catalog/items').length
    await openTab('Wearables')
    expect(screen.getByRole('button', { name: 'Feet' })).toHaveAttribute('aria-pressed', 'true')
    expect(calls(f, '/v3/catalog/items').length).toBe(before)
  })

  it('waits for the creator-sales answer before reading the listings', async () => {
    const f = serve({ catalogue: [[catalogueRow()]] })
    const served = f.getMockImplementation() as (input: RequestInfo | URL, init?: RequestInit) => Promise<Response>
    let answer: () => void = () => {}
    f.mockImplementation(async (input: RequestInfo | URL, init?: RequestInit) => {
      if (String(input).endsWith('/dapps.json')) {
        await new Promise<void>((r) => (answer = r))
        return Response.json({ flags: {} })
      }
      return served(input, init)
    })
    renderShop()
    await new Promise((r) => setTimeout(r, 50))
    expect(calls(f, '/v3/catalog/items')).toHaveLength(0)
    expect(screen.getByRole('status', { name: 'Loading the catalogue' })).toBeInTheDocument()
    answer()
    expect(await findCard()).toBeInTheDocument()
    expect(calls(f, '/v3/catalog/items')).toHaveLength(1)
  })

  it('plays a tried-on emote on the avatar, and STOP EMOTE stops it', async () => {
    serve({ catalogue: [[catalogueRow({ category: 'emote', name: 'Moonwalk', data: { emote: { category: 'dance' } } })]] })
    const { props } = renderShop({ equipped: [], emoteTryOn: true })
    await openTab('Emotes')
    await findCard('Moonwalk')
    // An emote plays over any look, so it doesn't wait for the equipped one.
    await userEvent.click(screen.getByRole('button', { name: 'TRY ON' }))
    expect(props.emote).toHaveBeenLastCalledWith(ARMOR_URN)
    expect(props.preview).not.toHaveBeenCalled()
    await userEvent.click(screen.getByRole('button', { name: 'STOP EMOTE' }))
    expect(props.emote).toHaveBeenLastCalledWith(null)
    expect(screen.queryByRole('button', { name: 'STOP EMOTE' })).not.toBeInTheDocument()
  })

  it('stops the emote when the Shop closes', async () => {
    serve({ catalogue: [[catalogueRow({ category: 'emote', name: 'Moonwalk', data: { emote: { category: 'dance' } } })]] })
    const { props, rerender } = renderShop({ emoteTryOn: true })
    await openTab('Emotes')
    await findCard('Moonwalk')
    await userEvent.click(screen.getByRole('button', { name: 'TRY ON' }))
    expect(props.emote).toHaveBeenLastCalledWith(ARMOR_URN)
    rerender({ shop: { open: false, toggle: vi.fn() } })
    expect(props.emote).toHaveBeenLastCalledWith(null)
  })

  it('tells the engine where its stage image is drawn, and takes it back on close', () => {
    serve({})
    const { props, rerender } = renderShop()
    // jsdom lays nothing out: the image's 1920×1080 at scale 1, its top 70px above the page (under the top bar).
    expect(props.stage).toHaveBeenLastCalledWith({ x: 0, y: -70, width: 1920, height: 1080 })
    rerender({ shop: { open: false, toggle: vi.fn() } })
    expect(props.stage).toHaveBeenLastCalledWith(null)
  })

  it('sends no stage without an engine to draw it', () => {
    serve({})
    const { props } = renderShop({ noEngine: true })
    expect(props.stage).not.toHaveBeenCalled()
  })

  it('opens from the menu, right after Places', () => {
    serve({})
    renderShop()
    const nav = screen.getAllByRole('navigation').find((n) => n.getAttribute('aria-label') !== 'Pages')!
    const labels = within(nav).getAllByRole('button').map((b) => b.textContent?.replace(/\s*\[.*\]$/, ''))
    expect(labels.slice(0, 3)).toEqual(['Places', 'Shop', 'Events'])
  })
})
