import { describe, it, expect } from 'vitest'
import { parcelsChunks, satelliteChunkCenter } from '../features/map/atlas'
import { clusterCellSize, clusterMarkers } from '../features/map/clusters'
import { parcelCenter, parcelWithinBounds, playerMarkerPosition, positionToCoords } from '../features/map/mapCoords'
import { createMapLayersStore } from '../features/map/mapLayersStore'
import { liveEventMarkers, poiMarkers, resultMarkers } from '../features/map/mapMarkers'
import type { MapEvent, MapPlace } from '../features/map/mapApi'
import {
  BASE_ZOOM,
  DEFAULT_ZOOM_LEVEL,
  cameraBounds,
  clampCamera,
  mapToScreen,
  orthoForLevel,
  pinMarkerScale,
  scaleForZoom,
  screenToMap,
  titleForScale,
  userMarkerScale
} from '../features/map/mapZoom'
import { parcelsCenter } from '../features/map/parcelsCenter'

describe('map coordinates', () => {
  it('centres a parcel half a parcel inside its far corner', () => {
    expect(parcelCenter({ x: 0, y: 0 })).toEqual({ x: -10, y: -10 })
    expect(parcelCenter({ x: 3, y: -2 })).toEqual({ x: 50, y: -50 })
  })

  it('maps a position back to the parcel that covers it', () => {
    expect(positionToCoords({ x: -10, y: -10 })).toEqual({ x: -0, y: -0 })
    expect(positionToCoords({ x: 0.5, y: 20 })).toEqual({ x: 1, y: 1 })
    expect(positionToCoords(parcelCenter({ x: -47, y: 112 }))).toEqual({ x: -47, y: 112 })
  })

  it('puts the player in the middle of the parcel they stand in the middle of', () => {
    // parcel (2, 3) spans world metres 32..48 east, 48..64 north
    expect(playerMarkerPosition(40, 56)).toEqual({ x: 30, y: 50 })
  })

  it('takes parcels inside the city and its two extra strips, and nothing outside', () => {
    expect(parcelWithinBounds(parcelCenter({ x: 150, y: 150 }))).toEqual({ x: 150, y: 150 })
    expect(parcelWithinBounds(parcelCenter({ x: 100, y: 155 }))).toEqual({ x: 100, y: 155 })
    expect(parcelWithinBounds(parcelCenter({ x: 160, y: 100 }))).toEqual({ x: 160, y: 100 })
    expect(parcelWithinBounds(parcelCenter({ x: 20, y: 155 }))).toBeNull()
    expect(parcelWithinBounds(parcelCenter({ x: -151, y: 0 }))).toBeNull()
  })
})

describe('map zoom', () => {
  it('opens on the middle level and spans 157 to 1 parcel of half-height', () => {
    expect(DEFAULT_ZOOM_LEVEL).toBe(3)
    expect(orthoForLevel(0)).toBe(3140)
    expect(orthoForLevel(6)).toBe(20)
    expect(orthoForLevel(3)).toBeCloseTo(1370, 0)
  })

  it('keeps a marker one parcel wide at the base zoom and the same size on screen beyond it', () => {
    expect(scaleForZoom(20, BASE_ZOOM)).toBe(20)
    const far = orthoForLevel(0)
    expect(scaleForZoom(20, far) / far).toBeCloseTo(20 / BASE_ZOOM)
  })

  it('shrinks a title on screen as the view pulls back', () => {
    const at = titleForScale(-1.2, 1, 20, 20)
    expect(at).toEqual({ y: -1.2, scale: 1 })
    const far = titleForScale(-1.2, 1, 20, 200)
    expect(far.scale).toBeCloseTo(0.1)
  })

  it('holds other players at map size until closer than the base zoom', () => {
    expect(userMarkerScale(orthoForLevel(0))).toBe(20)
    expect(userMarkerScale(BASE_ZOOM / 2)).toBe(10)
  })

  it('never lets a scene pin fall under its minimum past the base zoom', () => {
    expect(pinMarkerScale(BASE_ZOOM)).toBe(35)
    expect(pinMarkerScale(orthoForLevel(5))).toBe(35)
    expect(pinMarkerScale(orthoForLevel(0))).toBeCloseTo(235.5, 1)
    expect(pinMarkerScale(BASE_ZOOM / 2)).toBe(17.5)
  })

  it('round-trips a point between the map and the screen', () => {
    const view = { x: 120, y: -340, ortho: 803, width: 1920, height: 1010 }
    const screen = mapToScreen({ x: 400, y: 100 }, view)
    const back = screenToMap(screen, view)
    expect(back.x).toBeCloseTo(400)
    expect(back.y).toBeCloseTo(100)
    expect(mapToScreen({ x: view.x, y: view.y }, view)).toEqual({ x: 960, y: 505 })
  })

  it('keeps the camera within the world at a close zoom', () => {
    const ortho = orthoForLevel(5)
    const bounds = cameraBounds(ortho, 1920 / 1010)
    expect(clampCamera({ x: 99999, y: -99999 }, ortho, 1920 / 1010)).toEqual({ x: bounds.xMax, y: bounds.yMin })
    expect(clampCamera({ x: 0, y: 0 }, ortho, 1920 / 1010)).toEqual({ x: 0, y: 0 })
  })
})

describe('parcel centre of a place', () => {
  it('is the only parcel of a one-parcel place', () => {
    expect(parcelsCenter([{ x: 5, y: 7 }])).toEqual({ x: 5, y: 7 })
  })

  it('depends on the order the parcels come in', () => {
    const row = [
      { x: 0, y: 0 },
      { x: 1, y: 0 },
      { x: 2, y: 0 },
      { x: 3, y: 0 },
      { x: 4, y: 0 }
    ]
    // from the west end the walk never leaves the first parcel; from the east end it reaches the middle
    expect(parcelsCenter(row)).toEqual({ x: 0, y: 0 })
    expect(parcelsCenter([...row].reverse())).toEqual({ x: 2, y: 0 })
  })
})

describe('clusters', () => {
  const marker = (x: number, y: number): { key: { x: number; y: number }; position: { x: number; y: number } } => ({
    key: { x, y },
    position: parcelCenter({ x, y })
  })

  it('uses the cell size of each zoom level', () => {
    expect([0, 1, 2, 3, 4, 5, 6, 7].map(clusterCellSize)).toEqual([35, 25, 27, 12, 7, 4, 1, 1])
  })

  it('leaves lone markers alone and counts the ones that share a cell', () => {
    const { visible, clusters } = clusterMarkers([marker(0, 0), marker(3, 3), marker(40, 40)], 3)
    expect(visible).toHaveLength(1)
    expect(visible[0].key).toEqual({ x: 40, y: 40 })
    expect(clusters).toHaveLength(1)
    expect(clusters[0].count).toBe(2)
  })

  it('draws a cluster half a parcel off the average of its markers', () => {
    const { clusters } = clusterMarkers([marker(0, 0), marker(2, 2)], 3)
    expect(clusters[0].position).toEqual({ x: 0, y: 0 })
  })

  it('shows every marker apart at the closest zoom', () => {
    const { visible, clusters } = clusterMarkers([marker(0, 0), marker(1, 0)], 6)
    expect(visible).toHaveLength(2)
    expect(clusters).toHaveLength(0)
  })
})

describe('marker layers', () => {
  const place = (id: string, title: string, positions: string[], world = ''): MapPlace => ({ id, title, positions, base_position: positions[0], world_name: world })
  const event = (id: string, x: number, y: number, world = false): MapEvent => ({ id, name: `Event ${id}`, x, y, world, start_at: '' })

  it('marks one live event per parcel and none in a world', () => {
    const markers = liveEventMarkers([event('a', 1, 2), event('b', 1, 2), event('c', 9, 9, true)])
    expect(markers.map((m) => m.event.id)).toEqual(['a'])
    expect(markers[0].position).toEqual({ x: 10, y: 30 })
  })

  it('cuts a long title to 29 characters', () => {
    const markers = poiMarkers([place('p', 'A very long place name that keeps going', ['0,0'])])
    expect(markers[0].title).toBe('A very long place name that k')
  })

  it('skips empty parcels and a second point of interest on the same centre', () => {
    const markers = poiMarkers([place('a', 'One', ['4,4']), place('b', 'Two', ['4,4']), place('c', 'Empty parcel', ['8,8'])])
    expect(markers.map((m) => m.place.id)).toEqual(['a'])
  })

  it('keys a result by its base parcel but draws it on its centre parcel, and leaves worlds out', () => {
    const markers = resultMarkers([place('a', 'Row', ['2,0', '1,0', '0,0']), place('w', 'World', ['0,0'], 'w.dcl.eth')])
    expect(markers).toHaveLength(1)
    expect(markers[0].key).toEqual({ x: 2, y: 0 })
    expect(markers[0].position).toEqual(parcelCenter({ x: 1, y: 0 }))
  })
})

describe('map layers store', () => {
  it('starts with the pins, people and satellite on and parcels off', () => {
    const store = createMapLayersStore()
    expect([...store.getSnapshot()].sort()).toEqual(['liveEvents', 'people', 'pins', 'pois', 'satellite'])
  })

  it('tells subscribers of a change and keeps one map type on', () => {
    const store = createMapLayersStore()
    let calls = 0
    store.subscribe(() => calls++)
    store.set('people', false)
    store.set('people', false)
    expect(calls).toBe(1)
    store.setMapType('parcels')
    expect(store.getSnapshot().has('parcels')).toBe(true)
    expect(store.getSnapshot().has('satellite')).toBe(false)
  })
})

describe('atlases', () => {
  it('lays the parcels map as seven by seven chunks centred from -150', () => {
    const chunks = parcelsChunks()
    expect(chunks).toHaveLength(49)
    expect(chunks[0]).toMatchObject({ x: -150, y: -150 })
    expect(chunks[0].url).toContain('/v1/map.png?center=-150,-150&width=1020&height=1020&size=20')
    expect(chunks[48]).toMatchObject({ x: 156, y: 156 })
  })

  it('centres the first satellite chunk three west and two north of the corner chunk', () => {
    expect(satelliteChunkCenter(0, 0)).toEqual({ x: -133, y: 132 })
    expect(satelliteChunkCenter(7, 7)).toEqual({ x: 147, y: -148 })
  })
})
