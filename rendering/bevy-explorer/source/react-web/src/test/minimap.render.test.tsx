import { describe, it, expect, vi, beforeEach } from 'vitest'
import { screen } from '@testing-library/react'
import { Minimap } from '@hud/features/minimap/Minimap'
import { fakeSession, renderSettled } from './harness'

vi.mock('@hud/features/minimap/minimapPlaces', async (original) => ({
  ...await original<typeof import('@hud/features/minimap/minimapPlaces')>(),
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
