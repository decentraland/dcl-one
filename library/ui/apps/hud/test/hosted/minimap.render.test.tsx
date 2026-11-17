import { describe, it, expect, vi, beforeEach } from 'vitest'
import { fireEvent, screen } from '@testing-library/react'
import { DEFAULT_REALM } from '@hud-host'
import { Minimap } from '../../features/minimap/Minimap'
import { fakeSession, renderSettled } from '../harness'

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

beforeEach(() => localStorage.clear())

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
  async function openMenu(linkRealm: string | null): Promise<ReturnType<typeof fakeSession>> {
    const session = fakeSession()
    await renderSettled(<Minimap minimap={{ ...session.minimap, linkRealm }} map={session.map} sceneTitle="" setEngineViewport={vi.fn()} />)
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
})
