import { describe, it, expect, beforeEach } from 'vitest'
import type { DiscoverPlace } from '../features/places/placesApi'
import { GENESIS_SATELLITE_VIEW, isGenesisSatelliteView } from '../features/map/atlas'
import {
  DEFAULT_VISIBLE_METERS,
  MAX_VISIBLE_METERS,
  MIN_VISIBLE_METERS,
  loadOpen,
  loadRotation,
  loadStyle,
  loadZoom,
  saveOpen,
  saveRotation,
  saveStyle,
  saveZoom
} from '../features/minimap/minimapPrefs'
import { genesisStartParcel, minimapHomeTarget } from '../features/minimap/minimapHome'
import { isPlacePanelHome, isStandingInHome } from '../features/map/home'
import { jumpInLink } from '../features/map/shareLinks'
import { DEFAULT_REALM } from '@hud-host'
import { edgePoint, mapUnitsToMeters, offMap } from '../features/minimap/minimapLayers'
import { parcelCenter, playerMarkerPosition } from '../features/map/mapCoords'

beforeEach(() => localStorage.clear())

describe('minimap prefs — defaults and validation', () => {
  it('falls back to a renderable default when nothing is stored', () => {
    expect(loadStyle()).toBe('satellite')
    expect(loadRotation()).toBe('north')
    expect(loadZoom()).toBe(DEFAULT_VISIBLE_METERS)
  })

  it('round-trips a valid value', () => {
    saveStyle('imposters')
    saveRotation('camera')
    saveZoom(128)
    expect(loadStyle()).toBe('imposters')
    expect(loadRotation()).toBe('camera')
    expect(loadZoom()).toBe(128)
  })

  it('rejects a stale or hand-edited enum rather than rendering nothing', () => {
    localStorage.setItem('dcl-minimap-style', 'hologram')
    localStorage.setItem('dcl-minimap-rotation', 'sideways')
    expect(loadStyle()).toBe('satellite')
    expect(loadRotation()).toBe('north')
  })
})

describe('minimap prefs — open state per realm type', () => {
  it('defaults open in Genesis City and closed in a World', () => {
    expect(loadOpen(true)).toBe(true)
    expect(loadOpen(false)).toBe(false)
  })

  it('keeps the two realm types independent', () => {
    saveOpen(true, false)
    expect(loadOpen(true)).toBe(false)
    expect(loadOpen(false)).toBe(false)
    saveOpen(false, true)
    expect(loadOpen(false)).toBe(true)
    expect(loadOpen(true)).toBe(false)
  })

  it('falls back to the default on a hand-edited value', () => {
    localStorage.setItem('dcl-minimap-open', 'maybe')
    localStorage.setItem('dcl-minimap-open-world', '1')
    expect(loadOpen(true)).toBe(true)
    expect(loadOpen(false)).toBe(false)
  })
})

describe('genesis satellite view', () => {
  it('matches on the url of the map Genesis City advertises', () => {
    expect(isGenesisSatelliteView(GENESIS_SATELLITE_VIEW)).toBe(true)
    expect(isGenesisSatelliteView({ ...GENESIS_SATELLITE_VIEW, topLeftOffset: { x: 0, y: 0 } })).toBe(true)
    expect(isGenesisSatelliteView(null)).toBe(false)
    expect(isGenesisSatelliteView({ ...GENESIS_SATELLITE_VIEW, baseUrl: 'https://example.com/map' })).toBe(false)
  })
})

describe('minimap prefs — zoom clamping', () => {
  it('clamps a stored value that outlived a change to the range', () => {
    localStorage.setItem('dcl-minimap-zoom', '4')
    expect(loadZoom()).toBe(MIN_VISIBLE_METERS)
    localStorage.setItem('dcl-minimap-zoom', '99999')
    expect(loadZoom()).toBe(MAX_VISIBLE_METERS)
  })

  it('falls back on a non-numeric or non-positive value', () => {
    for (const stored of ['not-a-number', '0', '-100', '']) {
      localStorage.setItem('dcl-minimap-zoom', stored)
      expect(loadZoom()).toBe(DEFAULT_VISIBLE_METERS)
    }
  })
})

describe('minimap layers', () => {
  it("puts the full map's positions back in world metres", () => {
    expect(mapUnitsToMeters(parcelCenter({ x: 3, y: -2 }))).toEqual({ x: 3 * 16 + 8, z: -2 * 16 + 8 })
    expect(mapUnitsToMeters(playerMarkerPosition(40, -24))).toEqual({ x: 40, z: -24 })
  })

  it('puts a destination on the edge only once it is off the square map', () => {
    expect(offMap(119, -119, 120)).toBe(false)
    expect(offMap(0, 121, 120)).toBe(true)
    expect(offMap(-130, 10, 120)).toBe(true)
  })

  it('places a direction on the square edge', () => {
    expect(edgePoint(10, 0)).toEqual({ x: 1, y: 0 })
    expect(edgePoint(-5, 5)).toEqual({ x: -1, y: 1 })
    expect(edgePoint(3, -6)).toEqual({ x: 0.5, y: -1 })
    expect(edgePoint(0, 0)).toEqual({ x: 0, y: 0 })
  })
})

describe('minimap Set as Home', () => {
  const plaza = { id: 'p', title: 'Plaza', positions: ['10,20', '11,20'], base_position: '10,20' } as DiscoverPlace
  const road = { id: 'r', title: 'Long Road', positions: ['10,20', '11,20'], base_position: '10,20' } as DiscoverPlace

  it('makes home the parcel in Genesis City, the World by name elsewhere', () => {
    expect(minimapHomeTarget(DEFAULT_REALM, { x: 3, y: 4 })).toEqual({ realm: DEFAULT_REALM, parcel: { x: 3, y: 4 } })
    expect(minimapHomeTarget('boedo.dcl.eth', { x: 3, y: 4 })).toEqual({ realm: 'boedo.dcl.eth', parcel: { x: 0, y: 0 } })
  })

  it('jumps back to the Genesis City home parcel, or to Genesis Plaza without one', () => {
    expect(genesisStartParcel({ parcel: { x: 10, y: 20 }, world: null })).toEqual({ x: 10, y: 20 })
    expect(genesisStartParcel({ parcel: null, world: 'boedo.dcl.eth' })).toEqual({ x: 0, y: 0 })
    expect(genesisStartParcel({ parcel: null, world: null })).toEqual({ x: 0, y: 0 })
  })

  it('counts any parcel of the home place as home', () => {
    expect(isStandingInHome({ parcel: { x: 10, y: 20 }, world: null }, null, { x: 11, y: 20 }, plaza)).toBe(true)
  })

  it('on a road or with no place, counts only the home parcel itself', () => {
    const home = { parcel: { x: 10, y: 20 }, world: null }
    expect(isStandingInHome(home, null, { x: 11, y: 20 }, road)).toBe(false)
    expect(isStandingInHome(home, null, { x: 10, y: 20 }, road)).toBe(true)
    expect(isStandingInHome(home, null, { x: 10, y: 20 }, null)).toBe(true)
  })

  it("differs from the place panel's rule on roads and on World names, on purpose", () => {
    const home = { parcel: { x: 11, y: 20 }, world: null }
    // the panel counts a road home when any of its parcels is; the minimap only at the home parcel
    expect(isPlacePanelHome(home, road, { x: 10, y: 20 })).toBe(true)
    expect(isStandingInHome(home, null, { x: 10, y: 20 }, road)).toBe(false)
    const world = { parcel: null, world: 'Boedo.dcl.eth' }
    const boedo = { id: 'w', title: 'Boedo', world_name: 'boedo.dcl.eth', positions: [], base_position: '0,0' } as unknown as DiscoverPlace
    expect(isPlacePanelHome(world, boedo, null)).toBe(false)
    expect(isStandingInHome(world, 'boedo.dcl.eth', { x: 0, y: 0 }, null)).toBe(true)
  })

  it('in a World, compares its name without case', () => {
    expect(isStandingInHome({ parcel: null, world: 'Boedo.dcl.eth' }, 'boedo.dcl.eth', { x: 0, y: 0 }, null)).toBe(true)
    expect(isStandingInHome({ parcel: { x: 0, y: 0 }, world: null }, 'boedo.dcl.eth', { x: 0, y: 0 }, null)).toBe(false)
  })
})

describe('minimap jump link', () => {
  it("names a Genesis City parcel, or a World and its parcel, with the sharer's address", () => {
    expect(jumpInLink(3, 4, '0xabc', DEFAULT_REALM)).toMatch(/\/jump\/\?position=3,4&referrer=0xabc$/)
    expect(jumpInLink(3, 4, undefined, 'boedo.dcl.eth')).toMatch(/\/jump\/\?realm=boedo\.dcl\.eth&position=3,4$/)
  })
})
