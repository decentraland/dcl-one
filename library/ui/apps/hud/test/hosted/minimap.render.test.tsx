import { describe, it, expect, vi, beforeEach } from 'vitest'
import { fireEvent, screen } from '@testing-library/react'
import { Minimap } from '../../features/minimap/Minimap'
import { fakeSession, renderSettled } from '../harness'

vi.mock('@explorer-core/features/minimap/minimapPlaces', async (original) => ({
  ...await original<typeof import('@explorer-core/features/minimap/minimapPlaces')>(),
  loadMinimapPlaces: vi.fn(async () => [])
}))

// The minimap header: the scene name comes from the sceneInfo stream (see world.test.tsx for
// the wiring); this covers what the header actually renders, including the empty-parcel case.
async function renderMinimap(sceneTitle: string): Promise<void> {
  const session = fakeSession()
  await renderSettled(<Minimap minimap={session.minimap} map={session.map} sceneTitle={sceneTitle} setEngineViewport={vi.fn()} />)
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
})

describe('minimap menu', () => {
  async function openMenu(donatePlace: { parcel: { x: number; y: number } } | null): Promise<void> {
    const session = fakeSession()
    await renderSettled(<Minimap minimap={{ ...session.minimap, donatePlace }} map={session.map} sceneTitle="Genesis Plaza" setEngineViewport={vi.fn()} />)
    fireEvent.click(screen.getByRole('button', { name: 'Minimap settings' }))
  }

  it('offers Copy Link where the place has a link', async () => {
    await openMenu({ parcel: { x: 0, y: 0 } })
    expect(screen.getByRole('menuitem', { name: 'Copy Link' })).toBeEnabled()
  })

  it('leaves Copy Link out where the place has none', async () => {
    await openMenu(null)
    expect(screen.getByRole('menu', { name: 'Minimap settings' })).toBeInTheDocument()
    expect(screen.queryByRole('menuitem', { name: 'Copy Link' })).toBeNull()
  })
})
