// The in-world Shop page in the web shell: the Credits pill, BUY and BUY ITEM asking the shell to buy (the
// shell, faked here, owns the dialog and the wallet), and what the page does as the purchase moves on.
import { describe, it, expect, vi, afterEach } from 'vitest'
import { act, render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'

const shell = vi.hoisted(() => ({
  handlers: new Map<string, (message: Record<string, unknown>) => void>(),
  request: vi.fn()
}))
vi.mock('../lib/shell', () => ({
  inShell: true,
  postToShell: vi.fn(),
  shellRequest: (method: string, params?: unknown) => shell.request(method, params),
  onShellMessage: (type: string, handler: (message: Record<string, unknown>) => void) => {
    shell.handlers.set(type, handler)
    return () => shell.handlers.delete(type)
  }
}))

import InWorldShop, { type InWorldShopProps } from '../features/shop/InWorldShop'
import { primeRemoteFlags } from '../../shop/discovery/lib/remoteFlags'
import { SHOP_EVENT } from '../lib/shopProtocol'
import type { Wearable } from '../engine/protocol'
import { fakeProfileState } from './harness'

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

function serveCatalogue(): void {
  vi.stubGlobal(
    'fetch',
    vi.fn(async (input: RequestInfo | URL) => {
      const url = new URL(String(input instanceof Request ? input.url : input), location.href)
      if (url.pathname.endsWith('/v3/catalog/items')) return Response.json({ data: [row], total: 1 })
      return new Response('', { status: 404 })
    })
  )
}

let balance = 700
function answerShell(): void {
  shell.request.mockImplementation(async (method: string) => {
    if (method === 'shop.capabilities') return { v: 1, buyer: BUYER, wallet: 'managed', credits: true }
    if (method === 'shop.balance') return { v: 1, credits: balance }
    if (method === 'shop.buy') return { v: 1, operation: 'op', state: 'reviewing', item: { contract: CONTRACT, itemId: '1' } }
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
    emote: vi.fn()
  }
  render(
    <QueryClientProvider client={new QueryClient({ defaultOptions: { queries: { retry: false } } })}>
      <InWorldShop {...props} />
    </QueryClientProvider>
  )
  return props
}

const emit = (event: Record<string, unknown>): void =>
  act(() => shell.handlers.get(SHOP_EVENT)?.({ type: SHOP_EVENT, v: 1, operation: 'op', item: { contract: CONTRACT, itemId: '1' }, ...event }))

afterEach(() => {
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

  it('asks the shell to buy the card’s item with Credits: the item and the creator’s name, nothing else', async () => {
    serveCatalogue()
    answerShell()
    renderShop()
    const card = (await screen.findByText('Golden Armor', {}, { timeout: 5000 })).closest('[data-rarity]') as HTMLElement
    await userEvent.click(within(card).getByRole('button', { name: 'BUY' }))
    await waitFor(() => expect(shell.request).toHaveBeenCalledWith('shop.buy', expect.anything()))
    const [, params] = shell.request.mock.calls.find(([m]) => m === 'shop.buy')!
    expect(params).toEqual({ v: 1, item: { contract: CONTRACT, itemId: '1' }, method: 'credits', creatorName: expect.any(String) })
  })

  it('buys from the item panel’s BUY ITEM', async () => {
    serveCatalogue()
    answerShell()
    renderShop()
    await userEvent.click(await screen.findByRole('button', { name: 'Golden Armor' }, { timeout: 5000 }))
    await userEvent.click(await screen.findByRole('button', { name: 'Buy item' }))
    await waitFor(() => expect(shell.request.mock.calls.filter(([m]) => m === 'shop.buy')).toHaveLength(1))
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

  it('ignores a malformed event', async () => {
    serveCatalogue()
    answerShell()
    const props = renderShop()
    await screen.findByRole('status', { name: 'Credits balance: 700' })
    emit({ state: 'exploded', next: 'backpack' })
    expect(props.onNavigate).not.toHaveBeenCalled()
  })
})
