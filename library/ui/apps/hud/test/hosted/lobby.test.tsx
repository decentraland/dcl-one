import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { PopupHost, resetPopups } from '../../design'
import { BackpackModal } from '../../features/backpack/BackpackModal'
import modalStyles from '../../features/backpack/BackpackModal.module.css'
import { LobbyHome } from '../../features/lobby/LobbyHome'
import { openDestinationCard } from '../../features/places/DestinationCard'
import { openPassport } from '../../features/profile/Passport'
import { fetchEvents, type DclEvent } from '@explorer-core/features/events/eventsApi'
import { fetchHighlighted, fetchHomePlace, fetchRecents } from '@explorer-core/features/lobby/lobbyApi'
import { BACKDROP_ASPECT, backdropRect, PODIUM_IMAGE, podiumRect } from '@explorer-core/engine/lobbyStage'
import type { Friend } from '@explorer-core/engine/protocol'
import type { DiscoverPlace } from '@explorer-core/features/places/placesApi'
import { checkRealm } from '@explorer-core/lib/realmCheck'
import { receiveProfile, resetProfileStore, setProfileRequester } from '../../features/session/profileStore'
import { SessionProvider } from '../../features/session/SessionContext'
import { setBindingsSnapshot } from '../../lib/bindingLabels'
import { fakeSession } from '../harness'

vi.mock('@explorer-core/features/lobby/lobbyApi', () => ({
  fetchHomePlace: vi.fn(async () => null),
  fetchRecents: vi.fn(async () => []),
  fetchHighlighted: vi.fn(async () => [])
}))

vi.mock('@explorer-core/features/events/eventsApi', async (original) => ({
  ...(await original<typeof import('@explorer-core/features/events/eventsApi')>()),
  fetchEvents: vi.fn(async () => [])
}))

vi.mock('@explorer-core/lib/realmCheck', async (original) => ({
  ...(await original<typeof import('@explorer-core/lib/realmCheck')>()),
  checkRealm: vi.fn(async () => 'ok')
}))

vi.mock('../../features/places/DestinationCard', async (original) => ({
  ...(await original<typeof import('../../features/places/DestinationCard')>()),
  openDestinationCard: vi.fn()
}))

vi.mock('../../features/profile/Passport', async (original) => ({
  ...(await original<typeof import('../../features/profile/Passport')>()),
  openPassport: vi.fn()
}))

/** What the events service lists as on now and as to come; the lobby reads both lists. */
const listEvents = (lists: { live?: DclEvent[]; upcoming?: DclEvent[] }): void => {
  vi.mocked(fetchEvents).mockImplementation(async (list) => lists[list] ?? [])
}

afterEach(() => {
  vi.restoreAllMocks()
  vi.clearAllMocks()
  listEvents({})
  resetPopups()
  setBindingsSnapshot([]) // module store leaks across tests
})

const place = (id: string): DiscoverPlace => ({ id, title: `Place ${id}`, description: '', image: '', positions: ['1,1'], owner: null })

describe('lobby controls', () => {
  it('disables Customize until the player is ready', async () => {
    const session = { ...fakeSession(), playerReady: false }
    const view = (): React.JSX.Element => (
      <SessionProvider value={session}>
        <LobbyHome onPick={session.pickDestination} setEngineViewport={session.setEngineViewport} />
      </SessionProvider>
    )
    const { rerender } = render(view())
    expect(screen.getByRole('button', { name: 'Customize' })).toBeDisabled()
    fireEvent.click(screen.getByRole('button', { name: 'Customize' }))
    expect(session.backpack.toggle).not.toHaveBeenCalled()
    session.playerReady = true
    rerender(view())
    fireEvent.click(screen.getByRole('button', { name: 'Customize' }))
    expect(session.backpack.toggle).toHaveBeenCalledOnce()
    await waitFor(() => expect(screen.getByRole('button', { name: /jump in/i })).toBeInTheDocument())
  })

  it('turns the avatar by a drag across it and opens the Backpack only by a click', async () => {
    const session = fakeSession()
    render(
      <SessionProvider value={session}>
        <LobbyHome onPick={session.pickDestination} setEngineViewport={session.setEngineViewport} />
      </SessionProvider>
    )
    const avatar = screen.getByRole('button', { name: 'Customize' })
    fireEvent.pointerDown(avatar, { pointerId: 1, button: 0, clientX: 100 })
    fireEvent.pointerMove(avatar, { pointerId: 1, clientX: 102 })
    expect(session.turnLobbyAvatar).not.toHaveBeenCalled()
    fireEvent.pointerMove(avatar, { pointerId: 1, clientX: 140 })
    fireEvent.pointerMove(avatar, { pointerId: 1, clientX: 130 })
    expect(vi.mocked(session.turnLobbyAvatar).mock.calls).toEqual([[-20], [5]])
    fireEvent.pointerUp(avatar, { pointerId: 1 })
    // the click the browser sends at the end of the drag
    fireEvent.click(avatar, { detail: 1 })
    expect(session.backpack.toggle).not.toHaveBeenCalled()
    fireEvent.pointerMove(avatar, { pointerId: 1, clientX: 90 })
    expect(session.turnLobbyAvatar).toHaveBeenCalledTimes(2)
    // Enter or Space on the button (a click with no press) opens it, also when the drag sent no click
    fireEvent.pointerDown(avatar, { pointerId: 3, button: 0, clientX: 100 })
    fireEvent.pointerMove(avatar, { pointerId: 3, clientX: 140 })
    fireEvent.pointerUp(avatar, { pointerId: 3 })
    fireEvent.click(avatar, { detail: 0 })
    expect(session.backpack.toggle).toHaveBeenCalledOnce()
    vi.mocked(session.backpack.toggle).mockClear()

    fireEvent.pointerDown(avatar, { pointerId: 2, button: 0, clientX: 100 })
    fireEvent.pointerUp(avatar, { pointerId: 2 })
    fireEvent.click(avatar)
    expect(session.backpack.toggle).toHaveBeenCalledOnce()
    await waitFor(() => expect(screen.getByRole('button', { name: /jump in/i })).toBeInTheDocument())
  })

  it("jumps to an upcoming world event only when its server is a world's name", async () => {
    const soon = new Date(Date.now() + 3_600_000).toISOString()
    // the detail card's Jump In, without the popup layer
    vi.mocked(openDestinationCard).mockImplementation((_info, onJumpIn) => onJumpIn())
    const session = fakeSession()
    const onPick = vi.fn()
    const view = (): React.JSX.Element => (
      <SessionProvider value={session}>
        <LobbyHome onPick={onPick} setEngineViewport={session.setEngineViewport} />
      </SessionProvider>
    )
    // the lobby shows one upcoming event at a time
    listEvents({ upcoming: [{ id: 'bad', name: 'Bad Fest', start_at: soon, x: 0, y: 0, world: true, server: 'https://realm.example/about' }] })
    const bad = render(view())
    expect(await screen.findByRole('button', { name: /Bad Fest/ })).toBeDisabled()
    bad.unmount()
    listEvents({ upcoming: [{ id: 'good', name: 'Good Fest', start_at: soon, x: 0, y: 0, world: true, server: 'fest.dcl.eth' }] })
    render(view())
    fireEvent.click(await screen.findByRole('button', { name: /Good Fest/ }))
    expect(onPick).toHaveBeenCalledWith({ kind: 'world', realm: 'fest.dcl.eth' })
    // nothing is live: the lobby has no Live Events widget
    expect(screen.queryByRole('heading', { name: 'Live Events' })).not.toBeInTheDocument()
  })

  it('shows an event that is on now with who is there, jumps in by its button and opens its details by the card', async () => {
    vi.mocked(openDestinationCard).mockImplementation(() => {})
    const base = fakeSession()
    const friends = ['0xA1', '0xa2', '0xa3', '0xa4', '0xff'].map((address, i): Friend => ({ address, name: `Friend${i}`, status: 'online' }))
    const session = { ...base, friends: { ...base.friends, list: friends } }
    const onPick = vi.fn()
    // the busiest event comes first, whatever the service's order
    listEvents({
      live: [
        { id: 'quiz', name: 'Quiz Night', start_at: new Date(Date.now() - 60_000).toISOString(), x: 0, y: 0, live: true },
        { id: 'mic', name: 'Open Mic', start_at: new Date(Date.now() - 3_600_000).toISOString(), x: 10, y: -4, live: true, user_name: 'The Host', connected_addresses: ['0xa1', '0xA2', '0xa3', '0xa4', '0xb5', '0xb6'] }
      ]
    })
    render(
      <SessionProvider value={session}>
        <LobbyHome onPick={onPick} setEngineViewport={session.setEngineViewport} />
      </SessionProvider>
    )
    const widget = within((await screen.findByRole('heading', { name: 'Live Events' })).closest('section')!)
    // the service sends who is at an event only when asked
    expect(fetchEvents).toHaveBeenCalledWith('live', { connected: true })
    // one event a page: its name, its host, how many are there, and the friends among them
    expect(widget.getByText('Open Mic')).toBeInTheDocument()
    expect(widget.getByText('The Host')).toBeInTheDocument()
    expect(widget.getByText('Live')).toBeInTheDocument()
    expect(widget.getByText('6')).toBeInTheDocument()
    expect(widget.getByText('+1')).toBeInTheDocument()
    expect(widget.queryByText('Quiz Night')).not.toBeInTheDocument()

    fireEvent.click(widget.getByText('Open Mic'))
    expect(openDestinationCard).toHaveBeenCalledWith(expect.objectContaining({ title: 'Open Mic', count: 6 }), expect.any(Function))
    expect(onPick).not.toHaveBeenCalled()
    fireEvent.click(widget.getByRole('button', { name: /jump in/i }))
    expect(onPick).toHaveBeenCalledWith({ kind: 'parcel', x: 10, y: -4 })
    expect(openDestinationCard).toHaveBeenCalledOnce()

    fireEvent.click(widget.getByRole('button', { name: 'Next page' }))
    expect(widget.getByText('Quiz Night')).toBeInTheDocument()
    expect(widget.queryByText('Open Mic')).not.toBeInTheDocument()
    fireEvent.click(widget.getByRole('button', { name: 'View all' }))
    expect(session.events.toggle).toHaveBeenCalledWith('start_menu')
  })

  it('lists an event of a series by its next date: it stays in Upcoming Events, in order, with that countdown', async () => {
    const session = fakeSession()
    const at = (hours: number): string => new Date(Date.now() + hours * 3_600_000 + 60_000).toISOString()
    listEvents({
      upcoming: [
        { id: 'once', name: 'One Night', start_at: at(5), x: 0, y: 0 },
        // a weekly event: its first date is past, its next one is the soonest of the list
        { id: 'weekly', name: 'Weekly Jam', start_at: at(-24 * 30), next_start_at: at(2), x: 1, y: 1 },
        { id: 'over', name: 'Old Series', start_at: at(-24 * 60), next_start_at: at(-24), x: 2, y: 2 }
      ]
    })
    render(
      <SessionProvider value={session}>
        <LobbyHome onPick={session.pickDestination} setEngineViewport={session.setEngineViewport} />
      </SessionProvider>
    )
    const widget = within((await screen.findByRole('heading', { name: 'Upcoming Events' })).closest('section')!)
    // one event a page, the soonest first
    const shown = (): string | null => within(widget.getByRole('article')).getByRole('button', { name: /In \d/ }).textContent
    expect(shown()).toBe('Weekly JamIn 2 hours')
    fireEvent.click(widget.getByRole('button', { name: 'Next page' }))
    expect(shown()).toBe('One NightIn 5 hours')
    // two pages: the series whose next date is past is not in the list
    fireEvent.click(widget.getByRole('button', { name: 'Next page' }))
    expect(shown()).toBe('Weekly JamIn 2 hours')
  })

  it("puts an upcoming event in the player's calendar and shares it from its card, without opening it", async () => {
    const open = vi.spyOn(window, 'open').mockReturnValue(null)
    const session = fakeSession()
    const onPick = vi.fn()
    const start = new Date(Date.now() + 2 * 3_600_000)
    listEvents({ upcoming: [{ id: 'fest', name: 'Good Fest', start_at: start.toISOString(), duration: 3_600_000, x: 5, y: 7, user_name: 'The Host' }] })
    render(
      <SessionProvider value={session}>
        <LobbyHome onPick={onPick} setEngineViewport={session.setEngineViewport} />
        <PopupHost />
      </SessionProvider>
    )
    const card = within((await screen.findByRole('button', { name: /Good Fest/ })).closest('article')!)
    expect(card.getByRole('button', { name: /Good Fest/ })).toHaveTextContent('By The Host')
    fireEvent.click(card.getByRole('button', { name: 'Add to calendar' }))
    expect(open).toHaveBeenCalledOnce()
    const url = new URL(String(open.mock.calls[0][0]))
    expect(url.hostname).toBe('calendar.google.com')
    expect(url.searchParams.get('text')).toBe('Good Fest')
    const stamp = (d: Date): string => `${d.toISOString().slice(0, 19).replace(/[-:]/g, '')}Z`
    expect(url.searchParams.get('dates')).toBe(`${stamp(start)}/${stamp(new Date(start.getTime() + 3_600_000))}`)
    expect(url.searchParams.get('details')).toContain('position=5,7')

    expect(card.getByRole('button', { name: 'Share' })).toHaveAttribute('aria-haspopup', 'menu')
    fireEvent.click(card.getByRole('button', { name: 'Share' }))
    fireEvent.click(await screen.findByRole('menuitem', { name: 'Share on X' }))
    expect(open).toHaveBeenCalledTimes(2)
    expect(String(open.mock.calls[1][0])).toContain(encodeURIComponent('/whats-on/?id=fest'))
    expect(openDestinationCard).not.toHaveBeenCalled()
    expect(onPick).not.toHaveBeenCalled()
  })

  it('lets Customize Lobby take Live Events off and put it back', async () => {
    const session = fakeSession()
    listEvents({ live: [{ id: 'mic', name: 'Open Mic', start_at: new Date(Date.now() - 60_000).toISOString(), x: 1, y: 1, live: true }] })
    render(
      <SessionProvider value={session}>
        <LobbyHome onPick={session.pickDestination} setEngineViewport={session.setEngineViewport} />
      </SessionProvider>
    )
    await screen.findByRole('heading', { name: 'Live Events' })
    fireEvent.click(screen.getByRole('button', { name: 'Customize lobby' }))
    fireEvent.click(screen.getByRole('button', { name: 'Remove Live Events' }))
    expect(screen.queryByRole('heading', { name: 'Live Events' })).not.toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: /^Live Events ?Add$/ }))
    expect(screen.getByRole('heading', { name: 'Live Events' })).toBeInTheDocument()
  })

  it('loads the landing card when the native home arrives after mount', async () => {
    const session = fakeSession()
    const view = (): React.JSX.Element => (
      <SessionProvider value={session}>
        <LobbyHome onPick={session.pickDestination} setEngineViewport={session.setEngineViewport} />
      </SessionProvider>
    )
    const { rerender } = render(view())
    expect(screen.getByRole('button', { name: /jump in/i })).toBeDisabled()
    expect(fetchHomePlace).not.toHaveBeenCalled()
    session.homeScene = () => ({ realm: 'my-home.dcl.eth', parcel: '12,-7' })
    rerender(view())
    await waitFor(() => expect(fetchHomePlace).toHaveBeenCalledWith({ realm: 'my-home.dcl.eth', parcel: '12,-7' }))
    await waitFor(() => expect(screen.getByRole('button', { name: /jump in/i })).toBeEnabled())
    expect(screen.getByText('my-home.dcl.eth')).toBeInTheDocument()
  })

  it('takes a parcel or a World by /goto, and says so when there is none', async () => {
    vi.mocked(checkRealm).mockImplementation(async (realm) => (realm === 'nowhere.dcl.eth' ? 'not-found' : 'ok'))
    const session = fakeSession()
    const onPick = vi.fn()
    render(
      <SessionProvider value={session}>
        <LobbyHome onPick={onPick} setEngineViewport={session.setEngineViewport} />
      </SessionProvider>
    )
    const field = screen.getByRole('textbox', { name: 'Jump to any Genesis City coordinate or a World' })
    expect(field).toHaveAttribute('placeholder', 'e.g. 3,-5 or mana.dcl.eth')
    const go = screen.getByRole('button', { name: 'Go to destination' })
    expect(go).toBeDisabled()
    fireEvent.change(field, { target: { value: '3,-5' } })
    fireEvent.click(go)
    await waitFor(() => expect(onPick).toHaveBeenLastCalledWith({ kind: 'parcel', x: 3, y: -5 }))
    expect(checkRealm).not.toHaveBeenCalled()
    // Enter, with the command typed out as in chat
    fireEvent.change(field, { target: { value: '/goto mana' } })
    fireEvent.submit(field)
    await waitFor(() => expect(onPick).toHaveBeenLastCalledWith({ kind: 'world', realm: 'mana.dcl.eth' }))
    for (const wrong of ['1000,-500', 'nowhere', 'two words']) {
      fireEvent.change(field, { target: { value: wrong } })
      // an edit takes the last answer away
      expect(screen.queryByRole('alert')).not.toBeInTheDocument()
      expect(field).not.toHaveAttribute('aria-invalid')
      fireEvent.submit(field)
      expect(await screen.findByRole('alert')).toHaveTextContent("Coordinates or World name doesn't exist")
      expect(field).toHaveAttribute('aria-invalid', 'true')
    }
    expect(onPick).toHaveBeenCalledTimes(2)
    await waitFor(() => expect(screen.getByRole('button', { name: /jump in/i })).toBeInTheDocument())
  })

  it('holds a /goto pick while sign-in runs, and lets Customize Lobby take the widget off and put it back', async () => {
    const session = fakeSession()
    const onPick = vi.fn()
    const view = (busy: boolean): React.JSX.Element => (
      <SessionProvider value={{ ...session, login: { ...session.login, busy } }}>
        <LobbyHome onPick={onPick} setEngineViewport={session.setEngineViewport} />
      </SessionProvider>
    )
    const { rerender } = render(view(true))
    const name = 'Jump to any Genesis City coordinate or a World'
    fireEvent.change(screen.getByRole('textbox', { name }), { target: { value: '0,0' } })
    expect(screen.getByRole('button', { name: 'Go to destination' })).toBeDisabled()
    fireEvent.submit(screen.getByRole('textbox', { name }))
    rerender(view(false))
    expect(screen.getByRole('button', { name: 'Go to destination' })).toBeEnabled()
    expect(onPick).not.toHaveBeenCalled()

    fireEvent.click(screen.getByRole('button', { name: 'Customize lobby' }))
    fireEvent.click(screen.getByRole('button', { name: 'Remove Go To' }))
    expect(screen.queryByRole('textbox', { name })).not.toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: /^Go To ?Add$/ }))
    expect(screen.getByRole('textbox', { name })).toBeInTheDocument()
    await waitFor(() => expect(screen.getByRole('button', { name: /jump in/i })).toBeInTheDocument())
  })

  it("names a recent place's creator from the owner's profile when the place carries no name", async () => {
    resetProfileStore()
    const owner = '0xb8c4df381c6c305758f806c3aa71f37aabbcbd92'
    setProfileRequester((address) => receiveProfile(address, { address, name: 'boedo', hasClaimedName: true, isGuest: false }))
    const recent: DiscoverPlace = { id: 'w', title: 'My Scene', description: '', image: '', positions: [], owner, world: true, world_name: 'tortilla.dcl.eth' }
    vi.mocked(fetchRecents).mockResolvedValueOnce([recent])
    const session = fakeSession()
    render(
      <SessionProvider value={session}>
        <LobbyHome onPick={session.pickDestination} setEngineViewport={session.setEngineViewport} />
      </SessionProvider>
    )
    expect(await screen.findByText('boedo')).toBeInTheDocument()
    expect(screen.queryByText(owner)).not.toBeInTheDocument()
  })

  it('shows a place with its creator and who is there, jumps in by its button, opens its details by the card, and turns the recommended places three a page', async () => {
    vi.mocked(openDestinationCard).mockImplementation(() => {})
    const recent: DiscoverPlace = { ...place('recent'), positions: ['10,-4'], user_count: 24, user_name: 'Decentraland' }
    vi.mocked(fetchRecents).mockResolvedValueOnce([recent])
    vi.mocked(fetchHighlighted).mockResolvedValue(Array.from({ length: 4 }, (_, i) => place(String(i + 1))))
    const session = fakeSession()
    const onPick = vi.fn()
    render(
      <SessionProvider value={session}>
        <LobbyHome onPick={onPick} setEngineViewport={session.setEngineViewport} />
      </SessionProvider>
    )
    const back = within((await screen.findByRole('heading', { name: 'Jump Back In' })).closest('section')!)
    expect(back.getByText('Place recent')).toBeInTheDocument()
    expect(back.getByText('Decentraland')).toBeInTheDocument()
    expect(back.getByText('24')).toBeInTheDocument()
    // one page of places: no pager
    expect(back.queryByRole('button', { name: 'Page 1' })).not.toBeInTheDocument()
    expect(back.queryByRole('button', { name: 'Next page' })).not.toBeInTheDocument()

    fireEvent.click(back.getByText('Place recent'))
    expect(openDestinationCard).toHaveBeenCalledWith(expect.objectContaining({ title: 'Place recent', count: 24 }), expect.any(Function))
    expect(onPick).not.toHaveBeenCalled()
    fireEvent.click(back.getByRole('button', { name: /jump in/i }))
    expect(onPick).toHaveBeenCalledWith({ kind: 'parcel', x: 10, y: -4 })
    expect(openDestinationCard).toHaveBeenCalledOnce()

    const recommended = within((await screen.findByRole('heading', { name: 'Recommended Places' })).closest('section')!)
    await waitFor(() => expect(recommended.queryByText('Place 4')).not.toBeInTheDocument())
    // each card has its own Jump in; a place nobody is in and nobody signed carries the name alone
    expect(recommended.getAllByRole('button', { name: /jump in/i })).toHaveLength(3)
    expect(recommended.getByRole('button', { name: 'Page 1' })).toHaveAttribute('aria-current', 'true')
    fireEvent.click(recommended.getByRole('button', { name: 'Next page' }))
    expect(recommended.getByText('Place 4')).toBeInTheDocument()
    expect(recommended.queryByText('Place 1')).not.toBeInTheDocument()
    fireEvent.click(recommended.getByRole('button', { name: 'View all' }))
    expect(session.places.toggle).toHaveBeenCalledOnce()
    vi.mocked(fetchHighlighted).mockResolvedValue([])
  })

  it('closes a page card by a press on the lobby around it, for each of the four pages', async () => {
    const base = fakeSession()
    for (const page of ['settings', 'map', 'places', 'events'] as const) {
      const session = { ...base, [page]: { ...base[page], open: true } }
      const { unmount } = render(
        <SessionProvider value={session}>
          <LobbyHome onPick={session.pickDestination} setEngineViewport={session.setEngineViewport} />
        </SessionProvider>
      )
      fireEvent.click(screen.getByTestId('lobby-page-guard'))
      expect(session.closeAllPanels, page).toHaveBeenCalledOnce()
      vi.mocked(session.closeAllPanels).mockClear()
      unmount()
    }
    render(
      <SessionProvider value={base}>
        <LobbyHome onPick={base.pickDestination} setEngineViewport={base.setEngineViewport} />
      </SessionProvider>
    )
    expect(screen.queryByTestId('lobby-page-guard')).toBeNull()
  })

  it('leaves the avatar cutout outside both overlay hit regions and keeps Close clickable', async () => {
    const sheet = document.createElement('style')
    const css = readFileSync(join(__dirname, '../../features/backpack/BackpackModal.module.css'), 'utf8')
    sheet.textContent = css.replace(/\.([a-zA-Z]\w*)/g, (selector, name: string) => modalStyles[name] ? `.${modalStyles[name]}` : selector)
    document.head.append(sheet)
    vi.spyOn(HTMLElement.prototype, 'getBoundingClientRect').mockReturnValue(new DOMRect(100, 100, 1600, 850))
    vi.spyOn(HTMLElement.prototype, 'offsetWidth', 'get').mockReturnValue(1920)
    const session = fakeSession()
    session.backpack.open = true
    const view = (): React.JSX.Element => (
      <SessionProvider value={session}>
        <LobbyHome onPick={session.pickDestination} setEngineViewport={session.setEngineViewport} />
        <BackpackModal onClose={session.backpack.toggle}><div /></BackpackModal>
      </SessionProvider>
    )
    try {
      const { rerender } = render(view())
      session.avatarPreviewRect = { x: 90, y: 130, width: 600, height: 800 }
      rerender(view())
      const frame = screen.getByRole('dialog', { name: 'Backpack' })
      const dim = frame.previousElementSibling as HTMLElement
      const lobby = screen.getByRole('button', { name: 'Customize' }).parentElement!
      expect(getComputedStyle(frame.parentElement!).pointerEvents).toBe('none')
      expect(getComputedStyle(frame).pointerEvents).toBe('none')
      expect(dim.style.clipPath).toMatch(/^polygon\(evenodd,/)
      expect(lobby.style.clipPath).toMatch(/^polygon\(evenodd,/)
      fireEvent.click(dim, { clientX: 1500, clientY: 900 })
      expect(session.backpack.toggle).not.toHaveBeenCalled()
      fireEvent.click(dim, { clientX: 50, clientY: 50 })
      expect(session.backpack.toggle).toHaveBeenCalledOnce()
      const close = screen.getByRole('button', { name: 'Close' })
      expect(getComputedStyle(close).pointerEvents).toBe('auto')
      fireEvent.click(close)
      expect(session.backpack.toggle).toHaveBeenCalledTimes(2)
      await waitFor(() => expect(screen.getByRole('button', { name: /jump in/i })).toBeInTheDocument())
    } finally {
      sheet.remove()
    }
  })

  it('keeps the header names, and shows a page key after its name only where the key acts', async () => {
    setBindingsSnapshot([[{ System: 'Map' }, ['KeyM']]])
    const session = fakeSession()
    const view = (onClose?: () => void): React.JSX.Element => (
      <SessionProvider value={session}>
        <LobbyHome onPick={session.pickDestination} onClose={onClose} setEngineViewport={session.setEngineViewport} />
      </SessionProvider>
    )
    const { rerender } = render(view())
    for (const name of ['Customize lobby', 'Map', 'Places', 'Events', 'Notifications']) expect(screen.getByRole('button', { name })).toBeInTheDocument()
    // the startup lobby takes no world hotkey
    expect(screen.queryByText('[M]')).not.toBeInTheDocument()
    rerender(view(vi.fn()))
    const map = screen.getByRole('button', { name: 'Map' })
    expect(map).toHaveTextContent('Map[M]')
    expect(screen.getByText('[M]')).toHaveAttribute('aria-hidden', 'true')
    // Places has no key bound here: no hint
    expect(screen.getByRole('button', { name: 'Places' })).toHaveTextContent(/^Places$/)
    fireEvent.click(screen.getByRole('button', { name: 'Notifications' }))
    expect(session.notifications.toggle).toHaveBeenCalledOnce()
    fireEvent.click(screen.getByRole('button', { name: 'Customize lobby' }))
    expect(screen.getByRole('button', { name: 'Back [Esc]' })).toBeInTheDocument()
    await waitFor(() => expect(screen.getByRole('button', { name: /jump in/i })).toBeInTheDocument())
  })

  it('asks before the profile menu signs out: Cancel keeps the session, the confirmed Sign Out ends it', async () => {
    const session = fakeSession()
    const profile = { address: '0xb8c4df381c6c305758f806c3aa71f37aabbcbd92', name: 'Mojito', hasClaimedName: false, isGuest: false }
    render(
      <SessionProvider value={{ ...session, profile: { ...session.profile, data: profile } }}>
        <LobbyHome onPick={session.pickDestination} setEngineViewport={session.setEngineViewport} />
        <PopupHost />
      </SessionProvider>
    )
    const signOut = async (answer: string): Promise<void> => {
      fireEvent.click(screen.getByRole('button', { name: /^M ?Mojito$/ }))
      // the address in the menu is the short form
      expect(screen.getByRole('menu', { name: 'Profile' })).toHaveTextContent('0xb8c4…bd92')
      fireEvent.click(screen.getByRole('menuitem', { name: 'Sign Out' }))
      const question = within(await screen.findByRole('dialog', { name: 'Sign out?' }))
      fireEvent.click(question.getByRole('button', { name: answer }))
      await waitFor(() => expect(screen.queryByRole('dialog', { name: 'Sign out?' })).not.toBeInTheDocument())
    }
    await signOut('Cancel')
    expect(session.logout).not.toHaveBeenCalled()
    await signOut('Sign Out')
    expect(session.logout).toHaveBeenCalledOnce()
  })

  it('greets the player by name and marks a claimed name in the profile pill without renaming it', async () => {
    const session = fakeSession()
    const profile = { address: '0xb8c4df381c6c305758f806c3aa71f37aabbcbd92', name: 'Mojito', hasClaimedName: false, isGuest: false }
    const view = (hasClaimedName: boolean): React.JSX.Element => (
      <SessionProvider value={{ ...session, profile: { ...session.profile, data: { ...profile, hasClaimedName } } }}>
        <LobbyHome onPick={session.pickDestination} setEngineViewport={session.setEngineViewport} />
      </SessionProvider>
    )
    const { rerender } = render(view(false))
    expect(screen.getByRole('heading', { level: 1, name: 'Welcome Mojito' })).toBeInTheDocument()
    // the picture's initial, then the name: the badge adds nothing to it
    const pill = screen.getByRole('button', { name: /^M ?Mojito$/ })
    expect(pill).toHaveAttribute('aria-haspopup', 'menu')
    expect(pill.querySelector('[aria-label="verified"]')).toBeNull()
    rerender(view(true))
    expect(screen.getByRole('button', { name: /^M ?Mojito$/ })).toBe(pill)
    expect(pill.querySelector('[aria-hidden="true"] [aria-label="verified"]')).not.toBeNull()
    await waitFor(() => expect(screen.getByRole('button', { name: /jump in/i })).toBeInTheDocument())
  })

  it('keeps the recommended row in its own box, three places a page, with or without Jump Back In beside it', async () => {
    const places = Array.from({ length: 8 }, (_, i) => place(String(i + 1)))
    vi.mocked(fetchHighlighted).mockResolvedValue(places)
    const session = fakeSession()
    const view = (): React.JSX.Element => (
      <SessionProvider value={session}>
        <LobbyHome onPick={session.pickDestination} setEngineViewport={session.setEngineViewport} />
      </SessionProvider>
    )
    const first = render(view())
    expect(await screen.findByText('Place 3')).toBeInTheDocument()
    // the middle stays clear for the avatar: the row does not grow into it
    await waitFor(() => expect(screen.queryByText('Place 4')).not.toBeInTheDocument())
    expect(screen.queryByRole('heading', { name: 'Jump Back In' })).not.toBeInTheDocument()
    first.unmount()
    vi.mocked(fetchRecents).mockResolvedValueOnce([place('recent')])
    render(view())
    expect(await screen.findByText('Place recent')).toBeInTheDocument()
    await waitFor(() => expect(screen.queryByText('Place 4')).not.toBeInTheDocument())
    expect(screen.getByText('Place 3')).toBeInTheDocument()
    vi.mocked(fetchHighlighted).mockResolvedValue([])
  })

  const friend = (i: number, status: Friend['status'] = 'online'): Friend => ({ address: `0x${i}`, name: `Friend${i}`, status })
  const lobbyWith = (list: Friend[]): React.JSX.Element => {
    const base = fakeSession()
    const session = { ...base, friends: { ...base.friends, list } }
    return (
      <SessionProvider value={session}>
        <LobbyHome onPick={session.pickDestination} setEngineViewport={session.setEngineViewport} />
      </SessionProvider>
    )
  }

  it('shows the friends in the game four a page with their count, turns the pages both ways round, and opens a friend by their tile', async () => {
    render(lobbyWith([friend(6, 'offline'), friend(5, 'away'), friend(4), friend(3), friend(2), friend(1)]))
    expect(screen.getByText('5 Online')).toBeInTheDocument()
    const tiles = (): string[] => screen.getAllByRole('button', { name: /Friend\d/ }).map((b) => /Friend\d/.exec(b.textContent ?? '')![0])
    expect(tiles()).toEqual(['Friend1', 'Friend2', 'Friend3', 'Friend4'])
    expect(screen.getByRole('button', { name: 'Page 1' })).toHaveAttribute('aria-current', 'true')

    fireEvent.click(screen.getByRole('button', { name: 'Next page' }))
    expect(tiles()).toEqual(['Friend5'])
    expect(screen.getByRole('button', { name: 'Page 2' })).toHaveAttribute('aria-current', 'true')
    expect(screen.getByRole('button', { name: /Friend5/ })).toHaveTextContent('Away')
    fireEvent.click(screen.getByRole('button', { name: /Friend5/ }))
    expect(openPassport).toHaveBeenCalledWith('0x5')

    // the arrows wrap, and a dot still goes straight to its page
    fireEvent.click(screen.getByRole('button', { name: 'Next page' }))
    expect(tiles()).toHaveLength(4)
    fireEvent.click(screen.getByRole('button', { name: 'Previous page' }))
    expect(tiles()).toEqual(['Friend5'])
    fireEvent.click(screen.getByRole('button', { name: 'Page 1' }))
    expect(tiles()).toHaveLength(4)
    await waitFor(() => expect(screen.getByRole('button', { name: /jump in/i })).toBeInTheDocument())
  })

  it('has no pager for one page of friends', async () => {
    render(lobbyWith([friend(1), friend(2)]))
    expect(screen.getByText('2 Online')).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: 'Next page' })).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: 'Page 1' })).not.toBeInTheDocument()
    await waitFor(() => expect(screen.getByRole('button', { name: /jump in/i })).toBeInTheDocument())
  })

  it('tells a player with no friends how to make some, and one whose friends are all out of the game that none is online', async () => {
    const none = render(lobbyWith([]))
    expect(screen.getByText('Time to make some friends!')).toBeInTheDocument()
    expect(screen.queryByText('No Friends Online')).not.toBeInTheDocument()
    expect(screen.queryByText(/\d Online/)).not.toBeInTheDocument()
    await waitFor(() => expect(screen.getByRole('button', { name: /jump in/i })).toBeInTheDocument())
    none.unmount()

    render(lobbyWith([friend(1, 'offline')]))
    expect(screen.getByText('No Friends Online')).toBeInTheDocument()
    expect(screen.queryByText('Time to make some friends!')).not.toBeInTheDocument()
    expect(screen.queryByText(/\d Online/)).not.toBeInTheDocument()
    await waitFor(() => expect(screen.getByRole('button', { name: /jump in/i })).toBeInTheDocument())
  })
})

describe('lobby stage', () => {
  const root = join(__dirname, '../../../../../..')
  const page = join(root, 'library/ui/apps/hud/assets/lobby')
  const bridge = join(root, 'rendering/bevy-explorer/source/react-web/bridge-scene/images')

  it('places the podium where the design has it on a 1920x1080 stage, centred', () => {
    const r = podiumRect(1920, 1080)
    expect(r.width).toBeCloseTo(564)
    expect(r.height).toBeCloseTo(208)
    expect(r.top).toBeCloseTo(821.3)
    expect(r.left).toBeCloseTo(678)
    expect(PODIUM_IMAGE.top + PODIUM_IMAGE.height).toBeLessThan(1)
  })

  it('covers a stage of any shape with the backdrop, centred, at its own proportions', () => {
    for (const [w, h] of [[1920, 1080], [1920, 1200], [2560, 1080], [900, 1600]] as const) {
      const r = backdropRect(w, h)
      expect(r.width).toBeGreaterThanOrEqual(w)
      expect(r.height).toBeGreaterThanOrEqual(h)
      expect(r.width / r.height).toBeCloseTo(BACKDROP_ASPECT)
      expect(r.left * 2 + r.width).toBeCloseTo(w)
      expect(r.top * 2 + r.height).toBeCloseTo(h)
      // it fills one axis exactly
      expect(Math.min(r.width - w, r.height - h)).toBeCloseTo(0)
    }
  })

  it("keeps the bridge's pictures the same files as the page's, at the proportions the stage numbers state", () => {
    const backdrop = readFileSync(join(page, 'background.jpg'))
    expect(readFileSync(join(bridge, 'lobby-background.jpg')).equals(backdrop)).toBe(true)
    expect(readFileSync(join(bridge, 'lobby-podium.png')).equals(readFileSync(join(page, 'podium.png')))).toBe(true)
    // the frame header of a baseline JPEG: FF C0, length, precision, height, width
    const frame = backdrop.indexOf(Buffer.from([0xff, 0xc0, 0x00, 0x11, 0x08]))
    expect(frame).toBeGreaterThan(0)
    expect(backdrop.readUInt16BE(frame + 7) / backdrop.readUInt16BE(frame + 5)).toBeCloseTo(BACKDROP_ASPECT)
    expect(backdrop.length).toBeLessThan(400 * 1024)
  })
})
