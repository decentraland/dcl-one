import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { LIVE_EVENTS_POLL_MS, createMapLayerDataStore } from '../features/map/mapLayerData'
import { fetchLiveEvents, fetchPeers, fetchPoiParcels } from '../features/map/mapApi'
import { nearbyPlayers, travelCount } from '../features/map/mapPeople'

vi.mock('../features/map/mapApi', () => ({
  fetchLiveEvents: vi.fn(async () => []),
  fetchPoiParcels: vi.fn(async () => []),
  fetchPlacesAt: vi.fn(async () => []),
  fetchPeers: vi.fn(async () => [])
}))

beforeEach(() => {
  vi.useFakeTimers()
  vi.clearAllMocks()
})
afterEach(() => vi.useRealTimers())

describe('map layer data, shared by the maps', () => {
  it('reads live events once for two maps, polls once, and stops when both go', () => {
    const store = createMapLayerDataStore()
    const fullMap = store.want('liveEvents')
    const minimap = store.want('liveEvents')
    expect(fetchLiveEvents).toHaveBeenCalledTimes(1)
    vi.advanceTimersByTime(LIVE_EVENTS_POLL_MS)
    expect(fetchLiveEvents).toHaveBeenCalledTimes(2)
    fullMap()
    vi.advanceTimersByTime(LIVE_EVENTS_POLL_MS)
    expect(fetchLiveEvents).toHaveBeenCalledTimes(3)
    minimap()
    vi.advanceTimersByTime(LIVE_EVENTS_POLL_MS * 3)
    expect(fetchLiveEvents).toHaveBeenCalledTimes(3)
  })

  it('reads points of interest once while any map holds them', () => {
    const store = createMapLayerDataStore()
    const a = store.want('pois')
    const b = store.want('pois')
    expect(fetchPoiParcels).toHaveBeenCalledTimes(1)
    a()
    b()
    store.want('pois')
    expect(fetchPoiParcels).toHaveBeenCalledTimes(2)
  })

  it('follows the players while any map shows them, and reads them again after a teleport', () => {
    const watch = vi.spyOn(nearbyPlayers, 'watch')
    const store = createMapLayerDataStore()
    const a = store.want('people')
    const b = store.want('people')
    expect(watch).toHaveBeenCalledTimes(1)
    expect(fetchPeers).toHaveBeenCalledTimes(1)
    travelCount.traveled()
    expect(fetchPeers).toHaveBeenCalledTimes(2)
    a()
    expect(watch).toHaveBeenLastCalledWith(true)
    b()
    expect(watch).toHaveBeenLastCalledWith(false)
    travelCount.traveled()
    expect(fetchPeers).toHaveBeenCalledTimes(2)
  })
})
