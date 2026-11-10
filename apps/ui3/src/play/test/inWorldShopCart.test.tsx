// The in-world Shop's cart on the page, in the web shell: ADD TO CART, its toast and the header's badge,
// the cart popover, CHECKOUT asking the shell to buy the cart (the shell, faked here, owns the dialog), and
// what a purchase takes out of the cart.
import { describe, it, expect, vi, afterEach, beforeEach } from 'vitest'
import { act, cleanup, render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'

const shell = vi.hoisted(() => ({
  handlers: new Map<string, (message: Record<string, unknown>) => void>(),
  request: vi.fn()
}))
vi.mock('@explorer-core/lib/shell', () => ({
  inShell: true,
  postToShell: vi.fn(),
  shellRequest: (method: string, params?: unknown) => shell.request(method, params),
  onShellMessage: (type: string, handler: (message: Record<string, unknown>) => void) => {
    shell.handlers.set(type, handler)
    return () => shell.handlers.delete(type)
  }
}))

import InWorldShop, { type InWorldShopProps } from '../features/shop/InWorldShop'
import { inWorldCart } from '../features/shop/cartStore'
import { primeRemoteFlags } from '../../shop/discovery/lib/remoteFlags'
import { dispatchCancelLayer } from '@explorer-core/lib/cancelLayers'
import { LEGACY_SHOP_BUSY_MESSAGE, SHOP_EVENT, shopBusy } from '../lib/shopProtocol'
import type { Wearable } from '@explorer-core/engine/protocol'
import { fakeProfileState } from '@hud/test/harness'
import { PopupHost, resetPopups, ToastHost } from '@hud/design'

const CONTRACT = '0x1111111111111111111111111111111111111111'
const CREATOR = '0x2222222222222222222222222222222222222222'
const BUYER = '0x3333333333333333333333333333333333333333'

const row = {
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
  data: { wearable: { category: 'upper_body', bodyShapes: [], isSmart: false } }
}

/** `row` as the listings feed lists it (what is on sale: the catalogue's default), read from `row` on every answer. */
const listing = (): Record<string, unknown> => ({
  tradeId: null,
  listingType: 'primary',
  source: 'native',
  acquisition: 'store',
  contractAddress: row.contractAddress,
  itemId: row.itemId,
  tokenId: null,
  name: row.name,
  thumbnail: row.thumbnail,
  rarity: row.rarity,
  category: row.category,
  wearableCategory: row.data.wearable.category,
  creator: row.creator,
  priceCredits: row.priceCredits,
  manaWei: null,
  available: row.available,
  network: row.network,
  chainId: row.chainId
})

/** The catalogue (the Golden Armor), and the home's outfits when given. */
function serveCatalogue(outfits: unknown[] = []): void {
  vi.stubGlobal(
    'fetch',
    vi.fn(async (input: RequestInfo | URL) => {
      const url = new URL(String(input instanceof Request ? input.url : input), location.href)
      if (url.pathname.endsWith('/v3/catalog/items')) return Response.json({ data: [row], total: 1 })
      if (url.pathname.endsWith('/v3/catalog/unified')) return Response.json({ data: [listing()], total: 1 })
      if (url.pathname.endsWith('/v1/outfits')) return Response.json({ outfits })
      return new Response('', { status: 404 })
    })
  )
}

function answerShell(capabilities: Record<string, unknown> = { v: 1, buyer: BUYER, wallet: 'managed', credits: true }): void {
  shell.request.mockImplementation(async (method: string) => {
    if (method === 'shop.capabilities') return capabilities
    if (method === 'shop.balance') return { v: 1, credits: 7000 }
    if (method === 'shop.buy') return { v: 1, operation: 'op', state: 'reviewing', items: [{ contract: CONTRACT, itemId: '1', quantity: 1 }] }
    if (method === 'shop.status') return []
    throw new Error(`unexpected ${method}`)
  })
}

const LOOK = [{ urn: 'urn:decentraland:off-chain:base-avatars:BaseMale', category: 'body_shape' }] as Wearable[]

function renderShop(): void {
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
    rotate: vi.fn(),
    zoom: vi.fn()
  }
  render(
    <QueryClientProvider client={new QueryClient({ defaultOptions: { queries: { retry: false } } })}>
      <InWorldShop {...props} />
      <PopupHost />
      <ToastHost />
    </QueryClientProvider>
  )
}

/** Opens the item's panel and presses ADD TO CART `times` times. */
async function addFromPanel(times = 1): Promise<void> {
  await userEvent.click(await screen.findByRole('button', { name: 'Golden Armor' }, { timeout: 5000 }))
  const add = await screen.findByRole('button', { name: 'Add to cart' })
  await waitFor(() => expect(add).toBeEnabled())
  for (let i = 0; i < times; i++) await userEvent.click(add)
}

const cartButton = (): HTMLElement => screen.getByRole('button', { name: /^Cart/ })

// jsdom has no element scrolling, which the home's rails use.
const scrollTo = HTMLElement.prototype.scrollTo
beforeEach(() => {
  HTMLElement.prototype.scrollTo = function (): void {}
})
afterEach(async () => {
  HTMLElement.prototype.scrollTo = scrollTo
  resetPopups()
  primeRemoteFlags(null)
  shell.request.mockReset()
  shell.handlers.clear()
  vi.restoreAllMocks()
  vi.unstubAllGlobals()
  for (const line of [...inWorldCart.getSnapshot()]) await inWorldCart.remove(line)
  localStorage.clear()
})

describe('the in-world Shop’s cart', () => {
  it('keeps the item on ADD TO CART, says so, and counts it on the cart button', async () => {
    serveCatalogue()
    answerShell()
    renderShop()
    // The live region is there before the toast is, so it is announced as it lands in it.
    const region = await screen.findByText((_, el) => el?.getAttribute('aria-live') === 'polite' && el.getAttribute('role') === 'status')
    await addFromPanel()
    expect(await within(region).findByText('1 Item Added to Cart')).toBeInTheDocument()
    await waitFor(() => expect(cartButton()).toHaveAccessibleName('Cart, 1 item'))
    await userEvent.click(screen.getByRole('button', { name: 'Add to cart' }))
    await waitFor(() => expect(cartButton()).toHaveAccessibleName('Cart, 2 items'))
  })

  it('lists the cart: how many of each, the total, and taking one out', async () => {
    serveCatalogue()
    answerShell()
    renderShop()
    await addFromPanel(2)
    await userEvent.click(cartButton())
    const cart = await screen.findByRole('region', { name: 'Your cart' })
    expect(within(cart).getByText('Your Cart (2)')).toBeInTheDocument()
    expect(within(cart).getByLabelText('Quantity: 2')).toBeInTheDocument()
    expect(within(cart).getByText('Total').nextElementSibling).toHaveTextContent('1K')
    await userEvent.click(within(cart).getByRole('button', { name: 'One less Golden Armor' }))
    expect(within(cart).getByLabelText('Quantity: 1')).toBeInTheDocument()
    // At one, − takes it out, as the trash does.
    expect(within(cart).queryByRole('button', { name: 'One less Golden Armor' })).toBeNull()
    expect(within(cart).getAllByRole('button', { name: 'Remove Golden Armor' })).toHaveLength(2)
    await userEvent.click(within(cart).getAllByRole('button', { name: 'Remove Golden Armor' })[0])
    expect(within(cart).getByText('Your cart is empty.')).toBeInTheDocument()
    expect(within(cart).getByRole('button', { name: 'Checkout' })).toBeDisabled()
  })

  it('keeps a guest’s cart for the account that signs in', async () => {
    serveCatalogue()
    // A guest login: the shell names its ephemeral account, which is no one's cart.
    answerShell({ v: 1, buyer: '0x' + '55'.repeat(20), wallet: null, credits: false, reason: 'guest' })
    renderShop()
    await addFromPanel(2)
    await waitFor(() => expect(cartButton()).toHaveAccessibleName('Cart, 2 items'))
    cleanup()
    // Signed in (the app loads again for the account).
    answerShell()
    renderShop()
    await waitFor(() => expect(cartButton()).toHaveAccessibleName('Cart, 2 items'))
    await waitFor(() => expect(localStorage.getItem(`dcl-inworld-shop:cart:v2:${BUYER}`) ?? '').toContain('Golden Armor'))
    expect(localStorage.getItem('dcl-inworld-shop:cart:v2:guest')).toBeNull()
    expect(localStorage.getItem(`dcl-inworld-shop:cart:v2:${'0x' + '55'.repeat(20)}`)).toBeNull()
  })

  it('moves the focus to the cart’s title when the focused button’s line goes', async () => {
    serveCatalogue()
    answerShell()
    renderShop()
    await addFromPanel()
    await userEvent.click(cartButton())
    const cart = await screen.findByRole('region', { name: 'Your cart' })
    const trash = within(cart).getAllByRole('button', { name: 'Remove Golden Armor' })[1]
    trash.focus()
    await userEvent.keyboard('{Enter}')
    await waitFor(() => expect(within(cart).getByText('Your cart is empty.')).toBeInTheDocument())
    expect(document.activeElement).toBe(within(cart).getByRole('heading', { name: 'Your Cart (0)' }))
  })

  it('closes on Escape (the HUD’s cancel, which the engine sends) and on a press anywhere else', async () => {
    serveCatalogue()
    answerShell()
    renderShop()
    await addFromPanel()
    await userEvent.click(cartButton())
    await screen.findByRole('region', { name: 'Your cart' })
    act(() => void expect(dispatchCancelLayer()).toBe(true))
    await waitFor(() => expect(screen.queryByRole('region', { name: 'Your cart' })).toBeNull())
    // The focus goes back to the cart button.
    expect(document.activeElement).toBe(cartButton())
    await userEvent.click(cartButton())
    await screen.findByRole('region', { name: 'Your cart' })
    await userEvent.click(screen.getByRole('heading', { name: 'Shop' }))
    await waitFor(() => expect(screen.queryByRole('region', { name: 'Your cart' })).toBeNull())
  })

  it('asks the shell to buy the cart’s items on CHECKOUT, marked as the cart’s, and closes', async () => {
    serveCatalogue()
    answerShell()
    renderShop()
    await addFromPanel(3)
    await userEvent.click(cartButton())
    await userEvent.click(within(await screen.findByRole('region', { name: 'Your cart' })).getByRole('button', { name: 'Checkout' }))
    await waitFor(() => expect(shell.request).toHaveBeenCalledWith('shop.buy', expect.anything()))
    const [, params] = shell.request.mock.calls.find(([m]) => m === 'shop.buy')!
    expect(params).toEqual({ v: 1, items: [{ contract: CONTRACT, itemId: '1', quantity: 3 }], method: 'credits', cart: true, input: 'pointer' })
    expect(screen.queryByRole('region', { name: 'Your cart' })).toBeNull()
  })

  /** The shell's usual answers, with `shop.buy` failing with `error`. */
  const failBuy = (error: Error): void => {
    answerShell()
    const answered = shell.request.getMockImplementation()!
    shell.request.mockImplementation(async (method: string, params?: unknown) => {
      if (method === 'shop.buy') throw error
      return answered(method, params)
    })
  }

  // One test, busy first: a toast outlives its test (the toasts' store is the module's, and lets go on a timer).
  // answerShell's capabilities don't say `codedErrors`: a shell from before codes, busy by a code or in its words alone.
  it('says nothing on CHECKOUT while the shell is busy with another purchase, and that it couldn’t open when the shell refuses', async () => {
    const COULD_NOT_OPEN = "The purchase couldn't open. Try again in a moment."
    const checkout = async (): Promise<void> => {
      shell.request.mockClear()
      await userEvent.click(cartButton())
      await userEvent.click(within(await screen.findByRole('region', { name: 'Your cart' })).getByRole('button', { name: 'Checkout' }))
      await waitFor(() => expect(shell.request).toHaveBeenCalledWith('shop.buy', expect.anything()))
    }
    serveCatalogue()
    failBuy(shopBusy())
    renderShop()
    await addFromPanel()
    await checkout()
    // Its dialog is what the buyer sees.
    await act(() => new Promise((resolve) => setTimeout(resolve, 50)))
    expect(screen.queryByText(COULD_NOT_OPEN)).toBeNull()
    failBuy(new Error(LEGACY_SHOP_BUSY_MESSAGE))
    await checkout()
    await act(() => new Promise((resolve) => setTimeout(resolve, 50)))
    expect(screen.queryByText(COULD_NOT_OPEN)).toBeNull()
    failBuy(new Error('refused'))
    await checkout()
    expect(await screen.findByText(COULD_NOT_OPEN)).toBeInTheDocument()
  })

  it('buys an outfit on its BUY in one purchase, not as the cart’s, and leaves the cart as it was', async () => {
    serveCatalogue([{ id: 'o1', name: 'Neon Night', thumbnailHash: '', items: [{ contractAddress: CONTRACT, itemId: '1' }], gradientFrom: '#ff0000', gradientTo: '#0000ff' }])
    answerShell()
    renderShop()
    await addFromPanel()
    await waitFor(() => expect(cartButton()).toHaveAccessibleName('Cart, 1 item'))
    await userEvent.click(screen.getByRole('tab', { name: 'All' }))
    const tile = (await screen.findByRole('button', { name: 'Try on Neon Night' }, { timeout: 5000 })).parentElement!
    await userEvent.click(await within(tile).findByRole('button', { name: 'Buy Neon Night' }))
    await waitFor(() => expect(shell.request).toHaveBeenCalledWith('shop.buy', expect.anything()))
    const [, params] = shell.request.mock.calls.find(([m]) => m === 'shop.buy')!
    expect(params).toEqual({ v: 1, items: [{ contract: CONTRACT, itemId: '1', quantity: 1 }], method: 'credits', input: 'pointer' })
    expect(cartButton()).toHaveAccessibleName('Cart, 1 item')
  })

  // Two items, of which the catalogue knows only the Golden Armor: the outfit can't be priced here.
  const HALF_KNOWN = { id: 'o2', name: 'Half Known', thumbnailHash: '', items: [{ contractAddress: CONTRACT, itemId: '1' }, { contractAddress: CONTRACT, itemId: '9' }], gradientFrom: '#ff0000', gradientTo: '#0000ff' }
  const outfitBuy = async (name: string): Promise<HTMLElement> => {
    const tile = (await screen.findByRole('button', { name: `Try on ${name}` }, { timeout: 5000 })).parentElement!
    return within(tile).findByRole('button', { name: `Buy ${name}` })
  }
  const continueToWeb = async (): Promise<void> => {
    const dialog = await screen.findByRole('dialog', {}, { timeout: 5000 })
    expect(within(dialog).getByText('You will be redirected to the web Shop')).toBeInTheDocument()
    await userEvent.click(within(dialog).getByRole('button', { name: 'CONTINUE' }))
  }

  it('sends an outfit it can’t price in credits to the web Shop on its BUY', async () => {
    const open = vi.spyOn(window, 'open').mockReturnValue(null)
    serveCatalogue([HALF_KNOWN])
    answerShell()
    renderShop()
    await userEvent.click(await screen.findByRole('tab', { name: 'All' }))
    await userEvent.click(await outfitBuy('Half Known'))
    await continueToWeb()
    expect(open).toHaveBeenCalledWith(expect.stringContaining('/shop/items/outfits/o2'), '_blank', 'noopener')
    expect(shell.request).not.toHaveBeenCalledWith('shop.buy', expect.anything())
  })

  it('sends an outfit to the web Shop when the shell can’t open its purchase', async () => {
    const open = vi.spyOn(window, 'open').mockReturnValue(null)
    serveCatalogue([{ id: 'o1', name: 'Neon Night', thumbnailHash: '', items: [{ contractAddress: CONTRACT, itemId: '1' }], gradientFrom: '#ff0000', gradientTo: '#0000ff' }])
    answerShell()
    const answered = shell.request.getMockImplementation()!
    shell.request.mockImplementation(async (method: string, params?: unknown) => {
      if (method === 'shop.buy') throw new Error('refused')
      return answered(method, params)
    })
    renderShop()
    await userEvent.click(await screen.findByRole('tab', { name: 'All' }))
    await userEvent.click(await outfitBuy('Neon Night'))
    await continueToWeb()
    expect(open).toHaveBeenCalledWith(expect.stringContaining('/shop/items/outfits/o1'), '_blank', 'noopener')
  })

  for (const [busy, error] of [
    ['by its code', shopBusy()],
    ['in the words alone of a shell from before codes', new Error(LEGACY_SHOP_BUSY_MESSAGE)]
  ] as const) {
    it(`sends an outfit nowhere while the shell is busy with another purchase (${busy}): its dialog is what the buyer sees`, async () => {
      serveCatalogue([{ id: 'o1', name: 'Neon Night', thumbnailHash: '', items: [{ contractAddress: CONTRACT, itemId: '1' }], gradientFrom: '#ff0000', gradientTo: '#0000ff' }])
      failBuy(error)
      renderShop()
      await userEvent.click(await screen.findByRole('tab', { name: 'All' }))
      await userEvent.click(await outfitBuy('Neon Night'))
      await waitFor(() => expect(shell.request).toHaveBeenCalledWith('shop.buy', expect.anything()))
      await act(() => new Promise((resolve) => setTimeout(resolve, 50)))
      expect(screen.queryByText('You will be redirected to the web Shop')).toBeNull()
    })
  }

  it('offers no price or BUY on an outfit with no items', async () => {
    serveCatalogue([{ id: 'o0', name: 'Empty Look', thumbnailHash: '', items: [], gradientFrom: '#ff0000', gradientTo: '#0000ff' }])
    answerShell()
    renderShop()
    await userEvent.click(await screen.findByRole('tab', { name: 'All' }))
    const tile = (await screen.findByRole('button', { name: 'Try on Empty Look' }, { timeout: 5000 })).parentElement!
    await new Promise((r) => setTimeout(r, 50))
    expect(within(tile).getByText('0 Items')).toBeInTheDocument()
    expect(within(tile).queryByText('0', { exact: true })).toBeNull()
    expect(within(tile).queryByRole('button', { name: /^buy/i })).toBeNull()
  })

  const OP = (n: number): string => `0f0e0d0c-0000-4000-8000-${String(n).padStart(12, '0')}`
  const emit = (event: Record<string, unknown>): void =>
    act(() => shell.handlers.get(SHOP_EVENT)?.({ type: SHOP_EVENT, v: 1, buyer: BUYER, items: [{ contract: CONTRACT, itemId: '1', quantity: 1 }], ...event }))

  it('lets go of what a cart purchase bought, once, and keeps an item bought on its own', async () => {
    serveCatalogue()
    answerShell()
    renderShop()
    await addFromPanel(3)
    await waitFor(() => expect(cartButton()).toHaveAccessibleName('Cart, 3 items'))
    emit({ operation: OP(1), state: 'succeeded', txHash: '0xabc' })
    expect(cartButton()).toHaveAccessibleName('Cart, 3 items')
    emit({ operation: OP(2), state: 'succeeded', txHash: '0xabc', cart: true })
    await waitFor(() => expect(cartButton()).toHaveAccessibleName('Cart, 2 items'))
    // Heard again (GO TO BACKPACK publishes it again): nothing more leaves.
    emit({ operation: OP(2), state: 'succeeded', txHash: '0xabc', cart: true, next: 'backpack' })
    expect(cartButton()).toHaveAccessibleName('Cart, 2 items')
  })

  it('leaves another account’s purchase to that account’s cart', async () => {
    serveCatalogue()
    answerShell()
    renderShop()
    await addFromPanel()
    await waitFor(() => expect(cartButton()).toHaveAccessibleName('Cart, 1 item'))
    emit({ operation: OP(3), buyer: '0x' + '44'.repeat(20), state: 'succeeded', txHash: '0xabc', cart: true })
    expect(cartButton()).toHaveAccessibleName('Cart, 1 item')
  })

  it('takes out what the cart bought while the app wasn’t listening, from the shell’s status', async () => {
    localStorage.setItem(
      `dcl-inworld-shop:cart:v2:${BUYER}`,
      JSON.stringify({ v: 2, lines: [{ contract: CONTRACT, itemId: '1', quantity: 2, name: 'Golden Armor', rarity: 'epic', category: 'upper_body', creator: CREATOR, credits: 500, available: 35, addedAt: 1 }], applied: [] })
    )
    serveCatalogue()
    answerShell()
    const usual = shell.request.getMockImplementation()!
    shell.request.mockImplementation(async (method: string, params?: unknown) =>
      method === 'shop.status' ? [{ v: 1, operation: OP(4), buyer: BUYER, state: 'succeeded', items: [{ contract: CONTRACT, itemId: '1', quantity: 1 }], cart: true }] : usual(method, params)
    )
    renderShop()
    await waitFor(() => expect(cartButton()).toHaveAccessibleName('Cart, 1 item'))
  })
})
