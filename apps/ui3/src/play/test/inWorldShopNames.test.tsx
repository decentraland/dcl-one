// The in-world Shop's Names: a NAME typed in UNIQUE NAME shows over the avatar in its nametag, is checked as
// the web Shop checks it (length, banned list, the registrar), priced in Credits, and registered or offered
// for on the web; RESALE NAME lists the NAMEs on resale, each bought on the web Marketplace.
import { describe, it, expect, vi, afterEach } from 'vitest'
import { act, fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { keccak256, stringToHex } from 'viem'
import InWorldShop, { type InWorldShopProps } from '../features/shop/InWorldShop'
import { webNameTokenUrl, webNameUrl } from '../features/shop/itemLinks'
import { NAMETAG_PLACEHOLDER } from '../features/shop/ui'
import { userNameColor } from '../engine/nameColor'
import { nameQueries } from '../features/shop/namesQuery'
import { NAME_CHECK_DEBOUNCE_MS } from '../../shop/names/names'
import { CHAIN } from '../../shop/model/chain'
import { PopupHost, resetPopups } from '../design'
import type { Profile } from '../engine/protocol'
import { fakeProfileState } from './harness'

const ADDRESS = '0x00000000000000000000000000000000000ab47f'
const ME: Profile = { address: ADDRESS, name: 'Mojito', hasClaimedName: true, isGuest: false }
/** DCLRegistrar on mainnet, the network a build that names none registers on. */
const MAINNET_REGISTRAR = '0x2a187453064356c898cae034eaed119e1663acb8'
/** The Marketplace's rollout, which NAME resales and offers to an owner need. */
const MARKETPLACE_ON = { 'dapps-shop-marketplace-features': true }
const TRUE_WORD = `0x${'0'.repeat(63)}1`
const FALSE_WORD = `0x${'0'.repeat(64)}`

interface Routes {
  /** The NAMEs the registrar holds as taken (lowercase). */
  taken?: string[]
  banned?: string[]
  /** The credits quote of a NAME's 100 MANA. */
  credits?: string
  /** The registrar is down. */
  registrarDown?: boolean
  /** The banned list answers once this settles. */
  bannedAfter?: Promise<void>
  /** The banned list is down. */
  bannedDown?: boolean
  /** The credits quote is down. */
  quoteDown?: boolean
  /** decentraland.org's `dapps` rollout flags. */
  flags?: Record<string, boolean>
  /** The NAMEs on resale (/shop-api/v1/nfts?category=ens): their labels and MANA prices, or down; or the
   *  answer for each page, by how many it skips. */
  resales?: { names: { name: string; priceWei: string }[]; total?: number } | 'down' | ((skip: number) => { names: { name: string; priceWei: string }[]; total: number })
}

const REGISTRAR = '0x2a187453064356c898cae034eaed119e1663acb8'

/** A NAME on resale as the resale feed answers it. */
const resaleRow = (name: string, priceWei: string, tokenId: string): Record<string, unknown> => ({
  nft: { category: 'ens', chainId: 1, contractAddress: REGISTRAR, data: { ens: { subdomain: name } }, id: `${REGISTRAR}-${tokenId}`, name, network: 'ETHEREUM', tokenId, owner: '0x0000000000000000000000000000000000000001' },
  order: { id: `o-${tokenId}`, contractAddress: REGISTRAR, tokenId, price: priceWei, status: 'open' },
  rental: null
})

/** The banned list, the credits quote and the registrar's `available` through the catalyst's read relay. */
function serve(routes: Routes = {}): ReturnType<typeof vi.fn> {
  const f = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
    const url = new URL(String(input instanceof Request ? input.url : input), location.href)
    const path = url.pathname
    if (path.endsWith('/dapps.json')) return Response.json({ flags: routes.flags ?? {}, variants: {} })
    if (path.endsWith('/dcl-lists/banned-names')) {
      await routes.bannedAfter
      if (routes.bannedDown) return new Response('', { status: 503 })
      return Response.json({ data: routes.banned ?? [] })
    }
    if (path.endsWith('/shop-api/v1/nfts')) {
      if (routes.resales === 'down') return new Response('', { status: 503 })
      const page = typeof routes.resales === 'function' ? routes.resales(Number(url.searchParams.get('skip') ?? 0)) : routes.resales
      const names = page?.names ?? []
      return Response.json({ data: names.map((n, i) => resaleRow(n.name, n.priceWei, String(i + 1))), total: page?.total ?? names.length })
    }
    if (path.endsWith('/shop-api/v1/credit-quotes')) return routes.quoteDown ? new Response('', { status: 503 }) : Response.json({ items: [], amounts: [routes.credits ?? '94'] })
    if (path.includes('/rpc/')) {
      if (routes.registrarDown) return new Response('', { status: 503 })
      const body = JSON.parse(String(init?.body)) as { id: number; params: [{ data: string }] }
      // `available(string)`: the label is the call's last word, UTF-8, right-padded.
      const hex = body.params[0].data.slice(10 + 128)
      const label = Buffer.from(hex, 'hex').toString('utf8').replace(/\0+$/, '').toLowerCase()
      return Response.json({ jsonrpc: '2.0', id: body.id, result: (routes.taken ?? []).includes(label) ? FALSE_WORD : TRUE_WORD })
    }
    return new Response('', { status: 404 })
  })
  vi.stubGlobal('fetch', f)
  return f
}

function renderShop(over: Partial<InWorldShopProps> = {}) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } })
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
    zoom: vi.fn(),
    ...over
  }
  const tree = (next: Partial<InWorldShopProps> = {}): React.JSX.Element => (
    <QueryClientProvider client={client}>
      <InWorldShop {...props} {...next} />
      <PopupHost />
    </QueryClientProvider>
  )
  const view = render(tree())
  return { props, rerender: (next: Partial<InWorldShopProps>) => view.rerender(tree(next)) }
}

async function openNames(): Promise<HTMLInputElement> {
  await userEvent.click(screen.getByRole('tab', { name: 'Names' }))
  return screen.getByRole('textbox', { name: 'Your NAME' })
}

const nametag = (): HTMLElement | null => document.querySelector<HTMLElement>('[data-nametag]')
const claim = (): HTMLButtonElement => screen.getByRole('button', { name: /^Claim name/ })

/** CONTINUE on the redirect dialog with this title. */
async function continueTo(title: string): Promise<void> {
  const dialog = await screen.findByRole('dialog', {}, { timeout: 5000 })
  expect(within(dialog).getByText(title)).toBeInTheDocument()
  await userEvent.click(within(dialog).getByRole('button', { name: 'CONTINUE' }))
}


afterEach(() => {
  resetPopups()
  vi.unstubAllGlobals()
  vi.restoreAllMocks()
})

describe('the in-world Shop’s Names', () => {
  it('opens on UNIQUE NAME, the nametag over the avatar reading the placeholder until a NAME is typed', async () => {
    serve()
    renderShop()
    const field = await openNames()
    expect(screen.getByRole('heading', { name: 'Get your unique NAME!' })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'Unique name' })).toHaveAttribute('aria-pressed', 'true')
    expect(nametag()).toHaveTextContent(NAMETAG_PLACEHOLDER)

    await userEvent.type(field, 'Carolo')
    expect(nametag()).toHaveTextContent('Carolo')
    expect(nametag()).not.toHaveTextContent(NAMETAG_PLACEHOLDER)
    // In the colour the NAME takes once claimed, with the field's caret while it has the focus.
    expect(nametag()!.querySelector('span')).toHaveStyle({ color: userNameColor('Carolo', ADDRESS, true) })
    expect(within(nametag()!).getByTestId('nametag-caret')).toBeInTheDocument()
    fireEvent.blur(field)
    expect(within(nametag()!).queryByTestId('nametag-caret')).toBeNull()
  })

  it('keeps letters and digits only, and one past the limit says so', async () => {
    serve()
    renderShop()
    const field = await openNames()
    await userEvent.type(field, 'Calabaza Chop-eria!!')
    expect(field).toHaveValue('CalabazaChoperia')
    // Sixteen at most: the limit's one past it.
    await userEvent.type(field, 'XYZ')
    expect(field).toHaveValue('CalabazaChoperia')
    expect(screen.getByText(/16\/15/)).toHaveTextContent('16/15Character Limit Reached')
    expect(field).toHaveAttribute('aria-invalid', 'true')
    expect(claim()).toBeDisabled()
    expect(nametag()).toHaveTextContent('CalabazaChoperia')
  })

  it('cleans a pasted NAME before capping it, so its letters are kept and the limit shows', async () => {
    serve()
    renderShop()
    const field = await openNames()
    await userEvent.click(field)
    await userEvent.paste('Calabaza Chop-eria!!')
    expect(field).toHaveValue('CalabazaChoperia')
    expect(screen.getByText(/16\/15/)).toHaveTextContent('16/15Character Limit Reached')
  })

  it('offers a free NAME only with its Credits price, and asks again for it', async () => {
    serve({ quoteDown: true })
    renderShop()
    const field = await openNames()
    await userEvent.type(field, 'Carolo')
    expect(await screen.findByText(/couldn’t check that NAME/)).toBeInTheDocument()
    expect(claim()).toBeDisabled()
    serve({ credits: '94' })
    await userEvent.click(screen.getByRole('button', { name: 'TRY AGAIN' }))
    await waitFor(() => expect(claim()).toBeEnabled())
    expect(claim()).toHaveAccessibleName('Claim name for 94 Credits')
  })

  it('lets the registrar decide once the banned list fails', async () => {
    serve({ bannedDown: true })
    renderShop()
    const field = await openNames()
    await userEvent.type(field, 'Carolo')
    await waitFor(() => expect(claim()).toBeEnabled())
  })

  it('counts while typed in, and shows the account’s # and last four', async () => {
    serve()
    renderShop()
    const field = await openNames()
    await userEvent.type(field, 'Carolo')
    expect(screen.getByText('6/15')).toBeInTheDocument()
    expect(screen.getByText('#b47f')).toBeInTheDocument()
  })

  it('prices an available NAME in Credits, and CLAIM NAME registers it on the web Shop', async () => {
    serve({ credits: '94' })
    const open = vi.spyOn(window, 'open').mockReturnValue(null)
    renderShop()
    const field = await openNames()
    await userEvent.type(field, 'Carolo')
    await waitFor(() => expect(claim()).toBeEnabled())
    expect(claim()).toHaveAccessibleName('Claim name for 94 Credits')
    await userEvent.click(claim())
    await continueTo('You will be redirected to the web Shop')
    expect(open).toHaveBeenCalledWith(webNameUrl('Carolo'), '_blank', 'noopener')
    expect(webNameUrl('Carolo')).toMatch(/\/shop\/names\?name=Carolo&utm_source=/)
  })

  it('says a taken NAME is taken, and MAKE OFFER opens its bid page on the web Marketplace, listed or not', async () => {
    serve({ taken: ['mojito'], flags: MARKETPLACE_ON })
    const open = vi.spyOn(window, 'open').mockReturnValue(null)
    renderShop()
    const field = await openNames()
    await userEvent.type(field, 'Mojito')
    expect(await screen.findByText(/This NAME Is Taken/)).toBeInTheDocument()
    expect(field).toHaveAttribute('aria-invalid', 'true')
    expect(claim()).toBeDisabled()
    await userEvent.click(screen.getByRole('button', { name: 'MAKE OFFER' }))
    await continueTo('You will be redirected to the web Marketplace')
    // The registrar's token for the NAME (keccak of its lowercase label): an offer to its owner, which needs
    // no listing (the resale browse only shows NAMEs on sale).
    const token = BigInt(keccak256(stringToHex('mojito'))).toString()
    const url = new URL(open.mock.calls[0]![0] as string)
    expect(url.pathname).toBe(`/shop/token/${MAINNET_REGISTRAR}/${token}/bid`)
    expect(url.searchParams.has('section')).toBe(false)
  })

  it('without the Marketplace’s rollout a taken NAME is only taken: no MAKE OFFER and no RESALE NAME', async () => {
    const f = serve({ taken: ['mojito'] })
    renderShop()
    const field = await openNames()
    await userEvent.type(field, 'Mojito')
    expect(await screen.findByText('This NAME Is Taken. Choose Another NAME.')).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: 'MAKE OFFER' })).toBeNull()
    expect(screen.queryByRole('button', { name: 'Resale name' })).toBeNull()
    // Nor is the resale feed read.
    expect(f.mock.calls.some(([input]) => new URL(String(input), location.href).pathname.endsWith('/shop-api/v1/nfts'))).toBe(false)
  })

  it('a guest gets no MAKE OFFER and no RESALE NAME even with the rollout on: there is no account to hold a wallet', async () => {
    serve({ taken: ['mojito'], flags: MARKETPLACE_ON })
    renderShop({ profile: fakeProfileState({ data: { ...ME, isGuest: true, hasClaimedName: false } }) })
    const field = await openNames()
    await userEvent.type(field, 'Mojito')
    expect(await screen.findByText('This NAME Is Taken. Choose Another NAME.')).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: 'MAKE OFFER' })).toBeNull()
    expect(screen.queryByRole('button', { name: 'Resale name' })).toBeNull()
  })

  it('offers no NAME until the banned list answers', async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true })
    try {
      let answer = (): void => undefined
      const f = serve({ banned: ['badname'], bannedAfter: new Promise<void>((r) => (answer = r)) })
      renderShop()
      const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime })
      await user.click(screen.getByRole('tab', { name: 'Names' }))
      await user.type(screen.getByRole('textbox', { name: 'Your NAME' }), 'BadName')
      // Past the typing's rest, with the list still out: nothing is asked of the registrar.
      await act(() => vi.advanceTimersByTimeAsync(NAME_CHECK_DEBOUNCE_MS * 2))
      expect(claim()).toBeDisabled()
      expect(f.mock.calls.some(([input]) => String(input).includes('/rpc/'))).toBe(false)
      answer()
      expect(await screen.findByText(/This NAME Is Not Available/)).toBeInTheDocument()
      expect(f.mock.calls.some(([input]) => String(input).includes('/rpc/'))).toBe(false)
    } finally {
      vi.useRealTimers()
    }
  })

  it('takes nothing typed into a full field, and leaves the caret where it was', async () => {
    serve()
    renderShop()
    const field = await openNames()
    await userEvent.type(field, 'CalabazaChoperia')
    await userEvent.type(field, 'X', { initialSelectionStart: 3, initialSelectionEnd: 3 })
    expect(field).toHaveValue('CalabazaChoperia')
    await waitFor(() => expect(field.selectionStart).toBe(3))
  })

  it('announces what is wrong with the NAME, not the count on every keystroke', async () => {
    serve({ taken: ['mojito'] })
    renderShop()
    const field = await openNames()
    await userEvent.type(field, 'Moji')
    // The field's description holds the live region, as a screen reader relates them.
    const live = within(document.getElementById(field.getAttribute('aria-describedby')!)!).getByRole('status')
    expect(screen.getByText('4/15')).toBeInTheDocument()
    expect(live).not.toHaveTextContent('4/15')
    await userEvent.type(field, 'to')
    await waitFor(() => expect(live).toHaveTextContent(/This NAME Is Taken/))
  })

  it('keys a NAME’s availability by the network it is read on', () => {
    expect(nameQueries.availability('Mojito', CHAIN.ETHEREUM).queryKey).not.toEqual(nameQueries.availability('Mojito', CHAIN.SEPOLIA).queryKey)
    expect(nameQueries.availability('Mojito', CHAIN.SEPOLIA).queryKey).toEqual(nameQueries.availability('mojito', CHAIN.SEPOLIA).queryKey)
  })

  it('refuses a banned NAME without asking the registrar', async () => {
    const f = serve({ banned: ['badname'] })
    renderShop()
    const field = await openNames()
    await userEvent.type(field, 'BadName')
    expect(await screen.findByText(/This NAME Is Not Available/)).toBeInTheDocument()
    expect(claim()).toBeDisabled()
    expect(f.mock.calls.some(([input]) => String(input).includes('/rpc/'))).toBe(false)
  })

  it('offers to try again when the registrar can’t be read', async () => {
    serve({ registrarDown: true })
    renderShop()
    const field = await openNames()
    await userEvent.type(field, 'Carolo')
    expect(await screen.findByText(/couldn’t check that NAME/)).toBeInTheDocument()
    serve()
    await userEvent.click(screen.getByRole('button', { name: 'TRY AGAIN' }))
    await waitFor(() => expect(claim()).toBeEnabled())
  })

  it('RESALE NAME lists the NAMEs on resale with their MANA price, each bought on the web Marketplace', async () => {
    serve({ flags: MARKETPLACE_ON, resales: { names: [{ name: 'Mojito', priceWei: '500000000000000000000' }, { name: 'Cattie', priceWei: '12500000000000000000' }] } })
    const open = vi.spyOn(window, 'open').mockReturnValue(null)
    renderShop()
    await openNames()
    await userEvent.click(screen.getByRole('button', { name: 'Resale name' }))
    expect(screen.getByRole('button', { name: 'Resale name' })).toHaveAttribute('aria-pressed', 'true')
    const card = (await screen.findByRole('button', { name: 'Mojito.dcl.eth' })).parentElement!
    expect(within(card).getByText('Decentraland Name')).toBeInTheDocument()
    expect(within(card).getByTitle('500 MANA')).toHaveTextContent('500')
    expect(screen.getByRole('button', { name: 'Cattie.dcl.eth' })).toBeInTheDocument()
    await userEvent.click(screen.getByRole('button', { name: 'Mojito.dcl.eth' }))
    await continueTo('You will be redirected to the web Marketplace')
    expect(open).toHaveBeenCalledWith(webNameTokenUrl(REGISTRAR, '1'), '_blank', 'noopener')
    expect(new URL(open.mock.calls[0]![0] as string).pathname).toBe(`/shop/token/${REGISTRAR}/1`)
  })

  it('says when no NAME is on resale, and offers to try again when the list can’t be read', async () => {
    serve({ resales: 'down', flags: MARKETPLACE_ON })
    renderShop()
    await openNames()
    await userEvent.click(screen.getByRole('button', { name: 'Resale name' }))
    expect(await screen.findByText('Couldn\'t load the NAMEs on resale')).toBeInTheDocument()
    serve({ flags: MARKETPLACE_ON, resales: { names: [] } })
    await userEvent.click(screen.getByRole('button', { name: 'Retry' }))
    expect(await screen.findByText('No NAMEs on resale right now')).toBeInTheDocument()
  })

  it('pages RESALE NAME through the list, newest listings first', async () => {
    const f = serve({ flags: MARKETPLACE_ON, resales: { names: [{ name: 'Mojito', priceWei: '1000000000000000000' }], total: 30 } })
    renderShop()
    await openNames()
    await userEvent.click(screen.getByRole('button', { name: 'Resale name' }))
    await screen.findByRole('button', { name: 'Mojito.dcl.eth' })
    const asked = (): URL[] => f.mock.calls.map(([input]) => new URL(String(input), location.href)).filter((u) => u.pathname.endsWith('/shop-api/v1/nfts'))
    expect(asked().at(-1)!.searchParams.get('sortBy')).toBe('recently_listed')
    expect(asked().at(-1)!.searchParams.get('isOnSale')).toBe('true')
    await userEvent.click(screen.getByRole('button', { name: '2' }))
    await waitFor(() => expect(asked().at(-1)!.searchParams.get('skip')).toBe('24'))
  })

  it('goes back to the last page there is when the one shown empties', async () => {
    // Page 2 held one NAME, sold before it was read: 24 are left, all on page 1.
    const firstPage = Array.from({ length: 24 }, (_, i) => ({ name: i === 0 ? 'Mojito' : `Name${i}`, priceWei: '1000000000000000000' }))
    let sold = false
    serve({ flags: MARKETPLACE_ON, resales: (skip) => (skip === 0 ? { names: firstPage, total: sold ? 24 : 25 } : { names: [], total: 24 }) })
    renderShop()
    await openNames()
    await userEvent.click(screen.getByRole('button', { name: 'Resale name' }))
    await screen.findByRole('button', { name: 'Mojito.dcl.eth' })
    // Whatever shows on the way: "none on resale" never does, with 24 left. Each added node keeps its text
    // after it is gone, so even a state shown for one commit is seen.
    const seen: string[] = []
    const watch = new MutationObserver((records) => {
      for (const record of records) for (const node of record.addedNodes) seen.push(node.textContent ?? '')
    })
    watch.observe(document.body, { childList: true, subtree: true, characterData: true })
    sold = true
    await userEvent.click(screen.getByRole('button', { name: '2' }))
    expect(await screen.findByRole('button', { name: 'Mojito.dcl.eth' })).toBeInTheDocument()
    // The first page is read again for what's left, so the pager no longer offers the page that emptied.
    await waitFor(() => expect(screen.queryByRole('button', { name: '2' })).not.toBeInTheDocument())
    expect(screen.getByRole('button', { name: 'Mojito.dcl.eth' })).toBeInTheDocument()
    watch.disconnect()
    expect(seen.some((text) => text.includes('No NAMEs on resale right now'))).toBe(false)
  })

  it('shows the player’s own name over the avatar on RESALE NAME: claimed with its badge, unclaimed with its suffix', async () => {
    serve({ flags: MARKETPLACE_ON, resales: { names: [] } })
    const { rerender, props } = renderShop()
    await openNames()
    await userEvent.click(screen.getByRole('button', { name: 'Resale name' }))
    expect(nametag()).toHaveTextContent('Mojito')
    expect(nametag()!.querySelector('img')).not.toBeNull()
    // The engine names an unclaimed name with # and the last four of the address (ADDRESS ends in b47f).
    rerender({ profile: { ...props.profile, data: { ...ME, name: 'Guest#b47f', hasClaimedName: false } } })
    expect(nametag()).toHaveTextContent(/^Guest#b47f$/)
    expect(nametag()!.querySelector('img')).toBeNull()
    // A name that comes without them takes them from the address.
    rerender({ profile: { ...props.profile, data: { ...ME, name: 'Guest', hasClaimedName: false } } })
    expect(nametag()).toHaveTextContent(/^Guest#b47f$/)
    expect(nametag()!.querySelector('img')).toBeNull()
  })

  it('hides the nametag while the avatar is zoomed, and shows it once back out', async () => {
    serve()
    renderShop()
    await openNames()
    const surface = screen.getByTestId('avatar-drag-surface')
    fireEvent.wheel(surface, { deltaY: -400 })
    await waitFor(() => expect(nametag()).toBeNull())
    fireEvent.wheel(surface, { deltaY: 4000 })
    await waitFor(() => expect(nametag()).not.toBeNull())
  })

  it('shows no nametag outside Names, and a reopened Shop starts with no NAME typed', async () => {
    serve()
    const { rerender, props } = renderShop()
    expect(nametag()).toBeNull()
    const field = await openNames()
    await userEvent.type(field, 'Carolo')
    rerender({ shop: { ...props.shop, open: false } })
    rerender({ shop: { ...props.shop, open: true } })
    expect(await openNames()).toHaveValue('')
    expect(nametag()).toHaveTextContent(NAMETAG_PLACEHOLDER)
  })
})
