import { describe, it, expect, vi, afterEach, beforeEach } from 'vitest'
import { fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import InWorldShop, { type InWorldShopProps } from '../features/shop/InWorldShop'
import { nextWearablesPage, serverSort, SHOP_PAGE_SIZE, shopQueries, type WearableFilters } from '../features/shop/wearablesQuery'
import { primeRemoteFlags } from '../../shop/discovery/lib/remoteFlags'
import type { Wearable } from '../engine/protocol'
import { fakeProfileState } from './harness'

const CONTRACT = '0x1111111111111111111111111111111111111111'
const CREATOR = '0x2222222222222222222222222222222222222222'

// A row of the web Shop's unified feed (/v3/catalog/unified).
const listing = (over: Record<string, unknown> = {}): Record<string, unknown> => ({
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
  available: 10,
  network: 'MATIC',
  chainId: 137,
  ...over
})

type Reply = Record<string, unknown>[] | 'down'

/** Serves the feed's answers in order; every other request (profiles, flags) is a 404. */
function serveFeed(...replies: Reply[]): ReturnType<typeof vi.fn> {
  const queue = [...replies]
  const f = vi.fn(async (input: RequestInfo | URL) => {
    const url = String(input instanceof Request ? input.url : input)
    if (!url.includes('/v3/catalog/unified')) return new Response('', { status: 404 })
    const next = queue.length > 1 ? queue.shift()! : queue[0]!
    if (next === 'down') return new Response('', { status: 503 })
    return new Response(JSON.stringify({ data: next, total: next.length }), { headers: { 'Content-Type': 'application/json' } })
  })
  vi.stubGlobal('fetch', f)
  return f
}

const feedCalls = (f: ReturnType<typeof vi.fn>): URL[] =>
  f.mock.calls.map(([input]) => new URL(String(input instanceof Request ? input.url : input), location.href)).filter((u) => u.pathname.endsWith('/v3/catalog/unified'))

// The look the engine reports: the body shape first, then the wearables.
const LOOK = [
  { urn: 'urn:body', category: 'body_shape' },
  { urn: 'urn:old-shirt', category: 'upper_body' },
  { urn: 'urn:jeans', category: 'lower_body' }
] as Wearable[]
const ARMOR_URN = `urn:decentraland:matic:collections-v2:${CONTRACT}:1`

/** The first card on screen: the page loads its chunk, its feed and its profiles first, so it may take a while under load. */
const findCard = (name = 'Golden Armor'): Promise<HTMLElement> => screen.findByText(name, {}, { timeout: 5000 })

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
    ...over
  }
  const view = render(
    <QueryClientProvider client={client}>
      <InWorldShop {...props} />
    </QueryClientProvider>
  )
  const rerender = (next: Partial<InWorldShopProps>): void =>
    view.rerender(
      <QueryClientProvider client={client}>
        <InWorldShop {...props} {...next} />
      </QueryClientProvider>
    )
  return { props, rerender, unmount: view.unmount }
}

// The rollout flags as the service answers them; a test that needs one on primes it.
beforeEach(() => primeRemoteFlags({}))
afterEach(() => {
  primeRemoteFlags(null)
  vi.unstubAllGlobals()
})

describe('in-world shop query factory', () => {
  const base: WearableFilters = { category: 'all', sort: 'newest', search: '' }

  it('keys every input that changes the feed, the creator-sales flag included', () => {
    const key = (f: WearableFilters, creatorSales = false): string => JSON.stringify(shopQueries.wearables(f, { creatorSales }).queryKey)
    const keys = [key(base), key({ ...base, category: 'Feet' }), key({ ...base, sort: 'cheapest' }), key({ ...base, search: 'armor' }), key(base, true)]
    expect(new Set(keys).size).toBe(5)
  })

  it('pages by the rows loaded, until a short page or the total', () => {
    const page = (n: number, total: number) => ({ items: Array.from({ length: n }, () => ({})), total, meta: { freshness: null } })
    const first = page(SHOP_PAGE_SIZE, 120)
    expect(nextWearablesPage(first, [first])).toBe(SHOP_PAGE_SIZE)
    const second = page(SHOP_PAGE_SIZE, 120)
    expect(nextWearablesPage(second, [first, second])).toBe(2 * SHOP_PAGE_SIZE)
    const last = page(24, 120)
    expect(nextWearablesPage(last, [first, second, last])).toBeUndefined()
    const exact = page(SHOP_PAGE_SIZE, SHOP_PAGE_SIZE)
    expect(nextWearablesPage(exact, [exact])).toBeUndefined()
  })

  it('orders a search by relevance unless another sort was picked', () => {
    expect(serverSort({ ...base, search: 'armor' })).toBe('relevance')
    expect(serverSort({ ...base, search: 'armor', sort: 'cheapest' })).toBe('cheapest')
    expect(serverSort(base)).toBe('newest')
  })
})

describe('in-world shop', () => {
  it('opens from the menu, right after Places', () => {
    serveFeed([listing()])
    renderShop()
    const labels = within(screen.getByRole('navigation')).getAllByRole('button').map((b) => b.textContent?.replace(/\s*\[.*\]$/, ''))
    expect(labels.slice(0, 3)).toEqual(['Places', 'Shop', 'Events'])
  })

  it('lists the wearables on sale with their credits price and creator', async () => {
    const f = serveFeed([listing()])
    renderShop()
    const card = (await findCard()).closest('[data-rarity]') as HTMLElement
    expect(within(card).getByText('500')).toBeInTheDocument()
    expect(within(card).getByText('epic')).toBeInTheDocument()
    expect(within(card).getByText('0x2222…2222')).toBeInTheDocument()
    const q = feedCalls(f)[0]!.searchParams
    expect(q.get('category')).toBe('wearable')
    expect(q.get('onSale')).toBe('true')
    expect(q.get('sortBy')).toBe('newest')
    expect(q.get('first')).toBe('48')
    expect(q.get('skip')).toBe('0')
  })

  it('prices a resale in MANA', async () => {
    serveFeed([listing({ listingType: 'secondary', tokenId: '7', tradeId: 'trade-1', priceCredits: 0, manaWei: '12500000000000000000' })])
    renderShop()
    const card = (await findCard()).closest('[data-rarity]') as HTMLElement
    expect(within(card).getByText('12.5')).toBeInTheDocument()
  })

  it('tags a discounted credits price while creator sales are on', async () => {
    primeRemoteFlags({ 'dapps-shop-creator-sales': true })
    serveFeed([listing({ priceCredits: 400, compareAtCredits: 500 })])
    renderShop()
    const card = (await findCard()).closest('[data-rarity]') as HTMLElement
    expect(within(card).getByText('-20%')).toBeInTheDocument()
    expect(within(card).getByText('400')).toBeInTheDocument()
  })

  it('shows the full price, untagged, while creator sales are off', async () => {
    serveFeed([listing({ priceCredits: 400, compareAtCredits: 500 })])
    renderShop()
    const card = (await findCard()).closest('[data-rarity]') as HTMLElement
    expect(within(card).getByText('500')).toBeInTheDocument()
    expect(within(card).queryByText('-20%')).toBeNull()
  })

  it('narrows the feed to a category', async () => {
    const f = serveFeed([listing()])
    renderShop()
    await findCard()
    await userEvent.click(screen.getByRole('button', { name: 'Upper body' }))
    await waitFor(() => expect(feedCalls(f).at(-1)!.searchParams.get('wearableCategory')).toBe('upper_body'))
    expect(screen.getByRole('button', { name: 'Upper body' })).toHaveAttribute('aria-pressed', 'true')
  })

  it('sorts by the option picked', async () => {
    const f = serveFeed([listing()])
    renderShop()
    await findCard()
    await userEvent.click(screen.getByRole('button', { name: 'Sort by: Newest' }))
    await userEvent.click(screen.getByText('Cheapest'))
    await waitFor(() => expect(feedCalls(f).at(-1)!.searchParams.get('sortBy')).toBe('cheapest'))
  })

  it('searches, counts the results and clears the search', async () => {
    const f = serveFeed([listing()])
    renderShop()
    await findCard()
    fireEvent.change(screen.getByPlaceholderText('Search'), { target: { value: '  Armor ' } })
    await waitFor(() => expect(feedCalls(f).at(-1)!.searchParams.get('search')).toBe('armor'))
    expect(feedCalls(f).at(-1)!.searchParams.get('sortBy')).toBe('relevance')
    expect(await screen.findByText('Results for ‘armor’ (1)')).toBeInTheDocument()
    await userEvent.click(screen.getByRole('button', { name: 'Exit search' }))
    await waitFor(() => expect(screen.queryByText(/Results for/)).toBeNull())
  })

  it('offers to explore every wearable when a search matches nothing', async () => {
    serveFeed([listing()], [])
    renderShop()
    await findCard()
    await userEvent.click(screen.getByRole('button', { name: 'Feet' }))
    expect(await screen.findByText('No matches found', {}, { timeout: 5000 })).toBeInTheDocument()
    await userEvent.click(screen.getByRole('button', { name: 'EXPLORE WEARABLES' }))
    expect(await findCard()).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'All' })).toHaveAttribute('aria-pressed', 'true')
  })

  it('says nothing is on sale when the whole feed is empty', async () => {
    serveFeed([])
    renderShop()
    expect(await screen.findByText('Nothing on sale right now', {}, { timeout: 5000 })).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: 'EXPLORE WEARABLES' })).toBeNull()
  })

  it('shows the error and retries', async () => {
    serveFeed('down', [listing()])
    renderShop()
    expect(await screen.findByText("Couldn't load the Shop", {}, { timeout: 5000 })).toBeInTheDocument()
    await userEvent.click(screen.getByRole('button', { name: 'Retry' }))
    expect(await findCard()).toBeInTheDocument()
  })

  it('frames the whole avatar when it opens', async () => {
    serveFeed([listing()])
    const { props } = renderShop()
    await findCard()
    expect(props.focus).toHaveBeenCalledWith('body')
  })

  it('tries an item on over the current look, swapping its category, and restores the look on close', async () => {
    serveFeed([listing()])
    const { props, rerender } = renderShop()
    await findCard()
    await userEvent.click(screen.getByRole('button', { name: 'TRY ON' }))
    expect(props.preview).toHaveBeenLastCalledWith(['urn:body', 'urn:jeans', ARMOR_URN])
    expect(screen.getByRole('button', { name: 'Golden Armor' })).toHaveAttribute('aria-pressed', 'true')
    rerender({ shop: { open: false, toggle: vi.fn() } })
    expect(props.preview).toHaveBeenLastCalledWith(null)
  })

  it('takes a tried-on item off again', async () => {
    serveFeed([listing()])
    const { props } = renderShop()
    await findCard()
    await userEvent.click(screen.getByRole('button', { name: 'TRY ON' }))
    await userEvent.click(screen.getByRole('button', { name: 'TAKE OFF' }))
    expect(props.preview).toHaveBeenLastCalledWith(null)
    expect(screen.getByRole('button', { name: 'Golden Armor' })).toHaveAttribute('aria-pressed', 'false')
  })

  it('waits for the look before trying anything on', async () => {
    serveFeed([listing()])
    const { props } = renderShop({ equipped: [] })
    await findCard()
    const tryOn = screen.getByRole('button', { name: 'TRY ON' })
    expect(tryOn).toBeDisabled()
    expect(screen.queryByRole('button', { name: 'Golden Armor' })).toBeNull()
    expect(props.preview).not.toHaveBeenCalled()
  })

  it('offers no try-on for an item without a Polygon collection URN', async () => {
    serveFeed([listing({ chainId: 1, network: 'ETHEREUM' })])
    renderShop()
    expect(await findCard()).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: 'TRY ON' })).toBeNull()
  })

  it('restores the look when it unmounts while trying something on', async () => {
    serveFeed([listing()])
    const { props, unmount } = renderShop()
    await findCard()
    await userEvent.click(screen.getByRole('button', { name: 'TRY ON' }))
    unmount()
    expect(props.preview).toHaveBeenLastCalledWith(null)
  })

  it('leaves the preview avatar alone when nothing was tried on', async () => {
    serveFeed([listing()])
    const { props, rerender } = renderShop()
    await findCard()
    rerender({ shop: { open: false, toggle: vi.fn() } })
    expect(props.preview).not.toHaveBeenCalled()
  })
})
