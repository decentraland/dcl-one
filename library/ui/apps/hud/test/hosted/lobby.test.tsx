import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { act, fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { PopupHost, resetPopups } from '../../design'
import { BackpackModal } from '../../features/backpack/BackpackModal'
import modalStyles from '../../features/backpack/BackpackModal.module.css'
import { LobbyHome } from '../../features/lobby/LobbyHome'
import type { EventData } from '@explorer-core/engine/protocol'
import type { QueryStatus } from '../../features/events/eventsStore'
import { eventAt } from './eventFixtures'
import { fetchLandingPlace, fetchRecents, worldReachable, type LobbyPlace } from '@explorer-core/features/lobby/lobbyApi'
import { getPref, PREF, setPref } from '@explorer-core/lib/prefs'
import * as analytics from '@explorer-core/lib/analytics'
import { receiveProfile, resetProfileStore, setProfileRequester } from '../../features/session/profileStore'
import { SessionProvider } from '../../features/session/SessionContext'
import type { EngineSession } from '../../features/session/useEngineSession'
import { fakeSession } from '../harness'
import { dispatchCancelLayer } from '@explorer-core/lib/cancelLayers'
import { openCommunityModal } from '../../features/communities/CommunityModal'
import { profilePictureStyle } from '../../features/lobby/LobbyCards'

vi.mock('../../features/communities/CommunityModal', async (importOriginal) => ({
  ...(await importOriginal<typeof import('../../features/communities/CommunityModal')>()),
  openCommunityModal: vi.fn(() => () => undefined)
}))

vi.mock('@explorer-core/features/lobby/lobbyApi', async (importOriginal) => ({
  ...(await importOriginal<typeof import('@explorer-core/features/lobby/lobbyApi')>()),
  fetchLandingPlace: vi.fn(async () => null),
  fetchConnected: vi.fn(async () => null),
  fetchRecents: vi.fn(async () => []),
  fetchHighlighted: vi.fn(async () => []),
  fetchWhereabouts: vi.fn(async () => new Map()),
  fetchCrowd: vi.fn(async () => ({ x: 5, y: 6 })),
  worldReachable: vi.fn(async () => true)
}))

const lobby = (session: EngineSession, onClose?: () => void): React.JSX.Element => (
  <SessionProvider value={session}>
    <LobbyHome onPick={session.pickDestination} onClose={onClose} setEngineViewport={session.setEngineViewport} />
  </SessionProvider>
)

const lobbyWithPopups = (session: EngineSession): React.JSX.Element => (
  <SessionProvider value={session}>
    <LobbyHome onPick={session.pickDestination} setEngineViewport={session.setEngineViewport} />
    <PopupHost />
  </SessionProvider>
)

beforeEach(() => {
  setPref(PREF.lobbyLayout, '')
})

afterEach(() => {
  resetPopups()
  vi.restoreAllMocks()
  vi.clearAllMocks()
})


function lobbyRead(session: ReturnType<typeof fakeSession>, events: EventData[], status: QueryStatus = 'loaded'): void {
  const store = session.events.data.store
  session.events.data.store = {
    ...store,
    byId: { ...store.byId, ...Object.fromEntries(events.map((e) => [e.id, e])) },
    queries: { ...store.queries, lobby: { status, ids: events.map((e) => e.id) } }
  }
}

describe('lobby controls', () => {
  it('disables Customize until the player is ready', async () => {
    const session = { ...fakeSession(), playerReady: false }
    const { rerender } = render(lobby(session))
    expect(screen.getByRole('button', { name: 'Customize' })).toBeDisabled()
    fireEvent.click(screen.getByRole('button', { name: 'Customize' }))
    expect(session.backpack.toggle).not.toHaveBeenCalled()
    session.playerReady = true
    rerender(lobby(session))
    fireEvent.click(screen.getByRole('button', { name: 'Customize' }))
    expect(session.backpack.toggle).toHaveBeenCalledOnce()
    await waitFor(() => expect(screen.getByRole('button', { name: 'Jump In' })).toBeInTheDocument())
  })

  it("jumps to a live world event only when its server is a world's name", () => {
    const session = fakeSession()
    const now = new Date()
    lobbyRead(session, [
      eventAt('good', now, { name: 'Good Fest', x: 0, y: 0, world: true, live: true, server: 'fest.dcl.eth' }),
      eventAt('bad', now, { name: 'Bad Fest', x: 0, y: 0, world: true, live: true, server: 'https://realm.example/about' })
    ])
    render(lobby(session))
    expect(screen.queryByRole('button', { name: 'Jump in to Bad Fest' })).not.toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: 'Jump in to Good Fest' }))
    expect(session.pickDestination).toHaveBeenCalledWith({ kind: 'world', realm: 'fest.dcl.eth' })
  })

  it('loads the landing card when the native home arrives after mount', async () => {
    const session = fakeSession()
    session.homeScene = () => null
    const { rerender } = render(lobby(session))
    expect(screen.getByRole('button', { name: 'Jump In' })).toBeDisabled()
    expect(fetchLandingPlace).not.toHaveBeenCalled()
    session.homeScene = () => ({ realm: 'my-home.dcl.eth', parcel: '12,-7' })
    rerender(lobby(session))
    await waitFor(() => expect(fetchLandingPlace).toHaveBeenCalledWith({ world: 'my-home.dcl.eth', parcel: { x: 12, y: -7 } }))
    await waitFor(() => expect(screen.getByRole('button', { name: 'Jump In' })).toBeEnabled())
    // the place couldn't be read: the card stands in with the world's name
    expect(screen.getByText('my-home.dcl.eth')).toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: 'Jump In' }))
    expect(session.pickDestination).toHaveBeenCalledWith(null)
  })

  it("names a recent place's creator from the owner's profile when the place carries no name", async () => {
    resetProfileStore()
    const owner = '0xb8c4df381c6c305758f806c3aa71f37aabbcbd92'
    setProfileRequester((address) => receiveProfile(address, { address, name: 'boedo', hasClaimedName: true, isGuest: false }))
    const recent: LobbyPlace = { id: 'w', title: 'My Scene', description: '', image: '', positions: [], owner, world: true, world_name: 'tortilla.dcl.eth' }
    vi.mocked(fetchRecents).mockResolvedValueOnce([recent])
    render(lobby(fakeSession()))
    expect(await screen.findByText('boedo')).toBeInTheDocument()
    expect(screen.queryByText(owner)).not.toBeInTheDocument()
  })

  it("opens a place's detail from its card, naming the event live there, and jumps from it", async () => {
    const track = vi.spyOn(analytics, 'track')
    const recent: LobbyPlace = { id: 'p', title: 'Plaza', description: 'A square.', image: '', positions: ['3,4'], base_position: '3,4', owner: null, live: true }
    vi.mocked(fetchRecents).mockResolvedValueOnce([recent])
    const session = fakeSession()
    lobbyRead(session, [eventAt('l', new Date(), { name: 'Sunset Set', live: true, place_id: 'p' })])
    render(lobbyWithPopups(session))
    fireEvent.click(await screen.findByRole('button', { name: 'Open Plaza' }))
    expect(track).toHaveBeenCalledWith('place_card_clicked', expect.objectContaining({ place_id: 'p', from_section: 'recent', source: 'lobby' }))
    const detail = within(screen.getByRole('dialog', { name: 'Plaza' }))
    expect(detail.getByRole('tooltip')).toHaveTextContent('Sunset Set')
    fireEvent.click(detail.getByRole('button', { name: 'Jump in to Plaza' }))
    expect(session.pickDestination).toHaveBeenCalledWith({ kind: 'parcel', x: 3, y: 4 })
    expect(track).toHaveBeenCalledWith('place_jumped_in', expect.objectContaining({ place_id: 'p', from_section: 'recent' }))
    expect(screen.queryByRole('dialog')).toBeNull()
  })

  it('keeps the detail open and reports no jump when the pick is refused (a sign-in finishing)', async () => {
    const track = vi.spyOn(analytics, 'track')
    vi.mocked(fetchRecents).mockResolvedValueOnce([{ id: 'p', title: 'Plaza', description: '', image: '', positions: ['3,4'], base_position: '3,4', owner: null }])
    const session = fakeSession()
    vi.mocked(session.pickDestination).mockReturnValue(false)
    render(lobbyWithPopups(session))
    fireEvent.click(await screen.findByRole('button', { name: 'Open Plaza' }))
    fireEvent.click(within(screen.getByRole('dialog', { name: 'Plaza' })).getByRole('button', { name: 'Jump in to Plaza' }))
    expect(session.pickDestination).toHaveBeenCalledOnce()
    expect(track).not.toHaveBeenCalledWith('place_jumped_in', expect.anything())
    expect(screen.getByRole('dialog', { name: 'Plaza' })).toBeInTheDocument()
  })

  it('leaves the avatar cutout outside both overlay hit regions and keeps Close clickable', async () => {
    const sheet = document.createElement('style')
    const css = readFileSync(join(__dirname, '../../features/backpack/BackpackModal.module.css'), 'utf8')
    sheet.textContent = css.replace(/\.([a-zA-Z]\w*)/g, (selector, name: string) => (modalStyles[name] ? `.${modalStyles[name]}` : selector))
    document.head.append(sheet)
    vi.spyOn(HTMLElement.prototype, 'getBoundingClientRect').mockReturnValue(new DOMRect(100, 100, 1600, 850))
    vi.spyOn(HTMLElement.prototype, 'offsetWidth', 'get').mockReturnValue(1920)
    const session = fakeSession()
    session.backpack.open = true
    const view = (): React.JSX.Element => (
      <SessionProvider value={session}>
        <LobbyHome onPick={session.pickDestination} setEngineViewport={session.setEngineViewport} />
        <BackpackModal onClose={session.backpack.toggle}>
          <div />
        </BackpackModal>
      </SessionProvider>
    )
    try {
      const { rerender } = render(view())
      session.avatarPreviewRect = { x: 90, y: 130, width: 600, height: 800 }
      rerender(view())
      const frame = screen.getByRole('dialog', { name: 'Backpack' })
      const dim = frame.previousElementSibling as HTMLElement
      const page = screen.getByRole('button', { name: 'Customize' }).parentElement!
      expect(getComputedStyle(frame.parentElement!).pointerEvents).toBe('none')
      expect(getComputedStyle(frame).pointerEvents).toBe('none')
      expect(dim.style.clipPath).toMatch(/^polygon\(evenodd,/)
      expect(page.style.clipPath).toMatch(/^polygon\(evenodd,/)
      fireEvent.click(dim, { clientX: 1500, clientY: 900 })
      expect(session.backpack.toggle).not.toHaveBeenCalled()
      fireEvent.click(dim, { clientX: 50, clientY: 50 })
      expect(session.backpack.toggle).toHaveBeenCalledOnce()
      const close = within(frame.parentElement!).getByRole('button', { name: 'Close' })
      expect(getComputedStyle(close).pointerEvents).toBe('auto')
      fireEvent.click(close)
      expect(session.backpack.toggle).toHaveBeenCalledTimes(2)
    } finally {
      sheet.remove()
    }
  })
})

describe('lobby widgets', () => {
  it('tells nobody online apart from no friends at all', async () => {
    const session = fakeSession()
    session.friends.loading = false
    session.friends.list = []
    const { rerender } = render(lobby(session))
    expect(await screen.findByText('Time to make some friends!')).toBeInTheDocument()
    session.friends.list = [{ address: '0xf1', name: 'Mojito', status: 'offline' }]
    rerender(lobby(session))
    expect(await screen.findByText('No Friends Online')).toBeInTheDocument()
    session.friends.list = [{ address: '0xf1', name: 'Mojito', status: 'online' }]
    rerender(lobby(session))
    expect(await screen.findByText('1 Online')).toBeInTheDocument()
    expect(screen.getByText('Locating…')).toBeInTheDocument()
  })

  it('lists live and upcoming events and marks one interested', async () => {
    const session = fakeSession()
    const soon = new Date(Date.now() + 40 * 60_000)
    lobbyRead(session, [
      eventAt('l', soon, { name: 'Live Party', x: 1, y: 2, live: true, connected_addresses: ['0xa'] }),
      eventAt('u', soon, { name: 'Tower Smash', x: 3, y: 4 })
    ])
    render(lobby(session))
    expect(screen.getByText('Live Party')).toBeInTheDocument()
    expect(screen.getByText('Tower Smash')).toBeInTheDocument()
    expect(screen.getByText('In 40 min')).toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: 'Interested' }))
    expect(session.events.data.setAttending).toHaveBeenCalledWith(expect.objectContaining({ id: 'u' }), true)
    fireEvent.click(screen.getByRole('button', { name: 'Jump in to Live Party' }))
    expect(session.pickDestination).toHaveBeenCalledWith({ kind: 'parcel', x: 1, y: 2 })
  })

  it("opens an event's detail from its card and jumps from there through the lobby's pick", () => {
    const track = vi.spyOn(analytics, 'track')
    const session = fakeSession()
    lobbyRead(session, [eventAt('l', new Date(), { name: 'Live Party', x: 1, y: 2, live: true })])
    render(lobbyWithPopups(session))
    fireEvent.click(screen.getByRole('button', { name: 'Open Live Party' }))
    expect(track).toHaveBeenCalledWith('event_card_clicked', expect.objectContaining({ event_id: 'l', source: 'lobby', from_section: 'live_events' }))
    const detail = screen.getByRole('dialog', { name: 'Live Party' })
    fireEvent.click(within(detail).getByRole('button', { name: /^Jump in to/ }))
    expect(session.pickDestination).toHaveBeenCalledWith({ kind: 'parcel', x: 1, y: 2 })
    expect(track).toHaveBeenCalledWith('event_jumped_in', expect.objectContaining({ event_id: 'l', source: 'lobby' }))
    expect(screen.queryByRole('dialog')).toBeNull()
  })

  it("keeps an upcoming event's detail open when its world cannot be reached", () => {
    const session = fakeSession()
    lobbyRead(session, [eventAt('u', new Date(Date.now() + 3_600_000), { name: 'Bad Fest', world: true, server: 'https://realm.example/about' })])
    render(lobbyWithPopups(session))
    fireEvent.click(screen.getByRole('button', { name: 'Open Bad Fest' }))
    fireEvent.click(within(screen.getByRole('dialog', { name: 'Bad Fest' })).getByRole('button', { name: /^Jump in to/ }))
    expect(session.pickDestination).not.toHaveBeenCalled()
    expect(screen.getByRole('dialog', { name: 'Bad Fest' })).toBeInTheDocument()
  })

  it('clears the live events when the read fails and keeps the upcoming ones', () => {
    const session = fakeSession()
    lobbyRead(
      session,
      [eventAt('l', new Date(), { name: 'Live Party', live: true }), eventAt('u', new Date(Date.now() + 3_600_000), { name: 'Tower Smash' })],
      'failed'
    )
    render(lobby(session))
    expect(screen.queryByText('Live Party')).not.toBeInTheDocument()
    expect(screen.getByText('Tower Smash')).toBeInTheDocument()
  })

  it("opens Events in the lobby's section modal from VIEW ALL", () => {
    const track = vi.spyOn(analytics, 'track')
    const session = fakeSession()
    lobbyRead(session, [eventAt('u', new Date(Date.now() + 3_600_000), { name: 'Tower Smash' })])
    render(lobbyWithPopups(session))
    const upcoming = screen.getByRole('heading', { name: 'Upcoming Events' }).parentElement!
    fireEvent.click(within(upcoming).getByRole('button', { name: /view all/i }))
    expect(screen.getByRole('dialog', { name: 'Events' })).toBeInTheDocument()
    expect(session.events.toggle).not.toHaveBeenCalled()
    expect(session.events.data.loadCalendar).toHaveBeenCalledWith(expect.any(Date), 4, true)
    expect(track).toHaveBeenCalledWith('events_section_opened', expect.objectContaining({ source: 'lobby' }))
  })

  it('opens one section at a time from the top bar, and closes it', () => {
    const session = fakeSession()
    render(lobbyWithPopups(session))
    const bar = within(screen.getByRole('navigation', { name: 'Lobby' }))
    fireEvent.click(bar.getByRole('button', { name: 'Map' }))
    expect(screen.getByRole('dialog', { name: 'Map' })).toBeInTheDocument()
    expect(session.map.toggle).not.toHaveBeenCalled()
    fireEvent.click(bar.getByRole('button', { name: 'Events' }))
    expect(screen.getAllByRole('dialog')).toHaveLength(1)
    fireEvent.click(screen.getByRole('button', { name: 'Close Map' }))
    expect(screen.queryByRole('dialog')).toBeNull()
    fireEvent.click(bar.getByRole('button', { name: 'Events' }))
    expect(screen.getByRole('dialog', { name: 'Events' })).toBeInTheDocument()
  })

  it('hands over to the full page when it opens under the modal', () => {
    const session = fakeSession()
    const { rerender } = render(lobbyWithPopups(session))
    fireEvent.click(within(screen.getByRole('navigation', { name: 'Lobby' })).getByRole('button', { name: 'Events' }))
    expect(screen.getByRole('dialog', { name: 'Events' })).toBeInTheDocument()
    session.events.open = true
    rerender(lobbyWithPopups({ ...session }))
    expect(screen.queryByRole('dialog', { name: 'Events' })).toBeNull()
    expect(session.events.data.clear).not.toHaveBeenCalled()
  })
})

describe('lobby go to', () => {
  const goTo = (text: string): void => {
    fireEvent.change(screen.getByRole('textbox', { name: 'Go to a coordinate or a World' }), { target: { value: text } })
    fireEvent.keyDown(screen.getByRole('textbox', { name: 'Go to a coordinate or a World' }), { key: 'Enter' })
  }
  const withGoTo = (): EngineSession => {
    setPref(PREF.lobbyLayout, '{"Version":1,"Entries":[{"Corner":"TopLeft","Widget":"GoTo4By1","X":0,"Y":0}]}')
    return fakeSession()
  }

  it('flags a destination that is no place', async () => {
    render(lobby(withGoTo()))
    goTo('1000,-500')
    expect(await screen.findByRole('alert')).toHaveTextContent("Coordinates or World name doesn't exist")
  })

  it('goes to a parcel, and to a world once it answers', async () => {
    const session = withGoTo()
    render(lobby(session))
    goTo('3,-5')
    await waitFor(() => expect(session.pickDestination).toHaveBeenCalledWith({ kind: 'parcel', x: 3, y: -5 }))
    goTo('mana')
    await waitFor(() => expect(session.pickDestination).toHaveBeenCalledWith({ kind: 'world', realm: 'mana.dcl.eth', position: undefined }))
    vi.mocked(worldReachable).mockResolvedValueOnce(false)
    goTo('ghost')
    expect(await screen.findByRole('alert')).toBeInTheDocument()
  })
})

describe('lobby layout editor', () => {
  it('drops the changes on Cancel (the key or a gamepad button) and stores them on Save', async () => {
    render(lobby(fakeSession()))
    expect(screen.getByRole('heading', { name: /welcome/i })).toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: 'Customize lobby layout' }))
    const modal = screen.getByRole('dialog', { name: 'Customize Lobby' })
    expect(within(modal).getByRole('button', { name: 'SAVE' })).toBeDisabled()
    fireEvent.click(screen.getByRole('button', { name: 'Remove Jump In Home' }))
    expect(within(modal).getByRole('button', { name: 'SAVE' })).toBeEnabled()
    act(() => {
      expect(dispatchCancelLayer()).toBe(true)
    })
    expect(screen.queryByRole('dialog', { name: 'Customize Lobby' })).not.toBeInTheDocument()
    expect(dispatchCancelLayer()).toBe(false)
    expect(screen.getByRole('heading', { name: /welcome/i })).toBeInTheDocument()

    fireEvent.click(screen.getByRole('button', { name: 'Customize lobby layout' }))
    fireEvent.click(screen.getByRole('button', { name: 'Remove Jump In Home' }))
    fireEvent.click(screen.getByRole('button', { name: 'SAVE' }))
    expect(screen.queryByRole('heading', { name: /welcome/i })).not.toBeInTheDocument()
    expect(getPref(PREF.lobbyLayout)).not.toContain('Landing4By3')
  })

  it("fills a friend's picture with their profile colour, framed a fifth of the way to white", async () => {
    const session = fakeSession()
    session.friends.loading = false
    session.friends.list = [{ address: '0xf1', name: 'Mojito', status: 'online', claimed: true, nameColor: { r: 1, g: 0, b: 0 } }]
    render(lobby(session))
    const card = (await screen.findByRole('button', { name: "Mojito's profile" })).parentElement as HTMLElement
    const picture = card.querySelector('[style*="background-image"]') as HTMLElement
    expect(picture.style.backgroundColor).toBe('rgb(255, 0, 0)')
  })

  it('colours the pictures of the friends at a place the same way', () => {
    const style = profilePictureStyle('rgb(1, 2, 3)', null)
    expect(style.backgroundColor).toBe('rgb(1, 2, 3)')
    expect(style.borderColor).toBe('color-mix(in srgb, rgb(1, 2, 3) 80%, white)')
    expect(String(style.backgroundImage)).toMatch(/^url\(/)
  })

  it('opens a community from anywhere on its card', () => {
    setPref(PREF.lobbyLayout, JSON.stringify({ Version: 1, Entries: [{ Corner: 'TopRight', Widget: 'Communities4By2', X: 0, Y: 0 }] }))
    const session = fakeSession()
    session.communities.list = [{ id: 'c1', name: 'Builders', description: '', membersCount: 3, role: 'member', ownerName: 'o' }]
    render(lobby(session))
    fireEvent.click(screen.getByRole('button', { name: 'Builders' }))
    expect(vi.mocked(openCommunityModal)).toHaveBeenCalledWith('c1', expect.any(Function))
    fireEvent.click(screen.getByRole('button', { name: 'View Builders' }))
    expect(vi.mocked(openCommunityModal)).toHaveBeenCalledTimes(2)
  })

  it('lists only the widgets off the grids and places one on a click', () => {
    render(lobby(fakeSession()))
    fireEvent.click(screen.getByRole('button', { name: 'Customize lobby layout' }))
    const modal = screen.getByRole('dialog', { name: 'Customize Lobby' })
    expect(within(modal).getByRole('button', { name: 'Add My Communities' })).toBeInTheDocument()
    expect(within(modal).queryByRole('button', { name: 'Add Friends' })).not.toBeInTheDocument()
    fireEvent.click(within(modal).getByRole('button', { name: 'Add Go to' }))
    expect(within(modal).queryByRole('button', { name: 'Add Go to' })).not.toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'Remove Go to' })).toBeInTheDocument()
  })

  it('takes a widget dragged back onto the modal off the grid', () => {
    render(lobby(fakeSession()))
    fireEvent.click(screen.getByRole('button', { name: 'Customize lobby layout' }))
    const modal = screen.getByRole('dialog', { name: 'Customize Lobby' })
    expect(within(modal).queryByRole('button', { name: 'Add Friends' })).not.toBeInTheDocument()
    // jsdom lays nothing out: the modal's box is the point (0, 0), where the drop lands
    const friends = screen.getByRole('group', { name: 'Friends widget' })
    fireEvent.pointerDown(friends, { pointerId: 1, button: 0, clientX: 0, clientY: 0 })
    act(() => {
      window.dispatchEvent(Object.assign(new Event('pointermove'), { pointerId: 1, clientX: 0, clientY: 0 }))
      window.dispatchEvent(Object.assign(new Event('pointerup'), { pointerId: 1, clientX: 0, clientY: 0 }))
    })
    expect(screen.queryByRole('group', { name: 'Friends widget' })).not.toBeInTheDocument()
    expect(within(modal).getByRole('button', { name: 'Add Friends' })).toBeInTheDocument()
  })
})

describe('lobby profile menu and visit', () => {
  it('signs out from the menu and offers no Exit on the web', () => {
    const session = fakeSession()
    render(lobby(session))
    fireEvent.click(screen.getByRole('button', { name: /guest|test/i, expanded: false }))
    const menu = screen.getByRole('menu', { name: 'Profile menu' })
    expect(within(menu).queryByRole('menuitem', { name: 'Exit' })).not.toBeInTheDocument()
    fireEvent.click(within(menu).getByRole('menuitem', { name: 'Sign Out' }))
    expect(session.logout).toHaveBeenCalledOnce()
    expect(screen.queryByRole('menu')).not.toBeInTheDocument()
  })

  it('closes the menu on Cancel (the key or a gamepad button) and leaves the lobby up', () => {
    render(lobby(fakeSession()))
    fireEvent.click(screen.getByRole('button', { name: /guest|test/i, expanded: false }))
    expect(screen.getByRole('menu', { name: 'Profile menu' })).toBeInTheDocument()
    act(() => {
      expect(dispatchCancelLayer()).toBe(true)
    })
    expect(screen.queryByRole('menu')).not.toBeInTheDocument()
    expect(dispatchCancelLayer()).toBe(false)
  })

  it("fills the top bar's picture with the player's name colour", () => {
    const session = fakeSession()
    session.profile.data = { address: '0xme', name: 'Tester', hasClaimedName: true, nameColor: { r: 0, g: 1, b: 0 }, isGuest: false }
    render(lobby(session))
    const button = screen.getByRole('button', { name: /tester/i, expanded: false })
    const picture = button.querySelector('[style*="background-image"]') as HTMLElement
    expect(picture.style.backgroundColor).toBe('rgb(0, 255, 0)')
  })

  it('reports the visit opened and closed, and idles the avatar while up', () => {
    const track = vi.spyOn(analytics, 'track')
    const session = fakeSession()
    const { unmount } = render(lobby(session, vi.fn()))
    expect(track).toHaveBeenCalledWith('lobby_opened', { source: 'lobby', is_startup: false })
    expect(session.lobbyStage.set).toHaveBeenCalledWith({ idleEmotes: true })
    expect(session.lobbyData.warm).toHaveBeenCalled()
    unmount()
    expect(track).toHaveBeenCalledWith('lobby_closed', expect.objectContaining({ source: 'lobby', is_startup: false }))
    expect(session.lobbyStage.set).toHaveBeenCalledWith({ idleEmotes: false })
  })
})
