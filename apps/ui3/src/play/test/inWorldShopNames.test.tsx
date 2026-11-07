// The in-world Shop's Names: a NAME typed in UNIQUE NAME shows over the avatar in its nametag, is checked as
// the web Shop checks it (length, banned list, the registrar), priced in Credits, and registered or offered
// for on the web.
import { describe, it, expect, vi, afterEach } from 'vitest'
import { act, fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { keccak256, stringToHex } from 'viem'
import InWorldShop, { type InWorldShopProps } from '../features/shop/InWorldShop'
import { webNameResaleUrl, webNameUrl } from '../features/shop/itemLinks'
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
}

/** The banned list, the credits quote and the registrar's `available` through the catalyst's read relay. */
function serve(routes: Routes = {}): ReturnType<typeof vi.fn> {
  const f = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
    const url = new URL(String(input instanceof Request ? input.url : input), location.href)
    const path = url.pathname
    if (path.endsWith('/dcl-lists/banned-names')) {
      await routes.bannedAfter
      if (routes.bannedDown) return new Response('', { status: 503 })
      return Response.json({ data: routes.banned ?? [] })
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
    serve({ taken: ['mojito'] })
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

  it('RESALE NAME opens the NAMEs for resale on the web Marketplace', async () => {
    serve()
    const open = vi.spyOn(window, 'open').mockReturnValue(null)
    renderShop()
    await openNames()
    await userEvent.click(screen.getByRole('button', { name: 'Resale name' }))
    await continueTo('You will be redirected to the web Marketplace')
    expect(open).toHaveBeenCalledWith(webNameResaleUrl(), '_blank', 'noopener')
    // UNIQUE NAME stays open here.
    expect(screen.getByRole('button', { name: 'Unique name' })).toHaveAttribute('aria-pressed', 'true')
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
