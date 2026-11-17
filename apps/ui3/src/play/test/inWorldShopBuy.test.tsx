// The in-world Shop page in the web shell: the Credits pill, BUY and BUY ITEM asking the shell to buy (the
// shell, faked here, owns the dialog and the wallet), and what the page does as the purchase moves on.
import { readFileSync } from 'node:fs'
import { dirname, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
import { describe, it, expect, vi, afterEach } from 'vitest'
import { act, cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react'
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
import shopStyles from '../features/shop/InWorldShop.module.css'
import { primeRemoteFlags } from '../../shop/discovery/lib/remoteFlags'
import { LEGACY_SHOP_BUSY_MESSAGE, SHOP_EVENT, shopBusy } from '../lib/shopProtocol'
import type { Wearable } from '@explorer-core/engine/protocol'
import { fakeProfileState } from '@hud/test/harness'
import { PopupHost, resetPopups } from '@hud/design'

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

function serveCatalogue(flags: Record<string, boolean> = {}): ReturnType<typeof vi.fn> {
  const f = vi.fn(async (input: RequestInfo | URL) => {
      const url = new URL(String(input instanceof Request ? input.url : input), location.href)
      if (url.pathname.endsWith('/dapps.json')) return Response.json({ flags, variants: {} })
      if (url.pathname.endsWith('/v3/catalog/items')) return Response.json({ data: [row], total: 1 })
      if (url.pathname.endsWith('/v3/catalog/unified')) return Response.json({ data: [listing()], total: 1 })
      return new Response('', { status: 404 })
    })
  vi.stubGlobal('fetch', f)
  return f
}

let balance = 700
/** The shell's answers; `codedErrors` false is a shell from before codes, whose refusals carry words alone. */
function answerShell(wallet: 'managed' | 'web3' = 'managed', marketplace: boolean | 'reject' = wallet === 'web3', codedErrors = true): void {
  shell.request.mockImplementation(async (method: string) => {
    if (method === 'shop.capabilities') {
      if (marketplace === 'reject') throw new Error('the shell did not answer')
      return { v: 1, buyer: BUYER, wallet, credits: true, marketplace, ...(codedErrors ? { codedErrors } : {}) }
    }
    if (method === 'shop.balance') return { v: 1, credits: balance }
    if (method === 'shop.buy') return { v: 1, operation: 'op', state: 'reviewing', items: [{ contract: CONTRACT, itemId: '1', quantity: 1 }] }
    if (method === 'shop.status') return []
    throw new Error(`unexpected ${method}`)
  })
}

const LOOK = [{ urn: 'urn:decentraland:off-chain:base-avatars:BaseMale', category: 'body_shape' }] as Wearable[]

function renderShop(): InWorldShopProps {
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
    </QueryClientProvider>
  )
  return props
}

/** The page's own stylesheet in the document, under the class names the module gives it. The path is this file's
 *  (Vite rewrites a `new URL(…, import.meta.url)` in tests), so it holds wherever vitest starts. */
function withShopStyles(): HTMLStyleElement {
  const sheet = document.createElement('style')
  const names = shopStyles as Record<string, string>
  sheet.textContent = readFileSync(resolve(dirname(fileURLToPath(import.meta.url)), '../features/shop/InWorldShop.module.css'), 'utf8').replace(/\.([a-zA-Z]\w*)/g, (selector, name: string) => (names[name] ? `.${names[name]}` : selector))
  document.head.append(sheet)
  return sheet
}

const emit = (event: Record<string, unknown>): void =>
  act(() => shell.handlers.get(SHOP_EVENT)?.({ type: SHOP_EVENT, v: 1, operation: 'op', buyer: BUYER, items: [{ contract: CONTRACT, itemId: '1', quantity: 1 }], ...event }))

afterEach(() => {
  resetPopups()
  primeRemoteFlags(null)
  shell.request.mockReset()
  shell.handlers.clear()
  balance = 700
  vi.unstubAllGlobals()
})

describe('buying in the in-world Shop', () => {
  it('shows the Credits balance the shell reads, next to the cart', async () => {
    serveCatalogue()
    answerShell()
    renderShop()
    expect(await screen.findByRole('status', { name: 'Credits balance: 700' })).toBeTruthy()
  })

  it('asks the shell to buy the card’s item with Credits: one of the item, nothing else', async () => {
    serveCatalogue()
    answerShell()
    renderShop()
    const card = (await screen.findByText('Golden Armor', {}, { timeout: 5000 })).closest('[data-rarity]') as HTMLElement
    await userEvent.click(within(card).getByRole('button', { name: 'BUY' }))
    await waitFor(() => expect(shell.request).toHaveBeenCalledWith('shop.buy', expect.anything()))
    const [, params] = shell.request.mock.calls.find(([m]) => m === 'shop.buy')!
    expect(params).toEqual({ v: 1, items: [{ contract: CONTRACT, itemId: '1', quantity: 1 }], method: 'credits', input: 'pointer' })
  })

  it('tells the shell BUY was pressed from the keyboard, so its dialog rings the focus', async () => {
    serveCatalogue()
    answerShell()
    renderShop()
    const card = (await screen.findByText('Golden Armor', {}, { timeout: 5000 })).closest('[data-rarity]') as HTMLElement
    // Enter or Space on a focused button clicks it with no click count.
    fireEvent.click(within(card).getByRole('button', { name: 'BUY' }), { detail: 0 })
    await waitFor(() => expect(shell.request).toHaveBeenCalledWith('shop.buy', expect.objectContaining({ input: 'keyboard' })))
  })

  it('buys from the item panel’s BUY ITEM', async () => {
    serveCatalogue()
    answerShell()
    renderShop()
    await userEvent.click(await screen.findByRole('button', { name: 'Golden Armor' }, { timeout: 5000 }))
    await userEvent.click(await screen.findByRole('button', { name: 'Buy item' }))
    await waitFor(() => expect(shell.request.mock.calls.filter(([m]) => m === 'shop.buy')).toHaveLength(1))
  })

  it('shows the item panel’s stock as the listings read it again after a purchase', async () => {
    serveCatalogue()
    answerShell()
    renderShop()
    await userEvent.click(await screen.findByRole('button', { name: 'Golden Armor' }, { timeout: 5000 }))
    const panel = await screen.findByRole('complementary', { name: 'Golden Armor' })
    await waitFor(() => expect(panel.textContent).toMatch(/35\/\d+/))
    row.available = 34
    try {
      emit({ state: 'succeeded', txHash: '0xabc' })
      await waitFor(() => expect(screen.getByRole('complementary', { name: 'Golden Armor' }).textContent).toMatch(/34\/\d+/))
    } finally {
      row.available = 35
    }
  })

  it('reads the balance again once the purchase succeeds, and goes to the Backpack when asked', async () => {
    serveCatalogue()
    answerShell()
    const props = renderShop()
    await screen.findByRole('status', { name: 'Credits balance: 700' })
    balance = 200
    emit({ state: 'succeeded', txHash: '0xabc' })
    expect(await screen.findByRole('status', { name: 'Credits balance: 200' })).toBeTruthy()
    expect(props.onNavigate).not.toHaveBeenCalledWith('backpack')
    emit({ state: 'succeeded', txHash: '0xabc', next: 'backpack' })
    expect(props.onNavigate).toHaveBeenCalledWith('backpack')
  })

  it('asks the shell for its operations when it mounts, so a reloaded app follows them', async () => {
    serveCatalogue()
    answerShell()
    renderShop()
    await waitFor(() => expect(shell.request).toHaveBeenCalledWith('shop.status', {}))
  })

  it('offers the web Shop when the shell can’t open the purchase: a refusal, or an answer in another version', async () => {
    for (const answer of [Promise.reject(new Error('this item is not sold here')), Promise.resolve({ v: 2, operation: 'op', state: 'reviewing' })]) {
      answer.catch(() => {})
      serveCatalogue()
      answerShell()
      const usual = shell.request.getMockImplementation()!
      shell.request.mockImplementation(async (method: string, params?: unknown) => (method === 'shop.buy' ? answer : usual(method, params)))
      renderShop()
      const card = (await screen.findByText('Golden Armor', {}, { timeout: 5000 })).closest('[data-rarity]') as HTMLElement
      await userEvent.click(within(card).getByRole('button', { name: 'BUY' }))
      const dialog = await screen.findByRole('dialog', {}, { timeout: 5000 })
      expect(within(dialog).getByText('You will be redirected to the web Shop')).toBeInTheDocument()
      cleanup()
      resetPopups()
    }
  })

  it('names the web Shop’s page in the confirmation, broken anywhere so a url stays inside the dialog', async () => {
    const sheet = withShopStyles()
    try {
      const refusal = Promise.reject(new Error('this item is not sold here'))
      refusal.catch(() => {})
      serveCatalogue()
      answerShell()
      const usual = shell.request.getMockImplementation()!
      shell.request.mockImplementation(async (method: string, params?: unknown) => (method === 'shop.buy' ? refusal : usual(method, params)))
      renderShop()
      const card = (await screen.findByText('Golden Armor', {}, { timeout: 5000 })).closest('[data-rarity]') as HTMLElement
      await userEvent.click(within(card).getByRole('button', { name: 'BUY' }))
      const dialog = await screen.findByRole('dialog', {}, { timeout: 5000 })
      const page = dialog.querySelector<HTMLElement>(`.${shopStyles.link}`)
      expect(page?.textContent).toContain(`/shop/item/${CONTRACT}/1`)
      expect(getComputedStyle(page!).overflowWrap).toBe('anywhere')
    } finally {
      sheet.remove()
    }
  })

  /** BUY on the card, with the shell's capabilities saying `codedErrors` or not (or never answering), and its BUY failing with `error`. */
  const buyFailing = async (error: Error, codedErrors: boolean, capabilities: 'answered' | 'never' = 'answered'): Promise<void> => {
    serveCatalogue()
    answerShell('managed', capabilities === 'never' ? 'reject' : false, codedErrors)
    const usual = shell.request.getMockImplementation()!
    shell.request.mockImplementation(async (method: string, params?: unknown) => {
      if (method === 'shop.buy') throw error
      return usual(method, params)
    })
    renderShop()
    await waitFor(() => expect(shell.request).toHaveBeenCalledWith('shop.capabilities', undefined))
    const card = (await screen.findByText('Golden Armor', {}, { timeout: 5000 })).closest('[data-rarity]') as HTMLElement
    await userEvent.click(within(card).getByRole('button', { name: 'BUY' }))
    await waitFor(() => expect(shell.request).toHaveBeenCalledWith('shop.buy', expect.anything()))
  }

  // Both shells this HUD may be hosted by: this one, busy by its code, and one from before codes, busy in the
  // same words alone, whose page this HUD may outlive. A shell whose capabilities never answered may be either,
  // so both forms of busy hold there too.
  for (const [host, error, codedErrors, capabilities] of [
    ['the shell', shopBusy(), true, 'answered'],
    ['a shell from before codes', new Error(LEGACY_SHOP_BUSY_MESSAGE), false, 'answered'],
    ['a shell whose capabilities never answered, by its code,', shopBusy(), true, 'never'],
    ['a shell whose capabilities never answered, in the words alone,', new Error(LEGACY_SHOP_BUSY_MESSAGE), true, 'never']
  ] as const) {
    it(`sends nowhere when ${host} is busy with another purchase: that purchase’s dialog, open or opening, is what the buyer sees`, async () => {
      await buyFailing(error, codedErrors, capabilities)
      await act(() => new Promise((resolve) => setTimeout(resolve, 50)))
      expect(screen.queryByText('You will be redirected to the web Shop')).toBeNull()
    })
  }

  it('takes the busy words without their code as a refusal from a shell whose refusals carry one: the web Shop', async () => {
    await buyFailing(new Error(LEGACY_SHOP_BUSY_MESSAGE), true)
    expect(await screen.findByRole('dialog', {}, { timeout: 5000 })).toHaveTextContent('You will be redirected to the web Shop')
  })

  it('ignores a malformed event', async () => {
    serveCatalogue()
    answerShell()
    const props = renderShop()
    await screen.findByRole('status', { name: 'Credits balance: 700' })
    emit({ state: 'exploded', next: 'backpack' })
    expect(props.onNavigate).not.toHaveBeenCalled()
  })
})

describe('the Marketplace in the web shell', () => {
  const unified = (f: ReturnType<typeof vi.fn>): URL[] =>
    f.mock.calls.map(([input]) => new URL(String(input instanceof Request ? input.url : input), location.href)).filter((u) => u.pathname.endsWith('/v3/catalog/unified'))

  it('follows the shell\u2019s Marketplace answer: primary sales only unless it vouches for a web3 wallet', async () => {
    const managed = serveCatalogue({ 'dapps-shop-marketplace-features': true })
    answerShell('managed')
    renderShop()
    await waitFor(() => expect(shell.request).toHaveBeenCalledWith('shop.capabilities', undefined))
    await waitFor(() => expect(unified(managed).length).toBeGreaterThan(0))
    await act(async () => {})
    expect(unified(managed).every((u) => u.searchParams.get('listingType') === 'primary')).toBe(true)
    cleanup()

    // A web3 wallet the page doesn't vouch for (a guessed or unsignable sign-in): still primary only.
    const guessed = serveCatalogue({ 'dapps-shop-marketplace-features': true })
    answerShell('web3', false)
    renderShop()
    await waitFor(() => expect(unified(guessed).length).toBeGreaterThan(0))
    await act(async () => {})
    expect(unified(guessed).every((u) => u.searchParams.get('listingType') === 'primary')).toBe(true)
    cleanup()

    const web3 = serveCatalogue({ 'dapps-shop-marketplace-features': true })
    answerShell('web3')
    renderShop()
    await waitFor(() => expect(unified(web3).some((u) => !u.searchParams.has('listingType'))).toBe(true))
  })

  it('loads the primary listings when the shell never answers what the session can pay with', async () => {
    const f = serveCatalogue({ 'dapps-shop-marketplace-features': true })
    answerShell('web3', 'reject')
    renderShop()
    await waitFor(() => expect(unified(f).length).toBeGreaterThan(0))
    expect(unified(f).every((u) => u.searchParams.get('listingType') === 'primary')).toBe(true)
  })
})
