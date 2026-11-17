import { describe, it, expect, vi, beforeEach } from 'vitest'
import { act, fireEvent, screen } from '@testing-library/react'
import { dispatchCancelLayer } from '@explorer-core/lib/cancelLayers'
import { DEFAULT_REALM } from '@hud-host'
import { Minimap } from '../../features/minimap/Minimap'
import { fakeSession, renderSettled } from '../harness'

const placeAt = vi.hoisted(() => ({ place: null as unknown }))
vi.mock('@explorer-core/features/map/mapApi', async (original) => ({
  ...await original<typeof import('@explorer-core/features/map/mapApi')>(),
  fetchPlaceAt: vi.fn(async () => placeAt.place)
}))

vi.mock('@explorer-core/features/minimap/minimapPlaces', async (original) => ({
  ...await original<typeof import('@explorer-core/features/minimap/minimapPlaces')>(),
  loadMinimapPlaces: vi.fn(async () => [])
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
    { at = { x: 10, y: 20 }, home = null, referrer }: { at?: { x: number; y: number }; home?: { realm: string | null; parcel: string } | null; referrer?: string } = {}
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
    expect(session.map.changeRealm).toHaveBeenCalledWith(DEFAULT_REALM)
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

  it('reads on, and cannot be turned off, where the player stands in the home place', async () => {
    placeAt.place = { id: 'p', title: 'Plaza', positions: ['10,20', '11,20'], base_position: '10,20' }
    const session = await openMenu(DEFAULT_REALM, { at: { x: 11, y: 20 }, home: { realm: null, parcel: '10,20' } })
    const toggle = await screen.findByRole('switch', { name: 'Set as Home', checked: true })
    fireEvent.click(toggle)
    expect(toggle).toHaveAttribute('aria-checked', 'true')
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

  it("copies a Genesis City parcel's link with the player's address", async () => {
    const writeText = vi.fn(async () => {})
    Object.assign(navigator, { clipboard: { writeText } })
    await openMenu(DEFAULT_REALM, { at: { x: 3, y: 4 }, referrer: '0xabc' })
    fireEvent.click(screen.getByRole('menuitem', { name: 'Copy Link' }))
    expect(writeText).toHaveBeenCalledWith(expect.stringMatching(/\/jump\/\?position=3,4&referrer=0xabc$/))
  })

  it("copies a World's link with its parcel and the player's address", async () => {
    const writeText = vi.fn(async () => {})
    Object.assign(navigator, { clipboard: { writeText } })
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
    session.map = { ...session.map, home: { realm: null, parcel: '1,2' } }
    const { container } = await renderSettled(<Minimap minimap={session.minimap} map={session.map} sceneTitle="" setEngineViewport={vi.fn()} />)
    expect(container.querySelector('[data-marker="home"]')).not.toBeNull()
  })

  it('draws none for a World home', async () => {
    const session = fakeSession()
    session.map = { ...session.map, home: { realm: 'boedo.dcl.eth', parcel: '0,0' } }
    const { container } = await renderSettled(<Minimap minimap={session.minimap} map={session.map} sceneTitle="" setEngineViewport={vi.fn()} />)
    expect(container.querySelector('[data-marker="home"]')).toBeNull()
  })
})
