import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { act, fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { PopupHost, ToastHost, resetPopups } from '../../design'
import { BackpackModal } from '../../features/backpack/BackpackModal'
import modalStyles from '../../features/backpack/BackpackModal.module.css'
import { LobbyHome } from '../../features/lobby/LobbyHome'
import type { EventData } from '@explorer-core/engine/protocol'
import type { QueryStatus } from '../../features/events/eventsStore'
import { eventAt } from './eventFixtures'
import { fetchCrowd, fetchLandingPlace, fetchRecents, fetchWhereabouts, worldReachable, type FriendWhere, type LobbyPlace } from '@explorer-core/features/lobby/lobbyApi'
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
  worldReachable: vi.fn(async () => 'ok' as const),
  // a friend's location title would otherwise read the live places service
  whereTitle: vi.fn(async (where: FriendWhere) => (where.kind === 'genesis' ? `${where.parcel.x},${where.parcel.y}` : where.kind === 'world' ? where.world : 'Lobby'))
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

  it("drops the old home's card when the home changes, even when the new one cannot be read", async () => {
    const session = fakeSession()
    session.homeScene = () => ({ realm: null, parcel: '1,1' })
    const old: LobbyPlace = { id: 'old', title: 'Old Home', description: '', image: '', positions: ['1,1'], owner: '', world: false }
    vi.mocked(fetchLandingPlace).mockResolvedValueOnce(old).mockRejectedValueOnce(new Error('down'))
    const { rerender } = render(lobby(session))
    expect(await screen.findByText('Old Home')).toBeInTheDocument()
    session.homeScene = () => ({ realm: 'new-home.dcl.eth', parcel: '0,0' })
    rerender(lobby(session))
    expect(screen.queryByText('Old Home')).toBeNull()
    expect(await screen.findByText('new-home.dcl.eth')).toBeInTheDocument()
    expect(screen.queryByText('Old Home')).toBeNull()
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

  it("says so when JOIN finds the friend's location gone", async () => {
    const session = fakeSession()
    session.friends.loading = false
    session.friends.list = [{ address: '0xf1', name: 'Mojito', status: 'online' }]
    vi.mocked(fetchWhereabouts)
      .mockResolvedValueOnce(new Map([['0xf1', { kind: 'genesis', parcel: { x: 4, y: 5 } }]]))
      .mockResolvedValueOnce(new Map([['0xf1', { kind: 'unknown' }]]))
    render(
      <>
        {lobby(session)}
        <ToastHost />
      </>
    )
    fireEvent.click(await screen.findByRole('button', { name: 'Join Mojito' }))
    expect(await screen.findByText('Your friend’s location could not be found. Please try again.')).toBeInTheDocument()
    expect(session.pickDestination).not.toHaveBeenCalled()
  })

  it("shows a friend's location from a read slower than the 15 s re-check, and starts no second read", async () => {
    vi.useFakeTimers({ toFake: ['setInterval', 'clearInterval'] })
    try {
      const session = fakeSession()
      session.friends.loading = false
      session.friends.list = [{ address: '0xf1', name: 'Mojito', status: 'online' }]
      let answer: (found: Map<string, { kind: 'genesis'; parcel: { x: number; y: number } }>) => void = () => undefined
      vi.mocked(fetchWhereabouts).mockReturnValueOnce(new Promise((resolve) => (answer = resolve)))
      render(lobby(session))
      expect(fetchWhereabouts).toHaveBeenCalledOnce()
      act(() => vi.advanceTimersByTime(31_000))
      expect(fetchWhereabouts).toHaveBeenCalledOnce()
      await act(async () => answer(new Map([['0xf1', { kind: 'genesis', parcel: { x: 4, y: 5 } }]])))
      expect(await screen.findByRole('button', { name: 'Join Mojito' })).toBeInTheDocument()
      expect(screen.queryByText('Locating…')).toBeNull()
    } finally {
      vi.useRealTimers()
    }
  })

  it("disables a friend's JOIN while sign-in finishes", async () => {
    const session = fakeSession()
    session.friends.loading = false
    session.friends.list = [{ address: '0xf1', name: 'Mojito', status: 'online' }]
    session.login = { ...session.login, busy: true }
    vi.mocked(fetchWhereabouts).mockResolvedValue(new Map([['0xf1', { kind: 'genesis', parcel: { x: 4, y: 5 } }]]))
    render(lobby(session))
    expect(await screen.findByRole('button', { name: 'Join Mojito' })).toBeDisabled()
  })

  it("does not travel when a friend's location answers after a section opened over the lobby", async () => {
    const session = fakeSession()
    session.friends.loading = false
    session.friends.list = [{ address: '0xf1', name: 'Mojito', status: 'online' }]
    let answer: (found: Map<string, { kind: 'genesis'; parcel: { x: number; y: number } }>) => void = () => undefined
    vi.mocked(fetchWhereabouts)
      .mockResolvedValueOnce(new Map([['0xf1', { kind: 'genesis', parcel: { x: 4, y: 5 } }]]))
      .mockReturnValueOnce(new Promise((resolve) => (answer = resolve)))
    render(lobbyWithPopups(session))
    fireEvent.click(await screen.findByRole('button', { name: 'Join Mojito' }))
    await waitFor(() => expect(fetchWhereabouts).toHaveBeenCalledTimes(2))
    fireEvent.click(within(screen.getByRole('navigation', { name: 'Lobby' })).getByRole('button', { name: 'Map' }))
    await act(async () => answer(new Map([['0xf1', { kind: 'genesis', parcel: { x: 4, y: 5 } }]])))
    expect(session.pickDestination).not.toHaveBeenCalled()
    expect(screen.getByRole('dialog', { name: 'Map' })).toBeInTheDocument()
  })

  it("does not travel when a friend's location answers after a page opened over the lobby from elsewhere", async () => {
    const session = fakeSession()
    session.friends.loading = false
    session.friends.list = [{ address: '0xf1', name: 'Mojito', status: 'online' }]
    let answer: (found: Map<string, { kind: 'genesis'; parcel: { x: number; y: number } }>) => void = () => undefined
    vi.mocked(fetchWhereabouts)
      .mockResolvedValueOnce(new Map([['0xf1', { kind: 'genesis', parcel: { x: 4, y: 5 } }]]))
      .mockReturnValueOnce(new Promise((resolve) => (answer = resolve)))
    const { rerender } = render(lobby(session, () => undefined))
    fireEvent.click(await screen.findByRole('button', { name: 'Join Mojito' }))
    await waitFor(() => expect(fetchWhereabouts).toHaveBeenCalledTimes(2))
    rerender(lobby({ ...session, lobbyData: { ...session.lobbyData, covered: true } }, () => undefined))
    await act(async () => answer(new Map([['0xf1', { kind: 'genesis', parcel: { x: 4, y: 5 } }]])))
    expect(session.pickDestination).not.toHaveBeenCalled()
  })

  it('travels to a friend from a lobby that opened under a page already', async () => {
    const session = fakeSession()
    session.friends.loading = false
    session.friends.list = [{ address: '0xf1', name: 'Mojito', status: 'online' }]
    session.lobbyData = { ...session.lobbyData, covered: true }
    vi.mocked(fetchWhereabouts).mockResolvedValue(new Map([['0xf1', { kind: 'genesis', parcel: { x: 4, y: 5 } }]]))
    render(lobby(session, () => undefined))
    fireEvent.click(await screen.findByRole('button', { name: 'Join Mojito' }))
    await waitFor(() => expect(session.pickDestination).toHaveBeenCalledWith({ kind: 'parcel', x: 4, y: 5 }))
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

  it("keeps an event's detail open and reports no jump when the pick is refused", () => {
    const track = vi.spyOn(analytics, 'track')
    const session = fakeSession()
    vi.mocked(session.pickDestination).mockReturnValue(false)
    lobbyRead(session, [eventAt('l', new Date(), { name: 'Live Party', x: 1, y: 2, live: true })])
    render(lobbyWithPopups(session))
    fireEvent.click(screen.getByRole('button', { name: 'Open Live Party' }))
    fireEvent.click(within(screen.getByRole('dialog', { name: 'Live Party' })).getByRole('button', { name: /^Jump in to/ }))
    expect(session.pickDestination).toHaveBeenCalledOnce()
    expect(track).not.toHaveBeenCalledWith('event_jumped_in', expect.anything())
    expect(screen.getByRole('dialog', { name: 'Live Party' })).toBeInTheDocument()
  })

  it("closes an event's share menu on Cancel and leaves the lobby up", () => {
    const session = fakeSession()
    lobbyRead(session, [eventAt('u', new Date(Date.now() + 3_600_000), { name: 'Tower Smash', x: 3, y: 4 })])
    render(lobby(session))
    fireEvent.click(screen.getByRole('button', { name: 'Share' }))
    expect(screen.getByRole('menuitem', { name: 'Copy link' })).toBeInTheDocument()
    act(() => {
      expect(dispatchCancelLayer()).toBe(true)
    })
    expect(screen.queryByRole('menuitem', { name: 'Copy link' })).toBeNull()
    expect(dispatchCancelLayer()).toBe(false)
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

describe('lobby VIEW ALL of Communities and Gallery', () => {
  const photos = [
    { id: 'p1', url: 'https://img/p1.jpg', thumbnailUrl: 'https://img/p1_t.jpg', dateTime: '1707000000' },
    { id: 'p2', url: 'https://img/p2.jpg', thumbnailUrl: 'https://img/p2_t.jpg', dateTime: '1707000000' }
  ]
  const withWidgets = (): ReturnType<typeof fakeSession> => {
    setPref(
      PREF.lobbyLayout,
      JSON.stringify({
        Version: 1,
        Entries: [
          { Corner: 'TopLeft', Widget: 'Communities4By2', X: 0, Y: 0 },
          { Corner: 'TopRight', Widget: 'Gallery4By2', X: 0, Y: 0 }
        ]
      })
    )
    const session = fakeSession()
    session.communities.browse = { ...session.communities.browse, items: [{ id: 'c2', name: 'Painters', description: '', membersCount: 9, role: 'none', ownerName: 'o' }], total: 1 }
    session.gallery.fetchPage = vi.fn(async (offset: number, limit: number) => ({ ok: true, photos: photos.slice(offset, offset + limit), total: photos.length, max: 500 }))
    session.gallery.metas = { p1: { sceneName: 'Genesis Plaza', x: -9, y: 14 } }
    return session
  }
  const viewAll = (title: string): void => {
    const header = screen.getByRole('heading', { name: title }).parentElement!
    fireEvent.click(within(header).getByRole('button', { name: /view all/i }))
  }
  const openFirstPhoto = async (): Promise<void> => {
    const modal = screen.getByRole('dialog', { name: 'Gallery' })
    const [first] = await within(modal).findAllByRole('button', { name: 'Open photo' })
    fireEvent.load(first.querySelector('img')!)
    fireEvent.click(first)
  }

  it("opens Communities compact in the lobby's section modal", () => {
    const track = vi.spyOn(analytics, 'track')
    const session = withWidgets()
    render(lobbyWithPopups(session))
    viewAll('My Communities')
    const modal = screen.getByRole('dialog', { name: 'Communities' })
    expect(session.communities.toggle).not.toHaveBeenCalled()
    expect(session.communities.loadBrowse).toHaveBeenCalledWith('')
    expect(track).not.toHaveBeenCalledWith(expect.stringMatching(/_section_opened$/), expect.anything())
    fireEvent.click(within(modal).getByText('Painters'))
    expect(vi.mocked(openCommunityModal)).toHaveBeenCalledWith('c2', expect.any(Function))
    fireEvent.click(screen.getByRole('button', { name: 'Close Communities' }))
    expect(screen.queryByRole('dialog', { name: 'Communities' })).toBeNull()
  })

  it('opens Gallery compact, with a photo over the whole screen above it', async () => {
    const track = vi.spyOn(analytics, 'track')
    const session = withWidgets()
    render(lobbyWithPopups(session))
    viewAll('Gallery')
    expect(screen.getByRole('dialog', { name: 'Gallery' })).toBeInTheDocument()
    expect(session.gallery.toggle).not.toHaveBeenCalled()
    expect(track).not.toHaveBeenCalledWith(expect.stringMatching(/_section_opened$/), expect.anything())
    await openFirstPhoto()
    const photo = screen.getByRole('dialog', { name: 'Photo' })
    expect(screen.getByRole('dialog', { name: 'Gallery' })).not.toContainElement(photo)
    act(() => {
      expect(dispatchCancelLayer()).toBe(true)
    })
    expect(screen.queryByRole('dialog', { name: 'Photo' })).toBeNull()
    expect(screen.getByRole('dialog', { name: 'Gallery' })).toBeInTheDocument()
  })

  it("jumps from a Gallery photo through the lobby's travel, closing the photo and the modal", async () => {
    const session = withWidgets()
    render(lobbyWithPopups(session))
    viewAll('Gallery')
    await openFirstPhoto()
    fireEvent.click(screen.getByRole('button', { name: /Jump In/i }))
    expect(session.map.teleport).toHaveBeenCalledWith(-9, 14)
    expect(screen.queryByRole('dialog', { name: 'Photo' })).toBeNull()
    expect(screen.queryByRole('dialog', { name: 'Gallery' })).toBeNull()
  })

  it('keeps the photo and the modal when the jump is refused', async () => {
    const session = withWidgets()
    vi.mocked(session.map.teleport).mockReturnValue('refused')
    render(lobbyWithPopups(session))
    viewAll('Gallery')
    await openFirstPhoto()
    fireEvent.click(screen.getByRole('button', { name: /Jump In/i }))
    expect(screen.getByRole('dialog', { name: 'Photo' })).toBeInTheDocument()
    expect(screen.getByRole('dialog', { name: 'Gallery' })).toBeInTheDocument()
  })

  it('hands over to the full Communities or Gallery page when it opens under the modal', async () => {
    const session = withWidgets()
    const { rerender } = render(lobbyWithPopups(session))
    viewAll('Gallery')
    await openFirstPhoto()
    session.gallery.open = true
    rerender(lobbyWithPopups({ ...session }))
    expect(screen.queryByRole('dialog', { name: 'Gallery' })).toBeNull()
    expect(screen.queryByRole('dialog', { name: 'Photo' })).toBeNull()
    session.gallery.open = false
    rerender(lobbyWithPopups({ ...session }))
    viewAll('My Communities')
    session.communities.open = true
    rerender(lobbyWithPopups({ ...session }))
    expect(screen.queryByRole('dialog', { name: 'Communities' })).toBeNull()
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
    vi.mocked(worldReachable).mockResolvedValueOnce('not-found')
    goTo('ghost')
    expect(await screen.findByRole('alert')).toBeInTheDocument()
  })

  it('goes to a world named in capitals, lowercased', async () => {
    const session = withGoTo()
    render(lobby(session))
    goTo('Mana.DCL.ETH')
    await waitFor(() => expect(session.pickDestination).toHaveBeenCalledWith({ kind: 'world', realm: 'mana.dcl.eth', position: undefined }))
  })

  it('does not flag a world the worlds server cannot check, and says so', async () => {
    const session = withGoTo()
    vi.mocked(worldReachable).mockResolvedValueOnce('unreachable')
    render(
      <>
        {lobby(session)}
        <ToastHost />
      </>
    )
    goTo('mana')
    expect(await screen.findByText('The world "mana.dcl.eth" isn\'t reachable right now.')).toBeInTheDocument()
    expect(screen.queryByRole('alert')).toBeNull()
    expect(screen.getByRole('textbox', { name: 'Go to a coordinate or a World' })).toHaveValue('mana')
    expect(session.pickDestination).not.toHaveBeenCalled()
  })

  it('keeps the text, unflagged, when the pick is refused', async () => {
    const session = withGoTo()
    vi.mocked(session.pickDestination).mockReturnValue(false)
    render(lobby(session))
    goTo('3,-5')
    await waitFor(() => expect(session.pickDestination).toHaveBeenCalledOnce())
    expect(screen.getByRole('textbox', { name: 'Go to a coordinate or a World' })).toHaveValue('3,-5')
    expect(screen.queryByRole('alert')).toBeNull()
  })

  it('says nothing of a crowd lookup that fails after a section opened over the lobby', async () => {
    const session = withGoTo()
    let fail: (e: Error) => void = () => undefined
    vi.mocked(fetchCrowd).mockReturnValueOnce(new Promise((_, reject) => (fail = reject)))
    render(
      <>
        {lobbyWithPopups(session)}
        <ToastHost />
      </>
    )
    goTo('crowd')
    await waitFor(() => expect(fetchCrowd).toHaveBeenCalledOnce())
    fireEvent.click(within(screen.getByRole('navigation', { name: 'Lobby' })).getByRole('button', { name: 'Map' }))
    await act(async () => fail(new Error('down')))
    expect(screen.queryByText('The most crowded place could not be found. Please try again.')).toBeNull()
    expect(session.pickDestination).not.toHaveBeenCalled()
  })

  it('does not travel when a world answers after a page opened over the lobby from elsewhere', async () => {
    const session = withGoTo()
    let answer: (r: 'ok') => void = () => undefined
    vi.mocked(worldReachable).mockReturnValueOnce(new Promise((resolve) => (answer = resolve)))
    const { rerender } = render(lobby(session, () => undefined))
    goTo('foo')
    await waitFor(() => expect(worldReachable).toHaveBeenCalledOnce())
    rerender(lobby({ ...session, lobbyData: { ...session.lobbyData, covered: true } }, () => undefined))
    await act(async () => answer('ok'))
    expect(session.pickDestination).not.toHaveBeenCalled()
  })

  it('says sign-in is finishing when a world answers after it started and the pick is refused', async () => {
    const session = withGoTo()
    vi.mocked(session.pickDestination).mockReturnValue(false)
    let answer: (r: 'ok') => void = () => undefined
    vi.mocked(worldReachable).mockReturnValueOnce(new Promise((resolve) => (answer = resolve)))
    const view = (s: EngineSession): React.JSX.Element => (
      <>
        {lobby(s)}
        <ToastHost />
      </>
    )
    const { rerender } = render(view(session))
    goTo('foo')
    await waitFor(() => expect(worldReachable).toHaveBeenCalledOnce())
    rerender(view({ ...session, login: { ...session.login, busy: true } }))
    await act(async () => answer('ok'))
    expect(session.pickDestination).toHaveBeenCalledOnce()
    expect(await screen.findByText('Still signing in. Try again in a moment.')).toBeInTheDocument()
  })

  it('does not travel when the crowd answers after another pick', async () => {
    setPref(PREF.lobbyLayout, '{"Version":1,"Entries":[{"Corner":"TopLeft","Widget":"GoTo4By1","X":0,"Y":0},{"Corner":"TopRight","Widget":"LiveEvents3By3","X":0,"Y":0}]}')
    const session = fakeSession()
    lobbyRead(session, [eventAt('l', new Date(), { name: 'Live Party', x: 1, y: 2, live: true })])
    let answer: (p: { x: number; y: number }) => void = () => undefined
    vi.mocked(fetchCrowd).mockReturnValueOnce(new Promise((resolve) => (answer = resolve)))
    render(lobby(session))
    goTo('crowd')
    await waitFor(() => expect(fetchCrowd).toHaveBeenCalledOnce())
    fireEvent.click(screen.getByRole('button', { name: 'Jump in to Live Party' }))
    await act(async () => answer({ x: 9, y: 9 }))
    expect(session.pickDestination).toHaveBeenCalledOnce()
    expect(session.pickDestination).toHaveBeenCalledWith({ kind: 'parcel', x: 1, y: 2 })
  })

  it('travels to the friend when JOIN follows a crowd lookup that answers first', async () => {
    setPref(PREF.lobbyLayout, '{"Version":1,"Entries":[{"Corner":"TopLeft","Widget":"GoTo4By1","X":0,"Y":0},{"Corner":"TopRight","Widget":"FriendsOnline4By2","X":0,"Y":0}]}')
    const session = fakeSession()
    session.friends.loading = false
    session.friends.list = [{ address: '0xf1', name: 'Mojito', status: 'online' }]
    let crowd: (p: { x: number; y: number }) => void = () => undefined
    let join: (found: Map<string, { kind: 'genesis'; parcel: { x: number; y: number } }>) => void = () => undefined
    vi.mocked(fetchCrowd).mockReturnValueOnce(new Promise((resolve) => (crowd = resolve)))
    vi.mocked(fetchWhereabouts)
      .mockResolvedValueOnce(new Map([['0xf1', { kind: 'genesis', parcel: { x: 4, y: 5 } }]]))
      .mockReturnValueOnce(new Promise((resolve) => (join = resolve)))
    render(lobby(session))
    goTo('crowd')
    await waitFor(() => expect(fetchCrowd).toHaveBeenCalledOnce())
    fireEvent.click(await screen.findByRole('button', { name: 'Join Mojito' }))
    await act(async () => crowd({ x: 9, y: 9 }))
    expect(session.pickDestination).not.toHaveBeenCalled()
    await act(async () => join(new Map([['0xf1', { kind: 'genesis', parcel: { x: 4, y: 5 } }]])))
    expect(session.pickDestination).toHaveBeenCalledOnce()
    expect(session.pickDestination).toHaveBeenCalledWith({ kind: 'parcel', x: 4, y: 5 })
  })

  it('does not travel to a friend whose location answers after the layout editor opened', async () => {
    const session = fakeSession()
    session.friends.loading = false
    session.friends.list = [{ address: '0xf1', name: 'Mojito', status: 'online' }]
    let join: (found: Map<string, { kind: 'genesis'; parcel: { x: number; y: number } }>) => void = () => undefined
    vi.mocked(fetchWhereabouts)
      .mockResolvedValueOnce(new Map([['0xf1', { kind: 'genesis', parcel: { x: 4, y: 5 } }]]))
      .mockReturnValueOnce(new Promise((resolve) => (join = resolve)))
    render(lobby(session))
    fireEvent.click(await screen.findByRole('button', { name: 'Join Mojito' }))
    fireEvent.click(screen.getByRole('button', { name: 'Customize lobby layout' }))
    await act(async () => join(new Map([['0xf1', { kind: 'genesis', parcel: { x: 4, y: 5 } }]])))
    expect(session.pickDestination).not.toHaveBeenCalled()
    expect(screen.getByRole('dialog', { name: 'Customize Lobby' })).toBeInTheDocument()
  })

  it('records a JOIN or the landing Jump in only once the pick is taken', async () => {
    const track = vi.spyOn(analytics, 'track')
    const session = fakeSession()
    session.friends.loading = false
    session.friends.list = [{ address: '0xf1', name: 'Mojito', status: 'online' }]
    session.homeScene = () => ({ realm: null, parcel: '1,1' })
    vi.mocked(session.pickDestination).mockReturnValue(false)
    vi.mocked(fetchWhereabouts).mockResolvedValue(new Map([['0xf1', { kind: 'genesis', parcel: { x: 4, y: 5 } }]]))
    render(lobby(session))
    fireEvent.click(await screen.findByRole('button', { name: 'Join Mojito' }))
    await waitFor(() => expect(session.pickDestination).toHaveBeenCalledOnce())
    await waitFor(() => expect(screen.getByRole('button', { name: 'Jump In' })).toBeEnabled())
    fireEvent.click(screen.getByRole('button', { name: 'Jump In' }))
    expect(session.pickDestination).toHaveBeenCalledTimes(2)
    const sent = track.mock.calls.map(([name, props]) => [name, (props as { button_text?: string } | undefined)?.button_text])
    expect(sent).not.toContainEqual(['jump_to_friend_clicked', undefined])
    expect(sent).not.toContainEqual(['Click Button', 'JUMP_IN'])
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

  it('lets a page opened over the editor close first on Cancel and keeps the draft', () => {
    const session = fakeSession()
    const { rerender } = render(lobby(session))
    fireEvent.click(screen.getByRole('button', { name: 'Customize lobby layout' }))
    fireEvent.click(screen.getByRole('button', { name: 'Remove Jump In Home' }))
    rerender(lobby({ ...session, lobbyData: { ...session.lobbyData, covered: true } }))
    expect(dispatchCancelLayer()).toBe(false)
    rerender(lobby(session))
    const modal = screen.getByRole('dialog', { name: 'Customize Lobby' })
    expect(within(modal).getByRole('button', { name: 'SAVE' })).toBeEnabled()
    act(() => {
      expect(dispatchCancelLayer()).toBe(true)
    })
    expect(screen.queryByRole('dialog', { name: 'Customize Lobby' })).not.toBeInTheDocument()
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
  it('signs out from the menu once confirmed, and offers no Exit on the web', async () => {
    const session = fakeSession()
    render(lobbyWithPopups(session))
    fireEvent.click(screen.getByRole('button', { name: /guest|test/i, expanded: false }))
    const menu = screen.getByRole('menu', { name: 'Profile menu' })
    expect(within(menu).queryByRole('menuitem', { name: 'Exit' })).not.toBeInTheDocument()
    fireEvent.click(within(menu).getByRole('menuitem', { name: 'Sign Out' }))
    expect(screen.queryByRole('menu')).not.toBeInTheDocument()
    const ask = await screen.findByRole('dialog', { name: 'Sign out?' })
    expect(session.logout).not.toHaveBeenCalled()
    fireEvent.click(within(ask).getByRole('button', { name: 'Sign Out' }))
    await waitFor(() => expect(session.logout).toHaveBeenCalledOnce())
  })

  it('asks before signing out, and Cancel keeps the session', async () => {
    const session = fakeSession()
    render(lobbyWithPopups(session))
    fireEvent.click(screen.getByRole('button', { name: /guest|test/i, expanded: false }))
    fireEvent.click(within(screen.getByRole('menu', { name: 'Profile menu' })).getByRole('menuitem', { name: 'Sign Out' }))
    fireEvent.click(within(await screen.findByRole('dialog', { name: 'Sign out?' })).getByRole('button', { name: 'Cancel' }))
    await waitFor(() => expect(screen.queryByRole('dialog', { name: 'Sign out?' })).toBeNull())
    expect(session.logout).not.toHaveBeenCalled()
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

  it('opens Settings compact in the section modal, re-reading the settings, and closes it', () => {
    const session = fakeSession()
    render(lobbyWithPopups(session))
    fireEvent.click(screen.getByRole('button', { name: /guest|test/i, expanded: false }))
    fireEvent.click(within(screen.getByRole('menu', { name: 'Profile menu' })).getByRole('menuitem', { name: 'Settings' }))
    const dialog = screen.getByRole('dialog', { name: 'Settings' })
    expect(within(dialog).getByRole('tablist', { name: 'Settings categories' })).toBeInTheDocument()
    expect(session.settings.toggle).not.toHaveBeenCalled()
    expect(session.settings.refresh).toHaveBeenCalledOnce()
    fireEvent.click(screen.getByRole('button', { name: 'Close Settings' }))
    expect(screen.queryByRole('dialog')).toBeNull()
  })

  it('hands compact Settings over to the full page when it opens under the modal', () => {
    const session = fakeSession()
    const { rerender } = render(lobbyWithPopups(session))
    fireEvent.click(screen.getByRole('button', { name: /guest|test/i, expanded: false }))
    fireEvent.click(within(screen.getByRole('menu', { name: 'Profile menu' })).getByRole('menuitem', { name: 'Settings' }))
    expect(screen.getByRole('dialog', { name: 'Settings' })).toBeInTheDocument()
    session.settings.open = true
    rerender(lobbyWithPopups({ ...session }))
    expect(screen.queryByRole('dialog', { name: 'Settings' })).toBeNull()
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
