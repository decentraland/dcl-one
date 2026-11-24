import { describe, it, expect, vi, afterEach, beforeEach } from 'vitest'
import { act, fireEvent, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { dispatchCancelLayer } from '@explorer-core/lib/cancelLayers'
import { PopupHost, resetPopups, topPopupOrder } from '../../design/popups'
import { getPref, PREF, setPref } from '@explorer-core/lib/prefs'
import { Sidebar } from '../../features/sidebar/Sidebar'
import type { EngineSession } from '../../features/session/useEngineSession'
import { renderSettled, fakeSession } from '../harness'

const { native, signOutOfWorld } = vi.hoisted(() => ({ native: { value: false }, signOutOfWorld: vi.fn() }))
vi.mock('@hud-host', async (original) => ({ ...(await original<object>()), isNativeHud: () => native.value }))
// the confirmed sign-out loads the page again, which jsdom cannot do
vi.mock('../../features/gate/signOutOfWorld', async (original) => ({ ...(await original<object>()), signOutOfWorld }))

// the Map button shows the minimap, which reads the map's layers and the place under the player
vi.mock('@explorer-core/features/map/mapApi', async (original) => ({
  ...await original<typeof import('@explorer-core/features/map/mapApi')>(),
  fetchLiveEvents: vi.fn(async () => []),
  fetchPoiParcels: vi.fn(async () => []),
  fetchPeers: vi.fn(async () => []),
  fetchPlaceAt: vi.fn(async () => null)
}))

const bar = (): ReturnType<typeof within> => within(screen.getByRole('navigation', { name: 'Main navigation' }))
const barButton = (name: string): HTMLElement => bar().getByRole('button', { name })
const press = (name: string): Promise<void> => userEvent.click(barButton(name))
const panel = (name: string): HTMLElement | null => screen.queryByRole('dialog', { name })
/** The rows of a panel by label, without the key hint or the count a row draws after it. */
const rowsOf = (name: string): string[] =>
  within(screen.getByRole('dialog', { name }))
    .getAllByRole('button')
    .map((row) => row.getAttribute('aria-label') ?? (row.textContent ?? '').replace(/(\[[^\]]*\])?\d*$/, ''))

beforeEach(() => {
  localStorage.clear()
  native.value = false
  signOutOfWorld.mockReset()
})
afterEach(() => vi.restoreAllMocks())

// The navigation bar: its buttons, their art, and the voice state.
describe('sidebar parity', () => {
  it.each([
    [true, true],
    [false, false]
  ])('the Communities row carries the live badge when a member community streams (member %s)', async (isMember, shown) => {
    const s = fakeSession()
    s.voiceChat = { ...s.voiceChat, active: [{ communityId: 'c1', communityName: 'C', isMember, positions: [], worlds: [], participantCount: 3, moderatorCount: 1 }] }
    await renderSettled(<Sidebar session={s} />)
    await press('Discover')
    const row = within(screen.getByRole('dialog', { name: 'Discover' })).getByRole('button', { name: 'Communities' })
    expect(within(row).queryByRole('img', { name: 'A community stream is live' }) != null).toBe(shown)
  })

  it('the Communities row has no live badge while nothing streams', async () => {
    await renderSettled(<Sidebar session={fakeSession()} />)
    await press('Discover')
    const row = within(screen.getByRole('dialog', { name: 'Discover' })).getByRole('button', { name: 'Communities' })
    expect(within(row).queryByRole('img', { name: 'A community stream is live' })).toBeNull()
  })

  it.each(['Map', 'Discover', 'Shop', 'System', 'Notifications', 'Volume', 'Friends', 'Camera', 'Nearby Voice', 'Chat'])(
    '%s draws the bar\'s own glyph, not an icon-set glyph',
    async (name) => {
      await renderSettled(<Sidebar session={fakeSession()} />)
      const button = barButton(name)
      expect(button.querySelector('svg')).toBeNull()
      expect(button.querySelector('img')).not.toBeNull()
    }
  )

  it.each(['Skybox', 'Emotes'])('%s uses the bar\'s mask art, not an icon-set glyph', async (name) => {
    await renderSettled(<Sidebar session={fakeSession()} />)
    const button = barButton(name)
    expect(button.querySelector('svg')).toBeNull()
    expect(button.querySelector('span[aria-hidden="true"]')).not.toBeNull()
  })

  it('has no Help & Support button: the support rows are in the System panel', async () => {
    await renderSettled(<Sidebar session={fakeSession()} />)
    expect(screen.queryByRole('button', { name: 'Help & Support' })).toBeNull()
    await press('System')
    expect(rowsOf('System')).toEqual(['Settings', 'FAQ', 'Report Bug', 'Contact Support', 'Discord'])
  })

  it.each([
    [true, false, 'hearing', true],
    [true, true, 'speaking', true],
    [false, false, 'off', false]
  ])('voice button shows the nearby-voice state (hearing %s, mic %s)', async (hearing, enabled, state, dot) => {
    if (!hearing) localStorage.setItem('nearbyVoiceDisabled', 'true')
    const s = fakeSession()
    s.mic = { ...s.mic, available: true, enabled }
    await renderSettled(<Sidebar session={s} />)
    const button = screen.getByRole('button', { name: 'Nearby Voice' })
    expect(button).toHaveAttribute('data-voice', state)
    expect(button.querySelector('[data-indicator]') != null).toBe(dot)
    localStorage.clear()
  })

  it.each([
    [false, true],
    [true, false]
  ])('Chat shows its unread count only while the chat is closed (open %s)', async (open, shown) => {
    const s = fakeSession()
    s.chat = { ...s.chat, open, unread: 3 }
    await renderSettled(<Sidebar session={s} />)
    expect(within(barButton('Chat')).queryByText('3') != null).toBe(shown)
  })
})

// What the bar opens beside itself: one panel at a time, each a dialog named as its button.
describe('sidebar panels', () => {
  const PANELS: [string, string[]][] = [
    ['Me', ['Backpack', 'Profile', 'Exit']],
    ['Discover', ['Events', 'Places', 'Communities']],
    ['System', ['Settings', 'FAQ', 'Report Bug', 'Contact Support', 'Discord']],
    ['Camera', ['Take A Shot', 'Gallery']]
  ]

  it.each(PANELS)('%s opens its panel, reads pressed and drops its hover label; a second press closes it', async (name, rows) => {
    const s = fakeSession()
    s.profile.data = null
    await renderSettled(<Sidebar session={s} />)
    const button = barButton(name)
    expect(button).toHaveAttribute('aria-pressed', 'false')
    expect(button.parentElement?.querySelector('[role="tooltip"]')).not.toBeNull()
    await press(name)
    expect(rowsOf(name)).toEqual(rows)
    expect(button).toHaveAttribute('aria-pressed', 'true')
    expect(button.parentElement?.querySelector('[role="tooltip"]')).toBeNull()
    await press(name)
    expect(panel(name)).toBeNull()
    expect(button).toHaveAttribute('aria-pressed', 'false')
  })

  it('with a passport to open, Me has a Badges row that opens it on the Badges tab, and Profile opens it plain', async () => {
    const onViewProfile = vi.fn()
    await renderSettled(<Sidebar session={fakeSession()} onViewProfile={onViewProfile} />)
    await press('Me')
    expect(rowsOf('Me')).toEqual(['Backpack', 'Profile', 'Badges', 'Exit'])
    await userEvent.click(within(screen.getByRole('dialog', { name: 'Me' })).getByRole('button', { name: 'Badges' }))
    expect(onViewProfile).toHaveBeenLastCalledWith('badges')
    await press('Me')
    await userEvent.click(within(screen.getByRole('dialog', { name: 'Me' })).getByRole('button', { name: 'Profile' }))
    expect(onViewProfile).toHaveBeenLastCalledWith()
  })

  describe("Me's Sign Out row", () => {
    afterEach(() => {
      native.value = false
      return act(() => resetPopups())
    })

    // A sign-in on the web, a guest's included (the sign-out keeps the guest, as unity-explorer's); the
    // desktop client cannot end its engine session from the page; a sign-in the scene owns is not the
    // page's to end.
    it.each([
      ['an account on the web', 'fully_registered', false, true],
      ['a guest on the web', 'guest', false, true],
      ['a guest on the desktop client', 'guest', true, false],
      ['a session with a sign-in the scene owns', null, false, false],
      ['an account on the desktop client', 'fully_registered', true, false]
    ] as const)('%s (signed in as %s, native %s): the row is there -> %s', async (_who, signedInAs, isNative, shown) => {
      native.value = isNative
      const session = fakeSession()
      session.login.signedInAs = signedInAs
      await renderSettled(<Sidebar session={session} />)
      await press('Me')
      expect(rowsOf('Me').includes('Sign Out')).toBe(shown)
      expect(rowsOf('Me').includes('Exit')).toBe(true)
    })

    const ask = async (): Promise<{ session: EngineSession; row: HTMLElement; question: HTMLElement }> => {
      const session = fakeSession()
      session.login.signedInAs = 'fully_registered'
      await renderSettled(
        <>
          <Sidebar session={session} />
          <PopupHost />
        </>
      )
      await press('Me')
      const row = within(screen.getByRole('dialog', { name: 'Me' })).getByRole('button', { name: 'Sign Out' })
      await userEvent.click(row)
      return { session, row, question: screen.getByRole('dialog', { name: 'Sign out?' }) }
    }

    it('Cancel in the question leaves the session signed in, the panel open and the focus on the row', async () => {
      const { row, question } = await ask()
      await userEvent.click(within(question).getByRole('button', { name: 'Cancel' }))
      expect(screen.queryByRole('dialog', { name: 'Sign out?' })).toBeNull()
      expect(signOutOfWorld).not.toHaveBeenCalled()
      expect(panel('Me')).not.toBeNull()
      expect(row).toHaveFocus()
    })

    it('the Cancel action goes to the question before the panel under it', async () => {
      await ask()
      // the session's Cancel asks the layers opened after the top popup first: the panel is not one
      expect(dispatchCancelLayer(topPopupOrder())).toBe(false)
      expect(panel('Me')).not.toBeNull()
    })

    it('Sign Out in the question leaves the world once and closes the panel', async () => {
      const { session, question } = await ask()
      await userEvent.click(within(question).getByRole('button', { name: 'Sign Out' }))
      expect(signOutOfWorld).toHaveBeenCalledTimes(1)
      expect(panel('Me')).toBeNull()
      // the HUD's own sign-out would show the start screen over a live engine session
      expect(vi.mocked(session.logout)).not.toHaveBeenCalled()
    })
  })

  it('the panels with a heading name it: DISCOVER, CAMERA', async () => {
    await renderSettled(<Sidebar session={fakeSession()} />)
    await press('Discover')
    expect(screen.getByRole('heading', { name: 'DISCOVER' })).toBeInTheDocument()
    await press('Camera')
    expect(screen.getByRole('heading', { name: 'CAMERA' })).toBeInTheDocument()
  })

  it('opening a panel closes the one that was open', async () => {
    await renderSettled(<Sidebar session={fakeSession()} />)
    await press('Me')
    await press('Discover')
    expect(panel('Me')).toBeNull()
    expect(panel('Discover')).not.toBeNull()
    expect(barButton('Me')).toHaveAttribute('aria-pressed', 'false')
    expect(screen.getAllByRole('dialog')).toHaveLength(1)
  })

  it('an outside press closes the panel', async () => {
    await renderSettled(<Sidebar session={fakeSession()} />)
    await press('Discover')
    await userEvent.click(document.body)
    expect(panel('Discover')).toBeNull()
  })

  it('Cancel closes the panel', async () => {
    await renderSettled(<Sidebar session={fakeSession()} />)
    await press('System')
    act(() => {
      dispatchCancelLayer()
    })
    expect(panel('System')).toBeNull()
  })

  it('opening a panel that takes the notifications\' place closes them; Volume and Camera leave them', async () => {
    const s = fakeSession()
    s.notifications = { ...s.notifications, open: true }
    await renderSettled(<Sidebar session={s} />)
    await press('Volume')
    await press('Camera')
    expect(vi.mocked(s.notifications.toggle)).not.toHaveBeenCalled()
    await press('Discover')
    expect(vi.mocked(s.notifications.toggle)).toHaveBeenCalledTimes(1)
  })

  it('Take A Shot starts photo mode and closes the Camera panel', async () => {
    const s = fakeSession()
    await renderSettled(<Sidebar session={s} />)
    await press('Camera')
    await userEvent.click(screen.getByRole('button', { name: 'Take A Shot' }))
    expect(vi.mocked(s.photoMode.set)).toHaveBeenCalledWith(true, 'button')
    expect(panel('Camera')).toBeNull()
  })
})

describe('sidebar volume', () => {
  const master = (): EngineSession['settings']['list'][number] => ({
    name: 'Master Volume',
    category: 'Audio',
    description: '',
    minValue: 0,
    maxValue: 100,
    namedVariants: [],
    stepSize: 1,
    value: 70,
    default: 100,
    disabled: false
  })

  it('asks the engine for its settings when pressed', async () => {
    const s = fakeSession()
    await renderSettled(<Sidebar session={s} />)
    // Nearby Voice asks on mount, for its own volume (useNearbyVoice)
    const asked = vi.mocked(s.settings.load).mock.calls.length
    await press('Volume')
    expect(vi.mocked(s.settings.load)).toHaveBeenCalledTimes(asked + 1)
    expect(panel('Volume')).not.toBeNull()
  })

  it('has no slider until the engine lists the master volume', async () => {
    await renderSettled(<Sidebar session={fakeSession()} />)
    await press('Volume')
    expect(screen.queryByRole('slider')).toBeNull()
  })

  it('the Master slider sets the engine\'s master volume', async () => {
    const s = fakeSession()
    s.settings = { ...s.settings, list: [master()] }
    await renderSettled(<Sidebar session={s} />)
    await press('Volume')
    const slider = within(screen.getByRole('dialog', { name: 'Volume' })).getByRole('slider', { name: 'Master' })
    expect(slider).toHaveValue('70')
    fireEvent.change(slider, { target: { value: '40' } })
    expect(vi.mocked(s.settings.set)).toHaveBeenCalledWith('Master Volume', 40)
  })
})

describe('sidebar map', () => {
  const map = (): HTMLElement | null => screen.queryByRole('region', { name: 'Map' })

  it('Map shows the minimap and hides it, and reads pressed while it shows', async () => {
    await renderSettled(<Sidebar session={fakeSession()} />)
    expect(map()).toBeNull()
    expect(barButton('Map')).toHaveAttribute('aria-pressed', 'false')
    await press('Map')
    expect(within(map()!).getByRole('button', { name: 'Collapse minimap' })).toBeInTheDocument()
    expect(barButton('Map')).toHaveAttribute('aria-pressed', 'true')
    await press('Map')
    expect(map()).toBeNull()
    expect(barButton('Map')).toHaveAttribute('aria-pressed', 'false')
  })

  it('shows one minimap: the bar owns it, the page mounts none', async () => {
    setPref(PREF.sidebarMap, '1')
    await renderSettled(<Sidebar session={fakeSession()} />)
    expect(screen.getAllByRole('region', { name: 'Map' })).toHaveLength(1)
    expect(screen.getAllByRole('button', { name: 'Collapse minimap' })).toHaveLength(1)
  })

  it('remembers the choice across remounts (the bar unmounts under full-screen pages)', async () => {
    const first = await renderSettled(<Sidebar session={fakeSession()} />)
    await press('Map')
    first.unmount()
    const second = await renderSettled(<Sidebar session={fakeSession()} />)
    expect(map()).not.toBeNull()
    await press('Map')
    second.unmount()
    await renderSettled(<Sidebar session={fakeSession()} />)
    expect(map()).toBeNull()
  })

  it('the minimap gives its place to a panel that opens over it and comes back after', async () => {
    setPref(PREF.sidebarMap, '1')
    await renderSettled(<Sidebar session={fakeSession()} />)
    await press('Discover')
    expect(map()).toBeNull()
    expect(barButton('Map')).toHaveAttribute('aria-pressed', 'false')
    await press('Discover')
    expect(map()).not.toBeNull()
    // the Camera panel opens lower down, clear of the minimap
    await press('Camera')
    expect(map()).not.toBeNull()
  })

  it('Map, pressed while a panel lies over the minimap, closes the panel and shows the minimap', async () => {
    setPref(PREF.sidebarMap, '1')
    await renderSettled(<Sidebar session={fakeSession()} />)
    await press('Discover')
    expect(barButton('Map')).toHaveAttribute('aria-pressed', 'false')
    // a pointer press: the panel must not close before the button reads what shows
    await press('Map')
    expect(panel('Discover')).toBeNull()
    expect(map()).not.toBeNull()
    expect(barButton('Map')).toHaveAttribute('aria-pressed', 'true')
    expect(getPref(PREF.sidebarMap)).toBe('1')
  })

  it('the minimap gives its place to the notifications', async () => {
    setPref(PREF.sidebarMap, '1')
    const s = fakeSession()
    s.notifications = { ...s.notifications, open: true }
    await renderSettled(<Sidebar session={s} />)
    expect(map()).toBeNull()
  })
})
