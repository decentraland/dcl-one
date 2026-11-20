import { describe, it, expect, vi, beforeEach } from 'vitest'
import { fireEvent, screen, within } from '@testing-library/react'
import { MapPanel } from '../../features/sidebar/panels/MapPanel'
import { fakeSession, renderSettled } from '../harness'

vi.mock('@explorer-core/features/map/mapApi', async (original) => ({
  ...await original<typeof import('@explorer-core/features/map/mapApi')>(),
  fetchLiveEvents: vi.fn(async () => []),
  fetchPoiParcels: vi.fn(async () => []),
  fetchPeers: vi.fn(async () => []),
  fetchPlaceAt: vi.fn(async () => null)
}))

// The panel the navigation bar's Map button shows. It is the HUD's minimap under the name "Map":
// these cover the name, where the title comes from, and that every minimap control is still there.
async function renderPanel(sceneTitle = 'Genesis Plaza'): Promise<{ session: ReturnType<typeof fakeSession>; unmount: () => void }> {
  const session = fakeSession()
  const view = await renderSettled(
    <MapPanel minimap={{ ...session.minimap, sceneTitle }} map={session.map} setEngineViewport={vi.fn()} />
  )
  return { session, unmount: view.unmount }
}

beforeEach(() => localStorage.clear())

describe('map panel', () => {
  it('is a region named Map, titled by the scene the player stands in', async () => {
    await renderPanel('Genesis Plaza')
    expect(within(screen.getByRole('region', { name: 'Map' })).getByText('Genesis Plaza')).toBeInTheDocument()
  })

  it('keeps every minimap control inside the region', async () => {
    await renderPanel()
    const panel = within(screen.getByRole('region', { name: 'Map' }))
    expect(panel.getByRole('button', { name: 'Collapse minimap' })).toHaveAttribute('aria-expanded', 'true')
    expect(panel.getByRole('button', { name: 'Open map' })).toBeInTheDocument()
    expect(panel.getByRole('button', { name: 'Minimap settings' })).toHaveAttribute('aria-expanded', 'false')
    const zoom = within(panel.getByRole('group', { name: 'Minimap zoom' }))
    expect(zoom.getByRole('button', { name: 'Zoom in' })).toBeInTheDocument()
    expect(zoom.getByRole('button', { name: 'Zoom out' })).toBeInTheDocument()
  })

  it('opens the map page from the map', async () => {
    const { session } = await renderPanel()
    fireEvent.click(screen.getByRole('button', { name: 'Open map' }))
    expect(session.map.toggle).toHaveBeenCalledTimes(1)
  })

  it('opens the minimap settings from the menu button', async () => {
    await renderPanel()
    fireEvent.click(screen.getByRole('button', { name: 'Minimap settings' }))
    expect(within(screen.getByRole('region', { name: 'Map' })).getByRole('menu', { name: 'Minimap settings' })).toBeInTheDocument()
  })

  it('lays out a header row, the parcel and the square map, and leaves the map out collapsed', async () => {
    await renderPanel('Genesis Plaza')
    const panel = within(screen.getByRole('region', { name: 'Map' }))
    const collapse = panel.getByRole('button', { name: 'Collapse minimap' })
    const title = panel.getByText('Genesis Plaza')
    const menu = panel.getByRole('button', { name: 'Minimap settings' })
    const parcel = panel.getByText('0,0')
    const map = panel.getByRole('button', { name: 'Open map' })
    const zoom = panel.getByRole('group', { name: 'Minimap zoom' })

    // the header row holds the collapse button, the title and the menu; the parcel is a row of its own
    const header = collapse.parentElement!
    expect(header).toContainElement(title)
    expect(header).toContainElement(menu)
    expect(header).not.toContainElement(parcel)
    // top to bottom: header, parcel, map
    const inOrder = [collapse, title, menu, parcel, map, zoom]
    inOrder.slice(1).forEach((el, i) => {
      expect(inOrder[i]!.compareDocumentPosition(el) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy()
    })
    // the four compass badges stand on the map
    expect(['N', 'E', 'S', 'W'].map((point) => within(map).getByText(point).dataset.point)).toEqual(['N', 'E', 'S', 'W'])

    fireEvent.click(collapse)
    expect(panel.getByRole('button', { name: 'Expand minimap' })).toBeInTheDocument()
    expect(panel.getByText('Genesis Plaza')).toBeInTheDocument()
    expect(panel.getByText('0,0')).toBeInTheDocument()
    expect(panel.getByRole('button', { name: 'Minimap settings' })).toBeInTheDocument()
    expect(panel.queryByRole('button', { name: 'Open map' })).toBeNull()
    expect(panel.queryByRole('group', { name: 'Minimap zoom' })).toBeNull()
    expect(panel.queryByText('N')).toBeNull()
  })

  it('collapses to the header and the parcel, and stays collapsed when shown again', async () => {
    const first = await renderPanel()
    fireEvent.click(screen.getByRole('button', { name: 'Collapse minimap' }))
    expect(screen.queryByRole('button', { name: 'Open map' })).toBeNull()
    expect(screen.getByRole('button', { name: 'Expand minimap' })).toHaveAttribute('aria-expanded', 'false')
    expect(screen.getByText('Genesis Plaza')).toBeInTheDocument()

    // Hidden by the Map button and shown again: the panel remounts and reads the stored state.
    first.unmount()
    await renderPanel()
    expect(screen.getByRole('button', { name: 'Expand minimap' })).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: 'Open map' })).toBeNull()
  })
})
