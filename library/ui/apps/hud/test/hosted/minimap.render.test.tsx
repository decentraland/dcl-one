import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { act, fireEvent, screen, waitFor } from '@testing-library/react'
import { dispatchCancelLayer } from '@explorer-core/lib/cancelLayers'
import { DEFAULT_REALM } from '@hud-host'
import { Minimap } from '../../features/minimap/Minimap'
import { fakeSession, renderSettled } from '../harness'
import { mapLayers } from '@explorer-core/features/map/mapLayersStore'
import { navigation } from '@explorer-core/features/map/navigation'
import { mapPins } from '@explorer-core/features/map/mapPins'
import { MARKER_ART } from '../../features/map/mapArt'

const placeAt = vi.hoisted(() => ({ place: null as unknown }))
// The layer data is one store for the page, read the first time a map asks: the live event is there
// from the first read.
const layerData = vi.hoisted(() => ({ events: [{ id: 'e1', name: 'Party', x: 2, y: 3, live: true, world: false }] as unknown[], favourite: vi.fn() }))
vi.mock('@explorer-core/features/map/mapApi', async (original) => ({
  ...await original<typeof import('@explorer-core/features/map/mapApi')>(),
  fetchPlaceAt: vi.fn(async () => placeAt.place),
  fetchLiveEvents: vi.fn(async () => layerData.events),
  fetchPoiParcels: vi.fn(async () => []),
  fetchPeers: vi.fn(async () => []),
  setPlaceFavorite: layerData.favourite
}))

// The minimap header: the scene name comes from the sceneInfo stream (see world.test.tsx for
// the wiring); this covers what the header actually renders, including the empty-parcel case.
async function renderMinimap(sceneTitle: string, restrictions: string[] = []): Promise<void> {
  const session = fakeSession()
  await renderSettled(<Minimap minimap={{ ...session.minimap, restrictions }} map={session.map} sceneTitle={sceneTitle} setEngineViewport={vi.fn()} />)
}

beforeEach(() => {
  localStorage.clear()
  placeAt.place = null
  layerData.favourite.mockReset()
  mapLayers.set('liveEvents', true)
  mapLayers.set('pins', true)
  mapPins.clear()
  navigation.stop()
})
// jsdom has no clipboard: a test defines one, and it goes with the test
function stubClipboard(writeText: (text: string) => Promise<void>): void {
  Object.defineProperty(navigator, 'clipboard', { value: { writeText }, configurable: true })
}
afterEach(() => {
  Reflect.deleteProperty(navigator, 'clipboard')
})

describe('minimap header', () => {
  it('shows the scene the player is standing in', async () => {
    await renderMinimap('Genesis Plaza')
    expect(screen.getByText('Genesis Plaza')).toBeInTheDocument()
  })

  it('says the parcel is empty where nothing is deployed, not that the place is unknown', async () => {
    await renderMinimap('')
    expect(screen.getByText('Empty parcel')).toBeInTheDocument()
  })

  it('lists what the scene turns off', async () => {
    await renderMinimap('Quiet Scene', ['Nearby voice disabled'])
    expect(screen.getByRole('img', { name: 'Scene restrictions: Nearby voice disabled' })).toBeInTheDocument()
    expect(screen.getByRole('tooltip')).toHaveTextContent('\u2022 Nearby voice disabled')
  })

  it('shows no restrictions icon where the scene turns nothing off', async () => {
    await renderMinimap('Genesis Plaza')
    expect(screen.queryByRole('img', { name: /Scene restrictions/ })).toBeNull()
  })
})

describe('minimap menu', () => {
  // the place has no scene (no `donatePlace`) in each case: the realm alone tells
  async function openMenu(
    linkRealm: string | null,
    { at = { x: 10, y: 20 }, home = null, referrer }: { at?: { x: number; y: number }; home?: { realm: string | null; parcel: string; pinned: boolean } | null; referrer?: string } = {}
  ): Promise<ReturnType<typeof fakeSession>> {
    const session = fakeSession()
    session.map = { ...session.map, ...at, home }
    await renderSettled(
      <Minimap minimap={{ ...session.minimap, linkRealm }} map={session.map} sceneTitle="" referrer={referrer} setEngineViewport={vi.fn()} />
    )
    fireEvent.click(screen.getByRole('button', { name: 'Minimap settings' }))
    return session
  }

  it('offers Copy Link where the place has a link', async () => {
    await openMenu(DEFAULT_REALM)
    expect(screen.getByRole('menuitem', { name: 'Copy Link' })).toBeEnabled()
    expect(screen.queryByRole('button', { name: 'Jump back to Genesis City' })).toBeNull()
  })

  it('in a World, on a parcel with no scene too, offers the link and the way back to Genesis City', async () => {
    const session = await openMenu('boedo.dcl.eth')
    expect(screen.getByRole('menuitem', { name: 'Copy Link' })).toBeEnabled()
    fireEvent.click(screen.getByRole('button', { name: 'Jump back to Genesis City' }))
    expect(session.map.teleportToPlace).toHaveBeenCalledWith(0, 0)
    expect(session.map.changeRealm).not.toHaveBeenCalled()
  })

  it('jumps back to the Genesis City home parcel', async () => {
    const session = await openMenu('boedo.dcl.eth', { home: { realm: null, parcel: '52,-3', pinned: true } })
    fireEvent.click(screen.getByRole('button', { name: 'Jump back to Genesis City' }))
    expect(session.map.teleportToPlace).toHaveBeenCalledWith(52, -3)
  })

  it('jumps back to Genesis Plaza when home is a World', async () => {
    const session = await openMenu('boedo.dcl.eth', { home: { realm: 'boedo.dcl.eth', parcel: '5,5', pinned: true } })
    fireEvent.click(screen.getByRole('button', { name: 'Jump back to Genesis City' }))
    expect(session.map.teleportToPlace).toHaveBeenCalledWith(0, 0)
  })

  it('leaves Copy Link out where the place has none', async () => {
    await openMenu(null)
    expect(screen.getByRole('menu', { name: 'Minimap settings' })).toBeInTheDocument()
    expect(screen.queryByRole('menuitem', { name: 'Copy Link' })).toBeNull()
  })

  it('lists the scene options first: Set as Home, Copy Link, Reload Scene', async () => {
    await openMenu(DEFAULT_REALM)
    const menu = screen.getByRole('menu', { name: 'Minimap settings' })
    const names = [...menu.querySelectorAll('[role^="menuitem"], [role="switch"]')].map((el) => el.getAttribute('aria-label') ?? el.textContent)
    expect(names.slice(0, 3)).toEqual(['Set as Home', 'Copy Link', 'Reload Scene'])
  })

  it('Set as Home makes the parcel the player stands on home', async () => {
    const session = await openMenu(DEFAULT_REALM)
    const toggle = screen.getByRole('switch', { name: 'Set as Home' })
    expect(toggle).toHaveAttribute('aria-checked', 'false')
    fireEvent.click(toggle)
    expect(session.map.setHome).toHaveBeenCalledWith(DEFAULT_REALM, { x: 10, y: 20 })
  })

  it('in a World, Set as Home makes the World home', async () => {
    const session = await openMenu('boedo.dcl.eth')
    fireEvent.click(screen.getByRole('switch', { name: 'Set as Home' }))
    expect(session.map.setHome).toHaveBeenCalledWith('boedo.dcl.eth', { x: 0, y: 0 })
  })

  it('reads on where the player stands in the home place, and turning it off takes the home back', async () => {
    placeAt.place = { id: 'p', title: 'Plaza', positions: ['10,20', '11,20'], base_position: '10,20' }
    const session = await openMenu(DEFAULT_REALM, { at: { x: 11, y: 20 }, home: { realm: null, parcel: '10,20', pinned: true } })
    const toggle = await screen.findByRole('switch', { name: 'Set as Home', checked: true })
    fireEvent.click(toggle)
    expect(session.map.clearHome).toHaveBeenCalledTimes(1)
    expect(session.map.setHome).not.toHaveBeenCalled()
  })

  it('Reload Scene reloads the scene under the player and closes the menu', async () => {
    const session = await openMenu(DEFAULT_REALM)
    fireEvent.click(screen.getByRole('menuitem', { name: 'Reload Scene' }))
    expect(session.minimap.reloadScene).toHaveBeenCalled()
    expect(screen.queryByRole('menu', { name: 'Minimap settings' })).toBeNull()
  })

  it('in a preview offers Reload Scene but no Set as Home', async () => {
    await openMenu(null)
    expect(screen.getByRole('menuitem', { name: 'Reload Scene' })).toBeInTheDocument()
    expect(screen.queryByRole('switch', { name: 'Set as Home' })).toBeNull()
  })

  it('offers no Set as Home to a guest with no account', async () => {
    const session = fakeSession()
    session.map = { ...session.map, canPinHome: false }
    await renderSettled(
      <Minimap minimap={{ ...session.minimap, linkRealm: DEFAULT_REALM }} map={session.map} sceneTitle="" setEngineViewport={vi.fn()} />
    )
    fireEvent.click(screen.getByRole('button', { name: 'Minimap settings' }))
    expect(screen.getByRole('menuitem', { name: 'Copy Link' })).toBeInTheDocument()
    expect(screen.queryByRole('switch', { name: 'Set as Home' })).toBeNull()
  })

  it("copies a Genesis City parcel's link with the player's address", async () => {
    const writeText = vi.fn(async () => {})
    stubClipboard(writeText)
    await openMenu(DEFAULT_REALM, { at: { x: 3, y: 4 }, referrer: '0xabc' })
    fireEvent.click(screen.getByRole('menuitem', { name: 'Copy Link' }))
    expect(writeText).toHaveBeenCalledWith(expect.stringMatching(/\/jump\/\?position=3,4&referrer=0xabc$/))
  })

  it("copies a World's link with its parcel and the player's address", async () => {
    const writeText = vi.fn(async () => {})
    stubClipboard(writeText)
    await openMenu('boedo.dcl.eth', { at: { x: 3, y: 4 }, referrer: '0xabc' })
    fireEvent.click(screen.getByRole('menuitem', { name: 'Copy Link' }))
    expect(writeText).toHaveBeenCalledWith(expect.stringMatching(/\/jump\/\?realm=boedo\.dcl\.eth&position=3,4&referrer=0xabc$/))
  })
})

describe('minimap menu, closing', () => {
  it('closes on Cancel, as the topmost layer', async () => {
    const session = fakeSession()
    await renderSettled(<Minimap minimap={{ ...session.minimap, linkRealm: DEFAULT_REALM }} map={session.map} sceneTitle="" setEngineViewport={vi.fn()} />)
    fireEvent.click(screen.getByRole('button', { name: 'Minimap settings' }))
    act(() => {
      expect(dispatchCancelLayer()).toBe(true)
    })
    expect(screen.queryByRole('menu', { name: 'Minimap settings' })).toBeNull()
    expect(dispatchCancelLayer()).toBe(false)
  })
})

describe('minimap home pin', () => {
  it('marks a Genesis City home on the map', async () => {
    const session = fakeSession()
    session.map = { ...session.map, home: { realm: null, parcel: '1,2', pinned: true } }
    const { container } = await renderSettled(<Minimap minimap={session.minimap} map={session.map} sceneTitle="" setEngineViewport={vi.fn()} />)
    expect(container.querySelector('[data-marker="home"]')).not.toBeNull()
  })

  it('draws none in a World, for a Genesis City home', async () => {
    const session = fakeSession()
    session.map = { ...session.map, home: { realm: null, parcel: '1,2', pinned: true } }
    const { container } = await renderSettled(
      <Minimap minimap={{ ...session.minimap, satelliteView: null, linkRealm: 'boedo.dcl.eth' }} map={session.map} sceneTitle="" setEngineViewport={vi.fn()} />
    )
    expect(container.querySelector('[data-marker="home"]')).toBeNull()
  })

  it('draws none for a World home', async () => {
    const session = fakeSession()
    session.map = { ...session.map, home: { realm: 'boedo.dcl.eth', parcel: '0,0', pinned: true } }
    const { container } = await renderSettled(<Minimap minimap={session.minimap} map={session.map} sceneTitle="" setEngineViewport={vi.fn()} />)
    expect(container.querySelector('[data-marker="home"]')).toBeNull()
  })
})

describe('minimap layers', () => {
  async function renderAt(): Promise<HTMLElement> {
    const session = fakeSession()
    const view = await renderSettled(<Minimap minimap={{ ...session.minimap, linkRealm: DEFAULT_REALM }} map={session.map} sceneTitle="" setEngineViewport={vi.fn()} />)
    return view.container
  }

  it("draws the full map's live events, and the Layers panel hides them here too", async () => {
    const container = await renderAt()
    await waitFor(() => expect(container.querySelectorAll('img[src*="live"]').length).toBe(1))
    act(() => mapLayers.set('liveEvents', false))
    expect(container.querySelectorAll('img[src*="live"]').length).toBe(0)
  })

  it("draws the portable experiences' quest pins, and the Layers panel hides them here too", async () => {
    const container = await renderAt()
    act(() =>
      mapPins.receive([
        { id: 'bafypx:512', x: 1, y: 1, title: 'Memory Grid' },
        { id: 'bafypx:513', x: 2, y: 1, title: 'Tile Tally' }
      ])
    )
    const pins = container.querySelectorAll('[data-marker="pins"]')
    expect(pins.length).toBe(2)
    expect(pins[1].querySelector('img')).toHaveAttribute('src', MARKER_ART.quest)
    act(() => mapLayers.set('pins', false))
    expect(container.querySelectorAll('[data-marker="pins"]').length).toBe(0)
  })

  it('has no markers setting of its own any more', async () => {
    await renderAt()
    fireEvent.click(screen.getByRole('button', { name: 'Minimap settings' }))
    fireEvent.click(screen.getByRole('menuitem', { name: /Minimap settings/ }))
    expect(screen.queryByText('Markers')).toBeNull()
    expect(screen.queryByRole('menuitemradio', { name: 'Only POI' })).toBeNull()
  })

  it('draws the route, and puts a destination off the map on its edge', async () => {
    const container = await renderAt()
    expect(container.querySelector('[data-marker="destination"]')).toBeNull()
    act(() => navigation.start({ id: 'p', title: 'Far', positions: ['40,40'], base_position: '40,40' } as never))
    expect(container.querySelector('[data-marker="destination"]')).not.toBeNull()
    const edge = container.querySelector<HTMLElement>('[data-marker="destination-edge"]')
    await waitFor(() => expect(edge?.hidden).toBe(false))
  })

  it('leaves a destination on the map off the edge', async () => {
    const container = await renderAt()
    act(() => navigation.start({ id: 'p', title: 'Near', positions: ['1,0'], base_position: '1,0' } as never))
    const edge = container.querySelector<HTMLElement>('[data-marker="destination-edge"]')
    await new Promise((r) => setTimeout(r, 50))
    expect(edge?.hidden).toBe(true)
  })
})

describe('minimap favourite', () => {
  const plaza = { id: 'p', title: 'Plaza', positions: ['0,0'], base_position: '0,0', user_favorite: false }
  async function renderAt(): Promise<void> {
    const session = fakeSession()
    await renderSettled(<Minimap minimap={{ ...session.minimap, linkRealm: DEFAULT_REALM }} map={session.map} sceneTitle="" setEngineViewport={vi.fn()} />)
  }

  it('is off and greyed where no place is under the player', async () => {
    await renderAt()
    expect(screen.getByRole('button', { name: 'Favorite' })).toBeDisabled()
  })

  it('marks the place under the player at once and keeps what the service answers', async () => {
    placeAt.place = plaza
    layerData.favourite.mockResolvedValue({ favorites: 1, user_favorite: true })
    await renderAt()
    const heart = await screen.findByRole('button', { name: 'Favorite', pressed: false })
    await waitFor(() => expect(heart).toBeEnabled())
    fireEvent.click(heart)
    expect(heart).toHaveAttribute('aria-pressed', 'true')
    expect(layerData.favourite).toHaveBeenCalledWith('p', true)
    await waitFor(() => expect(heart).toHaveAttribute('aria-pressed', 'true'))
  })

  it('goes back when the service refuses', async () => {
    placeAt.place = plaza
    layerData.favourite.mockRejectedValue(new Error('refused'))
    await renderAt()
    const heart = await screen.findByRole('button', { name: 'Favorite' })
    await waitFor(() => expect(heart).toBeEnabled())
    fireEvent.click(heart)
    await waitFor(() => expect(heart).toHaveAttribute('aria-pressed', 'false'))
  })
})

describe('minimap place under the player', () => {
  const plaza = { id: 'p', title: 'Plaza', positions: ['0,0', '1,0'], base_position: '0,0', user_favorite: false }

  it('keeps the place, and asks nothing, while the player crosses parcels inside it', async () => {
    placeAt.place = plaza
    const { fetchPlaceAt } = await import('@explorer-core/features/map/mapApi')
    vi.mocked(fetchPlaceAt).mockClear()
    const session = fakeSession()
    const view = await renderSettled(<Minimap minimap={{ ...session.minimap, linkRealm: DEFAULT_REALM }} map={{ ...session.map, x: 0, y: 0 }} sceneTitle="" setEngineViewport={vi.fn()} />)
    await waitFor(() => expect(screen.getByRole('button', { name: 'Favorite' })).toBeEnabled())
    view.rerender(<Minimap minimap={{ ...session.minimap, linkRealm: DEFAULT_REALM }} map={{ ...session.map, x: 1, y: 0 }} sceneTitle="" setEngineViewport={vi.fn()} />)
    expect(screen.getByRole('button', { name: 'Favorite' })).toBeEnabled()
    expect(fetchPlaceAt).toHaveBeenCalledTimes(1)
  })
})

