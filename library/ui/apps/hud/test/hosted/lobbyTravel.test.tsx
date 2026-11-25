// DOMAIN: travel asked from a menu page over the startup lobby — through the Map page, the Places
// page and the events' jump-in, each closes its page exactly once and leaves the lobby; a pick the
// lobby refuses (sign-in still running) leaves the page open and says why; a place's detail holds
// its Jump In until the sign-in ends.
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { act, fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { PopupHost, ToastHost, resetPopups } from '../../design'
import { useEventActions, type EventActions } from '../../features/events/useEventActions'
import { MapPage } from '../../features/map/MapPage'
import { PlacesPage } from '../../features/places/PlacesPage'
import { SessionProvider } from '../../features/session/SessionContext'
import { useEngineSession, type EngineSession } from '../../features/session/useEngineSession'
import type { DiscoverPlace } from '@explorer-core/features/places/placesApi'
import { fixMapPublicReads, receiveMapReply, setMapTransport } from '@explorer-core/features/map/mapApi'
import { placeOf } from '@explorer-core/test/mapFixtures'
import { GENESIS_SATELLITE_VIEW } from '@explorer-core/features/map/atlas'
import { eventAt } from './eventFixtures'
import { FakeDriver } from '../harness'

const PLACE: DiscoverPlace = { id: 'p1', title: 'Plaza', description: 'd', image: '', positions: ['10,-5'], base_position: '10,-5', owner: null }

// the browser's grid is not under test: one card, picked the way a card click picks it
vi.mock('../../features/places/PlacesBrowser', () => ({
  PlacesBrowser: ({ onPick }: { onPick: (place: DiscoverPlace) => void }) => (
    <button type="button" onClick={() => onPick(PLACE)}>
      Plaza card
    </button>
  )
}))

interface Hud {
  driver: FakeDriver
  session: () => EngineSession
  actions: () => EventActions
  /** How many times each page went from open to closed or back. */
  flips: Record<'map' | 'places' | 'events', number>
}

function renderHud(driver: FakeDriver = new FakeDriver()): Hud {
  let current!: EngineSession
  let actions!: EventActions
  const flips = { map: 0, places: 0, events: 0 }
  const was = { map: false, places: false, events: false }
  function Actions(): null {
    actions = useEventActions()
    return null
  }
  function Tree(): React.JSX.Element {
    const session = useEngineSession(() => driver)
    current = session
    for (const page of ['map', 'places', 'events'] as const) {
      if (session[page].open !== was[page]) flips[page]++
      was[page] = session[page].open
    }
    return (
      <SessionProvider value={session}>
        {/* the lobby has no satellite view yet; the map's parcel picks are Genesis City's */}
        <MapPage map={session.map} minimap={{ ...session.minimap, satelliteView: GENESIS_SATELLITE_VIEW }} profile={session.profile} gallery={session.gallery} onNavigate={vi.fn()} />
        <PlacesPage places={session.places} profile={session.profile} onNavigate={vi.fn()} onTeleport={(x, y) => session.map.teleportToPlace(x, y)} onVisitWorld={(realm) => session.map.changeRealm(realm)} />
        <Actions />
        <PopupHost />
        <ToastHost />
      </SessionProvider>
    )
  }
  render(<Tree />)
  return { driver, session: () => current, actions: () => actions, flips }
}

async function inLobby(hud: Hud): Promise<void> {
  await waitFor(() => expect(hud.session().login.status).toBe('sign-in-or-guest'))
  act(() => hud.session().login.exploreAsGuest())
  await waitFor(() => expect(hud.session().phase).toBe('lobby'))
  await waitFor(() => expect(hud.session().login.busy).toBe(false))
  hud.driver.emit({ kind: 'event', name: 'playerReady' })
}

/** A lobby whose sign-in never finishes: it is up, and not taking picks. */
async function inBusyLobby(): Promise<Hud> {
  const driver = new FakeDriver()
  const never = (): Promise<never> => new Promise(() => {})
  Object.assign(driver, { loginGuest: never, loginWithIdentity: never })
  const hud = renderHud(driver)
  await waitFor(() => expect(hud.session().login.status).toBe('sign-in-or-guest'))
  act(() => hud.session().login.exploreAsGuest())
  // the lobby is held (its welcome page may still be over it) and the sign-in has not answered
  await waitFor(() => expect(hud.session().login.busy).toBe(true))
  expect(['welcome', 'lobby']).toContain(hud.session().phase)
  return hud
}

/** The map's bridge requests, answered with the one place, at 10,-5; anything else finds nothing. */
function serveMap(): void {
  const plaza = placeOf({ ...PLACE, categories: [], user_count: 1, like_rate: null })
  setMapTransport((request) => {
    if (request.kind !== 'getPlacesAt') return
    const places = plaza.positions.some((pos) => request.positions.includes(pos)) ? [plaza] : []
    queueMicrotask(() => receiveMapReply({ kind: 'placesAt', request: request.request, places, failed: false }))
  })
}

async function jumpInFromMap(): Promise<void> {
  // after the session has mounted: it sets the bridge transport of its own
  serveMap()
  // the view opens on parcel 0,0 at 7.372 px a parcel, its middle at 960,505: 10 east and 5 south
  fireEvent.click(screen.getByTestId('map-view'), { clientX: 960 + 9.5 * 7.372, clientY: 505 + 5.5 * 7.372 })
  const panel = screen.getByRole('complementary', { name: 'Places and events' })
  await within(panel).findByRole('heading', { name: 'Plaza' })
  await userEvent.click(within(panel).getByRole('button', { name: 'JUMP IN' }))
}

async function placeJumpIn(): Promise<HTMLElement> {
  await userEvent.click(screen.getByRole('button', { name: 'Plaza card' }))
  return screen.findByRole('button', { name: 'Jump in to Plaza' })
}

const REFUSED = 'Still signing in. Try again in a moment.'

describe('travel from a page over the startup lobby', () => {
  beforeEach(() => {
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue({ ok: true, json: async () => ({ data: [{ ...PLACE, user_count: 1, like_rate: null }] }) }))
    fixMapPublicReads({ pois: [], peers: [] })
  })
  afterEach(() => {
    vi.unstubAllGlobals()
    resetPopups()
    setMapTransport(null)
    fixMapPublicReads(null)
  })

  it('the Map page: JUMP IN leaves the lobby and closes the map once', async () => {
    const hud = renderHud()
    await inLobby(hud)
    act(() => hud.session().map.toggle())
    expect(hud.flips.map).toBe(1)
    await jumpInFromMap()
    expect(hud.driver.last('teleport')).toMatchObject({ x: 10, y: -5 })
    expect(hud.session().phase).toBe('entering')
    expect(hud.session().map.open).toBe(false)
    expect(hud.flips.map).toBe(2)
  })

  it('the Places page: a card\'s jump in leaves the lobby and closes the page once', async () => {
    const hud = renderHud()
    await inLobby(hud)
    act(() => hud.session().places.toggle())
    await userEvent.click(await placeJumpIn())
    expect(hud.driver.last('teleport')).toMatchObject({ x: 10, y: -5 })
    expect(hud.session().phase).toBe('entering')
    expect(hud.session().places.open).toBe(false)
    expect(hud.flips.places).toBe(2)
  })

  it('an event\'s jump in leaves the lobby and closes the Events page once', async () => {
    const hud = renderHud()
    await inLobby(hud)
    act(() => hud.session().events.toggle('sidebar'))
    let went = false
    act(() => {
      went = hud.actions().jumpIn(eventAt('e', new Date(), { x: 10, y: -5 }))
    })
    expect(went).toBe(true)
    expect(hud.driver.last('teleport')).toMatchObject({ x: 10, y: -5 })
    expect(hud.session().phase).toBe('entering')
    expect(hud.session().events.open).toBe(false)
    expect(hud.flips.events).toBe(2)
  })

  it('a pick refused while sign-in runs keeps the Map page open and says why', async () => {
    const hud = await inBusyLobby()
    act(() => hud.session().map.toggle())
    await jumpInFromMap()
    expect(hud.driver.sentOf('teleport')).toHaveLength(0)
    expect(hud.session().phase).not.toBe('entering')
    expect(hud.session().map.open).toBe(true)
    expect(hud.flips.map).toBe(1)
    expect(screen.getByText(REFUSED)).toBeInTheDocument()
  })

  it('a place\'s jump in waits for the sign-in, and an event\'s jump in reports that nothing happened', async () => {
    const hud = await inBusyLobby()
    act(() => hud.session().places.toggle())
    expect(await placeJumpIn()).toBeDisabled()
    expect(hud.session().places.open).toBe(true)
    expect(hud.flips.places).toBe(1)
    act(() => hud.session().events.toggle('sidebar'))
    let went = true
    act(() => {
      went = hud.actions().jumpIn(eventAt('e', new Date(), { x: 10, y: -5 }))
    })
    expect(went).toBe(false)
    expect(hud.session().events.open).toBe(true)
    expect(hud.driver.sentOf('teleport')).toHaveLength(0)
    expect(hud.driver.sentOf('changeRealm')).toHaveLength(0)
    expect(screen.getAllByText(REFUSED).length).toBeGreaterThan(0)
  })
})
