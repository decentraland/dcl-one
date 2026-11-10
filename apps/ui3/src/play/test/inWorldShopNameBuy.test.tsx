// The in-world Shop's CLAIM NAME in the web shell: where the shell rehearses NAME purchases
// (`namesMode: 'simulation'`) it opens that rehearsal there (the shell, faked here, owns the dialog), and
// what the page does as the rehearsal ends: PREVIEW ON AVATAR shows the NAME over the avatar, once, saving
// nothing. Anywhere else, CLAIM NAME registers the NAME on the web Shop. Each operation's outcome is acted
// on once, and a NAME's never reaches the items' handling.
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
import { webNameUrl } from '../features/shop/itemLinks'
import { primeRemoteFlags } from '../../shop/discovery/lib/remoteFlags'
import { LEGACY_SHOP_BUSY_MESSAGE, SHOP_EVENT, shopBusy } from '../lib/shopProtocol'
import type { Profile } from '@hud/engine/protocol'
import { PopupHost, resetPopups, ToastHost } from '@hud/design'
import { fakeProfileState } from './harness'

const BUYER = '0x00000000000000000000000000000000000ab47f'
const CONTRACT = '0x1111111111111111111111111111111111111111'
const ME: Profile = { address: BUYER, name: 'Mojito', hasClaimedName: true, isGuest: false }
const TRUE_WORD = `0x${'0'.repeat(63)}1`

/** The NAME check's reads: no banned NAMEs, 94 Credits for a NAME, every NAME free at the registrar. */
function serve(): void {
  vi.stubGlobal(
    'fetch',
    vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
      const path = new URL(String(input instanceof Request ? input.url : input), location.href).pathname
      if (path.endsWith('/dapps.json')) return Response.json({ flags: {}, variants: {} })
      if (path.endsWith('/dcl-lists/banned-names')) return Response.json({ data: [] })
      if (path.endsWith('/shop-api/v1/credit-quotes')) return Response.json({ items: [], amounts: ['94'] })
      if (path.includes('/rpc/')) {
        const body = JSON.parse(String(init?.body)) as { id: number }
        return Response.json({ jsonrpc: '2.0', id: body.id, result: TRUE_WORD })
      }
      return new Response('', { status: 404 })
    })
  )
}

let balanceReads = 0
/**
 * The shell's answers: its capabilities (`namesMode` as given; refusals with a code unless `codedErrors` is false,
 * a shell from before codes), and BUY NAME refused when said, or busy (`busy`, SHOP_BUSY's coded answer unless
 * said) once `busyAfter` of them have opened (their dialog is up).
 */
function answerShell(namesMode?: string, opts: { refuse?: boolean; busyAfter?: number; busy?: () => Error; codedErrors?: boolean; status?: unknown[] } = {}): void {
  let opened = 0
  shell.request.mockImplementation(async (method: string, params?: Record<string, unknown>) => {
    if (method === 'shop.capabilities') {
      return { v: 1, buyer: BUYER, wallet: 'web3', credits: true, marketplace: false, ...(namesMode ? { namesMode } : {}), ...(opts.codedErrors === false ? {} : { codedErrors: true }) }
    }
    if (method === 'shop.balance') {
      balanceReads++
      return { v: 1, credits: 700 }
    }
    if (method === 'shop.buyName') {
      if (opts.refuse) throw new Error('NAMEs are not bought here')
      if (opts.busyAfter != null && opened >= opts.busyAfter) throw (opts.busy ?? shopBusy)()
      opened++
      return { v: 1, operation: 'n1', buyer: BUYER, state: 'reviewing', items: [], name: params?.name, mode: params?.mode }
    }
    if (method === 'shop.status') return opts.status ?? []
    throw new Error(`unexpected ${method}`)
  })
}

function renderShop(): InWorldShopProps {
  const props: InWorldShopProps = {
    shop: { open: true, toggle: vi.fn() },
    profile: fakeProfileState({ data: ME }),
    onNavigate: vi.fn(),
    setEngineViewport: vi.fn(),
    equipped: [],
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
  return props
}

const nametag = (): HTMLElement | null => document.querySelector<HTMLElement>('[data-nametag]')
const claim = (): HTMLButtonElement => screen.getByRole('button', { name: /^Claim name/ })
const nameEvent = (event: Record<string, unknown>): void =>
  act(() => shell.handlers.get(SHOP_EVENT)?.({ type: SHOP_EVENT, v: 1, operation: 'n1', buyer: BUYER, items: [], name: 'Carolo', mode: 'simulation', ...event }))
const itemEvent = (event: Record<string, unknown>): void =>
  act(() => shell.handlers.get(SHOP_EVENT)?.({ type: SHOP_EVENT, v: 1, operation: 'i1', buyer: BUYER, items: [{ contract: CONTRACT, itemId: '1', quantity: 1 }], ...event }))

/** Names, a NAME typed, and CLAIM NAME once it is priced. */
async function claimName(name = 'Carolo'): Promise<void> {
  await userEvent.click(screen.getByRole('tab', { name: 'Names' }))
  await userEvent.type(screen.getByRole('textbox', { name: 'Your NAME' }), name)
  await waitFor(() => expect(claim()).toBeEnabled())
  await userEvent.click(claim())
}

const buyNameCalls = () => shell.request.mock.calls.filter(([m]) => m === 'shop.buyName')
/** On a catalogue tab: a preview would take the page to Names. */
async function awayFromNames(): Promise<void> {
  await userEvent.click(screen.getByRole('tab', { name: 'Wearables' }))
  expect(screen.queryByRole('textbox', { name: 'Your NAME' })).toBeNull()
}
/** Whatever the shell sent has been heard. */
const settle = async (): Promise<void> => {
  await act(() => new Promise<void>((resolve) => setTimeout(resolve, 10)))
}

afterEach(() => {
  resetPopups()
  primeRemoteFlags(null)
  shell.request.mockReset()
  shell.handlers.clear()
  balanceReads = 0
  vi.unstubAllGlobals()
  vi.restoreAllMocks()
})

describe('CLAIM NAME in the web shell', () => {
  it('opens the shell’s rehearsal where it simulates NAMEs: the NAME and the mode it said, nothing else', async () => {
    serve()
    answerShell('simulation')
    const open = vi.spyOn(window, 'open').mockReturnValue(null)
    renderShop()
    await claimName()
    await waitFor(() => expect(buyNameCalls()).toHaveLength(1))
    expect(buyNameCalls()[0][1]).toEqual({ v: 1, name: 'Carolo', method: 'credits', mode: 'simulation', input: 'pointer' })
    // No web Shop on the way.
    expect(screen.queryByText('You will be redirected to the web Shop')).toBeNull()
    expect(open).not.toHaveBeenCalled()
  })

  it.each([
    ['says nothing of NAMEs', undefined, false],
    ['buys them live, which this page can’t wear yet', 'live', false],
    ['names a mode it doesn’t know', 'dry-run', false],
    ['refuses BUY NAME', 'simulation', true]
  ])('registers the NAME on the web Shop where the shell %s', async (_, mode, refuse) => {
    serve()
    answerShell(mode, { refuse })
    const open = vi.spyOn(window, 'open').mockReturnValue(null)
    renderShop()
    await claimName()
    const dialog = await screen.findByRole('dialog', {}, { timeout: 5000 })
    expect(within(dialog).getByText('You will be redirected to the web Shop')).toBeInTheDocument()
    await userEvent.click(within(dialog).getByRole('button', { name: 'CONTINUE' }))
    expect(open).toHaveBeenCalledWith(webNameUrl('Carolo'), '_blank', 'noopener')
    expect(buyNameCalls()).toHaveLength(refuse ? 1 : 0)
  })

  /** CLAIM NAME twice, the shell busy with the first's rehearsal as it answers the second. */
  const claimTwice = async (opts: { busy?: () => Error; codedErrors?: boolean }): Promise<void> => {
    serve()
    answerShell('simulation', { busyAfter: 1, ...opts })
    renderShop()
    await claimName()
    await waitFor(() => expect(buyNameCalls()).toHaveLength(1))
    await userEvent.click(claim())
    await waitFor(() => expect(buyNameCalls()).toHaveLength(2))
  }

  // Both shells this HUD may be hosted by: this one, busy by its code, and one from before codes (#444's), busy
  // in the same words alone, whose page this HUD may outlive (it reloads in its frame while the page stays).
  for (const [host, opts] of [
    ['the shell', {}],
    ['a shell from before codes', { codedErrors: false, busy: () => new Error(LEGACY_SHOP_BUSY_MESSAGE) }]
  ] as const) {
    it(`sends nowhere when ${host} is busy with another purchase: a second CLAIM NAME while the rehearsal’s dialog is up`, async () => {
      await claimTwice(opts)
      // Long enough for a refusal to have asked about the web Shop.
      await act(() => new Promise((resolve) => setTimeout(resolve, 50)))
      expect(screen.queryByText('You will be redirected to the web Shop')).toBeNull()
    })
  }

  it('takes the busy words without their code as a refusal from a shell whose refusals carry one: the web Shop', async () => {
    await claimTwice({ busy: () => new Error(LEGACY_SHOP_BUSY_MESSAGE) })
    expect(await screen.findByText('You will be redirected to the web Shop')).toBeInTheDocument()
  })

  it('previews a rehearsed NAME over the avatar once, however often its choice is heard, and saves nothing', async () => {
    serve()
    answerShell('simulation')
    const props = renderShop()
    await waitFor(() => expect(shell.handlers.has(SHOP_EVENT)).toBe(true))
    await awayFromNames()
    nameEvent({ state: 'succeeded' })
    nameEvent({ state: 'succeeded', next: 'previewName' })
    expect(await screen.findByRole('textbox', { name: 'Your NAME' })).toHaveValue('Carolo')
    expect(nametag()).toHaveTextContent('Carolo')
    expect(await screen.findByText('Preview only: your profile’s name hasn’t changed.')).toBeInTheDocument()
    // The same choice again (a replayed event, or shop.status) previews nothing more.
    await awayFromNames()
    nameEvent({ state: 'succeeded', next: 'previewName' })
    await settle()
    expect(screen.queryByRole('textbox', { name: 'Your NAME' })).toBeNull()
    // Nothing of the profile is saved, and a NAME is no item: no Backpack.
    expect(props.profile.save).not.toHaveBeenCalled()
    expect(props.onNavigate).not.toHaveBeenCalled()
  })

  it('previews nothing for a choice made before the app came up', async () => {
    serve()
    answerShell('simulation', { status: [{ v: 1, operation: 'n1', buyer: BUYER, state: 'succeeded', items: [], name: 'Carolo', mode: 'simulation', next: 'previewName' }] })
    renderShop()
    await waitFor(() => expect(shell.request).toHaveBeenCalledWith('shop.status', {}))
    await settle()
    await awayFromNames()
    nameEvent({ state: 'succeeded', next: 'previewName' })
    await settle()
    expect(screen.queryByRole('textbox', { name: 'Your NAME' })).toBeNull()
  })

  it.each([
    ['another account’s rehearsal', { buyer: '0x' + '9'.repeat(40) }],
    ['a live NAME previewed', { mode: 'live' }],
    ['a preview before it succeeded', { state: 'processing' }],
    ['a NAME with items', { items: [{ contract: CONTRACT, itemId: '1', quantity: 1 }] }]
  ])('previews nothing from %s', async (_, over) => {
    serve()
    answerShell('simulation')
    renderShop()
    await waitFor(() => expect(shell.handlers.has(SHOP_EVENT)).toBe(true))
    await awayFromNames()
    nameEvent({ state: 'succeeded', next: 'previewName', ...over })
    await settle()
    expect(screen.queryByRole('textbox', { name: 'Your NAME' })).toBeNull()
  })

  it('keeps the items’ handling to items: a NAME between two of an item’s events doesn’t repeat its Backpack', async () => {
    serve()
    answerShell('simulation')
    const props = renderShop()
    await screen.findByRole('status', { name: 'Credits balance: 700' })
    const reads = balanceReads
    // A rehearsed NAME succeeding reads no balance again: it spent nothing.
    nameEvent({ state: 'succeeded' })
    await settle()
    expect(balanceReads).toBe(reads)
    itemEvent({ state: 'succeeded', next: 'backpack' })
    expect(props.onNavigate).toHaveBeenCalledTimes(1)
    nameEvent({ state: 'succeeded', next: 'previewName' })
    itemEvent({ state: 'succeeded', next: 'backpack' })
    expect(props.onNavigate).toHaveBeenCalledTimes(1)
  })
})
