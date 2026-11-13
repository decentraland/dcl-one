import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { act, fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { BackpackModal } from '../../features/backpack/BackpackModal'
import modalStyles from '../../features/backpack/BackpackModal.module.css'
import { LobbyHome } from '../../features/lobby/LobbyHome'
import { fetchLandingPlace, fetchRecents, worldReachable, type LobbyPlace } from '@explorer-core/features/lobby/lobbyApi'
import { getPref, PREF, setPref } from '@explorer-core/lib/prefs'
import * as analytics from '@explorer-core/lib/analytics'
import { receiveProfile, resetProfileStore, setProfileRequester } from '../../features/session/profileStore'
import { SessionProvider } from '../../features/session/SessionContext'
import type { EngineSession } from '../../features/session/useEngineSession'
import { fakeSession } from '../harness'

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

beforeEach(() => {
  setPref(PREF.lobbyLayout, '')
})

afterEach(() => {
  vi.restoreAllMocks()
  vi.clearAllMocks()
})

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
    const soon = new Date(Date.now() + 40 * 60_000).toISOString()
    session.lobbyEvents.list = [
      { id: 'l', name: 'Live Party', x: 1, y: 2, live: true, start_at: soon, connected_addresses: ['0xa'] },
      { id: 'u', name: 'Tower Smash', x: 3, y: 4, live: false, start_at: soon, next_start_at: soon, attending: false }
    ]
    render(lobby(session))
    expect(screen.getByText('Live Party')).toBeInTheDocument()
    expect(screen.getByText('Tower Smash')).toBeInTheDocument()
    expect(screen.getByText('In 40 min')).toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: 'Interested' }))
    expect(session.lobbyEvents.setInterest).toHaveBeenCalledWith('u', true)
    fireEvent.click(screen.getByRole('button', { name: 'Jump in to Live Party' }))
    expect(session.pickDestination).toHaveBeenCalledWith({ kind: 'parcel', x: 1, y: 2 })
  })

  it('clears the live events when the read fails and keeps the upcoming ones', () => {
    const session = fakeSession()
    session.lobbyEvents.list = [
      { id: 'l', name: 'Live Party', x: 1, y: 2, live: true, start_at: new Date().toISOString() },
      { id: 'u', name: 'Tower Smash', x: 3, y: 4, live: false, start_at: new Date(Date.now() + 3_600_000).toISOString() }
    ]
    session.lobbyEvents.failed = true
    render(lobby(session))
    expect(screen.queryByText('Live Party')).not.toBeInTheDocument()
    expect(screen.getByText('Tower Smash')).toBeInTheDocument()
  })

  it('opens the Events page on the right tab from VIEW ALL', () => {
    const session = fakeSession()
    session.lobbyEvents.list = [{ id: 'u', name: 'Tower Smash', x: 3, y: 4, live: false, start_at: new Date(Date.now() + 3_600_000).toISOString() }]
    render(lobby(session))
    const upcoming = screen.getByRole('heading', { name: 'Upcoming Events' }).parentElement!
    fireEvent.click(within(upcoming).getByRole('button', { name: /view all/i }))
    expect(session.events.openAt).toHaveBeenCalledWith('upcoming')
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
  it('drops the changes on Escape and stores them on Save', async () => {
    render(lobby(fakeSession()))
    expect(screen.getByRole('heading', { name: /welcome/i })).toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: 'Customize lobby layout' }))
    const modal = screen.getByRole('dialog', { name: 'Customize Lobby' })
    expect(within(modal).getByRole('button', { name: 'SAVE' })).toBeDisabled()
    fireEvent.click(screen.getByRole('button', { name: 'Remove Jump In Home' }))
    expect(within(modal).getByRole('button', { name: 'SAVE' })).toBeEnabled()
    act(() => {
      fireEvent.keyDown(window, { key: 'Escape' })
    })
    expect(screen.queryByRole('dialog', { name: 'Customize Lobby' })).not.toBeInTheDocument()
    expect(screen.getByRole('heading', { name: /welcome/i })).toBeInTheDocument()

    fireEvent.click(screen.getByRole('button', { name: 'Customize lobby layout' }))
    fireEvent.click(screen.getByRole('button', { name: 'Remove Jump In Home' }))
    fireEvent.click(screen.getByRole('button', { name: 'SAVE' }))
    expect(screen.queryByRole('heading', { name: /welcome/i })).not.toBeInTheDocument()
    expect(getPref(PREF.lobbyLayout)).not.toContain('Landing4By3')
  })

  it('lists only the widgets off the grids and places one from the keyboard', () => {
    render(lobby(fakeSession()))
    fireEvent.click(screen.getByRole('button', { name: 'Customize lobby layout' }))
    const modal = screen.getByRole('dialog', { name: 'Customize Lobby' })
    expect(within(modal).getByRole('button', { name: 'Add My Communities' })).toBeInTheDocument()
    expect(within(modal).queryByRole('button', { name: 'Add Friends' })).not.toBeInTheDocument()
    fireEvent.keyDown(within(modal).getByRole('button', { name: 'Add Go to' }), { key: 'Enter' })
    expect(within(modal).queryByRole('button', { name: 'Add Go to' })).not.toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'Remove Go to' })).toBeInTheDocument()
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

  it('reports the visit opened and closed, and idles the avatar while up', () => {
    const track = vi.spyOn(analytics, 'track')
    const session = fakeSession()
    const { unmount } = render(lobby(session, vi.fn()))
    expect(track).toHaveBeenCalledWith('lobby_opened', { source: 'lobby', is_startup: false })
    expect(session.lobbyStage.set).toHaveBeenCalledWith({ idleEmotes: true })
    expect(session.lobbyEvents.warm).toHaveBeenCalled()
    unmount()
    expect(track).toHaveBeenCalledWith('lobby_closed', expect.objectContaining({ source: 'lobby', is_startup: false }))
    expect(session.lobbyStage.set).toHaveBeenCalledWith({ idleEmotes: false })
  })
})
